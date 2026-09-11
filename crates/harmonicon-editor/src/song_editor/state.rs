// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::snap::SnapMode;
use super::{HEADER_H, NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};
use harmonicon_core::chart::Scale;

// The note vocabulary lives in `note_model`; re-exported here because
// `state` is the name every call site already reaches for, and the two are
// one concept split only for file size.
pub(super) use super::note_model::{
    ContentKind, Dir, DragKind, DragState, Edge, GridNote, HarmonicaKind, Mode, Pitch, Side,
    TimelineDrag, TimelineTool,
};
pub(super) use harmonicon_core::synth::Expr;

// ── Metadata field types ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Field {
    Tempo,
    /// The chart's meter, typed as `"4/4"`. Free text rather than a cycle
    /// button because the useful set is open-ended (4/4, 3/4, 6/8, 5/4,
    /// 7/8, ...); anything unparseable falls back to 4/4 at read time
    /// rather than being rejected, so a half-typed value can't wedge the
    /// grid mid-edit.
    TimeSignature,
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
    /// Whether this lesson belongs to the required core or an elective branch.
    LessonPath,
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
    /// Jam scale written to the lesson manifest; `"none"` omits it.
    LessonScale,
}

impl Field {
    /// True for the five fields that cycle through a fixed set of values on
    /// click (`Key`, `Position`, the three lesson-enum pickers) rather than
    /// accepting typed text. `meta_form::spawn_field_row` branches on this
    /// to decide whether to spawn a click-to-cycle button or a real text
    /// input (`dialogs::text_input::spawn_text_input`).
    pub(super) fn is_cycle(self) -> bool {
        matches!(
            self,
            Field::Key
                | Field::Position
                | Field::LessonPassCriteria
                | Field::LessonTechnique
                | Field::LessonProgression
                | Field::LessonScale
                | Field::LessonPath
        )
    }
}

