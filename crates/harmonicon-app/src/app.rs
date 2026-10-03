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
use harmonicon_core::harmonica::{Harmonica, Progression, detected_harp_key, key_offset, semitone};
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
    /// Downloading lesson or song packs that have never been installed;
    /// the menu opens once they are (`menu::pages::content_sync`).
    Syncing,
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
/// Defaults to `FirstPosition` (the blues hexatonic). Set explicitly by
/// "Generate Jam" (`menu::jam_generate`) and by a jam-based lesson's
/// `scale` manifest field (`menu::pages::lesson_reader::parse_scale`); the
/// real-song "Jam Session" button resets it to `FirstPosition`, mirroring
/// `JamProgression`'s own reset — though a real song's own declared
/// `Harmonica::scale()` (if it sets one) still wins over this resource,
/// see `jam::hole_map::build_hole_guide`'s caller.
#[derive(Resource, Default)]
pub struct JamScale(pub Scale);

/// Whether Jam Session should periodically call a new position (cycling
/// `JamScale` through First/Second/Third position) — see
/// `jam::position_guide`. Only ever `true` for a jam-based lesson that opts
/// in via its manifest's `position_cycle` field
/// (`menu::pages::lesson_reader::setup_lesson_reader`'s Start handler); the
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
pub struct GeneratedJamSession {
    /// Reproduces the generated band's small performance variations when the
    /// same jam is restarted or its next four-chorus buffer is queued.
    pub seed: u64,
}

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
/// **Everything the microphone depends on must resolve through here.** If
/// a chart's expected pitches, `PitchRange` or `ValidHarpNotes` read
/// `chart.harmonica` directly while the others don't, the game listens for
/// notes the player's harp cannot make. `harmonicon_core::harp_remap` documents the same invariant
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

    /// The key the music actually sounds in. Under `HarpMapping::SameHoles`
    /// the tab is kept and the tune moves with the harp — a C chart played
    /// on a G harp sounds in G — so anything that *names* the key (the
    /// "grab a G harmonica · key of …" hint) has to say so; under
    /// `Transpose`, or with no substitution, it's the chart's own key.
    /// Falls back to the chart's key when either harp's key can't be read
    /// off its hole-1 blow note.
    pub fn song_key_for(&self, chart: &HarpChart) -> String {
        let chart_key = chart.song.key.as_str();
        let Some(played) = self.harp.as_ref().filter(|_| self.mapping == HarpMapping::SameHoles)
        else {
            return chart_key.to_string();
        };
        match (detected_harp_key(&chart.harmonica), detected_harp_key(played)) {
            (Some(from), Some(to)) => semitone(chart_key, key_offset(&to) - key_offset(&from)),
            _ => chart_key.to_string(),
        }
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

/// The first-run welcome flow's state for this session. The Welcome page
/// offers three steps — microphone setup (Options), the guided tour, a
/// first lesson — and a player who takes one should land back on Welcome
/// afterwards to take the next, not on Main or Play where those pages
/// ordinarily return: `return_to_welcome` is set by the Welcome page's
/// buttons and consumed by the destination page's Back/Escape. The `*_done`
/// flags mark the steps taken, so the page can show a check beside them
/// when the player comes back. Reset with the session; the profile file's
/// existence is what decides whether Welcome shows at all (`FirstRun`).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WelcomeFlow {
    pub return_to_welcome: bool,
    pub mic_done: bool,
    pub tour_done: bool,
    pub lesson_done: bool,
}

impl WelcomeFlow {
    /// Where a page's Back/Escape should go: Welcome if the player came
    /// from there (consuming the flag), else `default`. `done` marks which
    /// step the page was.
    pub fn back_target<P>(&mut self, default: P, welcome: P, done: impl FnOnce(&mut Self)) -> P {
        if self.return_to_welcome {
            self.return_to_welcome = false;
            done(self);
            welcome
        } else {
            default
        }
    }
}

/// Set to `true` by the Credits screen (`AppState::Credits`) on every exit
/// path so that returning to `AppState::Menu` lands on the Help/About page
/// (where "Credits" lives) rather than the substate's own default of Main.
#[derive(Resource, Default)]
pub struct ReturnToHelpAbout(pub bool);

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::harmonica::richter_harp;

    fn chart_in(key: &str, harp_key: &str) -> HarpChart {
        let mut chart: HarpChart = serde_json::from_str(
            r#"{
            "song": { "title": "T", "artist": "A", "genre": "Test", "tempo_bpm": 120.0,
                      "key": "C", "difficulty": "easy" },
            "timing": { "resolution": 480, "tempo_map": [{"tick": 0, "bpm": 120.0}] },
            "harmonica": { "type": "diatonic", "holes": 10,
                           "bending_profile": "richter_standard",
                           "layout": { "blow": ["C4"], "draw": ["D4"] } },
            "track": [],
            "scoring": { "perfect_window_ms": 50, "good_window_ms": 100,
                         "miss_window_ms": 130 }
        }"#,
        )
        .unwrap();
        chart.song.key = key.to_string();
        chart.harmonica = richter_harp(harp_key);
        chart
    }

    #[test]
    fn the_sounding_key_moves_with_the_harp_only_under_same_holes() {
        // A chart in C for a C harp, played on a G harp.
        let chart = chart_in("C", "C");
        let on_g = |mapping| EffectiveHarmonica { harp: Some(richter_harp("G")), mapping };
        assert_eq!(on_g(HarpMapping::SameHoles).song_key_for(&chart), "G");
        assert_eq!(on_g(HarpMapping::Transpose).song_key_for(&chart), "C");
        assert_eq!(EffectiveHarmonica::default().song_key_for(&chart), "C");
    }

    #[test]
    fn the_offset_is_between_the_harps_not_from_the_song_key() {
        // A tune in G on a C harp (2nd position), played on an A harp: the
        // harp moved up a major sixth (C → A), so the tune moves G → E.
        let chart = chart_in("G", "C");
        let on_a =
            EffectiveHarmonica { harp: Some(richter_harp("A")), mapping: HarpMapping::SameHoles };
        assert_eq!(on_a.song_key_for(&chart), "E");
    }
}
