// SPDX-License-Identifier: MIT

mod bending_trainer;
mod countdown_overlay;
mod gameplay_2d;
mod gameplay_3d;
mod harmonica_overlay;
mod jam_session;
mod metronome_overlay;
mod modifier_legend;
pub mod note_tail_2d;
mod note_tail_3d;
pub mod note_visual_2d;
mod pause_menu;
mod phrase_overlay;
mod results;
mod scoring;
mod song_progress_overlay;
pub mod twelve_bar_blues_overlay;

use bevy::prelude::*;
pub use scoring::{NoteOutcome, classify_note, compute_points, sustain_points};
use scoring::{
    VIBRATO_MIN_SWING_CENTS, WAH_MIN_SWING_FRAC, combo_label, compute_multiplier,
    measured_oscillation_hz, measured_relative_oscillation_hz, oscillation_matches_rate,
    should_decay_combo,
};
use std::collections::HashMap;
use std::collections::HashSet;

use bevy::audio::Volume;

use crate::{
    audio_system::midi::{midi_to_note, note_to_freq_hz, note_to_midi},
    audio_system::pitch_detect::{AudioFrame, PitchEvent, PitchInfo, PitchRange},
    menu::{AppState, GameplayMode, SelectedSong},
    settings::AudioSettings,
    song::{SongManifest, chart::Modifier},
};

pub struct GameplayPlugin;

/// The shared per-frame gameplay logic (clock tick, scoring, loop handling).
/// Clock readers — note movement, hole/bar/metronome displays — must be ordered
/// after this set so they never sample a stale clock and stutter.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameplayLogic;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            countdown_overlay::CountdownPlugin,
            twelve_bar_blues_overlay::TwelveBarBluesPlugin,
            metronome_overlay::MetronomePlugin,
            phrase_overlay::PhrasePlugin,
            note_tail_2d::NoteTail2dPlugin,
            note_tail_3d::NoteTail3dPlugin,
            song_progress_overlay::SongProgressPlugin,
        ))
        .init_resource::<GameplayClock>()
        .init_resource::<PitchRange>()
        .init_resource::<ActivePitches>()
        .init_resource::<PitchGate>()
        .init_resource::<MusicStarted>()
        .init_resource::<ValidHarpNotes>()
        .init_resource::<Score>()
        .init_resource::<SongStats>()
        .init_resource::<SongEnd>()
        .init_resource::<HitFeedback>()
        .init_resource::<ScoringConfig>()
        .init_resource::<ActiveTargets>()
        .init_resource::<Paused>()
        .init_resource::<LoopConfig>()
        .init_resource::<bending_trainer::TrainerKey>()
        .init_resource::<bending_trainer::TrainerTarget>()
        .init_resource::<bending_trainer::DrillState>()
        .init_resource::<jam_session::JamLoop>()
        // Setup: shared pause menu + mode-specific scenes
        .add_systems(
            OnEnter(AppState::Playing),
            (
                reset_score,
                setup_scoring_config,
                pause_menu::setup_pause_menu,
                gameplay_2d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                gameplay_3d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                jam_session::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
            ),
        )
        // Standalone Bending Trainer (its own AppState, no song).
        .add_systems(OnEnter(AppState::BendingTrainer), bending_trainer::setup)
        .add_systems(OnExit(AppState::BendingTrainer), cleanup_gameplay)
        .add_systems(
            Update,
            (
                bending_trainer::tick_clock,
                collect_pitches,
                harmonica_overlay::update_harmonica_overlay,
                bending_trainer::rebuild_overlay,
                bending_trainer::update_pitch_range,
                bending_trainer::update_key_label,
                bending_trainer::update_target_label,
                bending_trainer::update_hint_label,
                bending_trainer::update_tuner_readout,
                bending_trainer::drill_update,
                bending_trainer::update_drill_label,
                bending_trainer::handle_escape,
            )
                .run_if(in_state(AppState::BendingTrainer)),
        )
        // Cleanup: shared entity despawn + restore camera on 3D exit
        .add_systems(OnExit(AppState::Playing), cleanup_gameplay)
        .add_systems(
            OnExit(AppState::Playing),
            gameplay_3d::restore_camera.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
        )
        // Pause input always runs during Playing (even when paused). The pause
        // buttons carry their own click/hover behaviour as inline `on(...)`
        // observers (see `setup_pause_menu`), so no button systems here.
        .add_systems(
            Update,
            pause_menu::handle_pause_input.run_if(in_state(AppState::Playing)),
        )
        // Apply live volume changes to the playing song (even while paused).
        .add_systems(
            Update,
            apply_music_volume
                .run_if(in_state(AppState::Playing).and_then(resource_changed::<AudioSettings>)),
        )
        // Gameplay-logic chains only run when not paused. This set ticks the
        // clock, so every clock reader below must run after it — otherwise the
        // executor may read a stale clock on some frames, making notes stutter.
        .add_systems(
            Update,
            (
                tick_clock,
                handle_loop_boundary,
                collect_pitches,
                update_active_targets,
                score_notes,
                update_score_display,
                detect_song_end,
                note_tail_2d::animate_note_tails,
            )
                .chain()
                .in_set(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        )
        // Jam Session: live harmonica hole-map feedback from the mic.
        .add_systems(
            Update,
            jam_session::update_hole_map.run_if(
                in_state(AppState::Playing)
                    .and_then(|p: Res<Paused>| !p.0)
                    .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
            ),
        )
        // Live bend-diagram feedback during Jam Session (the Bending Trainer
        // runs its own copy in its own AppState).
        .add_systems(
            Update,
            harmonica_overlay::update_harmonica_overlay.run_if(
                in_state(AppState::Playing)
                    .and_then(|p: Res<Paused>| !p.0)
                    .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
            ),
        )
        // Jam Session: music loop toggle + its readout.
        .add_systems(
            Update,
            (
                jam_session::apply_jam_loop_toggle,
                jam_session::update_jam_loop_label,
            )
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                ),
        )
        // Results screen lifecycle. The Retry/Continue buttons carry their own
        // click/hover behaviour as inline on(...) observers (see results::setup).
        .add_systems(OnEnter(AppState::Results), results::setup)
        .add_systems(OnExit(AppState::Results), results::cleanup)
        .add_systems(
            Update,
            results::handle_escape.run_if(in_state(AppState::Results)),
        )
        // 2D update chain
        .add_systems(
            Update,
            (
                gameplay_2d::update_notes,
                gameplay_2d::size_note_tails,
                gameplay_2d::update_note_visuals,
                gameplay_2d::update_holes,
            )
                .chain()
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                ),
        )
        // 3D update chain
        .add_systems(
            Update,
            (
                gameplay_3d::update_notes_3d,
                gameplay_3d::update_note_visuals_3d,
                gameplay_3d::animate_note_tails_3d,
                gameplay_3d::update_holes_3d,
                gameplay_3d::groove_harmonica,
            )
                .chain()
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                ),
        );
    }
}

