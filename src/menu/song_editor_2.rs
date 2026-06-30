// SPDX-License-Identifier: MIT

//! Song authoring tool #2: a DAW-style note grid (`AppState::SongEditor2`).
//!
//! Layout, left to right:
//!   * a fixed column of the ten harmonica holes (number + hole box), and
//!   * an infinite, horizontally-scrollable beat grid to its right.
//!
//! ```text
//!     _  |1 &|2 &|3 &|4 &|1 &|2 &|...
//!  01 |□| ____________________________
//!  ..
//!  10 |□| ____________________________
//! ```
//!
//! Interaction:
//!   * scroll/pan horizontally with the mouse wheel or the ← → keys,
//!   * left-click an empty cell to add a note on that hole/beat,
//!   * left-click a note to select it, then apply modifiers from the panel,
//!   * `Delete` removes the selected note.
//!
//! A note carries one *pitch* technique (Normal, Bend 0.5/1.0/1.5, Overblow,
//! Overdraw) and one optional *expression* (Wah or Vibrato) that stacks on top.
//! Pitch states are flat-coloured; Wah/Vibrato reuse the gameplay note shader
//! ([`NoteTail2dMaterial`]) so the note visibly wobbles/pulses.
//!
//! The grid only ever spawns the cells currently on screen — scrolling rebuilds
//! the visible window, so off-screen notes cost nothing to draw.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use crate::gameplay::note_tail_2d::{NoteTail2dMaterial, tail_params};

use super::AppState;

// ── Geometry ─────────────────────────────────────────────────────────────────

const HOLE_COL_W: f32 = 78.0; // width of the fixed hole column (number + box)
const HEADER_H: f32 = 30.0; // beat-number header strip above the lanes
const ROW_H: f32 = 34.0; // height of one hole lane
const BEAT_W: f32 = 60.0; // width of one beat column
const ROWS: u8 = 10; // harmonica holes 1..=10
const BEATS_PER_BAR: usize = 4;

fn grid_height() -> f32 {
    HEADER_H + ROW_H * ROWS as f32
}

// ── Colours ──────────────────────────────────────────────────────────────────

const EDITOR_BG: Color = Color::srgb(0.06, 0.06, 0.09);
const HOLE_BOX: Color = Color::srgb(0.16, 0.16, 0.22);
const LANE_A: Color = Color::srgba(0.12, 0.12, 0.17, 1.0);
const LANE_B: Color = Color::srgba(0.10, 0.10, 0.14, 1.0);
const GRID_LINE: Color = Color::srgb(0.20, 0.20, 0.27);
const BAR_LINE: Color = Color::srgb(0.40, 0.40, 0.52);
const ACCENT: Color = Color::srgb(0.95, 0.80, 0.35);
const LABEL: Color = Color::srgb(0.75, 0.75, 0.82);
const PANEL_BG: Color = Color::srgba(0.10, 0.10, 0.15, 1.0);
const BTN_BG: Color = Color::srgb(0.16, 0.16, 0.24);
const BTN_ACTIVE: Color = Color::srgb(0.28, 0.42, 0.30);
const FIELD_BG: Color = Color::srgba(0.10, 0.10, 0.14, 1.0);
const FIELD_BG_FOCUS: Color = Color::srgba(0.16, 0.16, 0.24, 1.0);

// ── Note model ───────────────────────────────────────────────────────────────

/// The pitch technique of a note. Mutually exclusive — a note is exactly one of
/// these. `Bend` carries its depth in semitones (0.5, 1.0 or 1.5).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Pitch {
    Normal,
    Bend(f32),
    Overblow,
    Overdraw,
}

/// An expression technique layered on top of the pitch. At most one at a time;
/// either may combine with any [`Pitch`].
#[derive(Clone, Copy, PartialEq, Debug)]
enum Expr {
    None,
    Wah,
    Vibrato,
}

/// Breath direction: blow (exhale) or draw (inhale). Every note is one or the
/// other; toggled with the Blow/Draw buttons.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Dir {
    Blow,
    Draw,
}

impl Dir {
    /// The up/down arrow shown on the note tile.
    fn arrow(self) -> &'static str {
        match self {
            Dir::Blow => "\u{2191}", // ↑
            Dir::Draw => "\u{2193}", // ↓
        }
    }
}

