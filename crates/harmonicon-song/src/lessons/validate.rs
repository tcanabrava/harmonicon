// SPDX-License-Identifier: MIT

//! Checks a lesson pack the way the game will read it. A pack is released on
//! its own schedule, so these checks run in the pack's CI (through the
//! `validate-pack` binary) rather than this repository's tests.
//!
//! Errors are what would break in the game: a manifest the schema refuses, a
//! chart that won't load, a prerequisite naming no lesson, a cycle, a
//! prerequisite the unit order can never satisfy, a string that would show
//! as its raw key, a translation file Fluent can't parse. Warnings are what
//! works but is probably unintended.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::song::validate::{Report, check_chart, content_dirs};

use super::graph::LessonGraph;
use super::manifest::{LessonManifest, LessonWidget, parse_lesson};
use super::units::{core_prerequisites_on_optional, crossing_prerequisites};

/// The engine's own strings, which a lesson may use without defining (the
/// widget chrome lives there).
const ENGINE_STRINGS: &str = include_str!("../../../../assets/locales/en-US/main/ui.ftl");

/// The language every pack must provide: it is the game's fallback, so a
/// key missing from it shows as a raw key to every player whose own
/// language the pack doesn't have.
const FALLBACK_LANGUAGE: &str = "en-US";

/// Message ids an FTL source defines, or the parser's complaints.
fn ftl_keys(source: &str) -> Result<BTreeSet<String>, Vec<String>> {
    match fluent_syntax::parser::parse(source) {
        Ok(resource) => Ok(message_ids(&resource)),
        Err((_, errors)) => Err(errors.iter().map(|e| format!("{e:?}")).collect()),
    }
}

fn message_ids(resource: &fluent_syntax::ast::Resource<&str>) -> BTreeSet<String> {
    resource
        .body
        .iter()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
            _ => None,
        })
        .collect()
}

/// Reads `dir/locales/*.ftl`, reporting syntax errors and keys missing from
/// any language that the fallback language defines. Returns the fallback
/// language's keys.
fn check_locales_dir(dir: &Path, root: &Path, report: &mut Report) -> BTreeSet<String> {
    let locales = dir.join("locales");
    let mut by_lang: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in std::fs::read_dir(&locales).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "ftl") {
            continue;
        }
        let lang = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if lang.parse::<unic_langid::LanguageIdentifier>().is_err() {
            report.error(&path, root, format!("{lang:?} is not a language tag"));
            continue;
        }
        match std::fs::read_to_string(&path) {
            Err(e) => report.error(&path, root, format!("cannot read: {e}")),
            Ok(source) => match ftl_keys(&source) {
                Ok(keys) => {
                    by_lang.insert(lang, keys);
                }
                Err(errors) => {
                    report.error(&path, root, format!("Fluent syntax: {}", errors.join("; ")))
                }
            },
        }
    }
    let Some(fallback) = by_lang.get(FALLBACK_LANGUAGE).cloned() else {
        if !by_lang.is_empty() {
            report.error(
                &locales,
                root,
                format!("no {FALLBACK_LANGUAGE}.ftl beside the other translations"),
            );
        }
        return BTreeSet::new();
    };
    for (lang, keys) in &by_lang {
        let missing: Vec<&String> = fallback.difference(keys).collect();
        if !missing.is_empty() {
            report.error(
                &locales.join(format!("{lang}.ftl")),
                root,
                format!("missing {missing:?}, which {FALLBACK_LANGUAGE} defines"),
            );
        }
    }
    fallback
}

/// The Fluent keys a manifest makes the game look up.
fn needed_keys(manifest: &LessonManifest) -> Vec<String> {
    let mut keys = vec![
        manifest.title_key.clone(),
        manifest.body_key.clone(),
        format!("lesson-unit-{}", manifest.unit),
    ];
    if let Some(track) = &manifest.track {
        keys.push(format!("lesson-track-{track}"));
    }
    for widget in &manifest.widgets {
        if let Some(key) = widget.title_key() {
            keys.push(key.to_string());
        }
    }
    keys
}

