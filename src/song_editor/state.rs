// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::{HEADER_H, NOTE_PAD, ROW_H, TICKS_PER_BEAT, TICK_W};

// ── Note model types ─────────────────────────────────────────────────────────

/// The pitch technique of a note. Mutually exclusive — a note is exactly one of
/// these. `Bend` carries its depth in semitones (0.5, 1.0 or 1.5). `Bend`,
/// `Overblow` and `Overdraw` only apply to [`HarmonicaKind::Diatonic`]; `Slide`
/// (the chromatic slide button, a half-step raise) only to
/// [`HarmonicaKind::Chromatic`] — which is in play is gated by which mod
/// buttons the UI shows for the current [`EditorState::harmonica_kind`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Pitch {
    Normal,
    Bend(f32),
    Overblow,
    Overdraw,
    Slide,
}

/// Which harmonica the chart is authored for. Diatonic gets the full
/// bend/overblow/overdraw technique set on 10 holes; chromatic gets a slide
/// button on 12 holes instead — see [`EditorState::hole_count`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum HarmonicaKind {
    #[default]
    Diatonic,
    Chromatic,
}

/// An expression technique layered on top of the pitch. At most one at a time;
/// either may combine with any [`Pitch`]. Both carry their oscillation rate in
/// Hz, cycled through by repeatedly clicking the mod button — same pattern as
/// `Bend`'s depth. Defined in `audio_system::synth` (shared synthesis
/// vocabulary, also used by `gameplay::call_response`'s call-and-response
/// demo audio); re-exported here under its established name.
pub(super) use crate::audio_system::synth::Expr;

/// Breath direction: blow (exhale) or draw (inhale). Every note is one or the
/// other; toggled with the Blow/Draw buttons.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Dir {
    Blow,
    Draw,
}

/// Song editor work mode. `Edit` shows note-editing controls (Blow, Draw,
/// Bend, ...) and allows adding/moving/resizing notes. `Perform` hides those
/// and shows playback/practice controls instead, and always behaves as
/// locked regardless of the user's own [`EditorState::user_locked`] toggle —
/// see [`EditorState::locked`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum Mode {
    #[default]
    Edit,
    Perform,
}

impl Dir {
    pub(super) fn arrow(self) -> &'static str {
        match self {
            Dir::Blow => "\u{2191}",
            Dir::Draw => "\u{2193}",
        }
    }
}

// ── Timeline erase/remove tool ───────────────────────────────────────────────

/// Which destructive timeline operation is currently selected, if any —
/// toggled by the Erase/Remove buttons, and read by the timeline surface's
/// click/drag observers to decide whether they do anything at all. Mutually
/// exclusive; picking one deselects the other rather than stacking.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum TimelineTool {
    #[default]
    None,
    // Creates a span of selection on the timeline, that the Erase and Remove buttons
    // will act upon.
    Select,
    /// Deletes every note in the picked range, leaving a gap — the song's
    /// own length and every other note's position are untouched.
    Erase,
    /// Deletes every note in the picked range *and* shifts every note after
    /// it earlier by the range's length, closing the gap — the song gets
    /// shorter.
    Remove,
    /// Click-to-toggle a tempo-change point at the clicked tick — unlike
    /// Select/Erase/Remove, a single plain click (not a two-step
    /// select-then-confirm span) either adds or removes one point, with no
    /// confirm dialog (non-destructive to notes, trivially undone by
    /// clicking again). See `timeline::on_timeline_click_tempo`/
    /// `toggle_tempo_point`.
    Tempo,
}

impl TimelineTool {
    pub(super) fn is_active(self) -> bool {
        self != TimelineTool::None
    }
}

/// Which side of a placed split point the pointer is currently hovering —
/// determines what a follow-up click on the timeline acts on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Side {
    Left,
    Right,
}

/// An in-progress press-drag gesture on the timeline ruler: `start` is
/// fixed at the press position, `end` follows the pointer. Not normalized —
/// `end` can be less than `start` — see [`normalize_range`]. Mirrors
/// [`DragState`]'s role for note dragging: set by `Pointer<DragStart>`,
/// live-updated by `Pointer<Drag>`, and always cleared by
/// `Pointer<DragEnd>`, which then either confirms it as an explicit span
/// (`end` genuinely moved past `start`) or, since `bevy_picking` fires
/// `DragStart` on any nonzero pixel motion — meaning ordinary mouse jitter
/// during a plain click routinely produces one of these — falls back to
/// treating a same-tick `start`/`end` exactly like the click it was meant
/// to be, against [`EditorState::timeline_split`]. Deliberately *not* what
/// drives `Pointer<Click>`: a `Click` and a `DragEnd` fire on the same
/// release whenever the pointer is still over the ruler at release (true
/// for most drags), `Click` first — routing every decision through the
/// `Drag*` chain alone avoids that race outright instead of coordinating
/// two competing handlers.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct TimelineDrag {
    pub(super) start: usize,
    pub(super) end: usize,
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
    Position,
    Music,
    Name,
    Author,
}

