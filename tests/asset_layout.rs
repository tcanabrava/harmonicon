// SPDX-License-Identifier: MIT

//! Smoke test for the asset tree's minimum structure.
//!
//! Each theme needs a fixed set of files for menu discovery and loading. A
//! file missing here breaks the game far from where the symptom shows up, so
//! this fails fast with a report listing every missing file, grouped by
//! theme, for quick local diagnosis.
//!
//! Songs and lessons are not here: they are content packs, checked by
//! `validate-pack` in their own repositories (`tests/validate_pack.rs`).
//!
//! Paths checked (per the design docs / asset conventions):
//!   assets/themes/<name>/{theme.json (valid against schema), preview.png, + all files listed in theme.json}

use std::path::{Path, PathBuf};

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
    path.strip_prefix("assets/").unwrap_or(path).display().to_string()
}

// ── Chart schema ──────────────────────────────────────────────────────────────

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
        .map(|e| format!("    - {e} (at /{path})", path = e.instance_path()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A chart built in memory must serialize to something the loader would
/// accept back.
///
/// `HarpChart` was deserialize-only in practice: nothing serialized one, so
/// nobody noticed that `serde_json` writes `null` for every absent optional
/// while the schema types those fields as strings and arrays. The Song
/// Editor sidesteps it by hand-building its JSON with `json!` rather than
/// serializing the struct, which is why saving from the editor has always
/// worked. A generated training chart has no such hand-written path, so the
/// round trip has to actually hold.
#[test]
fn a_generated_chart_serializes_to_something_the_schema_accepts() {
    use harmonicon_core::harmonica::richter_harp;
    use harmonicon_core::training::{DrillSpec, DrillTechnique, Tier, drill_chart};

    let validator = schema_validator("assets/song_schema.dtd.json");
    for tier in Tier::ALL {
        let spec =
            DrillSpec { technique: DrillTechnique::Bend, holes: vec![2, 3, 4], tier, seed: 4242 };
        let chart = drill_chart(&spec, &richter_harp("C"), "Drill", "Trainer")
            .expect("a C harp bends holes 2-4");
        let value = serde_json::to_value(&chart).expect("a chart must serialize");
        let errors = validation_errors(&validator, &value);
        assert!(
            errors.is_empty(),
            "tier {} produced a schema-invalid chart:\n{errors}",
            tier.number()
        );
    }
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
                report.push_str(&format!("  {}: JSON parse error: {e}\n", label(&theme_dir)));
                continue;
            }
        };

        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|e| format!("    - {e} (at /{path})", path = e.instance_path()))
            .collect();
        if !errors.is_empty() {
            report.push_str(&format!("  {}:\n{}\n", label(&theme_dir), errors.join("\n")));
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
    if let Some(f) = btn.pointer("/background_image/image_file").and_then(|v| v.as_str()) {
        refs.push(f.to_string());
    }
    if let Some(f) = btn.pointer("/icon/image_file").and_then(|v| v.as_str()) {
        refs.push(f.to_string());
    }
    for state in ["hover", "click", "idle"] {
        if let Some(f) = btn.pointer(&format!("/button_shaders/{state}")).and_then(|v| v.as_str()) {
            refs.push(f.to_string());
        }
    }
    for state in ["hover", "click"] {
        if let Some(f) = btn.pointer(&format!("/button_sounds/{state}")).and_then(|v| v.as_str()) {
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
            report.push_str(&format!("  {}: missing {}\n", label(&theme_dir), missing.join(", ")));
        }
    }

    assert!(report.is_empty(), "Incomplete theme assets:\n{report}");
}

