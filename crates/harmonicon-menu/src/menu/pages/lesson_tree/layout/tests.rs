// SPDX-License-Identifier: MIT

use super::*;
use harmonicon_app::profile::{record_lesson, record_training, training_key};
use harmonicon_song::lessons::{LessonManifest, TrainingBlock};

fn manifest(id: &str, track: &str, prerequisites: &[&str], trainings: bool) -> LessonManifest {
    LessonManifest {
        id: id.to_string(),
        unit: "u".to_string(),
        track: Some(track.to_string()),
        title_key: format!("lesson-{id}-title"),
        body_key: format!("lesson-{id}-body"),
        chart: None,
        prerequisites: prerequisites.iter().map(|s| s.to_string()).collect(),
        pass_criteria: None,
        training: trainings.then(|| TrainingBlock {
            technique: "bend".to_string(),
            holes: vec![2],
            seed: None,
        }),
        progression: None,
        scale: None,
        diagram: None,
        position_cycle: false,
    }
}

fn entry(id: &str, track: &str, prerequisites: &[&str], trainings: bool) -> LessonEntry {
    LessonEntry {
        manifest: manifest(id, track, prerequisites, trainings),
        chart_asset_path: Some(format!("{id}.harpchart")),
    }
}

fn build(entries: &[LessonEntry], profile: &PlayerProfile) -> TreeLayout {
    let manifests: Vec<LessonManifest> = entries.iter().map(|e| e.manifest.clone()).collect();
    let graph = LessonGraph::build(&manifests).expect("a valid graph");
    layout(entries, &graph, profile)
}

fn pass(profile: &mut PlayerProfile, id: &str) {
    let r = profile.lessons.entry(id.to_string()).or_default();
    record_lesson(r, true, 1.0);
}

// ── state ────────────────────────────────────────────────────────────────

