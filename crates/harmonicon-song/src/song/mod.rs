// SPDX-License-Identifier: MIT

mod loader;

// The chart/harmonica/note types are pure logic and live in
// `harmonicon-core`; re-exported here so `harmonicon_core::chart::…` and
// friends keep resolving unchanged across the tree.
pub use harmonicon_core::midi_file as midi;
pub use harmonicon_core::{chart, harmonica, harmonica_constraints, note_parser};

use std::path::PathBuf;

pub use harmonicon_core::chart::HarpChart;
pub use harmonicon_core::harmonica::Harmonica;
mod score_song;

pub use loader::SongChartLoader;
pub use score_song::ScoreSongLoader;

pub use harmonicon_score::ScoreTrack;
pub use harmonicon_score::convert::{ConversionReport, TrackConversion as TrackChart};

use bevy::{asset::AssetPath, audio::AudioSource, image::Image, prelude::*};

#[derive(Asset, TypePath, Debug)]
pub struct SongManifest {
    pub path: PathBuf,
    pub chart: HarpChart,
    /// Always present — a generated placeholder gradient when the song
    /// doesn't ship its own `background.png` (see `song::loader`), so this
    /// never needs to be optional.
    pub background: Handle<Image>,
    /// `None` when the song doesn't ship a `song/*.ogg`/`*.wav` — a
    /// scored/jam session then plays no backing track (the chart-timed
    /// clock free-runs instead of anchoring to a sink; see `gameplay::
    /// should_anchor_to_sink`) rather than failing to load. Also `None`
    /// when the song ships `song/music.mid` instead — see
    /// [`midi_tracks`](Self::midi_tracks), which carries that song's
    /// backing audio in that case; the two are mutually exclusive.
    pub music: Option<Handle<AudioSource>>,
    /// Present when the song ships `song/music.mid` instead of a single
    /// pre-mixed `music.ogg`/`.wav` — one already-rendered `AudioSource`
    /// per non-empty MIDI track (`song::midi::render_track_pcm`, the same
    /// synth `song_editor::playback`/`gameplay::call_response` share), so a
    /// Jam Session can play every track as its own simultaneous sink and
    /// mute individual tracks by muting their sink — no live re-mixing
    /// needed. Only Jam Session's UI (`jam::session`) shows a mute row;
    /// scored Play2D/3D has nothing meaningful to do with a backing track
    /// regardless of stem count.
    pub midi_tracks: Option<Vec<MidiTrackAudio>>,
    /// Peak-amplitude waveform of `music` (or, when [`midi_tracks`]
    /// (Self::midi_tracks) is populated instead, every track's stems
    /// summed together — a display-only reference mix; actual playback
    /// sums them for real, as separate simultaneous sinks), pre-analyzed
    /// at load time (see `audio_system::waveform`) so the gameplay
    /// progress bar can draw it immediately instead of decoding audio on
    /// the main thread mid-setup.
    pub waveform: Vec<f32>,
    /// `music`'s (or the combined MIDI-track mix's) real decoded duration
    /// in seconds — the timescale `waveform` is laid out on. Deliberately
    /// *not* the same thing as the gameplay `SongEnd` (last chart note +
    /// a fixed tail): a tightly-trimmed track ends before that tail
    /// elapses, a padded one keeps going after it. Anything positioned
    /// over the waveform (the playhead, the loop-range marker) must use
    /// this, or it drifts out of sync with the waveform it's drawn on top
    /// of.
    pub music_duration_secs: f64,
    /// Unused by gameplay today (`jam::backing`'s `build_generated_manifest`
    /// notes this explicitly) — `Handle::default()` when the song doesn't
    /// ship an `elements.png`, so there's nothing to fail to load.
    pub elements: Handle<Image>,
    /// Asset path of the song's own 2D note image, if it ships one. Stored as
    /// a full [`AssetPath`] (not a `Handle`, and not a bare `PathBuf`) so the
    /// image is *not* a manifest dependency — it is loaded lazily by
    /// `gameplay_2d::setup` only when entering a 2D game, and freed when those
    /// note entities despawn on leaving — while still carrying the source the
    /// song itself was loaded from (bundled `assets/` vs. the external
    /// `~/Harmonicon` drop folder). `None` → use the theme default.
    pub assets_2d: Option<AssetPath<'static>>,
    pub assets_2d_config: NoteThemeConfig,
    /// Asset path of the song's own 3D note GLB, if it ships one. Lazily loaded
    /// by `gameplay_3d::setup` (with the `#Mesh0/Primitive0` label) the same way.
    pub assets_3d: Option<AssetPath<'static>>,
    pub assets_3d_config: NoteCube3dConfig,
    /// Every playable part of the source file, each already converted onto
    /// its own best-fitting harmonica — see [`TrackChart`].
    ///
    /// Empty for a `.harpchart`, which has exactly one part by
    /// construction. Populated by `score_song`, because a file written for
    /// a band names no harmonica part and the loader's own pick is then a
    /// guess: the harp-check screen turns this into a track picker so the
    /// player can correct it, and `source_track` says which one is
    /// currently in [`chart`](Self::chart).
    pub source_tracks: Vec<TrackChart>,
    /// Index *into [`source_tracks`](Self::source_tracks)* — not the source
    /// file's own track number, which `TrackChart::track.index` carries.
    /// `None` when there was never a choice to make.
    pub source_track: Option<usize>,
}

