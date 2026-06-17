// SPDX-License-Identifier: MIT

//! Latency calibration screen.
//!
//! Plays a 60-BPM metronome and asks the player to blow into the harmonica on
//! each beat. After 8 recorded hits the screen shows the mean timing offset and
//! offers to apply it directly to `AudioSettings.input_latency_ms`.
//!
//! Entry: Options page → "Calibrate input lag" button.
//! Exit:  "Apply" or "Cancel" both return to the Options page.

use bevy::{
    audio::{AudioSource, Volume},
    prelude::*,
};

use crate::{
    assets_management::GlobalFonts,
    audio_system::pitch_detect::PitchEvent,
    settings::AudioSettings,
};

use super::{AppState, ReturnToOptions, btn_default};

// ── Constants ─────────────────────────────────────────────────────────────────

const BPM: f64 = 60.0;
const BEAT_DUR: f64 = 60.0 / BPM; // 1.0 s
/// Skip this many leading beats so the player has time to settle in.
const WARMUP_BEATS: u32 = 2;
/// Number of on-beat hits to collect before showing results.
const BEATS_NEEDED: usize = 8;
/// A detected pitch counts as "on beat N" when it falls within this window
/// (±seconds around the beat). 450 ms is generous but avoids false negatives
/// at the start of each beat.
const HIT_WINDOW: f64 = 0.45;

// ── Internal state ────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Copy)]
enum CalPhase {
    #[default]
    Waiting,
    Recording,
    Done,
}

#[derive(Resource, Default)]
struct CalState {
    phase: CalPhase,
    /// Calibration-local clock; only advances while Recording.
    clock: f64,
    /// Total beat crossings fired (including warmup).
    beat_count: u32,
    /// Compensated timing offsets (seconds) for each recorded hit.
    offsets: Vec<f64>,
    /// Whether a pitch was present on the previous frame (for attack detection).
    prev_has_pitch: bool,
    /// Prevents recording two hits for the same beat.
    last_recorded_beat: Option<i64>,
}

impl CalState {
    fn reset(&mut self) {
        *self = CalState::default();
    }

    fn mean_offset_ms(&self) -> Option<f64> {
        if self.offsets.is_empty() {
            return None;
        }
        Some(self.offsets.iter().sum::<f64>() / self.offsets.len() as f64 * 1000.0)
    }
}

/// Click sounds loaded once at app startup, reused by calibration.
#[derive(Resource)]
struct CalSounds {
    downbeat: Handle<AudioSource>,
    beat: Handle<AudioSource>,
}

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct CalRoot;

#[derive(Component, PartialEq, Clone, Copy)]
enum CalBtn {
    Start,
    Apply,
    TryAgain,
    Cancel,
}

/// One of the four beat-indicator squares.
#[derive(Component)]
struct BeatDot(usize);

/// The single line of dynamic status text (hit counter, result, etc.).
#[derive(Component)]
struct CalStatusText;

/// The result block (hidden until Done).
#[derive(Component)]
struct CalResultBlock;

/// The mean-offset line inside the result block.
#[derive(Component)]
struct CalMeanText;

/// The "suggested latency" line inside the result block.
#[derive(Component)]
struct CalSuggestedText;

// Only-during-Waiting / only-during-Done visibility guards.
#[derive(Component)]
struct ShowWaiting;
#[derive(Component)]
struct ShowDone;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct CalibrationPlugin;

impl Plugin for CalibrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CalState>()
            .add_systems(Startup, load_sounds)
            .add_systems(OnEnter(AppState::Calibration), (setup_ui, reset_state))
            .add_systems(OnExit(AppState::Calibration), cleanup)
            .add_systems(
                Update,
                (
                    tick,
                    play_clicks,
                    collect_hits,
                    update_beat_dots,
                    update_status,
                    update_result_block,
                    sync_phase_visibility,
                    handle_buttons,
                    button_hover,
                )
                    .run_if(in_state(AppState::Calibration)),
            );
    }
}

