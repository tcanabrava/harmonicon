// SPDX-License-Identifier: MIT

use super::*;

/// A manifest with only the fields this module reads.
fn lesson(id: &str, unit: &str, prerequisites: &[&str]) -> LessonManifest {
    LessonManifest {
        id: id.to_string(),
        unit: unit.to_string(),
        optional: false,
        track: None,
        training: None,
        title_key: format!("lesson-{id}-title"),
        body_key: format!("lesson-{id}-body"),
        chart: None,
        aural: false,
        prerequisites: prerequisites.iter().map(|s| s.to_string()).collect(),
        pass_criteria: None,
        progression: None,
        scale: None,
        diagram: None,
        widgets: Vec::new(),
        position_cycle: false,
    }
}

fn passed<'a>(ids: &[&'a str]) -> HashSet<&'a str> {
    ids.iter().copied().collect()
}

/// Two units of five, which makes the 70% threshold a clean four.
fn two_units() -> Vec<LessonManifest> {
    let mut m = Vec::new();
    for i in 1..=5 {
        m.push(lesson(&format!("a{i}"), "alpha", &[]));
    }
    for i in 1..=5 {
        m.push(lesson(&format!("b{i}"), "beta", &[]));
    }
    m
}

fn optional(mut lesson: LessonManifest) -> LessonManifest {
    lesson.optional = true;
    lesson
}

// ── build ────────────────────────────────────────────────────────────────

#[test]
fn units_keep_the_order_they_were_discovered_in() {
    // Discovery order is the `01_`/`02_` directory order, which is the
    // curriculum order — so it is also the order the chain gates in.
    let chain = UnitChain::build(&[
        lesson("a1", "alpha", &[]),
        lesson("b1", "beta", &[]),
        lesson("a2", "alpha", &[]),
    ]);
    let ids: Vec<&str> = chain.units().iter().map(|u| u.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "beta"]);
    assert_eq!(chain.units()[0].lessons, ["a1", "a2"]);
}

#[test]
fn electives_do_not_raise_a_units_gate() {
    let lessons = [
        lesson("core", "alpha", &[]),
        optional(lesson("elective-a", "alpha", &["core"])),
        optional(lesson("elective-b", "alpha", &["core"])),
    ];
    let chain = UnitChain::build(&lessons);
    assert_eq!(chain.required(0), 1);
    assert_eq!(
        chain.completed(0, &passed(&["elective-a", "elective-b"])),
        0
    );
    assert_eq!(chain.completed(0, &passed(&["core"])), 1);
}

#[test]
fn an_elective_only_unit_never_blocks_the_required_course() {
    let lessons = [
        optional(lesson("advanced", "electives", &[])),
        lesson("next-core", "next", &[]),
    ];
    let chain = UnitChain::build(&lessons);
    assert_eq!(chain.required(0), 0);
    assert!(chain.is_unlocked(1, &HashSet::new()));
}

#[test]
fn a_unit_is_elective_only_when_nothing_in_it_counts_toward_a_gate() {
    // The drawing needs this as its own question. `required(..) == 0` is
    // also what an out-of-range index answers, and "there is no such unit"
    // and "this unit gates nothing" want opposite treatment on screen.
    let lessons = [
        optional(lesson("elective-a", "electives", &[])),
        optional(lesson("elective-b", "electives", &[])),
        lesson("core", "mixed", &[]),
        optional(lesson("elective-c", "mixed", &[])),
    ];
    let chain = UnitChain::build(&lessons);
    assert!(chain.is_elective_only(0));
    assert!(!chain.is_elective_only(1), "a mixed unit still has a gate");
    assert!(
        !chain.is_elective_only(9),
        "no such unit is not elective-only"
    );

    assert_eq!(chain.total(0), 2);
    assert_eq!(chain.total(1), 2, "total counts electives alongside core");
    assert_eq!(chain.total(9), 0);
}

#[test]
fn a_core_lesson_cannot_hide_an_elective_in_its_prerequisites() {
    let lessons = [
        optional(lesson("overblow", "advanced", &[])),
        lesson("graduation", "advanced", &["overblow"]),
    ];
    assert_eq!(
        core_prerequisites_on_optional(&lessons),
        [("graduation".to_string(), "overblow".to_string())]
    );
}

