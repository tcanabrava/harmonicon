// SPDX-License-Identifier: MIT

//! Smoke test for the asset tree's minimum structure.
//!
//! Songs and lessons live in content packs and have separate validation.

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
