// SPDX-License-Identifier: MIT

//! `lesson.json` schema types and parsing: [`LessonManifest`] and its
//! [`PassCriteria`], schema-validated against `assets/lesson_schema.dtd.json`.

use harmonicon_core::training::{DrillSpec, DrillTechnique, Tier};
use serde::Deserialize;

const SCHEMA: &str = include_str!("../../../../assets/lesson_schema.dtd.json");

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
    /// (see `menu::pages::lesson_reader::setup_lesson_reader`), and a dedicated
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

/// A lesson's generated drills, as authored. The tier ladder itself is
/// fixed (`harmonicon_core::training::Tier`), so a lesson only says *what*
/// to drill and *where* — never how hard, which is the ladder's job.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TrainingBlock {
    pub technique: String,
    pub holes: Vec<u8>,
    /// Fixes the generated order so a retry is the same exercise. Derived
    /// from the lesson id when absent, so an author need not invent one.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// One `lesson.json`, as authored. See `assets/lesson_schema.dtd.json` for
/// field semantics.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LessonManifest {
    pub id: String,
    pub unit: String,
    /// An elective branch. Optional lessons can depend on the core, but unit
    /// completion and later-unit gates never require them.
    #[serde(default)]
    pub optional: bool,
    /// Which row of the skill tree this lesson sits in — see
    /// `docs/training_tree_plan.md`. Optional on purpose: a lesson authored
    /// outside this repo and dropped into `~/Harmonicon/lessons` has no
    /// reason to know the track vocabulary, and falling back to `unit`
    /// still groups it somewhere sensible. Read it through
    /// [`LessonManifest::track`], never directly.
    #[serde(default)]
    pub track: Option<String>,
    pub title_key: String,
    pub body_key: String,
    #[serde(default)]
    pub chart: Option<String>,
    /// Hides scrolling note prompts for an ear-training run while retaining
    /// synthesized call cues and ordinary microphone scoring.
    #[serde(default)]
    pub aural: bool,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub pass_criteria: Option<PassCriteria>,
    /// Generated practice drills, or `None` for a lesson that has none.
    ///
    /// Absent is the right answer for anything instructional-only: where the
    /// microphone cannot verify the technique, five exercises would only
    /// pretend to check it. See `docs/lessons_plan.md` on what is honestly
    /// scoreable.
    #[serde(default)]
    pub training: Option<TrainingBlock>,
    /// A jam-based lesson's backing progression (`"standard"`/
    /// `"quick-change"`/`"minor"`), seeded into `crate::app::JamProgression` when
    /// routing into `GameplayMode::JamSession` — see
    /// `menu::pages::lesson_reader::parse_progression`. `None` resets to `Standard`,
    /// the same "don't let a stale pick from an earlier generated jam linger"
    /// reasoning the real-song Jam Session button already applies.
    #[serde(default)]
    pub progression: Option<String>,
    /// A jam-based lesson's scale for live scale-adherence feedback
    /// (`"first-position"`/`"second-position"`/`"third-position"`/
    /// `"major"`/`"minor-pentatonic"`/`"country"`), seeded into
    /// `crate::app::JamScale` when routing into `GameplayMode::JamSession`
    /// — see `menu::pages::lesson_reader::parse_scale`. `None` resets to
    /// `first-position` (the blues hexatonic), the same "don't let a stale
    /// pick linger" reasoning `progression` above applies.
    #[serde(default)]
    pub scale: Option<String>,
    /// An instructional lesson's embedded reference diagram
    /// (`"circle-of-fifths"` — see `dialogs::circle_of_fifths`), rendered
    /// by `menu::pages::lesson_reader::setup_lesson_reader` alongside the body
    /// text. `None` (the common case) renders no diagram. Schema-enforced
    /// to a fixed enum, like `progression`/`scale` above, so a second
    /// diagram type later just adds another accepted value here rather
    /// than a new field.
    #[serde(default)]
    pub diagram: Option<String>,
    /// A jam-based lesson that periodically calls a new position (cycling
    /// `crate::app::JamScale` through First/Second/Third position every few
    /// bars — see `jam::position_guide`), seeded into
    /// `crate::app::JamPositionCycle` when routing into
    /// `GameplayMode::JamSession`. Defaults to `false` — an ordinary jam
    /// lesson's `scale` field stays fixed for the whole session.
    #[serde(default)]
    pub position_cycle: bool,
}

/// The compiled lesson schema, built once for the whole process: the
/// startup scan validates every bundled and external lesson in a row, and
/// compiling the schema again for each of them is the expensive half.
fn lesson_validator() -> &'static jsonschema::Validator {
    static VALIDATOR: std::sync::OnceLock<jsonschema::Validator> = std::sync::OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: serde_json::Value =
            serde_json::from_str(SCHEMA).expect("embedded lesson schema must be valid JSON");
        jsonschema::validator_for(&schema).expect("embedded lesson schema must compile")
    })
}

