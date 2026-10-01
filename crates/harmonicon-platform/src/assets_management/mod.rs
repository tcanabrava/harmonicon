// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::DirEntry;

use crate::content_packs::{ContentPacks, ContentPacksChanged, ContentPacksSet};

mod watch;
pub use watch::ExternalFolderChanged;

/// Build-time-generated equivalent of a directory scan, for wasm: Bevy's
/// wasm `AssetReader` talks HTTP and can't list a directory the way
/// `std::fs::read_dir` can, so the scan functions below run at build time
/// instead (`build.rs`'s `generate_wasm_asset_manifest`) and this just
/// `include!()`s the result. Native builds don't use this at all — they keep
/// scanning `assets/`/`~/Harmonicon` for real at runtime, so a player can add
/// content without a rebuild.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
mod manifest {
    include!(concat!(env!("OUT_DIR"), "/asset_manifest.rs"));
}

pub struct AssetsManagementPlugin;

/// Fired only when a *live* filesystem event actually triggered a rescan of
/// `AvailableSongs` — distinct from that resource simply changing (which
/// also happens once, uneventfully, from the ordinary Startup scan). A menu
/// page that's already open needs exactly this distinction to tell "the
/// watcher just found something new, rebuild me" from "this resource merely
/// exists" (see `watch::ExternalFolderChanged`'s doc comment).
#[derive(Message)]
pub struct SongsRescanned;

/// The `themes/` sibling of [`SongsRescanned`].
#[derive(Message)]
pub struct ThemesRescanned;

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

/// Whether falling notes show their harmonica hole number instead of the
/// blow/draw arrow. Off (arrows) by default.
#[derive(Resource, Default)]
pub struct ShowNoteNumbers(pub bool);

impl Plugin for AssetsManagementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableSongs>()
            .init_resource::<AvailableHarmonicas>()
            .init_resource::<SelectedHarmonicaModel>()
            .init_resource::<AvailableNoteThemes2d>()
            .init_resource::<AvailableNoteThemes3d>()
            .init_resource::<SelectedNoteTheme2d>()
            .init_resource::<SelectedNoteTheme3d>()
            .init_resource::<ShowNoteNumbers>()
            .init_resource::<AvailableThemes>()
            .init_resource::<SelectedTheme>()
            .add_message::<watch::ExternalFolderChanged>()
            .add_message::<SongsRescanned>()
            .add_message::<ThemesRescanned>()
            // Read by `rescan_on_external_change`; registered here too so an
            // app without `ContentPacksPlugin` (tests, tools) still runs.
            .add_message::<ContentPacksChanged>()
            .add_systems(
                Startup,
                (
                    scan_all_songs.after(ContentPacksSet),
                    scan_harmonica_models,
                    scan_note_themes,
                    scan_ui_themes,
                    override_default_font,
                    watch::start_watching_external_folder,
                ),
            )
            .add_systems(
                Update,
                (
                    watch::process_external_folder_events,
                    rescan_on_external_change,
                )
                    .chain()
                    .after(ContentPacksSet),
            );
    }
}

