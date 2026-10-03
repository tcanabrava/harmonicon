// SPDX-License-Identifier: MIT

//! Song-lifetime plumbing: resetting score state at song start, resolving a
//! chart's scoring/loop configuration, detecting when the song's content has
//! finished, and tearing everything down on exit.

use bevy::audio::Volume;
use bevy::prelude::*;

use harmonicon_app::app::{AppState, EffectiveHarmonica, GameplayMode, SelectedSong};
use harmonicon_audio::audio_input::AudioCapture;
use harmonicon_audio::pitch_detect::{PITCH_RANGE_MARGIN_SEMITONES, PitchRange};
use harmonicon_song::song::SongManifest;

use super::bars::chart_meter;
use super::clock::GameplayClock;
use super::notes::{ScheduledNote, SongNotes, last_note_end, resolve_item_time};
use super::song_info::SongInfo;
use super::state::{
    GameplayRoot, HarmonicaPitchFilter, HitFeedback, LoopConfig, MusicPlayer, MusicStarted, Paused,
    PitchGate, PracticeRequest, Score, ScoringConfig, SongEnd, SongStats, loop_range_valid,
};
use super::wait_freeze_overlay::WaitFreezeState;
use harmonicon_platform::localization::Localization;

/// The capture stream's sample rate is fixed for as long as it is open, so
/// the filter's onset lag is resolved once here rather than recomputed per
/// frame in the judge.
pub(crate) fn configure_pitch_filter(
    selected: Res<SelectedSong>,
    effective: Res<EffectiveHarmonica>,
    manifests: Res<Assets<SongManifest>>,
    capture: Option<Res<AudioCapture>>,
    mut filter: ResMut<HarmonicaPitchFilter>,
) {
    if let Some(manifest) = manifests.get(&selected.0) {
        filter.configure(
            effective.harp_for(&manifest.chart).clone(),
            capture.map(|capture| capture.sample_rate),
        );
    }
}

pub(crate) fn reset_pitch_filter(mut filter: ResMut<HarmonicaPitchFilter>) {
    *filter = HarmonicaPitchFilter::default();
}

pub(crate) fn reset_score(
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut paused: ResMut<Paused>,
    mut gate: ResMut<PitchGate>,
    mut wait_freeze: ResMut<WaitFreezeState>,
) {
    *score = Score::default();
    *stats = SongStats::default();
    *feedback = HitFeedback::default();
    paused.0 = false;
    *gate = PitchGate::default();
    // `tick_clock` would recompute this as `None` on the new song's first
    // frame anyway; resetting here makes "no song starts frozen" true by
    // construction rather than by system ordering.
    *wait_freeze = WaitFreezeState::default();
}

/// Extra seconds after the last note before the results screen, so the final
/// notes ring out.
const SONG_END_TAIL: f64 = 2.5;

/// Resolves the song's own description strings once per song, for every
/// screen that shows them (the in-play header, the countdown, the pause
/// menu). Its own system rather than a corner of `setup_scoring_config`
/// because the consumers have to be ordered *after* it — an `add_systems`
/// tuple is unordered, so without that the pause menu spawns its details
/// from a default-empty `SongInfo` about half the time.
pub(crate) fn setup_song_info(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    effective: Res<EffectiveHarmonica>,
    loc: Res<Localization>,
    mut song_info: ResMut<SongInfo>,
) {
    if let Some(manifest) = manifests.get(&selected.0) {
        let harp = effective.harp_for(&manifest.chart);
        let key = effective.song_key_for(&manifest.chart);
        *song_info = SongInfo::from_chart(&manifest.chart, harp, &key, &loc);
    }
}

pub(crate) fn setup_scoring_config(
    selected: Res<SelectedSong>,
    effective: Res<EffectiveHarmonica>,
    manifests: Res<Assets<SongManifest>>,
    mut config: ResMut<ScoringConfig>,
    mut loop_cfg: ResMut<LoopConfig>,
    mut song_end: ResMut<SongEnd>,
    mut pitch_range: ResMut<PitchRange>,
    practice: Res<PracticeRequest>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let s = &chart.scoring;

    // Size the detector to this harmonica instead of a fixed constant, so a
    // Low-F/Low-D harp's low notes aren't cut off by a floor tuned for
    // standard keys (see TODO.md).
    *pitch_range = effective
        .harp_for(chart)
        .frequency_range()
        .map(|(lo, hi)| PitchRange::from_freqs([lo, hi], PITCH_RANGE_MARGIN_SEMITONES))
        .unwrap_or_default();

    config.perfect_window = s.perfect_window_ms as f64 / 1000.0;
    config.good_window = s.good_window_ms as f64 / 1000.0;
    config.miss_window = s.miss_window_ms as f64 / 1000.0;

    config.meter = chart_meter(chart);

    if let Some(combo) = &s.combo {
        config.combo_enabled = combo.enabled;
        config.base_multiplier = combo.base_multiplier;
        config.step_multiplier = combo.step_multiplier;
        config.max_multiplier = combo.max_multiplier;
        config.decay_secs = combo.decay_ms.map(|ms| ms as f64 / 1000.0);
    }

    // Per-technique style points awarded when a technique note is hit.
    config.style_bonus = s.style_bonus.clone().unwrap_or_default();

    // Set up loop section if the chart requests repeat playback.
    *loop_cfg = LoopConfig::default();
    if let Some(ls) = &chart.loop_section
        && ls.repeat == Some(true)
    {
        let track = &chart.track;
        let si = ls.start_index;
        let ei = ls.end_index;
        if si < track.len() && ei < track.len() && si <= ei {
            loop_cfg.active = true;
            loop_cfg.start_time = resolve_item_time(&track[si], &chart.timing);
            loop_cfg.end_time = resolve_item_time(&track[ei], &chart.timing) + track[ei].duration;
            info!(
                "Loop section ({:?}): {:.2}s – {:.2}s",
                ls.section_type, loop_cfg.start_time, loop_cfg.end_time,
            );
        }
    }

    // "Practice missed section" from the results screen: the player asked
    // for this range, so it wins over the chart's own default.
    if let Some(range) = practice.0
        && loop_range_valid(range.start_time, range.end_time)
    {
        loop_cfg.active = true;
        loop_cfg.start_time = range.start_time;
        loop_cfg.end_time = range.end_time;
    }

    // Song end = last note's end + a tail, so the results screen appears once
    // the content finishes. A loop doesn't change where the song ends — it
    // just keeps the clock from getting there (`detect_song_end` waits on
    // `LoopConfig` itself), so clearing the loop mid-song still finishes,
    // and the progress bar keeps its playhead and loop marker meanwhile.
    song_end.0 = last_note_end(&chart.track, &chart.timing) + SONG_END_TAIL;

    info!(
        "Scoring config: perfect={:.0}ms good={:.0}ms miss={:.0}ms combo={} beats/bar={}",
        config.perfect_window * 1000.0,
        config.good_window * 1000.0,
        config.miss_window * 1000.0,
        config.combo_enabled,
        config.meter.beats_per_bar(),
    );
}

