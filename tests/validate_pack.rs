// SPDX-License-Identifier: MIT

//! The `validate-pack` binary end to end: what a pack repository's CI runs.
//! The rules themselves are unit-tested in `harmonicon-song`'s
//! `lessons::validate` and `song::validate`; this checks the front end reads
//! `pack.json`, dispatches on its kind and turns the report into an exit code.

use std::path::Path;
use std::process::Command;

fn validate(dir: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_validate-pack"))
        .arg(dir)
        .output()
        .expect("validate-pack runs");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lesson-pack")
}

#[test]
fn the_fixture_lesson_pack_passes() {
    let (ok, out) = validate(&fixture());
    assert!(ok, "{out}");
    assert!(out.contains("lessons pack fixture-lessons 1.0.0: 0 error(s)"), "{out}");
}

#[test]
fn a_directory_without_pack_json_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (ok, out) = validate(dir.path());
    assert!(!ok);
    assert!(out.contains("cannot read pack.json"), "{out}");
}

#[test]
fn a_pack_needing_a_newer_engine_fails() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pack.json"),
        r#"{"schema":1,"kind":"songs","id":"s","name":"S","version":"1.0.0",
            "requires":{"harmonicon":">=999"}}"#,
    )
    .unwrap();
    let (ok, out) = validate(dir.path());
    assert!(!ok);
    assert!(out.contains(">=999"), "{out}");
}
