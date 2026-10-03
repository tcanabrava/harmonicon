// SPDX-License-Identifier: MIT

//! Latency calibration from note attacks played against metronome beats.

use bevy::{
    audio::{AudioSource, Volume},
    ecs::system::IntoObserverSystem,
    input_focus::tab_navigation::TabGroup,
    prelude::*,
    ui_widgets::Activate,
};
use std::fmt::Write;

use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::PitchEvent;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::button;

use harmonicon_app::app::{AppState, ReturnToOptions};

// ── Constants ─────────────────────────────────────────────────────────────────

const BPM: f64 = 60.0;
const BEAT_DUR: f64 = 60.0 / BPM; // 1.0 s
const WARMUP_BEATS: u32 = 2;
const BEATS_NEEDED: usize = 8;
const HIT_WINDOW: f64 = 0.45; // ±450 ms around each beat

// Timing bar: what range to display (±200 ms) and game window sizes.
const BAR_RANGE_MS: f64 = 200.0; // ±200 ms displayed
const PERFECT_MS: f64 = 60.0; // matches ScoringConfig default
const GOOD_MS: f64 = 130.0; // matches ScoringConfig default
const BAR_WIDTH_PX: f32 = 360.0;

// How long a hit-marker stays visible before fading away.
const MARKER_LIFETIME: f32 = 2.5;

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Copy)]
enum CalPhase {
    #[default]
    Waiting,
    Recording,
    Done,
}

#[derive(Resource, Default)]
struct CalState {
    generation: u64,
    phase: CalPhase,
    clock: f64,
    beat_count: u32,
    offsets: Vec<f64>, // seconds, positive = late
    prev_has_pitch: bool,
    last_recorded_beat: Option<i64>,
}

impl CalState {
    fn reset(&mut self) {
        let generation = self.generation.wrapping_add(1);
        let offsets = std::mem::take(&mut self.offsets);
        *self = Self { generation, offsets, ..default() };
        self.offsets.clear();
    }

    fn mean_offset_ms(&self) -> Option<f64> {
        if self.offsets.is_empty() {
            return None;
        }
        Some(self.offsets.iter().sum::<f64>() / self.offsets.len() as f64 * 1000.0)
    }
}

#[derive(Resource)]
struct CalSounds {
    downbeat: Handle<AudioSource>,
    beat: Handle<AudioSource>,
}

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct CalRoot;

#[derive(Component, Default, Clone)]
struct BeatDot(usize);

#[derive(Component, Default, Clone)]
struct MicBarFill;

/// Container for the timing-window bar. Hit markers are added as children.
#[derive(Component)]
struct TimingBarContainer;

/// A per-hit tick mark inside the timing bar.
#[derive(Component, Default, Clone)]
struct TimingMarker {
    offset_secs: f64,
    age: f32,
}

/// Text showing the per-hit offset list ("±Xms  ±Yms …").
#[derive(Component, Default, Clone)]
struct HitOffsetsSummary;

/// Dynamic status / counter line.
#[derive(Component, Default, Clone)]
struct CalStatusText;

/// Result block — shown only after Done.
#[derive(Component, Default, Clone)]
struct CalMeanText;
#[derive(Component, Default, Clone)]
struct CalSuggestedText;

// Phase-gated visibility markers.
#[derive(Component)]
struct ShowWaiting;
#[derive(Component, Default, Clone)]
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
                    update_mic_bar,
                    sync_hit_markers,
                    fade_hit_markers,
                    update_offset_summary,
                    update_status,
                    update_result_block,
                    sync_phase_visibility,
                    handle_escape,
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
    cal.bypass_change_detection().clock += time.delta_secs_f64();
    let beat = (cal.clock / BEAT_DUR).floor() as u32;
    if beat > cal.beat_count {
        cal.beat_count = beat;
    }
}

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
    let sample = if beat % 4 == 0 { sounds.downbeat.clone() } else { sounds.beat.clone() };
    commands.spawn((
        AudioPlayer::<AudioSource>(sample),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.metronome_volume)),
    ));
}

