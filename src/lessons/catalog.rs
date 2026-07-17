// SPDX-License-Identifier: MIT

//! Startup discovery of every bundled lesson (`assets/lessons/<unit>/
//! <lesson>/lesson.json`) and unit grouping for the menu.

use std::path::Path;

use bevy::prelude::*;

use super::manifest::{LessonManifest, parse_lesson};

/// A discovered lesson: its manifest plus the asset path its chart loads
/// from (`None` for instructional-only lessons).
#[derive(Debug, Clone, PartialEq)]
pub struct LessonEntry {
    pub manifest: LessonManifest,
    pub chart_asset_path: Option<String>,
}

/// Every lesson found at startup, in menu order (sorted by
/// `<unit_dir>/<lesson_dir>` name — the `01_`/`02_` prefixes are the
/// ordering mechanism). Units are displayed in order of first appearance.
#[derive(Resource, Default)]
pub struct AvailableLessons(pub Vec<LessonEntry>);

/// Groups lessons by unit, preserving both the lesson order and the order
/// each unit first appears in. Returns `(unit, lessons-in-that-unit)` pairs
/// ready for the menu to render as sections.
pub fn group_by_unit(lessons: &[LessonEntry]) -> Vec<(&str, Vec<&LessonEntry>)> {
    let mut units: Vec<(&str, Vec<&LessonEntry>)> = Vec::new();
    for entry in lessons {
        let unit = entry.manifest.unit.as_str();
        match units.iter_mut().find(|(u, _)| *u == unit) {
            Some((_, list)) => list.push(entry),
            None => units.push((unit, vec![entry])),
        }
    }
    units
}

/// Scans `root` (the bundled `assets/lessons` tree) for
/// `<unit_dir>/<lesson_dir>/lesson.json`, sorted by directory name so the
/// `01_`/`02_` prefixes give the curriculum order. `asset_prefix` is what
/// the chart's engine-facing asset path starts with (`"lessons"` in the
/// game; a temp dir in tests). Invalid manifests are logged and skipped —
/// one bad lesson must not take down the whole menu.
fn scan_lessons_root(root: &Path, asset_prefix: &str) -> Vec<LessonEntry> {
    let mut entries = Vec::new();
    let mut unit_dirs: Vec<_> = match std::fs::read_dir(root) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect(),
        Err(_) => return entries,
    };
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
            let manifest_path = lesson_dir.join("lesson.json");
            let Ok(bytes) = std::fs::read(&manifest_path) else {
                continue; // not a lesson dir
            };
            let manifest = match parse_lesson(&bytes) {
                Ok(m) => m,
                Err(err) => {
                    warn!("Skipping invalid lesson {}: {err}", manifest_path.display());
                    continue;
                }
            };
            // The chart loads through the asset server, whose paths are
            // relative to the assets root — rebuild the path from the two
            // directory names rather than the scan's absolute path.
            let chart_asset_path = manifest.chart.as_ref().and_then(|chart| {
                let unit = unit_dir.file_name()?.to_str()?;
                let lesson = lesson_dir.file_name()?.to_str()?;
                Some(format!("{asset_prefix}/{unit}/{lesson}/{chart}"))
            });
            entries.push(LessonEntry {
                manifest,
                chart_asset_path,
            });
        }
    }
    entries
}

fn scan_lessons(mut available: ResMut<AvailableLessons>) {
    available.0 = scan_lessons_root(Path::new("assets/lessons"), "lessons");
    let ids: Vec<&str> = available.0.iter().map(|l| l.manifest.id.as_str()).collect();
    info!("Found {} lesson(s): {:?}", ids.len(), ids);
}

pub struct LessonsPlugin;

impl Plugin for LessonsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableLessons>()
            .add_systems(Startup, scan_lessons);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, unit: &str) -> LessonEntry {
        LessonEntry {
            manifest: LessonManifest {
                id: id.into(),
                unit: unit.into(),
                title_key: format!("lesson-{id}-title"),
                body_key: format!("lesson-{id}-body"),
                chart: None,
                prerequisites: Vec::new(),
                pass_criteria: None,
                progression: None,
            },
            chart_asset_path: None,
        }
    }

    // ── group_by_unit ─────────────────────────────────────────────────────────

    #[test]
    fn grouping_preserves_lesson_and_unit_order() {
        let lessons = [
            entry("a1", "blowing"),
            entry("a2", "blowing"),
            entry("r1", "rhythm"),
            entry("a3", "blowing"),
        ];
        let grouped = group_by_unit(&lessons);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "blowing");
        let ids: Vec<&str> = grouped[0].1.iter().map(|l| l.manifest.id.as_str()).collect();
        assert_eq!(ids, ["a1", "a2", "a3"]);
        assert_eq!(grouped[1].0, "rhythm");
    }

    // ── scan_lessons_root ─────────────────────────────────────────────────────

    #[test]
    fn scan_orders_by_directory_name_and_builds_chart_paths() {
        let dir = std::env::temp_dir().join(format!("harmonicon_lessons_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let write = |rel: &str, contents: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, contents).unwrap();
        };
        write(
            "02_rhythm/01_twelve_bar/lesson.json",
            r#"{"id":"twelve-bar","unit":"rhythm","title_key":"t","body_key":"b"}"#,
        );
        write(
            "01_blowing/01_single_note/lesson.json",
            r#"{"id":"single-note","unit":"blowing","title_key":"t","body_key":"b",
                "chart":"song/chart.harpchart"}"#,
        );
        write("01_blowing/99_broken/lesson.json", "{ not json");

        let entries = scan_lessons_root(&dir, "lessons");
        let ids: Vec<&str> = entries.iter().map(|l| l.manifest.id.as_str()).collect();
        // Broken manifest skipped; unit dirs sorted (01_ before 02_).
        assert_eq!(ids, ["single-note", "twelve-bar"]);
        assert_eq!(
            entries[0].chart_asset_path.as_deref(),
            Some("lessons/01_blowing/01_single_note/song/chart.harpchart")
        );
        assert_eq!(entries[1].chart_asset_path, None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
