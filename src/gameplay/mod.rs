mod gameplay_2d;
mod gameplay_3d;

use std::collections::HashSet;
use bevy::prelude::*;

use crate::{
    menu::{AppState, GameplayMode, SelectedSong},
    pitch_detect::{PitchEvent, PitchInfo},
    song::SongManifest,
};

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayClock>()
            .init_resource::<ActivePitches>()
            .init_resource::<MusicStarted>()
            .init_resource::<ValidHarpNotes>()
            .init_resource::<Score>()
            .init_resource::<HitFeedback>()
            .init_resource::<ScoringConfig>()
            // Mode-gated setup
            .add_systems(
                OnEnter(AppState::Playing),
                (
                    reset_score,
                    setup_scoring_config,
                    gameplay_2d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                    gameplay_3d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                ),
            )
            // Cleanup: shared entity despawn + restore camera on 3D exit
            .add_systems(OnExit(AppState::Playing), cleanup_gameplay)
            .add_systems(
                OnExit(AppState::Playing),
                gameplay_3d::restore_camera
                    .run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
            )
            // Shared systems: tick, pitch collection, scoring, display
            .add_systems(
                Update,
                (tick_clock, collect_pitches, score_notes, update_score_display)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            // 2D update chain
            .add_systems(
                Update,
                (
                    gameplay_2d::update_countdown,
                    gameplay_2d::update_notes,
                    gameplay_2d::update_bar,
                    gameplay_2d::update_holes,
                )
                    .chain()
                    .run_if(
                        in_state(AppState::Playing)
                            .and(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                    ),
            )
            // 3D update chain
            .add_systems(
                Update,
                (
                    gameplay_3d::update_countdown,
                    gameplay_3d::update_notes_3d,
                    gameplay_3d::update_bar_3d,
                    gameplay_3d::update_holes_3d,
                )
                    .chain()
                    .run_if(
                        in_state(AppState::Playing)
                            .and(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                    ),
            );
    }
}

// ── Shared resources ──────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct GameplayClock(pub f64);

#[derive(Resource, Default)]
pub struct ActivePitches(pub Vec<PitchInfo>);

#[derive(Resource, Default)]
pub struct MusicStarted(pub bool);

#[derive(Resource, Default)]
pub struct ValidHarpNotes(pub HashSet<String>);

#[derive(Resource, Default)]
pub struct Score {
    pub points: u32,
    pub combo: u32,
    pub max_combo: u32,
}

#[derive(Resource, Default)]
pub struct HitFeedback {
    pub quality: Option<HitQuality>,
    pub timer: f32,
}

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
    pub height_pct: f32,
}

/// Attached to every note entity (both modes). Drives scoring logic.
#[derive(Component)]
pub struct ScheduledNote {
    pub time: f64,
    pub hole: u8,
    pub is_blow: bool,
    /// The pitch string (e.g. "C4") this note expects, pre-computed at spawn.
    pub expected_pitch: String,
    pub hit: bool,
    pub missed: bool,
}

#[derive(Component)]
pub struct BarCell(pub usize);

#[derive(Component)]
pub struct HoleCell(pub u8);

#[derive(Component, Default)]
pub struct HoleState {
    pub brightness: f32,
    pub is_blow: bool,
}

#[derive(Component)]
pub struct CountdownOverlay;

#[derive(Component)]
pub struct CountdownText;

// Score HUD marker components
#[derive(Component)] pub struct ScoreText;
#[derive(Component)] pub struct ComboText;
#[derive(Component)] pub struct FeedbackText;

/// Scoring parameters resolved from the song's chart at game start.
/// Falls back to sensible defaults if the chart doesn't specify them.
#[derive(Resource)]
pub struct ScoringConfig {
    pub perfect_window: f64,   // seconds
    pub good_window: f64,      // seconds
    pub miss_window: f64,      // seconds — after this the combo breaks
    pub combo_enabled: bool,
    pub base_multiplier: f32,
    pub step_multiplier: f32,  // added per note in the current combo
    pub max_multiplier: f32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            perfect_window:  0.060,
            good_window:     0.130,
            miss_window:     0.130,
            combo_enabled:   true,
            base_multiplier: 1.0,
            step_multiplier: 0.1,
            max_multiplier:  4.0,
        }
    }
}

// ── Shared constants ──────────────────────────────────────────────────────────

pub const HOLE_COUNT: usize = 10;
pub const COUNTDOWN: f64 = 3.0;
pub const LANE_PCT: f32 = 100.0 / HOLE_COUNT as f32;
pub const HIT_H_PCT: f32 = 7.0;
pub const LOOKAHEAD: f64 = 3.0;

const PERFECT_POINTS: u32 = 100;
const GOOD_POINTS: u32    = 50;

// ── Shared systems ────────────────────────────────────────────────────────────

