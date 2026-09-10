// SPDX-License-Identifier: MIT

use super::*;
use harmonicon_app::profile::{record_lesson, record_training, training_key};
use harmonicon_song::lessons::{LessonManifest, TrainingBlock};

fn manifest(
    id: &str,
    unit: &str,
    track: &str,
    prerequisites: &[&str],
    trainings: bool,
) -> LessonManifest {
    LessonManifest {
        id: id.to_string(),
        unit: unit.to_string(),
        optional: false,
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

/// A lesson in unit `"u"` — the shape most of these tests want, where the
/// unit is scaffolding rather than the thing under test.
fn entry(id: &str, track: &str, prerequisites: &[&str], trainings: bool) -> LessonEntry {
    entry_in("u", id, track, prerequisites, trainings)
}

fn entry_in(
    unit: &str,
    id: &str,
    track: &str,
    prerequisites: &[&str],
    trainings: bool,
) -> LessonEntry {
    LessonEntry {
        manifest: manifest(id, unit, track, prerequisites, trainings),
        chart_asset_path: Some(format!("{id}.harpchart")),
    }
}

fn build(entries: &[LessonEntry], profile: &PlayerProfile) -> TreeLayout {
    let manifests: Vec<LessonManifest> = entries.iter().map(|e| e.manifest.clone()).collect();
    let graph = LessonGraph::build(&manifests).expect("a valid graph");
    let chain = UnitChain::build(&manifests);
    layout(entries, &graph, &chain, profile)
}

fn build_collapsed(
    entries: &[LessonEntry],
    profile: &PlayerProfile,
    collapsed: &[&str],
) -> TreeLayout {
    let manifests: Vec<LessonManifest> = entries.iter().map(|e| e.manifest.clone()).collect();
    let graph = LessonGraph::build(&manifests).expect("a valid graph");
    let chain = UnitChain::build(&manifests);
    let collapsed = collapsed.iter().map(|id| (*id).to_string()).collect();
    layout_with_collapsed(entries, &graph, &chain, profile, &collapsed)
}

fn pass(profile: &mut PlayerProfile, id: &str) {
    let r = profile.lessons.entry(id.to_string()).or_default();
    record_lesson(r, true, 1.0);
}

/// Passes enough of `unit`'s lessons to open the one after it.
fn satisfy(profile: &mut PlayerProfile, entries: &[LessonEntry], unit: &str) {
    let manifests: Vec<LessonManifest> = entries.iter().map(|e| e.manifest.clone()).collect();
    let chain = UnitChain::build(&manifests);
    let ix = chain.index_of(unit).expect("unit exists");
    for id in chain.units()[ix].lessons.iter().take(chain.required(ix)) {
        pass(profile, id);
    }
}

/// How many pairs of edges cross — the thing the ordering pass exists to
/// reduce. Two edges cross when one starts above the other and ends below.
fn crossings(l: &TreeLayout) -> usize {
    let mut n = 0;
    for (i, a) in l.edges.iter().enumerate() {
        for b in l.edges.iter().skip(i + 1) {
            if a.from.1 == b.from.1 && a.to.1 == b.to.1 {
                let starts_left = a.from.0 < b.from.0;
                let ends_left = a.to.0 < b.to.0;
                if starts_left != ends_left && a.from.0 != b.from.0 && a.to.0 != b.to.0 {
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
fn a_lesson_in_a_shut_unit_is_locked_even_with_every_prerequisite_met() {
    // The second gate. `b1` needs nothing at all, so without the unit gate
    // it would read as playable from the first minute of the game.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("beta", "b1", "t", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("b1").unwrap().state, NodeState::Locked);

    let mut p = PlayerProfile::default();
    satisfy(&mut p, &e, "alpha");
    assert_eq!(
        build(&e, &p).node("b1").unwrap().state,
        NodeState::Available
    );
}

#[test]
fn a_locked_lesson_names_what_it_is_waiting_on() {
    // The cross-unit prerequisites aren't drawn as edges any more, so this
    // is the only place that information still surfaces.
    let e = [
        entry("root", "t", &[], false),
        entry("later", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("later").unwrap().unmet, ["lesson-root-title"]);
    assert!(l.node("root").unwrap().unmet.is_empty());
}

#[test]
fn a_passed_prerequisite_drops_off_the_waiting_list() {
    let e = [
        entry("root", "t", &[], false),
        entry("later", "t", &["root"], false),
    ];
    let mut p = PlayerProfile::default();
    pass(&mut p, "root");
    assert!(build(&e, &p).node("later").unwrap().unmet.is_empty());
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

// ── the spine ────────────────────────────────────────────────────────────

#[test]
fn units_run_left_to_right_along_one_row() {
    // A straight spine across the top is what keeps its own edges from
    // crossing anything: everything else hangs below it.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("beta", "b1", "t", &[], false),
        entry_in("gamma", "c1", "t", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let ids: Vec<&str> = l.units.iter().map(|u| u.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "beta", "gamma"]);
    assert!(l.units.iter().all(|u| u.row == SPINE_ROW));
    for pair in l.units.windows(2) {
        assert!(
            pair[0].column < pair[1].column,
            "units out of order: {:?}",
            l.units
        );
    }
}

#[test]
fn each_unit_is_linked_to_the_next() {
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("beta", "b1", "t", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let spine: Vec<&Edge> = l
        .edges
        .iter()
        .filter(|x| x.kind == EdgeKind::Spine)
        .collect();
    assert_eq!(spine.len(), 1);
    assert_eq!(spine[0].from, (l.units[0].column, SPINE_ROW));
    assert_eq!(spine[0].to, (l.units[1].column, SPINE_ROW));
}

#[test]
fn a_units_lessons_all_sit_under_it_and_left_of_the_next_unit() {
    // The claim the whole drawing makes: a cluster is a group, and it is
    // *this* unit's group.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("alpha", "a2", "t", &["a1"], false),
        entry_in("beta", "b1", "t", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let beta_column = l.unit("beta").unwrap().column;
    for id in ["a1", "a2"] {
        let n = l.node(id).unwrap();
        assert!(n.row >= CLUSTER_TOP_ROW, "{id} is level with the spine");
        assert!(n.column < beta_column, "{id} strayed into the next unit");
    }
    assert!(l.node("b1").unwrap().column >= beta_column);
}

#[test]
fn a_cluster_root_hangs_off_its_own_unit_node() {
    // Otherwise the first column of every unit floats unattached.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("alpha", "a2", "t", &["a1"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let unit = l.unit("alpha").unwrap();
    let a1 = l.node("a1").unwrap();
    assert!(
        l.edges.iter().any(|x| {
            x.kind == EdgeKind::UnitBranch
                && x.from == (unit.column, unit.row)
                && x.to == (a1.column, a1.row)
        }),
        "no edge from the unit to its root lesson: {:?}",
        l.edges
    );
}

#[test]
fn a_cross_unit_prerequisite_is_not_drawn() {
    // The whole reason for the two-level shape: as one flat graph this is
    // the edge that spans the width of a cluster and cuts through whatever
    // lies in between. The unit gate stands in for it.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("beta", "b1", "t", &["a1"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let a1 = l.node("a1").unwrap();
    let b1 = l.node("b1").unwrap();
    assert!(
        !l.edges
            .iter()
            .any(|x| x.from == (a1.column, a1.row) && x.to == (b1.column, b1.row)),
        "the cross-unit edge was drawn after all: {:?}",
        l.edges
    );
    // Dropped from the drawing, not from the reasoning.
    assert_eq!(b1.unmet, ["lesson-a1-title"]);
}

#[test]
fn a_unit_reports_how_close_it_is_to_opening_the_next() {
    // A gate whose terms the player can't see is just an obstacle.
    let e: Vec<LessonEntry> = (1..=5)
        .map(|i| entry_in("alpha", &format!("a{i}"), "t", &[], false))
        .collect();
    let mut p = PlayerProfile::default();
    pass(&mut p, "a1");
    pass(&mut p, "a2");
    let l = build(&e, &p);
    let unit = l.unit("alpha").unwrap();
    assert_eq!(unit.completed, 2);
    assert_eq!(unit.required, 4, "70% of five, rounded up");
    assert!(!unit.locked, "the first unit is always open");
}

#[test]
fn a_unit_the_player_has_not_reached_is_locked() {
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("beta", "b1", "t", &[], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert!(!l.unit("alpha").unwrap().locked);
    assert!(l.unit("beta").unwrap().locked);

    let mut p = PlayerProfile::default();
    satisfy(&mut p, &e, "alpha");
    assert!(!build(&e, &p).unit("beta").unwrap().locked);
}

// ── placement inside a cluster ───────────────────────────────────────────

#[test]
fn a_row_is_the_lessons_depth_within_its_unit() {
    // What makes this a course tree rather than a grid: vertical position
    // records how many prerequisite steps came before a lesson.
    let e = [
        entry("root", "x", &[], false),
        entry("mid", "x", &["root"], false),
        entry("leaf", "x", &["mid"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.node("root").unwrap().row, CLUSTER_TOP_ROW);
    assert_eq!(l.node("mid").unwrap().row, CLUSTER_TOP_ROW + 1.0);
    assert_eq!(l.node("leaf").unwrap().row, CLUSTER_TOP_ROW + 2.0);
}

#[test]
fn depth_restarts_in_each_unit() {
    // A cluster is laid out on its own, so a deep lesson in unit 1 doesn't
    // push unit 2's first lesson to the right of it.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        entry_in("alpha", "a2", "t", &["a1"], false),
        entry_in("alpha", "a3", "t", &["a2"], false),
        entry_in("beta", "b1", "t", &["a3"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let beta = l.unit("beta").unwrap();
    assert_eq!(
        l.node("b1").unwrap().row,
        CLUSTER_TOP_ROW,
        "the first lesson of a unit starts at that unit's first depth"
    );
    assert_eq!(l.node("b1").unwrap().column, beta.column);
}

#[test]
fn a_node_always_sits_below_everything_it_depends_on_in_its_unit() {
    let e = [
        entry("a", "t", &[], false),
        entry("b", "u", &["a"], false),
        entry("c", "v", &["a", "b"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    for edge in l.edges.iter().filter(|x| x.from.1 >= CLUSTER_TOP_ROW) {
        assert!(edge.from.1 < edge.to.1, "edge points upwards: {edge:?}");
    }
}

#[test]
fn a_single_root_sits_alone_in_the_first_lesson_row() {
    let e = [
        entry("start", "t", &[], false),
        entry("a", "t", &["start"], false),
        entry("b", "u", &["start"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let first: Vec<&str> = l
        .nodes
        .iter()
        .filter(|n| n.row == CLUSTER_TOP_ROW)
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(first, vec!["start"]);
}

#[test]
fn a_short_row_is_centred_against_a_wide_one() {
    // Otherwise the root pins to the left corner and the cluster hangs off it.
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
        entry("c", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let root_column = l.node("root").unwrap().column;
    let children: Vec<f32> = ["a", "b", "c"]
        .iter()
        .map(|id| l.node(id).unwrap().column)
        .collect();
    let mean = children.iter().sum::<f32>() / 3.0;
    assert!(
        (root_column - mean).abs() < 0.01,
        "root at {root_column} should be centred over its children's mean {mean}"
    );
}

#[test]
fn a_unit_is_centred_over_its_widest_lesson_row() {
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
        entry("c", "t", &["root"], false),
        entry("d", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let children: Vec<f32> = ["a", "b", "c", "d"]
        .iter()
        .map(|id| l.node(id).unwrap().column)
        .collect();
    let midpoint = (children[0] + children[3]) / 2.0;
    assert_eq!(l.unit("u").unwrap().column, midpoint);
    assert_eq!(l.node("root").unwrap().column, midpoint);
}

#[test]
fn no_lesson_is_ever_level_with_the_spine() {
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert!(l.nodes.iter().all(|n| n.row >= CLUSTER_TOP_ROW));
}

#[test]
fn siblings_in_one_depth_row_never_share_a_column() {
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
        entry("c", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    let mut columns: Vec<f32> = l
        .nodes
        .iter()
        .filter(|n| n.row == CLUSTER_TOP_ROW + 1.0)
        .map(|n| n.column)
        .collect();
    columns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for w in columns.windows(2) {
        assert!(w[1] - w[0] >= 1.0, "columns overlap: {columns:?}");
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
fn an_elective_marker_travels_with_its_node() {
    let mut e = entry("advanced", "bend", &[], false);
    e.manifest.optional = true;
    assert!(
        build(&[e], &PlayerProfile::default())
            .node("advanced")
            .unwrap()
            .optional
    );
}

/// An elective: visible and playable, but outside its unit's gate.
fn elective(mut e: LessonEntry) -> LessonEntry {
    e.manifest.optional = true;
    e
}

/// The edge arriving at `id`, whatever it hangs off.
fn edge_into<'a>(l: &'a TreeLayout, id: &str) -> &'a Edge {
    let node = l.node(id).expect("a placed node");
    l.edges
        .iter()
        .find(|x| x.to == (node.column, node.row))
        .expect("an edge arriving at it")
}

#[test]
fn a_branch_is_elective_when_the_lesson_it_arrives_at_is() {
    // What makes the drawing say "you may skip this": the *destination*
    // decides, because that is what the edge is about.
    let e = [
        entry("core", "t", &[], false),
        entry("also-core", "t", &["core"], false),
        elective(entry("overblows", "bend", &["core"], false)),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert!(edge_into(&l, "overblows").optional);
    assert!(!edge_into(&l, "also-core").optional);
}

#[test]
fn an_elective_hanging_off_an_elective_stays_elective() {
    // Core may never depend on an elective, so once a branch leaves the
    // required path everything further down it is optional too.
    let e = [
        entry("core", "t", &[], false),
        elective(entry("overblows", "bend", &["core"], false)),
        elective(entry("overblow-licks", "bend", &["overblows"], false)),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert!(edge_into(&l, "overblow-licks").optional);
}

#[test]
fn a_unit_branch_into_an_elective_root_is_elective() {
    // An elective needing no prerequisites hangs straight off its unit, so
    // that edge is the only one carrying the distinction.
    let e = [
        entry("core", "t", &[], false),
        elective(entry("standalone", "bend", &[], false)),
    ];
    let l = build(&e, &PlayerProfile::default());
    let edge = edge_into(&l, "standalone");
    assert_eq!(edge.kind, EdgeKind::UnitBranch);
    assert!(edge.optional);
}

#[test]
fn the_spine_is_never_elective() {
    // A unit gate counts core lessons only, so the path from unit to unit
    // is mandatory by construction — even a unit made entirely of
    // electives is still on it.
    let e = [
        entry_in("alpha", "a1", "t", &[], false),
        elective(entry_in("beta", "b1", "t", &[], false)),
    ];
    let l = build(&e, &PlayerProfile::default());
    let spine: Vec<&Edge> = l
        .edges
        .iter()
        .filter(|x| x.kind == EdgeKind::Spine)
        .collect();
    assert!(!spine.is_empty(), "no spine to check");
    assert!(spine.iter().all(|x| !x.optional));
}

#[test]
fn columns_and_rows_size_the_canvas() {
    let e = [
        entry("root", "t", &[], false),
        entry("a", "t", &["root"], false),
        entry("b", "t", &["root"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    assert_eq!(l.columns(), 2.0);
    // One spine row plus two dependency-depth rows.
    assert_eq!(l.rows(), 3.0);
}

#[test]
fn a_collapsed_unit_reclaims_its_cluster_width() {
    let e = [
        entry_in("alpha", "root", "t", &[], false),
        entry_in("alpha", "a", "t", &["root"], false),
        entry_in("alpha", "b", "t", &["root"], false),
        entry_in("beta", "next", "t", &[], false),
    ];
    let expanded = build(&e, &PlayerProfile::default());
    let collapsed = build_collapsed(&e, &PlayerProfile::default(), &["alpha"]);

    assert!(collapsed.columns() < expanded.columns());
    assert!(collapsed.unit("beta").unwrap().column < expanded.unit("beta").unwrap().column);
    let alpha = collapsed.unit("alpha").unwrap();
    assert!(
        collapsed
            .nodes
            .iter()
            .filter(|node| node.unit_id == "alpha")
            .all(|node| node.column == alpha.column && node.row == alpha.row)
    );
}

#[test]
fn collapsing_one_unit_does_not_move_earlier_units() {
    let e = [
        entry_in("alpha", "first", "t", &[], false),
        entry_in("beta", "root", "t", &[], false),
        entry_in("beta", "a", "t", &["root"], false),
        entry_in("beta", "b", "t", &["root"], false),
    ];
    let expanded = build(&e, &PlayerProfile::default());
    let collapsed = build_collapsed(&e, &PlayerProfile::default(), &["beta"]);

    assert_eq!(collapsed.unit("alpha"), expanded.unit("alpha"));
    assert_eq!(collapsed.node("first"), expanded.node("first"));
}

#[test]
fn the_canvas_is_sized_for_the_spine_even_with_no_lessons_under_it() {
    // A unit whose lessons the catalogue can't label would otherwise leave
    // the canvas too narrow for its own spine.
    let e = [entry("a", "t", &[], false)];
    let l = build(&e, &PlayerProfile::default());
    let spine_width = l
        .units
        .iter()
        .map(|u| u.column + 1.0)
        .fold(0.0_f32, f32::max);
    assert!(l.columns() >= spine_width);
}

#[test]
fn every_prerequisite_becomes_an_edge_between_placed_positions() {
    let e = [
        entry("root", "a", &[], false),
        entry("mid", "b", &["root"], false),
        entry("leaf", "b", &["root", "mid"], false),
    ];
    let l = build(&e, &PlayerProfile::default());
    // root→mid, root→leaf, mid→leaf, plus the unit hanging onto root.
    assert_eq!(l.edges.len(), 4);
    let placed: Vec<(f32, f32)> = l
        .nodes
        .iter()
        .map(|n| (n.column, n.row))
        .chain(l.units.iter().map(|u| (u.column, u.row)))
        .collect();
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
    let chain = UnitChain::build(&manifests);
    let short = [e[0].clone(), e[2].clone()];
    let l = layout(&short, &graph, &chain, &PlayerProfile::default());
    assert!(l.node("b").is_none());
    // With nothing left to depend on, 'c' becomes a root of its cluster
    // rather than keeping a column it can no longer be connected to.
    assert_eq!(l.node("c").unwrap().row, CLUSTER_TOP_ROW);
    // 'c' is still locked, and still says what it wants — the reasoning
    // survives the hole in the drawing.
    assert_eq!(l.node("c").unwrap().state, NodeState::Locked);
    assert_eq!(l.node("c").unwrap().unmet, ["b"]);
    // Both are cluster roots, so both hang off the unit and nothing dangles.
    assert!(l.edges.iter().all(|x| x.kind == EdgeKind::UnitBranch));
    assert_eq!(l.edges.len(), 2);
}

#[test]
fn an_empty_curriculum_lays_out_to_nothing() {
    let l = build(&[], &PlayerProfile::default());
    assert!(l.nodes.is_empty());
    assert!(l.units.is_empty());
    assert!(l.edges.is_empty());
    assert_eq!(l.columns(), 0.0);
    assert_eq!(l.rows(), 0.0);
}