/// Once the song's content has finished (and we're not looping or jamming),
/// transition to the results screen. Gated on `music_started` so it never fires
/// during the countdown, and on `LoopConfig` because a loop holds the clock
/// short of the end — `handle_loop_boundary` runs earlier in the same chain,
/// so an active loop has always already rewound by the time this reads it.
pub(crate) fn detect_song_end(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    music_started: Res<MusicStarted>,
    loop_cfg: Res<LoopConfig>,
    mode: Res<GameplayMode>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if *mode == GameplayMode::JamSession || !music_started.0 || loop_cfg.active {
        return;
    }
    if clock.get() >= song_end.0 {
        next_state.set(AppState::Results);
    }
}

/// Jumps a "Practice missed section" run to its range the moment the music
/// sink exists, then forgets the request so the jump happens once. Runs
/// right after `tick_clock` in the `GameplayLogic` chain: the sink is
/// spawned by `update_countdown` through `Commands`, so it's a frame late,
/// and `rewind_to` has to seek it in the same step that moves the clock or
/// the next anchoring pass would drag the clock straight back to 0.
///
/// Everything before the range is resolved silently first
/// (`skip_notes_before`) — the player asked to practise *this* stretch,
/// and a wall of `MISS` for notes the song never reached would say the
/// opposite. A song with no backing audio has no sink to wait for and
/// jumps immediately; one with several MIDI-stem sinks gets the same
/// clock-only jump every other loop rewind already gives it (`rewind_to`
/// only ever seeks a single sink).
pub(crate) fn start_at_practice_range(
    mut practice: ResMut<PracticeRequest>,
    music_started: Res<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<GameplayClock>,
    mut song_notes: ResMut<SongNotes>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    let Some(range) = practice.0 else {
        return;
    };
    if !music_started.0 {
        return;
    }
    let has_music =
        manifests.get(&selected.0).is_some_and(|m| m.music.is_some() || m.backing_stems.is_some());
    if has_music && sinks.is_empty() {
        return;
    }
    practice.0 = None;
    skip_notes_before(&mut song_notes.notes, range.start_time);
    clock.rewind_to(range.start_time, sinks.single().ok());
    info!("Practice range: {:.2}s – {:.2}s", range.start_time, range.end_time);
}

/// Marks every still-pending note that starts before `t` as resolved, so
/// the judge neither scores nor tallies it. `notes` is sorted by `time`,
/// so this is one prefix.
pub(crate) fn skip_notes_before(notes: &mut [ScheduledNote], t: f64) {
    let end = notes.partition_point(|n| n.time < t);
    for note in &mut notes[..end] {
        if !note.hit {
            note.missed = true;
        }
    }
}

/// Clears a practice request the run never consumed — the player quit
/// during the countdown — so it can't apply to whatever song comes next.
pub(crate) fn clear_practice_request(mut practice: ResMut<PracticeRequest>) {
    practice.0 = None;
}

/// Push the current music level onto the playing song's sink whenever the
/// `AudioSettings` resource changes, so dragging the Options slider is heard
/// immediately. (Metronome clicks pick up their level when each click spawns.)
pub(crate) fn apply_music_volume(
    audio: Res<harmonicon_audio::AudioSettings>,
    mut sinks: Query<&mut AudioSink, With<MusicPlayer>>,
) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(audio.music_volume));
    }
}

pub(crate) fn cleanup_gameplay(
    mut commands: Commands,
    roots: Query<Entity, With<GameplayRoot>>,
    mut pitch_range: ResMut<PitchRange>,
    mut effective: ResMut<EffectiveHarmonica>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    // A harmonica chosen for *this* song must not silently apply to the
    // next one — the pre-play screen asks per song, so the answer expires
    // with it. Same end-of-life point the pitch range uses below.
    effective.clear();
    // Leaving Playing/BendingTrainer drops the chart- or key-derived range;
    // menus and the spectrogram fall back to the default until another chart
    // (or the trainer) sets it again.
    *pitch_range = PitchRange::default();
}