fn reset_score(mut score: ResMut<Score>, mut feedback: ResMut<HitFeedback>) {
    *score    = Score::default();
    *feedback = HitFeedback::default();
}

fn setup_scoring_config(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut config: ResMut<ScoringConfig>,
) {
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let s = &manifest.chart.scoring;

    config.perfect_window = s.perfect_window_ms as f64 / 1000.0;
    config.good_window    = s.good_window_ms    as f64 / 1000.0;
    config.miss_window    = s.miss_window_ms    as f64 / 1000.0;

    if let Some(combo) = &s.combo {
        config.combo_enabled    = combo.enabled;
        config.base_multiplier  = combo.base_multiplier;
        config.step_multiplier  = combo.step_multiplier;
        config.max_multiplier   = combo.max_multiplier;
    }

    info!(
        "Scoring config: perfect={:.0}ms good={:.0}ms miss={:.0}ms combo={}",
        config.perfect_window * 1000.0,
        config.good_window    * 1000.0,
        config.miss_window    * 1000.0,
        config.combo_enabled,
    );
}

fn tick_clock(mut clock: ResMut<GameplayClock>, time: Res<Time>) {
    clock.0 += time.delta_secs_f64();
}

fn collect_pitches(mut reader: MessageReader<PitchEvent>, mut active: ResMut<ActivePitches>) {
    for ev in reader.read() {
        active.0 = ev.0.clone();
    }
}

fn score_notes(
    clock: Res<GameplayClock>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    config: Res<ScoringConfig>,
    mut notes: Query<&mut ScheduledNote>,
    mut score: ResMut<Score>,
    mut feedback: ResMut<HitFeedback>,
) {
    // Don't score during the countdown
    if clock.0 < 0.0 { return; }

    // Only consider pitches the harmonica can produce
    let harp_pitches: Vec<String> = active
        .0
        .iter()
        .filter(|p| valid_notes.0.contains(&format!("{}{}", p.note, p.octave)))
        .map(|p| format!("{}{}", p.note, p.octave))
        .collect();

    for mut note in &mut notes {
        if note.hit || note.missed { continue; }

        let offset = clock.0 - note.time; // positive = clock is past the note

        // Note passed the miss window — combo breaks
        if offset > config.miss_window {
            note.missed = true;
            if config.combo_enabled { score.combo = 0; }
            continue;
        }

        // Note not yet inside the scoring window
        if offset < -config.good_window { continue; }

        // Note is unhittable (past good window) but miss_window hasn't been
        // reached yet — wait silently without breaking the combo
        if offset > config.good_window { continue; }

        // In window — check if the player is playing this note right now
        if !harp_pitches.contains(&note.expected_pitch) { continue; }

        note.hit = true;
        score.combo += 1;
        score.max_combo = score.max_combo.max(score.combo);

        let quality = if offset.abs() <= config.perfect_window {
            HitQuality::Perfect
        } else {
            HitQuality::Good
        };

        let base = if quality == HitQuality::Perfect { PERFECT_POINTS } else { GOOD_POINTS };
        let multiplier = if config.combo_enabled {
            (config.base_multiplier + score.combo as f32 * config.step_multiplier)
                .min(config.max_multiplier)
        } else {
            1.0
        };
        score.points += (base as f32 * multiplier) as u32;

        feedback.quality = Some(quality);
        feedback.timer   = 0.75;
    }
}

fn update_score_display(
    score: Res<Score>,
    mut feedback: ResMut<HitFeedback>,
    time: Res<Time>,
    mut q_score: Query<
        &mut Text,
        (With<ScoreText>, Without<ComboText>, Without<FeedbackText>),
    >,
    mut q_combo: Query<
        &mut Text,
        (With<ComboText>, Without<ScoreText>, Without<FeedbackText>),
    >,
    mut q_feedback: Query<
        (&mut Text, &mut TextColor),
        (With<FeedbackText>, Without<ScoreText>, Without<ComboText>),
    >,
) {
    for mut t in &mut q_score {
        t.0 = format!("{}", score.points);
    }

    for mut t in &mut q_combo {
        t.0 = if score.combo > 1 {
            let mult = (1 + score.combo / 10).min(4);
            if mult > 1 {
                format!("\u{00D7}{} [\u{00D7}{} pts]", score.combo, mult)
            } else {
                format!("\u{00D7}{}", score.combo)
            }
        } else {
            String::new()
        };
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
                    HitQuality::Good    => ("GOOD",     0.40,    1.00, 0.35),
                };
                t.0    = label.to_string();
                *color = TextColor(Color::srgba(r, g, b, alpha));
                if feedback.timer == 0.0 {
                    feedback.quality = None;
                }
            }
        }
    }
}

fn cleanup_gameplay(mut commands: Commands, roots: Query<Entity, With<GameplayRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}
