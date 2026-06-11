mod gameplay_2d;
mod gameplay_3d;
mod scoring;

use std::collections::HashSet;
use bevy::prelude::*;
use scoring::{
    classify_note, combo_label, compute_multiplier, compute_points,
    should_decay_combo, NoteOutcome,
};

use crate::{
    assets_management::GlobalFonts,
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
            .init_resource::<ActiveTargets>()
            .init_resource::<Paused>()
            // Setup: shared pause menu + mode-specific scenes
            .add_systems(
                OnEnter(AppState::Playing),
                (
                    reset_score,
                    setup_scoring_config,
                    setup_pause_menu,
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
            // Pause input always runs during Playing (even when paused)
            .add_systems(
                Update,
                (handle_pause_input, handle_pause_buttons, pause_button_hover)
                    .run_if(in_state(AppState::Playing)),
            )
            // Gameplay-logic chains only run when not paused
            .add_systems(
                Update,
                (tick_clock, collect_pitches, update_active_targets, score_notes, update_score_display)
                    .chain()
                    .run_if(
                        in_state(AppState::Playing)
                            .and_then(|p: Res<Paused>| !p.0),
                    ),
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
                            .and_then(|p: Res<Paused>| !p.0)
                            .and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
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

/// Set to true while gameplay is paused; all update chains gate on `!paused`.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// Marks the music audio entity so it can be found for pause/resume.
#[derive(Component)]
pub struct MusicPlayer;

#[derive(Component)]
struct PauseMenuRoot;

#[derive(Component)]
enum PauseButton {
    Resume,
    QuitSong,
}

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
            decay_secs:      None,
        }
    }
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

// ── Shared systems ────────────────────────────────────────────────────────────

fn reset_score(
    mut score: ResMut<Score>,
    mut feedback: ResMut<HitFeedback>,
    mut paused: ResMut<Paused>,
) {
    *score    = Score::default();
    *feedback = HitFeedback::default();
    paused.0  = false;
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
        config.decay_secs       = combo.decay_ms.map(|ms| ms as f64 / 1000.0);
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

fn update_active_targets(
    clock: Res<GameplayClock>,
    config: Res<ScoringConfig>,
    notes: Query<&ScheduledNote>,
    mut targets: ResMut<ActiveTargets>,
) {
    targets.0.clear();
    if clock.0 < 0.0 { return; }
    for note in &notes {
        if note.hit || note.missed { continue; }
        if (clock.0 - note.time).abs() <= config.good_window {
            targets.0.push((note.hole, note.is_blow));
        }
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
    if clock.0 < 0.0 { return; }

    if config.combo_enabled
        && should_decay_combo(score.combo, clock.0, score.last_hit_time, config.decay_secs)
    {
        score.combo = 0;
    }

    let harp_pitches: Vec<String> = active
        .0
        .iter()
        .filter(|p| valid_notes.0.contains(&format!("{}{}", p.note, p.octave)))
        .map(|p| format!("{}{}", p.note, p.octave))
        .collect();

    for mut note in &mut notes {
        if note.hit || note.missed { continue; }

        let offset = clock.0 - note.time;
        let playing = harp_pitches.contains(&note.expected_pitch);

        match classify_note(offset, playing, config.perfect_window, config.good_window, config.miss_window) {
            NoteOutcome::Missed => {
                note.missed = true;
                if config.combo_enabled { score.combo = 0; }
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap | NoteOutcome::Waiting => {}
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                score.last_hit_time = clock.0;
                score.combo += 1;
                score.max_combo = score.max_combo.max(score.combo);
                let multiplier = if config.combo_enabled {
                    compute_multiplier(score.combo, config.base_multiplier, config.step_multiplier, config.max_multiplier)
                } else {
                    1.0
                };
                score.points += compute_points(quality, multiplier);
                feedback.quality = Some(quality);
                feedback.timer   = 0.75;
            }
        }
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

fn setup_pause_menu(mut commands: Commands, fonts: Res<GlobalFonts>) {
    let font = fonts.gameplay.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            GlobalZIndex(200),
            Visibility::Hidden,
            GameplayRoot,
            PauseMenuRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("PAUSED"),
                TextFont { font_size: FontSize::Px(52.0), font: font.clone(), ..default() },
                TextColor(Color::WHITE),
            ));
            spawn_pause_button(p, "Resume",   PauseButton::Resume,  &font);
            spawn_pause_button(p, "Quit Song", PauseButton::QuitSong, &font);
        });
}

fn spawn_pause_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    btn: PauseButton,
    font: &FontSource,
) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(220.0),
                padding: UiRect::axes(Val::Px(28.0), Val::Px(12.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            btn,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FontSize::Px(20.0), font: font.clone(), ..default() },
                TextColor(Color::WHITE),
            ));
        });
}

fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) { return; }
    paused.0 = !paused.0;
    for mut vis in &mut overlay {
        *vis = if paused.0 { Visibility::Visible } else { Visibility::Hidden };
    }
    for sink in &sinks {
        if paused.0 { sink.pause(); } else { sink.play(); }
    }
}

fn handle_pause_buttons(
    buttons: Query<(&Interaction, &PauseButton), Changed<Interaction>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    mut next_state: ResMut<NextState<AppState>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed { continue; }
        match button {
            PauseButton::Resume => {
                paused.0 = false;
                for mut vis in &mut overlay { *vis = Visibility::Hidden; }
                for sink in &sinks { sink.play(); }
            }
            PauseButton::QuitSong => {
                paused.0 = false;
                next_state.set(AppState::Menu);
            }
        }
    }
}

fn pause_button_hover(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<PauseButton>),
    >,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = BackgroundColor(match interaction {
            Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
            Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
            Interaction::None    => Color::srgb(0.14, 0.14, 0.22),
        });
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
}