// ── Shared resources ──────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct GameplayClock(pub f64);

#[derive(Resource, Default)]
pub struct ActivePitches(pub Vec<PitchInfo>);

/// Enforces a fresh attack per note. A sustained pitch may satisfy only **one**
/// note: once it scores, the pitch is "consumed" and cannot score again until it
/// stops sounding and is articulated anew. Without this, a single held breath on
/// (say) G4 would clear every G4 note that later scrolls into its hit window.
#[derive(Resource, Default)]
pub struct PitchGate {
    /// Valid harp pitches consumed by a hit since their last onset. Each is
    /// dropped once the pitch is no longer detected, re-arming it for the next
    /// articulation.
    consumed: HashSet<String>,
}

impl PitchGate {
    /// Drop consumption for any pitch that is no longer sounding, so its next
    /// articulation counts as a fresh attack.
    fn release_absent(&mut self, playing: &HashSet<String>) {
        self.consumed.retain(|p| playing.contains(p));
    }

    /// True if `pitch` is sounding and has not already scored a note during its
    /// current, continuous sustain.
    fn is_fresh(&self, pitch: &str, playing: &HashSet<String>) -> bool {
        playing.contains(pitch) && !self.consumed.contains(pitch)
    }

    /// Mark `pitch` as having scored; it won't score another note until it is
    /// released (see [`release_absent`](Self::release_absent)) and replayed.
    fn consume(&mut self, pitch: &str) {
        self.consumed.insert(pitch.to_string());
    }
}

#[derive(Resource, Default)]
pub struct MusicStarted(pub bool);

#[derive(Resource, Default)]
pub struct ValidHarpNotes(pub HashSet<String>);

#[derive(Resource, Default)]
pub struct Score {
    pub points: u32,
    pub combo: u32,
    pub max_combo: u32,
    pub last_hit_time: f64, // clock time of the last successful hit, for decay
}

/// Hit/miss tally for one technique category, so the results screen can show
/// "your bends land, your overblows don't" instead of one blended accuracy
/// number — the diagnostic a self-taught player actually needs.
#[derive(Default, Clone, Copy)]
pub struct TechniqueStats {
    pub hits: u32,
    pub misses: u32,
}

impl TechniqueStats {
    pub fn total(&self) -> u32 {
        self.hits + self.misses
    }

    /// Hit rate in `[0.0, 1.0]`, or `None` if the technique never came up in
    /// this song (nothing to report, not "0% accurate").
    pub fn accuracy(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some(self.hits as f32 / total as f32)
        }
    }
}

/// Per-song hit tally shown on the results screen. Reset at the start of each
/// song. `good` are on-time/early Good hits; `delayed` are late Good hits.
#[derive(Resource, Default)]
pub struct SongStats {
    pub perfect: u32,
    pub good: u32,
    pub delayed: u32,
    pub miss: u32,
    /// Sum of compensated timing offsets (seconds) for every hit. Divide by hit
    /// count (`perfect + good + delayed`) to get the mean offset. Positive means
    /// the player is still sounding notes after the target time even with the
    /// current `input_latency_ms` applied; increasing that setting by the mean
    /// (in ms) should centre the distribution.
    pub offset_sum: f64,
    /// Notes with no technique modifier at all — the baseline every other
    /// category is implicitly compared against.
    pub normal: TechniqueStats,
    pub bend: TechniqueStats,
    pub overblow: TechniqueStats,
    pub overdraw: TechniqueStats,
    pub vibrato: TechniqueStats,
    pub wah: TechniqueStats,
}

impl SongStats {
    /// Tallies a note's hit/miss outcome against every technique modifier it
    /// carries (a note can have up to two, e.g. Bend + Vibrato — it counts
    /// toward both), or `normal` if it has none.
    fn record_technique(&mut self, modifiers: &[Modifier], hit: bool) {
        if modifiers.is_empty() {
            bump(&mut self.normal, hit);
            return;
        }
        for m in modifiers {
            let bucket = match m {
                Modifier::Bend { .. } => &mut self.bend,
                Modifier::Overblow => &mut self.overblow,
                Modifier::Overdraw => &mut self.overdraw,
                Modifier::Vibrato { .. } => &mut self.vibrato,
                Modifier::WahWah { .. } => &mut self.wah,
            };
            bump(bucket, hit);
        }
    }
}

fn bump(stats: &mut TechniqueStats, hit: bool) {
    if hit {
        stats.hits += 1;
    } else {
        stats.misses += 1;
    }
}

/// Gameplay-clock time at which the song's content ends (so the results screen
/// can appear). `INFINITY` for looping songs, which never finish.
#[derive(Resource)]
pub struct SongEnd(pub f64);

impl Default for SongEnd {
    fn default() -> Self {
        Self(f64::INFINITY)
    }
}

/// Extra seconds after the last note before the results screen, so the final
/// notes ring out.
const SONG_END_TAIL: f64 = 2.5;

#[derive(Resource, Default)]
pub struct HitFeedback {
    pub quality: Option<HitQuality>,
    pub timer: f32,
}

