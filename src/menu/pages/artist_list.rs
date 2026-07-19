// SPDX-License-Identifier: MIT

//! Artist picker — shared by two flows: Play Song (via `ModeSelect`) and
//! Jam Session's "Pick a Song" (see `GameplayMode` disambiguating the Back
//! button's target).

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::{GameplayMode, SelectedArtist};
use crate::assets_management::{self, AvailableSongs};
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_artist_list(
    mut commands: Commands,

    songs: Res<AvailableSongs>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("select-artist"),
        None,
        &theme,
        "ArtistList",
    );

    if songs.0.is_empty() {
        let msg = commands
            .spawn((
                Text::new(loc.msg("no-songs-found")),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.4, 0.4)),
            ))
            .id();
        commands.entity(root).add_child(msg);
    } else {
        let mut artists: Vec<&String> = songs.0.keys().collect();
        artists.sort_unstable();
        for artist in artists {
            let n = songs.0[artist].len();
            let label = format!("{artist}  ({n} song{})", if n == 1 { "" } else { "s" });
            let artist = artist.clone();
            spawn_button(
                &mut commands,
                root,
                &label,
                &theme,
                &btn_mats,
                move |_: On<Pointer<Click>>,
                      mut selected: ResMut<SelectedArtist>,
                      mut page: ResMut<NextState<MenuPage>>| {
                    selected.0 = artist.clone();
                    page.set(MenuPage::SongList);
                },
            );
        }
    }
    spawn_button(
        &mut commands,
        root,
        &loc.msg("refresh-songs"),
        &theme,
        &btn_mats,
        on_refresh_songs,
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        &theme,
        &btn_mats,
        // ArtistList is shared by two flows — Play Song (via ModeSelect) and
        // Jam Session's "Pick a Song" — that set `GameplayMode` before
        // navigating here, so it doubles as the flag for which page Back
        // should return to.
        |_: On<Pointer<Click>>, mode: Res<GameplayMode>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(match *mode {
                GameplayMode::JamSession => MenuPage::JamSessionMenu,
                GameplayMode::Play2D | GameplayMode::Play3D => MenuPage::ModeSelect,
            });
        },
    );
}

/// Re-scans the bundled + external (`~/Harmonicon/songs`) song folders and
/// re-enters this same page to rebuild the list from the refreshed
/// `AvailableSongs` — `NextState::set` re-fires `OnExit`/`OnEnter` even for a
/// same-state transition (see `CLAUDE.md`), which is what makes a self-target
/// transition rebuild the page at all. Lets a song dropped into
/// `~/Harmonicon/songs` while the game is already running show up without a
/// restart.
fn on_refresh_songs(
    _: On<Pointer<Click>>,
    available: ResMut<AvailableSongs>,
    mut page: ResMut<NextState<MenuPage>>,
) {
    assets_management::scan_all_songs(available);
    page.set(MenuPage::ArtistList);
}
