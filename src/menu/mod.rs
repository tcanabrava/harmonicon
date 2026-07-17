// SPDX-License-Identifier: MIT

//! The menu shell: plugin wiring only. `routing.rs` owns [`MenuPage`] and
//! the systems that route between pages; `scene.rs` owns the shared
//! root/button scene helpers; `pages/` has one file per top-level page.

use bevy::prelude::*;

use crate::app::{
    AppState, GameplayMode, JamProgression, ReturnToHelpAbout, ReturnToOptions, ReturnToPlay,
    ReturnToSongList, SelectedArtist,
};
use crate::dialogs::button_material::ButtonMaterialPlugin;
use crate::song_editor;

mod pages;
pub(crate) mod routing;
mod scene;

pub(crate) use pages::tutorial;
pub(crate) use routing::MenuPage;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<SelectedArtist>()
            .init_resource::<pages::lessons::SelectedLesson>()
            .init_resource::<pages::lessons::SelectedUnitIx>()
            .add_message::<pages::lessons::LessonUnitChanged>()
            .init_resource::<pages::jam_generate::JamGenerateConfig>()
            .init_resource::<GameplayMode>()
            .init_resource::<JamProgression>()
            .init_resource::<ReturnToSongList>()
            .init_resource::<ReturnToOptions>()
            .init_resource::<ReturnToPlay>()
            .init_resource::<ReturnToHelpAbout>()
            // The Options, Calibration, Credits, and Theme pages own their own lifecycles.
            .add_plugins(ButtonMaterialPlugin)
            .add_plugins(pages::options::OptionsPlugin)
            .add_plugins(pages::calibration::CalibrationPlugin)
            .add_plugins(pages::credits::CreditsPlugin)
            .add_plugins(song_editor::SongEditor2Plugin)
            .add_plugins(pages::theme_picker::ThemePickerPlugin)
            .add_systems(OnEnter(AppState::Menu), routing::route_menu_entry)
            // Each page manages its own lifetime.
            .add_systems(OnEnter(MenuPage::Main), pages::main::setup_main_menu)
            .add_systems(OnExit(MenuPage::Main), scene::cleanup_menu)
            .add_systems(OnEnter(MenuPage::Play), pages::play::setup_play_menu)
            .add_systems(OnExit(MenuPage::Play), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::ArtistList),
                pages::artist_list::setup_artist_list,
            )
            .add_systems(OnExit(MenuPage::ArtistList), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::SongList),
                pages::song_list::setup_song_list,
            )
            .add_systems(OnExit(MenuPage::SongList), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::ModeSelect),
                pages::mode_select::setup_mode_select,
            )
            .add_systems(OnExit(MenuPage::ModeSelect), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::Lessons),
                pages::lessons::setup_lessons_menu,
            )
            .add_systems(OnExit(MenuPage::Lessons), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::LessonReader),
                pages::lessons::setup_lesson_reader,
            )
            .add_systems(OnExit(MenuPage::LessonReader), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::JamSessionMenu),
                pages::jam_session::setup_jam_session_menu,
            )
            .add_systems(OnExit(MenuPage::JamSessionMenu), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::JamGenerate),
                pages::jam_generate::setup_jam_generate_menu,
            )
            .add_systems(OnExit(MenuPage::JamGenerate), scene::cleanup_menu)
            .add_systems(
                Update,
                pages::jam_generate::update_jam_generate_labels
                    .run_if(in_state(MenuPage::JamGenerate)),
            )
            .add_systems(
                OnEnter(MenuPage::HelpAbout),
                pages::help_about::setup_help_about_menu,
            )
            .add_systems(OnExit(MenuPage::HelpAbout), scene::cleanup_menu)
            .add_systems(
                OnEnter(MenuPage::About),
                pages::help_about::setup_about_page,
            )
            .add_systems(OnExit(MenuPage::About), scene::cleanup_menu)
            .add_systems(
                Update,
                routing::check_loading.run_if(in_state(AppState::SongLoading)),
            )
            // Tab switches on the Lessons page write `SelectedUnitIx` and
            // fire `LessonUnitChanged`; this swaps the scrollbox rows in
            // response (message-gated, not resource-change-gated — see the
            // doc comment on `LessonUnitChanged`).
            .add_systems(
                Update,
                pages::lessons::repopulate_lesson_list.run_if(in_state(MenuPage::Lessons)),
            )
            // The guided tour drives `NextState<AppState>`/`NextState<
            // MenuPage>` itself on a timer, and some steps leave
            // `AppState::Menu` entirely (a live gameplay/Bending Trainer/
            // Song Editor look) — so unlike every other system here, these
            // two run unconditionally (each checks `Option<Res<
            // TutorialTour>>` itself) rather than being gated to one page
            // or even to `AppState::Menu`. The overlay sync must see the
            // tour resource change after it ticks, hence `.chain()`.
            .add_systems(
                Update,
                (
                    tutorial::advance_tutorial_tour,
                    tutorial::sync_tutorial_overlay,
                )
                    .chain(),
            )
            // If a combobox dropdown was open, its own Escape handler closes
            // it and consumes the keypress — this handler never sees it, so
            // one Escape press doesn't both close a dropdown and navigate
            // back a page. Also skipped while the guided tour is driving
            // page changes itself — Escape shouldn't fight it mid-tour;
            // Skip Tutorial is the deliberate way out.
            .add_systems(
                Update,
                routing::handle_menu_escape
                    .after(super::dialogs::combobox::close_open_comboboxes_on_escape)
                    .run_if(in_state(AppState::Menu).and_then(not(tutorial::tour_active))),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    // Records page enter/exit so the close-then-open order can be asserted.
    #[derive(Resource, Default)]
    struct PageLog(Vec<String>);

    fn track_page(app: &mut App, page: MenuPage, label: &'static str) {
        app.add_systems(OnEnter(page.clone()), move |mut log: ResMut<PageLog>| {
            log.0.push(format!("enter {label}"))
        });
        app.add_systems(OnExit(page), move |mut log: ResMut<PageLog>| {
            log.0.push(format!("exit {label}"))
        });
    }

    #[test]
    fn changing_page_exits_the_old_before_entering_the_new() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<PageLog>();
        track_page(&mut app, MenuPage::Main, "Main");
        track_page(&mut app, MenuPage::Play, "Play");

        // Enter the menu → its default page (Main) opens.
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        // Open Play (Main must close first), then go Back to Main (Play closes).
        app.world_mut()
            .resource_mut::<NextState<MenuPage>>()
            .set(MenuPage::Play);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<MenuPage>>()
            .set(MenuPage::Main);
        app.update();

        let log = &app.world().resource::<PageLog>().0;
        assert_eq!(
            log,
            &[
                "enter Main",
                "exit Main",
                "enter Play",
                "exit Play",
                "enter Main"
            ],
        );
    }
}
