// SPDX-License-Identifier: MIT
//! Editor document capabilities and instrument-specific form adapters.
//! Shared metadata, timing and file controls must not assume holes or breath.
use super::state::{DocumentCommand, EditorState, HarmonicaKind, InstrumentSelection, Mode};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_core::instrument::Instrument;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::button;

pub(super) use harmonicon_core::instrument::InstrumentKind;
pub(super) trait EditorInstrumentKind: Sized {
    fn of(instrument: &Instrument) -> Self;
    fn label(self) -> String;
    fn selection(self) -> InstrumentSelection;
}
impl EditorInstrumentKind for InstrumentKind {
    fn of(instrument: &Instrument) -> Self {
        instrument.kind()
    }
    fn label(self) -> String {
        format!("song-instrument-{}", self.slug())
    }
    fn selection(self) -> InstrumentSelection {
        if self == Self::Harmonica {
            return InstrumentSelection::HarmonicaPreset(HarmonicaKind::Diatonic);
        }
        let instrument = self.default_instrument();
        InstrumentSelection::Configured { instrument, harmonica_origin: None }
    }
}

#[derive(Component, Clone, Copy)]
pub(super) enum InstrumentScope {
    Harmonica,
    OtherInstruments,
}

pub(super) fn scoped(
    parent: &mut ChildSpawnerCommands,
    scope: InstrumentScope,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            scope,
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() },
        ))
        .with_children(build);
}
pub(super) fn sync_scopes(
    state: Res<EditorState>,
    mut groups: Query<(&InstrumentScope, &mut Node)>,
) {
    let harmonica = state.plays_harmonica();
    for (scope, mut node) in &mut groups {
        let visible = match scope {
            InstrumentScope::Harmonica => harmonica,
            InstrumentScope::OtherInstruments => !harmonica,
        };
        let display = if visible { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

pub(super) fn spawn_selector(
    parent: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    parent.spawn((
        Text::new(loc.msg("editor-field-instrument").to_string()),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(colors.label),
    ));
    parent.spawn((
        Text::new(loc.msg("editor-instrument-select-empty").to_string()),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
        TextColor(colors.label),
    ));
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|p| {
            super::transport::spawn_file_buttons(
                p,
                loc,
                colors,
                harmonicon_platform::settings::ActionButtonStyle::TextOnly,
            );
        });
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|p| {
            for kind in InstrumentKind::ALL {
                p.spawn_empty().apply_scene(button::small(
                    &loc.msg(&kind.label()),
                    move |_: On<Activate>,
                          state: Res<EditorState>,
                          loc: Res<Localization>,
                          mut feedback: ResMut<super::save_feedback::SaveFeedback>,
                          mut commands: Commands| {
                        if InstrumentKind::of(&state.effective_instrument()) == kind {
                            return;
                        }
                        if !state.notes.is_empty() || state.mode != Mode::Edit {
                            feedback.set(loc.msg("editor-instrument-select-empty"));
                            return;
                        }
                        commands.write_message(DocumentCommand::SelectInstrument(kind.selection()));
                    },
                ));
            }
        });
    parent.spawn((super::guitar_setup::GuitarSetup, Node::default()));
    parent.spawn((
        super::clarinet_setup::ClarinetSetup,
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn common_form_scopes_follow_each_document_instrument() {
        let mut app = App::new();
        app.insert_resource(EditorState::default()).add_systems(Update, sync_scopes);
        let harp = app.world_mut().spawn((InstrumentScope::Harmonica, Node::default())).id();
        let other =
            app.world_mut().spawn((InstrumentScope::OtherInstruments, Node::default())).id();
        for kind in InstrumentKind::ALL {
            app.world_mut().resource_mut::<EditorState>().instrument = kind.selection();
            app.update();
            assert_eq!(
                app.world().get::<Node>(harp).unwrap().display == Display::Flex,
                kind == InstrumentKind::Harmonica
            );
            assert_eq!(
                app.world().get::<Node>(other).unwrap().display == Display::Flex,
                kind != InstrumentKind::Harmonica
            );
            assert_eq!(
                InstrumentKind::of(&app.world().resource::<EditorState>().effective_instrument()),
                kind
            );
        }
    }
}
