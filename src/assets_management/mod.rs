// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource)]
pub struct GlobalFonts {
    pub gameplay: FontSource,
    pub symbols: FontSource,
}

pub struct AssetsManagementPlugin;

#[derive(Debug, Clone)]
pub struct SongEntry {
    pub artist: String,
    pub name: String,
    pub asset_path: String,
}

/// Songs indexed by artist name. Each artist maps to a sorted list of songs.
#[derive(Resource, Default)]
pub struct AvailableSongs(pub HashMap<String, Vec<SongEntry>>);

/// Names of harmonica 3D models found under `assets/harmonicas/3d/<name>/harmonica.glb`.
#[derive(Resource, Default)]
pub struct AvailableHarmonicas(pub Vec<String>);

/// The currently selected harmonica model name (subfolder under `assets/harmonicas/3d/`).
#[derive(Resource)]
pub struct SelectedHarmonicaModel(pub String);

impl Default for SelectedHarmonicaModel {
    fn default() -> Self {
        Self("default".into())
    }
}

/// One clickable hole overlay box on a 3D harmonica model, in the model's local
/// space. Part of [`HarmonicaModelConfig`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HoleConfig {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Width along the X axis.
    pub w: f32,
    /// Height along the Y axis.
    pub h: f32,
    /// Depth along the Z axis.
    pub d: f32,
}

/// Placement of a 3D harmonica model and its hole overlays, loaded from
/// `assets/harmonicas/3d/<name>/holes.json`. Shared by the 3D gameplay view and
/// the `hole-editor` tool so the on-disk schema has a single definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HarmonicaModelConfig {
    /// World-space translation for the GLB scene root.
    pub model_translation: [f32; 3],
    /// Y-axis rotation applied to the GLB scene, in degrees.
    #[serde(default)]
    pub model_rotation_y_deg: f32,
    /// Uniform scale applied to the GLB scene.
    #[serde(default = "default_model_scale")]
    pub model_scale: f32,
    /// One entry per hole; index 0 = hole 1, index 9 = hole 10.
    pub holes: Vec<HoleConfig>,
}

pub fn default_model_scale() -> f32 {
    1.0
}

/// UI themes found under `assets/themes/<name>/theme.json`.
#[derive(Resource, Default)]
pub struct AvailableThemes(pub Vec<String>);

/// The currently selected UI theme name (subfolder under `assets/themes/`).
#[derive(Resource)]
pub struct SelectedTheme(pub String);

impl Default for SelectedTheme {
    fn default() -> Self {
        Self("default".into())
    }
}

/// 2D note themes found under `assets/notes/2d/<name>.png` (each paired with a
/// `<name>.json` tail layout). The string is the bare `<name>`.
#[derive(Resource, Default)]
pub struct AvailableNoteThemes2d(pub Vec<String>);

/// 3D note themes found under `assets/notes/3d/<name>.glb` (each paired with a
/// `<name>.json` cube layout). The string is the bare `<name>`.
#[derive(Resource, Default)]
pub struct AvailableNoteThemes3d(pub Vec<String>);

/// The currently selected 2D note theme. 2D and 3D themes are chosen separately
/// since the available drawings differ between the two views.
#[derive(Resource)]
pub struct SelectedNoteTheme2d(pub String);

impl Default for SelectedNoteTheme2d {
    fn default() -> Self {
        Self("circular".into())
    }
}

/// The currently selected 3D note theme (the cube/glTF head).
#[derive(Resource)]
pub struct SelectedNoteTheme3d(pub String);

impl Default for SelectedNoteTheme3d {
    fn default() -> Self {
        Self("circular".into())
    }
}

