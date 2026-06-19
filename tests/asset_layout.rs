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
//!   assets/themes/<name>/{theme.json (valid against schema), preview.png, + all files listed in theme.json}

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
    let songs: Vec<PathBuf> = subdirs(root)
        .iter()
        .flat_map(|artist| subdirs(artist))
        .collect();
    assert!(!songs.is_empty(), "no songs found under {}", root.display());

    let mut report = String::new();
    for song in songs {
        let missing = missing_files(&song, &SONG_FILES);
        if !missing.is_empty() {
            report.push_str(&format!(
                "  {}: missing {}\n",
                label(&song),
                missing.join(", ")
            ));
        }
    }

    assert!(report.is_empty(), "Incomplete song assets:\n{report}");
}

#[test]
fn harmonica_model_assets_are_complete() {
    let root = Path::new("assets/harmonicas/3d");
    assert!(root.is_dir(), "missing asset directory: {}", root.display());

    let models = subdirs(root);
    assert!(
        !models.is_empty(),
        "no harmonica models found under {}",
        root.display()
    );

    let mut report = String::new();
    for model in models {
        let missing = missing_files(&model, &MODEL_FILES);
        if !missing.is_empty() {
            report.push_str(&format!(
                "  {}: missing {}\n",
                label(&model),
                missing.join(", ")
            ));
        }
    }

    assert!(
        report.is_empty(),
        "Incomplete harmonica model assets:\n{report}"
    );
}

// ── Theme tests ───────────────────────────────────────────────────────────────

/// Loads the compiled JSON Schema validator for `theme_schema.dtd.json`.
fn theme_schema_validator() -> jsonschema::Validator {
    let schema_path = Path::new("assets/themes/theme_schema.dtd.json");
    let text = std::fs::read_to_string(schema_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", schema_path.display()));
    let schema_value: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("Cannot parse schema {}: {e}", schema_path.display()));
    jsonschema::validator_for(&schema_value)
        .unwrap_or_else(|e| panic!("Schema does not compile: {e}"))
}

/// Every `theme.json` in every `assets/themes/<name>/` directory must be valid
/// JSON and must conform to `theme_schema.dtd.json`.
#[test]
fn theme_json_validates_against_schema() {
    let validator = theme_schema_validator();
    let root = Path::new("assets/themes");
    let themes = subdirs(root);
    assert!(!themes.is_empty(), "no themes found under {}", root.display());

    let mut report = String::new();
    for theme_dir in themes {
        let json_path = theme_dir.join("theme.json");

        if !json_path.exists() {
            report.push_str(&format!("  {}: missing theme.json\n", label(&theme_dir)));
            continue;
        }

        let text = std::fs::read_to_string(&json_path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {e}", json_path.display()));

        let instance: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                report.push_str(&format!(
                    "  {}: JSON parse error: {e}\n",
                    label(&theme_dir)
                ));
                continue;
            }
        };

        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|e| format!("    - {e} (at /{path})", path = e.instance_path))
            .collect();
        if !errors.is_empty() {
            report.push_str(&format!(
                "  {}:\n{}\n",
                label(&theme_dir),
                errors.join("\n")
            ));
        }
    }

    assert!(report.is_empty(), "Theme JSON validation failures:\n{report}");
}

/// Collects every file path referenced inside a parsed `theme.json` value.
/// All paths are relative to the theme directory.
fn collect_theme_file_refs(theme: &serde_json::Value) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();

    // default_background.image
    if let Some(img) = theme
        .pointer("/default_background/image")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        refs.push(img.to_string());
    }

    // default_menu_button.*
    let btn = &theme["default_menu_button"];
    if let Some(f) = btn
        .pointer("/background_image/image_file")
        .and_then(|v| v.as_str())
    {
        refs.push(f.to_string());
    }
    if let Some(f) = btn.pointer("/icon/image_file").and_then(|v| v.as_str()) {
        refs.push(f.to_string());
    }
    for state in ["hover", "click", "idle"] {
        if let Some(f) = btn
            .pointer(&format!("/button_shaders/{state}"))
            .and_then(|v| v.as_str())
        {
            refs.push(f.to_string());
        }
    }
    for state in ["hover", "click"] {
        if let Some(f) = btn
            .pointer(&format!("/button_sounds/{state}"))
            .and_then(|v| v.as_str())
        {
            refs.push(f.to_string());
        }
    }

    // menus.<name>.background_image
    if let Some(menus) = theme["menus"].as_object() {
        for (_menu_id, menu) in menus {
            if let Some(bg) = menu["background_image"].as_str() {
                refs.push(bg.to_string());
            }
        }
    }

    refs
}

/// Every file path listed inside a `theme.json` must exist on disk, and every
/// theme must ship a `preview.png` for the theme picker.
#[test]
fn theme_assets_are_complete() {
    let root = Path::new("assets/themes");
    let themes = subdirs(root);
    assert!(!themes.is_empty(), "no themes found under {}", root.display());

    let mut report = String::new();
    for theme_dir in themes {
        let json_path = theme_dir.join("theme.json");
        if !json_path.exists() {
            continue; // already caught by theme_json_validates_against_schema
        }

        let text = std::fs::read_to_string(&json_path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {e}", json_path.display()));
        let instance: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue, // JSON error already reported above
        };

        // preview.png is required by the theme picker (not listed in theme.json itself).
        let mut missing: Vec<String> = Vec::new();
        if !theme_dir.join("preview.png").exists() {
            missing.push("preview.png".to_string());
        }

        // All paths referenced inside the JSON.
        for rel in collect_theme_file_refs(&instance) {
            if !theme_dir.join(&rel).exists() {
                missing.push(rel);
            }
        }

        if !missing.is_empty() {
            report.push_str(&format!(
                "  {}: missing {}\n",
                label(&theme_dir),
                missing.join(", ")
            ));
        }
    }

    assert!(report.is_empty(), "Incomplete theme assets:\n{report}");
}
