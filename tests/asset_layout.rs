// SPDX-License-Identifier: MIT

//! Smoke test for the asset tree's minimum structure.
//!
//! Each song and each 3D harmonica model needs a fixed set of files for menu
//! discovery, loading, and 3D behavior. A file missing here breaks the game far
//! from where the symptom shows up, so this fails fast with a report listing
//! every missing file, grouped by song or by model, for quick local diagnosis.
//!
//! Paths checked (per the design docs / asset conventions):
//!   assets/songs/<artist>/<song>/song/*.harpchart
//!   assets/harmonicas/3d/<model>/{harmonica.glb, holes.json}
//!   assets/themes/<name>/{theme.json (valid against schema), preview.png, + all files listed in theme.json}

use std::path::{Path, PathBuf};

/// Whether `dir/song/` contains at least one `.harpchart` file (any name —
/// `song::loader::SongChartLoader` is registered for the extension, not a
/// fixed filename) — the only asset a song strictly needs.
/// `background.png`/`elements.png`/`song/*.ogg` and the `2d/`/`3d/` note
/// asset folders are all optional: the loader falls back to a generated
/// background, silent (no) music, and the selected note theme's defaults
/// respectively when they're missing, rather than hanging `SongLoading`
/// waiting on a dependency that will never resolve. See `Example Song 3`
/// for a deliberately minimal example exercising every one of those
/// fallbacks at once.
fn has_harpchart(dir: &Path) -> bool {
    std::fs::read_dir(dir.join("song"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("harpchart"))
}

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
        if !has_harpchart(&song) {
            report.push_str(&format!("  {}: no *.harpchart file under song/\n", label(&song)));
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

// ── Lesson tests ──────────────────────────────────────────────────────────────

fn schema_validator(path: &str) -> jsonschema::Validator {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("Cannot read {path}: {e}"));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("Cannot parse {path}: {e}"));
    jsonschema::validator_for(&value)
        .unwrap_or_else(|e| panic!("{path} does not compile as a schema: {e}"))
}

