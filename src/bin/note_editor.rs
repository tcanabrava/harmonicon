// SPDX-License-Identifier: MIT

//! Visual editor for 2-D note layouts (`assets/notes/2d/<theme>.json`).
//!
//! Renders three fake "incoming" notes — short, medium and long duration —
//! side by side, exactly as `gameplay_2d` draws them: a wah-wah shader tail
//! trailing up from the head disc, the tail length scaled by duration with the
//! same formula the game uses. A translucent yellow rectangle behind each note
//! shows its full footprint (head + tail) as a placement hint.
//!
//! Tab selects the TAIL or the HEAD; the selected element tints blue. Arrows
//! move it, R+arrows resize it. Edits apply to the shared theme config and are
//! saved back to JSON with S.
//!
//! Usage:
//!   cargo run --bin note_editor -- circular
//!   cargo run --bin note_editor -- square
//!
//! Controls:
//!   Tab            select TAIL ↔ HEAD          (selected element tints blue)
//!   ← → ↑ ↓       TAIL: tail_x / tail_y        HEAD: move within lane square
//!   R + ← → ↑ ↓   TAIL: tail_width (←→)        HEAD: resize width/height
//!   Shift          larger step
//!   S              save to assets/notes/2d/<theme>.json

use bevy::{input::ButtonInput, prelude::*, ui_render::prelude::UiMaterialPlugin};
use harmonicon::gameplay::note_tail_2d::{NoteTail2dMaterial, tail_params};
use harmonicon::gameplay::note_visual_2d::{NoteChildConfig, spawn_note_children};
use harmonicon::gameplay::{HIT_H_PCT, LOOKAHEAD};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Serialised config (matches gameplay_2d::NoteThemeConfig) ──────────────────

/// Head destination rect within the lane square, in percentages (0..100).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HeadRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Default for HeadRect {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, width: 100.0, height: 100.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoteConfig {
    tail_x: f32,
    tail_y: f32,
    tail_width: f32,
    #[serde(default)]
    head: HeadRect,
}

impl Default for NoteConfig {
    fn default() -> Self {
        Self { tail_x: 0.5, tail_y: 0.5, tail_width: 0.45, head: HeadRect::default() }
    }
}

// ── Resource ──────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct EditorState {
    theme: String,
    json_path: PathBuf,
    config: NoteConfig,
    selected: Selected,
    resize: bool,
    dirty: bool,
}

#[derive(PartialEq, Clone, Copy, Default)]
enum Selected {
    #[default]
    Tail,
    Head,
}

impl EditorState {
    fn from_theme(theme: &str) -> Self {
        let json_path = PathBuf::from(format!("assets/notes/2d/{theme}.json"));
        let config = std::fs::read_to_string(&json_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            theme: theme.to_string(),
            json_path,
            config,
            selected: Selected::Tail,
            resize: false,
            dirty: false,
        }
    }

    fn save(&mut self) {
        match serde_json::to_string_pretty(&self.config) {
            Ok(json) => match std::fs::write(&self.json_path, json) {
                Ok(()) => {
                    info!("Saved {:?}", self.json_path);
                    self.dirty = false;
                }
                Err(e) => error!("Write failed: {e}"),
            },
            Err(e) => error!("Serialise failed: {e}"),
        }
    }
}

// ── Markers ─────────────────────────────────────────────────────────────────

/// Footprint container (yellow hint) of a preview note; holds its tail length so
/// its height can track the tail tip as `tail_y` changes.
#[derive(Component)]
struct NoteContainer {
    tail_len: f32,
}

/// Tail shader node of every preview note.
#[derive(Component)]
struct PreviewTail;

/// Head image node of every preview note.
#[derive(Component)]
struct PreviewHead;