/// Wraps a chart built in memory as a [`SongManifest`], for handing to
/// `Assets::add` rather than the `AssetServer`.
///
/// A generated training has no folder and so no siblings: no backing track,
/// no waveform, no art. Every one of those is already optional (see this
/// module's own notes on `SongManifest`), so the drill plays against the
/// metronome with a default background and nothing has to special-case it.
///
/// The caller must also insert `app::GeneratedSong`, or the route through
/// `AppState::SongLoading` waits forever on a handle that has no
/// `LoadState`.
pub fn training_manifest(chart: HarpChart) -> SongManifest {
    SongManifest {
        path: PathBuf::from(format!("training/{}", chart.song.title)),
        chart,
        background: Handle::default(),
        music: None,
        midi_tracks: None,
        waveform: Vec::new(),
        music_duration_secs: 0.0,
        elements: Handle::default(),
        assets_2d: None,
        assets_2d_config: NoteThemeConfig::default(),
        assets_3d: None,
        assets_3d_config: NoteCube3dConfig::default(),
        source_tracks: Vec::new(),
        source_track: None,
    }
}

/// One MIDI track's own, independently-playable audio stem — see
/// [`SongManifest::midi_tracks`].
#[derive(Debug, Clone)]
pub struct MidiTrackAudio {
    /// The track's own name (`song::midi::track_name_of`), or `"Track
    /// <index>"` when the MIDI file doesn't name it — shown as the mute
    /// row's label in `jam::session`.
    pub name: String,
    pub source: Handle<AudioSource>,
}

/// Head image destination rect within the note's lane square, in percentages
/// (0..100). Lets a theme position/resize the disc inside its cell when the
/// source PNG isn't centred or doesn't fill the frame. `(0, 0, 100, 100)` fills
/// the square (the default, matching a perfectly-cropped sprite).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NoteHeadRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for NoteHeadRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }
}

/// fractions of the head image: `tail_x` is the tail's horizontal center,
/// `tail_y` the vertical attach point on the head (0 = top, 1 = bottom), and
/// `tail_width` the tail base width — all relative to the head's width/height.
/// `head` positions/resizes the disc image within the lane square.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NoteThemeConfig {
    pub tail_x: f32,
    pub tail_y: f32,
    pub tail_width: f32,
    #[serde(default)]
    pub head: NoteHeadRect,
}

impl Default for NoteThemeConfig {
    fn default() -> Self {
        Self {
            tail_x: 0.5,
            tail_y: 0.5,
            tail_width: 0.45,
            head: NoteHeadRect::default(),
        }
    }
}

/// Per-song 3D note layout. Loaded from the song's own `3d/note_3d.json` when it
/// ships one, otherwise the default theme's `notes/3d/<theme>.json`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NoteCube3dConfig {
    /// Uniform scale applied to the cube head (relative to a lane-wide note).
    pub head_scale: f32,
    /// Tail ribbon width as a fraction of the note width.
    pub tail_width: f32,
}

impl Default for NoteCube3dConfig {
    fn default() -> Self {
        Self {
            head_scale: 0.8,
            tail_width: 0.6,
        }
    }
}

pub struct SongPlugin;

impl Plugin for SongPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SongManifest>()
            .register_asset_loader(SongChartLoader)
            // A score file dropped in *as* a song's chart, converted at
            // load time via `harmonicon-score`. Registers for every format
            // that crate can read, so adding one needs no change here.
            // Distinct from `song/music.mid`, which is backing audio for a
            // `.harpchart` song — see `score_song`.
            .register_asset_loader(ScoreSongLoader);
    }
}
