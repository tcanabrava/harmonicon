// SPDX-License-Identifier: MIT

//! Enforces two rules from `docs/physical_design_plan.md`: the file-size
//! budget (rule 1, below) and the acyclic-dependency rule (rule 2 — see
//! [`no_module_dependency_cycles`] at the bottom of this file).
//!
//! File-size rule: ~1000 lines of non-test code per file, test modules
//! relocated to a sibling `tests.rs` once they dominate. A file over budget
//! must be in [`ALLOWLIST`] — new violations can't land silently, and the
//! allowlist itself is the burndown chart: shrink it as files get split,
//! never grow it for a *new* file (split before adding, per rule 5).
//!
//! "Non-test line count" is everything before a top-level `#[cfg(test)]`
//! line (whether it introduces an inline `mod tests { ... }` or a `mod
//! tests;` pointing at a sibling file) — matching how the two coexist
//! throughout `src/`. A file with no `#[cfg(test)]` marker counts in full.
//! Files literally named `tests.rs` are pure test content with no budget of
//! their own; they're skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// ~1000 lines of non-test code (see the module doc comment). Not a hard
/// technical limit — just what `ALLOWLIST` measures every file against.
const BUDGET: usize = 1250;

/// Files over budget, one per line, each with the split it is waiting on
/// (see `docs/physical_design_plan.md`). Empty: the last four splits landed
/// as directory modules. A new entry is a debt with a named destination,
/// not a permanent exemption — remove it the moment the file drops under
/// [`BUDGET`]; `allowlist_has_no_stale_entries` fails the build if one
/// lingers past that point.
const ALLOWLIST: &[&str] = &[];

/// Every workspace member's `src/`: this package's own, plus each crate
/// under `crates/`. Both rules below must cover the whole tree — a file
/// that moves into a new crate mustn't silently stop being checked.
fn source_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("src")];
    if let Ok(entries) = std::fs::read_dir("crates") {
        for entry in entries.flatten() {
            let member = entry.path().join("src");
            if member.is_dir() {
                roots.push(member);
            }
        }
    }
    roots.sort();
    roots
}

/// Every `.rs` file under all of [`source_roots`], sorted for a stable report.
fn all_rust_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in source_roots() {
        out.extend(rust_files(&root));
    }
    out.sort();
    out
}

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
    assert!(Path::new("src").is_dir(), "missing src/ — run from the workspace root");

    let mut violations = Vec::new();
    for path in all_rust_files() {
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

// ── Rule 2: dependencies point downward, and never come back ────────────────
//
// `docs/physical_design_plan.md` states the layering rule in prose but,
// until this test, nothing checked it — which is how `gameplay ↔ jam`,
// `menu ↔ song_editor` and `gameplay → menu` all accumulated unnoticed. The
// rule is absolute: no cycle, no allowlist. A crate split makes cycles
// *impossible* between crates, but Rust permits cyclic modules inside one
// crate, so this test stays useful afterwards.

/// The top-level module a source file belongs to — `src/gameplay/judge.rs`
/// and `src/gameplay/mod.rs` are both `gameplay`; `src/theme.rs` is
/// `theme`. `None` for the composition roots (`main.rs`, `lib.rs`,
/// `src/bin/*`), whose whole job is wiring every module together and which
/// are therefore allowed to reach all of them.
fn module_of(path: &Path) -> Option<String> {
    let rel = path.strip_prefix("src").ok()?;
    let first = rel.components().next()?.as_os_str().to_string_lossy().into_owned();
    match first.strip_suffix(".rs") {
        // A directory module: src/<name>/...
        None => Some(first),
        // A single-file module: src/<name>.rs
        Some("main" | "lib") => None,
        Some(stem) => Some(stem.to_string()),
    }
}

/// The leading `foo` of `foo::Bar`, `foo as f`, or a bare `foo`.
fn leading_ident(s: &str) -> Option<String> {
    let s = s.trim_start();
    let end = s.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(s.len());
    (end > 0).then(|| s[..end].to_string())
}

/// The top-level module names in whatever follows a `crate::`. Handles the
/// brace-grouped, often multi-line form (`use crate::{app::X, song::Y}` —
/// `src/jam/session.rs` and `src/gameplay/plugin.rs` both use it) by taking
/// the first identifier of each depth-1 comma-separated entry; a nested
/// group's contents are all under that same entry, so they never contribute
/// a name of their own.
fn modules_after(after: &str) -> Vec<String> {
    let s = after.trim_start();
    if !s.starts_with('{') {
        return leading_ident(s).into_iter().collect();
    }
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, c) in s.char_indices() {
        match c {
            '{' => {
                depth += 1;
                if depth == 1 {
                    start = idx + 1;
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    out.extend(leading_ident(&s[start..idx]));
                    break;
                }
            }
            ',' if depth == 1 => {
                out.extend(leading_ident(&s[start..idx]));
                start = idx + 1;
            }
            _ => {}
        }
    }
    out
}

