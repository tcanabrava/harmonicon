// SPDX-License-Identifier: MIT
//! Editor document capabilities and instrument-specific form adapters.
//! Shared metadata, timing and file controls must not assume holes or breath.
use super::state::{DocumentCommand, EditorState, HarmonicaKind, InstrumentSelection, Mode};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_core::instrument::{
    Instrument,
    flute::Flute,
    guitar::{Guitar, GuitarBody},
    piano::Piano,
};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::button;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstrumentKind {
    Harmonica,
    Guitar,
    Piano,
    Flute,
}
impl InstrumentKind {
    pub const ALL: [Self; 4] = [Self::Harmonica, Self::Guitar, Self::Piano, Self::Flute];
    pub fn of(instrument: &Instrument) -> Self {
        match instrument {
            Instrument::Harmonica(_) => Self::Harmonica,
            Instrument::Guitar(_) => Self::Guitar,
            Instrument::Piano(_) => Self::Piano,
            Instrument::Flute(_) => Self::Flute,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Harmonica => "editor-instrument-harmonica",
            Self::Guitar => "editor-instrument-guitar",
            Self::Piano => "editor-instrument-piano",
            Self::Flute => "editor-instrument-flute",
        }
    }
    pub fn selection(self) -> InstrumentSelection {
        let instrument = match self {
            Self::Harmonica => {
                return InstrumentSelection::HarmonicaPreset(HarmonicaKind::Diatonic);
            }
            Self::Guitar => Guitar::standard(GuitarBody::Electric).into(),
            Self::Piano => Piano::grand().into(),
            Self::Flute => Flute::default().into(),
        };
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
                    &loc.msg(kind.label()),
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