/// Status bar text.
#[derive(Component)]
struct StatusText;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Lane square (head box) side, logical px.
const NOTE_PX: f32 = 90.0;
/// Reference highway height the editor uses to scale tail lengths, mirroring the
/// game where tail length is a fraction of the live highway height.
const EDITOR_HW: f32 = 540.0;
/// Span (in %) a note scrolls from entering to the hit line — same as the game's
/// private `SCROLL_SPAN = 100 - HIT_H_PCT * 0.5`.
const SCROLL_SPAN: f32 = 100.0 - HIT_H_PCT * 0.5;

/// The three demo notes: (label, duration seconds).
const DEMO_NOTES: [(&str, f32); 3] = [
    ("short  ·  0.25s", 0.25),
    ("medium ·  0.8s", 0.8),
    ("long   ·  1.6s", 1.6),
];

const BLUE: Color = Color::srgb(0.30, 0.55, 1.0);
const TAIL_IDLE: Color = Color::srgb(0.62, 0.62, 0.72);
const HEAD_IDLE: Color = Color::WHITE;

/// Tail length in logical px for a note of `duration` seconds — the same scaling
/// `size_note_tails` applies in game, against the editor's reference height.
fn tail_len_px(duration: f32) -> f32 {
    (SCROLL_SPAN / 100.0) * (duration / LOOKAHEAD as f32) * EDITOR_HW
}

/// Full vertical extent of a note, from the head box bottom to the tail tip. The
/// tail attaches at `(1 - tail_y)` up the head box and extends `tail_len` higher,
/// so the footprint is `(1 - tail_y)·NOTE_PX + tail_len` (never less than the
/// head square itself). This is what the yellow hint must span so the tail tip
/// meets its top edge.
fn footprint_h(tail_y: f32, tail_len: f32) -> f32 {
    ((1.0 - tail_y) * NOTE_PX + tail_len).max(NOTE_PX)
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let theme = std::env::args().nth(1).unwrap_or_else(|| "circular".to_string());

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.07, 0.07, 0.09)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Note Editor — {theme}"),
                resolution: (1100_u32, 720_u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(UiMaterialPlugin::<NoteTail2dMaterial>::default())
        .insert_resource(EditorState::from_theme(&theme))
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, sync_preview, apply_selection_tint, tick_tail))
        .run();
}

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut tail_mats: ResMut<Assets<NoteTail2dMaterial>>,
    state: Res<EditorState>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2d);

    let png: Handle<Image> = asset_server.load(format!("notes/2d/{}.png", state.theme));
    let cfg = &state.config;

    // Root: a centred row of note cells, bottom-aligned so every head sits on
    // the same line (the labels share a fixed height, anchoring the heads).
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(80.0),
            padding: UiRect::bottom(Val::Px(80.0)),
            ..default()
        })
        .with_children(|row| {
            for (label, duration) in DEMO_NOTES {
                let tail_len = tail_len_px(duration);
                let cell_h = footprint_h(cfg.tail_y, tail_len);

                // Each cell is a column: [note container] then [label].
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|cell| {
                    // Note container: full footprint (head + tail), relative so
                    // the yellow hint and head box stack inside it.
                    cell.spawn((
                        Node {
                            width: Val::Px(NOTE_PX),
                            height: Val::Px(cell_h),
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        NoteContainer { tail_len },
                    ))
                    .with_children(|container| {
                        // Yellow full-note footprint hint.
                        container.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 0.88, 0.0, 0.12)),
                        ));

                        // Head box (lane square) pinned to the bottom; the tail
                        // overflows upward into the yellow region.
                        container
                            .spawn(Node {
                                position_type: PositionType::Absolute,
                                bottom: Val::Px(0.0),
                                left: Val::Px(0.0),
                                width: Val::Px(NOTE_PX),
                                height: Val::Px(NOTE_PX),
                                overflow: Overflow::visible(),
                                ..default()
                            })
                            .with_children(|note| {
                                let (params, wah) = tail_params(40.0, None, None, None);
                                let mat = tail_mats.add(NoteTail2dMaterial {
                                    color: LinearRgba::from(TAIL_IDLE),
                                    params,
                                    wah,
                                });
                                spawn_note_children(
                                    note,
                                    &NoteChildConfig {
                                        tail_x: cfg.tail_x,
                                        tail_y: cfg.tail_y,
                                        tail_width: cfg.tail_width,
                                        tail_height: Val::Px(tail_len),
                                        tail_material: mat,
                                        head_image: png.clone(),
                                        head_color: HEAD_IDLE,
                                        head_left: cfg.head.x,
                                        head_top: cfg.head.y,
                                        head_width: cfg.head.width,
                                        head_height: cfg.head.height,
                                    },
                                    |cmd| { cmd.insert(PreviewTail); },
                                    |cmd| { cmd.insert(PreviewHead); },
                                );
                            });
                    });

                    // Duration label (fixed height → anchors the head line).
                    cell.spawn((
                        Text::new(label),
                        TextFont { font_size: FontSize::Px(13.0), ..default() },
                        TextColor(Color::srgb(0.55, 0.55, 0.65)),
                    ));
                });
            }
        });

    // Status bar.
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        StatusText,
    ));
}

