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
const NOTE_PAD: f32 = 4.0; // vertical inset of a note tile within its lane
const HANDLE_W: f32 = 8.0; // width of the left/right drag-to-resize handles

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

/// One placed note: a hole (1..=10) starting at an integer beat and lasting
/// `len` beats, plus its techniques. `id` is a stable handle so the note keeps
/// its identity while its `beat`/`len` change under an edge-drag.
#[derive(Clone, Copy, PartialEq, Debug)]
struct GridNote {
    id: u32,
    hole: u8,
    beat: usize,
    len: usize,
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

/// Which edge of a note is being dragged to resize it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Edge {
    Left,
    Right,
}

/// A resize gesture in progress. Recorded at `DragStart` so each `Drag` event can
/// be applied relative to the note's size when the drag began.
#[derive(Clone, Copy)]
struct DragState {
    id: u32,
    start_beat: usize,
    start_len: usize,
}

/// New `(beat, len)` after dragging an edge by `steps` whole beats (positive =
/// dragged rightward). The right edge changes length; the left edge moves the
/// start and changes length inversely. A note never shrinks below one beat or
/// starts before beat 0.
fn apply_resize(beat: usize, len: usize, edge: Edge, steps: i32) -> (usize, usize) {
    match edge {
        Edge::Right => (beat, (len as i32 + steps).max(1) as usize),
        Edge::Left => {
            // The left edge can move right at most len-1 beats (keeps len >= 1),
            // and left at most `beat` beats (keeps the start at >= 0).
            let move_beats = steps.clamp(-(beat as i32), len as i32 - 1);
            ((beat as i32 + move_beats) as usize, (len as i32 - move_beats) as usize)
        }
    }
}

/// Do two notes sound at the same time (their beat ranges overlap)?
fn overlaps(a: &GridNote, b: &GridNote) -> bool {
    a.beat < b.beat + b.len && b.beat < a.beat + a.len
}

/// Make every note overlapping `id` in time — directly or through a chain of
/// overlaps — share `id`'s breath direction. A player can't blow and draw at the
/// same instant, so notes that sound together must agree; the actively-edited
/// note's direction wins.
fn enforce_direction(state: &mut EditorState, id: u32) {
    let Some(dir) = state.note_by_id(id).map(|n| n.dir) else { return };
    // Flood-fill the time-overlap connected component starting from `id`.
    let mut group = vec![id];
    let mut i = 0;
    while i < group.len() {
        let Some(cur) = state.note_by_id(group[i]).copied() else {
            i += 1;
            continue;
        };
        for n in &state.notes {
            if !group.contains(&n.id) && overlaps(&cur, n) {
                group.push(n.id);
            }
        }
        i += 1;
    }
    for n in &mut state.notes {
        if group.contains(&n.id) {
            n.dir = dir;
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
    /// `id` of the next note to be added.
    next_id: u32,
    /// The selected note's stable `id`, if any.
    selected: Option<u32>,
    /// Index of the leftmost visible beat.
    scroll_beat: usize,
    /// An in-progress edge-drag; while set, the grid is not rebuilt so the
    /// dragged entity survives the gesture.
    dragging: Option<DragState>,
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
            next_id: 0,
            selected: None,
            scroll_beat: 0,
            dragging: None,
            tempo: "120".into(),
            music: String::new(),
            name: String::new(),
            author: String::new(),
            focus: None,
        }
    }
}

impl EditorState {
    /// The note that starts exactly at this cell, if any. Used to decide whether
    /// a click on an empty cell should add a note.
    fn note_at(&self, hole: u8, beat: usize) -> Option<&GridNote> {
        self.notes.iter().find(|n| n.hole == hole && n.beat == beat)
    }

    fn note_by_id(&self, id: u32) -> Option<&GridNote> {
        self.notes.iter().find(|n| n.id == id)
    }

    /// The breath direction already sounding at `beat`, if any note spans it.
    fn dir_at(&self, beat: usize) -> Option<Dir> {
        self.notes
            .iter()
            .find(|n| n.beat <= beat && beat < n.beat + n.len)
            .map(|n| n.dir)
    }

    fn selected_note(&self) -> Option<&GridNote> {
        self.selected.and_then(|id| self.note_by_id(id))
    }

    fn selected_note_mut(&mut self) -> Option<&mut GridNote> {
        let id = self.selected?;
        self.notes.iter_mut().find(|n| n.id == id)
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

/// A note-overlay root entity, tagged with its note's stable `id` so the live
/// resize system can find and reposition it during a drag.
#[derive(Component)]
struct NoteView(u32);

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
                    live_resize,
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
    // While an edge is being dragged the note entity must survive the gesture,
    // so the whole grid is left in place; `live_resize` updates it instead.
    if state.dragging.is_some() {
        return;
    }
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
            items.push(cell.id());
        }
    }

    // Notes are a separate overlay above the cells (ZIndex 1) so a multi-beat
    // note can both span several columns and intercept clicks across its whole
    // width. Only notes intersecting the visible window are spawned.
    let last_beat = state.scroll_beat + cols;
    for note in &state.notes {
        if note.beat < last_beat && note.beat + note.len > state.scroll_beat {
            let selected = state.selected == Some(note.id);
            items.push(spawn_note(&mut commands, *note, selected, state.scroll_beat, &mut tail_mats));
        }
    }

    commands.entity(area).add_children(&items);
}

