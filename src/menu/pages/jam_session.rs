// SPDX-License-Identifier: MIT

//! The "Jam Session" choice: pick a real song (`ArtistList`) or synthesize
//! one (`JamGenerate` — see `pages::jam_generate`).

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::{GameplayMode, JamProgression};
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::song::harmonica::Progression;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_jam_session_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("jam-session"),
        None,
        &theme,
        "JamSessionMenu",
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-session-pick-song"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>,
         mut mode: ResMut<GameplayMode>,
         mut progression: ResMut<JamProgression>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::JamSession;
            // A real song always plays its own actual chords regardless of
            // this resource (see `twelve_bar_blues_overlay::update_bar`),
            // but reset it anyway so a stale pick from an earlier generated
            // jam can't linger and confuse anyone reading the resource value.
            progression.0 = Progression::Standard;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-generate"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::JamGenerate)
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