/// Every `(line, module)` this file references through `crate::`.
///
/// Whole-line comments are blanked first (keeping line numbers intact): a
/// doc comment *naming* another module documents a relationship, it doesn't
/// create one, and several here do exactly that — `src/scoring.rs`'s header
/// cites `crate::gameplay`, `src/lessons/manifest.rs` cites `crate::app`.
/// Counting those would fail the build over prose.
fn crate_refs(contents: &str) -> Vec<(usize, String)> {
    let mut code = String::with_capacity(contents.len());
    for line in contents.lines() {
        if !line.trim_start().starts_with("//") {
            code.push_str(line);
        }
        code.push('\n');
    }

    const NEEDLE: &str = "crate::";
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(pos) = code[cursor..].find(NEEDLE) {
        let at = cursor + pos;
        let line = code[..at].bytes().filter(|&b| b == b'\n').count() + 1;
        for module in modules_after(&code[at + NEEDLE.len()..]) {
            out.push((line, module));
        }
        cursor = at + NEEDLE.len();
    }
    out
}

#[test]
fn no_module_dependency_cycles() {
    let files = all_rust_files();
    assert!(!files.is_empty(), "missing src/ — run from the workspace root");

    // module -> what it imports, each edge remembering one witness so a
    // failure names the line to go delete rather than just the cycle.
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut witness: BTreeMap<(String, String), String> = BTreeMap::new();
    for path in &files {
        let Some(from) = module_of(path) else {
            continue;
        };
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        for (line, to) in crate_refs(&contents) {
            if to == from {
                continue;
            }
            edges.entry(from.clone()).or_default().insert(to.clone());
            witness
                .entry((from.clone(), to))
                .or_insert_with(|| format!("{}:{line}", path.display()));
        }
    }

    // Transitive closure — tiny graph (~20 nodes), so the naive fixpoint is
    // both fast enough and obviously correct.
    let mut reach = edges.clone();
    loop {
        let mut changed = false;
        let nodes: Vec<String> = reach.keys().cloned().collect();
        for node in nodes {
            for next in reach.get(&node).cloned().unwrap_or_default() {
                for far in reach.get(&next).cloned().unwrap_or_default() {
                    changed |= reach.entry(node.clone()).or_default().insert(far);
                }
            }
        }
        if !changed {
            break;
        }
    }

    // A module inside a cycle can reach itself; two such modules share a
    // cycle when each can reach the other.
    let looping: Vec<String> = reach
        .iter()
        .filter(|(node, seen)| seen.contains(*node))
        .map(|(node, _)| node.clone())
        .collect();

    let mut groups: Vec<Vec<String>> = Vec::new();
    for node in &looping {
        match groups.iter_mut().find(|g| reach[&g[0]].contains(node)) {
            Some(group) => group.push(node.clone()),
            None => groups.push(vec![node.clone()]),
        }
    }

    let mut report = String::new();
    for group in &groups {
        report.push_str(&format!("\n  cycle between {}:\n", group.join(" ↔ ")));
        for from in group {
            for to in group {
                if let Some(w) = witness.get(&(from.clone(), to.clone())) {
                    report.push_str(&format!("    {from} -> {to}  ({w})\n"));
                }
            }
        }
    }

    assert!(
        groups.is_empty(),
        "circular module dependencies (docs/physical_design_plan.md rule 2). \
         There is deliberately no allowlist — break the cycle by moving the \
         shared item down to a module both sides may depend on:\n{report}"
    );
}