/// Every `ShaderRef` path named in the workspace's Rust sources resolves to a
/// real file under `assets/`.
///
/// A `ShaderRef` is a plain string: renaming or moving a shader compiles
/// perfectly and only fails at *runtime*, when the material's pipeline is
/// first built and the asset server has nothing to hand it. Worse, the
/// failure surfaces as a wgpu validation error about the shader's contents
/// rather than about its path, so it reads like a shader bug.
///
/// Bevy 0.20's move from naga_oil to WESL (`.wgsl` -> `.wesl`, since only
/// `.wesl` resolves `import`s) renamed all seven of them at once, which is
/// exactly the kind of sweep that leaves one behind.
#[test]
fn shader_ref_paths_exist() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = repo.join("assets");

    let mut sources = Vec::new();
    collect_rust_sources(&repo.join("src"), &mut sources);
    for crate_dir in subdirs(&repo.join("crates")) {
        collect_rust_sources(&crate_dir.join("src"), &mut sources);
    }
    assert!(!sources.is_empty(), "found no Rust sources to scan for shader paths");

    let mut referenced: Vec<(String, String)> = Vec::new();
    for path in &sources {
        let text = std::fs::read_to_string(path).expect("source file must be readable");
        for rel in shader_refs(&text) {
            referenced.push((label(path), rel));
        }
    }
    assert!(
        !referenced.is_empty(),
        "found no `shaders/...` paths at all — has the ShaderRef convention changed?"
    );

    let mut report = String::new();
    for (source, rel) in &referenced {
        if !assets.join(rel).exists() {
            report.push_str(&format!("  {source} references missing assets/{rel}\n"));
        }
    }
    assert!(report.is_empty(), "Dangling shader paths:\n{report}");
}

/// Every `.rs` file under `dir`, recursively, appended to `out`.
fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// The `shaders/<name>.<ext>` string literals in `text`.
///
/// Deliberately matches the whole `"shaders/..."` literal rather than a
/// specific extension, so a stale `.wgsl` reference is *reported as
/// dangling* instead of being skipped by the scan that was meant to catch it.
fn shader_refs(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (start, _) in text.match_indices("\"shaders/") {
        let rest = &text[start + 1..];
        if let Some(end) = rest.find('"') {
            found.push(rest[..end].to_string());
        }
    }
    found
}

/// No shader still uses naga_oil's preprocessor syntax.
///
/// Bevy 0.20 swapped naga_oil for WESL, which has its own spellings:
///
/// | naga_oil | WESL |
/// |---|---|
/// | `#import path::Thing` | `import path::Thing;` |
/// | `#{SHADER_DEF}` | `constants::SHADER_DEF` |
/// | `#ifdef X` / `#endif` | `@if(X)` on the declaration |
///
/// A leftover directive is not a compile error — `.wesl` is an asset, so it
/// only fails when the pipeline is first built, and the message points at
/// the offending column rather than saying "this is naga_oil syntax". Both
/// kinds were missed once during the port: the `#import`s were found and
/// fixed, then three mid-line `#{MATERIAL_BIND_GROUP}`s survived a check
/// that only looked for `#` at the start of a line. Hence a scan of the
/// whole file, anchored nowhere.
#[test]
fn shaders_use_no_naga_oil_directives() {
    let shaders = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/shaders");
    let mut checked = 0;
    let mut report = String::new();

    for entry in std::fs::read_dir(&shaders).expect("assets/shaders must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wesl") {
            continue;
        }
        checked += 1;
        let source = std::fs::read_to_string(&path).expect("shader must be readable");
        for (i, line) in source.lines().enumerate() {
            // `#` has no meaning in WESL at all, so any occurrence outside a
            // comment is a leftover directive. Checking the whole line (not
            // just its start) is the point of this test.
            let code = line.split("//").next().unwrap_or(line);
            if let Some(col) = code.find('#') {
                report.push_str(&format!(
                    "  {}:{}:{}: naga_oil directive `{}`\n",
                    path.file_name().unwrap().to_string_lossy(),
                    i + 1,
                    col + 1,
                    code.trim(),
                ));
            }
        }
    }

    assert!(checked > 0, "found no .wesl shaders to check");
    assert!(
        report.is_empty(),
        "Shaders still using naga_oil preprocessor syntax \
         (Bevy 0.20 uses WESL — see this test's doc comment):\n{report}"
    );
}
