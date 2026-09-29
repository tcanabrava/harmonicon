// SPDX-License-Identifier: MIT

//! Artist picker — shared by two flows: Play Song (via `ModeSelect`) and
//! Jam Session's "Pick a Song" (see `GameplayMode` disambiguating the Back
//! button's target).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{GameplayMode, SelectedArtist};
use harmonicon_platform::assets_management::{AvailableSongs, SongsRescanned};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

pub(crate) fn setup_artist_list(
    mut commands: Commands,

    songs: Res<AvailableSongs>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, header, _page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("select-artist"),
        None,
        &theme,
        "ArtistList",
    );

    if songs.0.is_empty() {
        let msg = commands
            .spawn_empty()
            .apply_scene(bsn! {
                Text({String::from(loc.msg("no-songs-found"))})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::srgb(0.8, 0.4, 0.4)})
            })
            .id();
        commands.entity(root).add_child(msg);
    } else {
        let mut artists: Vec<_> = songs.0.iter().collect();
        artists.sort_unstable_by_key(|(artist, _)| *artist);
        for (artist, artist_songs) in artists {
            let n = artist_songs.len();
            let count = loc.msg_args(
                if n == 1 {
                    "artist-song-count-one"
                } else {
                    "artist-song-count-many"
                },
                &[("n", n.to_string())],
            );
            let label = format!("{artist}  ({count})");
            let artist = artist.clone();
            spawn_button(
                &mut commands,
                root,
                &label,
                move |_: On<Activate>,
                      mut selected: ResMut<SelectedArtist>,
                      mut page: ResMut<NextState<MenuPage>>| {
                    selected.0 = artist.clone();
                    page.set(MenuPage::SongList);
                },
            );
        }
    }
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        // ArtistList is shared by two flows — Play Song (via ModeSelect) and
        // Jam Session's "Pick a Song" — that set `GameplayMode` before
        // navigating here, so it doubles as the flag for which page Back
        // should return to.
        |_: On<Activate>, mode: Res<GameplayMode>, mut page: ResMut<NextState<MenuPage>>| {
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
/// page is open appears without backing out and back in. Driven by
/// `SongsRescanned`, not `AvailableSongs::is_changed()`: this system only
/// runs while the page is open, so its own change-detection tick would
/// otherwise read stale-as-changed on every re-entry; a message fired only
/// from the watcher's own rescan call site has no such problem.
pub(crate) fn rebuild_on_songs_rescanned(
    mut rescanned: MessageReader<SongsRescanned>,
    mut page: ResMut<NextState<MenuPage>>,
) {
    if rescanned.read().next().is_some() {
        page.set(MenuPage::ArtistList);
    }
}
