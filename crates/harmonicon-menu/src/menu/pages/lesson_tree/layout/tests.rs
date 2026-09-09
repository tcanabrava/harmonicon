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

/// How many pairs of edges cross — the thing the ordering pass exists to
/// reduce. Two edges cross when one starts above the other and ends below.
fn crossings(l: &TreeLayout) -> usize {
    let mut n = 0;
    for (i, a) in l.edges.iter().enumerate() {
        for b in l.edges.iter().skip(i + 1) {
            if a.from.0 == b.from.0 && a.to.0 == b.to.0 {
                let starts_above = a.from.1 < b.from.1;
                let ends_above = a.to.1 < b.to.1;
                if starts_above != ends_above && a.from.1 != b.from.1 && a.to.1 != b.to.1 {
                    n += 1;
                }
            }
        }
    }
    n
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
    assert_eq!(
        build(&e, &p).node("bend").unwrap().state,
        NodeState::Mastered
    );
}

#[test]
fn a_lesson_with_no_trainings_never_claims_to_be_mastered() {
    // It would otherwise sit at Passed forever with an empty ring, reading
    // as "you have done none of it" rather than "there is none to do".
    let e = [entry("plain", "t", &[], false)];
    let mut p = PlayerProfile::default();
    pass(&mut p, "plain");
    let l = build(&e, &p);
    let node = l.node("plain").unwrap();
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
    // ring segments on something still locked.
    let e = [
        entry("root", "t", &[], false),
        entry("bend", "t", &["root"], true),
    ];
    let mut p = PlayerProfile::default();
    let r = p.trainings.entry(training_key("bend", 1)).or_default();
    record_training(r, true, 1.0);
    let l = build(&e, &p);
    assert_eq!(l.node("bend").unwrap().state, NodeState::Locked);
    assert!(l.node("bend").unwrap().mastery > 0.0);
}

// ── placement ────────────────────────────────────────────────────────────

