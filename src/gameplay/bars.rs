// SPDX-License-Identifier: MIT

//! Bar-position math: time-signature parsing, bar-index arithmetic, and the
//! per-frame [`CurrentBar`]/[`AbsoluteBar`] tracker shared by
//! `twelve_bar_blues_overlay` and `jam_session`.

use bevy::prelude::*;

use crate::app::SelectedSong;
use crate::song::SongManifest;

use super::clock::GameplayClock;
use super::state::ScoringConfig;

/// Parse the beat count from an optional "N/D" time-signature string.
pub fn parse_beats(time_sig: Option<&str>) -> f64 {
    time_sig
        .and_then(|s| s.split('/').next())
        .and_then(|n| n.parse::<f64>().ok())
        .unwrap_or(4.0)
}

/// Seconds per bar given BPM and beat count.
pub fn secs_per_bar(bpm: f64, beats: f64) -> f64 {
    (60.0 / bpm) * beats
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
/// `twelve_bar_blues_overlay::update_bar` and `jam_session::update_hole_map`
/// don't each recompute it (previously from two different beats-per-bar
/// sources that could disagree: `ScoringConfig::beats_per_bar`, which honors
/// a chart's `time_signature_map` override, vs `JamHoleGuide`'s own copy,
/// which didn't).
#[derive(Resource, Default)]
pub struct CurrentBar(pub usize);

/// [`absolute_bar_index`]'s result, tracked the same frame as [`CurrentBar`]
/// — `jam_session`'s phrase-discipline lesson primitive needs a play/rest
/// bar pattern that repeats consistently across an open-ended jam, not one
/// that resets every 12 bars the way `CurrentBar` does.
#[derive(Resource, Default)]
pub struct AbsoluteBar(pub usize);

/// Emitted by [`track_current_bar`] whenever the current bar changes,
/// forward or (on a loop rewind) backward — lets `update_bar` recolor the
/// 12-bar grid only on an actual bar change instead of writing
/// `BackgroundColor` on all 12 cells every frame forever. `update_hole_map`
/// doesn't need this: it repaints every frame anyway for live mic feedback,
/// so it just reads `CurrentBar` directly.
#[derive(Message)]
pub struct BarChanged(pub usize);

/// Computes the current bar once per frame (see `GameplayLogic` — must run
/// after `clock::handle_loop_boundary` so a loop rewind is reflected the
/// same frame) and emits [`BarChanged`] on a change, detected by recomputing
/// from the clock each frame rather than advancing an incrementing counter
/// — the same trick `phrase_overlay::watch_phrase_boundaries` uses so a
/// backward jump is picked up for free instead of needing special-case
/// handling.
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
    let spb = secs_per_bar(bpm, config.beats_per_bar);
    let bar = current_bar_index(clock.get(), spb);
    current.0 = bar;
    absolute.0 = absolute_bar_index(clock.get(), spb);
    if *last != Some(bar) {
        changed.write(BarChanged(bar));
    }
    *last = Some(bar);
}
