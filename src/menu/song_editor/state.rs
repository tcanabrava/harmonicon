// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::{HEADER_H, NOTE_PAD, ROW_H, TICK_W, ROWS};

// ── Note model types ─────────────────────────────────────────────────────────

/// The pitch technique of a note. Mutually exclusive — a note is exactly one of
/// these. `Bend` carries its depth in semitones (0.5, 1.0 or 1.5).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Pitch {
    Normal,
    Bend(f32),
    Overblow,
    Overdraw,
}

/// An expression technique layered on top of the pitch. At most one at a time;
/// either may combine with any [`Pitch`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Expr {
    None,
    Wah,
    Vibrato,
}

/// Breath direction: blow (exhale) or draw (inhale). Every note is one or the
/// other; toggled with the Blow/Draw buttons.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Dir {
    Blow,
    Draw,
}

impl Dir {
    pub(super) fn arrow(self) -> &'static str {
        match self {
            Dir::Blow => "\u{2191}",
            Dir::Draw => "\u{2193}",
        }
    }
}

/// One placed note: a hole (1..=10) starting at `tick` and lasting `len` ticks,
/// plus its techniques. `id` is a stable handle so the note keeps its identity
/// while its `tick`/`len` change under a drag.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct GridNote {
    pub(super) id: u32,
    pub(super) hole: u8,
    pub(super) tick: usize,
    pub(super) len: usize,
    pub(super) dir: Dir,
    pub(super) pitch: Pitch,
    pub(super) expr: Expr,
}

impl GridNote {
    pub(super) fn bend(&self) -> f32 {
        match self.pitch {
            Pitch::Bend(a) => a,
            _ => 0.0,
        }
    }
}

// ── Drag state ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Edge {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum DragKind {
    Move,
    Resize(Edge),
}

#[derive(Clone, Copy)]
pub(super) struct DragState {
    pub(super) id: u32,
    pub(super) kind: DragKind,
    pub(super) start_tick: usize,
    pub(super) start_len: usize,
    pub(super) start_hole: u8,
    pub(super) target_hole: u8,
    pub(super) target_tick: usize,
    pub(super) valid: bool,
}

impl DragState {
    pub(super) fn new(id: u32, kind: DragKind, note: &GridNote) -> Self {
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

// ── Metadata field types ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Field {
    Tempo,
    Key,
    Music,
    Name,
    Author,
}

/// Each entry pairs a [`Field`] with the localization key used for its label.
pub(super) const FIELDS: [(Field, &str); 5] = [
    (Field::Tempo, "editor-field-tempo"),
    (Field::Key,   "editor-field-key"),
    (Field::Music, "editor-field-music"),
    (Field::Name,  "editor-field-name"),
    (Field::Author,"editor-field-author"),
];

/// All valid diatonic harp keys in chromatic order.
pub(super) const HARP_KEYS: [&str; 12] =
    ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

// ── Resources ────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub(super) struct EditorState {
    pub(super) notes: Vec<GridNote>,
    pub(super) next_id: u32,
    pub(super) selected: Option<u32>,
    pub(super) scroll_beat: usize,
    pub(super) dragging: Option<DragState>,
    pub(super) tempo: String,
    pub(super) key: String,
    pub(super) music: String,
    pub(super) name: String,
    pub(super) author: String,
    pub(super) focus: Option<Field>,
    pub(super) drag_msg: crate::localization::LocalizedStr,
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
            drag_msg: crate::localization::LocalizedStr::default(),
        }
    }
}

impl EditorState {
    #[cfg(test)]
    pub(super) fn note_at(&self, hole: u8, tick: usize) -> Option<&GridNote> {
        self.notes.iter().find(|n| n.hole == hole && n.tick == tick)
    }

    pub(super) fn note_by_id(&self, id: u32) -> Option<&GridNote> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub(super) fn dir_at(&self, tick: usize) -> Option<Dir> {
        self.notes
            .iter()
            .find(|n| n.tick <= tick && tick < n.tick + n.len)
            .map(|n| n.dir)
    }

    pub(super) fn selected_note(&self) -> Option<&GridNote> {
        self.selected.and_then(|id| self.note_by_id(id))
    }

    pub(super) fn selected_note_mut(&mut self) -> Option<&mut GridNote> {
        let id = self.selected?;
        self.notes.iter_mut().find(|n| n.id == id)
    }

    pub(super) fn field_text(&self, field: Field) -> &str {
        match field {
            Field::Tempo => &self.tempo,
            Field::Key => &self.key,
            Field::Music => &self.music,
            Field::Name => &self.name,
            Field::Author => &self.author,
        }
    }

    pub(super) fn field_text_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Tempo => &mut self.tempo,
            Field::Key => &mut self.key,
            Field::Music => &mut self.music,
            Field::Name => &mut self.name,
            Field::Author => &mut self.author,
        }
    }
}

