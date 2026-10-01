// SPDX-License-Identifier: MIT

//! Checks a song pack's content the way the game will read it, so a broken
//! chart is caught in the pack's CI rather than when a player picks it.
//! `validate-pack` (a binary of the root package) is the front end; lesson
//! packs reuse the chart checks here from `lessons::validate`.

use std::path::{Path, PathBuf};

use harmonicon_core::chart::HarpChart;
use harmonicon_core::harmonica::{Harmonica, chromatic_harp};
use harmonicon_core::pitch_map::HARP_KEYS;

use super::validate_and_migrate_chart;

/// Problems found in a pack. `errors` fail validation; `warnings` describe
/// something that works but looks unintended.
#[derive(Debug, Default)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn error(&mut self, at: &Path, root: &Path, message: impl std::fmt::Display) {
        self.errors
            .push(format!("{}: {message}", relative(at, root)));
    }

    pub fn warning(&mut self, at: &Path, root: &Path, message: impl std::fmt::Display) {
        self.warnings
            .push(format!("{}: {message}", relative(at, root)));
    }
}

fn relative(path: &Path, root: &Path) -> String {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    if rel.is_empty() { ".".into() } else { rel }
}

/// Immediate, non-hidden subdirectories of `dir`, sorted. Hidden ones are a
/// checkout's own (`.git`, `.github`), not content.
pub fn content_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

/// Validates one `.harpchart` exactly as the loader would (migration,
/// schema, format version), plus the one rule the schema can't express: a
/// chromatic harmonica's layout must be a real instrument's.
pub fn check_chart(path: &Path, root: &Path, report: &mut Report) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => return report.error(path, root, format!("cannot read: {e}")),
    };
    let mut value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(e) => return report.error(path, root, format!("not valid JSON: {e}")),
    };
    if let Err(e) = validate_and_migrate_chart(&mut value) {
        return report.error(path, root, e);
    }
    let Ok(chart) = serde_json::from_value::<HarpChart>(value) else {
        return; // the loader's own deserialize already passed above
    };
    if let Harmonica::Chromatic { .. } = &chart.harmonica
        && !HARP_KEYS
            .iter()
            .any(|key| chromatic_layout(&chromatic_harp(key)) == chromatic_layout(&chart.harmonica))
    {
        report.error(
            path,
            root,
            "chromatic layout matches no key's standard tuning; no real harmonica has these holes",
        );
    }
}

/// A chromatic's four note tables, for comparing two harmonicas by layout.
fn chromatic_layout(harp: &Harmonica) -> Option<Vec<Vec<String>>> {
    match harp {
        Harmonica::Chromatic {
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

/// `<artist>/<song>/song/` must hold a `.harpchart` or an importable score
/// (MIDI, Guitar Pro, MuseScore, MusicXML), the same rule the song scan
/// uses; every `.harpchart` must load.
pub fn validate_song_pack(root: &Path) -> Report {
    let mut report = Report::default();
    let mut songs = 0;
    for artist in content_dirs(root) {
        for song in content_dirs(&artist) {
            songs += 1;
            let files: Vec<PathBuf> = std::fs::read_dir(song.join("song"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            let charts: Vec<&PathBuf> = files
                .iter()
                .filter(|p| p.extension().is_some_and(|e| e == "harpchart"))
                .collect();
            if charts.is_empty()
                && !files
                    .iter()
                    .any(|p| harmonicon_score::is_importable_file(p))
            {
                report.error(&song, root, "no .harpchart or importable score under song/");
            }
            for chart in charts {
                check_chart(chart, root, &mut report);
            }
        }
    }
    if songs == 0 {
        report.error(
            root,
            root,
            "no songs found (expected <artist>/<song>/song/)",
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real chart, from the fixture lesson pack.
    pub(crate) const CHART: &str = include_str!(
        "../../../../tests/fixtures/lesson-pack/01_basics/01_first/song/chart.harpchart"
    );

    fn write(root: &Path, rel: &str, contents: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }

    #[test]
    fn a_valid_pack_passes() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Bach/Minuet/song/chart.harpchart", CHART);
        write(dir.path(), ".git/x/y/song/chart.harpchart", "not json");
        let report = validate_song_pack(dir.path());
        assert!(report.errors.is_empty(), "{:?}", report.errors);
    }

    #[test]
    fn a_song_without_a_chart_and_a_broken_chart_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Bach/Empty/song/readme.txt", "");
        write(
            dir.path(),
            "Bach/Broken/song/chart.harpchart",
            r#"{"metadata":{}}"#,
        );
        let report = validate_song_pack(dir.path());
        assert_eq!(report.errors.len(), 2, "{:?}", report.errors);
        assert!(
            report.errors[0].starts_with("Bach/Broken/song/chart.harpchart"),
            "{:?}",
            report.errors
        );
        assert!(
            report.errors[1].starts_with("Bach/Empty"),
            "{:?}",
            report.errors
        );
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/song-pack")
    }

    #[test]
    fn the_fixture_pack_is_valid() {
        let report = validate_song_pack(&fixture());
        assert!(report.errors.is_empty(), "{:#?}", report.errors);
    }

    /// A chart can be schema-valid and still describe a chromatic no one
    /// makes: a C major scale one note per hole, stopping at A5. The
    /// schema can't see that; this check does.
    #[test]
    fn a_chromatic_with_an_impossible_layout_is_reported() {
        let source = fixture().join("Ludwig van Beethoven/Fur Elise/song/chart.harpchart");
        let mut chart: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&source).unwrap()).unwrap();
        assert_eq!(
            chart["harmonica"]["type"], "chromatic",
            "the fixture must be a chromatic"
        );
        let blow = chart["harmonica"]["layout"]["blow"].as_array_mut().unwrap();
        blow.swap(0, 1);

        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Beethoven/Fur Elise/song/chart.harpchart",
            &chart.to_string(),
        );
        let errors = validate_song_pack(dir.path()).errors;
        assert!(
            errors.iter().any(|e| e.contains("no real harmonica")),
            "{errors:#?}"
        );
    }

    #[test]
    fn an_empty_pack_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(validate_song_pack(dir.path()).errors.len(), 1);
    }
}
