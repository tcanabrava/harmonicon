// SPDX-License-Identifier: MIT

//! Song-lifetime plumbing: resetting score state at song start, resolving a
//! chart's scoring/loop configuration, detecting when the song's content has
//! finished, and tearing everything down on exit.

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::app::{AppState, GameplayMode, SelectedSong};
use crate::audio_system::pitch_detect::PitchRange;
use crate::song::SongManifest;

use super::bars::parse_beats;
use super::clock::GameplayClock;
use super::jam_session;
use super::notes::{last_note_end, resolve_item_time};
use super::state::{
    GameplayRoot, HitFeedback, LoopConfig, MusicPlayer, MusicStarted, Paused, PitchGate, Score,
    ScoringConfig, SongEnd, SongStats,
};

pub(crate) fn reset_score(
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut paused: ResMut<Paused>,
    mut gate: ResMut<PitchGate>,
    mut improv_gate: ResMut<jam_session::ImprovGate>,
    mut improv_stats: ResMut<jam_session::ImprovStats>,
) {
    *score = Score::default();
    *stats = SongStats::default();
    *feedback = HitFeedback::default();
    paused.0 = false;
    *gate = PitchGate::default();
    *improv_gate = jam_session::ImprovGate::default();
    *improv_stats = jam_session::ImprovStats::default();
}

/// Semitone margin added on each side of the harmonica's natural range when
/// sizing the pitch detector — covers bends/overblows landing just past a
/// charted note plus a little slop before a clean attack. Also used by the
/// bend trainer, which derives its own range from the current key.
pub(crate) const PITCH_RANGE_MARGIN_SEMITONES: f32 = 1.0;

/// Extra seconds after the last note before the results screen, so the final
/// notes ring out.
const SONG_END_TAIL: f64 = 2.5;

pub(crate) fn setup_scoring_config(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut config: ResMut<ScoringConfig>,
    mut loop_cfg: ResMut<LoopConfig>,
    mut song_end: ResMut<SongEnd>,
    mut pitch_range: ResMut<PitchRange>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let s = &chart.scoring;

    // Size the detector to this harmonica instead of a fixed constant, so a
    // Low-F/Low-D harp's low notes aren't cut off by a floor tuned for
    // standard keys (see TODO.md).
    *pitch_range = chart
        .harmonica
        .frequency_range()
        .map(|(lo, hi)| PitchRange::from_freqs([lo, hi], PITCH_RANGE_MARGIN_SEMITONES))
        .unwrap_or_default();

    config.perfect_window = s.perfect_window_ms as f64 / 1000.0;
    config.good_window = s.good_window_ms as f64 / 1000.0;
    config.miss_window = s.miss_window_ms as f64 / 1000.0;

    // Resolve beats per bar: time_signature_map at tick=0 takes precedence over song field.
    let beats_str = chart
        .timing
        .time_signature_map
        .as_deref()
        .and_then(|m| crate::song::chart::time_sig_at_tick(0, m))
        .or(chart.song.time_signature.as_deref());
    config.beats_per_bar = parse_beats(beats_str);

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

    // Song end = last note's end + a tail, so the results screen appears once the
    // content finishes. Looping songs never end.
    song_end.0 = if loop_cfg.active {
        f64::INFINITY
    } else {
        last_note_end(&chart.track, &chart.timing) + SONG_END_TAIL
    };

    info!(
        "Scoring config: perfect={:.0}ms good={:.0}ms miss={:.0}ms combo={} beats/bar={}",
        config.perfect_window * 1000.0,
        config.good_window * 1000.0,
        config.miss_window * 1000.0,
        config.combo_enabled,
        config.beats_per_bar,
    );
}

/// Once the song's content has finished (and we're not looping or jamming),
/// transition to the results screen. Gated on `music_started` so it never fires
/// during the countdown.
pub(crate) fn detect_song_end(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    music_started: Res<MusicStarted>,
    mode: Res<GameplayMode>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if *mode == GameplayMode::JamSession || !music_started.0 {
        return;
    }
    if clock.get() >= song_end.0 {
        next_state.set(AppState::Results);
    }
}

/// Push the current music level onto the playing song's sink whenever the
/// `AudioSettings` resource changes, so dragging the Options slider is heard
/// immediately. (Metronome clicks pick up their level when each click spawns.)
pub(crate) fn apply_music_volume(
    audio: Res<crate::settings::AudioSettings>,
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
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    // Leaving Playing/BendingTrainer drops the chart- or key-derived range;
    // menus and the spectrogram fall back to the default until another chart
    // (or the trainer) sets it again.
    *pitch_range = PitchRange::default();
}
