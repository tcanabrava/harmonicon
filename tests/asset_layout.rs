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
            report.push_str(&format!(
                "  {}: no *.harpchart file under song/\n",
                label(&song)
            ));
        }
    }

    assert!(report.is_empty(), "Incomplete song assets:\n{report}");
}

/// Every bundled song's chart must validate against the song schema — the
/// same check the engine performs at load time (`song::loader`), moved up
/// to CI so a hand-authored chart can't ship broken and only fail once a
/// player picks it.
#[test]
fn song_charts_are_schema_valid() {
    let root = Path::new("assets/songs");
    let chart_validator = schema_validator("assets/song_schema.dtd.json");

    let mut report = String::new();
    for song in subdirs(root).iter().flat_map(|artist| subdirs(artist)) {
        let Ok(entries) = std::fs::read_dir(song.join("song")) else {
            continue; // missing song/ is `song_assets_are_complete`'s complaint
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("harpchart") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            match serde_json::from_str::<serde_json::Value>(&text) {
                Err(e) => {
                    report.push_str(&format!("  {}: JSON parse error: {e}\n", label(&path)));
                }
                Ok(value) => {
                    let errors = validation_errors(&chart_validator, &value);
                    if !errors.is_empty() {
                        report.push_str(&format!("  {}:\n{errors}", label(&path)));
                    }
                }
            }
        }
    }

    assert!(report.is_empty(), "Schema-invalid song charts:\n{report}");
}

/// Every bundled chart that declares a *chromatic* harmonica must declare
/// one this codebase can actually build — see
/// `harmonicon_core::harmonica::chromatic_harp`.
///
/// Both shipped chromatic charts once carried a layout that no real
/// harmonica has: a C major scale ascending one note per hole, stopping at
/// A5 instead of C7. The music was right and the charts were internally
/// consistent, so nothing caught it — a player holding a real chromatic
/// would simply have found the hole numbers wrong, and the lesson that
/// teaches the slide was teaching it on a fictional instrument.
///
/// Deliberately only chromatics. A diatonic chart may legitimately use an
/// alternate tuning (paddy Richter, natural minor), so "matches the standard
/// table" is not a rule that holds there.
#[test]
fn bundled_chromatic_charts_use_a_real_instrument() {
    use harmonicon_core::harmonica::{Harmonica, chromatic_harp};
    use harmonicon_core::pitch_map::HARP_KEYS;

    let mut report = String::new();
    let mut checked = 0;
    let mut charts: Vec<PathBuf> = Vec::new();
    for root in ["assets/songs"] {
        for group in subdirs(Path::new(root)) {
            for item in subdirs(&group) {
                charts.push(item.join("song"));
            }
        }
    }

    for dir in charts {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("harpchart") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let Ok(chart) = serde_json::from_str::<harmonicon_core::chart::HarpChart>(&text) else {
                continue; // `song_charts_are_schema_valid` reports this
            };
            let Harmonica::Chromatic { .. } = &chart.harmonica else {
                continue;
            };
            checked += 1;
            let matches_a_real_harp = HARP_KEYS
                .iter()
                .any(|key| layout_of(&chromatic_harp(key)) == layout_of(&chart.harmonica));
            if !matches_a_real_harp {
                report.push_str(&format!(
                    "  {}: chromatic layout matches no key's standard tuning\n",
                    label(&path)
                ));
            }
        }
    }
    assert!(
        checked > 0,
        "no chromatic charts found — has the layout moved?"
    );
    assert!(
        report.is_empty(),
        "Charts declaring an impossible harmonica:\n{report}"
    );
}

/// A chromatic's four note tables, for comparing two harmonicas by layout.
fn layout_of(harp: &harmonicon_core::harmonica::Harmonica) -> Option<Vec<Vec<String>>> {
    match harp {
        harmonicon_core::harmonica::Harmonica::Chromatic {
            layout: Some(l), ..
        } => Some(vec![
            l.blow.clone().unwrap_or_default(),
            l.draw.clone().unwrap_or_default(),
            l.blow_slide.clone().unwrap_or_default(),
            l.draw_slide.clone().unwrap_or_default(),
        ]),
        _ => None,
    }
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
        let spec = DrillSpec {
            technique: DrillTechnique::Bend,
            holes: vec![2, 3, 4],
            tier,
            seed: 4242,
        };
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
            .map(|e| format!("    - {e} (at /{path})", path = e.instance_path()))
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
    assert!(
        !sources.is_empty(),
        "found no Rust sources to scan for shader paths"
    );

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