/// Continuous horizontal scroll in pixels. Kept separate from [`EditorState`]
/// so scrolling doesn't trigger a grid rebuild.
#[derive(Resource, Default)]
pub(super) struct Scroll {
    pub(super) px: f32,
}

// ── Note model logic ─────────────────────────────────────────────────────────

pub(super) fn can_place(notes: &[GridNote], id: u32, hole: u8, tick: usize, len: usize) -> bool {
    !notes
        .iter()
        .any(|n| n.id != id && n.hole == hole && n.tick < tick + len && tick < n.tick + n.len)
}

pub(super) fn pitch_compatible(pitch: Pitch, hole: u8) -> bool {
    match pitch {
        Pitch::Normal => true,
        Pitch::Bend(depth) => depth <= max_bend(hole) + f32::EPSILON,
        Pitch::Overblow => overblow_ok(hole),
        Pitch::Overdraw => overdraw_ok(hole),
    }
}

/// Returns the localization key for the reason a pitch is not allowed on a hole,
/// or `""` when the pitch is valid (callers should skip `loc.msg("")`).
pub(super) fn pitch_deny_key(pitch: Pitch, _hole: u8) -> &'static str {
    match pitch {
        Pitch::Bend(_) => "drag-denied-bend",
        Pitch::Overblow => "drag-denied-overblow",
        Pitch::Overdraw => "drag-denied-overdraw",
        Pitch::Normal => "",
    }
}

pub(super) fn max_bend(hole: u8) -> f32 {
    match hole {
        2 | 3 | 10 => 1.5,
        1 | 6 | 8 | 9 => 1.0,
        4 | 5 | 7 => 0.5,
        _ => 0.0,
    }
}

pub(super) fn overblow_ok(hole: u8) -> bool {
    (1..=6).contains(&hole)
}

pub(super) fn overdraw_ok(hole: u8) -> bool {
    (7..=10).contains(&hole)
}

pub(super) fn pitch_color(pitch: Pitch) -> Color {
    match pitch {
        Pitch::Normal => Color::srgb(0.30, 0.60, 0.95),
        Pitch::Bend(a) => {
            let t = (a / 1.5).clamp(0.0, 1.0);
            Color::srgb(0.95, 0.55 - 0.30 * t, 0.22)
        }
        Pitch::Overblow => Color::srgb(0.72, 0.42, 0.95),
        Pitch::Overdraw => Color::srgb(0.28, 0.85, 0.78),
    }
}

pub(super) fn move_target(start_hole: u8, start_tick: usize, dist_x: f32, dist_y: f32) -> (u8, usize) {
    let steps_x = (dist_x / TICK_W).round() as i32;
    let steps_y = (dist_y / ROW_H).round() as i32;
    let hole = (start_hole as i32 + steps_y).clamp(1, ROWS as i32) as u8;
    let tick = (start_tick as i32 + steps_x).max(0) as usize;
    (hole, tick)
}

pub(super) fn apply_resize(
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
            end = end.max((tick + 1) as i32);
            if let Some(rb) = right_bound {
                end = end.min(rb as i32);
            }
            (tick, end as usize - tick)
        }
        Edge::Left => {
            let end = tick + len;
            let mut start = tick as i32 + steps;
            start = start.min((end - 1) as i32);
            start = start.max(left_bound as i32);
            (start as usize, end - start as usize)
        }
    }
}

pub(super) fn overlaps(a: &GridNote, b: &GridNote) -> bool {
    a.tick < b.tick + b.len && b.tick < a.tick + a.len
}

pub(super) fn enforce_direction(state: &mut EditorState, id: u32) {
    let Some(dir) = state.note_by_id(id).map(|n| n.dir) else { return };
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

/// Wah (hand cupping) and vibrato (breath/diaphragm) are both whole-player
/// techniques: whichever one you're doing, it colours every hole sounding at
/// that instant, not just one. So `id`'s `expr` — Wah, Vibrato, or None — is
/// propagated to every note that overlaps it in time, transitively, the same
/// way `enforce_direction` propagates a shared Blow/Draw.
pub(super) fn enforce_expr(state: &mut EditorState, id: u32) {
    let Some(expr) = state.note_by_id(id).map(|n| n.expr) else { return };
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
            n.expr = expr;
        }
    }
}

/// Absolute pixel rect (left, top, width, height) of a note inside GridContent.
pub(super) fn note_rect(note: &GridNote) -> (f32, f32, f32, f32) {
    let left = note.tick as f32 * TICK_W + 1.0;
    let top = HEADER_H + (note.hole as f32 - 1.0) * ROW_H + NOTE_PAD;
    let width = note.len as f32 * TICK_W - 2.0;
    let height = ROW_H - 2.0 * NOTE_PAD;
    (left, top, width, height)
}
