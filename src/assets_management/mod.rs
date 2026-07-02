// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use std::{
    collections::HashMap,
    fs::DirEntry,
};

pub struct AssetsManagementPlugin;

#[derive(Debug, Clone)]
// Struct representing a song entry in the menu
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
                    override_default_font,
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

/// Replace Bevy's built-in default font (FiraMono) with GNU FreeSans, so text
/// spawned without an explicit `TextFont.font` — including `bsn!` UI, which can't
/// set it in 0.19 — renders normally. FreeSans covers, in one sans face, full
/// Latin, arrows, and the common BMP note glyphs (`♩ ♪ ♫ ♬`), so mixed
/// text+symbol runs render without relying on parley's per-glyph fallback. (The
/// SMP whole/half note glyphs aren't in any sans font, so those durations show a
/// word instead — see `dur_symbol`.) Embedded so it's ready at startup.
fn override_default_font(mut fonts: ResMut<Assets<Font>>) {
    const BYTES: &[u8] = include_bytes!("../../assets/fonts/FreeSans.otf");
    if let Err(err) = fonts.insert(&Handle::<Font>::default(), Font::from_bytes(BYTES.to_vec())) {
        warn!("Could not install default font: {err}");
    }
}

/// Collects the names of subfolders under `root` that contain a `theme.json`.
fn scan_theme_names(root: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| e.path().join("theme.json").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

// Discovers UI themes from the bundled `assets/themes/` directory, plus the
// external `~/Harmonicon/themes/` drop folder if present (see `load_theme` in
// `theme.rs`, which does the matching bundled-first resolution when loading).
fn scan_ui_themes(mut available: ResMut<AvailableThemes>) {
    let mut names = scan_theme_names(std::path::Path::new("assets/themes"));
    if names.is_empty() {
        warn!("No themes directory at assets/themes/; defaulting to \"default\"");
    }

    if let Some(external_root) = dirs::home_dir().map(|h| h.join("Harmonicon/themes")) {
        names.extend(scan_theme_names(&external_root));
    }

    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        names.push("default".into());
    }
    info!("Found {} UI theme(s): {:?}", names.len(), names);
    available.0 = names;
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

/// `source_prefix` is prepended to the built `SongEntry::asset_path` so it
/// loads from the right [`AssetSource`](bevy::asset::io::AssetSource): empty
/// for the bundled `assets/` root, or `"external://"` for the `~/Harmonicon`
/// drop folder registered under that source name in `main.rs`.
pub fn scan_artist_song(
    artist_dir: &DirEntry,
    available: &mut ResMut<AvailableSongs>,
    source_prefix: &str,
) {
    let Ok(song_dirs) = std::fs::read_dir(artist_dir.path()) else {
        return;
    };

    let artist = artist_dir.file_name().to_string_lossy().into_owned();
    for song_dir in song_dirs.flatten() {
        if !song_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        // Inside of the music directory, there may be a 3d, 2d models, and a song subfolder containing
        // the actual song files and chart.harpchart metadata file.
        if !song_dir.path().join("song/chart.harpchart").exists() {
            continue;
        }

        let name = song_dir.file_name().to_string_lossy().into_owned();

        available
            .0
            .entry(artist.clone())
            .or_default()
            .push(SongEntry {
                asset_path: format!("{source_prefix}songs/{artist}/{name}/song/chart.harpchart"),
                artist: artist.clone(),
                name,
            });
    }
}

/// Walks `songs_root` (bundled `assets/songs` or the external
/// `~/Harmonicon/songs` drop folder) and scans each artist subfolder into
/// `available`, tagging entries with `source_prefix` so they load from the
/// matching [`AssetSource`](bevy::asset::io::AssetSource).
fn scan_songs_root(songs_root: &std::path::Path, source_prefix: &str, available: &mut ResMut<AvailableSongs>) {
    let Ok(artists) = std::fs::read_dir(songs_root) else {
        return;
    };

    for artist_dir in artists.flatten() {
        if !artist_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        scan_artist_song(&artist_dir, available, source_prefix);
    }
}

// Scans the bundled songs directory, plus the external `~/Harmonicon/songs`
// drop folder if present, for harmonica models and songs, per artist. The
// external folder is optional — most players won't have one — so its absence
// is not a warning, unlike the bundled directory always shipped with the game.
pub fn scan_all_songs(mut available: ResMut<AvailableSongs>) {
    let bundled_root = std::path::Path::new("assets/songs");
    if bundled_root.is_dir() {
        scan_songs_root(bundled_root, "", &mut available);
    } else {
        warn!("No songs directory found at assets/songs/");
    }

    if let Some(external_root) = dirs::home_dir().map(|h| h.join("Harmonicon/songs")) {
        scan_songs_root(&external_root, "external://", &mut available);
    }

    let total: usize = available.0.values().map(|v| v.len()).sum();
    info!(
        "Found {} song(s) across {} artist(s)",
        total,
        available.0.len()
    );
}