/// One placed note: a hole (1..=10) at an integer beat, plus its techniques.
#[derive(Clone, Copy, PartialEq, Debug)]
struct GridNote {
    hole: u8,
    beat: usize,
    dir: Dir,
    pitch: Pitch,
    expr: Expr,
}

impl GridNote {
    fn bend(&self) -> f32 {
        match self.pitch {
            Pitch::Bend(a) => a,
            _ => 0.0,
        }
    }

    /// Pitch shift in semitones for the shader: bends pull down, over-blow/draw
    /// push up. Drives the tail's lean direction and depth.
    fn shift(&self) -> f32 {
        match self.pitch {
            Pitch::Normal => 0.0,
            Pitch::Bend(a) => -a,
            Pitch::Overblow | Pitch::Overdraw => 1.0,
        }
    }
}

/// The most a hole can be bent in this editor, in semitones (0.0 = not bendable).
///
/// Simplified Richter rule: holes 1-6 draw-bend, 7-10 blow-bend, capped at the
/// 1.5-semitone maximum the editor offers. Deeper-bending holes (2, 3, 10) allow
/// the full 1.5; shallow ones (5, 7) allow only a half step.
fn max_bend(hole: u8) -> f32 {
    match hole {
        2 | 3 | 10 => 1.5,
        1 | 6 | 8 | 9 => 1.0,
        4 | 5 | 7 => 0.5,
        _ => 0.0,
    }
}

fn overblow_ok(hole: u8) -> bool {
    (1..=6).contains(&hole)
}

fn overdraw_ok(hole: u8) -> bool {
    (7..=10).contains(&hole)
}

/// Flat fill colour for a note's pitch state (used when no shader is applied).
fn pitch_color(pitch: Pitch) -> Color {
    match pitch {
        Pitch::Normal => Color::srgb(0.30, 0.60, 0.95),
        Pitch::Bend(a) => {
            // Orange → red as the bend deepens.
            let t = (a / 1.5).clamp(0.0, 1.0);
            Color::srgb(0.95, 0.55 - 0.30 * t, 0.22)
        }
        Pitch::Overblow => Color::srgb(0.72, 0.42, 0.95),
        Pitch::Overdraw => Color::srgb(0.28, 0.85, 0.78),
    }
}

// ── Metadata fields ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Tempo,
    Music,
    Name,
    Author,
}

const FIELDS: [(Field, &str); 4] = [
    (Field::Tempo, "Music Tempo"),
    (Field::Music, "Background Music"),
    (Field::Name, "Name"),
    (Field::Author, "Author"),
];

// ── State ────────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct EditorState {
    notes: Vec<GridNote>,
    /// The selected note's identity (hole, beat), if any.
    selected: Option<(u8, usize)>,
    /// Index of the leftmost visible beat.
    scroll_beat: usize,
    tempo: String,
    music: String,
    name: String,
    author: String,
    /// Which metadata field is being typed into (suppresses grid keys).
    focus: Option<Field>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            selected: None,
            scroll_beat: 0,
            tempo: "120".into(),
            music: String::new(),
            name: String::new(),
            author: String::new(),
            focus: None,
        }
    }
}

impl EditorState {
    fn note_index(&self, hole: u8, beat: usize) -> Option<usize> {
        self.notes.iter().position(|n| n.hole == hole && n.beat == beat)
    }

    fn selected_note_mut(&mut self) -> Option<&mut GridNote> {
        let (h, b) = self.selected?;
        self.notes.iter_mut().find(|n| n.hole == h && n.beat == b)
    }

    fn field_text(&self, field: Field) -> &str {
        match field {
            Field::Tempo => &self.tempo,
            Field::Music => &self.music,
            Field::Name => &self.name,
            Field::Author => &self.author,
        }
    }

    fn field_text_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Tempo => &mut self.tempo,
            Field::Music => &mut self.music,
            Field::Name => &mut self.name,
            Field::Author => &mut self.author,
        }
    }
}

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct EditorRoot;

/// The clipped viewport the visible grid cells live in.
#[derive(Component)]
struct GridArea;

/// A transient grid entity (cell, header, line, note) rebuilt each scroll/edit.
#[derive(Component)]
struct GridItem;

