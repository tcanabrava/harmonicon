// SPDX-License-Identifier: MIT

mod countdown_overlay;
mod gameplay_2d;
mod gameplay_3d;
mod jam_session;
mod metronome_overlay;
mod modifier_legend;
mod note_tail_2d;
mod note_tail_3d;
mod pause_menu;
mod phrase_overlay;
mod results;
mod scoring;
mod song_progress_overlay;
mod twelve_bar_blues_overlay;

use bevy::prelude::*;
use scoring::{
    NoteOutcome, classify_note, combo_label, compute_multiplier, compute_points, should_decay_combo,
    sustain_points,
};
use std::collections::HashMap;
use std::collections::HashSet;

use bevy::audio::Volume;

use crate::{
    audio_system::midi::{midi_to_note, note_to_midi},
    audio_system::pitch_detect::{PitchEvent, PitchInfo},
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
            .init_resource::<FxMapping>()
            // Setup: shared pause menu + mode-specific scenes
            .add_systems(
                OnEnter(AppState::Playing),
                (
                    reset_score,
                    setup_scoring_config,
                    pause_menu::setup_pause_menu,
                    gameplay_2d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                    gameplay_3d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                    jam_session::setup
                        .run_if(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                ),
            )
            // Cleanup: shared entity despawn + restore camera on 3D exit
            .add_systems(OnExit(AppState::Playing), cleanup_gameplay)
            .add_systems(
                OnExit(AppState::Playing),
                gameplay_3d::restore_camera
                    .run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
            )
            // Pause input always runs during Playing (even when paused)
            .add_systems(
                Update,
                (
                    pause_menu::handle_pause_input,
                    pause_menu::handle_pause_buttons,
                    pause_menu::pause_button_hover,
                )
                    .run_if(in_state(AppState::Playing)),
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
            // Results screen lifecycle.
            .add_systems(OnEnter(AppState::Results), results::setup)
            .add_systems(OnExit(AppState::Results), results::cleanup)
            .add_systems(
                Update,
                (results::handle_buttons, results::button_hover)
                    .run_if(in_state(AppState::Results)),
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

/// Per-song hit tally shown on the results screen. Reset at the start of each
/// song. `good` are on-time/early Good hits; `delayed` are late Good hits.
#[derive(Resource, Default)]
pub struct SongStats {
    pub perfect: u32,
    pub good: u32,
    pub delayed: u32,
    pub miss: u32,
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

#[derive(Component)]
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
}

#[derive(Component)]
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

/// Maps modifier type names (e.g. `"bend"`, `"vibrato"`) to the DSP effect
/// processor name the chart author intends to activate (e.g. `"pitch_bend"`).
/// Populated from `chart.fx_mapping` at song start; consumed by the audio/DSP layer.
#[derive(Resource, Default)]
pub struct FxMapping(pub HashMap<String, String>);

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
pub fn resolve_item_time(item: &crate::song::chart::TrackItem, timing: &crate::song::chart::Timing) -> f64 {
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

fn setup_scoring_config(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut config: ResMut<ScoringConfig>,
    mut loop_cfg: ResMut<LoopConfig>,
    mut fx_mapping: ResMut<FxMapping>,
    mut song_end: ResMut<SongEnd>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let s = &chart.scoring;

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
    if let Some(ls) = &chart.loop_section {
        if ls.repeat == Some(true) {
            let track = &chart.track;
            let si = ls.start_index;
            let ei = ls.end_index;
            if si < track.len() && ei < track.len() && si <= ei {
                let resolve = |i: usize| -> f64 {
                    track[i].time.unwrap_or_else(|| {
                        let tick = track[i].tick.unwrap_or(0);
                        crate::song::chart::tick_to_seconds(
                            tick,
                            chart.timing.resolution,
                            &chart.timing.tempo_map,
                        )
                    })
                };
                loop_cfg.active = true;
                loop_cfg.start_time = resolve(si);
                loop_cfg.end_time = resolve(ei) + track[ei].duration;
                info!(
                    "Loop section ({:?}): {:.2}s – {:.2}s",
                    ls.section_type, loop_cfg.start_time, loop_cfg.end_time,
                );
            }
        }
    }

    // Song end = last note's end + a tail, so the results screen appears once the
    // content finishes. Looping songs never end.
    song_end.0 = if loop_cfg.active {
        f64::INFINITY
    } else {
        last_note_end(&chart.track, &chart.timing) + SONG_END_TAIL
    };

    // Resolve fx_mapping: modifier name → DSP effect processor name.
    fx_mapping.0 = chart
        .fx_mapping
        .as_ref()
        .map(|m| m.clone())
        .unwrap_or_default();

    info!(
        "Scoring config: perfect={:.0}ms good={:.0}ms miss={:.0}ms combo={} beats/bar={}",
        config.perfect_window * 1000.0,
        config.good_window * 1000.0,
        config.miss_window * 1000.0,
        config.combo_enabled,
        config.beats_per_bar,
    );
}

fn tick_clock(mut clock: ResMut<GameplayClock>, time: Res<Time>) {
    clock.0 += time.delta_secs_f64();
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
    notes: Query<&ScheduledNote>,
    mut targets: ResMut<ActiveTargets>,
) {
    targets.0.clear();
    if clock.0 < 0.0 {
        return;
    }
    for note in &notes {
        if note.hit || note.missed {
            continue;
        }
        if (clock.0 - note.time).abs() <= config.good_window {
            targets.0.push((note.hole, note.is_blow));
        }
    }
}

fn score_notes(
    clock: Res<GameplayClock>,
    time: Res<Time>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    config: Res<ScoringConfig>,
    fx_mapping: Res<FxMapping>,
    mut notes: Query<&mut ScheduledNote>,
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut gate: ResMut<PitchGate>,
) {
    if clock.0 < 0.0 {
        return;
    }
    let dt = time.delta_secs_f64();

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

    for mut note in &mut notes {
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
            } else {
                score.points += sustain_points(note.held, note.duration);
                note.sustain_scored = true;
            }
            continue;
        }

        let offset = clock.0 - note.time;
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
                if config.combo_enabled {
                    score.combo = 0;
                }
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap | NoteOutcome::Waiting => {}
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                // Claim the attack so a held breath can't also clear the next
                // same-pitch note; the player must re-articulate for that one.
                gate.consume(&note.expected_pitch);
                match quality {
                    HitQuality::Perfect => stats.perfect += 1,
                    // A late Good hit counts as "delayed"; early/on-time as "good".
                    HitQuality::Good if offset > 0.0 => stats.delayed += 1,
                    HitQuality::Good => stats.good += 1,
                }
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
                // Reward executing the note's techniques. Bends are genuinely
                // validated (the note's expected pitch is the bent one); the
                // bonus is the payoff for nailing them.
                score.points +=
                    style_bonus_points(&note.modifiers, &config.style_bonus).round() as u32;
                feedback.quality = Some(quality);
                feedback.timer = 0.75;

                // Resolve which DSP effects should activate for each modifier.
                for modifier in &note.modifiers {
                    let key = modifier_fx_key(modifier);
                    if let Some(effect) = fx_mapping.0.get(key) {
                        debug!("fx: modifier={key} → effect={effect}");
                    }
                }
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

    for mut t in &mut q_combo {
        t.0 = combo_label(score.combo);
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
fn apply_music_volume(audio: Res<AudioSettings>, mut sinks: Query<&mut AudioSink, With<MusicPlayer>>) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(audio.music_volume));
    }
}

fn cleanup_gameplay(mut commands: Commands, roots: Query<Entity, With<GameplayRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            tempo_map: vec![TempoPoint { tick: 0, bpm: 120.0 }],
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
        assert_eq!(modifier_fx_key(&Bend { semitones: -1.0, intensity: None }), "bend");
        assert_eq!(modifier_fx_key(&Vibrato { oscillation_hz: 5.0, intensity: None }), "vibrato");
        assert_eq!(modifier_fx_key(&WahWah { oscillation_hz: 3.0, intensity: None }), "wah-wah");
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
        let bend = vec![Modifier::Bend { semitones: -1.0, intensity: None }];
        // A 1-semitone draw bend on B4 must be played as A#4, not the natural B4.
        assert_eq!(target_pitch("B4", &bend), "A#4");
    }

    #[test]
    fn deeper_bend_targets_lower_pitch() {
        let bend = vec![Modifier::Bend { semitones: -2.0, intensity: None }];
        assert_eq!(target_pitch("B4", &bend), "A4");
    }

    #[test]
    fn non_bend_techniques_keep_the_natural_pitch() {
        let vib = vec![Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }];
        assert_eq!(target_pitch("D5", &vib), "D5");
        assert_eq!(target_pitch("D5", &[]), "D5");
    }

    #[test]
    fn unknown_pitch_name_is_left_alone() {
        let bend = vec![Modifier::Bend { semitones: -1.0, intensity: None }];
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
            Modifier::Bend { semitones: -1.0, intensity: None },
            Modifier::Vibrato { oscillation_hz: 5.0, intensity: None },
        ];
        assert_eq!(style_bonus_points(&mods, &bonus_table()), 75.0);
    }

    #[test]
    fn style_bonus_ignores_techniques_absent_from_the_table() {
        let mods = vec![Modifier::WahWah { oscillation_hz: 3.0, intensity: None }];
        assert_eq!(style_bonus_points(&mods, &bonus_table()), 0.0);
    }

    #[test]
    fn style_bonus_is_zero_without_modifiers() {
        assert_eq!(style_bonus_points(&[], &bonus_table()), 0.0);
    }

    // ── cleanup_gameplay ──────────────────────────────────────────────────────────

    #[test]
    fn cleanup_despawns_only_gameplay_entities() {
        // Leaving Playing must tear down the scene (every `GameplayRoot`) while
        // leaving unrelated entities (e.g. the persistent camera) untouched.
        let mut world = World::new();
        let scene_a = world.spawn(GameplayRoot).id();
        let scene_b = world.spawn((GameplayRoot, Transform::default())).id();
        let keep = world.spawn_empty().id();

        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_gameplay);
        schedule.run(&mut world);

        assert!(!world.entities().contains(scene_a), "GameplayRoot should be despawned");
        assert!(!world.entities().contains(scene_b), "GameplayRoot should be despawned");
        assert!(world.entities().contains(keep), "unrelated entities must survive");
    }
}
