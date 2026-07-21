// SPDX-License-Identifier: MIT

//! The Play menu: choose Play Song, Create Song, Jam Session, Bending
//! Trainer, or Lessons.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::AppState;
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_play_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(&mut commands, &loc.msg("menu-play"), None, &theme, "Play");
    // The render mode is chosen up front, before picking a song.
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-song"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::ModeSelect)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-create-song"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::SongEditor2)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-session"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::JamSessionMenu)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("bending-trainer"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::BendingTrainer)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-lessons"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Lessons),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}