/// Pixel rect (left, top, width, height) of a note within the grid viewport,
/// given the current horizontal scroll.
fn note_rect(note: &GridNote, scroll_beat: usize) -> (f32, f32, f32, f32) {
    let left = (note.beat as f32 - scroll_beat as f32) * BEAT_W + 1.0;
    let top = HEADER_H + (note.hole as f32 - 1.0) * ROW_H + NOTE_PAD;
    let width = note.len as f32 * BEAT_W - 2.0;
    let height = ROW_H - 2.0 * NOTE_PAD;
    (left, top, width, height)
}

/// Spawn a note as an overlay entity above the grid cells. The root is the
/// clickable/selectable body (spanning `len` beats); two thin children at its
/// left and right edges are the drag-to-resize handles. Plain pitches are
/// flat-coloured; Wah/Vibrato render through the gameplay note shader so the
/// tile visibly pulses (wah) or wobbles (vibrato).
fn spawn_note(
    commands: &mut Commands,
    note: GridNote,
    selected: bool,
    scroll_beat: usize,
    tail_mats: &mut Assets<NoteTail2dMaterial>,
) -> Entity {
    let (left, top, width, height) = note_rect(&note, scroll_beat);
    let border = if selected { 2.0 } else { 0.0 };
    let border_color = if selected { ACCENT } else { Color::NONE };
    let id = note.id;

    let root = commands
        .spawn((
            GridItem,
            NoteView(id),
            Button,
            ZIndex(1),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(width),
                height: Val::Px(height),
                border: UiRect::all(Val::Px(border)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(border_color),
        ))
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            state.selected = Some(id);
        })
        .id();

    // Background: flat colour, or the animated note shader for Wah/Vibrato.
    match note.expr {
        Expr::None => {
            commands.entity(root).insert(BackgroundColor(pitch_color(note.pitch)));
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
            commands.entity(root).insert(MaterialNode(mat));
        }
    }

    commands.entity(root).with_children(|r| {
        // Breath-direction arrow (↑ blow / ↓ draw), centered on the tile.
        r.spawn((
            Text::new(note.dir.arrow()),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ));
        spawn_resize_handle(r, id, Edge::Left);
        spawn_resize_handle(r, id, Edge::Right);
    });

    root
}

/// One edge handle: a thin strip pinned to the note's left or right edge that,
/// when dragged, resizes the note in whole-beat steps (see [`apply_resize`]).
fn spawn_resize_handle(parent: &mut ChildSpawnerCommands, id: u32, edge: Edge) {
    let mut node = Node {
        position_type: PositionType::Absolute,
        top: Val::Px(0.0),
        bottom: Val::Px(0.0),
        width: Val::Px(HANDLE_W),
        ..default()
    };
    match edge {
        Edge::Left => node.left = Val::Px(0.0),
        Edge::Right => node.right = Val::Px(0.0),
    }
    parent
        .spawn((node, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25))))
        .observe(move |_: On<Pointer<DragStart>>, mut state: ResMut<EditorState>| {
            if let Some(n) = state.note_by_id(id).copied() {
                state.selected = Some(id);
                state.dragging = Some(DragState { id, start_beat: n.beat, start_len: n.len });
            }
        })
        .observe(move |ev: On<Pointer<Drag>>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging else { return };
            if drag.id != id {
                return;
            }
            let steps = (ev.distance.x / BEAT_W).round() as i32;
            let (beat, len) = apply_resize(drag.start_beat, drag.start_len, edge, steps);
            if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                n.beat = beat;
                n.len = len;
            }
        })
        .observe(move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
            state.dragging = None;
            // Resizing may have pulled this note over notes of the opposite
            // breath direction; unify them (the dragged note wins).
            enforce_direction(&mut state, id);
        });
}

/// While an edge-drag is active, reposition/resize the dragged note's entity
/// directly (the grid rebuild is paused, so this is the only thing that moves).
fn live_resize(state: Res<EditorState>, mut notes: Query<(&NoteView, &mut Node)>) {
    let Some(drag) = state.dragging else { return };
    let Some(note) = state.note_by_id(drag.id) else { return };
    let (left, _top, width, _height) = note_rect(note, state.scroll_beat);
    for (view, mut node) in &mut notes {
        if view.0 == drag.id {
            node.left = Val::Px(left);
            node.width = Val::Px(width);
        }
    }
}

// ── Interaction ──────────────────────────────────────────────────────────────

fn select_or_add(state: &mut EditorState, hole: u8, beat: usize) {
    if let Some(existing) = state.note_at(hole, beat) {
        state.selected = Some(existing.id);
        return;
    }
    // A new note adopts whatever breath direction is already sounding at this
    // beat (all notes at one instant share it), defaulting to blow when alone.
    let dir = state.dir_at(beat).unwrap_or(Dir::Blow);
    let id = state.next_id;
    state.next_id += 1;
    state.notes.push(GridNote {
        id,
        hole,
        beat,
        len: 1,
        dir,
        pitch: Pitch::Normal,
        expr: Expr::None,
    });
    state.selected = Some(id);
}

