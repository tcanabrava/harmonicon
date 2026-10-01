// SPDX-License-Identifier: MIT

//! Lesson-facing UI: the curriculum tree, its pure layout engine, and the
//! lesson reader. Lesson manifests, discovery, and progress remain in
//! `harmonicon-song`; this crate owns how that domain is presented.

use bevy::prelude::*;

use harmonicon_menu::menu::{MenuPage, scene};

mod lesson_reader;
mod lesson_tree;

/// Whether the skill tree has a colour of its own for `track`. A track
/// without one still draws, in the shared grey every unknown track gets, so
/// `validate-pack` reports it as a warning: two such tracks in one pack are
/// indistinguishable on the tree.
pub fn track_has_colour(track: &str) -> bool {
    lesson_tree::declared_track_color(track).is_some()
}

/// Registers the two lesson pages and their page-local state.
pub struct LessonsUiPlugin;

impl Plugin for LessonsUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(lesson_tree::LessonEdgeMaterialPlugin)
            .init_resource::<lesson_reader::SelectedLesson>()
            .init_resource::<lesson_tree::CollapsedUnits>()
            .init_resource::<lesson_tree::UnitExpansions>()
            .init_resource::<lesson_tree::PendingCompaction>()
            .init_resource::<lesson_tree::RelayoutRequest>()
            .init_resource::<lesson_tree::CanvasSize>()
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
                    // First, so a cluster a relayout opens is already
                    // scaled to zero by the expansion pass on the same frame.
                    lesson_tree::relayout_tree,
                    lesson_tree::animate_unit_expansion,
                    lesson_tree::compact_finished_units,
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
                (
                    lesson_reader::update_lesson_metronomes,
                    lesson_reader::update_lesson_phrase_loopers,
                )
                    .run_if(in_state(MenuPage::LessonReader)),
            )
            .add_systems(
                OnExit(MenuPage::LessonReader),
                (lesson_reader::cleanup_lesson_audio, scene::cleanup_menu),
            );
    }
}
