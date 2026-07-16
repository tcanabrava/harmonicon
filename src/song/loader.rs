// SPDX-License-Identifier: MIT

use bevy::{
    asset::{AssetLoader, AssetPath, LoadContext, RenderAssetUsages, io::Reader},
    audio::AudioSource,
    image::Image,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use thiserror::Error;

use super::{
    SongManifest,
    chart::{CURRENT_FORMAT_VERSION, HarpChart, format_version_supported},
};

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

        // Catch a chart authored for a newer spec than this build's loader
        // understands up front, with a clear message — rather than either
        // silently misreading a field whose meaning later changed, or
        // failing on some confusing downstream `additionalProperties`
        // schema error instead. See `chart::format_version_supported`.
        let declared_version = chart
            .metadata
            .as_ref()
            .and_then(|m| m.format_version.as_deref());
        if !format_version_supported(declared_version, CURRENT_FORMAT_VERSION) {
            return Err(SongLoadError::Validation(format!(
                "chart declares metadata.format_version {declared:?}, which this build's loader \
                 (understands up to {CURRENT_FORMAT_VERSION}) can't load — update Harmonicon, or \
                 fix the chart's declared version if it was set in error",
                declared = declared_version.unwrap_or("<missing>"),
            )));
        }

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

        // Every sibling asset below is checked for existence with
        // `read_asset_bytes` *before* being handed to `load_context.load()`.
        // `load()` registers the path as a hard dependency of this manifest
        // asset; if that path doesn't exist, the dependency fails and
        // `AssetServer::is_loaded_with_dependencies` never returns true for
        // the manifest — `menu::check_loading` would then wait on
        // `SongLoading` forever. Falling back instead (a generated
        // background, no music, `Handle::default()` for the unused
        // `elements`) is what lets a song ship with only a chart file, like
        // `Example Song 3`.
        let background_path = sibling(song_folder.join("background.png"));
        let background = match load_context.read_asset_bytes(background_path.clone()).await {
            Ok(_) => load_context.load::<Image>(background_path),
            Err(_) => {
                let seed = format!("{}\u{0}{}", chart.song.artist, chart.song.title);
                load_context.add_labeled_asset(
                    "generated_background".to_string(),
                    generate_background_image(&seed),
                )
            }
        };

        let elements_path = sibling(song_folder.join("elements.png"));
        let elements = match load_context.read_asset_bytes(elements_path.clone()).await {
            Ok(_) => load_context.load::<Image>(elements_path),
            Err(_) => Handle::default(),
        };

        // Pre-analyze the waveform here (asset load time, off the main
        // thread) so the progress bar has it ready the instant the song
        // starts. The same read doubles as the existence check: no
        // `song/*.ogg` (or, as a fallback, `song/*.wav` — the Song Editor's
        // MIDI import writes one of these, a synthesized backing track,
        // since the engine can't play a raw `.mid` and no OGG encoder is in
        // the dependency tree; see `song_editor::midi_import`) means no
        // backing track rather than a load failure — see
        // `SongManifest::music`'s doc comment.
        let ogg_path = sibling(song_folder.join("song/music.ogg"));
        let wav_path = sibling(song_folder.join("song/music.wav"));
        let (music, waveform, music_duration_secs) =
            match load_context.read_asset_bytes(ogg_path.clone()).await {
                Ok(bytes) => {
                    let (waveform, duration) = crate::audio_system::waveform::analyze_ogg_waveform(
                        &bytes,
                        crate::audio_system::waveform::WAVEFORM_BUCKETS,
                    );
                    (
                        Some(load_context.load::<AudioSource>(ogg_path)),
                        waveform,
                        duration,
                    )
                }
                Err(_) => match load_context.read_asset_bytes(wav_path.clone()).await {
                    Ok(bytes) => {
                        let (waveform, duration) =
                            crate::audio_system::waveform::analyze_wav_waveform(
                                &bytes,
                                crate::audio_system::waveform::WAVEFORM_BUCKETS,
                            );
                        (
                            Some(load_context.load::<AudioSource>(wav_path)),
                            waveform,
                            duration,
                        )
                    }
                    Err(_) => (None, Vec::new(), 0.0),
                },
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
            music_duration_secs,
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

/// Side (px) of a generated placeholder background — small and stretched to
/// fill the screen (like every other background), so the gradient reads as
/// smooth rather than needing to be full resolution.
const GENERATED_BACKGROUND_SIZE: u32 = 64;

/// A vertical two-color gradient for a song that doesn't ship its own
/// `background.png`, generated in memory (never touches disk, so it can
/// never fail to load — see the dependency-hang note where this is called).
/// `seed` (the song's own artist/title) picks the hue deterministically, so
/// two songs without art still look distinct from each other rather than
/// identical gray boxes, and the same song looks the same every run.
fn generate_background_image(seed: &str) -> Image {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let hue = (hasher.finish() % 360) as f32;

    let top = Color::hsl(hue, 0.35, 0.16).to_srgba();
    let bottom = Color::hsl((hue + 40.0) % 360.0, 0.35, 0.06).to_srgba();

    let mut data = Vec::with_capacity((GENERATED_BACKGROUND_SIZE * GENERATED_BACKGROUND_SIZE * 4) as usize);
    for y in 0..GENERATED_BACKGROUND_SIZE {
        let t = y as f32 / (GENERATED_BACKGROUND_SIZE - 1) as f32;
        let pixel = [
            (top.red + (bottom.red - top.red) * t).clamp(0.0, 1.0) * 255.0,
            (top.green + (bottom.green - top.green) * t).clamp(0.0, 1.0) * 255.0,
            (top.blue + (bottom.blue - top.blue) * t).clamp(0.0, 1.0) * 255.0,
            255.0,
        ]
        .map(|c| c as u8);
        for _ in 0..GENERATED_BACKGROUND_SIZE {
            data.extend_from_slice(&pixel);
        }
    }

    Image::new(
        Extent3d {
            width: GENERATED_BACKGROUND_SIZE,
            height: GENERATED_BACKGROUND_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_background_image_is_the_expected_size_and_fully_opaque() {
        let image = generate_background_image("Some Artist\u{0}Some Song");
        let data = image.data.expect("generated image should have pixel data");
        assert_eq!(
            data.len(),
            (GENERATED_BACKGROUND_SIZE * GENERATED_BACKGROUND_SIZE * 4) as usize
        );
        assert!(data.chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn generate_background_image_is_deterministic_per_seed() {
        let a = generate_background_image("Same Seed");
        let b = generate_background_image("Same Seed");
        assert_eq!(a.data, b.data);
    }

    #[test]
    fn generate_background_image_varies_between_different_seeds() {
        let a = generate_background_image("Artist One\u{0}Song One");
        let b = generate_background_image("Artist Two\u{0}Song Two");
        // Not a strict guarantee for every possible pair (hashes can
        // collide), but true for these two — catches a seed that's
        // accidentally ignored entirely.
        assert_ne!(a.data, b.data);
    }
}
