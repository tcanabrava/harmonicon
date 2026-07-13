// SPDX-License-Identifier: MIT

//! Lesson curriculum: manifest loading, discovery, unlock gating, and pass
//! judgment. See `docs/lessons_plan.md` for the full design.
//!
//! A lesson is a directory under `assets/lessons/<unit_dir>/<lesson_dir>/`
//! holding a `lesson.json` manifest (schema-validated against
//! `assets/lesson_schema.dtd.json`) and, for chart-backed lessons, a normal
//! song folder (`song/chart.harpchart` + `song/music.ogg` + artwork) that
//! plays through the ordinary gameplay pipeline — lessons deliberately add
//! no scoring machinery of their own, so they stay as honest as regular
//! play. Directory names give the menu order (`01_...`, `02_...`); the
//! manifest's `id` is the stable identity used for profile records and
//! prerequisites.
//!
//! All user-visible lesson text is localized: the manifest carries Fluent
//! *keys* (`title_key`/`body_key`, plus `lesson-unit-<unit>` for the unit
//! heading), never display strings.

use bevy::prelude::*;
use serde::Deserialize;
use std::path::Path;

const SCHEMA: &str = include_str!("../assets/lesson_schema.dtd.json");

/// How a chart-backed lesson is judged when its run reaches the results
/// screen. `None` on [`LessonManifest`] means finishing at all counts.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PassCriteria {
    /// Minimum overall weighted accuracy (`results::accuracy`), 0..1.
    Accuracy { threshold: f32 },
    /// Minimum accuracy on one technique bucket — the same name vocabulary
    /// `SongStats`/`PlayerProfile::technique_best_accuracy` use
    /// (`"bend"`, `"wah-wah"`, ...).
    Technique { technique: String, threshold: f32 },
}

/// One `lesson.json`, as authored. See `assets/lesson_schema.dtd.json` for
/// field semantics.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LessonManifest {
    pub id: String,
    pub unit: String,
    pub title_key: String,
    pub body_key: String,
    #[serde(default)]
    pub chart: Option<String>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub pass_criteria: Option<PassCriteria>,
}

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

/// Present while a lesson run is in flight (from the reader's Start button,
/// through `SongLoading`/`Playing`/`Results` and any retries). Its presence
/// is what tells the results screen to judge pass criteria instead of
/// recording a song best, `setup_adaptive_difficulty` to leave every note
/// unlocked, and `route_menu_entry` to land back on the lesson list.
/// Removed on returning to the menu.
#[derive(Resource, Debug, Clone)]
pub struct LessonContext {
    pub lesson_id: String,
    pub pass_criteria: Option<PassCriteria>,
}

/// Parses and schema-validates one `lesson.json`'s bytes.
pub fn parse_lesson(bytes: &[u8]) -> Result<LessonManifest, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("JSON parse error: {e}"))?;
    let schema: serde_json::Value =
        serde_json::from_str(SCHEMA).expect("embedded lesson schema must be valid JSON");
    let validator =
        jsonschema::validator_for(&schema).map_err(|e| format!("schema is invalid: {e}"))?;
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("{e} (at /{})", e.instance_path))
        .collect();
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    serde_json::from_value(value).map_err(|e| format!("deserialize error: {e}"))
}

/// Whether a lesson is playable yet: every prerequisite id has a passed
/// record. `passed_ids` is the caller's view of `PlayerProfile::lessons`
/// (only the ids with `passed == true`).
pub fn is_unlocked(manifest: &LessonManifest, passed_ids: &[&str]) -> bool {
    manifest
        .prerequisites
        .iter()
        .all(|p| passed_ids.contains(&p.as_str()))
}