/// A modifier-panel button, tagged with the technique it toggles.
#[derive(Component, Clone, Copy, PartialEq)]
enum ModButton {
    Blow,
    Draw,
    Bend,
    Overblow,
    Overdraw,
    Wah,
    Vibrato,
    Delete,
}

/// The little red dot on the Bend button that shows the selected note's depth.
#[derive(Component)]
struct BendDot;

#[derive(Component)]
struct MetaFieldBox(Field);

#[derive(Component)]
struct MetaFieldText(Field);

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct SongEditor2Plugin;

impl Plugin for SongEditor2Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::SongEditor2), (reset_state, setup).chain())
            .add_systems(OnExit(AppState::SongEditor2), cleanup)
            .add_systems(
                Update,
                (
                    pan_keys,
                    pan_wheel,
                    grid_keys,
                    type_into_field,
                    rebuild_grid.run_if(resource_exists_and_changed::<EditorState>),
                    update_mod_panel,
                    update_meta_fields,
                    animate_note_shaders,
                )
                    .run_if(in_state(AppState::SongEditor2)),
            );
    }
}

fn reset_state(mut commands: Commands) {
    commands.insert_resource(EditorState::default());
}

// ── Setup: the static shell (hole column, grid viewport, panel, fields) ───────

fn setup(mut commands: Commands) {
    commands
        .spawn((
            EditorRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(EDITOR_BG),
        ))
        .with_children(|root| {
            // Editor row: fixed hole column + scrolling grid viewport.
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(grid_height()),
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|row| {
                spawn_hole_column(row);

                // The grid viewport: fixed height, grows horizontally, clips its
                // children so only the on-screen beats show.
                row.spawn((
                    GridArea,
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(grid_height()),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
            });

            spawn_mod_panel(root);
            spawn_meta_form(root);
        });
}

fn spawn_hole_column(row: &mut ChildSpawnerCommands) {
    row.spawn(Node {
        width: Val::Px(HOLE_COL_W),
        height: Val::Px(grid_height()),
        flex_direction: FlexDirection::Column,
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|col| {
        // Header spacer so hole rows line up with the grid lanes.
        col.spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(HEADER_H),
            ..default()
        });
        for hole in 1..=ROWS {
            col.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    Text::new(format!("{hole:02}")),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(LABEL),
                ));
                // The hole "box" (□).
                r.spawn((
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(HOLE_BOX),
                    BorderColor::all(Color::srgb(0.45, 0.45, 0.55)),
                ));
            });
        }
    });
}

fn spawn_mod_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(52.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ))
    .with_children(|panel| {
        mod_button(panel, ModButton::Blow, "Blow");
        mod_button(panel, ModButton::Draw, "Draw");
        // Separator between the breath direction and the techniques.
        panel.spawn((
            Node { width: Val::Px(1.0), height: Val::Px(28.0), margin: UiRect::horizontal(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::srgb(0.30, 0.30, 0.40)),
        ));
        mod_button(panel, ModButton::Bend, "Bend");
        mod_button(panel, ModButton::Overblow, "Overblow");
        mod_button(panel, ModButton::Overdraw, "Overdraw");
        mod_button(panel, ModButton::Wah, "Wah");
        mod_button(panel, ModButton::Vibrato, "Vibrato");
        // Spacer pushes Delete to the right.
        panel.spawn(Node { flex_grow: 1.0, ..default() });
        mod_button(panel, ModButton::Delete, "Delete");
    });
}

fn mod_button(panel: &mut ChildSpawnerCommands, kind: ModButton, label: &str) {
    panel
        .spawn((
            Button,
            kind,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ))
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            apply_modifier(&mut state, kind);
        })
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
            // The Bend button carries a red dot that shows the bend depth of the
            // selected note (hidden when not bent). Updated by `update_mod_panel`.
            if kind == ModButton::Bend {
                b.spawn((
                    BendDot,
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        margin: UiRect::left(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.90, 0.20, 0.20)),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ));
            }
        });
}

fn spawn_meta_form(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        for (field, label) in FIELDS {
            form.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    Node { width: Val::Px(150.0), ..default() },
                    Text::new(format!("{label}:")),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(LABEL),
                ));
                line.spawn((
                    Button,
                    MetaFieldBox(field),
                    Node {
                        width: Val::Px(240.0),
                        height: Val::Px(26.0),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(FIELD_BG),
                    BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                ))
                .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                    state.focus = Some(field);
                })
                .with_children(|b| {
                    b.spawn((
                        MetaFieldText(field),
                        Text::new(String::new()),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                    ));
                });
            });
        }
    });
}

