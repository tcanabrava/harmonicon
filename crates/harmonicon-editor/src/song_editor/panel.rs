// SPDX-License-Identifier: MIT

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::{EditableText, TextEdit};

use super::practice::PracticeState;
use super::record::RecordState;
use super::state::{ContentKind, Dir, EditorState, Expr, Field, HarmonicaKind, Mode, Pitch};
use super::ui::{
    BendDot, ContentKindText, EditModeGroup, EditorToolbar, ExpectedNotesGroup, HarmonicaKindText,
    MetaFieldBox, MetaFieldText, ModButton, ModButtonLabel, ModeButton, NoteColumn, PlayModeGroup,
    RecordModeGroup, SnapModeText, StatusMsg, TimelineToolButton, UndoRedoButton,
};
use super::undo::UndoHistory;
use harmonicon_audio::AudioSettings;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_ui::dialogs::button::BaseButtonColor;

/// Whether `kind`'s button should read as "on" for a note carrying `dir`/
/// `pitch`/`expr` — shared by the selected-note case (an existing
/// `GridNote`'s own fields) and the nothing-selected case (`EditorState`'s
/// `sticky_dir`/`sticky_pitch`/`sticky_expr`, previewing what a *new* note
/// would get), so the two can't drift out of sync with each other.
pub(super) fn mod_button_active(kind: ModButton, dir: Dir, pitch: Pitch, expr: Expr) -> bool {
    match kind {
        ModButton::Blow => dir == Dir::Blow,
        ModButton::Draw => dir == Dir::Draw,
        ModButton::Bend => matches!(pitch, Pitch::Bend(_)),
        ModButton::Overblow => pitch == Pitch::Overblow,
        ModButton::Overdraw => pitch == Pitch::Overdraw,
        ModButton::Slide => pitch == Pitch::Slide,
        ModButton::Wah => matches!(expr, Expr::Wah(_)),
        ModButton::Vibrato => matches!(expr, Expr::Vibrato(_)),
        // Call/Split read the selected note's *phrase*, which needs the
        // whole state — `update_mod_panel` handles them beside this.
        ModButton::Depth | ModButton::Call | ModButton::Split | ModButton::Phrase => false,
        ModButton::TransposeUp | ModButton::TransposeDown | ModButton::Delete => false,
    }
}

pub(super) fn update_mod_panel(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ModButton, &mut BaseButtonColor)>,
    mut dot: Query<&mut Visibility, With<BendDot>>,
    mut labels: Query<(&ModButtonLabel, &mut Text)>,
) {
    let colors = theme.song_editor_colors();
    let selected = state.selected_note().copied();
    // The selected note's own fields take priority when there is one — a
    // sticky setting armed from an earlier, now-deselected note shouldn't
    // visually compete with what's actually selected right now. With
    // nothing selected, the sticky fields preview what a newly *added*
    // note would get, exactly matching `select_or_add`.
    let (dir, pitch, expr) = match selected {
        Some(n) => (n.dir, n.pitch, n.expr),
        None => (state.sticky_dir, state.sticky_pitch, state.sticky_expr),
    };
    let call = state.selected_call();
    let split = state.selected_split();
    for (kind, mut bg) in &mut buttons {
        let active = match kind {
            ModButton::Call => call,
            ModButton::Split => split,
            _ => mod_button_active(*kind, dir, pitch, expr),
        };
        bg.0 = if active { colors.btn_active } else { colors.btn_bg };
    }
    let bent = matches!(pitch, Pitch::Bend(_));
    for mut vis in &mut dot {
        *vis = if bent { Visibility::Inherited } else { Visibility::Hidden };
    }
    // Show the selected (or, with nothing selected, sticky-armed) rate next
    // to Wah/Vibrato (e.g. "Vibrato 5Hz") so cycling the rate with repeated
    // clicks is legible.
    let depth = super::selected_metadata::depth_label(state.depth_for_button());
    for (label, mut text) in &mut labels {
        let suffix = match (label.kind, expr) {
            (ModButton::Vibrato, Expr::Vibrato(hz)) => Some(format!("{hz:.0}Hz")),
            (ModButton::Wah, Expr::Wah(hz)) => Some(format!("{hz:.0}Hz")),
            // Same shape as the rate: the selected note's depth, or the
            // sticky one — and nothing for a note with no expression.
            (ModButton::Depth, _) if !depth.is_empty() => Some(depth.clone()),
            _ => None,
        };
        **text = match suffix {
            Some(suffix) => format!("{} {suffix}", label.base),
            None => label.base.clone(),
        };
    }
}

