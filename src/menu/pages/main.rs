// SPDX-License-Identifier: MIT

//! The main menu: Play, Options, Help, Quit.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_main_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(&mut commands, &loc.msg("app-title"), None, &theme, "Main");
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-play"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-options"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-help"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::HelpAbout)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-quit"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        },
    );
}