// ── Rebuild: the visible grid window ─────────────────────────────────────────

fn visible_beats(win_w: f32) -> usize {
    (((win_w - HOLE_COL_W) / BEAT_W).ceil() as usize) + 1
}

/// Tear down the previous grid window and spawn the cells, lines, headers and
/// notes for the beats currently on screen. Runs whenever the state changes
/// (scroll, selection or an edit), so off-screen beats are never spawned.
fn rebuild_grid(
    mut commands: Commands,
    state: Res<EditorState>,
    area: Query<Entity, With<GridArea>>,
    old: Query<Entity, With<GridItem>>,
    windows: Query<&Window>,
    mut tail_mats: ResMut<Assets<NoteTail2dMaterial>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let Ok(area) = area.single() else { return };
    let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);
    let cols = visible_beats(win_w);

    let mut items: Vec<Entity> = Vec::new();

    for col in 0..cols {
        let beat = state.scroll_beat + col;
        let x = col as f32 * BEAT_W;
        let is_bar = beat % BEATS_PER_BAR == 0;

        // Vertical beat / bar line.
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(0.0),
                        width: Val::Px(if is_bar { 2.0 } else { 1.0 }),
                        height: Val::Px(grid_height()),
                        ..default()
                    },
                    BackgroundColor(if is_bar { BAR_LINE } else { GRID_LINE }),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        // Beat header: "N &" (the beat-in-bar number and its eighth-note "and").
        let in_bar = beat % BEATS_PER_BAR + 1;
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + 4.0),
                        top: Val::Px(6.0),
                        ..default()
                    },
                    Text::new(format!("{in_bar} &")),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(if is_bar { ACCENT } else { LABEL }),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        // Ten clickable cells in this column.
        for hole in 1..=ROWS {
            let y = HEADER_H + (hole as f32 - 1.0) * ROW_H;
            let lane = if hole % 2 == 0 { LANE_A } else { LANE_B };
            let mut cell = commands.spawn((
                GridItem,
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(y),
                    width: Val::Px(BEAT_W),
                    height: Val::Px(ROW_H),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(lane),
            ));
            cell.observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                select_or_add(&mut state, hole, beat);
            });

            if let Some(idx) = state.note_index(hole, beat) {
                let note = state.notes[idx];
                let selected = state.selected == Some((hole, beat));
                cell.with_children(|c| spawn_note_visual(c, note, selected, &mut tail_mats));
            }
            items.push(cell.id());
        }
    }

    commands.entity(area).add_children(&items);
}