#[test]
fn a_lesson_with_unmet_prerequisites_is_locked() {
    let e = [
        entry("root", "t", &[], false),
        entry("later", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("root").unwrap().state, NodeState::Available);
    assert_eq!(l.node("later").unwrap().state, NodeState::Locked);
}

#[test]
fn passing_a_prerequisite_unlocks_what_follows() {
    let e = [
        entry("root", "t", &[], false),
        entry("later", "t", &["root"], false),
    ];
    let mut p = PlayerProfile::default();
    pass(&mut p, "root");
    let l = build(&e, &p);
    assert_eq!(l.node("root").unwrap().state, NodeState::Passed);
    assert_eq!(l.node("later").unwrap().state, NodeState::Available);
}

#[test]
fn a_passed_lesson_is_mastered_only_once_every_tier_is_too() {
    let e = [entry("bend", "t", &[], true)];
    let mut p = PlayerProfile::default();
    pass(&mut p, "bend");
    assert_eq!(build(&e, &p).node("bend").unwrap().state, NodeState::Passed);

    for tier in 1..=Tier::ALL.len() as u8 {
        let r = p.trainings.entry(training_key("bend", tier)).or_default();
        record_training(r, true, 1.0);
    }
    let l = build(&e, &p);
    assert_eq!(l.node("bend").unwrap().state, NodeState::Mastered);
    assert_eq!(l.node("bend").unwrap().mastery, 1.0);
}

#[test]
fn a_lesson_with_no_trainings_never_claims_to_be_mastered() {
    // It would otherwise sit at Passed forever with an empty ring, reading
    // as "you have done none of it" rather than "there is none to do".
    let e = [entry("plain", "t", &[], false)];
    let mut p = PlayerProfile::default();
    pass(&mut p, "plain");
    let node = build(&e, &p);
    let node = node.node("plain").unwrap();
    assert_eq!(node.state, NodeState::Passed);
    assert!(!node.has_trainings);
    assert_eq!(node.mastery, 0.0);
}

#[test]
fn mastery_is_the_fraction_of_tiers_passed() {
    let e = [entry("bend", "t", &[], true)];
    let mut p = PlayerProfile::default();
    for tier in [1, 2] {
        let r = p.trainings.entry(training_key("bend", tier)).or_default();
        record_training(r, true, 1.0);
    }
    assert_eq!(build(&e, &p).node("bend").unwrap().mastery, 0.4);
}

#[test]
fn a_locked_lesson_still_shows_what_it_has_practised() {
    // Trainings are not gated on the lesson, so a player can have earned
    // ring segments on something whose prerequisites they later reset.
    let e = [
        entry("root", "t", &[], false),
        entry("bend", "t", &["root"], true),
    ];
    let mut p = PlayerProfile::default();
    let r = p.trainings.entry(training_key("bend", 1)).or_default();
    record_training(r, true, 1.0);
    let l = build(&e, &p);
    let node = l.node("bend").unwrap();
    assert_eq!(node.state, NodeState::Locked);
    assert!(node.mastery > 0.0);
}

// ── placement ────────────────────────────────────────────────────────────

#[test]
fn a_column_is_the_lessons_depth_not_its_place_in_the_track() {
    // What makes this a tree rather than a grid. Placing by position within
    // the track put every row in lockstep from column 0, so a node's
    // horizontal position said nothing about where it sat in the
    // curriculum.
    let e = [
        entry("late", "x", &["early"], false),
        entry("early", "x", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("early").unwrap().column, 0);
    assert_eq!(l.node("late").unwrap().column, 1);
}

#[test]
fn a_track_that_starts_late_starts_late_on_screen() {
    // The gap is the point: `scales` has nothing before depth 2, and
    // showing that is how the drawing communicates prerequisites at a
    // glance rather than only through its edges.
    let e = [
        entry("a", "early", &[], false),
        entry("b", "early", &["a"], false),
        entry("c", "late", &["b"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("c").unwrap().column, 2);
    assert_eq!(
        l.rows.iter().find(|r| r.track == "late").unwrap().nodes[0].column,
        2,
        "a late track must not be packed back to column 0"
    );
}

#[test]
fn a_node_always_sits_right_of_everything_it_depends_on() {
    let e = [
        entry("a", "t", &[], false),
        entry("b", "u", &["a"], false),
        entry("c", "v", &["a", "b"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    for row in &l.rows {
        for node in &row.nodes {
            for edge in l.edges.iter().filter(|x| x.to == (node.row, node.column)) {
                assert!(
                    edge.from.1 < node.column,
                    "{} is not right of its prerequisite",
                    node.id
                );
            }
        }
    }
}

#[test]
fn same_depth_siblings_in_a_track_stack_into_sub_rows() {
    // `tone` really has three lessons at depth 1; they cannot share a cell.
    let e = [
        entry("root", "t", &[], false),
        entry("x", "t", &["root"], false),
        entry("y", "t", &["root"], false),
        entry("z", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(
        l.rows[0].height, 3,
        "three at one depth need three sub-rows"
    );
    let rows: Vec<usize> = ["x", "y", "z"]
        .iter()
        .map(|id| l.node(id).unwrap().row)
        .collect();
    let unique: std::collections::HashSet<usize> = rows.iter().copied().collect();
    assert_eq!(unique.len(), 3, "siblings must not overlap: {rows:?}");
    assert!(
        ["x", "y", "z"]
            .iter()
            .all(|id| l.node(id).unwrap().column == 1)
    );
}

#[test]
fn a_tracks_rows_are_contiguous_and_do_not_overlap_the_next() {
    let e = [
        entry("root", "a", &[], false),
        entry("x", "a", &["root"], false),
        entry("y", "a", &["root"], false),
        entry("other", "b", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let mut expected = 0;
    for row in &l.rows {
        assert_eq!(row.first_row, expected, "track {} starts wrong", row.track);
        expected += row.height;
    }
    assert_eq!(l.height(), expected);
}

#[test]
fn tracks_are_drawn_in_teaching_order_not_by_size() {
    // The catalogue is ordered `01_blowing/01_single_note`, so its order is
    // the curriculum's. Ordering by track length put a five-lesson `form`
    // above `tone`, when `single-note` is where a player actually starts.
    let e = [
        entry("first-thing", "tone", &[], false),
        entry("a", "form", &[], false),
        entry("b", "form", &["a"], false),
        entry("c", "form", &["b"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let tracks: Vec<&str> = l.rows.iter().map(|r| r.track.as_str()).collect();
    assert_eq!(tracks, vec!["tone", "form"]);
}

#[test]
fn columns_and_height_size_the_canvas() {
    let e = [
        entry("a", "one", &[], false),
        entry("b", "one", &["a"], false),
        entry("c", "two", &["b"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.columns(), 3);
    assert_eq!(l.height(), 2);
}

#[test]
fn a_lesson_in_the_graph_but_not_the_catalogue_is_skipped() {
    // The graph is built from manifests and the catalogue carries the
    // titles; if they ever disagree, the drawing omits what it cannot
    // label rather than placing a blank node.
    let e = [
        entry("a", "t", &[], false),
        entry("b", "t", &["a"], false),
        entry("c", "t", &["b"], false),
    ];
    let manifests: Vec<LessonManifest> = e.iter().map(|x| x.manifest.clone()).collect();
    let graph = LessonGraph::build(&manifests).unwrap();
    let short = [e[0].clone(), e[2].clone()]; // 'b' missing from the catalogue
    let l = layout(&short, &graph, &PlayerProfile::default());
    assert!(l.node("b").is_none());
    // 'c' keeps its own depth regardless of the hole above it.
    assert_eq!(l.node("c").unwrap().column, 2);
}

// ── edges ────────────────────────────────────────────────────────────────

#[test]
fn every_prerequisite_becomes_an_edge_between_placed_nodes() {
    let e = [
        entry("root", "a", &[], false),
        entry("mid", "b", &["root"], false),
        entry("leaf", "b", &["root", "mid"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.edges.len(), 3);
    // Both ends are (absolute row, column) — grid coordinates, not indices
    // into `rows`/`nodes` — so an edge is drawable without knowing which
    // track either end sits in.
    let placed: std::collections::HashSet<(usize, usize)> = l
        .rows
        .iter()
        .flat_map(|r| &r.nodes)
        .map(|n| (n.row, n.column))
        .collect();
    for edge in &l.edges {
        assert!(placed.contains(&edge.from), "dangling edge start {edge:?}");
        assert!(placed.contains(&edge.to), "dangling edge end {edge:?}");
    }
}

#[test]
fn an_edge_within_one_row_is_kept() {
    // A row is a family, not a chain: neighbours are not implicitly
    // connected, so a real prerequisite between two of them still has to be
    // drawn or the reader cannot see it.
    let e = [
        entry("first", "t", &[], false),
        entry("second", "t", &["first"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(
        l.edges,
        vec![Edge {
            from: (0, 0),
            to: (0, 1)
        }]
    );
}

#[test]
fn edges_come_out_in_a_stable_order() {
    let e = [
        entry("root", "a", &[], false),
        entry("x", "b", &["root"], false),
        entry("y", "b", &["root"], false),
    ];
    let once = build(&e, &PlayerProfile::default()).edges;
    let twice = build(&e, &PlayerProfile::default()).edges;
    assert_eq!(once, twice);
}

#[test]
fn an_empty_curriculum_lays_out_to_nothing() {
    let l = build(&[], &PlayerProfile::default());
    assert!(l.rows.is_empty());
    assert!(l.edges.is_empty());
    assert_eq!(l.columns(), 0);
    assert_eq!(l.height(), 0);
}
