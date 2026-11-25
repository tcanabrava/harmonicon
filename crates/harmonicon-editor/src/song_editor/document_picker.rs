// SPDX-License-Identifier: MIT
//! New-document flow, independent of any instrument's note representation.
//! A new document is always for the instrument chosen on Select Instrument
//! (`MyInstruments::active`); the editor never asks which one.
use super::instrument_ui::EditorInstrumentKind;
use super::instrument_ui::InstrumentKind;
use super::state::{DocumentCommand, EditorState, FIELDS, Mode};
use bevy::prelude::*;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::confirm_dialog::{ConfirmChosen, DialogId, OpenConfirmDialog};

const REPLACE_DOCUMENT: DialogId = DialogId("editor_new_document");

/// A new document waiting on "discard the current one?".
#[derive(Resource, Default)]
pub(super) struct DocumentPicker {
    pending: Option<InstrumentKind>,
}

/// Changes to notes, metadata or instrument configuration require explicit
/// confirmation. View-only changes do not create content.
pub(super) fn has_content(state: &EditorState) -> bool {
    let defaults = EditorState::default();
    !state.notes.is_empty()
        || !state.expected_notes.is_empty()
        || !state.phrase_annotations.is_empty()
        || !state.tempo_changes.is_empty()
        || !state.meter_changes.is_empty()
        || !state.repeats.is_empty()
        || state.content_kind != defaults.content_kind
        || state.time_signature != defaults.time_signature
        || state.chart_author != defaults.chart_author
        || state.instrument != InstrumentKind::of(&state.effective_instrument()).selection()
        || FIELDS.iter().any(|(field, _)| state.field_text(*field) != defaults.field_text(*field))
}

fn finish(kind: InstrumentKind, commands: &mut Commands) {
    commands.write_message(DocumentCommand::New(kind.selection()));
    commands.insert_resource(super::playback::Playhead::default());
    commands.insert_resource(super::state::Scroll::default());
    commands.insert_resource(super::state::TimelineSelection::default());
    commands.insert_resource(super::clipboard::NoteClipboard::default());
}

/// Start a new document for `kind`, asking first if the open one has content.
pub(super) fn request_new(
    kind: InstrumentKind,
    state: &EditorState,
    picker: &mut DocumentPicker,
    loc: &Localization,
    confirm: &mut MessageWriter<OpenConfirmDialog>,
    commands: &mut Commands,
) {
    if has_content(state) {
        picker.pending = Some(kind);
        confirm.write(OpenConfirmDialog {
            purpose: REPLACE_DOCUMENT,
            message: loc.msg("editor-new-replace-warning").to_string(),
        });
    } else {
        finish(kind, commands);
    }
}

pub(super) fn handle_confirmation(
    mut results: MessageReader<ConfirmChosen>,
    mut picker: ResMut<DocumentPicker>,
    mut commands: Commands,
) {
    for result in results.read().filter(|result| result.purpose == REPLACE_DOCUMENT) {
        let pending = picker.pending.take();
        if result.confirmed
            && let Some(kind) = pending
        {
            finish(kind, &mut commands);
        }
    }
}

/// Every document starts in Edit mode; instrument-specific defaults come
/// from the adapter. No other document's notes or metadata are retained.
pub(super) fn fresh(selection: super::state::InstrumentSelection) -> EditorState {
    let mut state = EditorState { instrument: selection, mode: Mode::Edit, ..Default::default() };
    super::piano_note::reset_window(&mut state);
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_replacement_preserves_the_existing_document() {
        let mut app = App::new();
        app.add_message::<ConfirmChosen>()
            .add_message::<DocumentCommand>()
            .insert_resource(EditorState { name: "Keep this title".into(), ..Default::default() })
            .insert_resource(DocumentPicker { pending: Some(InstrumentKind::Piano) })
            .init_resource::<super::super::undo::UndoHistory>()
            .add_systems(
                Update,
                (handle_confirmation, super::super::undo::apply_document_commands).chain(),
            );
        app.world_mut()
            .write_message(ConfirmChosen { purpose: REPLACE_DOCUMENT, confirmed: false });
        app.update();
        assert_eq!(app.world().resource::<EditorState>().name, "Keep this title");
        assert!(app.world().resource::<DocumentPicker>().pending.is_none());
    }
    #[test]
    fn creating_a_document_clears_the_previous_undo_history() {
        let mut app = App::new();
        app.add_message::<DocumentCommand>()
            .insert_resource(EditorState { name: "Old title".into(), ..Default::default() })
            .init_resource::<super::super::record::RecordState>()
            .init_resource::<super::super::undo::UndoHistory>()
            .add_systems(
                Update,
                (super::super::undo::track_changes, super::super::undo::apply_document_commands)
                    .chain(),
            );
        app.update();
        app.world_mut().resource_mut::<EditorState>().name = "Another old title".into();
        app.update();
        app.world_mut().write_message(DocumentCommand::New(InstrumentKind::Piano.selection()));
        app.update();
        app.world_mut().write_message(DocumentCommand::Undo);
        app.update();
        let state = app.world().resource::<EditorState>();
        assert_eq!(state.name, EditorState::default().name);
        assert_eq!(InstrumentKind::of(&state.effective_instrument()), InstrumentKind::Piano);
    }

    #[test]
    fn new_documents_have_only_the_chosen_instruments_defaults() {
        for kind in InstrumentKind::ALL {
            let mut state = fresh(kind.selection());
            assert_eq!(InstrumentKind::of(&state.effective_instrument()), kind);
            assert!(state.notes.is_empty());
            assert!(!has_content(&state));
            // Drafts start empty; the chart format requires a note before
            // saving. Verify each adapter can author a valid first event.
            use harmonicon_core::instrument::InstrumentModel;
            let instrument = state.effective_instrument();
            let pitch =
                instrument.valid_pitches().into_iter().min().expect("a playable instrument");
            let position = instrument.positions_for(pitch)[0];
            state.notes.push(super::super::state::GridNote::new(
                1,
                0,
                super::super::TICKS_PER_BEAT,
                super::super::state::Expr::None,
                harmonicon_core::note_target::NoteTarget::Positioned(position),
            ));
            let mut value: serde_json::Value =
                serde_json::from_str(&super::super::harpchart::serialize_harpchart(&state))
                    .unwrap();
            harmonicon_song::song::validate_and_migrate_chart(&mut value).unwrap();
        }
    }
    #[test]
    fn metadata_and_notes_are_protected_but_scroll_is_not_content() {
        let mut state = EditorState { scroll_beat: 10, ..default() };
        assert!(!has_content(&state));
        state.name = "An unsaved title".into();
        assert!(has_content(&state));
    }
}
