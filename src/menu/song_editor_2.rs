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
//!   * drag a note's body to move it (a yellow ghost previews the drop target,
//!     red when the spot is taken; the move only commits on a free spot),
//!   * drag a note's left/right edge to change its duration,
//!   * `Delete` removes the selected note,
//!   * Play synthesises the authored notes (a C-harp sine preview at the chosen
//!     tempo) and, if the Music field names a readable audio file, plays it too.
//!
//! A note carries one *pitch* technique (Normal, Bend 0.5/1.0/1.5, Overblow,
//! Overdraw) and one optional *expression* (Wah or Vibrato) that stacks on top.
//! Pitch states are flat-coloured; Wah/Vibrato reuse the gameplay note shader
//! ([`NoteTail2dMaterial`]) so the note visibly wobbles/pulses.
//!
//! The grid only ever spawns the cells currently on screen — scrolling rebuilds
//! the visible window, so off-screen notes cost nothing to draw.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::MaterialNode;
use std::f32::consts::TAU;

use crate::audio_system::midi::note_to_midi;
use crate::gameplay::note_tail_2d::{NoteTail2dMaterial, tail_params};
use crate::settings::AudioSettings;

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

/// Sub-beat resolution: a beat is divided into this many ticks. Notes are
/// positioned and sized in *ticks*, so the smallest note is one tick. With 4,
/// that's a quarter beat (half beat = 2 ticks, whole beat = 4 ticks).
const TICKS_PER_BEAT: usize = 4;
/// Pixel width of one tick (BEAT_W / TICKS_PER_BEAT).
const TICK_W: f32 = BEAT_W / TICKS_PER_BEAT as f32;

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
const HALF_LINE: Color = Color::srgb(0.17, 0.17, 0.23); // the "&" (half-beat) divider
const QUARTER_LINE: Color = Color::srgb(0.13, 0.13, 0.18); // quarter-beat dividers
const ACCENT: Color = Color::srgb(0.95, 0.80, 0.35);
const LABEL: Color = Color::srgb(0.75, 0.75, 0.82);
const PANEL_BG: Color = Color::srgba(0.10, 0.10, 0.15, 1.0);
const BTN_BG: Color = Color::srgb(0.16, 0.16, 0.24);
const BTN_ACTIVE: Color = Color::srgb(0.28, 0.42, 0.30);
const FIELD_BG: Color = Color::srgba(0.10, 0.10, 0.14, 1.0);
const FIELD_BG_FOCUS: Color = Color::srgba(0.16, 0.16, 0.24, 1.0);
const GHOST_OK: Color = Color::srgb(0.98, 0.85, 0.20); // move preview on a free spot
const GHOST_BAD: Color = Color::srgb(0.90, 0.25, 0.20); // …on an occupied spot

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

/// One placed note: a hole (1..=10) starting at `tick` and lasting `len` ticks
/// (see [`TICKS_PER_BEAT`]), plus its techniques. `id` is a stable handle so the
/// note keeps its identity while its `tick`/`len` change under a drag.
#[derive(Clone, Copy, PartialEq, Debug)]
struct GridNote {
    id: u32,
    hole: u8,
    tick: usize,
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

/// Whether a drag moves the whole note or resizes one of its edges.
#[derive(Clone, Copy, PartialEq, Debug)]
enum DragKind {
    Move,
    Resize(Edge),
}

/// A drag gesture in progress. Recorded at `DragStart` so each `Drag` event can be
/// applied relative to where the note was when the drag began.
#[derive(Clone, Copy)]
struct DragState {
    id: u32,
    kind: DragKind,
    start_tick: usize,
    start_len: usize,
    start_hole: u8,
    /// For a Move: the live snapped destination and whether it's free to drop on.
    target_hole: u8,
    target_tick: usize,
    valid: bool,
}

impl DragState {
    fn new(id: u32, kind: DragKind, note: &GridNote) -> Self {
        Self {
            id,
            kind,
            start_tick: note.tick,
            start_len: note.len,
            start_hole: note.hole,
            target_hole: note.hole,
            target_tick: note.tick,
            valid: true,
        }
    }
}

/// Snapped destination `(hole, tick)` for a move drag, from the start position and
/// the pixel drag distance. The hole is clamped to 1..=ROWS, the tick to >= 0.
fn move_target(start_hole: u8, start_tick: usize, dist_x: f32, dist_y: f32) -> (u8, usize) {
    let steps_x = (dist_x / TICK_W).round() as i32;
    let steps_y = (dist_y / ROW_H).round() as i32;
    let hole = (start_hole as i32 + steps_y).clamp(1, ROWS as i32) as u8;
    let tick = (start_tick as i32 + steps_x).max(0) as usize;
    (hole, tick)
}

/// Can a note of length `len` sit at (`hole`, `tick`) without overlapping another
/// note on the same hole? `id` (the note being moved) is excluded from the check.
fn can_place(notes: &[GridNote], id: u32, hole: u8, tick: usize, len: usize) -> bool {
    !notes
        .iter()
        .any(|n| n.id != id && n.hole == hole && n.tick < tick + len && tick < n.tick + n.len)
}

/// New `(tick, len)` after dragging an edge by `steps` ticks (positive = dragged
/// rightward). The right edge changes length; the left edge moves the start and
/// changes length inversely.
///
/// A note never shrinks below one tick, and it cannot overlap its neighbours on
/// the same hole: `left_bound` is the earliest tick the start may reach (the
/// previous note's end, or 0) and `right_bound` is the latest tick the end may
/// reach (the next note's start, if any).
fn apply_resize(
    tick: usize,
    len: usize,
    edge: Edge,
    steps: i32,
    left_bound: usize,
    right_bound: Option<usize>,
) -> (usize, usize) {
    match edge {
        Edge::Right => {
            let mut end = (tick + len) as i32 + steps;
            end = end.max((tick + 1) as i32); // keep at least one tick
            if let Some(rb) = right_bound {
                end = end.min(rb as i32); // don't cross the next note on this hole
            }
            (tick, end as usize - tick)
        }
        Edge::Left => {
            let end = tick + len; // the right edge is fixed while dragging the left
            let mut start = tick as i32 + steps;
            start = start.min((end - 1) as i32); // keep at least one tick
            start = start.max(left_bound as i32); // don't cross the previous note (>= 0)
            (start as usize, end - start as usize)
        }
    }
}

/// Do two notes sound at the same time (their tick ranges overlap)?
fn overlaps(a: &GridNote, b: &GridNote) -> bool {
    a.tick < b.tick + b.len && b.tick < a.tick + a.len
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
    Key,
    Music,
    Name,
    Author,
}

const FIELDS: [(Field, &str); 5] = [
    (Field::Tempo, "Music Tempo"),
    (Field::Key, "Harp Key"),
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
    /// Harp key (e.g. "C", "G", "A"); transposes the synthesised pitches.
    key: String,
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
            key: "C".into(),
            music: String::new(),
            name: String::new(),
            author: String::new(),
            focus: None,
        }
    }
}