/// Each entry pairs a [`Field`] with the localization key used for its label.
pub(super) const FIELDS: [(Field, &str); 6] = [
    (Field::Tempo, "editor-field-tempo"),
    (Field::Key, "editor-field-key"),
    (Field::Position, "editor-field-position"),
    (Field::Music, "editor-field-music"),
    (Field::Name, "editor-field-name"),
    (Field::Author, "editor-field-author"),
];

/// All valid diatonic harp keys in chromatic order.
pub(super) const HARP_KEYS: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// Playing positions in the order harmonica players commonly reach for them:
/// 1st (straight), 2nd (cross harp, the blues staple), 3rd through 5th, and
/// 12th, which jazz players use for its major-scale-friendly hole layout.
pub(super) const POSITIONS: [&str; 6] = ["1st", "2nd", "3rd", "4th", "5th", "12th"];

// ── Resources ────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub(super) struct EditorState {
    pub(super) notes: Vec<GridNote>,
    pub(super) next_id: u32,
    pub(super) selected: Option<u32>,
    pub(super) scroll_beat: usize,
    pub(super) dragging: Option<DragState>,
    pub(super) tempo: String,
    /// Tempo changes after the song's opening tempo (`tempo`, tick 0) —
    /// `(tick, bpm)` pairs in the editor's own tick unit, added via the
    /// timeline's Tempo tool (`timeline::tempo_tool_click`). Not
    /// necessarily sorted as edits land; [`EditorState::tempo_map`] sorts
    /// on read. Empty for the overwhelmingly common single-tempo case.
    pub(super) tempo_changes: Vec<(usize, f32)>,
    pub(super) key: String,
    pub(super) position: String,
    pub(super) music: String,
    pub(super) name: String,
    pub(super) author: String,
    pub(super) focus: Option<Field>,
    pub(super) drag_msg: crate::localization::LocalizedStr,
    pub(super) mode: Mode,
    /// User's own Lock toggle, independent of `mode`. See [`EditorState::locked`].
    pub(super) user_locked: bool,
    pub(super) harmonica_kind: HarmonicaKind,
    pub(super) timeline_tool: TimelineTool,
    /// A split point placed by a plain click-and-release on the timeline
    /// ruler (no meaningful drag) — persists across frames (unlike
    /// `timeline_drag`, which only lives for one gesture) until a second
    /// such click picks a side and consumes it, or the tool is switched.
    pub(super) timeline_split: Option<usize>,
    pub(super) timeline_drag: Option<TimelineDrag>,
    /// A range the user has committed to (a placed split's side, or a
    /// released drag span) and is now waiting on the confirm dialog's
    /// answer for. Set right before opening the dialog; read and cleared
    /// once `ConfirmChosen` arrives — see `timeline::handle_timeline_confirm`.
    pub(super) pending_timeline_op: Option<(TimelineTool, usize, usize)>,

    /// The mod buttons' persistent "current setting" for notes not yet
    /// placed — separate from any single note's own fields. Clicking a mod
    /// button always updates the relevant one of these, regardless of
    /// whether a note is currently selected, and it stays armed (see
    /// `interaction::apply_modifier`) until cycled back to its own "off"
    /// value (`Pitch::Normal`/`Expr::None`; direction has no "off" value —
    /// a note is always Blow or Draw — so `sticky_dir` only switches, never
    /// clears) or `set_harmonica_kind` sanitizes it away, same as it
    /// already does for every placed note's own pitch. `select_or_add`
    /// applies these to every newly placed note, silently skipping
    /// `sticky_pitch` (falling back to `Pitch::Normal` for that one note)
    /// when it doesn't fit the hole — the same "silently do nothing on an
    /// incompatible hole" rule clicking a pitch button on a selected note
    /// already has.
    pub(super) sticky_dir: Dir,
    pub(super) sticky_pitch: Pitch,
    pub(super) sticky_expr: Expr,
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
            tempo_changes: Vec::new(),
            key: "C".into(),
            position: "2nd".into(),
            music: String::new(),
            name: String::new(),
            author: String::new(),
            focus: None,
            drag_msg: crate::localization::LocalizedStr::default(),
            mode: Mode::default(),
            user_locked: false,
            harmonica_kind: HarmonicaKind::default(),
            timeline_tool: TimelineTool::default(),
            timeline_split: None,
            timeline_drag: None,
            pending_timeline_op: None,
            sticky_dir: Dir::Blow,
            sticky_pitch: Pitch::Normal,
            sticky_expr: Expr::None,
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

    /// The full tempo map (sorted, always starting at tick 0), built from
    /// the song's opening tempo (`tempo`) and any [`EditorState::
    /// tempo_changes`] — the representation every tick↔real-time
    /// conversion in the editor reads, via `song::chart::
    /// tick_to_seconds`/`seconds_to_tick`. See [`build_tempo_map`].
    pub(super) fn tempo_map(&self) -> Vec<crate::song::chart::TempoPoint> {
        build_tempo_map(&self.tempo, &self.tempo_changes)
    }

    pub(super) fn field_text(&self, field: Field) -> &str {
        match field {
            Field::Tempo => &self.tempo,
            Field::Key => &self.key,
            Field::Position => &self.position,
            Field::Music => &self.music,
            Field::Name => &self.name,
            Field::Author => &self.author,
        }
    }

    pub(super) fn field_text_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Tempo => &mut self.tempo,
            Field::Key => &mut self.key,
            Field::Position => &mut self.position,
            Field::Music => &mut self.music,
            Field::Name => &mut self.name,
            Field::Author => &mut self.author,
        }
    }

    /// True when notes cannot be added, moved, or resized: either the user
    /// turned Lock on themselves, or `mode` is `Perform` (which is always
    /// locked, regardless of the user's own toggle).
    pub(super) fn locked(&self) -> bool {
        self.user_locked || self.mode == Mode::Perform
    }

    /// The number of playable holes for the current [`HarmonicaKind`] — 10 for
    /// diatonic, 12 for chromatic (the most common chromatic harp; the chart
    /// format also allows 16, but the editor doesn't offer that layout).
    pub(super) fn hole_count(&self) -> u8 {
        match self.harmonica_kind {
            HarmonicaKind::Diatonic => 10,
            HarmonicaKind::Chromatic => 12,
        }
    }

    /// Switches [`EditorState::harmonica_kind`] and repairs any note that
    /// wouldn't be valid on the new harp: notes on holes beyond the new
    /// harp's range are dropped, and pitch techniques exclusive to the old
    /// kind (bend/overblow/overdraw for diatonic, slide for chromatic) fall
    /// back to `Pitch::Normal` rather than being silently misinterpreted.
    pub(super) fn set_harmonica_kind(&mut self, kind: HarmonicaKind) {
        self.harmonica_kind = kind;
        let hole_count = self.hole_count();
        self.notes.retain(|n| n.hole <= hole_count);
        let sanitize = |kind: HarmonicaKind, pitch: Pitch| match (kind, pitch) {
            (HarmonicaKind::Diatonic, Pitch::Slide) => Pitch::Normal,
            (HarmonicaKind::Chromatic, Pitch::Bend(_) | Pitch::Overblow | Pitch::Overdraw) => {
                Pitch::Normal
            }
            (_, p) => p,
        };
        for n in &mut self.notes {
            n.pitch = sanitize(kind, n.pitch);
        }
        self.sticky_pitch = sanitize(kind, self.sticky_pitch);
        if let Some(id) = self.selected
            && !self.notes.iter().any(|n| n.id == id)
        {
            self.selected = None;
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
        // The slide button works on every chromatic hole, so dragging a
        // slide note never needs to be denied on that basis.
        Pitch::Slide => true,
    }
}

