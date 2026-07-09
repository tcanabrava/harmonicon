// SPDX-License-Identifier: MIT

use bevy::{
    asset::{AssetLoader, AssetPath, LoadContext, io::Reader},
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
            .parent() // songs/{artist}/{name}/song/
            .unwrap_or(std::path::Path::new(""))
            .parent() // songs/{artist}/{name}/
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();

        // The source this manifest itself was loaded from (bundled `assets/`,
        // or the external `~/Harmonicon` drop folder registered as
        // `external://`). Sibling loads below must reuse it explicitly: a bare
        // `PathBuf`/`&str` always resolves against the *default* source, so
        // without this, a song loaded from `external://...` would have its
        // manifest parsed correctly but its music/images silently looked up in
        // the bundled folder instead.
        let source = load_context.path().source().clone_owned();
        let sibling = |rel: std::path::PathBuf| AssetPath::from(rel).with_source(source.clone());

        let background = load_context.load::<Image>(sibling(song_folder.join("background.png")));
        let music_path = sibling(song_folder.join("song/music.ogg"));
        let music = load_context.load::<AudioSource>(music_path.clone());
        let elements = load_context.load::<Image>(sibling(song_folder.join("elements.png")));

        // Pre-analyze the waveform here (asset load time, off the main
        // thread) rather than at gameplay setup, so the progress bar has it
        // ready the instant the song starts — no synchronous decode competing
        // with note-track setup.
        let waveform = match load_context.read_asset_bytes(music_path).await {
            Ok(bytes) => crate::audio_system::waveform::analyze_ogg_waveform(
                &bytes,
                crate::audio_system::waveform::WAVEFORM_BUCKETS,
            ),
            Err(_) => Vec::new(),
        };

        // Note the song's own 2D image path if it ships one. We deliberately do
        // NOT `load()` it here: that would make it a manifest dependency, kept
        // resident for the whole song regardless of mode. gameplay_2d::setup
        // loads it on demand when entering a 2D game (and it frees on exit).
        let png_rel = sibling(song_folder.join("2d/note_2d.png"));
        let assets_2d: Option<AssetPath<'static>> =
            match load_context.read_asset_bytes(png_rel.clone()).await {
                Ok(_) => Some(png_rel),
                Err(_) => None,
            };

        // Try to read the note layout from the song's own folder; if it has
        // none, fall back to the default circular.json layout. The fallback
        // path is a bare string with no `source`, so it always resolves
        // against the bundled `assets/` source — shared defaults live there
        // regardless of where the song itself came from.
        let note_2d_json = match load_context
            .read_asset_bytes(sibling(song_folder.join("2d/note_2d.json")))
            .await
        {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => {
                let res = load_context
                    .read_asset_bytes("notes/2d/circular.json")
                    .await;
                String::from_utf8_lossy(&res.unwrap_or_default()).to_string()
            }
        };

        // Note the song's own 3D GLB path if present (without loading it, same
        // reasoning as 2D above). gameplay_3d::setup loads it with the
        // `#Mesh0/Primitive0` label when entering a 3D game; otherwise it falls
        // back to the selected theme's default mesh.
        let glb_rel = sibling(song_folder.join("3d/note_3d.glb"));
        let assets_3d: Option<AssetPath<'static>> =
            match load_context.read_asset_bytes(glb_rel.clone()).await {
                Ok(_) => Some(glb_rel),
                Err(_) => None,
            };

        // 3D note layout: the song's own json if present, else the default
        // circular.json layout (bundled source, same reasoning as 2D above).
        let note_3d_json = match load_context
            .read_asset_bytes(sibling(song_folder.join("3d/note_3d.json")))
            .await
        {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => {
                let res = load_context
                    .read_asset_bytes("notes/3d/circular.json")
                    .await;
                String::from_utf8_lossy(&res.unwrap_or_default()).to_string()
            }
        };

        Ok(SongManifest {
            path: song_folder,
            chart,
            background,
            music,
            waveform,
            elements,
            assets_2d,
            assets_2d_config: serde_json::from_str(&note_2d_json).unwrap_or_default(),
            assets_3d,
            assets_3d_config: serde_json::from_str(&note_3d_json).unwrap_or_default(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["harpchart"]
    }
}