/// Judges a finished lesson run. `accuracy` is the overall weighted accuracy
/// (`results::accuracy`); `technique_accuracy` pairs technique bucket names
/// with their per-run accuracy — the same slice `results` already builds for
/// `profile::record_play`. A criteria-less lesson passes by finishing; a
/// `Technique` criterion over a bucket the run never exercised fails (an
/// empty run can't demonstrate the technique).
pub fn lesson_passed(
    criteria: Option<&PassCriteria>,
    accuracy: f32,
    technique_accuracy: &[(&str, f32)],
) -> bool {
    match criteria {
        None => true,
        Some(PassCriteria::Accuracy { threshold }) => accuracy >= *threshold,
        Some(PassCriteria::Technique {
            technique,
            threshold,
        }) => technique_accuracy
            .iter()
            .find(|(name, _)| name == technique)
            .is_some_and(|(_, acc)| *acc >= *threshold),
    }
}

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

    fn manifest(id: &str, prereqs: &[&str]) -> LessonManifest {
        LessonManifest {
            id: id.into(),
            unit: "blowing".into(),
            title_key: format!("lesson-{id}-title"),
            body_key: format!("lesson-{id}-body"),
            chart: None,
            prerequisites: prereqs.iter().map(|s| s.to_string()).collect(),
            pass_criteria: None,
        }
    }

    fn entry(id: &str, unit: &str) -> LessonEntry {
        LessonEntry {
            manifest: LessonManifest {
                unit: unit.into(),
                ..manifest(id, &[])
            },
            chart_asset_path: None,
        }
    }

    // ── parse_lesson ──────────────────────────────────────────────────────────

    #[test]
    fn parses_a_minimal_instructional_lesson() {
        let m = parse_lesson(
            br#"{"id":"twelve-bar","unit":"rhythm","title_key":"t","body_key":"b"}"#,
        )
        .unwrap();
        assert_eq!(m.id, "twelve-bar");
        assert_eq!(m.chart, None);
        assert!(m.prerequisites.is_empty());
        assert_eq!(m.pass_criteria, None);
    }

    #[test]
    fn parses_a_full_chart_lesson() {
        let m = parse_lesson(
            br#"{
                "id": "hand-wah", "unit": "blowing",
                "title_key": "t", "body_key": "b",
                "chart": "song/chart.harpchart",
                "prerequisites": ["single-note"],
                "pass_criteria": {"type": "technique", "technique": "wah-wah", "threshold": 0.5}
            }"#,
        )
        .unwrap();
        assert_eq!(m.chart.as_deref(), Some("song/chart.harpchart"));
        assert_eq!(m.prerequisites, vec!["single-note"]);
        assert_eq!(
            m.pass_criteria,
            Some(PassCriteria::Technique {
                technique: "wah-wah".into(),
                threshold: 0.5
            })
        );
    }

    #[test]
    fn parses_a_clean_attack_technique_criterion() {
        // The single-note lesson's actual pass criterion — "clean-attack" is
        // a `SongStats` bucket like "bend"/"wah-wah", not a chart modifier,
        // but it goes through the same `Technique` criterion machinery.
        let m = parse_lesson(
            br#"{"id":"single-note","unit":"blowing","title_key":"t","body_key":"b",
                 "pass_criteria":{"type":"technique","technique":"clean-attack","threshold":0.6}}"#,
        )
        .unwrap();
        assert_eq!(
            m.pass_criteria,
            Some(PassCriteria::Technique {
                technique: "clean-attack".into(),
                threshold: 0.6
            })
        );
    }

    #[test]
    fn rejects_a_manifest_missing_required_fields() {
        let err = parse_lesson(br#"{"id":"x","unit":"blowing"}"#).unwrap_err();
        assert!(err.contains("title_key"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_an_unknown_field() {
        // additionalProperties: false — typos in hand-authored manifests must
        // fail loudly, not silently no-op.
        let err = parse_lesson(
            br#"{"id":"x","unit":"u","title_key":"t","body_key":"b","chrat":"oops"}"#,
        )
        .unwrap_err();
        assert!(err.contains("chrat"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_an_out_of_range_threshold() {
        let err = parse_lesson(
            br#"{"id":"x","unit":"u","title_key":"t","body_key":"b",
                 "pass_criteria":{"type":"accuracy","threshold":1.5}}"#,
        )
        .unwrap_err();
        assert!(err.contains("1.5"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_an_unknown_technique_name() {
        // The enum in the schema pins the technique vocabulary to what
        // SongStats actually tracks — a typo'd bucket could never pass.
        let err = parse_lesson(
            br#"{"id":"x","unit":"u","title_key":"t","body_key":"b",
                 "pass_criteria":{"type":"technique","technique":"wah","threshold":0.5}}"#,
        )
        .unwrap_err();
        assert!(!err.is_empty());
    }

    // ── is_unlocked ───────────────────────────────────────────────────────────

    #[test]
    fn a_lesson_with_no_prerequisites_is_unlocked() {
        assert!(is_unlocked(&manifest("a", &[]), &[]));
    }

    #[test]
    fn a_lesson_unlocks_only_when_every_prerequisite_is_passed() {
        let m = manifest("c", &["a", "b"]);
        assert!(!is_unlocked(&m, &[]));
        assert!(!is_unlocked(&m, &["a"]));
        assert!(is_unlocked(&m, &["a", "b"]));
    }

    // ── lesson_passed ─────────────────────────────────────────────────────────

    #[test]
    fn no_criteria_means_finishing_passes() {
        assert!(lesson_passed(None, 0.0, &[]));
    }

    #[test]
    fn accuracy_criterion_compares_against_overall_accuracy() {
        let c = PassCriteria::Accuracy { threshold: 0.6 };
        assert!(!lesson_passed(Some(&c), 0.59, &[]));
        assert!(lesson_passed(Some(&c), 0.6, &[]));
    }

    #[test]
    fn technique_criterion_reads_the_matching_bucket() {
        let c = PassCriteria::Technique {
            technique: "wah-wah".into(),
            threshold: 0.5,
        };
        let per_technique = [("normal", 1.0_f32), ("wah-wah", 0.4)];
        assert!(!lesson_passed(Some(&c), 1.0, &per_technique));
        let per_technique = [("normal", 0.0_f32), ("wah-wah", 0.5)];
        assert!(lesson_passed(Some(&c), 0.0, &per_technique));
    }

    #[test]
    fn technique_criterion_fails_when_the_bucket_was_never_exercised() {
        let c = PassCriteria::Technique {
            technique: "wah-wah".into(),
            threshold: 0.5,
        };
        assert!(!lesson_passed(Some(&c), 1.0, &[("normal", 1.0)]));
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