/// Consumes `watch::ExternalFolderChanged` for the two kinds this module
/// owns (`songs`/`themes`), re-scanning + firing the matching `*Rescanned`
/// message for whichever actually changed. `lessons::catalog` has its own
/// sibling consumer of the same message for `lessons`. A pack being
/// installed, updated or removed (`ContentPacksChanged`) rescans songs too.
fn rescan_on_external_change(
    mut changed: MessageReader<ExternalFolderChanged>,
    mut packs_changed: MessageReader<ContentPacksChanged>,
    mut available_songs: ResMut<AvailableSongs>,
    available_themes: ResMut<AvailableThemes>,
    packs: Option<Res<ContentPacks>>,
    mut songs_rescanned: MessageWriter<SongsRescanned>,
    mut themes_rescanned: MessageWriter<ThemesRescanned>,
) {
    let mut dirty_songs = packs_changed.read().count() > 0;
    let mut dirty_themes = false;
    for ev in changed.read() {
        dirty_songs |= ev.top_level_dirs.contains("songs");
        dirty_themes |= ev.top_level_dirs.contains("themes");
    }

    if dirty_songs {
        scan_all_songs_into(&mut available_songs, packs.as_deref());
        songs_rescanned.write(SongsRescanned);
    }
    if dirty_themes {
        scan_ui_themes(available_themes);
        themes_rescanned.write(ThemesRescanned);
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
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
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
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

/// wasm sibling of the native `scan_note_themes` above: reads the build-time
/// manifest instead of scanning a directory the wasm `AssetReader` can't
/// list.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn scan_note_themes(
    mut available_2d: ResMut<AvailableNoteThemes2d>,
    mut available_3d: ResMut<AvailableNoteThemes3d>,
) {
    available_2d.0 = manifest::NOTE_THEMES_2D
        .iter()
        .map(|s| s.to_string())
        .collect();
    available_3d.0 = manifest::NOTE_THEMES_3D
        .iter()
        .map(|s| s.to_string())
        .collect();
    info!(
        "Found note themes — 2D: {:?}  3D: {:?}",
        available_2d.0, available_3d.0
    );
}

/// Replace Bevy's built-in default font (FiraMono) with GNU FreeSans, so text
/// spawned without an explicit `TextFont.font` — including `bsn!` UI, which can't
/// set it in 0.19 — renders normally. FreeSans covers, in one sans face, full
/// Latin, arrows, and the common BMP note glyphs (`♩ ♪ ♫ ♬`), so mixed
/// text+symbol runs render without relying on parley's per-glyph fallback. (The
/// SMP whole/half note glyphs aren't in any sans font, so those durations show a
/// word instead — see `dur_symbol`.) Embedded so it's ready at startup.
fn override_default_font(mut fonts: ResMut<Assets<Font>>) {
    const BYTES: &[u8] = include_bytes!("../../../../assets/fonts/FreeSans.otf");
    if let Err(err) = fonts.insert(&Handle::<Font>::default(), Font::from_bytes(BYTES.to_vec())) {
        warn!("Could not install default font: {err}");
    }
}

/// Collects the names of subfolders under `root` that contain a `theme.json`.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
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
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
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

/// wasm sibling of the native `scan_ui_themes` above. No `~/Harmonicon`
/// external-folder equivalent under wasm — there's no home directory concept
/// in a browser, and `dirs::home_dir()` already returns `None` there, which
/// the native version already treats as "no external themes" gracefully.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn scan_ui_themes(mut available: ResMut<AvailableThemes>) {
    let mut names: Vec<String> = manifest::THEMES.iter().map(|s| s.to_string()).collect();
    if names.is_empty() {
        names.push("default".into());
    }
    info!("Found {} UI theme(s): {:?}", names.len(), names);
    available.0 = names;
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
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

/// wasm sibling of the native `scan_harmonica_models` above.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn scan_harmonica_models(mut available: ResMut<AvailableHarmonicas>) {
    available.0 = manifest::HARMONICA_MODELS
        .iter()
        .map(|s| s.to_string())
        .collect();
    info!(
        "Found {} harmonica model(s): {:?}",
        available.0.len(),
        available.0
    );
}

/// `songs_root` is the folder holding artist folders, and `source_prefix`
/// is what each built `SongEntry::asset_path` starts with, so it loads from
/// the right [`AssetSource`](bevy::asset::io::AssetSource): `songs/` for the
/// bundled `assets/` root, `external://songs/` for the `~/Harmonicon` drop
/// folder, or a pack's `packs://<slug>/`.
#[cfg(not(target_arch = "wasm32"))]
pub fn scan_artist_song(
    artist_dir: &DirEntry,
    songs_root: &std::path::Path,
    available: &mut AvailableSongs,
    source_prefix: &str,
) {
    let Ok(song_dirs) = std::fs::read_dir(artist_dir.path()) else {
        return;
    };

    let artist = artist_dir.file_name().to_string_lossy().into_owned();
    for song_dir in song_dirs.flatten() {
        if !is_visible_dir(&song_dir) {
            continue;
        }

        // The files for the music are inside of `song` subdirectory.
        //
        // A `.harpchart` always wins over an imported format, and the two
        // are checked in separate passes rather than by first-match:
        // `song/music.mid` is *backing audio* for a charted song, so a
        // directory holding both would otherwise pick whichever `read_dir`
        // happened to yield first and sometimes play the backing track as
        // the chart. A score file is only the chart when nothing else is
        // (see `harmonicon_song::song::score_song`).
        //
        // The importable extensions come from `harmonicon_score`, never a
        // list written out here: a format this scan didn't know about
        // would simply never appear in the song list, with nothing to
        // explain why.
        let song_file = (|| {
            let entries: Vec<_> = std::fs::read_dir(song_dir.path().join("song"))
                .ok()?
                .flatten()
                .collect();
            // Case-insensitively, matching `harmonicon_score::parse_import`
            // — a file saved as `.MID` is as common as `.mid`.
            let has_extension = |entry: &std::fs::DirEntry, want: &[&str]| {
                entry
                    .path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .is_some_and(|e| want.contains(&e.as_str()))
            };
            entries
                .iter()
                .find(|e| has_extension(e, &["harpchart"]))
                .or_else(|| {
                    // `.xml` is only offered when its head is MusicXML.
                    entries
                        .iter()
                        .find(|e| harmonicon_score::is_importable_file(&e.path()))
                })
                .map(|e| e.path())
        })();

        let Some(song_file) = song_file else {
            continue;
        };

        let Some(relative) = asset_relative_path(&song_file, songs_root) else {
            continue;
        };

        let name = song_dir.file_name().to_string_lossy().into_owned();
        available
            .0
            .entry(artist.clone())
            .or_default()
            .push(SongEntry {
                asset_path: format!("{source_prefix}{relative}"),
                artist: artist.clone(),
                name,
            });
    }
}

