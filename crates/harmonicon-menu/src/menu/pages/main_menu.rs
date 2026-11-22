// SPDX-License-Identifier: MIT

//! The main menu: Play, Options, Help, and — everywhere but the browser —
//! Quit.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_main_menu(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, _header, _page_root) =
        spawn_menu_root(&mut commands, &loc.msg("app-title"), None, &theme);
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-play"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-options"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-help"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::HelpAbout),
    );
    // No Quit in a browser tab: a page can't close itself (`window.close()`
    // only works for windows script opened), and `AppExit` under wasm just
    // stops Bevy's loop — leaving a frozen canvas with no way back short of
    // a reload, which is worse than not offering the button. Android keeps
    // it: `AppExit` there finishes the activity, which is a real exit.
    #[cfg(not(target_arch = "wasm32"))]
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-quit"),
        |_: On<Activate>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        },
    );
}
