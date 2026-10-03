// SPDX-License-Identifier: MIT

//! The editor's note vocabulary: what a placed note is ([`GridNote`],
//! [`Pitch`], [`Dir`]), what the editor is currently doing to one
//! ([`DragState`], [`Edge`]), and the timeline range tools
//! ([`TimelineTool`], [`TimelineDrag`]).
//!

use bevy::prelude::*;

// ── Note model types ─────────────────────────────────────────────────────────

/// A note's pitch technique. Bends, overblows, and overdraws apply to
/// diatonic kinds; the slide applies to chromatic kinds.
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
/// button on either 12 or 16 holes — see [`EditorState::hole_count`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum HarmonicaKind {
    #[default]
    Diatonic,
    CountryTuned,
    PaddyRichter,
    NaturalMinor,
    Chromatic,
    Chromatic16,
}

impl HarmonicaKind {
    pub(super) fn is_diatonic(self) -> bool {
        matches!(
            self,
            Self::Diatonic | Self::CountryTuned | Self::PaddyRichter | Self::NaturalMinor
        )
    }

    pub(super) fn is_chromatic(self) -> bool {
        matches!(self, Self::Chromatic | Self::Chromatic16)
    }
}

/// An expression technique layered on top of the pitch. At most one at a
/// time; either may combine with any [`Pitch`]. Both carry their oscillation
/// rate in Hz, cycled through by repeatedly clicking the mod button.
/// The shared type lives in `harmonicon_core::synth`.
pub(super) use harmonicon_core::synth::Expr;

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
    /// Live-recording transport (Play/Pause/Stop/Finish) — notes come from
    /// the microphone, so the grid is locked like [`Mode::Play`].
    Record,
    /// Playback/practice transport.
    Play,
    /// Dev-only benchmark-authoring mode — see `expected_notes`'s docs.
    /// Grid clicks place/select notes on `expected_notes`, not `notes`.
    ExpectedNotes,
}

/// What kind of content the editor is authoring — toggled by the "Record
/// Song"/"Record Lesson" button next to the harmonica-kind one. `Song`
/// saves/loads a plain `.harpchart`. `Lesson` shows the extra
/// `LESSON_FIELDS` panel (`lesson_form::spawn_lesson_form`) and saves/loads
/// a `lesson.json` instead (see `lesson_form::serialize_lesson`), written
/// alongside its own `.harpchart` at `song/chart.harpchart` — same layout as
/// a shipped lesson. Doesn't affect note editing, playback, or the grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum ContentKind {
    #[default]
    Song,
    Lesson,
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
    /// Click-to-cycle a meter change at the clicked beat — the Tempo
    /// tool's sibling for `meter_changes`. A click where no change exists adds
    /// one with the next signature in `TIME_SIGNATURES` after the one in
    /// force there; a click on an existing change steps it to the next
    /// signature, and stepping it round to the meter it changed *from*
    /// removes it, since that would be a change to nothing. One gesture,
    /// no modifier, no confirm dialog. See `timeline::
    /// on_timeline_click_meter`/`cycle_meter_point`.
    Meter,
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

/// Optional musical labels anchored to the first chart item at a grid tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PhraseAnnotation {
    pub(super) section: Option<String>,
    pub(super) chord: Option<String>,
    pub(super) groove: Option<String>,
    /// The syllable sung on this phrase's notes (chart `lyric`).
    pub(super) lyric: Option<String>,
    pub(super) call: bool,
    pub(super) split: bool,
}

/// An in-progress press-drag gesture on the timeline ruler: `start` is
/// fixed at the press position, `end` follows the pointer. Not normalized —
/// `end` can be less than `start` — see [`normalize_range`]. Mirrors
/// [`DragState`]'s role for note dragging: set by `PointerDragStart`,
/// live-updated by `PointerDrag`; `PointerDragEnd` then either keeps it
/// as the Select tool's persisted selection (an `end` that genuinely moved
/// past `start`), or — since `bevy_picking` fires `DragStart` on any
/// nonzero pixel motion, so ordinary click jitter routinely produces one —
/// falls back to treating a same-tick `start`/`end` as the click it was
/// meant to be, against [`EditorState::timeline_split`]. Deliberately not
/// driven by `PointerClick`: `Click` and `DragEnd` both fire on the same
/// release, `Click` first, so routing every decision through `Drag*` alone
/// avoids that race instead of coordinating two competing handlers.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct TimelineDrag {
    pub(super) start: usize,
    pub(super) end: usize,
    /// [`Scroll::px`] at the moment the drag started. `PointerDrag` only
    /// reports pointer motion, but the grid can keep scrolling *under* a
    /// held drag (wheel pan), so the span's moving end is pointer motion
    /// *plus* scroll delta since the press (`timeline::drag_end_tick`) —
    /// this lets a mid-drag wheel pan extend the selection over newly
    /// revealed area instead of pinning it to the press-time content.
    pub(super) scroll_px: f32,
    /// Accumulated pointer motion since the press (already ÷ UI scale) —
    /// the last `PointerDrag::distance.x` seen. Lets a wheel-scroll frame
    /// with a stationary pointer (no `Drag` event fires then) still
    /// recompute `end` — see `timeline::sync_selection_with_scroll`.
    pub(super) pointer_px: f32,
    /// True while the button is still held. A released Select span stays in
    /// [`TimelineSelection`] as the persisted selection but stops tracking
    /// scroll — only a *live* gesture follows the grid panning under it.
    pub(super) live: bool,
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

#[cfg(test)]
impl GridNote {
    /// A plain blow note, the shape most tests start from; set the rest with
    /// struct-update syntax (`GridNote { dir: Dir::Draw, ..GridNote::plain(…) }`).
    pub(super) fn plain(id: u32, hole: u8, tick: usize, len: usize) -> Self {
        Self { id, hole, tick, len, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None }
    }
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

#[derive(Clone)]
pub(super) struct DragState {
    pub(super) id: u32,
    pub(super) kind: DragKind,
    pub(super) start_tick: usize,
    pub(super) start_len: usize,
    pub(super) start_hole: u8,
    pub(super) target_hole: u8,
    pub(super) target_tick: usize,
    pub(super) valid: bool,
    /// Every *other* note moving together with the anchor (`id`) — its
    /// original `(hole, tick)` at drag start, carried alongside so
    /// `grid::group_move_targets` can shift the whole group by the same
    /// delta the anchor moved by. Populated only when the dragged note was part
    /// of a multi-selection (`EditorState::selected`) larger than one at
    /// drag start; empty for an ordinary single-note move and always empty
    /// for `DragKind::Resize` (resizing only ever affects the one handle
    /// being dragged, group or no group).
    pub(super) group: Vec<GridNote>,
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
            group: Vec::new(),
        }
    }

    /// Like [`DragState::new`], but for a multi-note move: `group` is every
    /// other selected note (the anchor `note` itself is excluded — it's
    /// already tracked via `id`/`start_hole`/`start_tick`).
    pub(super) fn new_group(id: u32, note: &GridNote, group: Vec<GridNote>) -> Self {
        Self { group, ..Self::new(id, DragKind::Move, note) }
    }
}