/// Notes currently inside the good-hit window: (hole, is_blow).
/// Updated every frame so hole-display systems can show a target hint.
#[derive(Resource, Default)]
pub struct ActiveTargets(pub Vec<(u8, bool)>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HitQuality {
    Perfect,
    Good,
}

// ── Shared components ─────────────────────────────────────────────────────────

#[derive(Component, Default, Clone)]
pub struct GameplayRoot;

#[derive(Component)]
pub struct NoteVisual {
    pub time: f64,
    /// The note's duration as a fraction of `LOOKAHEAD`. The tail spans the
    /// highway distance scrolled over that duration, so its tip meets the hit
    /// line at the note's end — and the note is recycled only once that whole
    /// tail has scrolled off the bottom.
    pub duration_frac: f32,
}

/// Attached to every note entity (both modes). Drives scoring logic.
#[derive(Component)]
pub struct ScheduledNote {
    pub time: f64,
    /// Note length in seconds; long notes reward sustaining the pitch.
    pub duration: f64,
    pub hole: u8,
    pub is_blow: bool,
    /// The pitch string (e.g. "C4") this note expects, pre-computed at spawn.
    pub expected_pitch: String,
    pub hit: bool,
    pub missed: bool,
    /// Seconds the expected pitch has been held since the onset was hit.
    pub held: f64,
    /// Set once the sustain window has closed and its bonus was awarded.
    pub sustain_scored: bool,
    /// Technique modifiers from the chart (bend, vibrato, etc.).
    /// Used to trigger fx sounds when the note is hit.
    pub modifiers: Vec<Modifier>,
    /// `(clock time, cents-from-expected-pitch)`, sampled once per frame
    /// while held — used to verify a declared `vibrato` was actually played
    /// at roughly its declared `oscillation_hz`, not just declared. Storing
    /// the timestamp (rather than trusting sample order) keeps the measured
    /// rate frame-rate independent.
    pub pitch_samples: Vec<(f64, f32)>,
    /// `(clock time, input loudness RMS)`, sampled once per frame while
    /// held — used to verify a declared `wah-wah` was actually played at
    /// roughly its declared `oscillation_hz`, not just declared.
    pub amp_samples: Vec<(f64, f32)>,
}

#[derive(Component)]
#[require(HoleState)]
pub struct HoleCell(pub u8);

#[derive(Component, Default)]
pub struct HoleState {
    pub brightness: f32,
    pub is_blow: bool,
}

// Score HUD marker components
#[derive(Component)]
pub struct ScoreText;
#[derive(Component)]
pub struct ComboText;
#[derive(Component)]
pub struct FeedbackText;

/// Set to true while gameplay is paused; all update chains gate on `!paused`.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// Marks the music audio entity so it can be found for pause/resume.
#[derive(Component)]
pub struct MusicPlayer;

/// Scoring parameters resolved from the song's chart at game start.
/// Falls back to sensible defaults if the chart doesn't specify them.
#[derive(Resource)]
pub struct ScoringConfig {
    pub perfect_window: f64,
    pub good_window: f64,
    pub miss_window: f64,
    pub combo_enabled: bool,
    pub base_multiplier: f32,
    pub step_multiplier: f32,
    pub max_multiplier: f32,
    /// Seconds without a hit before the combo resets. `None` = never decays.
    pub decay_secs: Option<f64>,
    /// Beats per bar resolved from `timing.time_signature_map` (or `song.time_signature`).
    pub beats_per_bar: f64,
    /// Bonus points per technique (keyed by technique name) awarded on a hit,
    /// from the chart's `scoring.style_bonus`. Empty = no style points.
    pub style_bonus: HashMap<String, f32>,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            perfect_window: 0.060,
            good_window: 0.130,
            miss_window: 0.130,
            combo_enabled: true,
            base_multiplier: 1.0,
            step_multiplier: 0.1,
            max_multiplier: 4.0,
            decay_secs: None,
            beats_per_bar: 4.0,
            style_bonus: HashMap::new(),
        }
    }
}

/// Active loop region. When `active`, the gameplay clock resets to `start_time`
/// each time it passes `end_time`, repeating that section indefinitely.
#[derive(Resource, Default)]
pub struct LoopConfig {
    pub active: bool,
    pub start_time: f64,
    pub end_time: f64,
}

// ── Shared constants ──────────────────────────────────────────────────────────

pub const HOLE_COUNT: usize = 10;
pub const COUNTDOWN: f64 = 3.0;
pub const LANE_PCT: f32 = 100.0 / HOLE_COUNT as f32;
pub const HIT_H_PCT: f32 = 7.0;
pub const LOOKAHEAD: f64 = 3.0;

// ── Shared pure helpers ───────────────────────────────────────────────────────

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

/// Which of the 12 bars in a twelve-bar cycle the clock is currently on.
pub fn current_bar_index(clock: f64, secs_per_bar: f64) -> usize {
    (clock.max(0.0) / secs_per_bar) as usize % 12
}

/// Resolve a track item's start time in seconds, preferring an explicit `time`
/// and falling back to converting its `tick` through the tempo map.
pub fn resolve_item_time(
    item: &crate::song::chart::TrackItem,
    timing: &crate::song::chart::Timing,
) -> f64 {
    item.time.unwrap_or_else(|| {
        let tick = item.tick.unwrap_or(0);
        crate::song::chart::tick_to_seconds(tick, timing.resolution, &timing.tempo_map)
    })
}

/// The latest moment any note finishes (start + duration) across the track, in
/// seconds. Drives when the song's content ends. Zero for an empty track.
pub fn last_note_end(
    track: &[crate::song::chart::TrackItem],
    timing: &crate::song::chart::Timing,
) -> f64 {
    track
        .iter()
        .map(|item| resolve_item_time(item, timing) + item.duration)
        .fold(0.0_f64, f64::max)
}

/// The pitch the player must actually produce for a note. A `bend` shifts the
/// note's natural pitch by its semitones (negative = down), so the bend is
/// *validated* by scoring — playing the unbent note no longer counts. Notes
/// without a bend (or an unknown pitch name) keep their natural pitch.
pub fn target_pitch(natural: &str, modifiers: &[Modifier]) -> String {
    let bend = modifiers.iter().find_map(|m| match m {
        Modifier::Bend { semitones, .. } => Some(semitones.round() as i32),
        _ => None,
    });
    match (bend, note_to_midi(natural)) {
        (Some(s), Some(midi)) if s != 0 => midi_to_note(midi + s),
        _ => natural.to_string(),
    }
}

/// Style-bonus points awarded for a hit note's techniques, summed over its
/// modifiers using the chart's `style_bonus` table (keyed by technique name).
pub fn style_bonus_points(modifiers: &[Modifier], table: &HashMap<String, f32>) -> f32 {
    modifiers
        .iter()
        .map(|m| table.get(modifier_fx_key(m)).copied().unwrap_or(0.0))
        .sum()
}

/// Vibrato and wah are hand/throat articulations sustained *through* the
/// note, not a pitch shift validated by the onset alone (unlike a bend, whose
/// `expected_pitch` already encodes the bent target). Their style bonus is
/// deferred to the end of the sustain window and only paid out if
/// [`technique_confirmed`] finds the player actually wobbled the pitch/level.
fn is_sustained_technique(modifier: &Modifier) -> bool {
    matches!(modifier, Modifier::Vibrato { .. } | Modifier::WahWah { .. })
}

/// How far a measured vibrato/wah rate may drift from the chart's declared
/// `oscillation_hz` and still count — generous, since hand technique speed
/// varies naturally between players and even between notes.
const OSCILLATION_RATE_TOLERANCE_FRAC: f32 = 0.4;