fn collect_hits(mut pitches: MessageReader<PitchEvent>, mut cal: ResMut<CalState>) {
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
    cal.bypass_change_detection().prev_has_pitch = has_pitch;

    if !is_attack || cal.beat_count <= WARMUP_BEATS {
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

fn update_beat_dots(cal: Res<CalState>, mut dots: Query<(&BeatDot, &mut BackgroundColor)>) {
    if cal.phase != CalPhase::Recording && !cal.is_changed() {
        return;
    }
    let (active, phase_f) = if cal.phase == CalPhase::Recording {
        let pos = cal.clock / BEAT_DUR;
        (pos.floor() as usize % 4, pos.fract() as f32)
    } else {
        (usize::MAX, 0.0)
    };
    for (dot, mut bg) in &mut dots {
        let wanted = if dot.0 == active {
            let b = (1.0 - phase_f).powf(1.5);
            BackgroundColor(Color::srgb(0.25 + b * 0.75, 0.55 + b * 0.45, 0.95))
        } else {
            BackgroundColor(Color::srgb(0.12, 0.12, 0.20))
        };
        if *bg != wanted {
            *bg = wanted;
        }
    }
}

/// Mic-level bar: fast attack, slow decay; reads pitch events independently
/// from `collect_hits` (Bevy messages are multi-consumer).
fn update_mic_bar(
    mut pitches: MessageReader<PitchEvent>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<MicBarFill>>,
    mut level: Local<f32>,
    time: Res<Time>,
) {
    let has_pitch = pitches.read().any(|ev| !ev.0.is_empty());
    let dt = time.delta_secs();
    let target = if has_pitch { 1.0_f32 } else { 0.0 };
    let rate = if has_pitch { 30.0 } else { 5.0 };
    *level += (target - *level) * rate * dt;
    *level = level.clamp(0.0, 1.0);

    for (mut node, mut bg) in &mut fills {
        let width = Val::Percent(*level * 100.0);
        if node.width != width {
            node.width = width;
        }
        let color = if *level > 0.1 {
            Color::srgb(0.20, 0.75 + *level * 0.25, 0.55 + *level * 0.45)
        } else {
            Color::srgb(0.10, 0.18, 0.14)
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Spawns a coloured tick mark inside the timing bar for every new hit.
fn sync_hit_markers(
    mut commands: Commands,
    cal: Res<CalState>,
    bar: Query<Entity, With<TimingBarContainer>>,
    mut spawned: Local<(u64, usize)>,
) {
    if spawned.0 != cal.generation {
        *spawned = (cal.generation, 0);
    }
    if cal.offsets.len() == spawned.1 {
        return;
    }
    let Ok(bar_entity) = bar.single() else { return };

    for &offset_secs in &cal.offsets[spawned.1..] {
        let ms = offset_secs * 1000.0;
        // Map ms → 0..1 within ±BAR_RANGE_MS.
        let frac = ((ms / BAR_RANGE_MS) * 0.5 + 0.5).clamp(0.0, 1.0) as f32;
        let color = if ms.abs() <= PERFECT_MS {
            Color::srgba(0.25, 1.00, 0.35, 1.0) // green = perfect zone
        } else if ms.abs() <= GOOD_MS {
            Color::srgba(0.95, 0.70, 0.20, 1.0) // orange = good zone
        } else {
            Color::srgba(0.95, 0.35, 0.30, 1.0) // red = outside good window
        };
        commands.entity(bar_entity).with_children(|bar| {
            bar.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    left: {Val::Percent(frac * 100.0)},
                    top: {Val::Percent(0.0)},
                    width: {Val::Px(3.0)},
                    height: {Val::Percent(100.0)},
                }
                BackgroundColor({color})
                TimingMarker { offset_secs: {offset_secs}, age: {0.0f32} }
            });
        });
    }
    spawned.1 = cal.offsets.len();
}

/// Fades hit markers over time and removes them when fully transparent.
fn fade_hit_markers(
    mut commands: Commands,
    time: Res<Time>,
    mut markers: Query<(Entity, &mut TimingMarker, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut marker, mut bg) in &mut markers {
        marker.age += dt;
        let alpha = (1.0 - marker.age / MARKER_LIFETIME).clamp(0.0, 1.0);
        if alpha == 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let ms = marker.offset_secs * 1000.0;
        let rgb = if ms.abs() <= PERFECT_MS {
            (0.25, 1.00, 0.35)
        } else if ms.abs() <= GOOD_MS {
            (0.95, 0.70, 0.20)
        } else {
            (0.95, 0.35, 0.30)
        };
        bg.0 = Color::srgba(rgb.0, rgb.1, rgb.2, alpha);
    }
}

/// Updates the hit-offsets summary text ("±Xms  ±Yms  …").
fn update_offset_summary(
    cal: Res<CalState>,
    mut texts: Query<&mut Text, With<HitOffsetsSummary>>,
    mut shown: Local<(u64, usize)>,
) {
    if *shown == (cal.generation, cal.offsets.len()) {
        return;
    }
    *shown = (cal.generation, cal.offsets.len());
    let mut line = String::new();
    for &offset in &cal.offsets {
        if !line.is_empty() {
            line.push_str("  ");
        }
        let ms = (offset * 1000.0).round() as i32;
        if ms >= 0 {
            write!(line, "+{ms}ms").unwrap();
        } else {
            write!(line, "{ms}ms").unwrap();
        }
    }
    for mut t in &mut texts {
        if t.0 != line {
            t.0.clone_from(&line);
        }
    }
}

fn update_status(
    cal: Res<CalState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<CalStatusText>>,
) {
    if !cal.is_changed() && !loc.is_changed() {
        return;
    }
    let msg = String::from(match cal.phase {
        CalPhase::Waiting => loc.msg("calibration-instructions"),
        CalPhase::Recording if cal.beat_count <= WARMUP_BEATS => loc.msg("calibration-get-ready"),
        CalPhase::Recording => loc.msg_args(
            "calibration-hits-recorded",
            &[("hits", cal.offsets.len().to_string()), ("total", BEATS_NEEDED.to_string())],
        ),
        CalPhase::Done => loc.msg("calibration-complete"),
    });
    for mut t in &mut texts {
        if t.0 != msg {
            t.0.clone_from(&msg);
        }
    }
}

fn update_result_block(
    cal: Res<CalState>,
    audio: Res<AudioSettings>,
    loc: Res<Localization>,
    mut mean_texts: Query<(&mut Text, &mut TextColor), With<CalMeanText>>,
    mut suggested_texts: Query<&mut Text, (With<CalSuggestedText>, Without<CalMeanText>)>,
) {
    if cal.phase != CalPhase::Done
        || (!cal.is_changed() && !audio.is_changed() && !loc.is_changed())
    {
        return;
    }
    if let Some(ms) = cal.mean_offset_ms() {
        let sign = if ms >= 0.0 { "+" } else { "" };
        for (mut t, mut color) in &mut mean_texts {
            t.0 = loc
                .msg_args(
                    "calibration-mean-offset",
                    &[("sign", sign.to_string()), ("ms", format!("{ms:.0}"))],
                )
                .into();
            color.0 = if ms.abs() < 10.0 {
                Color::srgb(0.45, 1.00, 0.45)
            } else {
                Color::srgb(0.95, 0.62, 0.30)
            };
        }
        let suggested = (audio.input_latency_ms + ms.round() as i32).max(0);
        for mut t in &mut suggested_texts {
            t.0 = loc
                .msg_args(
                    "calibration-suggested",
                    &[
                        ("current", audio.input_latency_ms.to_string()),
                        ("suggested", suggested.to_string()),
                    ],
                )
                .into();
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
        let wanted = if show_w { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
    }
    for mut v in &mut done {
        let wanted = if show_d { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
    }
}

// ── Dedicated button callbacks ────────────────────────────────────────────────

/// Start / Try Again: reset and begin a fresh recording pass.
fn begin_recording(_: On<Activate>, mut cal: ResMut<CalState>) {
    cal.reset();
    cal.phase = CalPhase::Recording;
}

/// Apply: fold the measured offset into the input-latency setting, then leave.
fn apply_calibration(
    _: On<Activate>,
    cal: Res<CalState>,
    mut audio: ResMut<AudioSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_options: ResMut<ReturnToOptions>,
) {
    if let Some(ms) = cal.mean_offset_ms() {
        audio.input_latency_ms = (audio.input_latency_ms + ms.round() as i32).max(0);
    }
    return_to_options.0 = true;
    next_state.set(AppState::Menu);
}

/// Cancel: leave without changing the latency setting.
fn cancel_calibration(
    _: On<Activate>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_options: ResMut<ReturnToOptions>,
) {
    return_to_options.0 = true;
    next_state.set(AppState::Menu);
}

/// Escape does the same as Cancel: leave without changing the latency setting.
fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_options: ResMut<ReturnToOptions>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    return_to_options.0 = true;
    next_state.set(AppState::Menu);
}

// ── UI construction ───────────────────────────────────────────────────────────

fn setup_ui(mut commands: Commands, loc: Res<Localization>) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            TabGroup::default(),
            CalRoot,
        ))
        .id();

    commands.entity(root).with_children(|p| {
        // ── Title ─────────────────────────────────────────────────────────────
        p.spawn_empty().apply_scene(bsn! {
            Text({String::from(loc.msg("calibration-title"))})
            TextFont { font_size: {FontSize::Px(38.0)} }
            TextColor({Color::WHITE})
        });

        // ── Status text ───────────────────────────────────────────────────────
        p.spawn_empty().apply_scene(bsn! {
            Text({String::from(loc.msg("calibration-instructions"))})
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::srgb(0.62, 0.65, 0.80)})
            TextLayout { justify: {Justify::Center} }
            Node { max_width: {Val::Px(480.0)} }
            CalStatusText
        });

        // ── Beat dots ─────────────────────────────────────────────────────────
        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(18.0),
            ..default()
        })
        .with_children(|row| {
            for i in 0..4usize {
                row.spawn_empty().apply_scene(bsn! {
                    Node {
                        width: {Val::Px(44.0)},
                        height: {Val::Px(44.0)},
                    }
                    BackgroundColor({Color::srgb(0.12, 0.12, 0.20)})
                    BeatDot(i)
                });
            }
        });

        // ── Mic-level bar ─────────────────────────────────────────────────────
        p.spawn_empty().apply_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Row},
                align_items: {AlignItems::Center},
                column_gap: {Val::Px(12.0)},
            }
            Children [
                Text({String::from(loc.msg("calibration-mic-label"))})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.55, 0.58, 0.68)})
                --
                Node {
                    width: {Val::Px(220.0)},
                    height: {Val::Px(12.0)},
                }
                BackgroundColor({Color::srgb(0.10, 0.12, 0.10)})
                Children [
                    // Fill — width is animated by update_mic_bar.
                    Node {
                        width: {Val::Percent(0.0)},
                        height: {Val::Percent(100.0)},
                    }
                    BackgroundColor({Color::srgb(0.10, 0.18, 0.14)})
                    MicBarFill
                ]
            ]
        });

        // ── Timing window bar ─────────────────────────────────────────────────
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|col| {
            // The bar itself: coloured zones + hit markers as absolute children.
            col.spawn((
                Node {
                    width: Val::Px(BAR_WIDTH_PX),
                    height: Val::Px(28.0),
                    flex_direction: FlexDirection::Row,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
                TimingBarContainer,
            ))
            .with_children(|bar| {
                spawn_timing_zones(bar);
            });

            // Axis labels: -200ms  |  0  |  +200ms
            col.spawn(Node {
                width: Val::Px(BAR_WIDTH_PX),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|labels| {
                for txt in ["-200ms", "0", "+200ms"] {
                    labels.spawn_empty().apply_scene(bsn! {
                        Text({txt})
                        TextFont { font_size: {FontSize::Px(15.0)} }
                        TextColor({Color::srgb(0.40, 0.42, 0.52)})
                    });
                }
            });

            // Per-hit offset summary text
            col.spawn_empty().apply_scene(bsn! {
                Text("")
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.70, 0.72, 0.85)})
                Node { margin: {UiRect::top(Val::Px(2.0))} }
                HitOffsetsSummary
            });
        });

        // ── Result block (Done only) ───────────────────────────────────────────
        p.spawn_empty().apply_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
                row_gap: {Val::Px(6.0)},
            }
            ~{Visibility::Hidden}
            ShowDone
            Children [
                Text({String::from(loc.msg("calibration-mean-offset-placeholder"))})
                TextFont { font_size: {FontSize::Px(20.0)} }
                TextColor({Color::srgb(0.95, 0.62, 0.30)})
                CalMeanText
                --
                Text({String::from(loc.msg("calibration-suggested-placeholder"))})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::srgb(0.62, 0.65, 0.80)})
                CalSuggestedText
            ]
        });

        // ── Buttons ───────────────────────────────────────────────────────────
        //
        //  Waiting:   [Start]    [Cancel]
        //  Recording: (nothing)  [Cancel]
        //  Done:      [Apply] [Try Again]  [Cancel]
        //
        // The Start row (Waiting only) and Done row (Done only) are phase-gated.
        // Cancel is always present in its own row.

        // Waiting-only row
        p.spawn((
            Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(14.0), ..default() },
            ShowWaiting,
        ))
        .with_children(|row| {
            spawn_cal_button(row, &loc.msg("calibration-start"), begin_recording);
        });

        // Done-only row
        p.spawn((
            Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(14.0), ..default() },
            Visibility::Hidden,
            ShowDone,
        ))
        .with_children(|row| {
            spawn_cal_button(row, &loc.msg("calibration-apply"), apply_calibration);
            spawn_cal_button(row, &loc.msg("calibration-try-again"), begin_recording);
        });

        // Cancel — always visible regardless of phase
        p.spawn(Node::default()).with_children(|row| {
            spawn_cal_button(row, &loc.msg("calibration-cancel"), cancel_calibration);
        });
    });
}