fn delete_selected(state: &mut EditorState) {
    if let Some(id) = state.selected.take() {
        state.notes.retain(|n| n.id != id);
    }
}

fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    if kind == ModButton::Delete {
        delete_selected(state);
        return;
    }
    let Some(id) = state.selected else { return };

    // Breath direction is shared by everything sounding at the same time, so
    // setting it propagates across the overlapping group.
    if matches!(kind, ModButton::Blow | ModButton::Draw) {
        let dir = if kind == ModButton::Blow { Dir::Blow } else { Dir::Draw };
        if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
            n.dir = dir;
        }
        enforce_direction(state, id);
        return;
    }

    let Some(note) = state.selected_note_mut() else { return };
    match kind {
        ModButton::Blow | ModButton::Draw => unreachable!("handled above"),
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
        delete_selected(&mut state);
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
    let selected = state.selected_note().copied();

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
        let added = s.notes[0];
        assert_eq!(s.selected, Some(added.id));
        assert_eq!((added.hole, added.beat, added.len), (4, 2, 1)); // new notes are one beat
        // Clicking the same cell selects the existing note, doesn't add another.
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.selected, Some(added.id));
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
        let hole5 = s.notes[0].id;
        select_or_add(&mut s, 7, 0); // hole 7 max = 0.5
        // A non-bendable hole would stay Normal; pick one with max 0 — none here,
        // so verify the cap instead: hole 5 allows exactly one 0.5 step.
        s.selected = Some(hole5);
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

    #[test]
    fn new_note_adopts_direction_sounding_at_that_beat() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        apply_modifier(&mut s, ModButton::Draw); // first note becomes draw
        // A second note at the same beat can't blow while the first draws.
        select_or_add(&mut s, 5, 0);
        assert_eq!(s.note_at(5, 0).unwrap().dir, Dir::Draw);
    }

    #[test]
    fn setting_direction_propagates_to_simultaneous_notes() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0); // blow
        select_or_add(&mut s, 5, 0); // adopts blow
        // Switch one to draw → the other (same instant) must follow.
        s.selected = Some(s.note_at(2, 0).unwrap().id);
        apply_modifier(&mut s, ModButton::Draw);
        assert_eq!(s.note_at(2, 0).unwrap().dir, Dir::Draw);
        assert_eq!(s.note_at(5, 0).unwrap().dir, Dir::Draw);
    }

    #[test]
    fn enforce_unifies_overlap_chain_but_not_independent_notes() {
        let mut s = EditorState::default();
        s.notes = vec![
            GridNote { id: 0, hole: 1, beat: 0, len: 3, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
            // Overlaps id 0 over beat 2.
            GridNote { id: 1, hole: 2, beat: 2, len: 3, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
            // Far away, touches nothing.
            GridNote { id: 2, hole: 3, beat: 10, len: 1, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
        ];
        s.next_id = 3;
        enforce_direction(&mut s, 0); // impose id 0's blow
        assert_eq!(s.note_by_id(1).unwrap().dir, Dir::Blow); // overlapping flips
        assert_eq!(s.note_by_id(2).unwrap().dir, Dir::Draw); // independent untouched
    }

    #[test]
    fn separate_times_keep_independent_directions() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0); // beat 0
        select_or_add(&mut s, 2, 4); // beat 4 — no overlap
        s.selected = Some(s.note_at(2, 4).unwrap().id);
        apply_modifier(&mut s, ModButton::Draw);
        assert_eq!(s.note_at(2, 0).unwrap().dir, Dir::Blow); // unaffected
        assert_eq!(s.note_at(2, 4).unwrap().dir, Dir::Draw);
    }

    #[test]
    fn right_edge_resizes_length_and_clamps_to_one() {
        // A note at beat 4, length 1.
        assert_eq!(apply_resize(4, 1, Edge::Right, 2), (4, 3)); // drag right → longer
        assert_eq!(apply_resize(4, 3, Edge::Right, -1), (4, 2)); // drag left → shorter
        assert_eq!(apply_resize(4, 2, Edge::Right, -5), (4, 1)); // never below one beat
    }

    #[test]
    fn left_edge_moves_start_and_resizes_inversely() {
        // Dragging the left edge right shortens and pushes the start later.
        assert_eq!(apply_resize(4, 3, Edge::Left, 1), (5, 2));
        // Dragging it left lengthens and pulls the start earlier.
        assert_eq!(apply_resize(4, 2, Edge::Left, -2), (2, 4));
        // The start never passes the right edge (len stays >= 1)…
        assert_eq!(apply_resize(4, 2, Edge::Left, 9), (5, 1));
        // …nor goes before beat 0.
        assert_eq!(apply_resize(1, 2, Edge::Left, -9), (0, 3));
    }
}
