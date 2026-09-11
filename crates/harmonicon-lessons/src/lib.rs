// SPDX-License-Identifier: MIT

//! Lesson-facing UI: the curriculum tree, its pure layout engine, and the
//! lesson reader. Lesson manifests, discovery, and progress remain in
//! `harmonicon-song`; this crate owns how that domain is presented.

use bevy::prelude::*;

use harmonicon_menu::menu::{MenuPage, scene};

mod lesson_reader;
mod lesson_tree;

/// Registers the two lesson pages and their page-local state.
pub struct LessonsUiPlugin;

impl Plugin for LessonsUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(lesson_tree::LessonEdgeMaterialPlugin)
            .init_resource::<lesson_reader::SelectedLesson>()
            .init_resource::<lesson_tree::CollapsedUnits>()
            .init_resource::<lesson_tree::UnitExpansions>()
            .init_resource::<lesson_tree::PendingCompaction>()
            .init_resource::<lesson_tree::PendingViewportAnchor>()
            .init_resource::<lesson_tree::PreviousUnitPositions>()
            .init_resource::<lesson_tree::UnitSlides>()
            .init_resource::<lesson_tree::LessonTreeViewport>()
            .init_resource::<lesson_tree::PendingLessonFocus>()
            .add_systems(
                OnEnter(MenuPage::LessonTree),
                lesson_tree::setup_lesson_tree,
            )
            .add_systems(OnExit(MenuPage::LessonTree), scene::cleanup_menu)
            .add_systems(
                Update,
                (
                    lesson_tree::rebuild_on_lessons_rescanned,
                    lesson_tree::animate_unit_expansion,
                    lesson_tree::compact_finished_units,
                    lesson_tree::restore_viewport_anchor,
                    lesson_tree::focus_pending_lesson,
                    lesson_tree::animate_unit_slides,
                    lesson_tree::remember_viewport,
                )
                    .chain()
                    .run_if(in_state(MenuPage::LessonTree)),
            )
            .add_systems(
                OnEnter(MenuPage::LessonReader),
                lesson_reader::setup_lesson_reader,
            )
            .add_systems(
                Update,
                lesson_reader::update_lesson_metronomes.run_if(in_state(MenuPage::LessonReader)),
            )
            .add_systems(
                OnExit(MenuPage::LessonReader),
                (lesson_reader::cleanup_lesson_audio, scene::cleanup_menu),
            );
    }
}