/// Returns the localization key for the reason a pitch is not allowed on a hole,
/// or `""` when the pitch is valid (callers should skip `loc.msg("")`).
pub(super) fn pitch_deny_key(pitch: Pitch, _hole: u8) -> &'static str {
    match pitch {
        Pitch::Bend(_) => "drag-denied-bend",
        Pitch::Overblow => "drag-denied-overblow",
        Pitch::Overdraw => "drag-denied-overdraw",
        Pitch::Normal | Pitch::Slide => "",
    }
}

/// Vibrato rate range (Hz) the editor cycles through when repeatedly clicking
/// the Vibrato button — spans realistic diaphragm/breath vibrato speed.
pub(super) const VIBRATO_HZ_MIN: f32 = 3.0;
pub(super) const VIBRATO_HZ_MAX: f32 = 7.0;
pub(super) const VIBRATO_HZ_STEP: f32 = 1.0;

/// Hand-wah rate range (Hz) the editor cycles through when repeatedly
/// clicking the Wah button — hand movement is naturally slower than
/// diaphragm vibrato.
pub(super) const WAH_HZ_MIN: f32 = 2.0;
pub(super) const WAH_HZ_MAX: f32 = 5.0;
pub(super) const WAH_HZ_STEP: f32 = 1.0;

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