/// Keeps click-to-cycle fields' displayed text in step with `EditorState` —
/// free-text fields render themselves (see [`sync_meta_field_text`]).
pub(super) fn update_meta_fields(
    state: Res<EditorState>,
    mut texts: Query<(&MetaFieldText, &mut Text)>,
) {
    for (tag, mut text) in &mut texts {
        **text = if tag.0 == Field::Key {
            format!("\u{2039}  {}  \u{203A}", state.key)
        } else {
            state.field_text(tag.0).to_string()
        };
    }
}

/// Keeps every meta-form text box's *displayed* buffer in step with
/// `EditorState` whenever it changes from outside the widget itself — Load,
/// MIDI import, Browse picking a music file, ... — since typing only
/// commits into `EditorState` on blur/Enter (`dialogs::text_input`), and
/// nothing else re-renders a box's buffer from `EditorState`. Skips whichever field
/// currently has real keyboard focus so it never fights the player's own
/// in-progress typing. Can't gate on `EditorState::is_changed()` to detect
/// "a field's text changed" — the resource changes on essentially every
/// frame regardless (note edits, selection, ...) — so this just compares
/// each field's own string every frame instead; cheap at nine short fields.
pub(super) fn sync_meta_field_text(
    state: Res<EditorState>,
    focus: Res<InputFocus>,
    mut fields: Query<(Entity, &MetaFieldBox, &mut EditableText)>,
) {
    let focused = focus.get();
    for (entity, tag, mut text) in &mut fields {
        if Some(entity) == focused {
            continue;
        }
        let want = state.field_text(tag.0);
        if text.value() != want {
            text.editor_mut().set_text(want);
            text.queue_edit(if tag.0 == Field::Description {
                TextEdit::TextStart(false)
            } else {
                TextEdit::TextEnd(false)
            });
        }
    }
}

/// Highlights whichever of Edit/Record/Play is the current mode, and Lock
/// when `state.locked()` — which includes the forced-lock that the Record
/// and Play modes always apply, not just the user's own toggle.
pub(super) fn update_mode_buttons(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ModeButton, &mut BaseButtonColor)>,
) {
    let colors = theme.song_editor_colors();
    for (kind, mut bg) in &mut buttons {
        let active = match kind {
            ModeButton::Edit => state.mode == Mode::Edit,
            ModeButton::Record => state.mode == Mode::Record,
            ModeButton::Play => state.mode == Mode::Play,
            ModeButton::Lock => state.locked(),
            ModeButton::ExpectedNotes => state.mode == Mode::ExpectedNotes,
        };
        bg.0 = if active { colors.btn_active } else { colors.btn_bg };
    }
}

pub(super) fn update_timeline_tool_buttons(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&TimelineToolButton, &mut BaseButtonColor)>,
) {
    let colors = theme.song_editor_colors();
    for (kind, mut bg) in &mut buttons {
        bg.0 = if kind.0 == state.timeline_tool { colors.btn_active } else { colors.btn_bg };
    }
}

/// Dims the Undo/Redo buttons when their respective stack is empty —
/// clicking still no-ops either way (see `undo::UndoHistory::undo`/`redo`'s
/// own doc comments), but a visibly inert button is a clearer signal than
/// a fully-lit one that silently does nothing.
pub(super) fn update_undo_redo_buttons(
    history: Res<UndoHistory>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&UndoRedoButton, &mut BaseButtonColor)>,
) {
    let colors = theme.song_editor_colors();
    for (kind, mut bg) in &mut buttons {
        let available = match kind {
            UndoRedoButton::Undo => history.can_undo(),
            UndoRedoButton::Redo => history.can_redo(),
        };
        bg.0 = if available { colors.btn_bg } else { colors.btn_bg.with_alpha(0.35) };
    }
}

