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
    fn default() -> Self { Self("default".into()) }
}

impl Plugin for AssetsManagementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableSongs>()
            .init_resource::<AvailableHarmonicas>()
            .init_resource::<SelectedHarmonicaModel>()
            .add_systems(Startup, (scan_all_songs, scan_harmonica_models, load_global_fonts));
    }
}

fn load_global_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("Loading global fonts...");
    commands.insert_resource(GlobalFonts {
        gameplay: FontSource::Handle(asset_server.load("fonts/UbuntuSansMono-Regular.otf")),
        symbols: FontSource::Handle(asset_server.load("fonts/NotoSansSymbols-Regular.ttf")),
    });
}

fn scan_harmonica_models(mut available: ResMut<AvailableHarmonicas>) {
    let root = std::path::Path::new("assets/harmonicas/3d");
    let Ok(entries) = std::fs::read_dir(root) else {
        warn!("No harmonica models directory at assets/harmonicas/3d/");
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) { continue; }
        if !entry.path().join("harmonica.glb").exists() { continue; }
        available.0.push(entry.file_name().to_string_lossy().into_owned());
    }
    available.0.sort_unstable();
    info!("Found {} harmonica model(s): {:?}", available.0.len(), available.0);
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
