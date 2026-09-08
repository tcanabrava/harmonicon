// SPDX-License-Identifier: MIT

//! App-wide vocabulary: the top-level state machine ([`AppState`]), the
//! gameplay mode selector ([`GameplayMode`]), the currently-selected
//! song/artist, and the cross-state `ReturnTo*` routing flags.
//!
//! Pure data plus the trivial run conditions over it — every feature
//! (gameplay, song editor, spectrogram, profile, menu) shares this level;
//! nothing here imports a feature.

use bevy::prelude::*;

use harmonicon_core::chart::{HarpChart, Scale};
use harmonicon_core::harmonica::{Harmonica, Progression};
use harmonicon_core::harp_remap::HarpMapping;
use harmonicon_song::song::SongManifest;

// ── App-level states ──────────────────────────────────────────────────────────

/// `Reflect` so the Bevy Remote Protocol can *drive* the app from outside —
/// `world.mutate_resources` on `NextState<AppState>` moves between screens
/// with no synthetic input at all. That's what captures the user guide's
/// screenshots (`contributing/src/remote-control.md`); BRP reaches nothing that isn't
/// reflected and registered.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash, Reflect)]
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

/// The scale Jam Session's live hole-map feedback (`jam::session::
/// JamHoleGuide`) judges played notes against — see `song::chart::Scale`.
/// Defaults to `FirstPosition` (the blues hexatonic — unchanged Jam
/// Session behavior before this resource existed). Set explicitly by
/// "Generate Jam" (`menu::jam_generate`) and by a jam-based lesson's
/// `scale` manifest field (`menu::pages::lessons::parse_scale`); the
/// real-song "Jam Session" button resets it to `FirstPosition`, mirroring
/// `JamProgression`'s own reset — though a real song's own declared
/// `Harmonica::scale()` (if it sets one) still wins over this resource,
/// see `jam::session::build_hole_guide`'s caller.
#[derive(Resource, Default)]
pub struct JamScale(pub Scale);

/// Whether Jam Session should periodically call a new position (cycling
/// `JamScale` through First/Second/Third position) — see
/// `jam::position_guide`. Only ever `true` for a jam-based lesson that opts
/// in via its manifest's `position_cycle` field
/// (`menu::pages::lessons::setup_lesson_reader`'s Start handler); the
/// real-song "Jam Session" button resets it to `false`, mirroring
/// `JamProgression`/`JamScale`'s own reset, so a previous lesson's cycling
/// can't leak into an ordinary jam.
#[derive(Resource, Default)]
pub struct JamPositionCycle(pub bool);

/// Present whenever the current `SelectedSong` was built in memory and
/// handed to `Assets::add`, rather than loaded through the `AssetServer`.
///
/// Such a handle has **no tracked `LoadState`**, so routing through
/// `AppState::SongLoading` would hang there forever waiting on
/// `check_loading`'s `is_loaded_with_dependencies`. Every route that would
/// otherwise pass through `SongLoading` checks this and goes straight to
/// `Playing` instead — `gameplay::pause_menu::on_restart` today.
///
/// Separate from [`GeneratedJamSession`] on purpose. That one also means
/// "this is a jam" — it picks the menu page to return to and gates the
/// rhythm guide — whereas this one is only ever a statement about how the
/// asset was made. A generated *training* is equally not-asset-loaded and
/// emphatically not a jam, and conflating the two would have sent it back
/// to the jam setup page.
#[derive(Resource)]
pub struct GeneratedSong;

/// Present while a generated-backing jam is in flight (from the "Start Jam"
/// button through `Playing`, including any Restart). Means *this is a jam*:
/// `menu::route_menu_entry` uses it to land back on the jam setup page, and
/// `jam::session`/`jam::rhythm_guide` gate their own widgets on it.
///
/// For the "was built by `Assets::add`" half, see [`GeneratedSong`], which
/// is inserted alongside it. Removed on returning to the menu, same
/// end-of-life point `LessonContext` uses.
#[derive(Resource)]
pub struct GeneratedJamSession;

/// Set while the guided tutorial tour (`menu::pages::tutorial`) is driving
/// the app automatically. Every screen the tour passes through
/// (`gameplay::pause_menu`, the Bending Trainer, the Song Editor's grid
/// keys) gates its own Escape/pause handling on this, so the tour's
/// click-blocking overlay isn't the only thing keeping the player from
/// steering it off course — "Skip Tutorial" is the one deliberate way out.
///
/// The tour's real state (`TutorialTour`: step, timer, return page) stays in
/// `menu`, which is the only writer; this flag is *derived* from it every
/// frame by `menu::pages::tutorial::sync_tour_active`. It lives down here
/// because `gameplay` and `song_editor` sit below `menu` and may not import
/// upward (`docs/physical_design_plan.md` rule 2).
#[derive(Resource, Default)]
pub struct TourActive(pub bool);

/// True while a guided tour is running — see [`TourActive`].
pub fn tour_active(tour: Res<TourActive>) -> bool {
    tour.0
}

// ── Selection resources ───────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SelectedSong(pub Handle<SongManifest>);

/// The harmonica the player will actually put to their mouth, when it isn't
/// the one the chart was written for.
///
/// `None` means "play the chart's own harp", which is the default and the
/// overwhelmingly common case — so nothing has to populate this when a song
/// loads, and the feature costs nothing until someone opts in. Resolve it
/// against a chart with [`Self::harp_for`] rather than reading the field:
/// that keeps the fallback in one place.
///
/// **Everything the microphone depends on must resolve through here.** A
/// chart's expected pitches, `PitchRange` and `ValidHarpNotes` all used to
/// come straight off `chart.harmonica`; if any one of them keeps doing that
/// while the others don't, the game listens for notes the player's harp
/// cannot make. `harmonicon_core::harp_remap` documents the same invariant
/// from the pure side.
#[derive(Resource, Default, Clone, Debug)]
pub struct EffectiveHarmonica {
    pub harp: Option<Harmonica>,
    pub mapping: HarpMapping,
}

impl EffectiveHarmonica {
    /// The harp to actually use for `chart` — the player's choice, or the
    /// chart's own when they haven't made one.
    pub fn harp_for<'a>(&'a self, chart: &'a HarpChart) -> &'a Harmonica {
        self.harp.as_ref().unwrap_or(&chart.harmonica)
    }

    /// Whether the player has chosen a harp other than the chart's.
    pub fn is_substituted(&self) -> bool {
        self.harp.is_some()
    }

    /// Back to the chart's own harmonica. Called when a song ends, so one
    /// song's substitution can't leak into the next.
    pub fn clear(&mut self) {
        self.harp = None;
        self.mapping = HarpMapping::default();
    }
}

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