/// Dims the metronome toggle button while muted — same "dim rather than
/// leave clickable-but-inert-looking" visual language as
/// [`update_undo_redo_buttons`], though here the click is never actually a
/// no-op (it always flips `MetronomeMuted`).
pub(super) fn update_metronome_toggle_button(
    muted: Res<harmonicon_gameplay::gameplay::metronome_overlay::MetronomeMuted>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<&mut BaseButtonColor, With<super::ui::MetronomeToggleButton>>,
) {
    let colors = theme.song_editor_colors();
    let bg = if muted.0 { colors.btn_bg.with_alpha(0.35) } else { colors.btn_bg };
    for mut button_bg in &mut buttons {
        button_bg.0 = bg;
    }
}

/// Shows exactly the current mode's button cluster — never more than one.
/// Toggles `Node::display`, not `Visibility`: `Visibility::Hidden` still
/// reserves the hidden group's layout space, which would push the visible
/// group aside instead of freeing its place.
pub(super) fn update_mode_visibility(
    state: Res<EditorState>,
    mut edit_group: Query<
        &mut Node,
        (
            With<EditModeGroup>,
            Without<RecordModeGroup>,
            Without<PlayModeGroup>,
            Without<ExpectedNotesGroup>,
        ),
    >,
    mut record_group: Query<
        &mut Node,
        (
            With<RecordModeGroup>,
            Without<EditModeGroup>,
            Without<PlayModeGroup>,
            Without<ExpectedNotesGroup>,
        ),
    >,
    mut play_group: Query<
        &mut Node,
        (
            With<PlayModeGroup>,
            Without<EditModeGroup>,
            Without<RecordModeGroup>,
            Without<ExpectedNotesGroup>,
        ),
    >,
    mut expected_notes_group: Query<
        &mut Node,
        (
            With<ExpectedNotesGroup>,
            Without<EditModeGroup>,
            Without<RecordModeGroup>,
            Without<PlayModeGroup>,
        ),
    >,
) {
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    for mut node in &mut edit_group {
        let want = display(state.mode == Mode::Edit);
        if node.display != want {
            node.display = want;
        }
    }
    for mut node in &mut record_group {
        let want = display(state.mode == Mode::Record);
        if node.display != want {
            node.display = want;
        }
    }
    for mut node in &mut play_group {
        let want = display(state.mode == Mode::Play);
        if node.display != want {
            node.display = want;
        }
    }
    for mut node in &mut expected_notes_group {
        let want = display(state.mode == Mode::ExpectedNotes);
        if node.display != want {
            node.display = want;
        }
    }
}