/// A tab cell the gameplay notation can draw: breath sign, hole 1-10, and an
/// optional bend, overnote or slide suffix. Anything without a breath sign
/// is a label (a chord name, a section marker) and always fine.
fn valid_tab_token(token: &str) -> bool {
    let Some(rest) = token.strip_prefix(['+', '-']) else {
        return true;
    };
    let digit_count = rest.bytes().take_while(u8::is_ascii_digit).count();
    let (hole, suffix) = rest.split_at(digit_count);
    matches!(hole.parse::<u8>(), Ok(1..=10))
        && (suffix.is_empty()
            || suffix == "o"
            || suffix == "*"
            || (suffix.len() <= 3 && suffix.bytes().all(|b| b == b'\'')))
}

/// Validates the pack at `root`: `<unit>/<lesson>/lesson.json`, translations
/// in `locales/` at the root and beside each lesson.
pub fn validate_lesson_pack(root: &Path) -> Report {
    let mut report = Report::default();
    let shared_keys = check_locales_dir(root, root, &mut report);
    let engine_keys = ftl_keys(ENGINE_STRINGS).unwrap_or_default();

    let mut lessons: Vec<(PathBuf, LessonManifest)> = Vec::new();
    for unit in content_dirs(root)
        .into_iter()
        .filter(|d| !d.ends_with("locales"))
    {
        for dir in content_dirs(&unit) {
            let manifest_path = dir.join("lesson.json");
            let bytes = match std::fs::read(&manifest_path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    report.error(&dir, root, "no lesson.json");
                    continue;
                }
            };
            let manifest = match parse_lesson(&bytes) {
                Ok(m) => m,
                Err(e) => {
                    report.error(&manifest_path, root, e);
                    continue;
                }
            };

            if let Some(chart) = &manifest.chart {
                let chart_path = dir.join(chart);
                if chart_path.is_file() {
                    check_chart(&chart_path, root, &mut report);
                } else {
                    report.error(&dir, root, format!("chart {chart} does not exist"));
                }
            }

            let own_keys = check_locales_dir(&dir, root, &mut report);
            for key in needed_keys(&manifest) {
                if !own_keys.contains(&key)
                    && !shared_keys.contains(&key)
                    && !engine_keys.contains(&key)
                {
                    report.error(
                        &dir,
                        root,
                        format!("string {key:?} is not defined in any {FALLBACK_LANGUAGE}.ftl"),
                    );
                }
            }

            for widget in &manifest.widgets {
                if let LessonWidget::PhraseLooper { steps, .. } = widget {
                    for step in steps {
                        for token in step.split_whitespace().flat_map(|part| part.split('/')) {
                            if !valid_tab_token(token) {
                                report.error(&dir, root, format!("tab {token:?} in {step:?} is not notation the game can draw"));
                            }
                        }
                    }
                }
            }

            if manifest.track.is_none() {
                report.warning(
                    &dir,
                    root,
                    "no `track`: the skill tree will give it a row named after its unit",
                );
            }
            lessons.push((dir, manifest));
        }
    }

    if lessons.is_empty() {
        report.error(
            root,
            root,
            "no lessons found (expected <unit>/<lesson>/lesson.json)",
        );
        return report;
    }

    let mut seen = HashSet::new();
    for (dir, m) in &lessons {
        if !seen.insert(m.id.as_str()) {
            report.error(
                dir,
                root,
                format!("id {:?} is already used by another lesson", m.id),
            );
        }
    }
    for (dir, m) in &lessons {
        for prerequisite in &m.prerequisites {
            if !seen.contains(prerequisite.as_str()) {
                report.error(
                    dir,
                    root,
                    format!("prerequisite {prerequisite:?} names no lesson"),
                );
            }
        }
    }

    let manifests: Vec<&LessonManifest> = lessons.iter().map(|(_, m)| m).collect();
    if let Err(e) = LessonGraph::build(&manifests) {
        report.error(
            root,
            root,
            format!("the curriculum is not a drawable graph: {e}"),
        );
    }
    let owned: Vec<LessonManifest> = manifests.iter().map(|m| (*m).clone()).collect();
    for (lesson, prerequisite) in crossing_prerequisites(&owned) {
        report.error(
            root,
            root,
            format!("{lesson} needs {prerequisite}, from a later unit, which unit gating can never satisfy"),
        );
    }
    for (lesson, elective) in core_prerequisites_on_optional(&owned) {
        report.error(
            root,
            root,
            format!("required lesson {lesson} depends on elective {elective}, making it mandatory"),
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/lesson-pack")
    }

    /// A copy of the fixture pack to break one thing in.
    fn copy_of_fixture() -> tempfile::TempDir {
        fn copy(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap().flatten() {
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    std::fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        copy(&fixture(), dir.path());
        dir
    }

    fn errors_after(edit: impl FnOnce(&Path)) -> Vec<String> {
        let pack = copy_of_fixture();
        edit(pack.path());
        validate_lesson_pack(pack.path()).errors
    }

    fn rewrite(path: &Path, from: &str, to: &str) {
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains(from), "{from:?} not in {}", path.display());
        std::fs::write(path, text.replace(from, to)).unwrap();
    }

    #[test]
    fn the_fixture_pack_is_valid() {
        let report = validate_lesson_pack(&fixture());
        assert!(report.errors.is_empty(), "{:#?}", report.errors);
        assert!(report.warnings.is_empty(), "{:#?}", report.warnings);
    }

    #[test]
    fn a_missing_prerequisite_is_reported() {
        let errors = errors_after(|p| {
            rewrite(
                &p.join("01_basics/02_second/lesson.json"),
                "\"fixture-first\"]",
                "\"nope\"]",
            )
        });
        assert!(
            errors
                .iter()
                .any(|e| e.contains("\"nope\" names no lesson")),
            "{errors:#?}"
        );
    }

    #[test]
    fn a_cycle_is_reported() {
        let errors = errors_after(|p| {
            rewrite(
                &p.join("01_basics/01_first/lesson.json"),
                "\"chart\"",
                "\"prerequisites\": [\"fixture-second\"],\n  \"chart\"",
            )
        });
        assert!(
            errors.iter().any(|e| e.contains("not a drawable graph")),
            "{errors:#?}"
        );
    }

    #[test]
    fn an_undefined_string_is_reported() {
        let errors = errors_after(|p| {
            rewrite(
                &p.join("01_basics/02_second/locales/en-US.ftl"),
                "fixture-second-title",
                "renamed",
            )
        });
        assert!(
            errors
                .iter()
                .any(|e| e.contains("\"fixture-second-title\" is not defined")),
            "{errors:#?}"
        );
    }

    #[test]
    fn a_translation_missing_a_key_is_reported() {
        let errors = errors_after(|p| {
            rewrite(
                &p.join("01_basics/02_second/locales/pt-BR.ftl"),
                "fixture-second-pattern = Padrão\n",
                "",
            )
        });
        assert!(
            errors.iter().any(|e| e.contains("pt-BR.ftl: missing")),
            "{errors:#?}"
        );
    }

    #[test]
    fn broken_fluent_syntax_is_reported() {
        let errors = errors_after(|p| {
            std::fs::write(p.join("locales/pt-BR.ftl"), "lesson-unit-basics = {\n").unwrap()
        });
        assert!(
            errors.iter().any(|e| e.contains("Fluent syntax")),
            "{errors:#?}"
        );
    }

    #[test]
    fn a_missing_or_broken_chart_is_reported() {
        let errors = errors_after(|p| {
            std::fs::write(p.join("01_basics/01_first/song/chart.harpchart"), "{}").unwrap()
        });
        assert!(
            errors.iter().any(|e| e.contains("chart.harpchart")),
            "{errors:#?}"
        );
        let errors = errors_after(|p| {
            std::fs::remove_file(p.join("01_basics/01_first/song/chart.harpchart")).unwrap()
        });
        assert!(
            errors.iter().any(|e| e.contains("does not exist")),
            "{errors:#?}"
        );
    }

    #[test]
    fn unsupported_tab_notation_is_reported() {
        let errors = errors_after(|p| {
            rewrite(
                &p.join("01_basics/02_second/lesson.json"),
                "\"-4\"",
                "\"-11\"",
            )
        });
        assert!(errors.iter().any(|e| e.contains("\"-11\"")), "{errors:#?}");
    }

    #[test]
    fn tab_tokens() {
        for ok in ["+4", "-3'", "-3'''", "+10", "-6o", "+7*", "A7", "I"] {
            assert!(valid_tab_token(ok), "{ok}");
        }
        for bad in ["+0", "-11", "+4''''", "-4x", "+"] {
            assert!(!valid_tab_token(bad), "{bad}");
        }
    }
}