#[test]
fn a_column_is_the_lessons_depth() {
    // What makes this a tree rather than a grid: a node's horizontal
    // position is where it sits in the curriculum, not its index in a row.
    let e = [
        entry("root", "x", &[], false),
        entry("mid", "x", &["root"], false),
        entry("leaf", "x", &["mid"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("root").unwrap().column, 0);
    assert_eq!(l.node("mid").unwrap().column, 1);
    assert_eq!(l.node("leaf").unwrap().column, 2);
}

#[test]
fn a_node_always_sits_right_of_everything_it_depends_on() {
    let e = [
        entry("a", "t", &[], false),
        entry("b", "u", &["a"], false),
        entry("c", "v", &["a", "b"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    for edge in &l.edges {
        assert!(edge.from.0 < edge.to.0, "edge points backwards: {edge:?}");
    }
}

#[test]
fn a_crowded_column_spills_into_the_next_one() {
    // Nine lessons all three steps in is a wall, not a tree. Depth is only
    // the earliest column a node may take; the layout pushes whole sibling
    // groups right until no column stacks deeper than `MAX_PER_COLUMN`.
    let mut e = vec![entry("root", "t", &[], false)];
    for parent in ["a", "b", "c"] {
        e.push(entry(parent, "t", &["root"], false));
        for child in 0..3 {
            let id = format!("{parent}{child}");
            e.push(entry(&id, "t", &[parent], false));
        }
    }
    let l = build(&e, &PlayerProfile::default());

    let mut per_column: HashMap<usize, usize> = HashMap::new();
    for n in &l.nodes {
        *per_column.entry(n.column).or_default() += 1;
    }
    for (column, count) in per_column {
        assert!(
            count <= MAX_PER_COLUMN,
            "column {column} holds {count} nodes, more than {MAX_PER_COLUMN}"
        );
    }
}

#[test]
fn a_node_pushed_right_drags_what_depends_on_it() {
    // The whole point of spreading is readability; a node landing level
    // with — or left of — its own prerequisite would trade one unreadable
    // shape for a wrong one.
    let mut e = vec![entry("root", "t", &[], false)];
    for parent in ["a", "b", "c"] {
        e.push(entry(parent, "t", &["root"], false));
        for child in 0..3 {
            let id = format!("{parent}{child}");
            e.push(entry(&id, "t", &[parent], false));
            e.push(entry(&format!("{id}x"), "t", &[id.as_str()], false));
        }
    }
    let l = build(&e, &PlayerProfile::default());
    for edge in &l.edges {
        assert!(edge.from.0 < edge.to.0, "edge points backwards: {edge:?}");
    }
}

#[test]
fn a_single_root_sits_alone_in_the_first_column() {
    // The curriculum was given one root on purpose, so the tree opens from
    // one place instead of three unrelated starting points.
    let e = [
        entry("start", "t", &[], false),
        entry("a", "t", &["start"], false),
        entry("b", "u", &["start"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let first: Vec<&str> = l
        .nodes
        .iter()
        .filter(|n| n.column == 0)
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(first, vec!["start"]);
}

#[test]
fn a_short_column_is_centred_against_a_tall_one() {
    // Otherwise the root pins to the top corner and the tree hangs off it.
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
        entry("c", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let root_row = l.node("root").unwrap().row;
    let children: Vec<f32> = ["a", "b", "c"]
        .iter()
        .map(|id| l.node(id).unwrap().row)
        .collect();
    let mean = children.iter().sum::<f32>() / 3.0;
    assert!(
        (root_row - mean).abs() < 0.01,
        "root at {root_row} should sit level with its children's mean {mean}"
    );
}

#[test]
fn siblings_in_one_column_never_share_a_row() {
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
        entry("c", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let mut rows: Vec<f32> = l
        .nodes
        .iter()
        .filter(|n| n.column == 1)
        .map(|n| n.row)
        .collect();
    rows.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for w in rows.windows(2) {
        assert!(w[1] - w[0] >= 1.0, "rows overlap: {rows:?}");
    }
}

#[test]
fn ordering_untangles_edges_that_would_otherwise_cross() {
    // `x` depends on the second of the pair and `y` on the first, so the
    // catalogue's own order draws them crossed. The barycentre sweep is
    // what puts them back.
    let e = [
        entry("root", "t", &[], false),
        entry("first", "t", &["root"], false),
        entry("second", "t", &["root"], false),
        entry("x", "t", &["second"], false),
        entry("y", "t", &["first"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(crossings(&l), 0, "layout left crossings in a solvable case");
}

#[test]
fn the_same_curriculum_always_lays_out_the_same_way() {
    // The sweep is iterative; if it were order-dependent on a hash map the
    // tree would reshuffle between runs.
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "u", &["root"], false),
        entry("c", "v", &["a", "b"], false),
    ];
    let once = build(&e, &PlayerProfile::default());
    let twice = build(&e, &PlayerProfile::default());
    assert_eq!(once, twice);
}

// ── shape ────────────────────────────────────────────────────────────────

#[test]
fn a_track_travels_with_its_node_for_colouring() {
    // Tracks stopped being rows, so colour is the only thing left carrying
    // the grouping.
    let e = [entry("a", "bend", &[], false)];
    assert_eq!(
        build(&e, &PlayerProfile::default())
            .node("a")
            .unwrap()
            .track,
        "bend"
    );
}

#[test]
fn columns_and_rows_size_the_canvas() {
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.columns(), 2);
    assert_eq!(l.rows(), 2.0);
}

#[test]
fn every_prerequisite_becomes_an_edge_between_placed_nodes() {
    let e = [
        entry("root", "a", &[], false),
        entry("mid", "b", &["root"], false),
        entry("leaf", "b", &["root", "mid"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.edges.len(), 3);
    let placed: Vec<(usize, f32)> = l.nodes.iter().map(|n| (n.column, n.row)).collect();
    for edge in &l.edges {
        assert!(placed.contains(&edge.from), "dangling edge start {edge:?}");
        assert!(placed.contains(&edge.to), "dangling edge end {edge:?}");
    }
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
    let short = [e[0].clone(), e[2].clone()];
    let l = layout(&short, &graph, &PlayerProfile::default());
    assert!(l.node("b").is_none());
    // 'c' keeps its own depth regardless of the hole above it.
    assert_eq!(l.node("c").unwrap().column, 2);
    // And the edge to the missing node is dropped rather than dangling.
    assert!(l.edges.is_empty());
}

#[test]
fn an_empty_curriculum_lays_out_to_nothing() {
    let l = build(&[], &PlayerProfile::default());
    assert!(l.nodes.is_empty());
    assert!(l.edges.is_empty());
    assert_eq!(l.columns(), 0);
    assert_eq!(l.rows(), 0.0);
}
