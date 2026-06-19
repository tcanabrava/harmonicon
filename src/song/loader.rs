// SPDX-License-Identifier: MIT

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    audio::AudioSource,
    image::Image,
    prelude::*,
};
use thiserror::Error;

use super::{SongManifest, chart::HarpChart};

const SCHEMA: &str = include_str!("../../assets/song_schema.dtd.json");

#[derive(Error, Debug)]
pub enum SongLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Schema is invalid: {0}")]
    Schema(String),
    #[error("Chart validation failed:\n{0}")]
    Validation(String),
}

#[derive(Default, TypePath)]
pub struct SongChartLoader;

impl AssetLoader for SongChartLoader {
    type Asset = SongManifest;
    type Settings = ();
    type Error = SongLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<SongManifest, SongLoadError> {
        // Read chart.json bytes.
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // Parse to a generic JSON value first so we can validate before deserializing.
        let chart_value: serde_json::Value = serde_json::from_slice(&bytes)?;

        // Validate against the embedded schema.
        let schema_value: serde_json::Value = serde_json::from_str(SCHEMA)?;
        let validator = jsonschema::validator_for(&schema_value)
            .map_err(|e| SongLoadError::Schema(e.to_string()))?;

        let errors: Vec<String> = validator
            .iter_errors(&chart_value)
            .map(|e| format!("  - {e} (at /{path})", path = e.instance_path))
            .collect();

        if !errors.is_empty() {
            return Err(SongLoadError::Validation(errors.join("\n")));
        }

        // Validation passed — deserialize into typed structs.
        let chart: HarpChart = serde_json::from_value(chart_value)?;

        // Materialise the parent path before calling load() to avoid holding
        // an immutable borrow on load_context across its mutable load() calls.
        let song_folder = load_context
            .path()
            .path()
            .parent()   // songs/{artist}/{name}/song/
            .unwrap_or(std::path::Path::new(""))
            .parent()// songs/{artist}/{name}/
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();

        let background = load_context.load::<Image>(song_folder.join("background.png"));
        let music = load_context.load::<AudioSource>(song_folder.join("song/music.ogg"));
        let elements = load_context.load::<Image>(song_folder.join("elements.png"));

        let assets_2d = match load_context
            .read_asset_bytes(song_folder.join("2d/note_2d.png"))
            .await
        {
            Ok(_) => Some(load_context.load::<Image>(song_folder.join("2d/note_2d.png"))),
            Err(_) => None,
        };

        // Try to read the note from the folder, if not, fall back to the default circular.json
        let note_2d_json = match load_context.read_asset_bytes(song_folder.join("2d/note_2d.json"))
            .await
        {
            Ok(bytes) => {
                println!("Load correct json");
                String::from_utf8_lossy(&bytes).to_string()
            }
            Err(_) => {
                println!("Load default json");
                let res = load_context.read_asset_bytes("notes/2d/circular.json").await;
                let res = res.unwrap_or_default();
                String::from_utf8_lossy(&res).to_string()
            }
        };

        let assets_3d: Option<String> = None;

        Ok(SongManifest {
            path: song_folder,
            chart,
            background,
            music,
            elements,
            assets_2d,
            assets_2d_config: serde_json::from_str(&note_2d_json).ok().expect("Could not parse note_2d.json"),
            assets_3d,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["harpchart"]
    }
}