/// Shows the toolbar's note column in Edit mode only, and gives the grid
/// its width back outside it — a Record/Play toolbar has nothing to put in
/// that column, and 56 px of empty toolbar is real grid width on a phone.
/// Its own system rather than another arm of [`update_mode_visibility`]
/// because it also resizes the toolbar itself, which that system's
/// group-only queries don't reach.
pub(super) fn update_note_column(
    state: Res<EditorState>,
    mut column: Query<&mut Node, With<NoteColumn>>,
    mut toolbar: Query<(&EditorToolbar, &mut Node), Without<NoteColumn>>,
) {
    let editing = state.mode == Mode::Edit;
    for mut node in &mut column {
        let want = if editing { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (widths, mut node) in &mut toolbar {
        let want =
            Val::Px(if editing { widths.width_with_note_column } else { widths.width_without });
        if node.width != want {
            node.width = want;
        }
    }
}

/// Shows Bend/Overblow/Overdraw for a diatonic chart and Slide for a
/// chromatic one — never both, since the two harmonicas don't share
/// techniques. Mirrors [`update_mode_visibility`]'s `Node::display` approach.
pub(super) fn update_technique_button_visibility(
    state: Res<EditorState>,
    mut buttons: Query<(&ModButton, &mut Node)>,
) {
    let diatonic_only = state.harmonica_kind.is_diatonic();
    for (kind, mut node) in &mut buttons {
        let visible = match kind {
            ModButton::Bend | ModButton::Overblow | ModButton::Overdraw => diatonic_only,
            ModButton::Slide => !diatonic_only,
            _ => continue,
        };
        let want = if visible { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

/// Keeps the harmonica-kind toggle's label in sync with `state.harmonica_kind`.
pub(super) fn update_harmonica_kind_text(
    state: Res<EditorState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<HarmonicaKindText>>,
) {
    let key = match state.harmonica_kind {
        HarmonicaKind::Diatonic => "editor-harmonica-diatonic",
        HarmonicaKind::PaddyRichter => "editor-harmonica-paddy-richter",
        HarmonicaKind::CountryTuned => "editor-harmonica-country-tuned",
        HarmonicaKind::NaturalMinor => "editor-harmonica-natural-minor",
        HarmonicaKind::Chromatic => "editor-harmonica-chromatic",
        HarmonicaKind::Chromatic16 => "editor-harmonica-chromatic-16",
    };
    let label = String::from(loc.msg(key));
    for mut text in &mut texts {
        **text = label.clone();
    }
}

/// Keeps the record-mode toggle's label in sync with `state.content_kind`.
pub(super) fn update_content_kind_text(
    state: Res<EditorState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<ContentKindText>>,
) {
    let key = match state.content_kind {
        ContentKind::Song => "editor-content-kind-song",
        ContentKind::Lesson => "editor-content-kind-lesson",
    };
    let label = String::from(loc.msg(key));
    for mut text in &mut texts {
        **text = label.clone();
    }
}

/// Keeps the grid-snap toggle's label in sync with `state.snap_mode`.
pub(super) fn update_snap_mode_text(
    state: Res<EditorState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<SnapModeText>>,
) {
    let label = String::from(loc.msg(state.snap_mode.label_key()));
    for mut text in &mut texts {
        **text = label.clone();
    }
}

pub(super) fn update_status_bar(
    state: Res<EditorState>,
    audio: Res<AudioSettings>,
    practice: Res<PracticeState>,
    record: Res<RecordState>,
    count_in: Res<super::metronome::CountIn>,
    feedback: Res<super::save_feedback::SaveFeedback>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<StatusMsg>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    // A just-finished Save/Load comes first — it's a direct response to
    // something the player just clicked, and (unlike every other tier
    // here) is the only one that can report failure, so it shouldn't get
    // silently buried under a count-in or a drag in progress. A count-in
    // comes next — it's the most time-critical (a take starts the instant
    // it hits zero) and the player needs to know recording hasn't begun
    // yet. Drag messages after that (ephemeral and action-specific); a
    // live recording after that (it's actively running, unlike the
    // practice message which just sits there between hits); practice
    // messages fill the bar otherwise. An unscorable-chord warning sits
    // between active work and passive practice feedback.
    **text = if let Some(msg) = feedback.current() {
        msg.to_string()
    } else if let Some(secs) = count_in.remaining_secs_display() {
        loc.msg_args("editor-count-in-status", &[("seconds", format!("{secs:.1}"))]).to_string()
    } else if !state.drag_msg.is_empty() {
        state.drag_msg.to_string()
    } else if record.active {
        loc.msg_args("editor-record-status", &[("count", record.note_count.to_string())])
            .to_string()
    } else if editor_chords_unhearable(&state, &audio) {
        loc.msg("chord-warning-monophonic").to_string()
    } else {
        practice.msg.to_string()
    };
}

/// Whether the current chart contains a serialized chord the selected pitch
/// detector cannot hear. Notes form one chart item only when onset and duration
/// match, mirroring `harpchart::serialize_harpchart_notes`.
fn editor_chords_unhearable(state: &EditorState, audio: &AudioSettings) -> bool {
    if audio.pitch_algorithm.is_polyphonic() {
        return false;
    }
    let mut spans = std::collections::HashSet::new();
    state.notes.iter().any(|note| !spans.insert((note.tick, note.len)))
}

#[cfg(test)]
mod tests {
    use super::super::state::GridNote;
    use super::*;
    use harmonicon_audio::pitch_detect::PitchAlgorithm;

    fn note(dir: Dir, pitch: Pitch, expr: Expr) -> GridNote {
        GridNote { id: 1, hole: 4, tick: 0, len: 1, dir, pitch, expr }
    }

    #[test]
    fn monophonic_detector_warns_only_for_notes_that_serialize_as_a_chord() {
        let mut state = EditorState {
            notes: vec![
                GridNote {
                    id: 1,
                    hole: 4,
                    tick: 0,
                    len: 4,
                    dir: Dir::Blow,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
                GridNote {
                    id: 2,
                    hole: 5,
                    tick: 0,
                    len: 2,
                    dir: Dir::Blow,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
            ],
            ..Default::default()
        };
        let mut audio =
            AudioSettings { pitch_algorithm: PitchAlgorithm::Yin, ..Default::default() };

        assert!(!editor_chords_unhearable(&state, &audio));
        state.notes[1].len = 4;
        assert!(editor_chords_unhearable(&state, &audio));
        audio.pitch_algorithm = PitchAlgorithm::Nmf;
        assert!(!editor_chords_unhearable(&state, &audio));
    }

    // ── update_mod_panel ─────────────────────────────────────────────────────

    #[test]
    fn update_mod_panel_reflects_the_selected_notes_direction_pitch_and_expr_rate() {
        let mut world = World::new();
        let state = EditorState {
            notes: vec![note(Dir::Blow, Pitch::Bend(0.5), Expr::Vibrato(5.0))],
            selected: vec![1],
            ..Default::default()
        };
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let blow = world.spawn((ModButton::Blow, BaseButtonColor(colors.btn_bg))).id();
        let draw = world.spawn((ModButton::Draw, BaseButtonColor(colors.btn_bg))).id();
        let bend_dot = world.spawn((BendDot, Visibility::Hidden)).id();
        let vibrato_label = world
            .spawn((
                ModButtonLabel { kind: ModButton::Vibrato, base: "Vibrato".into() },
                Text::new("Vibrato"),
            ))
            .id();
        let wah_label = world
            .spawn((ModButtonLabel { kind: ModButton::Wah, base: "Wah".into() }, Text::new("Wah")))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<BaseButtonColor>(blow).unwrap().0,
            colors.btn_active,
            "the selected note's direction button should highlight"
        );
        assert_eq!(
            world.get::<BaseButtonColor>(draw).unwrap().0,
            colors.btn_bg,
            "the other direction button should not"
        );
        assert_eq!(
            *world.get::<Visibility>(bend_dot).unwrap(),
            Visibility::Inherited,
            "a bent note shows the bend dot"
        );
        assert_eq!(world.get::<Text>(vibrato_label).unwrap().0, "Vibrato 5Hz");
        assert_eq!(
            world.get::<Text>(wah_label).unwrap().0,
            "Wah",
            "a mismatched expr kind keeps just the base label"
        );
    }

    #[test]
    fn update_mod_panel_falls_back_to_sticky_defaults_when_nothing_selected() {
        let mut world = World::new();
        world.insert_resource(EditorState::default());
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        // `EditorState::default()`'s `sticky_dir` is `Dir::Blow` — a note
        // added right now would be Blow, so the Blow button should already
        // read "on" even though nothing is selected. Every pitch/expr
        // button stays off, since `sticky_pitch`/`sticky_expr` default to
        // their own "off" variants.
        let blow = world.spawn((ModButton::Blow, BaseButtonColor(colors.btn_bg))).id();
        let draw = world.spawn((ModButton::Draw, BaseButtonColor(colors.btn_active))).id();
        let bend = world.spawn((ModButton::Bend, BaseButtonColor(colors.btn_active))).id();
        let bend_dot = world.spawn((BendDot, Visibility::Inherited)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(world.get::<BaseButtonColor>(blow).unwrap().0, colors.btn_active);
        assert_eq!(world.get::<BaseButtonColor>(draw).unwrap().0, colors.btn_bg);
        assert_eq!(world.get::<BaseButtonColor>(bend).unwrap().0, colors.btn_bg);
        assert_eq!(*world.get::<Visibility>(bend_dot).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn update_mod_panel_lights_call_and_labels_depth_from_the_selected_phrase() {
        use super::super::state::PhraseAnnotation;
        let mut world = World::new();
        let mut state = EditorState {
            notes: vec![note(Dir::Blow, Pitch::Normal, Expr::Vibrato(5.0))],
            selected: vec![1],
            ..Default::default()
        };
        state.phrase_annotations.insert(0, PhraseAnnotation { call: true, ..Default::default() });
        state.expression_intensities.insert(1, "0.75".into());
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let call = world.spawn((ModButton::Call, BaseButtonColor(colors.btn_bg))).id();
        let split = world.spawn((ModButton::Split, BaseButtonColor(colors.btn_bg))).id();
        let depth_label = world
            .spawn((
                ModButtonLabel { kind: ModButton::Depth, base: "Depth".into() },
                Text::new("Depth"),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<BaseButtonColor>(call).unwrap().0,
            colors.btn_active,
            "the phrase is a call, so Call lights"
        );
        assert_eq!(
            world.get::<BaseButtonColor>(split).unwrap().0,
            colors.btn_bg,
            "and Split doesn't"
        );
        assert_eq!(world.get::<Text>(depth_label).unwrap().0, "Depth 75%");
    }

    #[test]
    fn update_mod_panel_shows_no_depth_for_a_note_with_no_expression() {
        let mut world = World::new();
        world.insert_resource(EditorState {
            notes: vec![note(Dir::Blow, Pitch::Normal, Expr::None)],
            selected: vec![1],
            ..Default::default()
        });
        world.insert_resource(LoadedTheme::default());
        let depth_label = world
            .spawn((
                ModButtonLabel { kind: ModButton::Depth, base: "Depth".into() },
                Text::new("Depth 50%"),
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);
        assert_eq!(world.get::<Text>(depth_label).unwrap().0, "Depth");
    }

    #[test]
    fn update_mod_panel_reflects_an_armed_sticky_modifier_when_nothing_selected() {
        let mut world = World::new();
        world.insert_resource(EditorState {
            sticky_dir: Dir::Draw,
            sticky_pitch: Pitch::Bend(1.0),
            sticky_expr: Expr::Wah(3.0),
            ..Default::default()
        });
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let draw = world.spawn((ModButton::Draw, BaseButtonColor(colors.btn_bg))).id();
        let bend = world.spawn((ModButton::Bend, BaseButtonColor(colors.btn_bg))).id();
        let wah = world.spawn((ModButton::Wah, BaseButtonColor(colors.btn_bg))).id();
        let bend_dot = world.spawn((BendDot, Visibility::Hidden)).id();
        let wah_label = world
            .spawn((ModButtonLabel { kind: ModButton::Wah, base: "Wah".into() }, Text::new("Wah")))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(world.get::<BaseButtonColor>(draw).unwrap().0, colors.btn_active);
        assert_eq!(world.get::<BaseButtonColor>(bend).unwrap().0, colors.btn_active);
        assert_eq!(world.get::<BaseButtonColor>(wah).unwrap().0, colors.btn_active);
        assert_eq!(*world.get::<Visibility>(bend_dot).unwrap(), Visibility::Inherited);
        assert_eq!(world.get::<Text>(wah_label).unwrap().0, "Wah 3Hz");
    }

    // ── update_meta_fields ────────────────────────────────────────────────────

    #[test]
    fn update_meta_fields_formats_the_key_field_specially() {
        let mut world = World::new();
        let state = EditorState { key: "G".into(), position: "3rd".into(), ..Default::default() };
        world.insert_resource(state);

        let key_text = world.spawn((MetaFieldText(Field::Key), Text::new(""))).id();
        let position_text = world.spawn((MetaFieldText(Field::Position), Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_meta_fields);
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(key_text).unwrap().0, "\u{2039}  G  \u{203A}");
        assert_eq!(world.get::<Text>(position_text).unwrap().0, "3rd");
    }

    // ── sync_meta_field_text ──────────────────────────────────────────────────

    #[test]
    fn sync_meta_field_text_pushes_an_external_change_into_the_unfocused_box() {
        let mut world = World::new();
        world.insert_resource(EditorState { name: "New Title".into(), ..Default::default() });
        world.insert_resource(InputFocus::default());

        let name_input =
            world.spawn((MetaFieldBox(Field::Name), EditableText::new("Old Title"))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(sync_meta_field_text);
        schedule.run(&mut world);

        assert_eq!(world.get::<EditableText>(name_input).unwrap().value().to_string(), "New Title");
    }

    #[test]
    fn sync_meta_field_text_never_touches_the_focused_box() {
        let mut world = World::new();
        world.insert_resource(EditorState { name: "New Title".into(), ..Default::default() });

        let name_input =
            world.spawn((MetaFieldBox(Field::Name), EditableText::new("still typing"))).id();
        let mut focus = InputFocus::default();
        focus.set(name_input, bevy::input_focus::FocusCause::Pressed);
        world.insert_resource(focus);

        let mut schedule = Schedule::default();
        schedule.add_systems(sync_meta_field_text);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<EditableText>(name_input).unwrap().value().to_string(),
            "still typing",
            "the box the player is actively typing into must never be overwritten"
        );
    }

    // ── update_mode_buttons ───────────────────────────────────────────────────

    #[test]
    fn update_mode_buttons_highlights_the_active_mode_and_lock_state() {
        let mut world = World::new();
        let state = EditorState { mode: Mode::Play, ..Default::default() };
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let edit = world.spawn((ModeButton::Edit, BaseButtonColor(colors.btn_active))).id();
        let record = world.spawn((ModeButton::Record, BaseButtonColor(colors.btn_active))).id();
        let play = world.spawn((ModeButton::Play, BaseButtonColor(colors.btn_bg))).id();
        // Play mode is always locked, even without the user's own toggle.
        let lock = world.spawn((ModeButton::Lock, BaseButtonColor(colors.btn_bg))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mode_buttons);
        schedule.run(&mut world);

        assert_eq!(world.get::<BaseButtonColor>(edit).unwrap().0, colors.btn_bg);
        assert_eq!(world.get::<BaseButtonColor>(record).unwrap().0, colors.btn_bg);
        assert_eq!(world.get::<BaseButtonColor>(play).unwrap().0, colors.btn_active);
        assert_eq!(
            world.get::<BaseButtonColor>(lock).unwrap().0,
            colors.btn_active,
            "Play mode forces Lock active regardless of user_locked"
        );
    }

    // ── update_mode_visibility ────────────────────────────────────────────────

    #[test]
    fn update_mode_visibility_shows_only_the_current_modes_group() {
        let mut world = World::new();
        let state = EditorState { mode: Mode::Play, ..Default::default() };
        world.insert_resource(state);

        let edit_group = world.spawn((EditModeGroup, Node::default())).id();
        let record_group = world.spawn((RecordModeGroup, Node::default())).id();
        let play_group = world.spawn((PlayModeGroup, Node::default())).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mode_visibility);
        schedule.run(&mut world);

        assert_eq!(world.get::<Node>(edit_group).unwrap().display, Display::None);
        assert_eq!(world.get::<Node>(record_group).unwrap().display, Display::None);
        assert_eq!(world.get::<Node>(play_group).unwrap().display, Display::Flex);
    }

    // ── update_technique_button_visibility ────────────────────────────────────

    #[test]
    fn update_technique_button_visibility_shows_bend_family_for_diatonic_and_slide_for_chromatic() {
        let mut world = World::new();
        let state = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
        world.insert_resource(state);

        let bend = world.spawn((ModButton::Bend, Node::default())).id();
        let slide = world.spawn((ModButton::Slide, Node::default())).id();
        // Untouched by either branch — must be left exactly as spawned.
        let blow =
            world.spawn((ModButton::Blow, Node { display: Display::Grid, ..default() })).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_technique_button_visibility);
        schedule.run(&mut world);

        assert_eq!(world.get::<Node>(bend).unwrap().display, Display::None);
        assert_eq!(world.get::<Node>(slide).unwrap().display, Display::Flex);
        assert_eq!(
            world.get::<Node>(blow).unwrap().display,
            Display::Grid,
            "buttons outside the bend/slide family are never touched"
        );
    }

    // ── update_harmonica_kind_text ────────────────────────────────────────────

    #[test]
    fn update_harmonica_kind_text_keys_off_the_current_harmonica_kind() {
        let mut world = World::new();
        let state = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
        world.insert_resource(state);
        world.insert_resource(Localization::default());

        let label = world.spawn((HarmonicaKindText, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_harmonica_kind_text);
        schedule.run(&mut world);

        // No FTL bundle is loaded, so `loc.msg` falls back to the key itself
        // — enough to confirm the right key was chosen for this kind.
        assert_eq!(world.get::<Text>(label).unwrap().0, "editor-harmonica-chromatic");
    }

    // ── update_status_bar ─────────────────────────────────────────────────────

    #[test]
    fn update_status_bar_prefers_the_drag_message_over_the_practice_message() {
        let mut world = World::new();
        let loc = Localization::default();
        let state = EditorState { drag_msg: loc.msg("editor-drag-msg"), ..Default::default() };
        world.insert_resource(state);
        // `PracticeState` has private fields not reachable from here, so a
        // `..Default::default()` struct literal isn't an option — only the
        // in-module `#[cfg(test)]` helpers get that.
        #[allow(clippy::field_reassign_with_default)]
        let practice = {
            let mut p = PracticeState::default();
            p.msg = loc.msg("editor-practice-msg");
            p
        };
        world.insert_resource(practice);
        world.insert_resource(AudioSettings::default());
        world.insert_resource(RecordState::default());
        world.insert_resource(super::super::metronome::CountIn::default());
        world.insert_resource(super::super::save_feedback::SaveFeedback::default());
        world.insert_resource(loc);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-drag-msg");
    }

    #[test]
    fn update_status_bar_falls_back_to_the_practice_message_when_no_drag_is_in_progress() {
        let mut world = World::new();
        let loc = Localization::default();
        world.insert_resource(EditorState::default());
        #[allow(clippy::field_reassign_with_default)]
        let practice = {
            let mut p = PracticeState::default();
            p.msg = loc.msg("editor-practice-msg");
            p
        };
        world.insert_resource(practice);
        world.insert_resource(AudioSettings::default());
        world.insert_resource(RecordState::default());
        world.insert_resource(super::super::metronome::CountIn::default());
        world.insert_resource(super::super::save_feedback::SaveFeedback::default());
        world.insert_resource(loc);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-practice-msg");
    }

    #[test]
    fn update_status_bar_prefers_the_recording_message_over_the_practice_message() {
        let mut world = World::new();
        let loc = Localization::default();
        world.insert_resource(EditorState::default());
        #[allow(clippy::field_reassign_with_default)]
        let practice = {
            let mut p = PracticeState::default();
            p.msg = loc.msg("editor-practice-msg");
            p
        };
        world.insert_resource(practice);
        world.insert_resource(AudioSettings::default());
        // `RecordState::open` is private to the `record` module, so build
        // this via field mutation (`active` is `pub(super)`) rather than a
        // struct literal, which would need every field visible here.
        #[allow(clippy::field_reassign_with_default)]
        let record = {
            let mut r = RecordState::default();
            r.active = true;
            r
        };
        world.insert_resource(record);
        world.insert_resource(super::super::metronome::CountIn::default());
        world.insert_resource(super::super::save_feedback::SaveFeedback::default());
        world.insert_resource(loc);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        // `Localization::default()` has no bundle loaded, so `loc.msg_args`
        // falls back to returning the key itself — just confirm it picked
        // the recording branch over the (also-set) practice message.
        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-record-status");
    }
}
