// SPDX-License-Identifier: MIT

//! Artist picker — shared by two flows: Play Song (via `ModeSelect`) and
//! Jam Session's "Pick a Song" (see `GameplayMode` disambiguating the Back
//! button's target).

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::{GameplayMode, SelectedArtist};
use crate::assets_management::{AvailableSongs, SongsRescanned};
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

/// `assets_management::watch` rescans `AvailableSongs` live when
/// `~/Harmonicon/songs` changes, but that alone doesn't touch this page's
/// already-spawned button list. If the Artist List page happens to be open
/// when that happens, force a rebuild the same way any other same-page
/// transition does — `NextState::set` re-fires `OnExit`/`OnEnter` even for a
/// same-state transition (see `CLAUDE.md`) — so a song dropped in while this
/// page is on screen appears without the player needing to back out and back
/// in. Driven by `SongsRescanned`, not `AvailableSongs::is_changed()`: this
/// system only runs while the page is open, so its own change-detection tick
/// goes stale every time the page is closed, and would otherwise read as
/// "changed" the moment the page re-opens even when nothing outside this
/// system's own dormant window actually happened — a message fired only from
/// the watcher's own rescan call site has no such staleness problem.
pub(crate) fn rebuild_on_songs_rescanned(
    mut rescanned: MessageReader<SongsRescanned>,
    mut page: ResMut<NextState<MenuPage>>,
) {
    if rescanned.read().next().is_some() {
        page.set(MenuPage::ArtistList);
    }
}