/// Bakes the green/orange/dark-red timing-window zones into the bar as fixed
/// child nodes.  Zones are (left_pct, width_pct, color).
fn spawn_timing_zones(bar: &mut ChildSpawnerCommands) {
    // Map ms to 0..1: frac = (ms / BAR_RANGE_MS) * 0.5 + 0.5
    let ms_to_pct =
        |ms: f64| -> f32 { ((ms / BAR_RANGE_MS) * 0.5 + 0.5).clamp(0.0, 1.0) as f32 * 100.0 };

    let left_outer = ms_to_pct(-BAR_RANGE_MS); //   0%
    let left_good = ms_to_pct(-GOOD_MS); //  17.5%
    let left_perf = ms_to_pct(-PERFECT_MS); //  35%
    let right_perf = ms_to_pct(PERFECT_MS); //  65%
    let right_good = ms_to_pct(GOOD_MS); //  82.5%
    let right_outer = ms_to_pct(BAR_RANGE_MS); // 100%

    let zones: &[(f32, f32, Color)] = &[
        (left_outer, left_good - left_outer, Color::srgb(0.30, 0.10, 0.10)), // dark red left
        (left_good, left_perf - left_good, Color::srgb(0.50, 0.32, 0.08)),   // orange left
        (left_perf, right_perf - left_perf, Color::srgb(0.10, 0.38, 0.14)),  // green centre
        (right_perf, right_good - right_perf, Color::srgb(0.50, 0.32, 0.08)), // orange right
        (right_good, right_outer - right_good, Color::srgb(0.30, 0.10, 0.10)), // dark red right
    ];

    for &(left, width, color) in zones {
        bar.spawn_empty().apply_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                left: {Val::Percent(left)},
                top: {Val::Percent(0.0)},
                width: {Val::Percent(width)},
                height: {Val::Percent(100.0)},
            }
            BackgroundColor({color})
        });
    }

    // Centre line (beat target).
    bar.spawn_empty().apply_scene(bsn! {
        Node {
            position_type: {PositionType::Absolute},
            left: {Val::Percent(50.0)},
            top: {Val::Percent(0.0)},
            width: {Val::Px(1.0)},
            height: {Val::Percent(100.0)},
        }
        BackgroundColor({Color::srgba(1.0, 1.0, 1.0, 0.35)})
    });
}