impl Plugin for AssetsManagementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableSongs>()
            .init_resource::<AvailableHarmonicas>()
            .init_resource::<SelectedHarmonicaModel>()
            .init_resource::<AvailableNoteThemes2d>()
            .init_resource::<AvailableNoteThemes3d>()
            .init_resource::<SelectedNoteTheme2d>()
            .init_resource::<SelectedNoteTheme3d>()
            .init_resource::<AvailableThemes>()
            .init_resource::<SelectedTheme>()
            .add_systems(
                Startup,
                (
                    scan_all_songs,
                    scan_harmonica_models,
                    scan_note_themes,
                    scan_ui_themes,
                    load_global_fonts,
                ),
            );
    }
}

fn scan_note_themes(
    mut available_2d: ResMut<AvailableNoteThemes2d>,
    mut available_3d: ResMut<AvailableNoteThemes3d>,
) {
    available_2d.0 = scan_theme_dir("assets/notes/2d", "png");
    available_3d.0 = scan_theme_dir("assets/notes/3d", "glb");
    info!(
        "Found note themes — 2D: {:?}  3D: {:?}",
        available_2d.0, available_3d.0
    );
}

/// Collects the `<name>` stems of files with `ext` directly under `dir`.
fn scan_theme_dir(dir: &str, ext: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        warn!("No note themes directory at {dir}/");
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        // Match the exact extension; skips editor backups like `circular.png~`.
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ext))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

fn load_global_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("Loading global fonts...");
    commands.insert_resource(GlobalFonts {
        gameplay: FontSource::Handle(asset_server.load("fonts/UbuntuSansMono-Regular.otf")),
        symbols: FontSource::Handle(asset_server.load("fonts/NotoSansSymbols-Regular.ttf")),
    });
}

fn scan_ui_themes(mut available: ResMut<AvailableThemes>) {
    let root = std::path::Path::new("assets/themes");
    let Ok(entries) = std::fs::read_dir(root) else {
        warn!("No themes directory at assets/themes/; defaulting to \"default\"");
        available.0.push("default".into());
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if !entry.path().join("theme.json").exists() {
            continue;
        }
        available.0.push(entry.file_name().to_string_lossy().into_owned());
    }
    available.0.sort_unstable();
    if available.0.is_empty() {
        available.0.push("default".into());
    }
    info!("Found {} UI theme(s): {:?}", available.0.len(), available.0);
}

fn scan_harmonica_models(mut available: ResMut<AvailableHarmonicas>) {
    let root = std::path::Path::new("assets/harmonicas/3d");
    let Ok(entries) = std::fs::read_dir(root) else {
        warn!("No harmonica models directory at assets/harmonicas/3d/");
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if !entry.path().join("harmonica.glb").exists() {
            continue;
        }
        available
            .0
            .push(entry.file_name().to_string_lossy().into_owned());
    }
    available.0.sort_unstable();
    info!(
        "Found {} harmonica model(s): {:?}",
        available.0.len(),
        available.0
    );
}

pub fn scan_all_songs(mut available: ResMut<AvailableSongs>) {
    let songs_root = std::path::Path::new("assets/songs");
    let Ok(artists) = std::fs::read_dir(songs_root) else {
        warn!("No songs directory found at assets/songs/");
        return;
    };

    for artist_dir in artists.flatten() {
        if !artist_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let artist = artist_dir.file_name().to_string_lossy().into_owned();
        let Ok(song_dirs) = std::fs::read_dir(artist_dir.path()) else {
            continue;
        };
        for song_dir in song_dirs.flatten() {
            if !song_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            if !song_dir.path().join("chart.harpchart").exists() {
                continue;
            }
            let name = song_dir.file_name().to_string_lossy().into_owned();
            available
                .0
                .entry(artist.clone())
                .or_default()
                .push(SongEntry {
                    asset_path: format!("songs/{artist}/{name}/chart.harpchart"),
                    artist: artist.clone(),
                    name,
                });
        }
    }

    let total: usize = available.0.values().map(|v| v.len()).sum();
    info!(
        "Found {} song(s) across {} artist(s)",
        total,
        available.0.len()
    );
}