/// The breath direction `pitch` physically requires, if any — `Overblow`
/// only exists while *blowing* (the name says so), `Overdraw` only while
/// *drawing*, regardless of which reed the resulting pitch happens to sit
/// near (`song::harmonica::hole_notes`'s doc comment: overblow sounds a
/// semitone above the *draw* reed, overdraw above the *blow* reed — the
/// technique name is about the breath action, not the reed). `Bend`/`Slide`
/// have no such constraint — a bend can be dialed in on either a blow or a
/// draw note depending on the hole. Used to keep a note's `dir` and `pitch`
/// from drifting into a physically impossible pairing (e.g. "overblow"
/// tagged on a draw note) as either one changes independently.
pub(super) fn pitch_forced_dir(pitch: Pitch) -> Option<Dir> {
    match pitch {
        Pitch::Overblow => Some(Dir::Blow),
        Pitch::Overdraw => Some(Dir::Draw),
        _ => None,
    }
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
        Pitch::Slide => Color::srgb(0.90, 0.80, 0.25),
    }
}

pub(super) fn move_target(
    start_hole: u8,
    start_tick: usize,
    dist_x: f32,
    dist_y: f32,
    hole_count: u8,
) -> (u8, usize) {
    let steps_x = (dist_x / TICK_W).round() as i32;
    let steps_y = (dist_y / ROW_H).round() as i32;
    let hole = (start_hole as i32 + steps_y).clamp(1, hole_count as i32) as u8;
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
    let Some(dir) = state.note_by_id(id).map(|n| n.dir) else {
        return;
    };
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
    let Some(expr) = state.note_by_id(id).map(|n| n.expr) else {
        return;
    };
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

// ── Tempo map ─────────────────────────────────────────────────────────────────

/// Builds the full tempo map (sorted, always starting at tick 0) from the
/// song's opening tempo (`tempo`, the `Field::Tempo` text value) and any
/// additional tempo-change points (`tempo_changes`, added via the
/// timeline's Tempo tool) — the representation every tick↔real-time
/// conversion in the editor reads, via `song::chart::
/// tick_to_seconds`/`seconds_to_tick`. Ticks here are the editor's own
/// tick unit (`TICKS_PER_BEAT` per beat), which is also exactly the
/// `resolution` the editor writes to a saved chart's `timing.resolution`
/// (`harpchart::serialize_harpchart`) — so this can be handed straight to
/// those functions with no unit conversion. A duplicate tick (e.g. a
/// tempo-change point placed at tick 0, where the opening tempo already
/// applies) silently keeps the earlier-sorted entry rather than erroring —
/// same "always resolves to something reasonable" spirit the rest of the
/// editor's fallback chains follow.
pub(super) fn build_tempo_map(tempo: &str, tempo_changes: &[(usize, f32)]) -> Vec<crate::song::chart::TempoPoint> {
    use crate::song::chart::TempoPoint;
    let bpm0: f32 = tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
    let mut map = vec![TempoPoint { tick: 0, bpm: bpm0 }];
    map.extend(
        tempo_changes
            .iter()
            .map(|&(tick, bpm)| TempoPoint { tick: tick as u64, bpm: bpm.max(1.0) }),
    );
    map.sort_by_key(|p| p.tick);
    map.dedup_by_key(|p| p.tick);
    map
}

/// The bpm in effect at `tick` per `tempo_map` (whichever point last took
/// effect at or before it) — the starting point [`toggle_tempo_point`]
/// steps a new point's bpm from, so a freshly-added point doesn't silently
/// jump to some unrelated tempo.
fn bpm_at(tempo_map: &[crate::song::chart::TempoPoint], tick: usize) -> f32 {
    tempo_map
        .iter()
        .rev()
        .find(|p| p.tick <= tick as u64)
        .map(|p| p.bpm)
        .unwrap_or(120.0)
}

/// How close (in ticks) a click has to land to an existing tempo-change
/// point for [`toggle_tempo_point`] to treat it as "that point" (removing
/// it) rather than adding a near-duplicate one right next to it.
const TEMPO_POINT_SNAP_TICKS: usize = TICKS_PER_BEAT / 2;

/// How much a freshly-added tempo point's bpm steps from whatever's
/// already in effect at its tick — enough to be clearly audible/visible
/// immediately, adjustable afterward the same way (click again nearby to
/// remove, then re-add).
const TEMPO_STEP_BPM: f32 = 10.0;

/// The timeline's Tempo tool's click-to-toggle interaction
/// (`timeline::on_timeline_click_tempo`): removes the closest existing
/// tempo-change point within [`TEMPO_POINT_SNAP_TICKS`] of `tick`, or adds
/// a new one at `tick` (bpm = [`bpm_at`] plus [`TEMPO_STEP_BPM`]) if none is
/// that close. A tick at or near 0 — already controlled by the opening
/// tempo's `Field::Tempo` box — is a no-op rather than adding a point
/// [`build_tempo_map`] would just discard as a tick-0 collision.
pub(super) fn toggle_tempo_point(state: &mut EditorState, tick: usize) {
    if tick < TEMPO_POINT_SNAP_TICKS {
        return;
    }
    let nearest = state
        .tempo_changes
        .iter()
        .enumerate()
        .filter(|&(_, &(t, _))| t.abs_diff(tick) <= TEMPO_POINT_SNAP_TICKS)
        .min_by_key(|&(_, &(t, _))| t.abs_diff(tick));
    if let Some((idx, _)) = nearest {
        state.tempo_changes.remove(idx);
        return;
    }
    let bpm = bpm_at(&state.tempo_map(), tick) + TEMPO_STEP_BPM;
    state.tempo_changes.push((tick, bpm));
}

// ── Timeline erase/remove ────────────────────────────────────────────────────

/// One past the last tick any note currently occupies — the right-hand
/// bound for a "from the split point to the end of the song" range. `0` for
/// an empty song.
pub(super) fn song_end_tick(notes: &[GridNote]) -> usize {
    notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0)
}

