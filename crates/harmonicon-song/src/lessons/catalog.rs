// SPDX-License-Identifier: MIT

//! Discovery of every lesson (`<unit>/<lesson>/lesson.json`, in a lesson
//! pack or the `~/Harmonicon/lessons` drop folder) and unit grouping for the
//! menu. The game ships no lessons of its own: they come from the
//! configured lesson packs (`harmonicon_platform::content_packs`).

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use bevy::prelude::*;

use super::manifest::{LessonManifest, parse_lesson};
use harmonicon_platform::assets_management::ExternalFolderChanged;
use harmonicon_platform::content_packs::{ContentPacks, ContentPacksChanged, ContentPacksSet};

/// wasm can neither download a pack nor read a directory, so its lessons
/// are a pack bundled at build time: `build.rs` embeds each `lesson.json`
/// from the directory `HARMONICON_LESSONS_DIR` names, with `include_str!`,
/// and this `include!()`s the result. The web bundle must serve that same
/// pack under `assets/lessons/` for the charts to load, which
/// `scripts/build_web.sh` arranges. Every other target
/// reads its packs from disk.
#[cfg(target_arch = "wasm32")]
mod bundled {
    include!(concat!(env!("OUT_DIR"), "/lesson_manifest.rs"));
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

/// Scans `root` (a lesson pack's checkout or the external
/// `~/Harmonicon/lessons` drop folder) for
/// `<unit_dir>/<lesson_dir>/lesson.json`, sorted by directory name so the
/// `01_`/`02_` prefixes give the curriculum order. `asset_prefix` is what
/// the chart's engine-facing asset path starts with (a pack's
/// `"packs://<slug>"`, `"external://lessons"` for the drop folder — the same
/// `AssetSource` scheme prefixes
/// `assets_management::scan_artist_song` uses for songs; a temp dir in
/// tests). Invalid manifests are logged and skipped — one bad lesson must
/// not take down the whole menu. Hidden directories (a pack's `.git`) are
/// not units.
#[cfg(not(target_arch = "wasm32"))]
fn scan_lessons_root(root: &Path, asset_prefix: &str) -> Vec<LessonEntry> {
    use harmonicon_platform::assets_management::is_visible_dir;

    let mut entries = Vec::new();
    let mut unit_dirs: Vec<_> = match std::fs::read_dir(root) {
        Ok(rd) => rd.flatten().filter(is_visible_dir).map(|e| e.path()).collect(),
        Err(_) => return entries,
    };
    unit_dirs.sort();

    for unit_dir in unit_dirs {
        let Ok(rd) = std::fs::read_dir(&unit_dir) else {
            continue;
        };
        let mut lesson_dirs: Vec<_> =
            rd.flatten().filter(is_visible_dir).map(|e| e.path()).collect();
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
            entries.push(LessonEntry { manifest, chart_asset_path });
        }
    }
    entries
}

/// Every usable lesson pack in configured order, then, if present, an
/// external `~/Harmonicon/lessons` drop folder — mirroring
/// `assets_management::scan_all_songs_into`. Packs come first, so the
/// curriculum's ordering and prerequisites are unaffected by whatever a
/// player drops in.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn scan_all_lessons(packs: Option<&ContentPacks>) -> Vec<LessonEntry> {
    let mut entries = scan_lesson_packs(packs);
    if let Some(external_root) = dirs::home_dir().map(|h| h.join("Harmonicon/lessons")) {
        entries.extend(scan_lessons_root(&external_root, "external://lessons"));
    }
    dedupe_by_id(entries)
}

/// Packs only: an Android app has no `~/Harmonicon` a player could put a
/// lesson in.
#[cfg(target_os = "android")]
fn scan_all_lessons(packs: Option<&ContentPacks>) -> Vec<LessonEntry> {
    dedupe_by_id(scan_lesson_packs(packs))
}

