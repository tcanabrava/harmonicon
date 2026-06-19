// SPDX-License-Identifier: MIT

pub mod chart;
pub mod harmonica;
mod loader;

use std::path::PathBuf;

pub use chart::HarpChart;
pub use harmonica::Harmonica;
pub use loader::SongChartLoader;

use bevy::{audio::AudioSource, image::Image, prelude::*};

#[derive(Asset, TypePath, Debug)]
pub struct SongManifest {
    pub path: PathBuf,
    pub chart: HarpChart,
    pub background: Handle<Image>,
    pub music: Handle<AudioSource>,
    pub elements: Handle<Image>,
    pub assets_2d: Option<Handle<Image>>,
    pub assets_2d_config: NoteThemeConfig,
    pub assets_3d: Option<String>,
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
        Self { x: 0.0, y: 0.0, width: 100.0, height: 100.0 }
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

pub struct SongPlugin;

impl Plugin for SongPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SongManifest>()
            .register_asset_loader(SongChartLoader);
    }
}