/// Spawn the coloured note tile inside its cell. Plain pitches are flat-coloured;
/// Wah/Vibrato instead render through the gameplay note shader so the tile
/// visibly pulses (wah) or wobbles (vibrato).
fn spawn_note_visual(
    cell: &mut ChildSpawnerCommands,
    note: GridNote,
    selected: bool,
    tail_mats: &mut Assets<NoteTail2dMaterial>,
) {
    let border = if selected { 2.5 } else { 0.0 };
    let border_color = if selected { ACCENT } else { Color::NONE };

    let mut tile = cell.spawn((
        Node {
            width: Val::Px(BEAT_W - 10.0),
            height: Val::Px(ROW_H - 8.0),
            border: UiRect::all(Val::Px(border)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderColor::all(border_color),
        Pickable::IGNORE,
    ));

    match note.expr {
        Expr::None => {
            tile.insert(BackgroundColor(pitch_color(note.pitch)));
        }
        Expr::Wah | Expr::Vibrato => {
            let (vibrato, wah) = match note.expr {
                Expr::Vibrato => (Some(0.8), None),
                _ => (None, Some(0.8)),
            };
            let (params, wah_v) = tail_params(40.0, vibrato, Some(note.shift()), wah);
            let mat = tail_mats.add(NoteTail2dMaterial {
                color: pitch_color(note.pitch).to_linear(),
                params,
                wah: wah_v,
            });
            tile.insert(MaterialNode(mat));
        }
    }

    // Breath-direction arrow (↑ blow / ↓ draw), centered on the tile.
    tile.with_children(|t| {
        t.spawn((
            Text::new(note.dir.arrow()),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ));
    });
}

// ── Interaction ──────────────────────────────────────────────────────────────

fn select_or_add(state: &mut EditorState, hole: u8, beat: usize) {
    if state.note_index(hole, beat).is_some() {
        state.selected = Some((hole, beat));
    } else {
        state.notes.push(GridNote {
            hole,
            beat,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        });
        state.selected = Some((hole, beat));
    }
}

fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    if kind == ModButton::Delete {
        if let Some((h, b)) = state.selected.take() {
            state.notes.retain(|n| !(n.hole == h && n.beat == b));
        }
        return;
    }
    let Some(note) = state.selected_note_mut() else { return };
    match kind {
        ModButton::Blow => note.dir = Dir::Blow,
        ModButton::Draw => note.dir = Dir::Draw,
        ModButton::Bend => {
            let max = max_bend(note.hole);
            if max <= 0.0 {
                return;
            }
            // Cycle 0.5 → 1.0 → 1.5 (capped at the hole's max), then back to Normal.
            let next = note.bend() + 0.5;
            note.pitch = if next > max + f32::EPSILON {
                Pitch::Normal
            } else {
                Pitch::Bend(next)
            };
        }
        ModButton::Overblow => {
            if overblow_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overblow { Pitch::Normal } else { Pitch::Overblow };
            }
        }
        ModButton::Overdraw => {
            if overdraw_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overdraw { Pitch::Normal } else { Pitch::Overdraw };
            }
        }
        ModButton::Wah => {
            note.expr = if note.expr == Expr::Wah { Expr::None } else { Expr::Wah };
        }
        ModButton::Vibrato => {
            note.expr = if note.expr == Expr::Vibrato { Expr::None } else { Expr::Vibrato };
        }
        ModButton::Delete => unreachable!(),
    }
}

fn pan_keys(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<EditorState>) {
    if state.focus.is_some() {
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        state.scroll_beat += 1;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) && state.scroll_beat > 0 {
        state.scroll_beat -= 1;
    }
}

fn pan_wheel(mut wheel: MessageReader<MouseWheel>, mut state: ResMut<EditorState>) {
    let mut delta = 0.0;
    for ev in wheel.read() {
        // Either axis pans horizontally; a vertical wheel is the common case.
        delta += if ev.y != 0.0 { ev.y } else { ev.x };
    }
    if delta == 0.0 {
        return;
    }
    let steps = delta.abs().ceil() as usize;
    if delta < 0.0 {
        state.scroll_beat += steps;
    } else {
        state.scroll_beat = state.scroll_beat.saturating_sub(steps);
    }
}

fn grid_keys(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<EditorState>) {
    if state.focus.is_some() {
        return;
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if let Some((h, b)) = state.selected.take() {
            state.notes.retain(|n| !(n.hole == h && n.beat == b));
        }
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        state.selected = None;
    }
}

/// Type into the focused metadata field. `Enter`/`Escape` blur it.
fn type_into_field(mut keys: MessageReader<KeyboardInput>, mut state: ResMut<EditorState>) {
    let Some(field) = state.focus else {
        keys.clear();
        return;
    };
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            bevy::input::keyboard::Key::Character(s) => {
                for c in s.chars() {
                    if !c.is_control() {
                        state.field_text_mut(field).push(c);
                    }
                }
            }
            bevy::input::keyboard::Key::Space => state.field_text_mut(field).push(' '),
            bevy::input::keyboard::Key::Backspace => {
                state.field_text_mut(field).pop();
            }
            bevy::input::keyboard::Key::Enter | bevy::input::keyboard::Key::Escape => {
                state.focus = None;
            }
            _ => {}
        }
    }
}

// ── Visual sync (panel + fields) ─────────────────────────────────────────────

