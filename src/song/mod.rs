// SPDX-License-Identifier: MIT

pub mod chart;
pub mod harmonica;
mod loader;

pub use chart::HarpChart;
pub use harmonica::Harmonica;
pub use loader::SongChartLoader;

use bevy::{audio::AudioSource, image::Image, prelude::*};

#[derive(Asset, TypePath, Debug)]
pub struct SongManifest {
    pub chart: HarpChart,
    pub background: Handle<Image>,
    pub music: Handle<AudioSource>,
    pub elements: Handle<Image>,
}

pub struct SongPlugin;

impl Plugin for SongPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SongManifest>()
            .register_asset_loader(SongChartLoader);
    }
}