// SPDX-License-Identifier: MIT
//! New-document flow, independent of any instrument's note representation.
use super::instrument_ui::InstrumentKind;
use super::state::{DocumentCommand, EditorState, FIELDS, Mode};
use bevy::input_focus::tab_navigation::TabGroup;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_app::app::AppState;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::{
    button,
    confirm_dialog::{ConfirmChosen, DialogId, OpenConfirmDialog},
};

const REPLACE_DOCUMENT: DialogId = DialogId("editor_new_document");

#[derive(Resource, Default)]
pub(super) struct DocumentPicker {
    pub open: bool,
    pub initial: bool,
    pub chosen: bool,
    pending: Option<InstrumentKind>,
}
#[derive(Component)]
pub(super) struct PickerRoot;

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

fn finish(kind: InstrumentKind, picker: &mut DocumentPicker, commands: &mut Commands) {
    picker.open = false;
    picker.chosen = true;
    picker.pending = None;
    commands.write_message(DocumentCommand::New(kind.selection()));
    commands.insert_resource(super::playback::Playhead::default());
    commands.insert_resource(super::state::Scroll::default());
    commands.insert_resource(super::state::TimelineSelection::default());
    commands.insert_resource(super::clipboard::NoteClipboard::default());
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
            finish(kind, &mut picker, &mut commands);
        }
    }
}

pub(super) fn sync_picker(
    mut commands: Commands,
    picker: Res<DocumentPicker>,
    loc: Res<Localization>,
    roots: Query<Entity, With<PickerRoot>>,
) {
    if !picker.open {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }
    commands
        .spawn((
            PickerRoot,
            super::ui::EditorRoot,
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.97)),
            GlobalZIndex(350),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(440.0),
                        max_width: Val::Percent(95.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.10, 0.14)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(loc.msg("editor-new-title").to_string()),
                        TextFont { font_size: FontSize::Px(26.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    panel.spawn((
                        Text::new(loc.msg("editor-new-instrument-question").to_string()),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    for kind in InstrumentKind::ALL {
                        panel.spawn_empty().apply_scene(button::default(&loc.msg(kind.label()),
                    move |_: On<Activate>, state: Res<EditorState>,
                        mut picker: ResMut<DocumentPicker>, mut commands: Commands,
                        loc: Res<Localization>, mut confirm: MessageWriter<OpenConfirmDialog>| {
                        if has_content(&state) {
                            picker.pending = Some(kind);
                            confirm.write(OpenConfirmDialog { purpose: REPLACE_DOCUMENT,
                                message: loc.msg("editor-new-replace-warning").to_string() });
                        } else {
                            finish(kind, &mut picker, &mut commands);
                        }
                    }));
                    }
                    panel.spawn_empty().apply_scene(button::default(
                        &loc.msg("dialog-cancel"),
                        |_: On<Activate>,
                         mut picker: ResMut<DocumentPicker>,
                         mut next: ResMut<NextState<AppState>>| {
                            picker.open = false;
                            picker.pending = None;
                            if picker.initial {
                                next.set(AppState::Menu);
                            }
                        },
                    ));
                });
        });
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
            .insert_resource(DocumentPicker {
                open: true,
                pending: Some(InstrumentKind::Piano),
                ..Default::default()
            })
            .init_resource::<super::super::undo::UndoHistory>()
            .add_systems(
                Update,
                (handle_confirmation, super::super::undo::apply_document_commands).chain(),
            );
        app.world_mut()
            .write_message(ConfirmChosen { purpose: REPLACE_DOCUMENT, confirmed: false });
        app.update();
        assert_eq!(app.world().resource::<EditorState>().name, "Keep this title");
        assert!(app.world().resource::<DocumentPicker>().open);
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
            state.notes.push(super::super::state::GridNote {
                id: 1,
                tick: 0,
                len: super::super::TICKS_PER_BEAT,
                expr: super::super::state::Expr::None,
                target: harmonicon_core::note_target::NoteTarget::Positioned(position),
            });
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
