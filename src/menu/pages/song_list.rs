// SPDX-License-Identifier: MIT

//! Song picker for the currently-selected artist. The render mode/jam
//! choice is already made by the time the player reaches this page —
//! picking a song starts the game.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::{AppState, SelectedArtist, SelectedSong};
use crate::assets_management::AvailableSongs;
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::song::SongManifest;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

pub(crate) fn setup_song_list(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    selected_artist: Res<SelectedArtist>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let subtitle = format!("by {}", selected_artist.0);
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("select-song"),
        Some(&subtitle),
        &theme,
        "SongList",
    );

    if let Some(artist_songs) = songs.0.get(&selected_artist.0) {
        let mut sorted = artist_songs.clone();
        sorted.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        for song in &sorted {
            let path = song.asset_path.clone();
            // The mode is already chosen — picking a song starts the game.
            spawn_button(
                &mut commands,
                root,
                &song.name,
                &theme,
                &btn_mats,
                move |_: On<Pointer<Click>>,
                      asset_server: Res<AssetServer>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(path.clone()),
                    ));
                    state.set(AppState::SongLoading);
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
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::ArtistList)
        },
    );
}
