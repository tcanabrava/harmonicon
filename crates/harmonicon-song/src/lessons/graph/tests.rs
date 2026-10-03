// SPDX-License-Identifier: MIT

use super::*;

/// A manifest with only the fields the graph reads.
fn lesson(id: &str, track: &str, prerequisites: &[&str]) -> LessonManifest {
    LessonManifest {
        id: id.to_string(),
        unit: "test".to_string(),
        optional: false,
        track: Some(track.to_string()),
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

// ── build ────────────────────────────────────────────────────────────────

#[test]
fn depth_is_the_longest_path_not_the_shortest() {
    // `c` can be reached in one step from `a`, but not *played* until `b` is
    // done too. The longest path is what says when it is actually available,
    // which is the whole reason depth exists.
    let g = LessonGraph::build(&[
        lesson("a", "t", &[]),
        lesson("b", "t", &["a"]),
        lesson("c", "t", &["a", "b"]),
    ])
    .unwrap();
    assert_eq!(g.get("a").unwrap().depth, 0);
    assert_eq!(g.get("b").unwrap().depth, 1);
    assert_eq!(g.get("c").unwrap().depth, 2);
    assert_eq!(g.max_depth(), 2);
}

#[test]
fn a_prerequisite_always_precedes_its_dependents() {
    let g = LessonGraph::build(&[
        lesson("c", "t", &["b"]),
        lesson("a", "t", &[]),
        lesson("b", "t", &["a"]),
    ])
    .unwrap();
    let order: Vec<&str> = g.nodes().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(order, vec!["a", "b", "c"]);
}

#[test]
fn a_cycle_is_refused_rather_than_half_built() {
    // Every lesson in a cycle is unreachable forever, and a renderer following the edges would not
    // terminate — so this must fail loudly at build, not produce a graph
    // that looks fine until someone walks it.
    let err = LessonGraph::build(&[
        lesson("a", "t", &["c"]),
        lesson("b", "t", &["a"]),
        lesson("c", "t", &["b"]),
    ])
    .unwrap_err();
    match err {
        GraphError::Cycle(ids) => assert_eq!(ids, vec!["a", "b", "c"]),
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[test]
fn a_lesson_downstream_of_a_cycle_is_reported_with_it() {
    // It is equally stuck, and saying so avoids a second failure the moment
    // the first is fixed.
    let err = LessonGraph::build(&[
        lesson("a", "t", &["b"]),
        lesson("b", "t", &["a"]),
        lesson("later", "t", &["a"]),
    ])
    .unwrap_err();
    assert!(matches!(err, GraphError::Cycle(ref ids) if ids.contains(&"later".to_string())));
}

#[test]
fn a_prerequisite_that_does_not_exist_is_its_own_error() {
    // Distinct from a cycle because the fix is different: a typo, or a
    // lesson deleted out from under its dependents.
    let err = LessonGraph::build(&[lesson("a", "t", &["ghost"])]).unwrap_err();
    assert_eq!(
        err,
        GraphError::UnknownPrerequisite { lesson: "a".into(), missing: "ghost".into() }
    );
}

#[test]
fn an_empty_curriculum_builds() {
    let g = LessonGraph::build::<LessonManifest>(&[]).unwrap();
    assert!(g.nodes().is_empty());
    assert_eq!(g.max_depth(), 0);
}

// ── available ────────────────────────────────────────────────────────────

#[test]
fn only_roots_are_available_before_anything_is_passed() {
    let g = LessonGraph::build(&[
        lesson("a", "t", &[]),
        lesson("b", "t", &[]),
        lesson("c", "t", &["a"]),
    ])
    .unwrap();
    assert_eq!(g.available(&passed(&[])), vec!["a", "b"]);
}

#[test]
fn a_lesson_needs_every_prerequisite_not_just_one() {
    let g = LessonGraph::build(&[
        lesson("a", "t", &[]),
        lesson("b", "t", &[]),
        lesson("c", "t", &["a", "b"]),
    ])
    .unwrap();
    assert!(!g.available(&passed(&["a"])).contains(&"c"));
    assert!(g.available(&passed(&["a", "b"])).contains(&"c"));
}

#[test]
fn something_already_passed_is_not_offered_again() {
    let g = LessonGraph::build(&[lesson("a", "t", &[]), lesson("b", "t", &["a"])]).unwrap();
    assert_eq!(g.available(&passed(&["a"])), vec!["b"]);
}

// ── rows ─────────────────────────────────────────────────────────────────

#[test]
fn a_row_holds_its_track_ordered_by_depth() {
    let g =
        LessonGraph::build(&[lesson("late", "x", &["early"]), lesson("early", "x", &[])]).unwrap();
    let rows = g.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].lessons, vec!["early", "late"]);
}

#[test]
fn tracks_that_start_earlier_are_drawn_first() {
    // Foundations at the top; a track a player cannot touch yet below.
    let g = LessonGraph::build(&[
        lesson("root", "foundation", &[]),
        lesson("advanced", "later", &["root"]),
    ])
    .unwrap();
    let rows = g.rows();
    let order: Vec<&str> = rows.iter().map(|r| r.track.as_str()).collect();
    assert_eq!(order, vec!["foundation", "later"]);
}

#[test]
fn equally_deep_tracks_are_ordered_longest_first_then_by_name() {
    let g = LessonGraph::build(&[
        lesson("a1", "alpha", &[]),
        lesson("b1", "beta", &[]),
        lesson("b2", "beta", &[]),
        lesson("c1", "gamma", &[]),
    ])
    .unwrap();
    let rows = g.rows();
    let order: Vec<&str> = rows.iter().map(|r| r.track.as_str()).collect();
    assert_eq!(order, vec!["beta", "alpha", "gamma"]);
}

#[test]
fn a_lesson_with_no_track_falls_back_to_its_unit() {
    // An externally authored lesson has no reason to know this repo's track
    // vocabulary; it still has to land in some row.
    let mut m = lesson("a", "ignored", &[]);
    m.track = None;
    let g = LessonGraph::build(&[m]).unwrap();
    assert_eq!(g.get("a").unwrap().track, "test");
}

// ── choice_report ────────────────────────────────────────────────────────

fn report(lessons: &[LessonManifest], trials: u32, while_remaining: usize) -> ChoiceReport {
    let graph = LessonGraph::build(lessons).unwrap();
    let chain = UnitChain::build(lessons);
    choice_report(&graph, &chain, trials, while_remaining, 12345)
}

fn in_unit(mut lesson: LessonManifest, unit: &str) -> LessonManifest {
    lesson.unit = unit.to_string();
    lesson
}

#[test]
fn a_single_gateway_is_named_as_the_chokepoint() {
    // After `a`, the only thing to do is `gate`; everything else waits.
    let r = report(
        &[
            lesson("a", "t", &[]),
            lesson("gate", "t", &["a"]),
            lesson("x", "t", &["gate"]),
            lesson("y", "t", &["gate"]),
            lesson("z", "t", &["gate"]),
        ],
        50,
        3,
    );
    assert_eq!(r.fewest, 1);
    // `a` is the only start as well; each is the sole option once a trial.
    assert_eq!(r.sole_options, vec![("a".to_string(), 50), ("gate".to_string(), 50)]);
}

#[test]
fn a_wide_curriculum_has_no_chokepoint() {
    let r = report(
        &[
            lesson("a", "t", &[]),
            lesson("b", "t", &[]),
            lesson("c", "t", &[]),
            lesson("d", "t", &[]),
        ],
        50,
        2,
    );
    assert!(r.fewest >= 2);
    assert!(r.sole_options.is_empty());
}

#[test]
fn a_closed_unit_narrows_the_choice_even_with_prerequisites_met() {
    // No prerequisites at all, so the graph alone offers everything. But
    // unit `second` only opens once `first` is mostly done, so the second
    // unit's lessons are not really on offer at the start.
    let r = report(
        &[
            in_unit(lesson("a", "t", &[]), "first"),
            in_unit(lesson("x", "t", &[]), "second"),
            in_unit(lesson("y", "t", &[]), "second"),
            in_unit(lesson("z", "t", &[]), "second"),
        ],
        20,
        4,
    );
    assert_eq!(r.fewest, 1, "only `a` is open while unit two is shut");
    assert_eq!(r.sole_options, vec![("a".to_string(), 20)]);
}

#[test]
fn the_report_is_deterministic_for_a_seed() {
    let lessons = [
        lesson("a", "t", &[]),
        lesson("b", "t", &["a"]),
        lesson("c", "t", &["a"]),
        lesson("d", "t", &["b", "c"]),
    ];
    assert_eq!(report(&lessons, 30, 2), report(&lessons, 30, 2));
}
