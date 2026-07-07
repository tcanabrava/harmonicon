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

use bevy::{
    asset::AssetPath, input::ButtonInput, prelude::*, ui_render::prelude::UiMaterialPlugin,
};
use harmonicon::gameplay::note_tail_2d::{NoteTail2dMaterial, tail_params};
use harmonicon::gameplay::note_visual_2d::{NoteChildConfig, spawn_note_children};
use harmonicon::gameplay::{HIT_H_PCT, LOOKAHEAD};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Serialised config (matches gameplay_2d::NoteThemeConfig) ──────────────────

/// Head destination rect within the lane square, in percentages (0..100).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct HeadRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Default for HeadRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
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
        Self {
            tail_x: 0.5,
            tail_y: 0.5,
            tail_width: 0.45,
            head: HeadRect::default(),
        }
    }
}

impl NoteConfig {
    /// Apply a directional nudge from arrow input to whichever element is
    /// selected, in the current mode. `dx`/`dy` are unit directions (−1/0/+1);
    /// `resize` chooses size-vs-position; `shift` picks the larger step. All
    /// fields are clamped to their valid ranges. Pure so it can be unit-tested
    /// without a running app.
    fn nudge(&mut self, selected: Selected, resize: bool, shift: bool, dx: f32, dy: f32) {
        match selected {
            Selected::Tail => {
                let s = if shift { 0.05 } else { 0.01 };
                if resize {
                    self.tail_width = (self.tail_width + dx * s).clamp(0.01, 1.0);
                } else {
                    self.tail_x = (self.tail_x + dx * s).clamp(0.0, 1.0);
                    self.tail_y = (self.tail_y + dy * s).clamp(0.0, 1.0);
                }
            }
            Selected::Head => {
                let s = if shift { 5.0 } else { 1.0 };
                let h = &mut self.head;
                if resize {
                    h.width = (h.width + dx * s).clamp(1.0, 200.0);
                    h.height = (h.height + dy * s).clamp(1.0, 200.0);
                } else {
                    h.x = (h.x + dx * s).clamp(-50.0, 100.0);
                    h.y = (h.y + dy * s).clamp(-50.0, 100.0);
                }
            }
        }
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

#[derive(PartialEq, Clone, Copy, Default, Debug)]
enum Selected {
    #[default]
    Tail,
    Head,
}

impl Selected {
    /// The other element — `Tab` flips between the two.
    fn toggled(self) -> Self {
        match self {
            Selected::Tail => Selected::Head,
            Selected::Head => Selected::Tail,
        }
    }
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

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let theme = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "circular".to_string());

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
        .add_systems(
            Update,
            (handle_input, sync_preview, apply_selection_tint, tick_tail),
        )
        .run();
}

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut tail_mats: ResMut<Assets<NoteTail2dMaterial>>,
    state: Res<EditorState>,
) {
    commands.spawn(Camera2d);

    // The head node loads its image by path through its `bsn!` scene, so we just
    // hand `spawn_note_children` the path — no `Handle`/`AssetServer` needed here.
    let head_path: AssetPath<'static> = format!("notes/2d/{}.png", state.theme).into();
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
                // Fixed footprint: head square + full tail length. Stays put as a
                // reference so the tail can be positioned against it.
                let cell_h = NOTE_PX + tail_len;

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
                    cell.spawn(Node {
                        width: Val::Px(NOTE_PX),
                        height: Val::Px(cell_h),
                        position_type: PositionType::Relative,
                        ..default()
                    })
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
                                        head_image: head_path.clone(),
                                        head_color: HEAD_IDLE,
                                        head_left: cfg.head.x,
                                        head_top: cfg.head.y,
                                        head_width: cfg.head.width,
                                        head_height: cfg.head.height,
                                    },
                                    |cmd| {
                                        cmd.insert(PreviewTail);
                                    },
                                    |cmd| {
                                        cmd.insert(PreviewHead);
                                    },
                                );
                            });
                    });

                    // Duration label (fixed height → anchors the head line).
                    cell.spawn((
                        Text::new(label),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.55, 0.55, 0.65)),
                    ));
                });
            }
        });

    // Status bar.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
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
        state.selected = state.selected.toggled();
    }

    state.resize = keys.pressed(KeyCode::KeyR);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let dt = time.delta_secs();
    let mut dx = 0.0_f32;
    let mut dy = 0.0_f32;
    if arrow_fires(KeyCode::ArrowLeft, &keys, dt, &mut rep.left) {
        dx = -1.0;
    }
    if arrow_fires(KeyCode::ArrowRight, &keys, dt, &mut rep.right) {
        dx = 1.0;
    }
    if arrow_fires(KeyCode::ArrowUp, &keys, dt, &mut rep.up) {
        dy = -1.0;
    }
    if arrow_fires(KeyCode::ArrowDown, &keys, dt, &mut rep.down) {
        dy = 1.0;
    }

    if dx != 0.0 || dy != 0.0 {
        let (selected, resize) = (state.selected, state.resize);
        state.config.nudge(selected, resize, shift, dx, dy);
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
                let act = if state.resize {
                    "RESIZE WIDTH"
                } else {
                    "MOVE        "
                };
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
    mut tails: Query<&mut Node, (With<PreviewTail>, Without<PreviewHead>)>,
    mut heads: Query<&mut Node, (With<PreviewHead>, Without<PreviewTail>)>,
) {
    if !state.is_changed() {
        return;
    }
    let c = &state.config;
    // Tail length (height) is per-note duration, set at spawn — only update the
    // attach point and width here.
    for mut node in &mut tails {
        node.left = Val::Percent((c.tail_x - c.tail_width * 0.5) * 100.0);
        node.bottom = Val::Percent((1.0 - c.tail_y) * 100.0);
        node.width = Val::Percent(c.tail_width * 100.0);
    }
    for mut node in &mut heads {
        node.left = Val::Percent(c.head.x);
        node.top = Val::Percent(c.head.y);
        node.width = Val::Percent(c.head.width);
        node.height = Val::Percent(c.head.height);
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
    let head_color = if state.selected == Selected::Head {
        BLUE
    } else {
        HEAD_IDLE
    };
    for mut img in &mut heads {
        img.color = head_color;
    }
    let tail_color = if state.selected == Selected::Tail {
        BLUE
    } else {
        TAIL_IDLE
    };
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── tail length scaling ──────────────────────────────────────────────────

    #[test]
    fn tail_length_is_proportional_to_duration() {
        // Twice the duration → twice the on-screen tail length.
        let short = tail_len_px(0.4);
        let long = tail_len_px(0.8);
        assert!(
            (long - 2.0 * short).abs() < 1e-3,
            "short={short} long={long}"
        );
    }

    #[test]
    fn tail_length_matches_gameplay_formula() {
        // Same scaling the game's `size_note_tails` applies, against EDITOR_HW.
        let d = 1.0;
        let expected = (SCROLL_SPAN / 100.0) * (d / LOOKAHEAD as f32) * EDITOR_HW;
        assert!((tail_len_px(d) - expected).abs() < 1e-3);
    }

    #[test]
    fn zero_duration_has_no_tail() {
        assert_eq!(tail_len_px(0.0), 0.0);
    }

    // ── selection toggle ─────────────────────────────────────────────────────

    #[test]
    fn toggle_flips_and_round_trips() {
        assert_eq!(Selected::Tail.toggled(), Selected::Head);
        assert_eq!(Selected::Head.toggled(), Selected::Tail);
        assert_eq!(Selected::Tail.toggled().toggled(), Selected::Tail);
    }

    // ── nudge: tail ──────────────────────────────────────────────────────────

    #[test]
    fn nudge_moves_tail_attach_point() {
        let mut c = NoteConfig::default(); // tail_x/y = 0.5
        c.nudge(Selected::Tail, false, false, 1.0, 0.0);
        assert!((c.tail_x - 0.51).abs() < 1e-6);
        assert_eq!(c.tail_y, 0.5, "dx must not touch tail_y");
        c.nudge(Selected::Tail, false, false, 0.0, -1.0);
        assert!((c.tail_y - 0.49).abs() < 1e-6);
    }

    #[test]
    fn shift_takes_the_larger_tail_step() {
        let mut c = NoteConfig::default();
        c.nudge(Selected::Tail, false, true, 1.0, 0.0); // shift
        assert!((c.tail_x - 0.55).abs() < 1e-6);
    }

    #[test]
    fn nudge_resizes_only_tail_width_in_resize_mode() {
        let mut c = NoteConfig::default(); // tail_width = 0.45
        c.nudge(Selected::Tail, true, false, 1.0, 0.0);
        assert!((c.tail_width - 0.46).abs() < 1e-6);
        assert_eq!(c.tail_x, 0.5, "resize must leave the attach point alone");
        assert_eq!(c.tail_y, 0.5);
    }

    #[test]
    fn tail_fractions_clamp_to_unit_range() {
        let mut c = NoteConfig::default();
        for _ in 0..500 {
            c.nudge(Selected::Tail, false, true, 1.0, 1.0); // push hard to the max
        }
        assert_eq!(c.tail_x, 1.0);
        assert_eq!(c.tail_y, 1.0);
        for _ in 0..500 {
            c.nudge(Selected::Tail, false, true, -1.0, -1.0);
        }
        assert_eq!(c.tail_x, 0.0);
        assert_eq!(c.tail_y, 0.0);
    }

    #[test]
    fn tail_width_never_collapses_to_zero() {
        let mut c = NoteConfig::default();
        for _ in 0..500 {
            c.nudge(Selected::Tail, true, true, -1.0, 0.0);
        }
        assert_eq!(c.tail_width, 0.01);
    }

    // ── nudge: head ──────────────────────────────────────────────────────────

    #[test]
    fn nudge_moves_head_in_percent_steps() {
        let mut c = NoteConfig::default(); // head x/y = 0
        c.nudge(Selected::Head, false, false, 1.0, 1.0);
        assert_eq!(c.head.x, 1.0);
        assert_eq!(c.head.y, 1.0);
        c.nudge(Selected::Head, false, true, 1.0, 0.0); // shift = 5
        assert_eq!(c.head.x, 6.0);
    }

    #[test]
    fn nudge_resizes_head_without_moving_it() {
        let mut c = NoteConfig::default(); // head w/h = 100, x/y = 0
        c.nudge(Selected::Head, true, false, -1.0, 1.0);
        assert_eq!(c.head.width, 99.0);
        assert_eq!(c.head.height, 101.0);
        assert_eq!(c.head.x, 0.0, "resize must not move the head");
        assert_eq!(c.head.y, 0.0);
    }

    #[test]
    fn head_rect_clamps_to_bounds() {
        let mut c = NoteConfig::default();
        for _ in 0..200 {
            c.nudge(Selected::Head, false, true, 1.0, 1.0); // position max = 100
            c.nudge(Selected::Head, true, true, 1.0, 1.0); // size max = 200
        }
        assert_eq!(c.head.x, 100.0);
        assert_eq!(c.head.y, 100.0);
        assert_eq!(c.head.width, 200.0);
        assert_eq!(c.head.height, 200.0);
        for _ in 0..200 {
            c.nudge(Selected::Head, false, true, -1.0, -1.0); // position min = -50
            c.nudge(Selected::Head, true, true, -1.0, -1.0); // size min = 1
        }
        assert_eq!(c.head.x, -50.0);
        assert_eq!(c.head.y, -50.0);
        assert_eq!(c.head.width, 1.0);
        assert_eq!(c.head.height, 1.0);
    }

    #[test]
    fn selecting_tail_leaves_head_untouched_and_vice_versa() {
        let mut c = NoteConfig::default();
        c.nudge(Selected::Tail, false, false, 1.0, 1.0);
        assert_eq!(
            c.head,
            HeadRect::default(),
            "tail edits must not touch head"
        );

        let mut c = NoteConfig::default();
        c.nudge(Selected::Head, false, false, 1.0, 1.0);
        assert_eq!((c.tail_x, c.tail_y, c.tail_width), (0.5, 0.5, 0.45));
    }

    // ── key repeat (long press) ──────────────────────────────────────────────

    /// Drives `arrow_fires` over simulated frames, counting how many times it
    /// fires. `clear()` mimics the end-of-frame reset the input plugin does.
    fn count_fires(hold_secs: f32, dt: f32) -> u32 {
        let key = KeyCode::ArrowRight;
        let mut input = ButtonInput::<KeyCode>::default();
        let mut cooldown = 0.0;
        let mut fires = 0;

        // First frame: the press.
        input.press(key);
        if arrow_fires(key, &input, dt, &mut cooldown) {
            fires += 1;
        }
        input.clear(); // press is no longer "just"; still held

        let mut elapsed = 0.0;
        while elapsed < hold_secs {
            if arrow_fires(key, &input, dt, &mut cooldown) {
                fires += 1;
            }
            elapsed += dt;
        }
        fires
    }

    #[test]
    fn tap_fires_exactly_once() {
        // Pressed then released within a frame → a single nudge.
        let key = KeyCode::ArrowRight;
        let mut input = ButtonInput::<KeyCode>::default();
        let mut cooldown = 0.0;

        input.press(key);
        assert!(arrow_fires(key, &input, 0.016, &mut cooldown));
        input.clear();
        input.release(key);
        assert!(!arrow_fires(key, &input, 0.016, &mut cooldown));
    }

    #[test]
    fn brief_hold_under_delay_does_not_repeat() {
        // Held for less than REPEAT_DELAY → still just the initial fire.
        let fires = count_fires(REPEAT_DELAY * 0.5, 0.016);
        assert_eq!(fires, 1, "no repeat should kick in before the delay");
    }

    #[test]
    fn long_press_repeats_after_the_delay() {
        let hold = 1.0;
        let dt = 0.016;
        let fires = count_fires(hold, dt);
        // 1 initial + roughly (hold - delay) / rate repeats.
        let expected_repeats = ((hold - REPEAT_DELAY) / REPEAT_RATE).floor() as u32;
        assert!(fires > 1, "a long press must auto-repeat, got {fires}");
        // Allow a small slop for frame quantisation.
        let diff = (fires as i32 - (1 + expected_repeats) as i32).abs();
        assert!(
            diff <= 2,
            "fires={fires}, expected≈{}",
            1 + expected_repeats
        );
    }

    // ── serde / config schema ─────────────────────────────────────────────────

    #[test]
    fn config_round_trips_through_json() {
        let c = NoteConfig {
            tail_x: 0.3,
            tail_y: 0.7,
            tail_width: 0.2,
            head: HeadRect {
                x: 5.0,
                y: -10.0,
                width: 80.0,
                height: 120.0,
            },
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: NoteConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tail_x, c.tail_x);
        assert_eq!(back.tail_y, c.tail_y);
        assert_eq!(back.tail_width, c.tail_width);
        assert_eq!(back.head, c.head);
    }

    #[test]
    fn missing_head_falls_back_to_default_fill() {
        // Older files without a `head` block parse, defaulting to fill.
        let c: NoteConfig =
            serde_json::from_str(r#"{ "tail_x": 0.5, "tail_y": 0.5, "tail_width": 0.45 }"#)
                .unwrap();
        assert_eq!(c.head, HeadRect::default());
        assert_eq!(c.head.width, 100.0);
    }

    #[test]
    fn shipped_note_assets_parse() {
        // Guards against the JSON drifting out of sync with the editor struct.
        for theme in ["circular", "square"] {
            let path = format!("assets/notes/2d/{theme}.json");
            let text =
                std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            let cfg: NoteConfig =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"));
            assert!(
                cfg.tail_width > 0.0,
                "{theme}: tail_width should be positive"
            );
        }
    }
}