// ── Asset loading ─────────────────────────────────────────────────────────────

fn load_sounds(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(CalSounds {
        downbeat: asset_server.load("sounds/metronome_high.ogg"),
        beat: asset_server.load("sounds/metronome_low.ogg"),
    });
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

fn reset_state(mut cal: ResMut<CalState>) {
    cal.reset();
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<CalRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

// ── Core systems ──────────────────────────────────────────────────────────────

fn tick(time: Res<Time>, mut cal: ResMut<CalState>) {
    if cal.phase != CalPhase::Recording {
        return;
    }
    cal.clock += time.delta_secs_f64();

    // Count beat crossings (used for warmup tracking).
    let beat = (cal.clock / BEAT_DUR).floor() as u32;
    if beat > cal.beat_count {
        cal.beat_count = beat;
    }
}

/// Plays one metronome click per beat, using the same sounds as gameplay.
fn play_clicks(
    cal: Res<CalState>,
    sounds: Res<CalSounds>,
    audio: Res<AudioSettings>,
    mut commands: Commands,
    mut last_click: Local<Option<i64>>,
) {
    if cal.phase != CalPhase::Recording {
        *last_click = None;
        return;
    }
    let beat = (cal.clock / BEAT_DUR).floor() as i64;
    if *last_click == Some(beat) {
        return;
    }
    *last_click = Some(beat);
    let sample = if beat % 4 == 0 {
        sounds.downbeat.clone()
    } else {
        sounds.beat.clone()
    };
    commands.spawn((
        AudioPlayer::<AudioSource>(sample),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.metronome_volume)),
    ));
}

/// On each pitch attack (silence → sound), records the offset vs. the nearest
/// beat if it's within HIT_WINDOW and past the warmup period.
fn collect_hits(
    mut pitches: MessageReader<PitchEvent>,
    mut cal: ResMut<CalState>,
) {
    if cal.phase != CalPhase::Recording {
        return;
    }

    let mut has_pitch = false;
    for ev in pitches.read() {
        if !ev.0.is_empty() {
            has_pitch = true;
        }
    }
    let is_attack = has_pitch && !cal.prev_has_pitch;
    cal.prev_has_pitch = has_pitch;

    if !is_attack {
        return;
    }
    if cal.beat_count <= WARMUP_BEATS {
        return;
    }

    let nearest_beat = (cal.clock / BEAT_DUR).round() as i64;
    let offset = cal.clock - nearest_beat as f64 * BEAT_DUR;
    if offset.abs() > HIT_WINDOW {
        return;
    }
    if cal.last_recorded_beat == Some(nearest_beat) {
        return;
    }

    cal.offsets.push(offset);
    cal.last_recorded_beat = Some(nearest_beat);

    if cal.offsets.len() >= BEATS_NEEDED {
        cal.phase = CalPhase::Done;
    }
}

// ── Visual update systems ─────────────────────────────────────────────────────

fn update_beat_dots(
    cal: Res<CalState>,
    mut dots: Query<(&BeatDot, &mut BackgroundColor)>,
) {
    let (active, phase_f) = if cal.phase == CalPhase::Recording {
        let pos = cal.clock / BEAT_DUR;
        (pos.floor() as usize % 4, pos.fract() as f32)
    } else {
        (usize::MAX, 0.0)
    };

    for (dot, mut bg) in &mut dots {
        if dot.0 == active {
            let b = (1.0 - phase_f).powf(1.5);
            *bg = BackgroundColor(Color::srgb(0.25 + b * 0.75, 0.55 + b * 0.45, 0.95));
        } else {
            *bg = BackgroundColor(Color::srgb(0.12, 0.12, 0.20));
        }
    }
}