#[cfg(not(target_arch = "wasm32"))]
fn scan_lesson_packs(packs: Option<&ContentPacks>) -> Vec<LessonEntry> {
    packs
        .into_iter()
        .flat_map(|p| p.usable(harmonicon_packs::pack::PackKind::Lessons))
        .flat_map(|pack| scan_lessons_root(&pack.root, &pack.asset_prefix()))
        .collect()
}

/// Keeps the first lesson with each id. An id is a profile key and a
/// prerequisite target, so two lessons sharing one would make progress and
/// unlocking ambiguous; the earlier source (an earlier pack, packs before the
/// drop folder) wins.
fn dedupe_by_id(entries: Vec<LessonEntry>) -> Vec<LessonEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|entry| {
            let fresh = seen.insert(entry.manifest.id.clone());
            if !fresh {
                warn!(
                    "Skipping lesson {:?} from {:?}: an earlier lesson already uses that id",
                    entry.manifest.id, entry.chart_asset_path
                );
            }
            fresh
        })
        .collect()
}

/// The pack bundled at build time (see the `bundled` module above), and
/// nothing else: a browser has no `~/Harmonicon`.
///
/// Deliberately mirrors `scan_lessons_root`'s error handling — an invalid
/// manifest is logged and skipped, never fatal — even though the pack's own
/// CI (`validate-pack`) should have refused it first.
#[cfg(target_arch = "wasm32")]
fn scan_all_lessons(_packs: Option<&ContentPacks>) -> Vec<LessonEntry> {
    let bundled = bundled::BUNDLED_LESSONS.iter().filter_map(|(unit, lesson, json)| {
        let manifest = match parse_lesson(json.as_bytes()) {
            Ok(m) => m,
            Err(err) => {
                warn!("Skipping invalid lesson {unit}/{lesson}: {err}");
                return None;
            }
        };
        let chart_asset_path =
            manifest.chart.as_ref().map(|chart| format!("lessons/{unit}/{lesson}/{chart}"));
        Some(LessonEntry { manifest, chart_asset_path })
    });
    dedupe_by_id(bundled.collect())
}

fn scan_lessons(mut available: ResMut<AvailableLessons>, packs: Option<Res<ContentPacks>>) {
    available.0 = scan_all_lessons(packs.as_deref());
    let ids: Vec<&str> = available.0.iter().map(|l| l.manifest.id.as_str()).collect();
    info!("Found {} lesson(s): {:?}", ids.len(), ids);
}

/// Fired only when a *live* filesystem event under `~/Harmonicon/lessons`
/// actually triggered a rescan — see `assets_management::watch::
/// ExternalFolderChanged`'s doc comment for why this is a message rather
/// than a bare `AvailableLessons::is_changed()` poll.
#[derive(Message)]
pub struct LessonsRescanned;

/// This module's own consumer of the shared `~/Harmonicon` watcher's
/// generic change signal — `assets_management` stays agnostic of what a
/// lesson is (it's low-level shared vocabulary; lessons is a feature built
/// on top, so the dependency points this way and not the reverse), while
/// still reusing its one OS-level watch instead of starting a second one
/// scoped to `lessons/` alone.
///
/// A pack being installed, updated or removed (`ContentPacksChanged`)
/// rescans too.
fn rescan_lessons_on_external_change(
    mut changed: MessageReader<ExternalFolderChanged>,
    mut packs_changed: MessageReader<ContentPacksChanged>,
    mut available: ResMut<AvailableLessons>,
    packs: Option<Res<ContentPacks>>,
    mut rescanned: MessageWriter<LessonsRescanned>,
) {
    let external = changed.read().any(|ev| ev.top_level_dirs.contains("lessons"));
    let dirty = packs_changed.read().count() > 0 || external;
    if dirty {
        available.0 = scan_all_lessons(packs.as_deref());
        let ids: Vec<&str> = available.0.iter().map(|l| l.manifest.id.as_str()).collect();
        info!("Re-scanned lessons: found {} lesson(s): {:?}", ids.len(), ids);
        rescanned.write(LessonsRescanned);
    }
}

pub struct LessonsPlugin;

