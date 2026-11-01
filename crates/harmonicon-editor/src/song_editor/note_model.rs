// SPDX-License-Identifier: MIT

//! The editor's note vocabulary: what a placed note is ([`GridNote`],
//! [`HarpTechnique`], [`Dir`]), what the editor is currently doing to one
//! ([`DragState`], [`Edge`]), and the timeline range tools
//! ([`TimelineTool`], [`TimelineDrag`]).
//!

use bevy::prelude::*;
use harmonicon_core::chart::Action;
use harmonicon_core::pitch_map::Technique;

// ── Note model types ─────────────────────────────────────────────────────────

/// A note's pitch technique. Bends, overblows, and overdraws apply to
/// diatonic kinds; the slide applies to chromatic kinds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum HarpTechnique {
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
/// time; either may combine with any [`HarpTechnique`]. Both carry their oscillation
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
    pub(super) fn action(self) -> Action {
        match self {
            Dir::Blow => Action::Blow,
            Dir::Draw => Action::Draw,
        }
    }

    pub(super) fn from_action(action: Action) -> Self {
        match action {
            Action::Blow => Dir::Blow,
            Action::Draw => Dir::Draw,
        }
    }

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

/// One timed musical instruction, independent of grid rows. `target` owns
/// its pitch/placement intent; `id` survives timeline and visual edits.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct GridNote {
    pub(super) id: u32,
    pub(super) tick: usize,
    pub(super) len: usize,
    pub(super) expr: Expr,
    pub(super) target: harmonicon_core::note_target::NoteTarget,
}

/// The flute's highest note, C7: the grid's top row on a flute.
pub(super) const FLUTE_TOP_NOTE: u8 = 96;

impl GridNote {
    /// The grid row the note sits on, counted from 1 at the top: a
    /// harmonica's hole, a guitar's string, a flute's pitch counted down
    /// from C7 ([`FLUTE_TOP_NOTE`]). Everything that lays notes out
    /// or compares where they sit (drawing, dragging, overlap, paste) asks
    /// this, never the harmonica's own hole.
    pub(super) fn row(&self) -> u8 {
        match self.target.position() {
            Some(harmonicon_core::instrument::PlayPosition::Hole(a)) => a.hole,
            Some(harmonicon_core::instrument::PlayPosition::Fret(a)) => a.string,
            Some(harmonicon_core::instrument::PlayPosition::Fingering(a)) => {
                FLUTE_TOP_NOTE.saturating_sub(a.note) + 1
            }
            None => 1,
        }
    }

    /// Moves the note to `row`, keeping the rest of its position: a hole's
    /// breath and technique, a string's fret and technique. A tab moves the
    /// same way on either instrument.
    pub(super) fn set_row(&mut self, row: u8) {
        use harmonicon_core::instrument::PlayPosition;
        use harmonicon_core::note_target::NoteTarget;
        let moved = match self.target.position() {
            Some(PlayPosition::Hole(mut a)) => {
                a.hole = row;
                PlayPosition::Hole(a)
            }
            Some(PlayPosition::Fret(mut a)) => {
                a.string = row;
                PlayPosition::Fret(a)
            }
            // A flute row is a pitch, and an alternate fingering belongs
            // to one note, so the moved note takes its new one's standard.
            Some(PlayPosition::Fingering(_)) => {
                PlayPosition::Fingering(harmonicon_core::instrument::flute::FingeringAssignment {
                    note: FLUTE_TOP_NOTE + 1 - row,
                    alternate: 0,
                })
            }
            None => return,
        };
        self.target = NoteTarget::Positioned(moved);
    }
}

#[cfg(test)]
impl GridNote {
    /// A C-style harmonica tab instruction for existing view tests.
    pub(super) fn plain(id: u32, hole: u8, tick: usize, len: usize) -> Self {
        Self {
            id,
            tick,
            len,
            expr: Expr::None,
            target: GridNote::harmonica_target(hole, Dir::Blow, HarpTechnique::Normal),
        }
    }
}

impl HarpTechnique {
    /// The editor's technique in core's vocabulary.
    pub(super) fn technique(self) -> Technique {
        match self {
            HarpTechnique::Normal => Technique::Natural,
            HarpTechnique::Bend(depth) => Technique::Bend(depth),
            HarpTechnique::Overblow => Technique::Overblow,
            HarpTechnique::Overdraw => Technique::Overdraw,
            HarpTechnique::Slide => Technique::Slide,
        }
    }

    pub(super) fn from_technique(technique: Technique) -> Self {
        match technique {
            Technique::Natural => HarpTechnique::Normal,
            Technique::Bend(depth) => HarpTechnique::Bend(depth),
            Technique::Overblow => HarpTechnique::Overblow,
            Technique::Overdraw => HarpTechnique::Overdraw,
            Technique::Slide => HarpTechnique::Slide,
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
            start_hole: note.row(),
            target_hole: note.row(),
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
