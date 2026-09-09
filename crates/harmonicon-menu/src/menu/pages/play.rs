// SPDX-License-Identifier: MIT

//! The Play menu: choose Play Song, Create Song, Jam Session, Bending
//! Trainer, or Lessons.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_fluent::Localization;

use harmonicon_app::app::AppState;
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

pub(crate) fn setup_play_menu(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, header, _page_root) =
        spawn_menu_root(&mut commands, &loc.msg("menu-play"), None, &theme, "Play");
    // The render mode is chosen up front, before picking a song.
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-song"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::ModeSelect),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-create-song"),
        |_: On<Activate>, mut state: ResMut<NextState<AppState>>| state.set(AppState::SongEditor2),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-session"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::JamSessionMenu),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("bending-trainer"),
        |_: On<Activate>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::BendingTrainer)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-lessons"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::LessonTree),
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}