impl Plugin for LessonsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableLessons>()
            .add_message::<LessonsRescanned>()
            // Read by the rescan below; registered here too so an app
            // without `ContentPacksPlugin` (tests, tools) still runs.
            .add_message::<ContentPacksChanged>()
            .add_systems(Startup, scan_lessons.after(ContentPacksSet))
            .add_systems(Update, rescan_lessons_on_external_change.after(ContentPacksSet));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::Schedule;
    use harmonicon_packs::pack::PackKind;

    fn entry(id: &str, unit: &str) -> LessonEntry {
        LessonEntry {
            manifest: LessonManifest {
                id: id.into(),
                unit: unit.into(),
                optional: false,
                track: None,
                training: None,
                title_key: format!("lesson-{id}-title"),
                body_key: format!("lesson-{id}-body"),
                chart: None,
                aural: false,
                prerequisites: Vec::new(),
                pass_criteria: None,
                progression: None,
                scale: None,
                diagram: None,
                widgets: Vec::new(),
                position_cycle: false,
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

    #[test]
    fn an_external_asset_prefix_builds_an_external_source_chart_path() {
        let dir =
            std::env::temp_dir().join(format!("harmonicon_lessons_ext_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("01_basics/01_first/lesson.json");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(
            &p,
            r#"{"id":"first","unit":"basics","title_key":"t","body_key":"b",
                "chart":"song/chart.harpchart"}"#,
        )
        .unwrap();

        let entries = scan_lessons_root(&dir, "external://lessons");
        assert_eq!(
            entries[0].chart_asset_path.as_deref(),
            Some("external://lessons/01_basics/01_first/song/chart.harpchart")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lesson_pack_builds_packs_source_chart_paths_and_skips_git() {
        use harmonicon_packs::pack::PackManifest;
        use harmonicon_packs::repo::RepoSpec;
        use harmonicon_platform::content_packs::{PackEntry, PackStatus};

        let dir =
            std::env::temp_dir().join(format!("harmonicon_lessons_pack_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for rel in ["01_basics/01_first/lesson.json", ".git/01_x/lesson.json"] {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(
                &p,
                r#"{"id":"first","unit":"basics","title_key":"t","body_key":"b",
                    "chart":"song/chart.harpchart"}"#,
            )
            .unwrap();
        }
        let manifest = PackManifest::parse(
            br#"{"schema":1,"kind":"lessons","id":"p","name":"P","version":"1.0.0"}"#,
        )
        .unwrap()
        .unwrap();
        let packs = ContentPacks(vec![PackEntry {
            kind: PackKind::Lessons,
            spec: RepoSpec::Local { path: dir.clone() },
            slug: "core".into(),
            root: dir.clone(),
            status: PackStatus::Ready { manifest, commit: None },
        }]);

        let entries = scan_lesson_packs(Some(&packs));
        assert_eq!(entries.len(), 1, "the .git copy must not be scanned");
        assert_eq!(
            entries[0].chart_asset_path.as_deref(),
            Some("packs://core/01_basics/01_first/song/chart.harpchart")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_first_lesson_with_an_id_wins() {
        let mut later = entry("a", "second-source");
        later.chart_asset_path = Some("packs://p/x".into());
        let kept = dedupe_by_id(vec![entry("a", "first-source"), entry("b", "u"), later]);
        let units: Vec<&str> = kept.iter().map(|l| l.manifest.unit.as_str()).collect();
        assert_eq!(units, ["first-source", "u"]);
    }

    // ── scan_lessons (Startup system) ────────────────────────────────────────

    #[test]
    fn scan_lessons_does_not_duplicate_entries_when_run_again() {
        let mut world = World::new();
        world.init_resource::<AvailableLessons>();
        let mut schedule = Schedule::default();
        schedule.add_systems(scan_lessons);

        schedule.run(&mut world);
        let first = world.resource::<AvailableLessons>().0.len();

        schedule.run(&mut world);
        let second = world.resource::<AvailableLessons>().0.len();

        assert_eq!(first, second);
    }
}
