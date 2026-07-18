// SPDX-License-Identifier: MIT

//! Enforces `docs/physical_design_plan.md`'s file-size rule: ~500 lines of
//! non-test code per file (rule 1), test modules relocated to a sibling
//! `tests.rs` once they dominate (rule 2). A file over budget must be in
//! [`ALLOWLIST`] — new violations can't land silently, and the allowlist
//! itself is the burndown chart: shrink it as files get split, never grow
//! it for a *new* file (split before adding, per rule 5).
//!
//! "Non-test line count" is everything before a top-level `#[cfg(test)]`
//! line (whether it introduces an inline `mod tests { ... }` or a `mod
//! tests;` pointing at a sibling file) — matching how the two coexist
//! throughout `src/`. A file with no `#[cfg(test)]` marker counts in full.
//! Files literally named `tests.rs` are pure test content with no budget of
//! their own; they're skipped.

use std::path::{Path, PathBuf};

/// ~500 lines of non-test code (see the module doc comment). Not a hard
/// technical limit — just what `ALLOWLIST` measures every file against.
const BUDGET: usize = 500;

/// Current offenders, one per line, with the split this file is nominally
/// waiting on (see `docs/physical_design_plan.md`'s Phase 6 — "no dedicated
/// push," split opportunistically when next touched). Remove an entry the
/// moment its file drops under [`BUDGET`]; `allowlist_has_no_stale_entries`
/// fails the build if one lingers past that point.
const ALLOWLIST: &[&str] = &[
    // Phase 6 targets named explicitly by the plan, with a destination:
    "src/gameplay/bending_trainer.rs", // split: drill logic vs UI
    "src/gameplay/gameplay_2d.rs",     // split: scene setup vs note spawn/despawn vs tails
    "src/gameplay/gameplay_3d.rs",     // split: scene setup vs note spawn/despawn vs tails
    "src/menu/pages/options.rs",       // split: one section per file
    "src/menu/pages/calibration.rs",   // split: measurement logic vs UI
    // Other current offenders, no assigned destination yet:
    "src/gameplay/pause_menu.rs",
    "src/audio_system/pitch_detect.rs",
    "src/song_editor/state.rs",
    "src/gameplay/song_progress_overlay.rs",
    "src/song_editor/grid.rs",
    "src/bin/midi_to_chart.rs",
    "src/jam/session.rs",
    "src/song/harmonica.rs",
    "src/bin/note_editor.rs",
    "src/menu/pages/lessons.rs",
    "src/dialogs/file_dialog.rs",
    "src/song_editor/harpchart.rs",
    "src/song_editor/interaction.rs",
];

/// Every `.rs` file under `root`, recursively, sorted for a stable report.
fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The non-test line count for one file's contents (see the module doc
/// comment for what that means). Only a *top-level* `#[cfg(test)]`
/// immediately gating `mod tests` marks the boundary — an indented
/// `#[cfg(test)]` on one helper method (e.g. a test-only accessor inside an
/// `impl` block) isn't the test module and mustn't truncate the count, and
/// nor is an earlier, differently-named test module (e.g. `song/chart.rs`'s
/// `mod format_version_tests`, ahead of its real `mod tests`).
fn non_test_line_count(contents: &str) -> usize {
    let lines: Vec<&str> = contents.lines().collect();
    lines
        .windows(2)
        .position(|w| w[0] == "#[cfg(test)]" && w[1].starts_with("mod tests"))
        .unwrap_or(lines.len())
}

#[test]
fn no_file_exceeds_the_line_budget_unless_allowlisted() {
    let root = Path::new("src");
    assert!(root.is_dir(), "missing src/ — run from the crate root");

    let mut violations = Vec::new();
    for path in rust_files(root) {
        if path.file_name().and_then(|n| n.to_str()) == Some("tests.rs") {
            continue;
        }
        let rel = path.to_string_lossy().replace('\\', "/");
        if ALLOWLIST.contains(&rel.as_str()) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines = non_test_line_count(&contents);
        if lines > BUDGET {
            violations.push(format!("{rel} ({lines} non-test lines)"));
        }
    }

    assert!(
        violations.is_empty(),
        "file(s) exceed the {BUDGET}-line budget (docs/physical_design_plan.md) \
         and aren't in tests/physical_design.rs's ALLOWLIST — split the file, \
         or add it to ALLOWLIST with a justification:\n{}",
        violations.join("\n")
    );
}

/// Keeps `ALLOWLIST` itself honest: an entry for a file that's already back
/// under budget must be removed, not left to rot — that's what makes the
/// allowlist a burndown chart rather than a one-way ratchet.
#[test]
fn allowlist_has_no_stale_entries() {
    let mut stale = Vec::new();
    for &rel in ALLOWLIST {
        let path = Path::new(rel);
        let Ok(contents) = std::fs::read_to_string(path) else {
            // Missing/renamed file — also stale, in a different way.
            stale.push(format!("{rel} (not found)"));
            continue;
        };
        let lines = non_test_line_count(&contents);
        if lines <= BUDGET {
            stale.push(format!("{rel} ({lines} non-test lines, now under budget)"));
        }
    }

    assert!(
        stale.is_empty(),
        "ALLOWLIST entries no longer need an exemption — remove them from \
         tests/physical_design.rs:\n{}",
        stale.join("\n")
    );
}