fn update_status(
    cal: Res<CalState>,
    mut texts: Query<&mut Text, With<CalStatusText>>,
) {
    if !cal.is_changed() {
        return;
    }
    let msg: String = match cal.phase {
        CalPhase::Waiting => {
            "Play any note on each beat.\nThe game measures your mic latency.".into()
        }
        CalPhase::Recording => {
            if cal.beat_count <= WARMUP_BEATS {
                "Get ready…".into()
            } else {
                format!("{} / {} hits", cal.offsets.len(), BEATS_NEEDED)
            }
        }
        CalPhase::Done => "Calibration complete!".into(),
    };
    for mut t in &mut texts {
        t.0 = msg.clone();
    }
}

fn update_result_block(
    cal: Res<CalState>,
    audio: Res<AudioSettings>,
    mut mean_texts: Query<(&mut Text, &mut TextColor), With<CalMeanText>>,
    mut suggested_texts: Query<&mut Text, (With<CalSuggestedText>, Without<CalMeanText>)>,
) {
    if !cal.is_changed() || cal.phase != CalPhase::Done {
        return;
    }
    if let Some(ms) = cal.mean_offset_ms() {
        let sign = if ms >= 0.0 { "+" } else { "" };
        for (mut t, mut color) in &mut mean_texts {
            t.0 = format!("Mean offset: {sign}{ms:.0}ms");
            color.0 = if ms.abs() < 10.0 {
                Color::srgb(0.45, 1.00, 0.45)
            } else {
                Color::srgb(0.95, 0.62, 0.30)
            };
        }
        let suggested = (audio.input_latency_ms + ms.round() as i32).max(0);
        for mut t in &mut suggested_texts {
            t.0 = format!(
                "Current: {}ms   →   Suggested: {}ms",
                audio.input_latency_ms, suggested
            );
        }
    }
}

fn sync_phase_visibility(
    cal: Res<CalState>,
    mut waiting: Query<&mut Visibility, (With<ShowWaiting>, Without<ShowDone>)>,
    mut done: Query<&mut Visibility, (With<ShowDone>, Without<ShowWaiting>)>,
) {
    if !cal.is_changed() {
        return;
    }
    let show_w = cal.phase == CalPhase::Waiting;
    let show_d = cal.phase == CalPhase::Done;
    for mut v in &mut waiting {
        *v = if show_w { Visibility::Inherited } else { Visibility::Hidden };
    }
    for mut v in &mut done {
        *v = if show_d { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn handle_buttons(
    buttons: Query<(&Interaction, &CalBtn), Changed<Interaction>>,
    mut cal: ResMut<CalState>,
    mut audio: ResMut<AudioSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_options: ResMut<ReturnToOptions>,
) {
    for (interaction, btn) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            CalBtn::Start => {
                cal.reset();
                cal.phase = CalPhase::Recording;
            }
            CalBtn::Apply => {
                if let Some(ms) = cal.mean_offset_ms() {
                    audio.input_latency_ms =
                        (audio.input_latency_ms + ms.round() as i32).max(0);
                }
                return_to_options.0 = true;
                next_state.set(AppState::Menu);
            }
            CalBtn::TryAgain => {
                cal.reset();
                cal.phase = CalPhase::Recording;
            }
            CalBtn::Cancel => {
                return_to_options.0 = true;
                next_state.set(AppState::Menu);
            }
        }
    }
}

fn button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<CalBtn>)>,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = BackgroundColor(match interaction {
            Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
            Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
            Interaction::None => btn_default(),
        });
    }
}

// ── UI construction ───────────────────────────────────────────────────────────