/// One calibration button: shared shell + label, wired with its own dedicated
/// click callback plus the shared hover highlight — all inline `on(...)`.
fn spawn_cal_button<M: 'static>(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    on_click: impl IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) {
    parent.spawn_empty().apply_scene(button::default(label, on_click));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_offsets(offsets: &[f64]) -> CalState {
        CalState { offsets: offsets.to_vec(), ..default() }
    }

    #[test]
    fn no_hits_gives_no_mean() {
        assert_eq!(state_with_offsets(&[]).mean_offset_ms(), None);
    }

    #[test]
    fn perfectly_timed_hits_give_zero_mean() {
        let ms = state_with_offsets(&[0.0; 4]).mean_offset_ms().unwrap();
        assert!(ms.abs() < 1e-9);
    }

    #[test]
    fn consistently_late_hits_report_positive_mean() {
        let ms = state_with_offsets(&[0.070; 8]).mean_offset_ms().unwrap();
        assert!((ms - 70.0).abs() < 0.1, "expected 70ms, got {ms}");
    }

    #[test]
    fn mixed_offsets_average_correctly() {
        let ms = state_with_offsets(&[0.040, 0.060]).mean_offset_ms().unwrap();
        assert!((ms - 50.0).abs() < 0.1, "expected 50ms, got {ms}");
    }

    #[test]
    fn apply_clamps_suggested_latency_to_zero() {
        // If mean is very negative, clamping at 0 prevents negative latency.
        let cal = state_with_offsets(&[-0.200; 4]);
        let ms = cal.mean_offset_ms().unwrap();
        let suggested = (20_i32 + ms.round() as i32).max(0);
        assert_eq!(suggested, 0);
    }

    #[test]
    fn marker_colour_matches_timing_zone() {
        // Perfect zone: |ms| ≤ 60
        let ms = 30.0_f64;
        let in_perfect = ms.abs() <= PERFECT_MS;
        assert!(in_perfect);
        // Good zone: 60 < |ms| ≤ 130
        let ms = 90.0_f64;
        let in_good = ms.abs() > PERFECT_MS && ms.abs() <= GOOD_MS;
        assert!(in_good);
        // Outside: |ms| > 130
        let ms = 160.0_f64;
        assert!(ms.abs() > GOOD_MS);
    }

    #[test]
    fn faded_hits_do_not_respawn_but_new_hits_and_sessions_do() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::scene::ScenePlugin));
        let world = app.world_mut();
        world.insert_resource(state_with_offsets(&[0.03]));
        world.spawn(TimingBarContainer);
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_hit_markers);
        let mut markers = world.query_filtered::<Entity, With<TimingMarker>>();

        schedule.run(world);
        let first = markers.single(world).unwrap();
        world.despawn(first);
        world.resource_mut::<CalState>().clock += 1.0;
        schedule.run(world);
        assert_eq!(markers.iter(world).count(), 0);

        world.resource_mut::<CalState>().offsets.push(0.08);
        schedule.run(world);
        let second = markers.single(world).unwrap();
        world.despawn(second);

        {
            let mut cal = world.resource_mut::<CalState>();
            cal.reset();
            cal.offsets.push(-0.02);
        }
        schedule.run(world);
        assert_eq!(markers.iter(world).count(), 1);
    }
}
