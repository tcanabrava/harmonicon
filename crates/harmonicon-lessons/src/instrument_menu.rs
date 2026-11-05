// SPDX-License-Identifier: MIT
//! Pick an instrument before reading its lesson manifests or translations.
use crate::{lesson_reader::SelectedLesson, lesson_tree};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_core::instrument::InstrumentKind;
use harmonicon_menu::menu::{MenuPage, scene};
use harmonicon_platform::lesson_selection::SelectedLessonInstrument;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::lessons::{AvailableLessons, LoadedLessonInstrument};

pub(crate) fn instrument_key(kind: InstrumentKind) -> &'static str {
    match kind {
        InstrumentKind::Harmonica => "song-instrument-harmonica",
        InstrumentKind::Guitar => "song-instrument-guitar",
        InstrumentKind::Piano => "song-instrument-piano",
        InstrumentKind::Flute => "song-instrument-flute",
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    mut selection: ResMut<SelectedLessonInstrument>,
    mut lessons: ResMut<AvailableLessons>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    selection.0 = None;
    lessons.0.clear();
    commands.insert_resource(SelectedLesson::default());
    commands.insert_resource(lesson_tree::CollapsedUnits::default());
    commands.insert_resource(lesson_tree::UnitExpansions::default());
    commands.insert_resource(lesson_tree::PendingCompaction::default());
    commands.insert_resource(lesson_tree::RelayoutRequest::default());
    commands.insert_resource(lesson_tree::PreviousUnitPositions::default());
    commands.insert_resource(lesson_tree::UnitSlides::default());
    commands.insert_resource(lesson_tree::LessonTreeViewport::default());
    commands.insert_resource(lesson_tree::PendingLessonFocus::default());
    let (root, header, _) = scene::spawn_menu_root(
        &mut commands,
        &loc.msg("lesson-instrument-title"),
        None,
        &theme,
        "LessonInstrument",
    );
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Text::new(loc.msg("lesson-instrument-help").to_string()),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            Node { max_width: Val::Px(620.0), margin: UiRect::bottom(Val::Px(16.0)), ..default() },
        ));
    });
    for kind in InstrumentKind::ALL {
        scene::spawn_button(
            &mut commands,
            root,
            &loc.msg(instrument_key(kind)),
            move |_: On<Activate>, mut selection: ResMut<SelectedLessonInstrument>| {
                selection.0 = Some(kind);
            },
        );
    }
    scene::spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>,
         mut page: ResMut<NextState<MenuPage>>,
         mut welcome: ResMut<harmonicon_app::app::WelcomeFlow>| {
            page.set(
                welcome
                    .back_target(MenuPage::Play, MenuPage::Welcome, |flow| flow.lesson_done = true),
            );
        },
    );
}

/// Both catalog scanning and localization merging run before this gate, so the
/// first tree frame uses the newly selected curriculum, including empty paths.
pub(crate) fn open_loaded(
    selection: Res<SelectedLessonInstrument>,
    loaded: Res<LoadedLessonInstrument>,
    mut page: ResMut<NextState<MenuPage>>,
) {
    if selection.0.is_some() && selection.0 == loaded.0 {
        page.set(MenuPage::LessonTree);
    }
}

#[derive(SystemParam)]
pub(crate) struct Curriculum<'w> {
    pub lessons: Res<'w, AvailableLessons>,
    pub selection: Res<'w, SelectedLessonInstrument>,
    pub loaded: Res<'w, LoadedLessonInstrument>,
    pub page: ResMut<'w, NextState<MenuPage>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_tree_waits_for_the_chosen_instrument_and_accepts_an_empty_loaded_catalog() {
        let mut app = App::new();
        app.init_resource::<SelectedLessonInstrument>()
            .init_resource::<LoadedLessonInstrument>()
            .init_resource::<NextState<MenuPage>>()
            .add_systems(Update, open_loaded);
        app.update();
        assert!(matches!(app.world().resource::<NextState<MenuPage>>(), NextState::Unchanged));
        app.world_mut().resource_mut::<SelectedLessonInstrument>().0 = Some(InstrumentKind::Flute);
        app.world_mut().resource_mut::<LoadedLessonInstrument>().0 =
            Some(InstrumentKind::Harmonica);
        app.update();
        assert!(matches!(app.world().resource::<NextState<MenuPage>>(), NextState::Unchanged));
        app.world_mut().resource_mut::<LoadedLessonInstrument>().0 = Some(InstrumentKind::Flute);
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<MenuPage>>(),
            NextState::Pending(MenuPage::LessonTree)
        ));
    }
}
