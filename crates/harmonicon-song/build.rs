// SPDX-License-Identifier: MIT

//! Generates the bundled-lesson manifest that `lessons::catalog` includes
//! on wasm, which can neither download a lesson pack nor list a directory
//! (its asset reader talks HTTP). Every other target reads its packs from
//! disk at runtime, Android included.
//!
//! The pack comes from the directory `HARMONICON_LESSONS_DIR` names (a
//! relative path is taken from the workspace root) — a checkout of
//! `harmonicon-lessons`, fetched by whatever builds the web
//! bundle, which must also serve that pack under `assets/lessons/` so the
//! charts load. Unset, the wasm build has no lessons and says so.
//!
//! It can't live in `harmonicon-platform`'s build script:
//! `include!(concat!(env!("OUT_DIR"), ...))` reads the *including* crate's
//! own OUT_DIR, and OUT_DIR is per-package. And unlike the song/theme
//! manifests, which only need *names*, a lesson is discovered by reading
//! `lesson.json` itself, so this embeds the JSON text with `include_str!`.

use std::path::Path;

fn main() {
    generate_bundled_lesson_manifest();
}

/// Writes `$OUT_DIR/lesson_manifest.rs`. A no-op (an env var read) unless
/// building for wasm.
fn generate_bundled_lesson_manifest() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    println!("cargo:rerun-if-env-changed=HARMONICON_LESSONS_DIR");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let dest = Path::new(&out_dir).join("lesson_manifest.rs");

    // `include_str!` in the generated file resolves relative to that file,
    // which lives in OUT_DIR — so the paths it embeds have to be absolute.
    let lessons_root = match std::env::var("HARMONICON_LESSONS_DIR") {
        Ok(dir) => {
            // Relative to the workspace root, where the build is run from —
            // not to this crate, which is where a build script runs.
            let manifest_dir =
                std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by cargo");
            let root = Path::new(&manifest_dir).join("../..").join(&dir);
            println!("cargo:rerun-if-changed={}", root.display());
            root.canonicalize()
                .unwrap_or_else(|e| panic!("HARMONICON_LESSONS_DIR={dir}: {e}"))
        }
        Err(_) => {
            println!(
                "cargo:warning=HARMONICON_LESSONS_DIR is not set; this wasm build has no lessons"
            );
            Path::new(&out_dir).join("no-lessons")
        }
    };

    let mut out = String::from("// Auto-generated at build time by build.rs — do not edit.\n");
    out.push_str(
        "/// `(unit_dir, lesson_dir, lesson.json contents)`, in curriculum order.\n\
         pub const BUNDLED_LESSONS: &[(&str, &str, &str)] = &[\n",
    );

    for (unit, lesson, path) in scan_lessons_for_manifest(&lessons_root) {
        out.push_str(&format!(
            "    ({unit:?}, {lesson:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    out.push_str("];\n");

    std::fs::write(&dest, out).expect("failed to write lesson manifest");
}

/// Mirrors `lessons::catalog::scan_lessons_root`'s discovery rule exactly —
/// `<unit_dir>/<lesson_dir>/lesson.json`, both levels sorted by directory
/// name so the `01_`/`02_` prefixes give the curriculum order — so the two
/// implementations can't drift.
fn scan_lessons_for_manifest(root: &Path) -> Vec<(String, String, std::path::PathBuf)> {
    let mut found = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else {
        return found;
    };
    let mut unit_dirs: Vec<_> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .collect();
    unit_dirs.sort();

    for unit_dir in unit_dirs {
        let Ok(rd) = std::fs::read_dir(&unit_dir) else {
            continue;
        };
        let mut lesson_dirs: Vec<_> = rd
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        lesson_dirs.sort();

        for lesson_dir in lesson_dirs {
            let manifest = lesson_dir.join("lesson.json");
            if !manifest.is_file() {
                continue; // not a lesson dir
            }
            let (Some(unit), Some(lesson)) = (
                unit_dir.file_name().and_then(|n| n.to_str()),
                lesson_dir.file_name().and_then(|n| n.to_str()),
            ) else {
                continue;
            };
            found.push((unit.to_string(), lesson.to_string(), manifest));
        }
    }
    found
}
