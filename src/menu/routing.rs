// SPDX-License-Identifier: MIT

//! The menu's own sub-state ([`MenuPage`]) and the systems that route
//! between pages: `route_menu_entry` (deciding which page `AppState::Menu`
//! lands on), `check_loading` (the `SongLoading` → `Playing` gate), and
//! `handle_menu_escape` (Back-navigation on Esc).

use bevy::prelude::*;

use crate::app::{
    AppState, GameplayMode, ReturnToHelpAbout, ReturnToOptions, ReturnToPlay, ReturnToSongList,
    SelectedSong,
};

use super::pages::tutorial;

// ── Menu sub-states (only active while AppState == Menu) ──────────────────────

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Menu)]
pub(crate) enum MenuPage {
    #[default]
    Main,
    Play,
    ArtistList,
    SongList,
    ModeSelect,
    Options,
    Theme,
    /// Curriculum list, grouped by unit (see `crate::lessons`).
    Lessons,
    /// One lesson's instructional page (+ Start for chart-backed lessons).
    LessonReader,
    /// The "Jam Session" choice on the Play menu lands here first: pick a
    /// real song (`ArtistList`) or synthesize one (`JamGenerate`) — see
    /// `pages::jam_session::setup_jam_session_menu`.
    JamSessionMenu,
    /// Key/tempo picker for a synthesized Jam Session backing (see
    /// `crate::jam::backing`) — the no-existing-song alternative to
    /// `ArtistList`'s real-song Jam Session flow.
    JamGenerate,
    /// Documentation link, About, Tutorial, and Credits — see
    /// `pages::help_about::setup_help_about_menu`.
    HelpAbout,
    /// Static "what is this app" page, reached from `HelpAbout`.
    About,
}

/// Escape navigates back one level in the menu hierarchy, mirroring each
/// page's own "Back" button target. `Main` has no parent, so it's a no-op
/// there.
pub(crate) fn handle_menu_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    page: Res<State<MenuPage>>,
    mode: Res<GameplayMode>,
    mut next_page: ResMut<NextState<MenuPage>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    let target = match page.get() {
        MenuPage::Main => return,
        MenuPage::Play | MenuPage::Options | MenuPage::HelpAbout => MenuPage::Main,
        MenuPage::Lessons | MenuPage::JamSessionMenu => MenuPage::Play,
        MenuPage::ModeSelect => MenuPage::Play,
        // Shared by two flows — Play Song (via ModeSelect) and Jam
        // Session's "Pick a Song" — see the Back button in
        // `pages::artist_list::setup_artist_list` for why `GameplayMode` is
        // what disambiguates.
        MenuPage::ArtistList => match *mode {
            GameplayMode::JamSession => MenuPage::JamSessionMenu,
            GameplayMode::Play2D | GameplayMode::Play3D => MenuPage::ModeSelect,
        },
        MenuPage::JamGenerate => MenuPage::JamSessionMenu,
        MenuPage::SongList => MenuPage::ArtistList,
        MenuPage::Theme => MenuPage::Options,
        MenuPage::LessonReader => MenuPage::Lessons,
        MenuPage::About => MenuPage::HelpAbout,
    };
    next_page.set(target);
}

pub(crate) fn check_loading(
    selected: Res<SelectedSong>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if asset_server.is_loaded_with_dependencies(selected.0.id()) {
        info!("Song loaded — starting game");
        next_state.set(AppState::Playing);
    }
}

/// On entering the menu, jump straight to the song list if we just quit a song
/// (so "Quit Song" returns to the list, not the main menu). Otherwise the menu
/// opens on its default page (Main), unless a return-to flag says otherwise.
/// A finished/quit *lesson* run returns to the lesson list instead, and its
/// [`LessonContext`] ends here — the menu is the boundary where a lesson run
/// stops being in flight (Results→Retry never passes through the menu, so
/// retries keep their context).
pub(crate) fn route_menu_entry(
    tour: Option<Res<tutorial::TutorialTour>>,
    lesson: Option<Res<crate::lessons::LessonContext>>,
    generated_jam: Option<Res<crate::jam::backing::GeneratedJamSession>>,
    mut ret_song: ResMut<ReturnToSongList>,
    mut ret_opts: ResMut<ReturnToOptions>,
    mut ret_play: ResMut<ReturnToPlay>,
    mut ret_help: ResMut<ReturnToHelpAbout>,
    mut next_page: ResMut<NextState<MenuPage>>,
    mut commands: Commands,
) {
    // The guided tour takes full priority over every other routing flag
    // below while it's running — a tour step re-entering `Menu` (e.g. after
    // a live-gameplay step) must land wherever *the tour* says, not
    // wherever an unrelated, possibly-stale flag from before the tour
    // started says. Those flags are left untouched (not consumed) rather
    // than acted on, so whichever one would have applied still does once
    // the tour actually ends.
    if let Some(tour) = tour {
        next_page.set(tutorial::tour_menu_landing(&tour));
        if tutorial::tour_finished(&tour) {
            commands.remove_resource::<tutorial::TutorialTour>();
        }
        return;
    }
    if lesson.is_some() {
        commands.remove_resource::<crate::lessons::LessonContext>();
        // "Quit Song" sets this unconditionally; for a lesson run the lesson
        // list is the right place to land, so the flag is consumed here.
        ret_song.0 = false;
        next_page.set(MenuPage::Lessons);
    } else if generated_jam.is_some() {
        commands.remove_resource::<crate::jam::backing::GeneratedJamSession>();
        // Same reasoning as the lesson branch above: a generated jam never
        // went through the song list, so land back on its own setup page
        // instead (ready to jam again with one click).
        ret_song.0 = false;
        next_page.set(MenuPage::JamGenerate);
    } else if ret_song.0 {
        ret_song.0 = false;
        next_page.set(MenuPage::SongList);
    } else if ret_opts.0 {
        ret_opts.0 = false;
        next_page.set(MenuPage::Options);
    } else if ret_play.0 {
        ret_play.0 = false;
        next_page.set(MenuPage::Play);
    } else if ret_help.0 {
        ret_help.0 = false;
        next_page.set(MenuPage::HelpAbout);
    }
}
