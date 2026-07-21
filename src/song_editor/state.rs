// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::{HEADER_H, NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};
use crate::song::chart::Scale;

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
    /// Live-recording transport (Play/Pause/Stop/Finish) — notes come from
    /// the microphone, so the grid is locked like [`Mode::Play`].
    Record,
    /// Playback/practice transport.
    Play,
}

/// What kind of content the editor is authoring — toggled by the "Record
/// Song"/"Record Lesson" button next to the harmonica-kind one. `Song`
/// (the original, only behaviour) saves/loads a plain `.harpchart`, same as
/// always. `Lesson` shows the extra `LESSON_FIELDS` panel
/// (`lesson_form::spawn_lesson_form`) and saves/loads a `lesson.json`
/// instead — see `lesson_form::serialize_lesson`. Doesn't affect anything
/// about how notes are edited; the grid/mod-panel/playback all work exactly
/// the same regardless, since a chart-backed lesson's chart *is* an ordinary
/// `.harpchart` (written alongside the `lesson.json`, at `song/
/// chart.harpchart` relative to it, exactly like a shipped lesson's own
/// folder layout).
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
/// live-updated by `Pointer<Drag>`; `Pointer<DragEnd>` then either keeps
/// it as the Select tool's persisted selection (an `end` that genuinely
/// moved past `start`) or, since `bevy_picking` fires
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
    /// [`Scroll::px`] at the moment the drag started. `Pointer<Drag>` only
    /// reports pointer motion, but the grid can keep scrolling *under* a
    /// held drag (wheel pan) — the span's moving end is pointer motion
    /// *plus* however far the content scrolled since the press
    /// (`timeline::drag_end_tick`), so scrolling mid-drag extends the
    /// selection over the newly revealed area instead of silently pinning
    /// it to wherever the content sat at press time.
    pub(super) scroll_px: f32,
    /// Accumulated pointer motion since the press, already divided by the
    /// UI scale — the last `Pointer<Drag>::distance.x` seen. Kept so a
    /// wheel-scroll frame with a *stationary* pointer (no `Drag` event
    /// fires at all then) can still recompute `end` from the new scroll
    /// position — see `timeline::sync_selection_with_scroll`.
    pub(super) pointer_px: f32,
    /// True while the button is still held (press → release). A released
    /// Select span stays in [`TimelineSelection`] as the persisted
    /// selection, but must stop tracking scroll — only a *live* gesture
    /// follows the grid panning under it.
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
        Self {
            group,
            ..Self::new(id, DragKind::Move, note)
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
    /// Stable lesson identifier — the profile key/prerequisite target. Never
    /// rename one that's shipped; see `lesson_schema.dtd.json`.
    LessonId,
    /// Curriculum unit grouping this lesson in the menu (`lesson-unit-
    /// <unit>` is its own separate Fluent key, not authored here).
    LessonUnit,
    /// Raw display text for the lesson's instructional body — `Name` above
    /// doubles as the lesson's title text the same way. Neither is written
    /// into `lesson.json` directly (which only stores Fluent *keys*, per
    /// this codebase's localization convention); `lesson_form::
    /// serialize_lesson` derives `title_key`/`body_key` from `LessonId` and
    /// prints the key/text pairs an author still needs to add to the
    /// locale files by hand — the same manual step authoring any bundled
    /// lesson already requires.
    LessonExplanation,
    /// Comma-separated lesson ids that must be passed first.
    LessonPrerequisites,
    /// One of [`PASS_CRITERIA_KINDS`] — click-to-cycle, like `Key`/
    /// `Position`, not a free-text field a player types into.
    LessonPassCriteria,
    /// The active pass criterion's threshold (0..1), as typed text —
    /// ignored when `LessonPassCriteria` is `"none"`.
    LessonThreshold,
    /// One of [`TECHNIQUE_NAMES`] — only meaningful (and only written) when
    /// `LessonPassCriteria` is `"technique"`; click-to-cycle like `Key`.
    LessonTechnique,
    /// One of [`PROGRESSIONS`] (`"none"` omits the field) — click-to-cycle
    /// like `Key`.
    LessonProgression,
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

/// The extra rows `lesson_form::spawn_lesson_form` shows only while
/// [`ContentKind::Lesson`] is active — everything `lesson_schema.dtd.json`
/// needs beyond what [`FIELDS`] already covers (title/tempo/key/... are
/// shared with a plain song, since a chart-backed lesson's chart is an
/// ordinary chart).
pub(super) const LESSON_FIELDS: [(Field, &str); 8] = [
    (Field::LessonId, "editor-field-lesson-id"),
    (Field::LessonUnit, "editor-field-lesson-unit"),
    (Field::LessonExplanation, "editor-field-lesson-explanation"),
    (
        Field::LessonPrerequisites,
        "editor-field-lesson-prerequisites",
    ),
    (
        Field::LessonPassCriteria,
        "editor-field-lesson-pass-criteria",
    ),
    (Field::LessonThreshold, "editor-field-lesson-threshold"),
    (Field::LessonTechnique, "editor-field-lesson-technique"),
    (Field::LessonProgression, "editor-field-lesson-progression"),
];

/// All valid diatonic harp keys in chromatic order.
pub(super) const HARP_KEYS: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// Playing positions in the order harmonica players commonly reach for them:
/// 1st (straight), 2nd (cross harp, the blues staple), 3rd through 5th, and
/// 12th, which jazz players use for its major-scale-friendly hole layout.
pub(super) const POSITIONS: [&str; 6] = ["1st", "2nd", "3rd", "4th", "5th", "12th"];

/// `Field::LessonPassCriteria`'s cycle — `"none"` (finishing counts as done)
/// plus the five `pass_criteria.type` values `lesson_schema.dtd.json` allows.
pub(super) const PASS_CRITERIA_KINDS: [&str; 6] = [
    "none",
    "accuracy",
    "technique",
    "scale-adherence",
    "chord-tone-adherence",
    "phrase-discipline",
];

/// `Field::LessonTechnique`'s cycle — the same technique-bucket vocabulary
/// `SongStats`/`PlayerProfile::technique_best_accuracy` use, pinned by
/// `lesson_schema.dtd.json`'s own enum.
pub(super) const TECHNIQUE_NAMES: [&str; 8] = [
    "normal",
    "bend",
    "vibrato",
    "wah-wah",
    "overblow",
    "overdraw",
    "slide",
    "clean-attack",
];

/// `Field::LessonProgression`'s cycle — `"none"` omits the manifest field
/// entirely (defaults to Standard in-game); the rest are
/// `lesson_schema.dtd.json`'s own enum.
pub(super) const PROGRESSIONS: [&str; 4] = ["none", "standard", "quick-change", "minor"];

/// Advances `current` to the next entry in `options`, wrapping — every
/// click-to-cycle metadata field (`Key`, `Position`, and the lesson-only
/// pass-criteria kind/technique/progression fields) steps through its own
/// fixed vocabulary this way rather than accepting free text.
pub(super) fn cycle_next(options: &[&str], current: &str) -> String {
    let idx = options.iter().position(|&o| o == current).unwrap_or(0);
    options[(idx + 1) % options.len()].to_string()
}

// ── Resources ────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub(super) struct EditorState {
    pub(super) notes: Vec<GridNote>,
    pub(super) next_id: u32,
    /// Every currently-selected note id, in the order each was added to the
    /// selection — empty means nothing selected. A plain click replaces the
    /// whole selection with one id ([`EditorState::select_only`]); a
    /// Ctrl+click toggles one id in or out ([`EditorState::
    /// toggle_selected`]) without disturbing the rest, which is what lets
    /// multiple notes be selected at once. [`EditorState::selected_note`]/
    /// [`EditorState::selected_note_mut`] — the mod panel's "the selected
    /// note's own fields" source — read the *last* entry as the "primary"
    /// note technique edits (Bend, Overblow, ...) apply to; Move and Delete
    /// are the two operations that act on the whole set instead of just the
    /// primary (see `interaction::delete_selected` and the note drag
    /// observers in `grid.rs`).
    pub(super) selected: Vec<u32>,
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
    /// Which scale the grid colors notes against — see [`Scale`]. A
    /// picker-only field (the Scale combobox), unlike `key`/`position`,
    /// which route through the generic [`Field`]/[`FIELDS`] click-to-cycle
    /// machinery — six named options is a lot for a cycle button, and the
    /// combobox shows all of them at once.
    pub(super) scale: Scale,
    pub(super) music: String,
    pub(super) name: String,
    pub(super) author: String,
    pub(super) focus: Option<Field>,
    pub(super) drag_msg: crate::localization::LocalizedStr,
    pub(super) mode: Mode,
    /// Whether this editing session is authoring a song or a lesson — see
    /// [`ContentKind`].
    pub(super) content_kind: ContentKind,
    pub(super) lesson_id: String,
    pub(super) lesson_unit: String,
    pub(super) lesson_explanation: String,
    pub(super) lesson_prerequisites: String,
    pub(super) lesson_pass_criteria: String,
    pub(super) lesson_threshold: String,
    pub(super) lesson_technique: String,
    pub(super) lesson_progression: String,
    /// Whether the lesson-fields panel's body (the two field columns) is
    /// expanded — folded by default, since lesson metadata is filled in
    /// occasionally, not every session, and shouldn't compete with the note
    /// grid for screen space by default. See `lesson_form::spawn_lesson_form`.
    pub(super) lesson_details_expanded: bool,
    /// User's own Lock toggle, independent of `mode`. See [`EditorState::locked`].
    pub(super) user_locked: bool,
    pub(super) harmonica_kind: HarmonicaKind,
    pub(super) timeline_tool: TimelineTool,
    /// A split point placed by a plain click-and-release on the timeline
    /// ruler (no meaningful drag) — persists across frames (unlike
    /// `timeline_drag`, which only lives for one gesture) until a second
    /// such click picks a side and consumes it, or the tool is switched.
    pub(super) timeline_split: Option<usize>,
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
            selected: Vec::new(),
            scroll_beat: 0,
            dragging: None,
            tempo: "120".into(),
            tempo_changes: Vec::new(),
            key: "C".into(),
            position: "2nd".into(),
            scale: Scale::default(),
            music: String::new(),
            name: String::new(),
            author: String::new(),
            focus: None,
            drag_msg: crate::localization::LocalizedStr::default(),
            mode: Mode::default(),
            content_kind: ContentKind::default(),
            lesson_id: String::new(),
            lesson_unit: String::new(),
            lesson_explanation: String::new(),
            lesson_prerequisites: String::new(),
            lesson_pass_criteria: "none".into(),
            lesson_threshold: "0.7".into(),
            lesson_technique: "normal".into(),
            lesson_progression: "none".into(),
            lesson_details_expanded: false,
            user_locked: false,
            harmonica_kind: HarmonicaKind::default(),
            timeline_tool: TimelineTool::default(),
            timeline_split: None,
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

    /// The "primary" selected note — the most recently added to the
    /// selection — whose own fields the mod panel reflects/edits. `None`
    /// with nothing selected.
    pub(super) fn selected_note(&self) -> Option<&GridNote> {
        self.selected.last().and_then(|&id| self.note_by_id(id))
    }

    pub(super) fn selected_note_mut(&mut self) -> Option<&mut GridNote> {
        let id = *self.selected.last()?;
        self.notes.iter_mut().find(|n| n.id == id)
    }

    pub(super) fn is_selected(&self, id: u32) -> bool {
        self.selected.contains(&id)
    }

    /// Replaces the whole selection with just `id` — a plain (non-Ctrl)
    /// click on a note.
    pub(super) fn select_only(&mut self, id: u32) {
        self.selected.clear();
        self.selected.push(id);
    }

    /// Adds `id` to the selection, or removes it if already present —
    /// a Ctrl+click on a note, which extends/shrinks the selection without
    /// disturbing the rest of it.
    pub(super) fn toggle_selected(&mut self, id: u32) {
        if let Some(pos) = self.selected.iter().position(|&x| x == id) {
            self.selected.remove(pos);
        } else {
            self.selected.push(id);
        }
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
            Field::LessonId => &self.lesson_id,
            Field::LessonUnit => &self.lesson_unit,
            Field::LessonExplanation => &self.lesson_explanation,
            Field::LessonPrerequisites => &self.lesson_prerequisites,
            Field::LessonPassCriteria => &self.lesson_pass_criteria,
            Field::LessonThreshold => &self.lesson_threshold,
            Field::LessonTechnique => &self.lesson_technique,
            Field::LessonProgression => &self.lesson_progression,
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
            Field::LessonId => &mut self.lesson_id,
            Field::LessonUnit => &mut self.lesson_unit,
            Field::LessonExplanation => &mut self.lesson_explanation,
            Field::LessonPrerequisites => &mut self.lesson_prerequisites,
            Field::LessonPassCriteria => &mut self.lesson_pass_criteria,
            Field::LessonThreshold => &mut self.lesson_threshold,
            Field::LessonTechnique => &mut self.lesson_technique,
            Field::LessonProgression => &mut self.lesson_progression,
        }
    }

    /// True when notes cannot be added, moved, or resized: either the user
    /// turned Lock on themselves, or `mode` is `Perform` (which is always
    /// locked, regardless of the user's own toggle).
    pub(super) fn locked(&self) -> bool {
        self.user_locked || self.mode != Mode::Edit
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
        let notes = &self.notes;
        self.selected.retain(|id| notes.iter().any(|n| n.id == *id));
    }
}

