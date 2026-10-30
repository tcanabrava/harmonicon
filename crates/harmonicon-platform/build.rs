// SPDX-License-Identifier: MIT

//! Generates the asset manifest that `assets_management` includes on targets
//! whose `assets/` tree isn't a readable local directory: wasm (no
//! filesystem — the asset reader talks HTTP) and Android (assets live inside
//! the APK, reachable only through the JNI `AssetManager`). iOS is *not* one
//! of these; an app bundle's Resources directory reads like any other, so it
//! keeps the runtime scan.
//!
//! Songs are a downloaded pack, not part of `assets/`. Android downloads
//! them at runtime like desktop, so its manifest lists none; wasm can't, so
//! it lists the songs of the pack in the directory `HARMONICON_SONGS_DIR`
//! names (relative paths from the workspace root), which the web bundle
//! must serve under `assets/songs/`.
//!
//! Lives here rather than in the workspace root's build.rs because
//! `include!(concat!(env!("OUT_DIR"), ...))` reads the *including* crate's
//! own OUT_DIR, and OUT_DIR is per-package. Paths reach back to the
//! workspace root, where assets/ actually lives.

use std::path::Path;

#[path = "src/assets_management/song_parts.rs"]
mod song_parts;

fn main() {
    generate_bundled_asset_manifest();
}

/// Writes `$OUT_DIR/asset_manifest.rs` for the manifest-backed scan
/// functions in `assets_management` to `include!()` — see the module doc
/// comment above for why. A no-op (and cheap: two env var reads) unless the
/// crate is actually being built for one of those targets, so native builds
/// pay nothing here.
///
/// The guard must stay in step with the `#[cfg(any(target_arch = "wasm32",
/// target_os = "android"))]` on `assets_management`'s `manifest` module —
/// a mismatch is a missing-file build error, not a silent fallback.
fn generate_bundled_asset_manifest() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH");
    let os = std::env::var("CARGO_CFG_TARGET_OS");
    if arch.as_deref() != Ok("wasm32") && os.as_deref() != Ok("android") {
        return;
    }

    println!("cargo:rerun-if-env-changed=HARMONICON_SONGS_DIR");
    println!("cargo:rerun-if-changed=../../assets/themes");
    println!("cargo:rerun-if-changed=../../assets/notes");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let dest = Path::new(&out_dir).join("asset_manifest.rs");

    let songs = if arch.as_deref() == Ok("wasm32") {
        match std::env::var("HARMONICON_SONGS_DIR") {
            Ok(dir) => {
                let root = Path::new("../..").join(&dir);
                println!("cargo:rerun-if-changed={}", root.display());
                scan_songs_for_manifest(&root)
            }
            Err(_) => {
                println!(
                    "cargo:warning=HARMONICON_SONGS_DIR is not set; this wasm build has no songs"
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let themes = scan_theme_dir_names(Path::new("../../assets/themes"));
    let notes_2d = scan_ext_stems(Path::new("../../assets/notes/2d"), "png");

    let mut out = String::new();
    out.push_str("// Auto-generated at build time by build.rs — do not edit.\n");
    out.push_str("pub static SONGS: &[(&str, &str, &str, &str, &str)] = &[\n");
    for (artist, name, genre, difficulty, asset_path) in &songs {
        out.push_str(&format!(
            "    ({artist:?}, {name:?}, {genre:?}, {difficulty:?}, {asset_path:?}),\n"
        ));
    }
    out.push_str("];\n\n");

    write_str_slice(&mut out, "THEMES", &themes);
    write_str_slice(&mut out, "NOTE_THEMES_2D", &notes_2d);

    std::fs::write(&dest, out).expect("failed to write asset manifest");
}

fn write_str_slice(out: &mut String, name: &str, values: &[String]) {
    out.push_str(&format!("pub static {name}: &[&str] = &[\n"));
    for v in values {
        out.push_str(&format!("    {v:?},\n"));
    }
    out.push_str("];\n\n");
}

/// A forward-slash-joined asset path — Bevy asset paths always use `/`
/// regardless of host OS, unlike `Path::to_string_lossy()` on Windows.
fn asset_relative_path(path: &Path, strip: &Path) -> String {
    path.strip_prefix(strip)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// (artist, song name, genre, difficulty, asset-relative chart path) for every song under
/// `root` — mirrors `assets_management::scan_songs_root`/`scan_artist_song`'s
/// discovery rules exactly (all `*.harpchart` files directly under each
/// song's `song/` subfolder), but as a one-shot build-time walk instead of a
/// runtime `ResMut` system.
fn scan_songs_for_manifest(root: &Path) -> Vec<(String, String, String, String, String)> {
    let mut out = Vec::new();
    let Ok(artists) = std::fs::read_dir(root) else {
        return out;
    };
    for artist_dir in artists.flatten() {
        if !artist_dir.file_type().map(|t| t.is_dir()).unwrap_or(false)
            || artist_dir.file_name().to_string_lossy().starts_with('.')
        {
            continue;
        }
        let artist = artist_dir.file_name().to_string_lossy().into_owned();
        let Ok(song_dirs) = std::fs::read_dir(artist_dir.path()) else {
            continue;
        };
        for song_dir in song_dirs.flatten() {
            if !song_dir.file_type().map(|t| t.is_dir()).unwrap_or(false)
                || song_dir.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            let charts = song_parts::charts_in(&song_dir.path().join("song"));
            for chart in &charts {
                let chart_metadata = std::fs::read(chart)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
                let catalog_field = |field: &str| {
                    chart_metadata
                        .as_ref()
                        .and_then(|value| value.get("song")?.get(field)?.as_str())
                        .unwrap_or("Uncategorized")
                        .to_string()
                };
                let genre = catalog_field("genre");
                let difficulty = catalog_field("difficulty");
                let name = song_dir.file_name().to_string_lossy().into_owned();
                // Served under `assets/songs/` by the web bundle.
                let asset_path = format!("songs/{}", asset_relative_path(chart, root));
                out.push((
                    artist.clone(),
                    song_parts::part_name(&name, chart, charts.len()),
                    genre,
                    difficulty,
                    asset_path,
                ));
            }
        }
    }
    out.sort();
    out
}

/// Names of subfolders under `root` that contain a `theme.json` — mirrors
/// `assets_management::scan_theme_names`.
fn scan_theme_dir_names(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| e.path().join("theme.json").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// `<name>` stems of files with extension `ext` directly under `dir` —
/// mirrors `assets_management::scan_theme_dir`.
fn scan_ext_stems(dir: &Path, ext: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ext))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_manifest_lists_each_native_part_with_its_metadata() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Band/Duet/song");
        std::fs::create_dir_all(&folder).unwrap();
        for (name, difficulty) in [("guitar", "easy"), ("harmonica", "hard")] {
            std::fs::write(
                folder.join(format!("{name}.harpchart")),
                serde_json::json!({"song": {"genre": "Test", "difficulty": difficulty}})
                    .to_string(),
            )
            .unwrap();
        }
        std::fs::write(folder.join("music.mid"), "backing").unwrap();
        let songs = super::scan_songs_for_manifest(root.path());
        assert_eq!(
            songs,
            vec![
                (
                    "Band".into(),
                    "Duet — guitar".into(),
                    "Test".into(),
                    "easy".into(),
                    "songs/Band/Duet/song/guitar.harpchart".into()
                ),
                (
                    "Band".into(),
                    "Duet — harmonica".into(),
                    "Test".into(),
                    "hard".into(),
                    "songs/Band/Duet/song/harmonica.harpchart".into()
                ),
            ]
        );
    }
}
