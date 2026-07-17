// SPDX-License-Identifier: MIT

//! App-wide vocabulary: the top-level state machine ([`AppState`]), the
//! gameplay mode selector ([`GameplayMode`]), the currently-selected
//! song/artist, and the cross-state `ReturnTo*` routing flags.
//!
//! Pure data, no systems — every feature (gameplay, song editor,
//! spectrogram, profile, menu) shares this level; nothing here imports a
//! feature.

use bevy::prelude::*;

use crate::song::SongManifest;
use crate::song::harmonica::Progression;

// ── App-level states ──────────────────────────────────────────────────────────

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Startup,
    Menu,
    SongLoading,
    Playing,
    /// Post-song results / statistics screen.
    Results,
    /// Latency calibration screen (outside the menu sub-state hierarchy).
    Calibration,
    /// Credits screen with scrolling text and 3D harmonica background.
    Credits,
    /// Song authoring tool, launched from the main menu.
    SongEditor2,
    /// Standalone bending practice: harmonica bend diagram + metronome, with a
    /// directly pickable key and adjustable tempo (no song).
    BendingTrainer,
}

#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub enum GameplayMode {
    #[default]
    Play2D,
    Play3D,
    /// Free-play: the 12-bar chart + metronome, no falling notes.
    JamSession,
}

/// The 12-bar variant Jam Session's grid/hole-map/(for a generated jam)
/// backing audio all follow — see `song::harmonica::Progression`. Only ever
/// anything but `Standard` for a "Generate Jam" session (`menu::
/// jam_generate` sets it explicitly on Start); the real-song "Jam Session"
/// button resets it to `Standard` so a previous generated jam's pick can't
/// leak into a real song (which always plays its own actual chords,
/// regardless of this resource — see `twelve_bar_blues_overlay::update_bar`).
#[derive(Resource, Default)]
pub struct JamProgression(pub Progression);

// ── Selection resources ───────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SelectedSong(pub Handle<SongManifest>);

#[derive(Resource, Default)]
pub struct SelectedArtist(pub String);

// ── Cross-state routing flags ─────────────────────────────────────────────────
//
// Crossing an `AppState` boundary back into `Menu` can't set
// `NextState<MenuPage>` directly — it loses to the substate machinery
// resetting to its own default first — so exits set one of these flags and
// `menu::route_menu_entry` consumes it on arrival.

/// Set to `true` by the pause menu's "Quit Song" button so that re-entering
/// `AppState::Menu` lands on the song list rather than the main menu.
#[derive(Resource, Default)]
pub struct ReturnToSongList(pub bool);

/// Set to `true` by the calibration screen so that returning to `AppState::Menu`
/// lands on the Options page (where the Input lag slider lives).
#[derive(Resource, Default)]
pub struct ReturnToOptions(pub bool);

/// Set to `true` by the Song Editor (`AppState::SongEditor2`) on every exit
/// path so that returning to `AppState::Menu` lands on the Play page (where
/// "Create Song" lives) rather than the substate's own default of Main.
#[derive(Resource, Default)]
pub struct ReturnToPlay(pub bool);

/// Set to `true` by the Credits screen (`AppState::Credits`) on every exit
/// path so that returning to `AppState::Menu` lands on the Help/About page
/// (where "Credits" lives) rather than the substate's own default of Main.
#[derive(Resource, Default)]
pub struct ReturnToHelpAbout(pub bool);
