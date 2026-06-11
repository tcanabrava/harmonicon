pub mod chart;
mod loader;

pub use chart::HarpChart;
pub use loader::SongChartLoader;

use std::collections::HashMap;
use bevy::{audio::AudioSource, image::Image, prelude::*};

#[derive(Asset, TypePath, Debug)]
pub struct SongManifest {
    pub chart: HarpChart,
    pub background: Handle<Image>,
    pub music: Handle<AudioSource>,
    pub elements: Handle<Image>,
    /// Audio handles keyed by modifier type name (e.g. `"bend"`, `"vibrato"`).
    /// Loaded from the chart's `fx_mapping` field.
    pub fx_sounds: HashMap<String, Handle<AudioSource>>,
}

pub struct SongPlugin;

impl Plugin for SongPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SongManifest>()
            .register_asset_loader(SongChartLoader);
    }
}
