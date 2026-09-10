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
        prerequisites: prerequisites.iter().map(|s| s.to_string()).collect(),
        pass_criteria: None,
        progression: None,
        scale: None,
        diagram: None,
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
    // Unchecked before this module existed. Every lesson in a cycle is
    // unreachable forever, and a renderer following the edges would not
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
        GraphError::UnknownPrerequisite {
            lesson: "a".into(),
            missing: "ghost".into()
        }
    );
}

#[test]
fn an_empty_curriculum_builds() {
    let g = LessonGraph::build(&[]).unwrap();
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

// ── min_choices ──────────────────────────────────────────────────────────

#[test]
fn min_choices_finds_a_funnel() {
    // A curriculum with a single gateway: after `a`, the only thing to do is
    // `gate`, and everything else waits behind it.
    let g = LessonGraph::build(&[
        lesson("a", "t", &[]),
        lesson("gate", "t", &["a"]),
        lesson("x", "t", &["gate"]),
        lesson("y", "t", &["gate"]),
        lesson("z", "t", &["gate"]),
    ])
    .unwrap();
    assert_eq!(min_choices(&g, 50, 3, 12345), 1);
}

#[test]
fn min_choices_sees_a_wide_curriculum_as_wide() {
    let g = LessonGraph::build(&[
        lesson("a", "t", &[]),
        lesson("b", "t", &[]),
        lesson("c", "t", &[]),
        lesson("d", "t", &[]),
    ])
    .unwrap();
    assert!(min_choices(&g, 50, 2, 999) >= 2);
}

#[test]
fn min_choices_is_deterministic_for_a_seed() {
    let g = LessonGraph::build(&[
        lesson("a", "t", &[]),
        lesson("b", "t", &["a"]),
        lesson("c", "t", &["a"]),
        lesson("d", "t", &["b", "c"]),
    ])
    .unwrap();
    let once = min_choices(&g, 30, 2, 4242);
    assert_eq!(once, min_choices(&g, 30, 2, 4242));
}