// ── Input ─────────────────────────────────────────────────────────────────────

/// Seconds a key must be held before it begins auto-repeating.
const REPEAT_DELAY: f32 = 0.35;
/// Interval between auto-repeats once long-press kicks in.
const REPEAT_RATE: f32 = 0.04;

/// Per-arrow countdown to the next fire, persisted across frames in a `Local`.
#[derive(Default)]
struct ArrowRepeat {
    left: f32,
    right: f32,
    up: f32,
    down: f32,
}

/// Returns whether `key` should "fire" this frame: immediately on press, then —
/// after [`REPEAT_DELAY`] of being held — every [`REPEAT_RATE`] seconds. This is
/// what turns a long press into continuous nudging.
fn arrow_fires(key: KeyCode, keys: &ButtonInput<KeyCode>, dt: f32, cooldown: &mut f32) -> bool {
    if keys.just_pressed(key) {
        *cooldown = REPEAT_DELAY;
        return true;
    }
    if keys.pressed(key) {
        *cooldown -= dt;
        if *cooldown <= 0.0 {
            *cooldown = REPEAT_RATE;
            return true;
        }
    }
    false
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut rep: Local<ArrowRepeat>,
    mut state: ResMut<EditorState>,
    mut status: Query<&mut Text, With<StatusText>>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        state.selected = match state.selected {
            Selected::Tail => Selected::Head,
            Selected::Head => Selected::Tail,
        };
    }

    state.resize = keys.pressed(KeyCode::KeyR);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let dt = time.delta_secs();
    let mut dx = 0.0_f32;
    let mut dy = 0.0_f32;
    if arrow_fires(KeyCode::ArrowLeft,  &keys, dt, &mut rep.left)  { dx = -1.0; }
    if arrow_fires(KeyCode::ArrowRight, &keys, dt, &mut rep.right) { dx =  1.0; }
    if arrow_fires(KeyCode::ArrowUp,    &keys, dt, &mut rep.up)    { dy = -1.0; }
    if arrow_fires(KeyCode::ArrowDown,  &keys, dt, &mut rep.down)  { dy =  1.0; }

    if dx != 0.0 || dy != 0.0 {
        let resize = state.resize;
        match state.selected {
            Selected::Tail => {
                let s = if shift { 0.05 } else { 0.01 };
                let c = &mut state.config;
                if resize {
                    c.tail_width = (c.tail_width + dx * s).clamp(0.01, 1.0);
                } else {
                    c.tail_x = (c.tail_x + dx * s).clamp(0.0, 1.0);
                    c.tail_y = (c.tail_y + dy * s).clamp(0.0, 1.0);
                }
            }
            Selected::Head => {
                let s = if shift { 5.0 } else { 1.0 };
                let h = &mut state.config.head;
                if resize {
                    h.width  = (h.width  + dx * s).clamp(1.0, 200.0);
                    h.height = (h.height + dy * s).clamp(1.0, 200.0);
                } else {
                    h.x = (h.x + dx * s).clamp(-50.0, 100.0);
                    h.y = (h.y + dy * s).clamp(-50.0, 100.0);
                }
            }
        }
        state.dirty = true;
    }

    if keys.just_pressed(KeyCode::KeyS) {
        state.save();
    }

    if let Ok(mut text) = status.single_mut() {
        let c = &state.config;
        let h = &c.head;
        let dirty = if state.dirty { "  [unsaved — S]" } else { "" };
        let line = match state.selected {
            Selected::Tail => {
                let act = if state.resize { "RESIZE WIDTH" } else { "MOVE        " };
                format!(
                    "[TAIL {act}]  tail_x:{:.2}  tail_y:{:.2}  tail_width:{:.2}",
                    c.tail_x, c.tail_y, c.tail_width,
                )
            }
            Selected::Head => {
                let act = if state.resize { "RESIZE" } else { "MOVE  " };
                format!(
                    "[HEAD {act}]  x:{:.0}%  y:{:.0}%  w:{:.0}%  h:{:.0}%",
                    h.x, h.y, h.width, h.height,
                )
            }
        };
        **text = format!("{line}{dirty}   |   Tab=select  R=resize  Shift=big step  S=save");
    }
}