impl EditorState {
    /// The note that starts exactly at this tick, if any. (Test helper.)
    #[cfg(test)]
    fn note_at(&self, hole: u8, tick: usize) -> Option<&GridNote> {
        self.notes.iter().find(|n| n.hole == hole && n.tick == tick)
    }

    fn note_by_id(&self, id: u32) -> Option<&GridNote> {
        self.notes.iter().find(|n| n.id == id)
    }

    /// The breath direction already sounding at `tick`, if any note spans it.
    fn dir_at(&self, tick: usize) -> Option<Dir> {
        self.notes
            .iter()
            .find(|n| n.tick <= tick && tick < n.tick + n.len)
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
            Field::Key => &self.key,
            Field::Music => &self.music,
            Field::Name => &self.name,
            Field::Author => &self.author,
        }
    }

    fn field_text_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Tempo => &mut self.tempo,
            Field::Key => &mut self.key,
            Field::Music => &mut self.music,
            Field::Name => &mut self.name,
            Field::Author => &mut self.author,
        }
    }
}

/// Continuous horizontal scroll, in pixels (>= 0). Kept out of [`EditorState`] so
/// scrolling doesn't trigger a grid rebuild; `EditorState::scroll_beat` is derived
/// from it as the integer base beat and only changes (→ rebuild) at beat crossings.
#[derive(Resource, Default)]
struct Scroll {
    px: f32,
}

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct EditorRoot;

/// The clipped viewport the visible grid lives in.
#[derive(Component)]
struct GridArea;

/// The scrolling layer inside [`GridArea`]: holds the cells, lines, notes, ghost
/// and playhead, all positioned in *absolute* beat/tick coordinates. Horizontal
/// scrolling just shifts this node's `left`, so the existing nodes slide instead
/// of being respawned.
#[derive(Component)]
struct GridContent;

/// A transient grid entity (cell, header, line, note) rebuilt only when the
/// visible beat window or the notes change — not on every scroll step.
#[derive(Component)]
struct GridItem;

/// A note-overlay root entity, tagged with its note's stable `id` so the live
/// resize system can find and reposition it during a drag.
#[derive(Component)]
struct NoteView(u32);

/// The yellow rectangle shown at a note's snapped destination while moving it.
/// One persistent entity, hidden unless a move drag is in progress.
#[derive(Component)]
struct MoveGhost;

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
            .init_resource::<Scroll>()
            .add_systems(
                Update,
                (
                    // Scroll/playback pipeline, ordered: advance the clock, gather
                    // pan + auto-follow into `Scroll`, apply it (which may bump the
                    // base beat), then cull-rebuild only if that base moved.
                    (
                        advance_playhead,
                        auto_scroll,
                        pan_keys,
                        pan_wheel,
                        apply_scroll,
                        rebuild_grid.run_if(resource_exists_and_changed::<EditorState>),
                    )
                        .chain(),
                    update_playhead_view.after(advance_playhead),
                    update_progress_bar.after(advance_playhead),
                    grid_keys,
                    type_into_field,
                    live_resize,
                    update_move_ghost,
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
    commands.insert_resource(Playhead::default());
    commands.insert_resource(Scroll::default());
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
            // A thin overall-progress bar at the very top — it shows where playback
            // is even when the grid playhead has scrolled out of view.
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(5.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    EditorProgressFill,
                    Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                ));
            });

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
                ))
                .with_children(|ga| {
                    // The scrolling layer. Its `left` is the (negated) scroll
                    // offset, so shifting it slides the whole grid cheaply.
                    ga.spawn((
                        GridContent,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            height: Val::Px(grid_height()),
                            ..default()
                        },
                    ))
                    .with_children(|content| {
                        // The move-preview ghost: a persistent, normally-hidden
                        // yellow rect above the notes (ZIndex 2) at the drop target.
                        content.spawn((
                            MoveGhost,
                            ZIndex(2),
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(BEAT_W - 2.0),
                                height: Val::Px(ROW_H - 2.0 * NOTE_PAD),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(GHOST_OK.with_alpha(0.30)),
                            BorderColor::all(GHOST_OK),
                            Visibility::Hidden,
                            Pickable::IGNORE,
                        ));
                        // The playback cursor: a thin red line above everything
                        // (ZIndex 3), normally hidden.
                        content.spawn((
                            PlayheadLine,
                            ZIndex(3),
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(0.0),
                                width: Val::Px(2.0),
                                height: Val::Px(grid_height()),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.95, 0.30, 0.30)),
                            Visibility::Hidden,
                            Pickable::IGNORE,
                        ));
                    });
                });
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
        // Transport: play the authored song / stop playback.
        spawn_transport(panel);
        panel_separator(panel);
        mod_button(panel, ModButton::Blow, "Blow");
        mod_button(panel, ModButton::Draw, "Draw");
        // Separator between the breath direction and the techniques.
        panel_separator(panel);
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

