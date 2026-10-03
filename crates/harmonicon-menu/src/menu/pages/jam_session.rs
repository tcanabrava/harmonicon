// SPDX-License-Identifier: MIT

//! The "Jam Session" choice: pick a real song (`ArtistList`) or synthesize
//! one (`JamGenerate` — see `pages::jam_generate`).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{GameplayMode, JamPositionCycle, JamProgression, JamScale};
use harmonicon_core::chart::Scale;
use harmonicon_core::harmonica::Progression;
use harmonicon_jam::jam::backing::{Genre, JamGenre};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

pub(crate) fn setup_jam_session_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, header, _page_root) =
        spawn_menu_root(&mut commands, &loc.msg("jam-session"), None, &theme, "JamSessionMenu");
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-session-pick-song"),
        |_: On<Activate>,
         mut mode: ResMut<GameplayMode>,
         mut progression: ResMut<JamProgression>,
         mut scale: ResMut<JamScale>,
         mut genre: ResMut<JamGenre>,
         mut position_cycle: ResMut<JamPositionCycle>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::JamSession;
            // Clear generated-jam settings before selecting a real song.
            progression.0 = Progression::Standard;
            scale.0 = Scale::FirstPosition;
            genre.0 = Genre::Blues;
            position_cycle.0 = false;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-generate"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::JamGenerate),
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