/// Each entry pairs a [`Field`] with the localization key used for its label.
pub(super) const FIELDS: [(Field, &str); 7] = [
    (Field::Tempo, "editor-field-tempo"),
    (Field::TimeSignature, "editor-field-time-signature"),
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
pub(super) const LESSON_FIELDS: [(Field, &str); 10] = [
    (Field::LessonId, "editor-field-lesson-id"),
    (Field::LessonUnit, "editor-field-lesson-unit"),
    (Field::LessonPath, "editor-field-lesson-path"),
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
    (Field::LessonScale, "editor-field-lesson-scale"),
];

/// All valid diatonic harp keys in chromatic order.
pub(super) use harmonicon_core::pitch_map::HARP_KEYS;

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
pub(super) const PROGRESSIONS: [&str; 5] =
    ["none", "standard", "quick-change", "minor", "jazz-blues"];
pub(super) const LESSON_SCALES: [&str; 7] = [
    "none",
    "first-position",
    "second-position",
    "third-position",
    "major",
    "minor-pentatonic",
    "country",
];
pub(super) const LESSON_PATHS: [&str; 2] = ["core", "elective"];

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
    /// Dev-only benchmark ground truth — see `expected_notes`'s docs.
    /// Hand-placed, never collision-checked. Always present rather than
    /// `#[cfg]`-gated, so `EditorState { ..default() }` sites don't need touching.
    #[cfg_attr(not(feature = "dev"), allow(dead_code))]
    pub(super) expected_notes: Vec<GridNote>,
    #[cfg_attr(not(feature = "dev"), allow(dead_code))]
    pub(super) expected_next_id: u32,
    /// A single `Option`, not a `Vec`: this layer only ever needs one
    /// "primary" note for the mod-panel buttons to edit.
    #[cfg_attr(not(feature = "dev"), allow(dead_code))]
    pub(super) expected_selected: Option<u32>,
    /// Separate from `dragging`: `notes`/`expected_notes` are independent id
    /// spaces, so reusing `dragging` risks one layer's drag updating the
    /// other's note (`live_resize`/`update_move_ghost` match purely by id).
    #[cfg_attr(not(feature = "dev"), allow(dead_code))]
    pub(super) expected_dragging: Option<DragState>,
    /// Currently-selected note ids, in add order — empty means nothing
    /// selected. A plain click replaces the whole selection
    /// ([`EditorState::select_only`]); Ctrl+click toggles one id without
    /// disturbing the rest ([`EditorState::toggle_selected`]), enabling
    /// multi-select. [`EditorState::selected_note`]/`_mut` (the mod panel's
    /// field source) read the *last* entry as the "primary" note technique
    /// edits apply to; Move and Delete act on the whole set instead (see
    /// `interaction::delete_selected` and the drag observers in `grid.rs`).
    pub(super) selected: Vec<u32>,
    pub(super) scroll_beat: usize,
    pub(super) dragging: Option<DragState>,
    pub(super) tempo: String,
    /// Tempo changes after the song's opening tempo (`tempo`, tick 0) —
    /// `(tick, bpm)` pairs in the editor's own tick unit, added via the
    /// timeline's Tempo tool (`timeline::tempo_tool_click`). Not
    /// necessarily sorted as edits land; [`EditorState::tempo_map`] sorts
    /// on read. Empty for the common single-tempo case.
    pub(super) tempo_changes: Vec<(usize, f32)>,
    /// The song's meter, as it will be written to the chart (`"4/4"`,
    /// `"3/4"`, `"6/8"`). Everything that needs a bar length asks
    /// [`EditorState::beats_per_bar`] rather than assuming four — the grid's
    /// bar lines, the metronome's count-in, the timeline's bar readout and
    /// the notation staff all follow it.
    pub(super) time_signature: String,
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
    pub(super) drag_msg: harmonicon_platform::localization::LocalizedStr,
    pub(super) mode: Mode,
    /// Whether this editing session is authoring a song or a lesson — see
    /// [`ContentKind`].
    pub(super) content_kind: ContentKind,
    pub(super) lesson_id: String,
    pub(super) lesson_unit: String,
    pub(super) lesson_path: String,
    pub(super) lesson_explanation: String,
    pub(super) lesson_prerequisites: String,
    pub(super) lesson_pass_criteria: String,
    pub(super) lesson_threshold: String,
    pub(super) lesson_technique: String,
    pub(super) lesson_progression: String,
    pub(super) lesson_scale: String,
    /// Whether the lesson-fields panel's body is expanded — folded by
    /// default so it doesn't compete with the note grid for screen space.
    /// See `lesson_form::spawn_lesson_form`.
    pub(super) lesson_details_expanded: bool,
    /// Whether the meta form's third column (`meta_form::spawn_color_legend`)
    /// is shown — toggled by the mod panel's "ℹ Legend" button
    /// (`mod_panel.rs`). Visible by default, same as before this toggle
    /// existed.
    pub(super) legend_visible: bool,
    /// User's own Lock toggle, independent of `mode`. See [`EditorState::locked`].
    pub(super) user_locked: bool,
    pub(super) harmonica_kind: HarmonicaKind,
    /// Which within-beat tick positions a click places a new note at — see
    /// [`SnapMode`]. A UI preference, not chart content or undo-tracked.
    pub(super) snap_mode: SnapMode,
    pub(super) timeline_tool: TimelineTool,
    /// A split point placed by a plain click-and-release on the timeline
    /// ruler — persists across frames (unlike `timeline_drag`, which only
    /// lives for one gesture) until a second click picks a side and
    /// consumes it, or the tool is switched.
    pub(super) timeline_split: Option<usize>,
    /// A range the user has committed to (a placed split's side, or a
    /// released drag span) and is waiting on the confirm dialog's answer
    /// for. Set right before opening the dialog; read and cleared once
    /// `ConfirmChosen` arrives — see `timeline::handle_timeline_confirm`.
    pub(super) pending_timeline_op: Option<(TimelineTool, usize, usize)>,

    /// The mod buttons' persistent "current setting" for notes not yet
    /// placed. Clicking a mod button always updates the relevant one of
    /// these regardless of selection, and it stays armed (see
    /// `interaction::apply_modifier`) until cycled back to "off"
    /// (`Pitch::Normal`/`Expr::None`; `sticky_dir` has no off value, so it
    /// only switches) or sanitized away by `set_harmonica_kind`.
    /// `select_or_add` applies these to every newly placed note, silently
    /// falling back to `Pitch::Normal` for `sticky_pitch` when it doesn't
    /// fit the hole.
    pub(super) sticky_dir: Dir,
    pub(super) sticky_pitch: Pitch,
    pub(super) sticky_expr: Expr,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            next_id: 0,
            expected_notes: Vec::new(),
            expected_next_id: 0,
            expected_selected: None,
            expected_dragging: None,
            selected: Vec::new(),
            scroll_beat: 0,
            dragging: None,
            tempo: "120".into(),
            tempo_changes: Vec::new(),
            time_signature: "4/4".into(),
            key: "C".into(),
            position: "2nd".into(),
            scale: Scale::default(),
            music: String::new(),
            name: String::new(),
            author: String::new(),
            drag_msg: harmonicon_platform::localization::LocalizedStr::default(),
            mode: Mode::default(),
            content_kind: ContentKind::default(),
            lesson_id: String::new(),
            lesson_unit: String::new(),
            lesson_path: "core".into(),
            lesson_explanation: String::new(),
            lesson_prerequisites: String::new(),
            lesson_pass_criteria: "none".into(),
            lesson_threshold: "0.7".into(),
            lesson_technique: "normal".into(),
            lesson_progression: "none".into(),
            lesson_scale: "none".into(),
            lesson_details_expanded: false,
            legend_visible: true,
            user_locked: false,
            harmonica_kind: HarmonicaKind::default(),
            snap_mode: SnapMode::default(),
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
    /// Quarter-note beats per bar, from [`time_signature`]. A 6/8 bar is
    /// three quarter-note beats, not six — the grid, the metronome and the
    /// staff all count in quarters.
    ///
    /// [`time_signature`]: EditorState::time_signature
    pub(super) fn beats_per_bar(&self) -> usize {
        let m = harmonicon_ui::music_score::parse_time_signature(&self.time_signature);
        (m.beats_per_bar().round() as usize).max(1)
    }

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

    // `expected_note_by_id`/`expected_selected_note`/`expected_selected_note_mut`
    // — the expected-notes layer's own siblings of the three methods above —
    // live in `expected_notes.rs` (dev-only) instead of here, in their own
    // `impl EditorState` block: purely a file-size trim (`docs/
    // physical_design_plan.md`'s ~1000-line budget), not a meaningful
    // separation — same struct, just declared in another file, which Rust
    // allows freely for inherent impls.

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
    pub(super) fn tempo_map(&self) -> Vec<harmonicon_core::chart::TempoPoint> {
        build_tempo_map(&self.tempo, &self.tempo_changes)
    }

    pub(super) fn field_text(&self, field: Field) -> &str {
        match field {
            Field::Tempo => &self.tempo,
            Field::TimeSignature => &self.time_signature,
            Field::Key => &self.key,
            Field::Position => &self.position,
            Field::Music => &self.music,
            Field::Name => &self.name,
            Field::Author => &self.author,
            Field::LessonId => &self.lesson_id,
            Field::LessonUnit => &self.lesson_unit,
            Field::LessonPath => &self.lesson_path,
            Field::LessonExplanation => &self.lesson_explanation,
            Field::LessonPrerequisites => &self.lesson_prerequisites,
            Field::LessonPassCriteria => &self.lesson_pass_criteria,
            Field::LessonThreshold => &self.lesson_threshold,
            Field::LessonTechnique => &self.lesson_technique,
            Field::LessonProgression => &self.lesson_progression,
            Field::LessonScale => &self.lesson_scale,
        }
    }

    pub(super) fn field_text_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Tempo => &mut self.tempo,
            Field::TimeSignature => &mut self.time_signature,
            Field::Key => &mut self.key,
            Field::Position => &mut self.position,
            Field::Music => &mut self.music,
            Field::Name => &mut self.name,
            Field::Author => &mut self.author,
            Field::LessonId => &mut self.lesson_id,
            Field::LessonUnit => &mut self.lesson_unit,
            Field::LessonPath => &mut self.lesson_path,
            Field::LessonExplanation => &mut self.lesson_explanation,
            Field::LessonPrerequisites => &mut self.lesson_prerequisites,
            Field::LessonPassCriteria => &mut self.lesson_pass_criteria,
            Field::LessonThreshold => &mut self.lesson_threshold,
            Field::LessonTechnique => &mut self.lesson_technique,
            Field::LessonProgression => &mut self.lesson_progression,
            Field::LessonScale => &mut self.lesson_scale,
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
        self.prune_selection();
    }

    /// Drops selected ids whose note no longer exists. Any path that can
    /// remove a note without going through the selection has to call this:
    /// a harmonica-kind switch, an undo, a recording take punching out what
    /// it overlaps. Otherwise `selected_note` keeps naming a note that
    /// isn't there and the mod panel's edits land on nothing.
    pub(super) fn prune_selection(&mut self) {
        let notes = &self.notes;
        self.selected.retain(|id| notes.iter().any(|n| n.id == *id));
    }
}