/// Did the player actually perform this sustained technique, judged from the
/// pitch/loudness samples collected while the note was held — both that it
/// swung enough to be a real wobble, and that it swung at roughly the
/// chart's declared `oscillation_hz` rather than some unrelated rate.
/// Non-sustained modifiers (bend, overblow, overdraw) are validated at onset
/// instead — this always returns `true` for them since it shouldn't be asked.
fn technique_confirmed(
    modifier: &Modifier,
    pitch_samples: &[(f64, f32)],
    amp_samples: &[(f64, f32)],
) -> bool {
    match modifier {
        Modifier::Vibrato { oscillation_hz, .. } => {
            measured_oscillation_hz(pitch_samples, VIBRATO_MIN_SWING_CENTS).is_some_and(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        Modifier::WahWah { oscillation_hz, .. } => {
            measured_relative_oscillation_hz(amp_samples, WAH_MIN_SWING_FRAC).is_some_and(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        _ => true,
    }
}

/// The currently-detected frequency (Hz) matching `pitch_name` (e.g. `"D4"`),
/// or `None` if that exact note isn't among the detected pitches this frame.
fn active_frequency_for(active: &[PitchInfo], pitch_name: &str) -> Option<f32> {
    active
        .iter()
        .find(|p| format!("{}{}", p.note, p.octave) == pitch_name)
        .map(|p| p.frequency)
}

/// RMS loudness of a block of audio samples.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

// ── Shared systems ────────────────────────────────────────────────────────────

fn reset_score(
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut paused: ResMut<Paused>,
    mut gate: ResMut<PitchGate>,
) {
    *score = Score::default();
    *stats = SongStats::default();
    *feedback = HitFeedback::default();
    paused.0 = false;
    gate.consumed.clear();
}

/// Semitone margin added on each side of the harmonica's natural range when
/// sizing the pitch detector — covers bends/overblows landing just past a
/// charted note plus a little slop before a clean attack. Also used by the
/// bend trainer, which derives its own range from the current key.
pub(crate) const PITCH_RANGE_MARGIN_SEMITONES: f32 = 1.0;

fn setup_scoring_config(
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

/// Max per-frame pull toward the audio sink's playback position. Keeping this
/// small (rather than snapping straight to `audio_pos`) means a coarse or
/// jittery sink read never makes notes visibly jump; larger drift closes
/// gradually over a few frames instead.
const CLOCK_CORRECTION_STEP: f64 = 0.003;

/// Advance the clock by `dt`, re-anchoring toward `audio_pos` (the music
/// sink's playback position) when it's known. `audio_pos` is `None` during
/// the countdown, in Jam Session, and before the sink reports a position —
/// callers fall back to plain frame-delta accumulation in those cases.
fn advance_clock(current: f64, dt: f64, audio_pos: Option<f64>) -> f64 {
    let projected = current + dt;
    match audio_pos {
        Some(audio_pos) => {
            projected + (audio_pos - projected).clamp(-CLOCK_CORRECTION_STEP, CLOCK_CORRECTION_STEP)
        }
        None => projected,
    }
}

/// Ticks the single [`GameplayClock`] all gameplay systems read. Once the
/// countdown finishes and the song's music starts, the clock is kept
/// anchored to the `AudioSink` playback position instead of free-running on
/// `Time::delta` — otherwise decoder start-up delay and frame hitches drift
/// the notes out of sync with the audio over a long song. Jam Session has no
/// long track to drift against and stays frame-timer driven (metronome-led).
fn tick_clock(
    mut clock: ResMut<GameplayClock>,
    time: Res<Time>,
    mode: Res<GameplayMode>,
    music_started: Res<MusicStarted>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    let dt = time.delta_secs_f64();
    let audio_pos = (clock.0 >= 0.0 && music_started.0 && *mode != GameplayMode::JamSession)
        .then(|| sinks.single().ok())
        .flatten()
        .map(|sink| sink.position().as_secs_f64());
    clock.0 = advance_clock(clock.0, dt, audio_pos);
}

fn handle_loop_boundary(
    loop_cfg: Res<LoopConfig>,
    mut clock: ResMut<GameplayClock>,
    mut notes: Query<&mut ScheduledNote>,
) {
    if !loop_cfg.active || clock.0 < loop_cfg.end_time {
        return;
    }
    clock.0 = loop_cfg.start_time;
    for mut note in &mut notes {
        if note.time >= loop_cfg.start_time && note.time <= loop_cfg.end_time {
            note.hit = false;
            note.missed = false;
            note.held = 0.0;
            note.sustain_scored = false;
        }
    }
}

fn collect_pitches(mut reader: MessageReader<PitchEvent>, mut active: ResMut<ActivePitches>) {
    for ev in reader.read() {
        active.0 = ev.0.clone();
    }
}

fn update_active_targets(
    clock: Res<GameplayClock>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    notes: Query<&ScheduledNote>,
    mut targets: ResMut<ActiveTargets>,
) {
    targets.0.clear();
    if clock.0 < 0.0 {
        return;
    }
    // Shift the judgment point back by the microphone pipeline latency so the
    // highlighted hole tracks what the player is *actually* hearing, not what
    // the raw clock says.
    let judged = clock.0 - audio.input_latency_ms as f64 / 1000.0;
    for note in &notes {
        if note.hit || note.missed {
            continue;
        }
        if (judged - note.time).abs() <= config.good_window {
            targets.0.push((note.hole, note.is_blow));
        }
    }
}

fn score_notes(
    clock: Res<GameplayClock>,
    time: Res<Time>,
    active: Res<ActivePitches>,
    frame: Res<AudioFrame>,
    valid_notes: Res<ValidHarpNotes>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    mut notes: Query<(Entity, &mut ScheduledNote)>,
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut gate: ResMut<PitchGate>,
) {
    if clock.0 < 0.0 {
        return;
    }
    let dt = time.delta_secs_f64();
    // Compensate for microphone pipeline latency: a pitch detected at clock T
    // was actually played at T - latency. Shift the judgment window accordingly.
    let judged = clock.0 - audio.input_latency_ms as f64 / 1000.0;

    if config.combo_enabled
        && should_decay_combo(score.combo, clock.0, score.last_hit_time, config.decay_secs)
    {
        score.combo = 0;
    }

    let harp_pitches: HashSet<String> = active
        .0
        .iter()
        .filter(|p| valid_notes.0.contains(&format!("{}{}", p.note, p.octave)))
        .map(|p| format!("{}{}", p.note, p.octave))
        .collect();

    // Re-arm any pitch the player has stopped sounding, so its next attack is
    // fresh. Pitches still held remain consumed and can't score again.
    gate.release_absent(&harp_pitches);

    // Notes not yet hit or missed are classified in a second pass below,
    // ordered by |offset| (closest to the judged instant first). Query
    // iteration order is otherwise arbitrary, and when two same-pitch notes
    // overlap the hit window, whichever one happened to be classified first
    // would consume the attack — not necessarily the one actually due.
    let mut pending: Vec<(Entity, f64)> = Vec::new();

    for (entity, mut note) in &mut notes {
        if note.missed {
            continue;
        }

        // Already-hit notes are in their sustain phase: reward holding the pitch
        // through the note's length, then award the bonus once when it ends.
        if note.hit {
            if note.sustain_scored {
                continue;
            }
            if clock.0 < note.time + note.duration {
                // The held pitch stays "consumed" by the gate, so checking the
                // raw detected set keeps crediting this same note's sustain.
                if harp_pitches.contains(&note.expected_pitch) {
                    note.held += dt;
                }
                // Track pitch/loudness through the hold so a declared vibrato
                // or wah can be verified (rather than trusted) once it ends.
                if note.modifiers.iter().any(is_sustained_technique) {
                    if let Some(hz) = active_frequency_for(&active.0, &note.expected_pitch)
                        && let Some(expected_hz) = note_to_freq_hz(&note.expected_pitch)
                    {
                        note.pitch_samples
                            .push((clock.0, 1200.0 * (hz / expected_hz).log2()));
                    }
                    note.amp_samples.push((clock.0, rms(&frame.samples)));
                }
            } else {
                score.points += sustain_points(note.held, note.duration);

                let sustained: Vec<Modifier> = note
                    .modifiers
                    .iter()
                    .filter(|&x| is_sustained_technique(x))
                    .cloned()
                    .collect();
                if !sustained.is_empty() {
                    let (verified, unverified): (Vec<Modifier>, Vec<Modifier>) =
                        sustained.into_iter().partition(|m| {
                            technique_confirmed(m, &note.pitch_samples, &note.amp_samples)
                        });
                    if !verified.is_empty() {
                        score.points +=
                            style_bonus_points(&verified, &config.style_bonus).round() as u32;
                        stats.record_technique(&verified, true);
                    }
                    if !unverified.is_empty() {
                        stats.record_technique(&unverified, false);
                    }
                }
                note.sustain_scored = true;
            }
            continue;
        }

        pending.push((entity, judged - note.time));
    }

    pending.sort_by(|a, b| {
        a.1.abs()
            .partial_cmp(&b.1.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (entity, offset) in pending {
        let Ok((_, mut note)) = notes.get_mut(entity) else {
            continue;
        };
        // A note counts as "playing" only on a fresh attack: the pitch must be
        // sounding and not already consumed by an earlier note in this sustain.
        let playing = gate.is_fresh(&note.expected_pitch, &harp_pitches);

        match classify_note(
            offset,
            playing,
            config.perfect_window,
            config.good_window,
            config.miss_window,
        ) {
            NoteOutcome::Missed => {
                note.missed = true;
                stats.miss += 1;
                stats.record_technique(&note.modifiers, false);
                if config.combo_enabled {
                    score.combo = 0;
                }
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap | NoteOutcome::Waiting => {}
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                // Vibrato/wah are judged from the sustain, not the onset — see
                // the sustain branch above. A note with only those modifiers
                // has nothing to credit yet, so it's left out of `stats` here
                // rather than falling through to the "normal" bucket.
                let immediate: Vec<Modifier> = note
                    .modifiers
                    .iter()
                    .filter(|&m| !is_sustained_technique(m))
                    .cloned()
                    .collect();
                if note.modifiers.is_empty() || !immediate.is_empty() {
                    stats.record_technique(&immediate, true);
                }
                // Claim the attack so a held breath can't also clear the next
                // same-pitch note; the player must re-articulate for that one.
                gate.consume(&note.expected_pitch);
                match quality {
                    HitQuality::Perfect => stats.perfect += 1,
                    // A late Good hit counts as "delayed"; early/on-time as "good".
                    HitQuality::Good if offset > 0.0 => stats.delayed += 1,
                    HitQuality::Good => stats.good += 1,
                }
                stats.offset_sum += offset;
                score.last_hit_time = clock.0;
                score.combo += 1;
                score.max_combo = score.max_combo.max(score.combo);
                let multiplier = if config.combo_enabled {
                    compute_multiplier(
                        score.combo,
                        config.base_multiplier,
                        config.step_multiplier,
                        config.max_multiplier,
                    )
                } else {
                    1.0
                };
                score.points += compute_points(quality, multiplier);
                // Reward executing the note's onset techniques. Bends are
                // genuinely validated (the note's expected pitch is the bent
                // one); the bonus is the payoff for nailing them. Vibrato/wah
                // bonuses are awarded later, once the sustain confirms them.
                score.points += style_bonus_points(&immediate, &config.style_bonus).round() as u32;
                feedback.quality = Some(quality);
                feedback.timer = 0.75;
            }
        }
    }
}

fn modifier_fx_key(modifier: &Modifier) -> &'static str {
    match modifier {
        Modifier::Bend { .. } => "bend",
        Modifier::Vibrato { .. } => "vibrato",
        Modifier::WahWah { .. } => "wah-wah",
        Modifier::Overblow => "overblow",
        Modifier::Overdraw => "overdraw",
    }
}

fn update_score_display(
    score: Res<Score>,
    config: Res<ScoringConfig>,
    mut feedback: ResMut<HitFeedback>,
    time: Res<Time>,
    mut q_score: Query<&mut Text, (With<ScoreText>, Without<ComboText>, Without<FeedbackText>)>,
    mut q_combo: Query<&mut Text, (With<ComboText>, Without<ScoreText>, Without<FeedbackText>)>,
    mut q_feedback: Query<
        (&mut Text, &mut TextColor),
        (With<FeedbackText>, Without<ScoreText>, Without<ComboText>),
    >,
) {
    for mut t in &mut q_score {
        t.0 = format!("{}", score.points);
    }

    // Same multiplier `score_notes` actually applies to points, so the HUD
    // can never show a number the score disagrees with.
    let multiplier = if config.combo_enabled {
        compute_multiplier(
            score.combo,
            config.base_multiplier,
            config.step_multiplier,
            config.max_multiplier,
        )
    } else {
        1.0
    };
    for mut t in &mut q_combo {
        t.0 = combo_label(score.combo, multiplier);
    }

    feedback.timer = (feedback.timer - time.delta_secs()).max(0.0);

    for (mut t, mut color) in &mut q_feedback {
        match feedback.quality {
            None => {
                *color = TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
            Some(q) => {
                let alpha = (feedback.timer / 0.75).clamp(0.0, 1.0);
                // Scale up then fade: pulse from 1.4× down to 1× size isn't
                // easily done here, so we just fade alpha.
                let (label, r, g, b) = match q {
                    HitQuality::Perfect => ("PERFECT!", 1.00f32, 0.85, 0.10),
                    HitQuality::Good => ("GOOD", 0.40, 1.00, 0.35),
                };
                t.0 = label.to_string();
                *color = TextColor(Color::srgba(r, g, b, alpha));
                if feedback.timer == 0.0 {
                    feedback.quality = None;
                }
            }
        }
    }
}

/// Once the song's content has finished (and we're not looping or jamming),
/// transition to the results screen. Gated on `music_started` so it never fires
/// during the countdown.
fn detect_song_end(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    music_started: Res<MusicStarted>,
    mode: Res<GameplayMode>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if *mode == GameplayMode::JamSession || !music_started.0 {
        return;
    }
    if clock.0 >= song_end.0 {
        next_state.set(AppState::Results);
    }
}

/// Push the current music level onto the playing song's sink whenever the
/// `AudioSettings` resource changes, so dragging the Options slider is heard
/// immediately. (Metronome clicks pick up their level when each click spawns.)
fn apply_music_volume(
    audio: Res<AudioSettings>,
    mut sinks: Query<&mut AudioSink, With<MusicPlayer>>,
) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(audio.music_volume));
    }
}

fn cleanup_gameplay(
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── TechniqueStats / SongStats::record_technique ───────────────────────

    #[test]
    fn technique_stats_accuracy_is_none_when_never_exercised() {
        assert_eq!(TechniqueStats::default().accuracy(), None);
    }

    #[test]
    fn technique_stats_accuracy_divides_hits_by_total() {
        let s = TechniqueStats { hits: 3, misses: 1 };
        assert_eq!(s.total(), 4);
        assert!((s.accuracy().unwrap() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn record_technique_with_no_modifiers_goes_to_normal() {
        let mut stats = SongStats::default();
        stats.record_technique(&[], true);
        stats.record_technique(&[], false);
        assert_eq!(stats.normal.hits, 1);
        assert_eq!(stats.normal.misses, 1);
        assert_eq!(stats.bend.total(), 0);
    }

    #[test]
    fn record_technique_routes_each_modifier_to_its_own_bucket() {
        let mut stats = SongStats::default();
        stats.record_technique(
            &[Modifier::Bend {
                semitones: -1.0,
                intensity: None,
            }],
            true,
        );
        stats.record_technique(&[Modifier::Overblow], false);
        stats.record_technique(
            &[Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: None,
            }],
            true,
        );
        stats.record_technique(
            &[Modifier::WahWah {
                oscillation_hz: 3.0,
                intensity: None,
            }],
            true,
        );
        stats.record_technique(&[Modifier::Overdraw], true);

        assert_eq!(stats.bend.hits, 1);
        assert_eq!(stats.overblow.misses, 1);
        assert_eq!(stats.vibrato.hits, 1);
        assert_eq!(stats.wah.hits, 1);
        assert_eq!(stats.overdraw.hits, 1);
        assert_eq!(stats.normal.total(), 0, "no plain notes were recorded");
    }

    #[test]
    fn record_technique_with_two_modifiers_credits_both() {
        // A note that's both bent and vibrato'd counts as a data point for
        // both techniques' accuracy — hitting/missing it is informative for both.
        let mut stats = SongStats::default();
        stats.record_technique(
            &[
                Modifier::Bend {
                    semitones: -1.0,
                    intensity: None,
                },
                Modifier::Vibrato {
                    oscillation_hz: 5.0,
                    intensity: None,
                },
            ],
            true,
        );
        assert_eq!(stats.bend.hits, 1);
        assert_eq!(stats.vibrato.hits, 1);
    }

    #[test]
    fn parse_beats_4_4() {
        assert_eq!(parse_beats(Some("4/4")), 4.0);
    }

    #[test]
    fn parse_beats_3_4() {
        assert_eq!(parse_beats(Some("3/4")), 3.0);
    }

    #[test]
    fn parse_beats_none_defaults_to_4() {
        assert_eq!(parse_beats(None), 4.0);
    }

    #[test]
    fn parse_beats_malformed_defaults_to_4() {
        assert_eq!(parse_beats(Some("invalid")), 4.0);
    }

    #[test]
    fn secs_per_bar_120bpm_4beats() {
        assert!((secs_per_bar(120.0, 4.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn secs_per_bar_60bpm_4beats() {
        assert!((secs_per_bar(60.0, 4.0) - 4.0).abs() < 1e-9);
    }

    // ── advance_clock (audio-synced gameplay clock) ─────────────────────────

    #[test]
    fn advance_clock_with_no_audio_pos_is_plain_frame_delta() {
        assert!((advance_clock(1.0, 0.016, None) - 1.016).abs() < 1e-9);
    }

    #[test]
    fn advance_clock_snaps_fully_when_drift_is_within_the_correction_step() {
        // Projected clock is 1.016; audio says 1.017 — 1ms of drift, well
        // under the correction cap, so it's corrected in a single frame.
        let result = advance_clock(1.0, 0.016, Some(1.017));
        assert!((result - 1.017).abs() < 1e-9);
    }

    #[test]
    fn advance_clock_clamps_large_positive_drift() {
        // Audio is way ahead of the projected clock (e.g. after a stall) —
        // the correction must not jump straight there in one frame.
        let projected = 1.0 + 0.016;
        let result = advance_clock(1.0, 0.016, Some(projected + 0.5));
        assert!((result - (projected + CLOCK_CORRECTION_STEP)).abs() < 1e-9);
    }

    #[test]
    fn advance_clock_clamps_large_negative_drift() {
        let projected = 1.0 + 0.016;
        let result = advance_clock(1.0, 0.016, Some(projected - 0.5));
        assert!((result - (projected - CLOCK_CORRECTION_STEP)).abs() < 1e-9);
    }

    #[test]
    fn current_bar_index_at_zero() {
        assert_eq!(current_bar_index(0.0, 2.0), 0);
    }

    #[test]
    fn current_bar_index_advances() {
        assert_eq!(current_bar_index(2.0, 2.0), 1);
        assert_eq!(current_bar_index(4.0, 2.0), 2);
    }

    #[test]
    fn current_bar_index_wraps_at_12() {
        // 12 bars × 2 s/bar = 24 s → wraps back to bar 0
        assert_eq!(current_bar_index(24.0, 2.0), 0);
    }

    #[test]
    fn current_bar_index_clamps_negative_clock() {
        // During countdown the clock is negative — should give bar 0
        assert_eq!(current_bar_index(-1.5, 2.0), 0);
    }

    // ── resolve_item_time ───────────────────────────────────────────────────────

    use crate::song::chart::{TempoPoint, Timing, TrackItem};

    fn track_item(time: Option<f64>, tick: Option<u64>) -> TrackItem {
        TrackItem {
            id: None,
            time,
            tick,
            duration: 0.5,
            phrase: None,
            groove: None,
            play_mode: None,
            events: vec![],
        }
    }

    fn timing_120bpm() -> Timing {
        Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint {
                tick: 0,
                bpm: 120.0,
            }],
            time_signature_map: None,
        }
    }

    #[test]
    fn resolve_item_time_prefers_explicit_time() {
        let item = track_item(Some(2.5), Some(9999));
        assert!((resolve_item_time(&item, &timing_120bpm()) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn resolve_item_time_falls_back_to_tick() {
        // One quarter note (480 ticks) at 120 BPM = 0.5 s
        let item = track_item(None, Some(480));
        assert!((resolve_item_time(&item, &timing_120bpm()) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn resolve_item_time_defaults_missing_tick_to_zero() {
        let item = track_item(None, None);
        assert_eq!(resolve_item_time(&item, &timing_120bpm()), 0.0);
    }

    // ── last_note_end ─────────────────────────────────────────────────────────────

    #[test]
    fn last_note_end_is_latest_finish() {
        // Items at 0.0 and 2.0, each 0.5 s long → latest finish is 2.5 s.
        let track = vec![track_item(Some(0.0), None), track_item(Some(2.0), None)];
        assert!((last_note_end(&track, &timing_120bpm()) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn last_note_end_ignores_order() {
        // The latest end wins even when the longest note isn't last in the track.
        let track = vec![track_item(Some(5.0), None), track_item(Some(1.0), None)];
        assert!((last_note_end(&track, &timing_120bpm()) - 5.5).abs() < 1e-9);
    }

    #[test]
    fn last_note_end_empty_track_is_zero() {
        assert_eq!(last_note_end(&[], &timing_120bpm()), 0.0);
    }

    // ── modifier_fx_key ───────────────────────────────────────────────────────────

    #[test]
    fn modifier_fx_keys_match_technique_names() {
        use crate::song::chart::Modifier::*;
        assert_eq!(
            modifier_fx_key(&Bend {
                semitones: -1.0,
                intensity: None
            }),
            "bend"
        );
        assert_eq!(
            modifier_fx_key(&Vibrato {
                oscillation_hz: 5.0,
                intensity: None
            }),
            "vibrato"
        );
        assert_eq!(
            modifier_fx_key(&WahWah {
                oscillation_hz: 3.0,
                intensity: None
            }),
            "wah-wah"
        );
        assert_eq!(modifier_fx_key(&Overblow), "overblow");
        assert_eq!(modifier_fx_key(&Overdraw), "overdraw");
    }

    // ── PitchGate (re-attack detection) ──────────────────────────────────────────

    fn playing(pitches: &[&str]) -> HashSet<String> {
        pitches.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn a_sounding_unconsumed_pitch_is_fresh() {
        let gate = PitchGate::default();
        assert!(gate.is_fresh("G4", &playing(&["G4"])));
    }

    #[test]
    fn a_silent_pitch_is_never_fresh() {
        let gate = PitchGate::default();
        assert!(!gate.is_fresh("G4", &playing(&[])));
    }

    #[test]
    fn a_held_pitch_cannot_score_twice() {
        // The core fix: one sustained breath clears one note, not the next.
        let mut gate = PitchGate::default();
        let held = playing(&["G4"]);

        // First note: fresh attack scores, then we consume the pitch.
        assert!(gate.is_fresh("G4", &held));
        gate.consume("G4");

        // The breath is still held — a second G4 note must NOT count.
        gate.release_absent(&held);
        assert!(!gate.is_fresh("G4", &held));
    }

    #[test]
    fn re_articulating_a_pitch_re_arms_it() {
        let mut gate = PitchGate::default();
        gate.consume("G4");

        // Player stops playing: G4 drops out of the detected set and re-arms.
        gate.release_absent(&playing(&[]));

        // Next attack on G4 is fresh again.
        assert!(gate.is_fresh("G4", &playing(&["G4"])));
    }

    #[test]
    fn consuming_one_pitch_leaves_others_fresh() {
        let mut gate = PitchGate::default();
        let chord = playing(&["G4", "B4"]);
        gate.consume("G4");
        gate.release_absent(&chord);

        assert!(!gate.is_fresh("G4", &chord));
        assert!(gate.is_fresh("B4", &chord));
    }

    // ── target_pitch (bend validation) ───────────────────────────────────────────

    #[test]
    fn bend_targets_the_bent_pitch() {
        let bend = vec![Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        }];
        // A 1-semitone draw bend on B4 must be played as A#4, not the natural B4.
        assert_eq!(target_pitch("B4", &bend), "A#4");
    }

    #[test]
    fn deeper_bend_targets_lower_pitch() {
        let bend = vec![Modifier::Bend {
            semitones: -2.0,
            intensity: None,
        }];
        assert_eq!(target_pitch("B4", &bend), "A4");
    }

    #[test]
    fn non_bend_techniques_keep_the_natural_pitch() {
        let vib = vec![Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        }];
        assert_eq!(target_pitch("D5", &vib), "D5");
        assert_eq!(target_pitch("D5", &[]), "D5");
    }

    #[test]
    fn unknown_pitch_name_is_left_alone() {
        let bend = vec![Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        }];
        assert_eq!(target_pitch("\u{2014}", &bend), "\u{2014}");
    }

    // ── style_bonus_points ───────────────────────────────────────────────────────

    fn bonus_table() -> HashMap<String, f32> {
        [("bend".to_string(), 50.0), ("vibrato".to_string(), 25.0)]
            .into_iter()
            .collect()
    }

    #[test]
    fn style_bonus_sums_matched_techniques() {
        let mods = vec![
            Modifier::Bend {
                semitones: -1.0,
                intensity: None,
            },
            Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: None,
            },
        ];
        assert_eq!(style_bonus_points(&mods, &bonus_table()), 75.0);
    }

    #[test]
    fn style_bonus_ignores_techniques_absent_from_the_table() {
        let mods = vec![Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        }];
        assert_eq!(style_bonus_points(&mods, &bonus_table()), 0.0);
    }

    #[test]
    fn style_bonus_is_zero_without_modifiers() {
        assert_eq!(style_bonus_points(&[], &bonus_table()), 0.0);
    }

    // ── sustained-technique validation (vibrato / wah) ──────────────────────────

    #[test]
    fn vibrato_and_wah_are_sustained_bend_and_overblow_are_not() {
        let vibrato = Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        };
        let wah = Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        };
        let bend = Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        };
        assert!(is_sustained_technique(&vibrato));
        assert!(is_sustained_technique(&wah));
        assert!(!is_sustained_technique(&bend));
        assert!(!is_sustained_technique(&Modifier::Overblow));
        assert!(!is_sustained_technique(&Modifier::Overdraw));
    }

    // Timestamped sine samples around `offset`, `n` samples spaced `dt` seconds apart.
    fn timestamped_sine(
        freq_hz: f32,
        offset: f32,
        amplitude: f32,
        n: usize,
        dt: f64,
    ) -> Vec<(f64, f32)> {
        (0..n)
            .map(|i| {
                let t = i as f64 * dt;
                let v =
                    offset + amplitude * (2.0 * std::f32::consts::PI * freq_hz * t as f32).sin();
                (t, v)
            })
            .collect()
    }

    #[test]
    fn technique_confirmed_requires_real_wobble_for_vibrato() {
        let vibrato = Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        };
        let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.0)).collect();
        let wobbling = timestamped_sine(5.0, 0.0, 25.0, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&vibrato, &steady, &[]));
        assert!(technique_confirmed(&vibrato, &wobbling, &[]));
    }

    #[test]
    fn technique_confirmed_requires_real_wobble_for_wah() {
        let wah = Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        };
        let steady_volume: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.2)).collect();
        let pumping_volume = timestamped_sine(3.0, 0.2, 0.06, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&wah, &[], &steady_volume));
        assert!(technique_confirmed(&wah, &[], &pumping_volume));
    }

    #[test]
    fn technique_confirmed_rejects_vibrato_at_the_wrong_rate() {
        // The chart declares a 5 Hz vibrato, but the player wobbled at ~1.5 Hz
        // — real oscillation, just not the declared rate. A flip-count-only
        // check (the old behavior) couldn't tell these apart.
        let vibrato = Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        };
        let slow_wobble = timestamped_sine(1.5, 0.0, 25.0, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&vibrato, &slow_wobble, &[]));
    }

    #[test]
    fn technique_confirmed_rejects_wah_at_the_wrong_rate() {
        let wah = Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        };
        let fast_pumping = timestamped_sine(9.0, 0.2, 0.06, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&wah, &[], &fast_pumping));
    }

    #[test]
    fn technique_confirmed_is_always_true_for_onset_validated_modifiers() {
        // Bend/overblow/overdraw are judged at onset, not from the sustain
        // buffers — this should never gate them on empty/steady samples.
        assert!(technique_confirmed(
            &Modifier::Bend {
                semitones: -1.0,
                intensity: None
            },
            &[],
            &[]
        ));
        assert!(technique_confirmed(&Modifier::Overblow, &[], &[]));
    }

    #[test]
    fn active_frequency_for_matches_by_note_and_octave() {
        let active = vec![
            PitchInfo {
                note: "D".into(),
                octave: 4,
                frequency: 293.66,
            },
            PitchInfo {
                note: "G".into(),
                octave: 4,
                frequency: 392.00,
            },
        ];
        assert_eq!(active_frequency_for(&active, "D4"), Some(293.66));
        assert_eq!(active_frequency_for(&active, "A4"), None);
    }

    // ── cleanup_gameplay ──────────────────────────────────────────────────────────

    #[test]
    fn cleanup_despawns_only_gameplay_entities() {
        // Leaving Playing must tear down the scene (every `GameplayRoot`) while
        // leaving unrelated entities (e.g. the persistent camera) untouched.
        let mut world = World::new();
        world.init_resource::<PitchRange>();
        let scene_a = world.spawn(GameplayRoot).id();
        let scene_b = world.spawn((GameplayRoot, Transform::default())).id();
        let keep = world.spawn_empty().id();

        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_gameplay);
        schedule.run(&mut world);

        assert!(
            !world.entities().contains(scene_a),
            "GameplayRoot should be despawned"
        );
        assert!(
            !world.entities().contains(scene_b),
            "GameplayRoot should be despawned"
        );
        assert!(
            world.entities().contains(keep),
            "unrelated entities must survive"
        );
    }

    // ── score_notes (same-pitch overlap ordering) ───────────────────────────

    fn overlap_test_note(time: f64) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 1.0,
            hole: 1,
            is_blow: true,
            expected_pitch: "C4".to_string(),
            hit: false,
            missed: false,
            held: 0.0,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
        }
    }

    #[test]
    fn score_notes_credits_the_closest_offset_when_two_same_pitch_notes_overlap() {
        // Two C4 notes both sit inside the hit window at clock=0.5 while C4 is
        // sounding: one 0.01s away (should score), one 0.10s away (should
        // stay `Waiting` — the pitch is fresh only once). Before the |offset|
        // sort this depended on arbitrary Query iteration order; spawning the
        // farther note first reproduces the exact bug (whichever the query
        // visited first used to win, regardless of which was actually due).
        let mut world = World::new();
        world.insert_resource(GameplayClock(0.5));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ActivePitches(vec![PitchInfo {
            note: "C".to_string(),
            octave: 4,
            frequency: note_to_freq_hz("C4").unwrap(),
        }]));
        world.insert_resource(AudioFrame::default());
        world.insert_resource(ValidHarpNotes(HashSet::from(["C4".to_string()])));
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(AudioSettings::default());
        world.insert_resource(Score::default());
        world.insert_resource(SongStats::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(PitchGate::default());

        let far = world.spawn(overlap_test_note(0.40)).id(); // offset -0.10
        let close = world.spawn(overlap_test_note(0.49)).id(); // offset -0.01

        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        assert!(
            world.get::<ScheduledNote>(close).unwrap().hit,
            "the note actually due should be credited"
        );
        assert!(
            !world.get::<ScheduledNote>(far).unwrap().hit,
            "the farther note must not steal the attack meant for the closer one"
        );
    }
}