/// Orders a possibly-backwards drag span into `(start, end)` with
/// `start <= end`.
pub(super) fn normalize_range(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

// ── Silence track ────────────────────────────────────────────────────────────

/// The tick ranges strictly *between* two sounding notes — merging every
/// note's `[tick, tick+len)` interval across all holes first, since silence
/// means nothing at all is sounding, not just one particular hole (two holes
/// overlapping as a chord, or one note's tail overlapping the next note's
/// onset, must not read as a gap). Leading silence (before the first note)
/// and trailing silence (after the last) are deliberately excluded — the
/// silence track shows the space *between* notes, not lead-in/lead-out.
pub(super) fn silence_gaps(notes: &[GridNote]) -> Vec<(usize, usize)> {
    let mut intervals: Vec<(usize, usize)> = notes.iter().map(|n| (n.tick, n.tick + n.len)).collect();
    intervals.sort_by_key(|&(start, _)| start);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in intervals {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged.windows(2).map(|w| (w[0].1, w[1].0)).collect()
}

/// The whole-side range a split point resolves to once the user clicks the
/// highlighted side: from the start of the song up to `split` (`Side::Left`,
/// the pointer was hovering left of the split), or from `split` to the end
/// of the song (`Side::Right`).
pub(super) fn split_side_range(split: usize, side: Side, notes: &[GridNote]) -> (usize, usize) {
    match side {
        Side::Left => (0, split),
        Side::Right => (split, song_end_tick(notes).max(split)),
    }
}

/// Whether a note spanning `[tick, tick+len)` overlaps `[start, end)`.
fn range_overlaps(tick: usize, len: usize, start: usize, end: usize) -> bool {
    tick < end && start < tick + len
}

/// Deletes every note overlapping `[start, end)`, leaving every other note
/// exactly where it is — the song's own length is unaffected, just a gap
/// where those notes used to be. The **Erase** tool.
pub(super) fn erase_range(notes: &[GridNote], start: usize, end: usize) -> Vec<GridNote> {
    notes
        .iter()
        .copied()
        .filter(|n| !range_overlaps(n.tick, n.len, start, end))
        .collect()
}

/// Deletes every note overlapping `[start, end)`, *and* shifts every note
/// that starts at or after `end` earlier by `end - start` ticks, closing the
/// gap — the song gets shorter. The **Remove** tool.
pub(super) fn remove_range(notes: &[GridNote], start: usize, end: usize) -> Vec<GridNote> {
    let span = end.saturating_sub(start);
    notes
        .iter()
        .copied()
        .filter(|n| !range_overlaps(n.tick, n.len, start, end))
        .map(|mut n| {
            if n.tick >= end {
                n.tick -= span;
            }
            n
        })
        .collect()
}