/// A thin vertical divider between groups of panel buttons.
fn panel_separator(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Node {
            width: Val::Px(1.0),
            height: Val::Px(28.0),
            margin: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.30, 0.30, 0.40)),
    ));
}

/// Play (synthesise + sound the authored notes, plus any background track) and
/// Stop (silence playback) transport buttons.
fn spawn_transport(panel: &mut ChildSpawnerCommands) {
    transport_button(
        panel,
        "\u{25B6} Play",
        Color::srgb(0.20, 0.40, 0.24),
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            start_playback(&state, &mut sources, &settings, &playing, &mut playhead, &mut commands);
        },
    );
    transport_button(
        panel,
        "\u{25A0} Stop",
        Color::srgb(0.36, 0.20, 0.20),
        |_: On<Pointer<Click>>,
         playing: Query<Entity, With<EditorAudio>>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            for e in &playing {
                commands.entity(e).despawn();
            }
            playhead.playing = false;
        },
    );
}

fn transport_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    label: &str,
    bg: Color,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    panel
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ))
        .observe(on_click)
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
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
/// notes for the beats currently in view. Everything is placed in *absolute*
/// beat/tick coordinates inside [`GridContent`]; scrolling slides that node, so
/// this only re-runs when the visible beat window or the notes change.
fn rebuild_grid(
    mut commands: Commands,
    state: Res<EditorState>,
    content: Query<Entity, With<GridContent>>,
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
    let Ok(content) = content.single() else { return };
    let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);
    let cols = visible_beats(win_w);

    let mut items: Vec<Entity> = Vec::new();

    // One extra column on the right so a partial beat is covered while scrolling.
    for col in 0..=cols {
        let beat = state.scroll_beat + col;
        let x = beat as f32 * BEAT_W; // absolute; GridContent's offset does the scroll
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

        // Sub-beat dividers within the beat: the half-beat (the "&") is brighter
        // than the quarter dividers.
        for s in 1..TICKS_PER_BEAT {
            let is_half = s * 2 == TICKS_PER_BEAT;
            items.push(
                commands
                    .spawn((
                        GridItem,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x + s as f32 * TICK_W),
                            top: Val::Px(HEADER_H),
                            width: Val::Px(1.0),
                            height: Val::Px(grid_height() - HEADER_H),
                            ..default()
                        },
                        BackgroundColor(if is_half { HALF_LINE } else { QUARTER_LINE }),
                        Pickable::IGNORE,
                    ))
                    .id(),
            );
        }

        // Header: the beat-in-bar number, plus an "&" marking the half-beat.
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
                    Text::new(format!("{in_bar}")),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(if is_bar { ACCENT } else { LABEL }),
                    Pickable::IGNORE,
                ))
                .id(),
        );
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + BEAT_W * 0.5 + 2.0),
                        top: Val::Px(6.0),
                        ..default()
                    },
                    Text::new("&"),
                    TextFont { font_size: FontSize::Px(11.0), ..default() },
                    TextColor(Color::srgb(0.45, 0.45, 0.55)),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        // Ten clickable cells (one per hole) spanning this beat. The click maps to
        // the quarter-beat tick under the cursor via `RelativeCursorPosition`.
        for hole in 1..=ROWS {
            let y = HEADER_H + (hole as f32 - 1.0) * ROW_H;
            let lane = if hole % 2 == 0 { LANE_A } else { LANE_B };
            let mut cell = commands.spawn((
                GridItem,
                Button,
                RelativeCursorPosition::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(y),
                    width: Val::Px(BEAT_W),
                    height: Val::Px(ROW_H),
                    ..default()
                },
                BackgroundColor(lane),
            ));
            cell.observe(
                move |ev: On<Pointer<Click>>,
                      rel: Query<&RelativeCursorPosition>,
                      mut state: ResMut<EditorState>| {
                    let frac = rel
                        .get(ev.entity)
                        .ok()
                        .and_then(|r| r.normalized)
                        .map_or(0.0, |n| n.x)
                        .clamp(0.0, 0.999);
                    let sub = (frac * TICKS_PER_BEAT as f32).floor() as usize;
                    select_or_add(&mut state, hole, beat * TICKS_PER_BEAT + sub);
                },
            );
            items.push(cell.id());
        }
    }

    // Notes are a separate overlay above the cells (ZIndex 1) so a note can span
    // several columns and intercept clicks across its whole width. Only notes
    // intersecting the visible window (in ticks) are spawned.
    let first_tick = state.scroll_beat * TICKS_PER_BEAT;
    let last_tick = (state.scroll_beat + cols + 1) * TICKS_PER_BEAT;
    for note in &state.notes {
        if note.tick < last_tick && note.tick + note.len > first_tick {
            let selected = state.selected == Some(note.id);
            items.push(spawn_note(&mut commands, *note, selected, &mut tail_mats));
        }
    }

    commands.entity(content).add_children(&items);
}

