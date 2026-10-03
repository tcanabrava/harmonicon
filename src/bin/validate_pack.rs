// SPDX-License-Identifier: MIT

//! Checks a lesson or song pack the way this build of the game will read it.
//!
//! ```text
//! cargo run --bin validate-pack -- ../harmonicon-lessons
//! cargo run --bin validate-pack -- --choice-report ../harmonicon-lessons
//! ```
//!
//! The pack's `pack.json` says which kind it is. Exits non-zero on any
//! error, so a pack repository's CI can run it against the engine's `main`
//! and refuse a change that would break in the game. Warnings are printed
//! but don't fail.
//!
//! `--choice-report` also prints where a lesson pack's curriculum narrows
//! to a single lesson (`lessons::graph::choice_report`): a report to read
//! when changing prerequisites or units, not a pass/fail check.

use std::path::PathBuf;
use std::process::ExitCode;

use harmonicon_packs::pack::{EngineSupport, PackKind, PackManifest};
use harmonicon_song::lessons::LESSON_FORMAT_VERSION;
use harmonicon_song::lessons::validate::validate_lesson_pack;
use harmonicon_song::song::validate::{Report, validate_song_pack};

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let choice_report = args.iter().any(|a| a == "--choice-report");
    args.retain(|a| a != "--choice-report");
    let [dir] = args.as_slice() else {
        eprintln!("usage: validate-pack [--choice-report] <pack directory>");
        return ExitCode::from(2);
    };
    let root = PathBuf::from(dir);

    let manifest = match std::fs::read(root.join("pack.json")) {
        Err(e) => return fail(&format!("{}: cannot read pack.json: {e}", root.display())),
        Ok(bytes) => match PackManifest::parse(&bytes) {
            Err(e) => return fail(&format!("pack.json: {e}")),
            Ok(Err(incompatible)) => return fail(&format!("pack.json: {incompatible}")),
            Ok(Ok(manifest)) => manifest,
        },
    };
    let engine = EngineSupport::new(env!("CARGO_PKG_VERSION"), LESSON_FORMAT_VERSION);
    if let Err(incompatible) = manifest.check(manifest.kind, &engine) {
        return fail(&format!(
            "pack.json: {incompatible} (validating with Harmonicon {})",
            engine.harmonicon
        ));
    }

    let report = match manifest.kind {
        PackKind::Songs => validate_song_pack(&root),
        PackKind::Lessons => {
            let mut report = validate_lesson_pack(&root);
            warn_about_uncoloured_tracks(&root, &mut report);
            if choice_report && report.errors.is_empty() {
                print_choice_report(&root);
            }
            report
        }
    };

    for warning in &report.warnings {
        println!("warning: {warning}");
    }
    for error in &report.errors {
        println!("error: {error}");
    }
    println!(
        "{} pack {} {}: {} error(s), {} warning(s)",
        manifest.kind,
        manifest.id,
        manifest.version,
        report.errors.len(),
        report.warnings.len()
    );
    if report.errors.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn fail(message: &str) -> ExitCode {
    println!("error: {message}");
    ExitCode::FAILURE
}

/// A track the skill tree has no colour for draws in the grey every unknown
/// track shares. That is a presentation choice of this engine, not a rule
/// of the lesson format, so it only warns.
fn warn_about_uncoloured_tracks(root: &std::path::Path, report: &mut Report) {
    let mut uncoloured = std::collections::BTreeSet::new();
    for path in glob_lessons(root) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if let Ok(manifest) = harmonicon_song::lessons::parse_lesson(&bytes)
            && let Some(track) = manifest.track
            && !harmonicon_lessons::track_has_colour(&track)
        {
            uncoloured.insert(track);
        }
    }
    for track in uncoloured {
        report.warning(
            root,
            root,
            format!("track {track:?} has no colour of its own in this Harmonicon; it draws grey"),
        );
    }
}

/// See the module doc comment.
fn print_choice_report(root: &std::path::Path) {
    use harmonicon_song::lessons::graph::{LessonGraph, choice_report};
    use harmonicon_song::lessons::units::UnitChain;

    let manifests: Vec<_> = glob_lessons(root)
        .iter()
        .filter_map(|path| std::fs::read(path).ok())
        .filter_map(|bytes| harmonicon_song::lessons::parse_lesson(&bytes).ok())
        .collect();
    let Ok(graph) = LessonGraph::build(&manifests) else {
        return; // validation reports why
    };
    let chain = UnitChain::build(&manifests);
    let report = choice_report(&graph, &chain, 3000, 4, 0x5eed);
    println!("{} lessons, 3000 playthroughs, counting while at least 4 remain", manifests.len());
    println!("fewest lessons ever on offer: {}", report.fewest);
    println!("only lesson on offer, by how often:");
    for (id, count) in &report.sole_options {
        println!("  {count:>6}  {id}");
    }
}

fn glob_lessons(root: &std::path::Path) -> Vec<PathBuf> {
    use harmonicon_song::song::validate::content_dirs;
    content_dirs(root)
        .iter()
        .flat_map(|unit| content_dirs(unit))
        .map(|lesson| lesson.join("lesson.json"))
        .collect()
}