/// `path` below `root`, `/`-separated whatever the platform, since asset
/// paths are.
#[cfg(not(target_arch = "wasm32"))]
fn asset_relative_path(path: &std::path::Path, root: &std::path::Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = relative.iter().map(|c| c.to_str()).collect();
    Some(parts?.join("/"))
}

/// A directory a scan should look into. Hidden ones are skipped: a pack's
/// checkout carries its own `.git`, which is not content.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_visible_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_ok_and(|t| t.is_dir())
        && !entry.file_name().to_string_lossy().starts_with('.')
}

/// Walks `songs_root` (a pack's checkout or the external
/// `~/Harmonicon/songs` drop folder) and scans each
/// artist subfolder into `available`, tagging entries with `source_prefix`
/// so they load from the matching
/// [`AssetSource`](bevy::asset::io::AssetSource).
#[cfg(not(target_arch = "wasm32"))]
fn scan_songs_root(
    songs_root: &std::path::Path,
    source_prefix: &str,
    available: &mut AvailableSongs,
) {
    let Ok(artists) = std::fs::read_dir(songs_root) else {
        return;
    };

    for artist_dir in artists.flatten() {
        if !is_visible_dir(&artist_dir) {
            continue;
        }
        scan_artist_song(&artist_dir, songs_root, available, source_prefix);
    }
}

/// Every usable song pack, after whatever else is already in `available`.
#[cfg(not(target_arch = "wasm32"))]
fn scan_song_packs(packs: Option<&ContentPacks>, available: &mut AvailableSongs) {
    for pack in packs
        .into_iter()
        .flat_map(|p| p.usable(harmonicon_packs::pack::PackKind::Songs))
    {
        scan_songs_root(&pack.root, &format!("{}/", pack.asset_prefix()), available);
    }
}

#[cfg(target_arch = "wasm32")]
fn scan_song_packs(_packs: Option<&ContentPacks>, _available: &mut AvailableSongs) {}

fn log_song_count(available: &AvailableSongs) {
    let total: usize = available.0.values().map(|v| v.len()).sum();
    info!(
        "Found {} song(s) across {} artist(s)",
        total,
        available.0.len()
    );
}

pub fn scan_all_songs(mut available: ResMut<AvailableSongs>, packs: Option<Res<ContentPacks>>) {
    scan_all_songs_into(&mut available, packs.as_deref());
}

// Scans the external `~/Harmonicon/songs` drop folder if present, plus every
// usable song pack, per artist. The game ships no songs of its own; they come
// from the packs. Clears `available` first, so this is safe to call again at
// runtime (e.g. after the player drops a song into `~/Harmonicon/songs`, or a
// pack updates), not just once at Startup.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub fn scan_all_songs_into(available: &mut AvailableSongs, packs: Option<&ContentPacks>) {
    available.0.clear();
    if let Some(external_root) = dirs::home_dir().map(|h| h.join("Harmonicon/songs")) {
        scan_songs_root(&external_root, "external://songs/", available);
    }

    scan_song_packs(packs, available);
    log_song_count(available);
}