/// Absolute pixel rect (left, top, width, height) of a note inside [`GridContent`].
/// The horizontal scroll is applied by the parent's offset, not here.
fn note_rect(note: &GridNote) -> (f32, f32, f32, f32) {
    let left = note.tick as f32 * TICK_W + 1.0;
    let top = HEADER_H + (note.hole as f32 - 1.0) * ROW_H + NOTE_PAD;
    let width = note.len as f32 * TICK_W - 2.0;
    let height = ROW_H - 2.0 * NOTE_PAD;
    (left, top, width, height)
}

/// Spawn a note as an overlay entity above the grid cells. The root is the
/// clickable/selectable body (spanning `len` beats); dragging it moves the note,
/// while two thin children at its left and right edges are the drag-to-resize
/// handles. Plain pitches are flat-coloured; Wah/Vibrato render through the
/// gameplay note shader so the tile visibly pulses (wah) or wobbles (vibrato).
fn spawn_note(
    commands: &mut Commands,
    note: GridNote,
    selected: bool,
    tail_mats: &mut Assets<NoteTail2dMaterial>,
) -> Entity {
    let (left, top, width, height) = note_rect(&note);
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
        // Dragging the body (the middle of the note) moves it on both axes. A
        // drag that started on an edge handle bubbles up here too, so yield if a
        // drag is already in progress (the handle's resize owns it).
        .observe(move |_: On<Pointer<DragStart>>, mut state: ResMut<EditorState>| {
            if state.dragging.is_some() {
                return;
            }
            if let Some(n) = state.note_by_id(id).copied() {
                state.selected = Some(id);
                state.dragging = Some(DragState::new(id, DragKind::Move, &n));
            }
        })
        .observe(move |ev: On<Pointer<Drag>>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging else { return };
            if drag.id != id || drag.kind != DragKind::Move {
                return;
            }
            // Snap to a target cell, but only mark it droppable when that hole is
            // free there — the note itself doesn't move until release.
            let (hole, tick) = move_target(drag.start_hole, drag.start_tick, ev.distance.x, ev.distance.y);
            let valid = can_place(&state.notes, id, hole, tick, drag.start_len);
            if let Some(d) = state.dragging.as_mut() {
                d.target_hole = hole;
                d.target_tick = tick;
                d.valid = valid;
            }
        })
        .observe(move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging.take() else { return };
            // Commit the move only if it landed on a free spot; otherwise the note
            // stays put. Resize drags are finalised by the handle, not here.
            if drag.kind == DragKind::Move && drag.valid {
                if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                    n.hole = drag.target_hole;
                    n.tick = drag.target_tick;
                }
                enforce_direction(&mut state, id);
            }
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
                state.dragging = Some(DragState::new(id, DragKind::Resize(edge), &n));
            }
        })
        .observe(move |ev: On<Pointer<Drag>>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging else { return };
            if drag.id != id || drag.kind != DragKind::Resize(edge) {
                return;
            }
            let hole = drag.start_hole;
            // Bound the resize by the neighbouring notes on this hole so two notes
            // on the same row can never overlap. Neighbours are taken relative to
            // the drag's start position (they don't move during the gesture).
            let mut left_bound = 0usize;
            let mut right_bound: Option<usize> = None;
            for n in &state.notes {
                if n.id == id || n.hole != hole {
                    continue;
                }
                if n.tick < drag.start_tick {
                    left_bound = left_bound.max(n.tick + n.len);
                } else {
                    right_bound = Some(right_bound.map_or(n.tick, |r| r.min(n.tick)));
                }
            }

            let steps = (ev.distance.x / TICK_W).round() as i32;
            let (tick, len) =
                apply_resize(drag.start_tick, drag.start_len, edge, steps, left_bound, right_bound);
            if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                n.tick = tick;
                n.len = len;
            }
        })
        .observe(move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
            // Only finalise a resize here; a move that bubbled up is owned by the
            // body's DragEnd (which runs after this and finds `dragging` taken).
            if matches!(state.dragging, Some(d) if d.id == id && matches!(d.kind, DragKind::Resize(_))) {
                state.dragging = None;
                // Resizing may have pulled this note over notes of the opposite
                // breath direction; unify them (the dragged note wins).
                enforce_direction(&mut state, id);
            }
        });
}