fn validation_errors(validator: &jsonschema::Validator, instance: &serde_json::Value) -> String {
    validator
        .iter_errors(instance)
        .map(|e| format!("    - {e} (at /{path})", path = e.instance_path))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every bundled lesson must have a schema-valid `lesson.json`; a
/// chart-backed lesson's referenced chart must exist and validate against
/// the song schema (background/music/note art are optional — same fallbacks
/// as an ordinary song, see `has_harpchart`'s doc comment). Lesson ids must
/// be unique, and every prerequisite must name a lesson that exists (a
/// typo'd prerequisite would lock a lesson forever).
#[test]
fn lesson_assets_are_complete_and_valid() {
    let root = Path::new("assets/lessons");
    assert!(root.is_dir(), "missing asset directory: {}", root.display());

    let lesson_dirs: Vec<PathBuf> = subdirs(root)
        .iter()
        .flat_map(|unit| subdirs(unit))
        .collect();
    assert!(
        !lesson_dirs.is_empty(),
        "no lessons found under {}",
        root.display()
    );

    let lesson_validator = schema_validator("assets/lesson_schema.dtd.json");
    let chart_validator = schema_validator("assets/song_schema.dtd.json");

    let mut report = String::new();
    let mut ids: Vec<String> = Vec::new();
    let mut prerequisites: Vec<(String, String)> = Vec::new(); // (lesson dir, prereq id)

    for dir in &lesson_dirs {
        let manifest_path = dir.join("lesson.json");
        if !manifest_path.exists() {
            report.push_str(&format!("  {}: missing lesson.json\n", label(dir)));
            continue;
        }
        let text = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {e}", manifest_path.display()));
        let manifest: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                report.push_str(&format!("  {}: JSON parse error: {e}\n", label(dir)));
                continue;
            }
        };
        let errors = validation_errors(&lesson_validator, &manifest);
        if !errors.is_empty() {
            report.push_str(&format!("  {}:\n{errors}\n", label(dir)));
            continue;
        }

        if let Some(id) = manifest["id"].as_str() {
            ids.push(id.to_string());
        }
        for p in manifest["prerequisites"].as_array().into_iter().flatten() {
            if let Some(p) = p.as_str() {
                prerequisites.push((label(dir), p.to_string()));
            }
        }

        // Chart-backed lessons: the referenced chart must exist and validate.
        if let Some(chart_rel) = manifest["chart"].as_str() {
            let chart_path = dir.join(chart_rel);
            if !chart_path.exists() {
                report.push_str(&format!(
                    "  {}: missing referenced chart {chart_rel}\n",
                    label(dir)
                ));
            } else {
                let chart_text = std::fs::read_to_string(&chart_path)
                    .unwrap_or_else(|e| panic!("Cannot read {}: {e}", chart_path.display()));
                match serde_json::from_str::<serde_json::Value>(&chart_text) {
                    Ok(chart) => {
                        let errors = validation_errors(&chart_validator, &chart);
                        if !errors.is_empty() {
                            report.push_str(&format!(
                                "  {} ({chart_rel}):\n{errors}\n",
                                label(dir)
                            ));
                        }
                    }
                    Err(e) => report.push_str(&format!(
                        "  {} ({chart_rel}): JSON parse error: {e}\n",
                        label(dir)
                    )),
                }
            }
        }
    }

    let mut sorted_ids = ids.clone();
    sorted_ids.sort_unstable();
    sorted_ids.dedup();
    if sorted_ids.len() != ids.len() {
        report.push_str(&format!("  duplicate lesson ids in {ids:?}\n"));
    }
    for (dir, prereq) in prerequisites {
        if !ids.contains(&prereq) {
            report.push_str(&format!(
                "  {dir}: prerequisite {prereq:?} names no existing lesson\n"
            ));
        }
    }

    assert!(report.is_empty(), "Lesson asset failures:\n{report}");
}

/// Every Fluent key a lesson manifest declares (`title_key`, `body_key`,
/// `lesson-unit-<unit>`) must exist in the en-US locale — the parity test in
/// `localization.rs` then guarantees every other locale has it too. A missing
/// key would render as the raw key name in the menu.
#[test]
fn lesson_localization_keys_exist() {
    let ftl = std::fs::read_to_string("assets/locales/en-US/main/ui.ftl")
        .expect("en-US ui.ftl must exist");
    let defined: Vec<&str> = ftl
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim()))
        .filter(|k| !k.is_empty())
        .collect();

    let root = Path::new("assets/lessons");
    let mut report = String::new();
    for dir in subdirs(root).iter().flat_map(|unit| subdirs(unit)) {
        let Ok(text) = std::fs::read_to_string(dir.join("lesson.json")) else {
            continue; // absence reported by lesson_assets_are_complete_and_valid
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let mut needed: Vec<String> = Vec::new();
        for field in ["title_key", "body_key"] {
            if let Some(k) = manifest[field].as_str() {
                needed.push(k.to_string());
            }
        }
        if let Some(unit) = manifest["unit"].as_str() {
            needed.push(format!("lesson-unit-{unit}"));
        }
        for key in needed {
            if !defined.contains(&key.as_str()) {
                report.push_str(&format!(
                    "  {}: key {key:?} not defined in en-US ui.ftl\n",
                    label(&dir)
                ));
            }
        }
    }
    assert!(report.is_empty(), "Missing lesson locale keys:\n{report}");
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
    assert!(
        !themes.is_empty(),
        "no themes found under {}",
        root.display()
    );

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
                report.push_str(&format!("  {}: JSON parse error: {e}\n", label(&theme_dir)));
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

    assert!(
        report.is_empty(),
        "Theme JSON validation failures:\n{report}"
    );
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
    assert!(
        !themes.is_empty(),
        "no themes found under {}",
        root.display()
    );

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