// ── Sync preview nodes to config ──────────────────────────────────────────────

fn sync_preview(
    state: Res<EditorState>,
    mut tails: Query<&mut Node, (With<PreviewTail>, Without<PreviewHead>, Without<NoteContainer>)>,
    mut heads: Query<&mut Node, (With<PreviewHead>, Without<PreviewTail>, Without<NoteContainer>)>,
    mut containers: Query<(&NoteContainer, &mut Node), (Without<PreviewTail>, Without<PreviewHead>)>,
) {
    if !state.is_changed() {
        return;
    }
    let c = &state.config;
    // Tail length (height) is per-note duration, set at spawn — only update the
    // attach point and width here.
    for mut node in &mut tails {
        node.left   = Val::Percent((c.tail_x - c.tail_width * 0.5) * 100.0);
        node.bottom = Val::Percent((1.0 - c.tail_y) * 100.0);
        node.width  = Val::Percent(c.tail_width * 100.0);
    }
    for mut node in &mut heads {
        node.left   = Val::Percent(c.head.x);
        node.top    = Val::Percent(c.head.y);
        node.width  = Val::Percent(c.head.width);
        node.height = Val::Percent(c.head.height);
    }
    // Keep the yellow footprint's top exactly at the tail tip as tail_y changes.
    for (container, mut node) in &mut containers {
        node.height = Val::Px(footprint_h(c.tail_y, container.tail_len));
    }
}

// ── Selection tint (blue on the selected element) ─────────────────────────────

fn apply_selection_tint(
    state: Res<EditorState>,
    mut heads: Query<&mut ImageNode, With<PreviewHead>>,
    mut mats: ResMut<Assets<NoteTail2dMaterial>>,
) {
    if !state.is_changed() {
        return;
    }
    let head_color = if state.selected == Selected::Head { BLUE } else { HEAD_IDLE };
    for mut img in &mut heads {
        img.color = head_color;
    }
    let tail_color = if state.selected == Selected::Tail { BLUE } else { TAIL_IDLE };
    for (_, mat) in mats.iter_mut() {
        mat.color = LinearRgba::from(tail_color);
    }
}

// ── Animate the tail shader flow ──────────────────────────────────────────────

fn tick_tail(time: Res<Time>, mut mats: ResMut<Assets<NoteTail2dMaterial>>) {
    let t = time.elapsed_secs();
    for (_, mat) in mats.iter_mut() {
        mat.params.z = t;
    }
}
