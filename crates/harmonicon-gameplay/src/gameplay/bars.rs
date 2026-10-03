// SPDX-License-Identifier: MIT

//! Bar-position math: which meter a chart is in, bar-index arithmetic, and
//! the per-frame [`CurrentBar`]/[`AbsoluteBar`] tracker shared by
//! `twelve_bar_blues_overlay` and `jam::session`.

use bevy::prelude::*;

use harmonicon_app::app::SelectedSong;
use harmonicon_core::chart::{HarpChart, tick_to_seconds, time_sig_at_tick};
use harmonicon_song::song::SongManifest;
use harmonicon_ui::music_score::{MusicScoreMeter, parse_time_signature};

use super::clock::GameplayClock;
use super::state::ScoringConfig;

/// The meter a chart is in — **the one place gameplay reads it from.**
///
/// Every bar-length figure in gameplay, Jam Session and the metronome
/// derives from the `MusicScoreMeter` this returns, through its own
/// methods (`beats_per_bar` for quarter-note beats, `numerator` for the
/// meter's own, `bar_secs` for seconds). Don't derive one yourself: taking
/// the numerator as a beat count, say, makes 6/8 accent every second bar
/// and walks the accent around a 3/8 bar.
///
/// A `timing.time_signature_map` entry at tick 0 wins over the song-level
/// field.
pub fn chart_meter(chart: &HarpChart) -> MusicScoreMeter {
    let sig = chart
        .timing
        .time_signature_map
        .as_deref()
        .and_then(|m| time_sig_at_tick(0, m))
        .or(chart.song.time_signature.as_deref())
        .unwrap_or("4/4");
    parse_time_signature(sig)
}

/// How many ticks one beat of `meter` lasts, given the chart's `resolution`
/// (ticks per quarter note).
///
/// The meter's *own* beat, not a quarter note — an eighth in 6/8, matching
/// `MusicScoreMeter::beat_secs`'s `60/bpm × 4/denominator`. Reading the
/// numerator alone and calling it a beat count is the mistake
/// [`chart_meter`]'s comment documents; this is the tick-space sibling of
/// that rule.
pub fn ticks_per_beat(resolution: u32, meter: &MusicScoreMeter) -> u64 {
    let denominator = u64::from(meter.denominator.max(1));
    (u64::from(resolution) * 4 / denominator).max(1)
}

/// How far before tick 0 the chart's bar grid starts, in ticks: the part of
/// bar 0 a pickup leaves out. Zero without a pickup, or for one a whole bar
/// long. Every beat and bar count in gameplay is taken on a clock moved on
/// by this, so bar 1's downbeat lands where the chart's pickup ends.
pub fn pickup_lead_ticks(chart: &HarpChart) -> u64 {
    let meter = chart_meter(chart);
    let bar_ticks =
        ticks_per_beat(chart.timing.resolution, &meter) * u64::from(meter.numerator.max(1));
    let pickup = chart.timing.pickup() % bar_ticks;
    (bar_ticks - pickup) % bar_ticks
}

/// Every meter beat whose tick falls in `start_tick..=end_tick`, as
/// `(tick, is_downbeat)` — a downbeat being the first beat of a bar, with
/// the grid starting `lead_ticks` before tick 0 ([`pickup_lead_ticks`]).
///
/// Tick space rather than seconds so a chart with a real tempo map gets
/// beats where the *music* has them: the caller converts each tick back with
/// `chart::tick_to_seconds`, which already honours that map. Nothing here
/// re-derives a scroll position or a second notion of "now".
pub fn beat_ticks_in_range(
    start_tick: u64,
    end_tick: u64,
    ticks_per_beat: u64,
    beats_per_bar: usize,
    lead_ticks: u64,
) -> impl Iterator<Item = (u64, bool)> {
    let beats_per_bar = beats_per_bar.max(1) as u64;
    // An empty range for a zero stride or a reversed window.
    let (first, last) = if ticks_per_beat == 0 || end_tick < start_tick {
        (1, 0)
    } else {
        (
            (start_tick + lead_ticks).div_ceil(ticks_per_beat),
            (end_tick + lead_ticks) / ticks_per_beat,
        )
    };
    (first..=last).map(move |beat| (beat * ticks_per_beat - lead_ticks, beat % beats_per_bar == 0))
}

/// How many whole bars have elapsed since the clock last hit 0 (song/jam
/// start, or a loop rewind) — unlike [`current_bar_index`], not wrapped to
/// the 12-bar cycle.
pub fn absolute_bar_index(clock: f64, secs_per_bar: f64) -> usize {
    (clock.max(0.0) / secs_per_bar) as usize
}

/// Which of the 12 bars in a twelve-bar cycle the clock is currently on.
pub fn current_bar_index(clock: f64, secs_per_bar: f64) -> usize {
    absolute_bar_index(clock, secs_per_bar) % 12
}

/// The bar `track_current_bar` last computed — shared so
/// `twelve_bar_blues_overlay::update_bar` and `jam::session::
/// update_hole_map` don't each recompute it from two different
/// beats-per-bar sources that could disagree (`ScoringConfig::
/// beats_per_bar`, which honors a chart's `time_signature_map` override,
/// vs `JamHoleGuide`'s own copy).
#[derive(Resource, Default)]
pub struct CurrentBar(pub usize);

/// [`absolute_bar_index`]'s result, tracked the same frame as [`CurrentBar`]
/// — `jam::improv`'s phrase-discipline lesson primitive needs a play/rest
/// bar pattern that repeats consistently across an open-ended jam, not one
/// that resets every 12 bars the way `CurrentBar` does.
#[derive(Resource, Default)]
pub struct AbsoluteBar(pub usize);

/// Emitted by [`track_current_bar`] whenever the current bar changes,
/// forward or (on a loop rewind) backward — lets `update_bar` recolor the
/// 12-bar grid only on an actual change instead of writing `BackgroundColor`
/// on all 12 cells every frame. `update_hole_map` doesn't need this: it
/// repaints every frame anyway for live mic feedback.
#[derive(Message)]
pub struct BarChanged(pub usize);

/// Computes the current bar once per frame (must run after `clock::
/// handle_loop_boundary` so a loop rewind is reflected the same frame) and
/// emits [`BarChanged`] on a change, detected by recomputing from the clock
/// each frame rather than an incrementing counter — the same trick
/// `phrase_overlay::watch_phrase_boundaries` uses so a backward jump needs
/// no special-case handling.
pub(crate) fn track_current_bar(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    mut current: ResMut<CurrentBar>,
    mut absolute: ResMut<AbsoluteBar>,
    mut last: Local<Option<usize>>,
    mut changed: MessageWriter<BarChanged>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let bpm = manifest.chart.song.tempo_bpm as f64;
    let spb = config.meter.bar_secs(bpm);
    // A pickup is not a bar of the form: the first bar counted is the one
    // its last note leads into.
    let timing = &manifest.chart.timing;
    let pickup_secs = tick_to_seconds(timing.pickup(), timing.resolution, &timing.tempo_map);
    let clock = clock.get() - pickup_secs;
    let bar = current_bar_index(clock, spb);
    // Written only on a change: Jam Session's labels and ending check gate on
    // these resources' `is_changed()` to mean "the bar moved".
    if current.0 != bar {
        current.0 = bar;
    }
    let absolute_bar = absolute_bar_index(clock, spb);
    if absolute.0 != absolute_bar {
        absolute.0 = absolute_bar;
    }
    if *last != Some(bar) {
        changed.write(BarChanged(bar));
    }
    *last = Some(bar);
}
