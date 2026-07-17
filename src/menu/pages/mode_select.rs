// SPDX-License-Identifier: MIT

//! Render-mode picker (2D/3D) shown before picking a song to play.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::GameplayMode;
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_mode_select(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("select-mode"),
        None,
        &theme,
        "ModeSelect",
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-2d"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>,
         mut mode: ResMut<GameplayMode>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::Play2D;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-3d"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>,
         mut mode: ResMut<GameplayMode>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::Play3D;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