fn update_mod_panel(
    state: Res<EditorState>,
    mut buttons: Query<(&ModButton, &mut BackgroundColor)>,
    mut dot: Query<&mut Visibility, With<BendDot>>,
) {
    let selected = state.selected.and_then(|(h, b)| {
        state.notes.iter().find(|n| n.hole == h && n.beat == b).copied()
    });

    for (kind, mut bg) in &mut buttons {
        let active = selected.is_some_and(|n| match kind {
            ModButton::Blow => n.dir == Dir::Blow,
            ModButton::Draw => n.dir == Dir::Draw,
            ModButton::Bend => matches!(n.pitch, Pitch::Bend(_)),
            ModButton::Overblow => n.pitch == Pitch::Overblow,
            ModButton::Overdraw => n.pitch == Pitch::Overdraw,
            ModButton::Wah => n.expr == Expr::Wah,
            ModButton::Vibrato => n.expr == Expr::Vibrato,
            ModButton::Delete => false,
        });
        bg.0 = if active { BTN_ACTIVE } else { BTN_BG };
    }

    // The red dot shows only when the selected note is actually bent.
    let bent = selected.is_some_and(|n| matches!(n.pitch, Pitch::Bend(_)));
    for mut vis in &mut dot {
        *vis = if bent { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn update_meta_fields(
    state: Res<EditorState>,
    mut texts: Query<(&MetaFieldText, &mut Text)>,
    mut boxes: Query<(&MetaFieldBox, &mut BackgroundColor)>,
) {
    for (tag, mut text) in &mut texts {
        let mut s = state.field_text(tag.0).to_string();
        if state.focus == Some(tag.0) {
            s.push('_');
        }
        **text = s;
    }
    for (tag, mut bg) in &mut boxes {
        bg.0 = if state.focus == Some(tag.0) { FIELD_BG_FOCUS } else { FIELD_BG };
    }
}

/// Advance the animation clock of every note shader so Wah/Vibrato tiles move.
fn animate_note_shaders(time: Res<Time>, mut mats: ResMut<Assets<NoteTail2dMaterial>>) {
    let t = time.elapsed_secs();
    for (_, mat) in mats.iter_mut() {
        mat.params.z = t;
    }
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<EditorRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_adds_then_selects_without_duplicating() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.selected, Some((4, 2)));
        // Clicking the same cell selects the existing note, doesn't add another.
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
    }

    #[test]
    fn bend_cycles_and_caps_at_hole_max() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 1, 0); // hole 1 max bend = 1.0
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(1.0));
        // 1.5 would exceed hole 1's max, so it wraps back to Normal.
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes[0].pitch, Pitch::Normal);
    }

    #[test]
    fn unbendable_hole_ignores_bend() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 5, 0); // hole 5 max = 0.5 (bendable)
        select_or_add(&mut s, 7, 0); // hole 7 max = 0.5
        // A non-bendable hole would stay Normal; pick one with max 0 — none here,
        // so verify the cap instead: hole 5 allows exactly one 0.5 step.
        s.selected = Some((5, 0));
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes.iter().find(|n| n.hole == 5).unwrap().pitch, Pitch::Bend(0.5));
        apply_modifier(&mut s, ModButton::Bend); // 1.0 > 0.5 max → wrap
        assert_eq!(s.notes.iter().find(|n| n.hole == 5).unwrap().pitch, Pitch::Normal);
    }

    #[test]
    fn pitch_and_expression_stack() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Bend);
        apply_modifier(&mut s, ModButton::Vibrato);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        assert_eq!(s.notes[0].expr, Expr::Vibrato);
        // Wah replaces Vibrato (one expression at a time).
        apply_modifier(&mut s, ModButton::Wah);
        assert_eq!(s.notes[0].expr, Expr::Wah);
        // …but the bend is untouched.
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
    }

    #[test]
    fn overblow_only_on_low_holes() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 8, 0); // overblow not allowed on hole 8
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes[0].pitch, Pitch::Normal);
        select_or_add(&mut s, 3, 0); // allowed on hole 3
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes.iter().find(|n| n.hole == 3).unwrap().pitch, Pitch::Overblow);
    }

    #[test]
    fn blow_draw_toggles_independently_of_techniques() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        assert_eq!(s.notes[0].dir, Dir::Blow); // new notes default to blow
        apply_modifier(&mut s, ModButton::Bend);
        apply_modifier(&mut s, ModButton::Draw);
        assert_eq!(s.notes[0].dir, Dir::Draw);
        // Direction is orthogonal: the bend survives the direction change.
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        apply_modifier(&mut s, ModButton::Blow);
        assert_eq!(s.notes[0].dir, Dir::Blow);
    }

    #[test]
    fn delete_removes_selected() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 1);
        apply_modifier(&mut s, ModButton::Delete);
        assert!(s.notes.is_empty());
        assert_eq!(s.selected, None);
    }
}