#[test]
fn a_units_title_is_the_key_the_curriculum_already_uses() {
    // `lesson-unit-<id>` is what the lesson list's unit tabs read, and all
    // three locales already define one per shipped unit — deriving it is
    // what lets units become nodes with no new authoring.
    let chain = UnitChain::build(&[lesson("a1", "blowing", &[])]);
    assert_eq!(chain.units()[0].title_key, "lesson-unit-blowing");
}

// ── the threshold ────────────────────────────────────────────────────────

#[test]
fn a_unit_opens_the_next_before_every_lesson_in_it_is_done() {
    // The whole point of a threshold: one lesson a player has stalled on
    // must not be a wall in front of the rest of the course.
    let chain = UnitChain::build(&two_units());
    assert_eq!(chain.required(0), 4, "70% of five, rounded up");
    assert!(chain.is_satisfied(0, &passed(&["a1", "a2", "a3", "a4"])));
    assert!(chain.is_unlocked(1, &passed(&["a1", "a2", "a3", "a4"])));
}

#[test]
fn a_unit_short_of_its_threshold_keeps_the_next_one_shut() {
    let chain = UnitChain::build(&two_units());
    let three = passed(&["a1", "a2", "a3"]);
    assert_eq!(chain.completed(0, &three), 3);
    assert!(!chain.is_satisfied(0, &three));
    assert!(!chain.is_unlocked(1, &three));
}

#[test]
fn the_first_unit_is_always_open() {
    let chain = UnitChain::build(&two_units());
    assert!(chain.is_unlocked(0, &passed(&[])));
}

#[test]
fn a_later_unit_needs_every_unit_before_it_not_just_the_last() {
    // Otherwise finishing unit 2 out of order would open unit 3 while unit
    // 1 sat untouched, and the drawing would be claiming something false.
    let mut m = two_units();
    for i in 1..=5 {
        m.push(lesson(&format!("c{i}"), "gamma", &[]));
    }
    let chain = UnitChain::build(&m);
    let beta_only = passed(&["b1", "b2", "b3", "b4"]);
    assert!(chain.is_satisfied(1, &beta_only));
    assert!(!chain.is_unlocked(2, &beta_only));
}

#[test]
fn a_single_lesson_unit_needs_that_lesson() {
    // 70% of one rounds up to one; a unit must never open the next for free.
    let chain = UnitChain::build(&[lesson("only", "alpha", &[]), lesson("b1", "beta", &[])]);
    assert_eq!(chain.required(0), 1);
    assert!(!chain.is_unlocked(1, &passed(&[])));
    assert!(chain.is_unlocked(1, &passed(&["only"])));
}

#[test]
fn the_threshold_never_asks_for_more_lessons_than_a_unit_has() {
    let chain = UnitChain::build(&two_units());
    for ix in 0..chain.units().len() {
        assert!(chain.required(ix) <= chain.units()[ix].lessons.len());
    }
}

// ── lookup ───────────────────────────────────────────────────────────────

#[test]
fn a_lesson_reports_the_unit_holding_it() {
    let chain = UnitChain::build(&two_units());
    assert_eq!(chain.unit_of("b3"), Some(1));
    assert_eq!(chain.unit_of("nope"), None);
}

// ── crossing_prerequisites ───────────────────────────────────────────────

#[test]
fn a_prerequisite_pointing_at_a_later_unit_is_reported() {
    // This is the one shape that makes unit gating unsafe: `a1` can never
    // be passed, because what it needs lives behind a gate `a1` itself has
    // to open.
    let backwards =
        crossing_prerequisites(&[lesson("a1", "alpha", &["b1"]), lesson("b1", "beta", &[])]);
    assert_eq!(backwards, [("a1".to_string(), "b1".to_string())]);
}

#[test]
fn forward_and_same_unit_prerequisites_are_not_reported() {
    let backwards = crossing_prerequisites(&[
        lesson("a1", "alpha", &[]),
        lesson("a2", "alpha", &["a1"]),
        lesson("b1", "beta", &["a1"]),
    ]);
    assert!(backwards.is_empty(), "unexpected: {backwards:?}");
}

#[test]
fn an_unknown_prerequisite_is_left_for_the_graph_to_complain_about() {
    // Two checks reporting the same typo twice would be noise; `graph`
    // already refuses to build over it, with a message naming both ends.
    let backwards = crossing_prerequisites(&[lesson("a1", "alpha", &["ghost"])]);
    assert!(backwards.is_empty());
}