/// Parses and schema-validates one `lesson.json`'s bytes.
pub fn parse_lesson(bytes: &[u8]) -> Result<LessonManifest, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("JSON parse error: {e}"))?;
    let errors: Vec<String> = lesson_validator()
        .iter_errors(&value)
        .map(|e| format!("{e} (at /{})", e.instance_path()))
        .collect();
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    serde_json::from_value(value).map_err(|e| format!("deserialize error: {e}"))
}

impl LessonManifest {
    /// The skill-tree row this lesson belongs to, falling back to its unit
    /// when it declares no `track` of its own.
    pub fn track(&self) -> &str {
        self.track.as_deref().unwrap_or(&self.unit)
    }

    /// The drill this lesson's trainings are built from, at `tier`, or
    /// `None` if it has no trainings.
    pub fn drill_spec(&self, tier: Tier) -> Option<DrillSpec> {
        let block = self.training.as_ref()?;
        let technique = match block.technique.as_str() {
            "bend" => DrillTechnique::Bend,
            // Schema-enumerated, so this is unreachable from a validated
            // manifest; refusing beats drilling the wrong thing.
            _ => return None,
        };
        Some(DrillSpec {
            technique,
            holes: block.holes.clone(),
            tier,
            seed: block.seed.unwrap_or_else(|| seed_from_id(&self.id)),
        })
    }
}

/// A stable seed for a lesson that declares none — FNV-1a over the id, so
/// the same lesson always generates the same exercises without an author
/// having to pick a number, and two lessons don't collide.
fn seed_from_id(id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
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
        assert!(!m.optional);
        assert!(m.prerequisites.is_empty());
        assert_eq!(m.pass_criteria, None);
    }

    #[test]
    fn parses_an_optional_lesson() {
        let m = parse_lesson(
            br#"{"id":"overblows","unit":"advanced","optional":true,"title_key":"t","body_key":"b"}"#,
        )
        .unwrap();
        assert!(m.optional);
    }

    #[test]
    fn parses_an_aural_lesson_and_defaults_ordinary_lessons_to_visual() {
        let aural = parse_lesson(
            br#"{"id":"echo","unit":"ear","title_key":"t","body_key":"b","aural":true}"#,
        )
        .unwrap();
        assert!(aural.aural);

        let ordinary =
            parse_lesson(br#"{"id":"x","unit":"u","title_key":"t","body_key":"b"}"#).unwrap();
        assert!(!ordinary.aural);
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
    fn parses_a_scale_field() {
        let m = parse_lesson(
            br#"{"id":"major-scale-improv","unit":"scales","title_key":"t","body_key":"b",
                 "scale":"major"}"#,
        )
        .unwrap();
        assert_eq!(m.scale.as_deref(), Some("major"));
    }

    #[test]
    fn scale_defaults_to_none_when_absent() {
        let m = parse_lesson(br#"{"id":"x","unit":"u","title_key":"t","body_key":"b"}"#).unwrap();
        assert_eq!(m.scale, None);
    }

    #[test]
    fn rejects_an_unknown_scale_value() {
        let err = parse_lesson(
            br#"{"id":"x","unit":"u","title_key":"t","body_key":"b","scale":"dorian"}"#,
        )
        .unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn parses_a_diagram_field() {
        let m = parse_lesson(
            br#"{"id":"circle-of-fifths","unit":"scales","title_key":"t","body_key":"b",
                 "diagram":"circle-of-fifths"}"#,
        )
        .unwrap();
        assert_eq!(m.diagram.as_deref(), Some("circle-of-fifths"));
    }

    #[test]
    fn diagram_defaults_to_none_when_absent() {
        let m = parse_lesson(br#"{"id":"x","unit":"u","title_key":"t","body_key":"b"}"#).unwrap();
        assert_eq!(m.diagram, None);
    }

    #[test]
    fn rejects_an_unknown_diagram_value() {
        let err = parse_lesson(
            br#"{"id":"x","unit":"u","title_key":"t","body_key":"b","diagram":"mandala"}"#,
        )
        .unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn parses_a_position_cycle_field() {
        let m = parse_lesson(
            br#"{"id":"circle-of-fifths-jam","unit":"scales","title_key":"t","body_key":"b",
                 "position_cycle":true}"#,
        )
        .unwrap();
        assert!(m.position_cycle);
    }

    #[test]
    fn position_cycle_defaults_to_false_when_absent() {
        let m = parse_lesson(br#"{"id":"x","unit":"u","title_key":"t","body_key":"b"}"#).unwrap();
        assert!(!m.position_cycle);
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