/// Vertical scroll of the left tool sidebar, in logical pixels.
///
/// Its own resource rather than a field on [`Scroll`] because the two move
/// for completely unrelated reasons — this one only ever from a drag on the
/// sidebar itself — and change detection on [`Scroll`] already drives grid
/// work that has nothing to do with the toolbar.
#[derive(Resource, Default)]
pub(super) struct ToolbarScroll {
    pub(super) y_px: f32,
}

/// Continuous view offset in pixels. Kept separate from [`EditorState`]
/// so scrolling doesn't trigger a grid rebuild.
#[derive(Resource, Default)]
pub(super) struct Scroll {
    /// Horizontal scroll *within the grid* — applied to `GridContent`, so
    /// the hole column and chrome stay put while the notes move.
    pub(super) px: f32,
    /// Vertical offset of the **whole editor**, applied to `EditorRoot`
    /// itself rather than any inner scroll area.
    ///
    /// It has to be the root, because what overflows on a short screen is
    /// the *fixed chrome* — the grid plus the mod panel — which is
    /// deliberately outside the form's `ScrollArea` (see `ui::setup`). On a
    /// phone the grid alone can exceed the viewport: `grid_height` is
    /// ~424 logical px for a 10-hole harp, against roughly 400 available in
    /// landscape at typical Android DPI, which leaves the mod panel, status
    /// bar and form entirely off-screen and unreachable.
    ///
    /// Only ever moved by [`view_scroll::pan_touch`](super::interaction::
    /// pan_touch); on desktop it stays 0 and this costs nothing.
    pub(super) y_px: f32,
}

