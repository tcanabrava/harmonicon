// SPDX-License-Identifier: MIT

//! Prerequisite gating and pass judging: [`is_unlocked`], [`lesson_passed`],
//! and the [`LessonContext`] resource that marks a lesson run in flight.

use bevy::prelude::*;

use super::manifest::{LessonManifest, PassCriteria};

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
/// `profile::record_play`; `jam_fraction` is whichever single 0..1 fraction
/// a jam-based criterion (`ScaleAdherence`/`ChordToneAdherence`/
/// `PhraseDiscipline`) reads — the caller picks the right one off
/// `jam::improv::ImprovStats` for whichever criterion is actually active
/// (see `gameplay::pause_menu::jam_fraction_for`); `None` for any
/// chart-backed run (which never accumulates it) and always `Some` by the
/// time a jam-based lesson's "Finish Lesson" button calls this. A
/// criteria-less lesson passes by finishing; a `Technique`/jam criterion the
/// run never exercised fails (an empty run can't demonstrate anything).
pub fn lesson_passed(
    criteria: Option<&PassCriteria>,
    accuracy: f32,
    technique_accuracy: &[(&str, f32)],
    jam_fraction: Option<f32>,
) -> bool {
    match criteria {
        None => true,
        Some(PassCriteria::Accuracy { threshold }) => accuracy >= *threshold,
        Some(PassCriteria::ScaleAdherence { threshold })
        | Some(PassCriteria::ChordToneAdherence { threshold })
        | Some(PassCriteria::PhraseDiscipline { threshold }) => {
            jam_fraction.is_some_and(|a| a >= *threshold)
        }
        Some(PassCriteria::Technique {
            technique,
            threshold,
        }) => technique_accuracy
            .iter()
            .find(|(name, _)| name == technique)
            .is_some_and(|(_, acc)| *acc >= *threshold),
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
            progression: None,
        }
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
        assert!(lesson_passed(None, 0.0, &[], None));
    }

    #[test]
    fn accuracy_criterion_compares_against_overall_accuracy() {
        let c = PassCriteria::Accuracy { threshold: 0.6 };
        assert!(!lesson_passed(Some(&c), 0.59, &[], None));
        assert!(lesson_passed(Some(&c), 0.6, &[], None));
    }

    #[test]
    fn technique_criterion_reads_the_matching_bucket() {
        let c = PassCriteria::Technique {
            technique: "wah-wah".into(),
            threshold: 0.5,
        };
        let per_technique = [("normal", 1.0_f32), ("wah-wah", 0.4)];
        assert!(!lesson_passed(Some(&c), 1.0, &per_technique, None));
        let per_technique = [("normal", 0.0_f32), ("wah-wah", 0.5)];
        assert!(lesson_passed(Some(&c), 0.0, &per_technique, None));
    }

    #[test]
    fn technique_criterion_fails_when_the_bucket_was_never_exercised() {
        let c = PassCriteria::Technique {
            technique: "wah-wah".into(),
            threshold: 0.5,
        };
        assert!(!lesson_passed(Some(&c), 1.0, &[("normal", 1.0)], None));
    }

    #[test]
    fn scale_adherence_criterion_compares_against_the_jam_tally() {
        let c = PassCriteria::ScaleAdherence { threshold: 0.8 };
        assert!(!lesson_passed(Some(&c), 0.0, &[], Some(0.79)));
        assert!(lesson_passed(Some(&c), 0.0, &[], Some(0.8)));
    }

    #[test]
    fn scale_adherence_criterion_fails_when_nothing_was_played() {
        let c = PassCriteria::ScaleAdherence { threshold: 0.8 };
        assert!(!lesson_passed(Some(&c), 0.0, &[], None));
    }

    #[test]
    fn chord_tone_adherence_criterion_compares_against_the_jam_fraction() {
        let c = PassCriteria::ChordToneAdherence { threshold: 0.4 };
        assert!(!lesson_passed(Some(&c), 0.0, &[], Some(0.39)));
        assert!(lesson_passed(Some(&c), 0.0, &[], Some(0.4)));
    }

    #[test]
    fn phrase_discipline_criterion_compares_against_the_jam_fraction() {
        let c = PassCriteria::PhraseDiscipline { threshold: 0.7 };
        assert!(!lesson_passed(Some(&c), 0.0, &[], Some(0.69)));
        assert!(lesson_passed(Some(&c), 0.0, &[], Some(0.7)));
    }

    #[test]
    fn jam_criteria_fail_when_nothing_was_played() {
        for c in [
            PassCriteria::ScaleAdherence { threshold: 0.1 },
            PassCriteria::ChordToneAdherence { threshold: 0.1 },
            PassCriteria::PhraseDiscipline { threshold: 0.1 },
        ] {
            assert!(!lesson_passed(Some(&c), 1.0, &[], None));
        }
    }
}
