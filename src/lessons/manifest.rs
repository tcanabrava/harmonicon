// SPDX-License-Identifier: MIT

//! `lesson.json` schema types and parsing: [`LessonManifest`] and its
//! [`PassCriteria`], schema-validated against `assets/lesson_schema.dtd.json`.

use serde::Deserialize;

const SCHEMA: &str = include_str!("../../assets/lesson_schema.dtd.json");

/// How a lesson is judged. `Accuracy`/`Technique` are judged when a
/// chart-backed run reaches the results screen; `None` on [`LessonManifest`]
/// means finishing at all counts. `ScaleAdherence` is judged differently —
/// see its own doc comment — because it backs the one lesson type that
/// never reaches a results screen at all.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PassCriteria {
    /// Minimum overall weighted accuracy (`results::accuracy`), 0..1.
    Accuracy { threshold: f32 },
    /// Minimum accuracy on one technique bucket — the same name vocabulary
    /// `SongStats`/`PlayerProfile::technique_best_accuracy` use
    /// (`"bend"`, `"wah-wah"`, ...).
    Technique { technique: String, threshold: f32 },
    /// Minimum fraction of notes played that were at least in-scale
    /// (`jam::improv::ImprovStats::adherence`), 0..1 — the improvisation
    /// lesson's criterion. Unlike the other two variants, this is judged
    /// from an *open* Jam Session, which has no chart notes to score and no
    /// natural end: the lesson reader's Start button routes a lesson with
    /// this criterion into `GameplayMode::JamSession` instead of `Play2D`
    /// (see `menu::pages::lessons::setup_lesson_reader`), and a dedicated
    /// "Finish Lesson" pause-menu button (jam mode + a `LessonContext` in
    /// flight — see `gameplay::pause_menu`) judges it on demand and returns
    /// to the menu directly, bypassing the results screen entirely (there's
    /// no score/grade that would mean anything for an open jam).
    ScaleAdherence { threshold: f32 },
    /// Minimum fraction of jam attacks that were specifically chord tones
    /// (`jam::improv::ImprovStats::chord_tone_adherence`), 0..1 — stricter
    /// than `ScaleAdherence` (which also accepts merely-in-scale notes).
    /// Same jam-session routing and "Finish Lesson" judging as
    /// `ScaleAdherence`.
    ChordToneAdherence { threshold: f32 },
    /// Minimum fraction of jam attacks that landed *outside* a rest window
    /// of a repeating play/rest bar pattern
    /// (`jam::improv::ImprovStats::phrase_discipline`), 0..1 — judges "did
    /// you leave space", not what was played. Same jam-session routing and
    /// "Finish Lesson" judging as `ScaleAdherence`.
    PhraseDiscipline { threshold: f32 },
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
    /// A jam-based lesson's backing progression (`"standard"`/
    /// `"quick-change"`/`"minor"`), seeded into `crate::app::JamProgression` when
    /// routing into `GameplayMode::JamSession` — see
    /// `menu::pages::lessons::parse_progression`. `None` resets to `Standard`,
    /// the same "don't let a stale pick from an earlier generated jam linger"
    /// reasoning the real-song Jam Session button already applies.
    #[serde(default)]
    pub progression: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_instructional_lesson() {
        let m =
            parse_lesson(br#"{"id":"twelve-bar","unit":"rhythm","title_key":"t","body_key":"b"}"#)
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
    fn parses_a_scale_adherence_criterion() {
        // The improvisation lesson's pass criterion — no "technique" field,
        // unlike Technique.
        let m = parse_lesson(
            br#"{"id":"improv","unit":"rhythm","title_key":"t","body_key":"b",
                 "pass_criteria":{"type":"scale-adherence","threshold":0.8}}"#,
        )
        .unwrap();
        assert_eq!(
            m.pass_criteria,
            Some(PassCriteria::ScaleAdherence { threshold: 0.8 })
        );
    }

    #[test]
    fn parses_a_chord_tone_adherence_criterion() {
        let m = parse_lesson(
            br#"{"id":"chord-tone-improv","unit":"blues","title_key":"t","body_key":"b",
                 "pass_criteria":{"type":"chord-tone-adherence","threshold":0.4}}"#,
        )
        .unwrap();
        assert_eq!(
            m.pass_criteria,
            Some(PassCriteria::ChordToneAdherence { threshold: 0.4 })
        );
    }

    #[test]
    fn parses_a_phrase_discipline_criterion() {
        let m = parse_lesson(
            br#"{"id":"question-answer","unit":"blues","title_key":"t","body_key":"b",
                 "pass_criteria":{"type":"phrase-discipline","threshold":0.7}}"#,
        )
        .unwrap();
        assert_eq!(
            m.pass_criteria,
            Some(PassCriteria::PhraseDiscipline { threshold: 0.7 })
        );
    }

    #[test]
    fn parses_a_progression_field() {
        let m = parse_lesson(
            br#"{"id":"minor-blues-improv","unit":"blues","title_key":"t","body_key":"b",
                 "progression":"minor"}"#,
        )
        .unwrap();
        assert_eq!(m.progression.as_deref(), Some("minor"));
    }

    #[test]
    fn progression_defaults_to_none_when_absent() {
        let m = parse_lesson(br#"{"id":"x","unit":"u","title_key":"t","body_key":"b"}"#).unwrap();
        assert_eq!(m.progression, None);
    }

    #[test]
    fn rejects_an_unknown_progression_value() {
        let err = parse_lesson(
            br#"{"id":"x","unit":"u","title_key":"t","body_key":"b","progression":"jazz"}"#,
        )
        .unwrap_err();
        assert!(!err.is_empty());
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
        let err =
            parse_lesson(br#"{"id":"x","unit":"u","title_key":"t","body_key":"b","chrat":"oops"}"#)
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
}