/// wasm/Android sibling of the native `scan_all_songs_into` above, with no
/// `~/Harmonicon/songs` drop folder (there's no home directory concept in a
/// browser, and an Android app can only reach its own sandbox). wasm's songs
/// are the pack bundled into the build-time manifest; Android's manifest
/// lists none and it scans its downloaded packs instead.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
pub fn scan_all_songs_into(available: &mut AvailableSongs, packs: Option<&ContentPacks>) {
    available.0.clear();
    for (artist, name, asset_path) in manifest::SONGS {
        available
            .0
            .entry((*artist).to_string())
            .or_default()
            .push(SongEntry {
                artist: (*artist).to_string(),
                name: (*name).to_string(),
                asset_path: (*asset_path).to_string(),
            });
    }
    scan_song_packs(packs, available);
    log_song_count(available);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::Schedule;
    use harmonicon_packs::pack::PackKind;

    #[test]
    fn scan_all_songs_does_not_duplicate_entries_when_run_again() {
        let mut world = World::new();
        world.init_resource::<AvailableSongs>();
        let mut schedule = Schedule::default();
        schedule.add_systems(scan_all_songs);

        schedule.run(&mut world);
        let first: usize = world
            .resource::<AvailableSongs>()
            .0
            .values()
            .map(|v| v.len())
            .sum();

        schedule.run(&mut world);
        let second: usize = world
            .resource::<AvailableSongs>()
            .0
            .values()
            .map(|v| v.len())
            .sum();

        assert_eq!(first, second);
    }

    fn song_tree(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for rel in files {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "{}").unwrap();
        }
        dir
    }

    #[test]
    fn song_paths_are_relative_to_the_scanned_root() {
        let root = song_tree(&["Bach/Minuet/song/chart.harpchart"]);
        let mut available = AvailableSongs::default();
        scan_songs_root(root.path(), "packs://some-pack/", &mut available);
        assert_eq!(
            available.0["Bach"][0].asset_path,
            "packs://some-pack/Bach/Minuet/song/chart.harpchart"
        );
    }

    #[test]
    fn hidden_folders_are_not_artists_or_songs() {
        let root = song_tree(&[
            ".git/objects/song/chart.harpchart",
            "Bach/.draft/song/chart.harpchart",
            "Bach/Minuet/song/chart.harpchart",
        ]);
        let mut available = AvailableSongs::default();
        scan_songs_root(root.path(), "", &mut available);
        assert_eq!(available.0.len(), 1);
        assert_eq!(available.0["Bach"].len(), 1);
    }

    #[test]
    fn usable_song_packs_are_scanned_after_everything_else() {
        use crate::content_packs::{PackEntry, PackStatus};
        use harmonicon_packs::pack::PackManifest;
        use harmonicon_packs::repo::RepoSpec;

        let root = song_tree(&["Bach/Minuet/song/chart.harpchart"]);
        let manifest = PackManifest::parse(
            br#"{"schema":1,"kind":"songs","id":"p","name":"P","version":"1.0.0"}"#,
        )
        .unwrap()
        .unwrap();
        let entry = |slug: &str, status| PackEntry {
            kind: PackKind::Songs,
            spec: RepoSpec::Local {
                path: root.path().into(),
            },
            slug: slug.into(),
            root: root.path().into(),
            status,
        };
        let packs = ContentPacks(vec![
            entry(
                "ready",
                PackStatus::Ready {
                    manifest,
                    commit: None,
                },
            ),
            entry("broken", PackStatus::Unusable("no".into())),
        ]);
        let mut available = AvailableSongs::default();
        scan_song_packs(Some(&packs), &mut available);
        let paths: Vec<_> = available.0["Bach"]
            .iter()
            .map(|s| s.asset_path.as_str())
            .collect();
        assert_eq!(paths, ["packs://ready/Bach/Minuet/song/chart.harpchart"]);
    }

    #[test]
    fn scan_ui_themes_does_not_duplicate_entries_when_run_again() {
        let mut world = World::new();
        world.init_resource::<AvailableThemes>();
        let mut schedule = Schedule::default();
        schedule.add_systems(scan_ui_themes);

        schedule.run(&mut world);
        let first = world.resource::<AvailableThemes>().0.clone();

        schedule.run(&mut world);
        let second = world.resource::<AvailableThemes>().0.clone();

        assert_eq!(first, second);
    }
}
