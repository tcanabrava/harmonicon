// SPDX-License-Identifier: MIT

pub mod chart;
pub mod harmonica;
pub mod midi;
pub mod note_parser;

mod loader;

use std::path::PathBuf;

pub use chart::HarpChart;
pub use harmonica::Harmonica;
pub use loader::SongChartLoader;

use bevy::{asset::AssetPath, audio::AudioSource, image::Image, prelude::*};

#[derive(Asset, TypePath, Debug)]
pub struct SongManifest {
    pub path: PathBuf,
    pub chart: HarpChart,
    /// Always present — a generated placeholder gradient when the song
    /// doesn't ship its own `background.png` (see `song::loader`), so this
    /// never needs to be optional.
    pub background: Handle<Image>,
    /// `None` when the song doesn't ship a `song/*.ogg` — a scored/jam
    /// session then simply plays no backing track (the chart-timed clock
    /// free-runs instead of anchoring to a sink; see `gameplay::
    /// should_anchor_to_sink`), rather than failing to load.
    pub music: Option<Handle<AudioSource>>,
    /// Peak-amplitude waveform of `music`, pre-analyzed at load time (see
    /// `audio_system::waveform`) so the gameplay progress bar can draw it
    /// immediately instead of decoding audio on the main thread mid-setup.
    pub waveform: Vec<f32>,
    /// `music`'s real decoded duration in seconds — the timescale `waveform`
    /// is laid out on. Deliberately *not* the same thing as the gameplay
    /// `SongEnd` (last chart note + a fixed tail): a tightly-trimmed track
    /// ends before that tail elapses, a padded one keeps going after it.
    /// Anything positioned over the waveform (the playhead, the loop-range
    /// marker) must use this, or it drifts out of sync with the waveform
    /// it's drawn on top of.
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
            .register_asset_loader(SongChartLoader);
    }
}
