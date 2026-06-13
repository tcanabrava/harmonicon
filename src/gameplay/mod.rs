mod countdown_overlay;
mod gameplay_2d;
mod gameplay_3d;
mod jam_session;
mod metronome_overlay;
mod modifier_legend;
mod note_shape_material;
mod phrase_overlay;
mod results;
mod scoring;
mod twelve_bar_blues_overlay;

use bevy::prelude::*;
use scoring::{
    NoteOutcome, classify_note, combo_label, compute_multiplier, compute_points, should_decay_combo,
};
use std::collections::HashMap;
use std::collections::HashSet;

use crate::{
    assets_management::GlobalFonts,
    audio_system::pitch_detect::{PitchEvent, PitchInfo},
    menu::{AppState, GameplayMode, SelectedSong},
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
            note_shape_material::NoteShapePlugin,
        ))
        .init_resource::<GameplayClock>()
            .init_resource::<ActivePitches>()
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
                    setup_pause_menu,
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
                (handle_pause_input, handle_pause_buttons, pause_button_hover)
                    .run_if(in_state(AppState::Playing)),
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

#[derive(Component)]
struct PauseMenuRoot;

#[derive(Component)]
enum PauseButton {
    Resume,
    Restart,
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
    /// Beats per bar resolved from `timing.time_signature_map` (or `song.time_signature`).
    pub beats_per_bar: f64,
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

/// Accent colour for a note-technique modifier, shared by the 2D badge/border
/// hints and the 3D note-material tint so both modes read the same.
pub fn modifier_color(m: &crate::song::chart::Modifier) -> Color {
    use crate::song::chart::Modifier::*;
    match m {
        Bend { .. } => Color::srgb(0.78, 0.42, 0.96),
        Vibrato { .. } => Color::srgb(0.35, 0.82, 0.96),
        WahWah { .. } => Color::srgb(1.00, 0.72, 0.25),
        Hold { .. } => Color::srgb(0.85, 0.85, 0.92),
        Overblow => Color::srgb(0.32, 0.88, 0.62),
        Overdraw => Color::srgb(0.96, 0.55, 0.30),
    }
}

/// Short badge abbreviation for a modifier, shared by the note badges and the
/// HUD legend so the two never drift apart.
pub fn modifier_abbrev(m: &crate::song::chart::Modifier) -> &'static str {
    use crate::song::chart::Modifier::*;
    match m {
        Bend { .. } => "\u{266D}",
        Vibrato { .. } => "vib",
        WahWah { .. } => "wah",
        Hold { .. } => "hold",
        Overblow => "ob",
        Overdraw => "od",
    }
}

// ── Shared systems ────────────────────────────────────────────────────────────

fn reset_score(
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut paused: ResMut<Paused>,
) {
    *score = Score::default();
    *stats = SongStats::default();
    *feedback = HitFeedback::default();
    paused.0 = false;
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
    let last_note_end = chart
        .track
        .iter()
        .map(|item| resolve_item_time(item, &chart.timing) + item.duration)
        .fold(0.0_f64, f64::max);
    song_end.0 = if loop_cfg.active {
        f64::INFINITY
    } else {
        last_note_end + SONG_END_TAIL
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
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    config: Res<ScoringConfig>,
    fx_mapping: Res<FxMapping>,
    mut notes: Query<&mut ScheduledNote>,
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
) {
    if clock.0 < 0.0 {
        return;
    }

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
        if note.hit || note.missed {
            continue;
        }

        let offset = clock.0 - note.time;
        let playing = harp_pitches.contains(&note.expected_pitch);

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
        Modifier::Hold { .. } => "hold",
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
                TextFont {
                    font_size: FontSize::Px(52.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            spawn_pause_button(p, "Resume", PauseButton::Resume, &font);
            spawn_pause_button(p, "Restart", PauseButton::Restart, &font);
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
                TextFont {
                    font_size: FontSize::Px(20.0),
                    font: font.clone(),
                    ..default()
                },
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
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    paused.0 = !paused.0;
    for mut vis in &mut overlay {
        *vis = if paused.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for sink in &sinks {
        if paused.0 {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

fn handle_pause_buttons(
    buttons: Query<(&Interaction, &PauseButton), Changed<Interaction>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<crate::menu::ReturnToSongList>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            PauseButton::Resume => {
                paused.0 = false;
                for mut vis in &mut overlay {
                    *vis = Visibility::Hidden;
                }
                for sink in &sinks {
                    sink.play();
                }
            }
            PauseButton::Restart => {
                paused.0 = false;
                // Re-enter via SongLoading so the whole song setup runs fresh
                // (the asset is already loaded, so it resumes immediately).
                next_state.set(AppState::SongLoading);
            }
            PauseButton::QuitSong => {
                paused.0 = false;
                // Land back on the song list, not the main menu.
                return_to_song_list.0 = true;
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
            Interaction::None => Color::srgb(0.14, 0.14, 0.22),
        });
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
}
