// SPDX-License-Identifier: MIT

//! Generates the asset manifest that `assets_management` includes on targets
//! whose `assets/` tree isn't a readable local directory: wasm (no
//! filesystem — the asset reader talks HTTP) and Android (assets live inside
//! the APK, reachable only through the JNI `AssetManager`). iOS is *not* one
//! of these; an app bundle's Resources directory reads like any other, so it
//! keeps the runtime scan.
//!
//! Lives here rather than in the workspace root's build.rs because
//! `include!(concat!(env!("OUT_DIR"), ...))` reads the *including* crate's
//! own OUT_DIR, and OUT_DIR is per-package. Paths reach back to the
//! workspace root, where assets/ actually lives.

use std::path::Path;

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

    println!("cargo:rerun-if-changed=../../assets/songs");
    println!("cargo:rerun-if-changed=../../assets/themes");
    println!("cargo:rerun-if-changed=../../assets/notes");
    println!("cargo:rerun-if-changed=../../assets/harmonicas");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let dest = Path::new(&out_dir).join("asset_manifest.rs");

    let songs = scan_songs_for_manifest(Path::new("../../assets/songs"));
    let themes = scan_theme_dir_names(Path::new("../../assets/themes"));
    let notes_2d = scan_ext_stems(Path::new("../../assets/notes/2d"), "png");
    let notes_3d = scan_ext_stems(Path::new("../../assets/notes/3d"), "glb");
    let harmonicas = scan_harmonica_model_names(Path::new("../../assets/harmonicas/3d"));

    let mut out = String::new();
    out.push_str("// Auto-generated at build time by build.rs — do not edit.\n");
    out.push_str("pub static SONGS: &[(&str, &str, &str)] = &[\n");
    for (artist, name, asset_path) in &songs {
        out.push_str(&format!("    ({artist:?}, {name:?}, {asset_path:?}),\n"));
    }
    out.push_str("];\n\n");

    write_str_slice(&mut out, "THEMES", &themes);
    write_str_slice(&mut out, "NOTE_THEMES_2D", &notes_2d);
    write_str_slice(&mut out, "NOTE_THEMES_3D", &notes_3d);
    write_str_slice(&mut out, "HARMONICA_MODELS", &harmonicas);

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
fn asset_relative_path(path: &Path, strip: &str) -> String {
    path.strip_prefix(strip)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// (artist, song name, asset-relative chart path) for every song under
/// `root` — mirrors `assets_management::scan_songs_root`/`scan_artist_song`'s
/// discovery rules exactly (first `*.harpchart` file directly under each
/// song's `song/` subfolder), but as a one-shot build-time walk instead of a
/// runtime `ResMut` system.
fn scan_songs_for_manifest(root: &Path) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let Ok(artists) = std::fs::read_dir(root) else {
        return out;
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
            // Mirrors `assets_management::scan_artist_song` exactly,
            // including its two passes and its use of
            // `harmonicon_score::IMPORT_EXTENSIONS` — a bundled song whose
            // chart is a score file would otherwise be visible on desktop
            // and silently missing on wasm/Android.
            let chart = std::fs::read_dir(song_dir.path().join("song"))
                .ok()
                .and_then(|entries| {
                    let entries: Vec<_> = entries.flatten().collect();
                    let has_extension = |e: &std::fs::DirEntry, want: &[&str]| {
                        e.path()
                            .extension()
                            .and_then(|x| x.to_str())
                            .map(|x| x.to_ascii_lowercase())
                            .is_some_and(|x| want.contains(&x.as_str()))
                    };
                    entries
                        .iter()
                        .find(|e| has_extension(e, &["harpchart"]))
                        .or_else(|| {
                            entries
                                .iter()
                                .find(|e| has_extension(e, harmonicon_score::IMPORT_EXTENSIONS))
                        })
                        .map(|e| e.path())
                });
            let Some(chart) = chart else {
                continue;
            };
            let name = song_dir.file_name().to_string_lossy().into_owned();
            let asset_path = asset_relative_path(&chart, "../../assets");
            out.push((artist.clone(), name, asset_path));
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

/// Names of subfolders under `root` that contain a `harmonica.glb` — mirrors
/// `assets_management::scan_harmonica_models`.
fn scan_harmonica_model_names(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| e.path().join("harmonica.glb").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