/// The timeline Select tool's span: the in-progress drag gesture while the
/// button is held, and — once released with a real extent — the persisted
/// selection the Erase/Remove buttons act on. Its own resource rather than
/// an `EditorState` field for the same reason [`Scroll`] is: the span
/// updates on every pointer move during a drag, and routing that through
/// `EditorState` would either rebuild the whole grid every frame or
/// suppress the scroll-driven rebuilds a mid-drag wheel pan needs.
#[derive(Resource, Default)]
pub(super) struct TimelineSelection {
    pub(super) drag: Option<TimelineDrag>,
}

// ── Note model logic ─────────────────────────────────────────────────────────

pub(super) fn pitch_compatible(pitch: Pitch, hole: u8) -> bool {
    // Delegated so the editor's idea of what a hole can physically do can't
    // drift from the resolver's — `pitch_map` decides which notes are
    // reachable, and this decides which the UI will let you place.
    harmonicon_core::pitch_map::technique_fits_hole(
        match pitch {
            Pitch::Normal => harmonicon_core::pitch_map::Technique::Natural,
            Pitch::Bend(depth) => harmonicon_core::pitch_map::Technique::Bend(depth),
            Pitch::Overblow => harmonicon_core::pitch_map::Technique::Overblow,
            Pitch::Overdraw => harmonicon_core::pitch_map::Technique::Overdraw,
            Pitch::Slide => harmonicon_core::pitch_map::Technique::Slide,
        },
        hole,
    )
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

pub(super) use harmonicon_core::pitch_map::max_bend;

/// Matches `song::harmonica::hole_notes`'s `over` field exactly — holes
/// 1/4/5/6 only, or the pitch resolves to nothing downstream.
pub(super) use harmonicon_core::pitch_map::overblow_ok;

pub(super) use harmonicon_core::pitch_map::overdraw_ok;

/// The breath direction `pitch` physically requires, if any — `Overblow`
/// only exists while *blowing*, `Overdraw` only while *drawing*, regardless
/// of which reed the resulting pitch sits near (`song::harmonica::
/// hole_notes`: overblow sounds a semitone above the *draw* reed, overdraw
/// above the *blow* reed — the technique name is about breath action, not
/// reed). `Bend`/`Slide` have no such constraint. Used to keep a note's
/// `dir` and `pitch` from drifting into a physically impossible pairing as
/// either changes independently.
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
/// `enforce_direction`/`enforce_expr` build on, so a change propagates
/// through a whole stack of simultaneous notes, not just immediate neighbors.
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

/// Wah (hand cupping) and vibrato (breath/diaphragm) are whole-player
/// techniques: whichever you're doing colours every hole sounding at that
/// instant, not just one. So `id`'s `expr` is propagated to every note that
/// overlaps it in time, transitively, same as `enforce_direction` for Blow/Draw.
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
/// tick_to_seconds`/`seconds_to_tick`. Ticks are the editor's own tick unit
/// (`TICKS_PER_BEAT` per beat), which is exactly the `resolution` the
/// editor writes to a saved chart's `timing.resolution`
/// (`harpchart::serialize_harpchart`), so no unit conversion is needed. A
/// duplicate tick silently keeps the earlier-sorted entry rather than erroring.
pub(super) fn build_tempo_map(
    tempo: &str,
    tempo_changes: &[(usize, f32)],
) -> Vec<harmonicon_core::chart::TempoPoint> {
    use harmonicon_core::chart::TempoPoint;
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
fn bpm_at(tempo_map: &[harmonicon_core::chart::TempoPoint], tick: usize) -> f32 {
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
/// that close. A tick at or near 0 is a no-op — that's already controlled
/// by the opening tempo's `Field::Tempo` box.
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