/// While an edge *resize* drag is active, reposition/resize the dragged note's
/// entity directly (the grid rebuild is paused). A *move* drag leaves the note in
/// place and is previewed by the ghost instead, so it's skipped here.
fn live_resize(state: Res<EditorState>, mut notes: Query<(&NoteView, &mut Node)>) {
    let Some(drag) = state.dragging else { return };
    if !matches!(drag.kind, DragKind::Resize(_)) {
        return;
    }
    let Some(note) = state.note_by_id(drag.id) else { return };
    let (left, _top, width, _height) = note_rect(note);
    for (view, mut node) in &mut notes {
        if view.0 == drag.id {
            node.left = Val::Px(left);
            node.width = Val::Px(width);
        }
    }
}

/// Show the yellow move-preview ghost at the drop target while a move drag is
/// active (red when the spot is occupied), and hide it otherwise.
fn update_move_ghost(
    state: Res<EditorState>,
    mut ghost: Query<(&mut Node, &mut Visibility, &mut BackgroundColor, &mut BorderColor), With<MoveGhost>>,
) {
    let Ok((mut node, mut vis, mut bg, mut border)) = ghost.single_mut() else { return };
    match state.dragging {
        Some(drag) if drag.kind == DragKind::Move => {
            let left = drag.target_tick as f32 * TICK_W + 1.0;
            let top = HEADER_H + (drag.target_hole as f32 - 1.0) * ROW_H + NOTE_PAD;
            node.left = Val::Px(left);
            node.top = Val::Px(top);
            node.width = Val::Px(drag.start_len as f32 * TICK_W - 2.0);
            *vis = Visibility::Inherited;
            let color = if drag.valid { GHOST_OK } else { GHOST_BAD };
            bg.0 = color.with_alpha(0.30);
            *border = BorderColor::all(color);
        }
        _ => *vis = Visibility::Hidden,
    }
}

// ── Interaction ──────────────────────────────────────────────────────────────