/// Continuous horizontal scroll in pixels. Kept separate from [`EditorState`]
/// so scrolling doesn't trigger a grid rebuild.
#[derive(Resource, Default)]
pub(super) struct Scroll {
    pub(super) px: f32,
}

/// The timeline Select tool's span: the in-progress drag gesture while the
/// button is held, and — once released with a real extent — the persisted
/// selection the Erase/Remove buttons act on. Its own resource rather than
/// an `EditorState` field for the same reason [`Scroll`] is: the span
/// updates on every pointer move during a drag, and routing that through
/// `EditorState` would either rebuild the whole grid every one of those
/// frames or (the old guard against exactly that) suppress the
/// scroll-driven rebuilds a mid-drag wheel pan needs — notes scrolled into
/// view during a selection were never spawned, so only what was visible at
/// press time could be selected.
#[derive(Resource, Default)]
pub(super) struct TimelineSelection {
    pub(super) drag: Option<TimelineDrag>,
}

// ── Note model logic ─────────────────────────────────────────────────────────

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

// `group_move_targets`/`group_move_valid` (the multi-select group-drag
// pure functions) live in `grid.rs`, next to the note-drag observers that
// are their only callers — split out to stay under the file-size budget.

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

/// Every note transitively overlapping `id` in time (including `id` itself):
/// starting from `id`, repeatedly pulls in any note overlapping one already
/// in the group until nothing new joins — the shared traversal
/// `enforce_direction`/`enforce_expr` each build on, since a change to one
/// note in a stack of simultaneous notes must propagate to every note that
/// note's stack overlaps in turn, not just its immediate neighbors.
fn overlapping_group(state: &EditorState, id: u32) -> Vec<u32> {
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
    group
}

pub(super) fn enforce_direction(state: &mut EditorState, id: u32) {
    let Some(dir) = state.note_by_id(id).map(|n| n.dir) else {
        return;
    };
    let group = overlapping_group(state, id);
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
    let group = overlapping_group(state, id);
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
pub(super) fn build_tempo_map(
    tempo: &str,
    tempo_changes: &[(usize, f32)],
) -> Vec<crate::song::chart::TempoPoint> {
    use crate::song::chart::TempoPoint;
    let bpm0: f32 = tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
    let mut map = vec![TempoPoint { tick: 0, bpm: bpm0 }];
    map.extend(tempo_changes.iter().map(|&(tick, bpm)| TempoPoint {
        tick: tick as u64,
        bpm: bpm.max(1.0),
    }));
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

// Timeline erase/remove and silence-track range logic — `song_end_tick`,
// `normalize_range`, `silence_gaps`, `split_side_range`, `erase_range`,
// `remove_range` — live in `ranges.rs` (split out to stay under the
// file-size budget; none of them touch `EditorState` itself).
