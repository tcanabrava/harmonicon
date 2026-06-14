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
        let parent = load_context
            .path()
            .path()
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();

        let background = load_context.load::<Image>(parent.join("background.png"));
        let music = load_context.load::<AudioSource>(parent.join("music.ogg"));
        let elements = load_context.load::<Image>(parent.join("elements.png"));

        Ok(SongManifest {
            chart,
            background,
            music,
            elements,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["harpchart"]
    }
}