fn setup_ui(mut commands: Commands, fonts: Res<GlobalFonts>) {
    let font = fonts.gameplay.clone();

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            CalRoot,
        ))
        .id();

    commands.entity(root).with_children(|p| {
        // Title
        p.spawn((
            Text::new("Latency Calibration"),
            TextFont { font_size: FontSize::Px(44.0), font: font.clone(), ..default() },
            TextColor(Color::WHITE),
        ));

        // Instructions / status (updated live)
        p.spawn((
            Text::new("Play any note on each beat.\nThe game measures your mic latency."),
            TextFont { font_size: FontSize::Px(18.0), font: font.clone(), ..default() },
            TextColor(Color::srgb(0.65, 0.68, 0.78)),
            TextLayout { justify: Justify::Center, ..default() },
            CalStatusText,
        ));

        // Beat dots (4 squares, current beat lights up)
        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(20.0),
            ..default()
        })
        .with_children(|row| {
            for i in 0..4 {
                row.spawn((
                    Node {
                        width: Val::Px(48.0),
                        height: Val::Px(48.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.12, 0.20)),
                    BeatDot(i),
                ));
            }
        });

        // Result block (hidden until Done)
        p.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            Visibility::Hidden,
            ShowDone,
        ))
        .with_children(|block| {
            block.spawn((
                Text::new("Mean offset: —"),
                TextFont { font_size: FontSize::Px(22.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.95, 0.62, 0.30)),
                CalMeanText,
            ));
            block.spawn((
                Text::new("Current: —   →   Suggested: —"),
                TextFont { font_size: FontSize::Px(18.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.65, 0.68, 0.78)),
                CalSuggestedText,
            ));
        });

        // Button row (Waiting): Start + Cancel
        p.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                ..default()
            },
            ShowWaiting,
        ))
        .with_children(|row| {
            spawn_cal_button(row, &font, "Start", CalBtn::Start);
            spawn_cal_button(row, &font, "Cancel", CalBtn::Cancel);
        });

        // Button row (Done): Apply + Try Again + Cancel
        p.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                ..default()
            },
            Visibility::Hidden,
            ShowDone,
        ))
        .with_children(|row| {
            spawn_cal_button(row, &font, "Apply", CalBtn::Apply);
            spawn_cal_button(row, &font, "Try Again", CalBtn::TryAgain);
            spawn_cal_button(row, &font, "Cancel", CalBtn::Cancel);
        });
    });
}

fn spawn_cal_button(parent: &mut ChildSpawnerCommands, font: &FontSource, label: &str, btn: CalBtn) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(160.0),
                padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(btn_default()),
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(offsets: &[f64]) -> CalState {
        CalState {
            phase: CalPhase::Done,
            offsets: offsets.to_vec(),
            ..default()
        }
    }

    #[test]
    fn no_hits_gives_no_mean() {
        assert_eq!(make_state(&[]).mean_offset_ms(), None);
    }

    #[test]
    fn perfectly_timed_hits_give_zero_mean() {
        let ms = make_state(&[0.0, 0.0, 0.0, 0.0]).mean_offset_ms().unwrap();
        assert!(ms.abs() < 1e-9);
    }

    #[test]
    fn consistently_late_hits_report_positive_mean() {
        // 8 hits each 70 ms late
        let offsets: Vec<f64> = vec![0.070; 8];
        let ms = make_state(&offsets).mean_offset_ms().unwrap();
        assert!((ms - 70.0).abs() < 0.1, "expected 70ms, got {ms}");
    }

    #[test]
    fn mixed_offsets_average_correctly() {
        // Two hits: one 40 ms late, one 60 ms late → mean 50 ms
        let ms = make_state(&[0.040, 0.060]).mean_offset_ms().unwrap();
        assert!((ms - 50.0).abs() < 0.1, "expected 50ms, got {ms}");
    }

    #[test]
    fn apply_adds_mean_to_current_latency() {
        // If current latency is 20 ms and mean offset is +50 ms, apply → 70 ms.
        let mut cal = make_state(&[0.050; 4]);
        let mean_ms = cal.mean_offset_ms().unwrap();
        let current = 20_i32;
        let suggested = (current + mean_ms.round() as i32).max(0);
        assert_eq!(suggested, 70);
        // Should not go negative even if mean is very negative.
        cal.offsets = vec![-0.200; 4];
        let mean_ms = cal.mean_offset_ms().unwrap();
        let suggested = (current + mean_ms.round() as i32).max(0);
        assert_eq!(suggested, 0);
    }
}
