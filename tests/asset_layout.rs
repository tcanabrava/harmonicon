// SPDX-License-Identifier: MIT

//! Smoke test for the asset tree's minimum structure.
//!
//! Each song and each 3D harmonica model needs a fixed set of files for menu
//! discovery, loading, and 3D behavior. A file missing here breaks the game far
//! from where the symptom shows up, so this fails fast with a report listing
//! every missing file, grouped by song or by model, for quick local diagnosis.
//!
//! Paths checked (per the design docs / asset conventions):
//!   assets/songs/<artist>/<song>/{chart.harpchart, background.png, music.ogg, elements.png}
//!   assets/harmonicas/3d/<model>/{harmonica.glb, holes.json}

use std::path::{Path, PathBuf};

/// Files every `assets/songs/<artist>/<song>/` directory must contain.
const SONG_FILES: [&str; 4] = [
    "chart.harpchart",
    "background.png",
    "music.ogg",
    "elements.png",
];

/// Files every `assets/harmonicas/3d/<model>/` directory must contain.
const MODEL_FILES: [&str; 2] = ["harmonica.glb", "holes.json"];

/// The required files absent from `dir`, in declared order.
fn missing_files(dir: &Path, required: &[&str]) -> Vec<String> {
    required
        .iter()
        .filter(|name| !dir.join(name).exists())
        .map(|name| name.to_string())
        .collect()
}

/// Immediate subdirectories of `root`, sorted by path. Empty if `root` is absent.
fn subdirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// A path relative to `assets/`, for compact report lines.
fn label(path: &Path) -> String {
    path.strip_prefix("assets/")
        .unwrap_or(path)
        .display()
        .to_string()
}

#[test]
fn song_assets_are_complete() {
    let root = Path::new("assets/songs");
    assert!(root.is_dir(), "missing asset directory: {}", root.display());

    // songs/<artist>/<song>/
    let songs: Vec<PathBuf> = subdirs(root).iter().flat_map(|artist| subdirs(artist)).collect();
    assert!(!songs.is_empty(), "no songs found under {}", root.display());

    let mut report = String::new();
    for song in songs {
        let missing = missing_files(&song, &SONG_FILES);
        if !missing.is_empty() {
            report.push_str(&format!("  {}: missing {}\n", label(&song), missing.join(", ")));
        }
    }

    assert!(report.is_empty(), "Incomplete song assets:\n{report}");
}

#[test]
fn harmonica_model_assets_are_complete() {
    let root = Path::new("assets/harmonicas/3d");
    assert!(root.is_dir(), "missing asset directory: {}", root.display());

    let models = subdirs(root);
    assert!(!models.is_empty(), "no harmonica models found under {}", root.display());

    let mut report = String::new();
    for model in models {
        let missing = missing_files(&model, &MODEL_FILES);
        if !missing.is_empty() {
            report.push_str(&format!("  {}: missing {}\n", label(&model), missing.join(", ")));
        }
    }

    assert!(report.is_empty(), "Incomplete harmonica model assets:\n{report}");
}