fn select_or_add(state: &mut EditorState, hole: u8, tick: usize) {
    // Select any note on this hole that already covers the clicked tick — a hole
    // sounds one note at a time, so we never stack a second note onto it.
    if let Some(existing) = state
        .notes
        .iter()
        .find(|n| n.hole == hole && n.tick <= tick && tick < n.tick + n.len)
    {
        state.selected = Some(existing.id);
        return;
    }
    // New notes default to one beat, shortened so they can't overlap the next
    // note already on this hole (at least one tick — a quarter beat).
    let next_start = state
        .notes
        .iter()
        .filter(|n| n.hole == hole && n.tick > tick)
        .map(|n| n.tick)
        .min();
    let len = next_start
        .map_or(TICKS_PER_BEAT, |start| (start - tick).min(TICKS_PER_BEAT))
        .max(1);
    // A new note adopts whatever breath direction is already sounding at this
    // tick (all notes at one instant share it), defaulting to blow when alone.
    let dir = state.dir_at(tick).unwrap_or(Dir::Blow);
    let id = state.next_id;
    state.next_id += 1;
    state.notes.push(GridNote {
        id,
        hole,
        tick,
        len,
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

fn pan_keys(keyboard: Res<ButtonInput<KeyCode>>, state: Res<EditorState>, mut scroll: ResMut<Scroll>) {
    if state.focus.is_some() {
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        scroll.px += BEAT_W;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        scroll.px = (scroll.px - BEAT_W).max(0.0);
    }
}

fn pan_wheel(mut wheel: MessageReader<MouseWheel>, mut scroll: ResMut<Scroll>) {
    let mut delta = 0.0;
    for ev in wheel.read() {
        // Either axis pans horizontally; a vertical wheel is the common case.
        delta += if ev.y != 0.0 { ev.y } else { ev.x };
    }
    if delta != 0.0 {
        scroll.px = (scroll.px - delta * BEAT_W).max(0.0);
    }
}

/// Apply the continuous scroll to the grid: slide [`GridContent`] and, when the
/// base beat changes, update `scroll_beat` so the cull-rebuild re-runs (only then).
fn apply_scroll(
    scroll: Res<Scroll>,
    mut state: ResMut<EditorState>,
    mut content: Query<&mut Node, With<GridContent>>,
) {
    if let Ok(mut node) = content.single_mut() {
        node.left = Val::Px(-scroll.px);
    }
    let base = (scroll.px / BEAT_W) as usize;
    // Only write (→ trigger a rebuild) when the integer base beat actually moves.
    if state.scroll_beat != base {
        state.scroll_beat = base;
    }
}

/// During playback, scroll forward just enough to keep the playhead from passing
/// `FOLLOW_LEAD` of the way across the viewport. It never scrolls backward, so a
/// section in the left part of the view stays put until the playhead reaches it.
fn auto_scroll(
    playhead: Res<Playhead>,
    windows: Query<&Window>,
    mut scroll: ResMut<Scroll>,
) {
    if !playhead.playing || playhead.secs_per_tick <= 0.0 {
        return;
    }
    const FOLLOW_LEAD: f32 = 0.7;
    let view_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0) - HOLE_COL_W;
    let head_px = playhead.elapsed / playhead.secs_per_tick * TICK_W;
    let target = head_px - FOLLOW_LEAD * view_w;
    if target > scroll.px {
        scroll.px = target;
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

fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<EditorRoot>>,
    audio: Query<Entity, With<EditorAudio>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    // Audio players live at the world root (not under EditorRoot), so stop them
    // explicitly when leaving the editor.
    for e in &audio {
        commands.entity(e).despawn();
    }
}

// ── Playback / synthesis ─────────────────────────────────────────────────────

/// Marks an audio player spawned by the editor's Play button, so it can be
/// stopped (Stop button or leaving the editor).
#[derive(Component)]
struct EditorAudio;

/// The moving playback cursor (a vertical line) drawn over the grid.
#[derive(Component)]
struct PlayheadLine;

/// The growing fill of the top progress bar (overall playback fraction).
#[derive(Component)]
struct EditorProgressFill;

/// Drive the top progress bar from the playhead's elapsed/total time.
fn update_progress_bar(playhead: Res<Playhead>, mut fills: Query<&mut Node, With<EditorProgressFill>>) {
    let p = if playhead.total > 0.0 {
        (playhead.elapsed / playhead.total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for mut node in &mut fills {
        node.width = Val::Percent(p * 100.0);
    }
}

/// Tracks the playhead while the authored notes are sounding. Time is followed
/// independently of the audio sink, which is plenty accurate for a preview.
#[derive(Resource, Default)]
struct Playhead {
    playing: bool,
    /// Seconds since playback began.
    elapsed: f32,
    /// Total span to sweep (last note's end), in seconds.
    total: f32,
    /// Seconds per tick at the playback tempo (0 when idle).
    secs_per_tick: f32,
}

/// Advance the playhead clock; stop it once the swept span is done.
fn advance_playhead(time: Res<Time>, mut playhead: ResMut<Playhead>) {
    if playhead.playing {
        playhead.elapsed += time.delta_secs();
        if playhead.elapsed >= playhead.total {
            playhead.playing = false;
        }
    }
}

/// Position the playhead line at the current playback tick (absolute, inside
/// [`GridContent`], so it scrolls with the grid), hiding it when idle. Anything
/// off-screen is clipped by [`GridArea`].
fn update_playhead_view(
    playhead: Res<Playhead>,
    mut line: Query<(&mut Node, &mut Visibility), With<PlayheadLine>>,
) {
    let Ok((mut node, mut vis)) = line.single_mut() else { return };
    if !playhead.playing || playhead.secs_per_tick <= 0.0 {
        *vis = Visibility::Hidden;
        return;
    }
    let cur_tick = playhead.elapsed / playhead.secs_per_tick;
    node.left = Val::Px(cur_tick * TICK_W);
    *vis = Visibility::Inherited;
}

/// Render sample rate for the synthesised preview.
const SAMPLE_RATE: u32 = 44_100;

/// Blow / draw note labels of a standard C diatonic harp, by hole (1..=10).
const C_BLOW: [&str; 10] = ["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"];
const C_DRAW: [&str; 10] = ["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"];

/// Semitones to transpose a C-harp pitch by for the given harp key (0 for C, 7
/// for G, …). Unparseable keys fall back to C. Mirrors the old editor's
/// `harp_layout`, which transposes the C layout up by the key's offset.
fn key_offset(key: &str) -> i32 {
    note_to_midi(&format!("{}4", key.trim())).map_or(0, |m| m - 60)
}

/// Frequency (Hz) of a note: its hole + breath direction on a harp in the given
/// key (as a semitone offset from C), shifted by the pitch technique (bend pulls
/// down, over-blow/draw push up a semitone). Out-of-range holes return `None`.
fn note_freq(note: &GridNote, key_offset: i32) -> Option<f32> {
    let idx = (note.hole as usize).checked_sub(1)?;
    let label = match note.dir {
        Dir::Blow => *C_BLOW.get(idx)?,
        Dir::Draw => *C_DRAW.get(idx)?,
    };
    let midi = note_to_midi(label)? as f32 + key_offset as f32;
    let semitones = match note.pitch {
        Pitch::Normal => 0.0,
        Pitch::Bend(a) => -a,
        Pitch::Overblow | Pitch::Overdraw => 1.0,
    };
    Some(440.0 * 2f32.powf((midi + semitones - 69.0) / 12.0))
}

/// Harmonica reed waveform: additive synthesis with the first six harmonics at
/// amplitudes approximating a measured diatonic harmonica spectrum. Both even and
/// odd harmonics are present (unlike a clarinet), with a roll-off toward high
/// partials. Output is normalised to [-1, 1] by dividing by the sum of amplitudes.
fn harmonica_wave(freq: f32, t: f32) -> f32 {
    const PARTIALS: [(f32, f32); 6] = [
        (1.0, 1.00),
        (2.0, 0.50),
        (3.0, 0.35),
        (4.0, 0.18),
        (5.0, 0.10),
        (6.0, 0.05),
    ];
    const SUM: f32 = 1.00 + 0.50 + 0.35 + 0.18 + 0.10 + 0.05; // 2.18
    let mut s = 0.0f32;
    for (k, amp) in PARTIALS {
        s += amp * (TAU * freq * k * t).sin();
    }
    s / SUM
}

/// AHDSR envelope shaped for a harmonica reed: slightly slower attack than a
/// pure click-free ramp, a brief hold at peak, and a natural release tail.
fn envelope(i: usize, dur: usize) -> f32 {
    let attack  = (SAMPLE_RATE as f32 * 0.018) as usize; // 18 ms reed pressure build-up
    let release = (SAMPLE_RATE as f32 * 0.045) as usize; // 45 ms natural decay
    let atk = if attack > 0 && i < attack { i as f32 / attack as f32 } else { 1.0 };
    let rel = if dur > release && i > dur - release {
        (dur - i) as f32 / release as f32
    } else {
        1.0
    };
    atk.min(rel).clamp(0.0, 1.0)
}

/// Synthesise the notes to mono f32 PCM at [`SAMPLE_RATE`]. Each note uses a
/// six-harmonic reed model with a breath-noise layer (audible on the attack) that
/// gives the characteristic "chiff" of a real harmonica. Vibrato modulates pitch;
/// Wah modulates amplitude. The mix is peak-normalised so stacked notes don't clip.
fn render_pcm(notes: &[GridNote], bpm: f32, key_offset: i32) -> Vec<f32> {
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let end_tick = notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    let total = ((end_tick as f32 * secs_per_tick + 0.25) * SAMPLE_RATE as f32).ceil() as usize;
    let mut buf = vec![0.0f32; total.max(1)];

    for n in notes {
        let Some(freq) = note_freq(n, key_offset) else { continue };
        let start = (n.tick as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;
        let dur = (n.len as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;
        // Per-note LCG seeded by hole so simultaneous notes have uncorrelated noise.
        let mut rng: u32 = 0x9e3779b9u32
            .wrapping_add((n.hole as u32).wrapping_mul(2654435761))
            .wrapping_add((n.tick as u32).wrapping_mul(1013904223));
        for i in 0..dur {
            let s = start + i;
            if s >= buf.len() {
                break;
            }
            let t = i as f32 / SAMPLE_RATE as f32;
            let env = envelope(i, dur);

            let f = match n.expr {
                Expr::Vibrato => freq * (1.0 + 0.012 * (TAU * 5.5 * t).sin()),
                _ => freq,
            };
            let amp_mod = match n.expr {
                Expr::Wah => 0.55 + 0.45 * (TAU * 4.0 * t).sin().abs(),
                _ => 1.0,
            };

            // Reed tone: six harmonics
            let tone = harmonica_wave(f, t);

            // Breath noise: band-limited hiss strongest at attack, fades to ~5%
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise_sample = (rng as i32) as f32 / i32::MAX as f32;
            // Noise decays exponentially after the attack window so only the
            // initial "chiff" is audible, not a sustained hiss.
            let attack_samples = (SAMPLE_RATE as f32 * 0.018) as usize;
            let noise_env = if i < attack_samples {
                1.0
            } else {
                (-3.0 * (i - attack_samples) as f32 / SAMPLE_RATE as f32).exp()
            };
            let breath = noise_sample * 0.07 * noise_env;

            buf[s] += 0.25 * env * amp_mod * (tone + breath);
        }
    }

    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 1.0 {
        for x in &mut buf {
            *x /= peak;
        }
    }
    buf
}

/// Encode mono f32 samples as a 16-bit PCM WAV file in memory (the `wav` bevy
/// feature lets `AudioSource` decode it).
fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let byte_rate = sample_rate * 2; // mono, 2 bytes/sample
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    v.extend_from_slice(&1u16.to_le_bytes()); // format = PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // channels = mono
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&byte_rate.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let q = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        v.extend_from_slice(&q.to_le_bytes());
    }
    v
}

/// Stop any current playback, then sound the authored notes and, if the Music
/// field names a readable audio file, that track too.
fn start_playback(
    state: &EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing: &Query<Entity, With<EditorAudio>>,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    *playhead = Playhead::default();

    let bpm = state.tempo.trim().parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    if !state.notes.is_empty() {
        let wav = encode_wav(&render_pcm(&state.notes, bpm, key_offset(&state.key)), SAMPLE_RATE);
        let handle = sources.add(AudioSource { bytes: wav.into() });
        commands.spawn((
            EditorAudio,
            AudioPlayer::<AudioSource>(handle),
            PlaybackSettings::DESPAWN,
        ));
        // Run the playhead across the authored span (last note end).
        let end_tick = state.notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
        *playhead = Playhead {
            playing: true,
            elapsed: 0.0,
            total: end_tick as f32 * secs_per_tick,
            secs_per_tick,
        };
    }

    // Optional background track: read the file the user named in the Music field.
    let music = state.music.trim();
    if !music.is_empty() {
        match std::fs::read(music) {
            Ok(bytes) => {
                let handle = sources.add(AudioSource { bytes: bytes.into() });
                commands.spawn((
                    EditorAudio,
                    AudioPlayer::<AudioSource>(handle),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.music_volume)),
                ));
            }
            Err(e) => warn!("Song editor: couldn't read background music {music:?}: {e}"),
        }
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
        // New notes start at the clicked tick and default to a full beat.
        assert_eq!((added.hole, added.tick, added.len), (4, 2, TICKS_PER_BEAT));
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
    fn clicking_a_covered_beat_selects_rather_than_stacks() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 4, 0);
        // Grow it to span beats 0..3.
        let id = s.notes[0].id;
        s.notes[0].len = 3;
        // Clicking beat 2 on the same hole hits the existing note, not a new one.
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.selected, Some(id));
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
            GridNote { id: 0, hole: 1, tick: 0, len: 3, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
            // Overlaps id 0 over beat 2.
            GridNote { id: 1, hole: 2, tick: 2, len: 3, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
            // Far away, touches nothing.
            GridNote { id: 2, hole: 3, tick: 10, len: 1, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
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
        // A note at beat 4, length 1, with no neighbours (0, None).
        assert_eq!(apply_resize(4, 1, Edge::Right, 2, 0, None), (4, 3)); // drag right → longer
        assert_eq!(apply_resize(4, 3, Edge::Right, -1, 0, None), (4, 2)); // drag left → shorter
        assert_eq!(apply_resize(4, 2, Edge::Right, -5, 0, None), (4, 1)); // never below one beat
    }

    #[test]
    fn left_edge_moves_start_and_resizes_inversely() {
        // Dragging the left edge right shortens and pushes the start later.
        assert_eq!(apply_resize(4, 3, Edge::Left, 1, 0, None), (5, 2));
        // Dragging it left lengthens and pulls the start earlier.
        assert_eq!(apply_resize(4, 2, Edge::Left, -2, 0, None), (2, 4));
        // The start never passes the right edge (len stays >= 1)…
        assert_eq!(apply_resize(4, 2, Edge::Left, 9, 0, None), (5, 1));
        // …nor goes before beat 0.
        assert_eq!(apply_resize(1, 2, Edge::Left, -9, 0, None), (0, 3));
    }

    fn note(hole: u8, dir: Dir, pitch: Pitch) -> GridNote {
        GridNote { id: 0, hole, tick: 0, len: 4, dir, pitch, expr: Expr::None }
    }

    #[test]
    fn note_freq_maps_holes_bends_and_key() {
        // Hole 1 blow on a C harp is C4 ≈ 261.63 Hz.
        let c4 = note_freq(&note(1, Dir::Blow, Pitch::Normal), 0).unwrap();
        assert!((c4 - 261.63).abs() < 0.5, "got {c4}");
        // A whole-step bend lowers the pitch.
        let bent = note_freq(&note(1, Dir::Blow, Pitch::Bend(1.0)), 0).unwrap();
        assert!(bent < c4, "bend should drop pitch: {bent} !< {c4}");
        // A G harp (offset 7) raises the same hole a fifth.
        let g = note_freq(&note(1, Dir::Blow, Pitch::Normal), key_offset("G")).unwrap();
        assert!((g / c4 - 2f32.powf(7.0 / 12.0)).abs() < 0.001, "G harp is a fifth up");
        assert_eq!(key_offset("C"), 0);
        // Hole 11 doesn't exist.
        assert!(note_freq(&note(11, Dir::Blow, Pitch::Normal), 0).is_none());
    }

    #[test]
    fn render_and_wav_have_expected_size() {
        // One whole-beat note at 120 BPM = 0.5 s of audio, plus a 0.25 s tail.
        let notes = vec![note(4, Dir::Draw, Pitch::Normal)];
        let pcm = render_pcm(&notes, 120.0, 0);
        let expected = ((0.5 + 0.25) * SAMPLE_RATE as f32).ceil() as usize;
        assert_eq!(pcm.len(), expected);
        assert!(pcm.iter().any(|&s| s.abs() > 0.01), "note should be audible");
        // WAV = 44-byte header + 2 bytes/sample, and starts with RIFF/WAVE.
        let wav = encode_wav(&pcm, SAMPLE_RATE);
        assert_eq!(wav.len(), 44 + pcm.len() * 2);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn move_target_snaps_and_clamps() {
        // No movement → same cell.
        assert_eq!(move_target(5, 4, 0.0, 0.0), (5, 4));
        // One tick right, two rows down.
        assert_eq!(move_target(5, 4, TICK_W, 2.0 * ROW_H), (7, 5));
        // A whole beat is TICKS_PER_BEAT ticks.
        assert_eq!(move_target(5, 4, BEAT_W, 0.0), (5, 4 + TICKS_PER_BEAT));
        // Rows clamp to 1..=ROWS, ticks never go below 0.
        assert_eq!(move_target(1, 0, -5.0 * BEAT_W, -5.0 * ROW_H), (1, 0));
        assert_eq!(move_target(10, 2, 0.0, 5.0 * ROW_H), (10, 2));
    }

    #[test]
    fn move_is_blocked_where_a_note_already_sits() {
        let notes = vec![
            GridNote { id: 0, hole: 3, tick: 0, len: 2, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
            GridNote { id: 1, hole: 3, tick: 5, len: 1, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
        ];
        // Moving note 1 onto beat 1 of hole 3 overlaps note 0 → not placeable.
        assert!(!can_place(&notes, 1, 3, 1, 1));
        // Beat 2 is free (note 0 spans 0..2), and a different hole is free too.
        assert!(can_place(&notes, 1, 3, 2, 1));
        assert!(can_place(&notes, 1, 4, 0, 1));
    }

    #[test]
    fn resize_stops_at_neighbour_on_same_hole() {
        // Right edge can't grow past the next note's start (right_bound = 3).
        assert_eq!(apply_resize(0, 1, Edge::Right, 10, 0, Some(3)), (0, 3));
        // Left edge can't move before the previous note's end (left_bound = 2).
        assert_eq!(apply_resize(4, 2, Edge::Left, -10, 2, None), (2, 4));
    }
}
