// SPDX-License-Identifier: MIT

use super::*;

#[test]
fn lesson_grid_steps_and_wraps_in_both_directions() {
    assert_eq!(stepped_bar(0, -1), 11);
    assert_eq!(stepped_bar(11, 1), 0);
    assert_eq!(stepped_bar(5, 1), 6);
}

#[test]
fn form_sections_step_and_wrap_for_any_form_length() {
    assert_eq!(stepped_section(0, -1, 4), 3);
    assert_eq!(stepped_section(3, 1, 4), 0);
    assert_eq!(stepped_section(1, 1, 3), 2);
    assert_eq!(stepped_section(0, 1, 0), 0);
}

#[test]
fn triplet_pattern_clicks_three_equal_subdivisions() {
    let pattern = LessonMetronomePattern::Triplet;
    assert_eq!(pattern.click(0, 4.0), Some((true, 1.0)));
    assert_eq!(pattern.click(1, 4.0), Some((false, 0.55)));
    assert_eq!(pattern.click(2, 4.0), Some((false, 0.55)));
    assert_eq!(pattern.click(3, 4.0), Some((false, 1.0)));
}

#[test]
fn lesson_metronome_pattern_cycles_through_every_mode() {
    assert!(matches!(LessonMetronomePattern::Straight.next(), LessonMetronomePattern::Shuffle));
    assert!(matches!(LessonMetronomePattern::Shuffle.next(), LessonMetronomePattern::Triplet));
    assert!(matches!(LessonMetronomePattern::Triplet.next(), LessonMetronomePattern::Straight));
}

#[test]
fn tempo_schedule_advances_on_configured_bar_boundaries() {
    let steps = [75.0, 80.0];
    assert_eq!(scheduled_tempo(3, 4, 1, LessonMetronomePattern::Straight, 0, &steps), None);
    assert_eq!(scheduled_tempo(4, 4, 1, LessonMetronomePattern::Straight, 0, &steps), Some(75.0));
    assert_eq!(scheduled_tempo(12, 4, 1, LessonMetronomePattern::Triplet, 1, &steps), Some(80.0));
    assert_eq!(scheduled_tempo(4, 4, 1, LessonMetronomePattern::Straight, 2, &steps), None);
}

#[test]
fn lesson_audio_cleanup_despawns_every_active_click() {
    let mut app = App::new();
    app.add_systems(Update, cleanup_lesson_audio);
    let first = app.world_mut().spawn(LessonMetronomeAudio).id();
    let second = app.world_mut().spawn(LessonMetronomeAudio).id();
    app.update();
    assert!(app.world().get_entity(first).is_err());
    assert!(app.world().get_entity(second).is_err());
}

// ── is_jam_criteria ───────────────────────────────────────────────────────

#[test]
fn every_jam_based_criterion_routes_into_jam_session() {
    for c in [
        PassCriteria::ScaleAdherence { threshold: 0.1 },
        PassCriteria::ChordToneAdherence { threshold: 0.1 },
        PassCriteria::PhraseDiscipline { threshold: 0.1 },
    ] {
        assert!(is_jam_criteria(Some(&c)));
    }
}

#[test]
fn chart_based_criteria_and_none_stay_on_the_ordinary_pipeline() {
    assert!(!is_jam_criteria(None));
    assert!(!is_jam_criteria(Some(&PassCriteria::Accuracy { threshold: 0.5 })));
    assert!(!is_jam_criteria(Some(&PassCriteria::Technique {
        technique: "bend".into(),
        threshold: 0.5
    })));
}

// ── parse_progression ─────────────────────────────────────────────────────

#[test]
fn parse_progression_reads_each_known_value() {
    assert_eq!(parse_progression(Some("standard")), Progression::Standard);
    assert_eq!(parse_progression(Some("quick-change")), Progression::QuickChange);
    assert_eq!(parse_progression(Some("minor")), Progression::Minor);
    assert_eq!(parse_progression(Some("jazz-blues")), Progression::JazzBlues);
}

#[test]
fn parse_progression_defaults_to_standard_when_absent_or_unknown() {
    assert_eq!(parse_progression(None), Progression::Standard);
    assert_eq!(parse_progression(Some("jazz")), Progression::Standard);
}

// ── parse_scale ────────────────────────────────────────────────────────────

#[test]
fn parse_scale_reads_each_known_value() {
    assert_eq!(parse_scale(Some("first-position")), Scale::FirstPosition);
    assert_eq!(parse_scale(Some("second-position")), Scale::SecondPosition);
    assert_eq!(parse_scale(Some("third-position")), Scale::ThirdPosition);
    assert_eq!(parse_scale(Some("major")), Scale::Major);
    assert_eq!(parse_scale(Some("minor-pentatonic")), Scale::MinorPentatonic);
    assert_eq!(parse_scale(Some("country")), Scale::Country);
}

#[test]
fn parse_scale_defaults_to_first_position_when_absent_or_unknown() {
    assert_eq!(parse_scale(None), Scale::FirstPosition);
    assert_eq!(parse_scale(Some("dorian")), Scale::FirstPosition);
}
