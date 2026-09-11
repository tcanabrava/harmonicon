// SPDX-License-Identifier: MIT

use super::*;
use crate::lesson_tree::{EDGE_PX, NODE_PX, UNIT_PX, node_centre};

const HALF: f32 = 1.75;

/// The box has to *contain* the segment with the stroke's own width to
/// spare, or the line is clipped by its own node before the scroll area
/// ever sees it.
fn assert_contains(start: Vec2, end: Vec2) {
    let (top_left, size) = edge_box(start, end, HALF);
    let bottom_right = top_left + size;
    for point in [start, end] {
        assert!(
            point.x - HALF >= top_left.x - 1.0e-4 && point.x + HALF <= bottom_right.x + 1.0e-4,
            "{point} is not inside {top_left}..{bottom_right} horizontally"
        );
        assert!(
            point.y - HALF >= top_left.y - 1.0e-4 && point.y + HALF <= bottom_right.y + 1.0e-4,
            "{point} is not inside {top_left}..{bottom_right} vertically"
        );
    }
}

#[test]
fn the_box_holds_the_whole_stroke_whichever_way_the_edge_runs() {
    let cases = [
        (Vec2::new(10.0, 10.0), Vec2::new(90.0, 60.0)),
        (Vec2::new(90.0, 60.0), Vec2::new(10.0, 10.0)),
        (Vec2::new(10.0, 60.0), Vec2::new(90.0, 10.0)),
        (Vec2::new(90.0, 10.0), Vec2::new(10.0, 60.0)),
    ];
    for (start, end) in cases {
        assert_contains(start, end);
    }
}

#[test]
fn a_horizontal_or_vertical_edge_still_gets_a_box_with_thickness() {
    // The degenerate case: one axis of the bounding box is zero, so
    // without the padding the node would have no height (or width) to
    // draw a stroke in at all.
    let (_, horizontal) = edge_box(Vec2::new(10.0, 40.0), Vec2::new(90.0, 40.0), HALF);
    assert_eq!(horizontal.y, HALF * 2.0);
    assert_eq!(horizontal.x, 80.0 + HALF * 2.0);

    let (_, vertical) = edge_box(Vec2::new(40.0, 10.0), Vec2::new(40.0, 90.0), HALF);
    assert_eq!(vertical.x, HALF * 2.0);
    assert_eq!(vertical.y, 80.0 + HALF * 2.0);
}

#[test]
fn the_box_is_the_same_whichever_end_is_given_first() {
    // The shader reconstructs the line from the box and a diagonal, so a
    // reversed edge has to produce an identical box — otherwise the same
    // relationship drawn either way would land in two different places.
    let (a, b) = (Vec2::new(12.0, 80.0), Vec2::new(97.0, 31.0));
    assert_eq!(edge_box(a, b, HALF), edge_box(b, a, HALF));
    assert_eq!(edge_diagonal(a, b), edge_diagonal(b, a));
}

#[test]
fn the_diagonal_follows_the_direction_the_edge_actually_runs() {
    // Screen y grows downward, so "down and to the right" is the same
    // diagonal as "up and to the left".
    assert_eq!(
        edge_diagonal(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
        1.0
    );
    assert_eq!(
        edge_diagonal(Vec2::new(10.0, 10.0), Vec2::new(0.0, 0.0)),
        1.0
    );
    assert_eq!(
        edge_diagonal(Vec2::new(0.0, 10.0), Vec2::new(10.0, 0.0)),
        -1.0
    );
    assert_eq!(
        edge_diagonal(Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)),
        -1.0
    );
}

#[test]
fn the_reconstructed_endpoints_match_the_ones_the_box_was_built_from() {
    // The invariant the whole no-uniform-endpoints idea rests on: the
    // shader insets the box by the half-thickness and takes the diagonal
    // `edge_diagonal` chose. That has to land back on the real endpoints.
    for (start, end) in [
        (Vec2::new(10.0, 10.0), Vec2::new(90.0, 60.0)),
        (Vec2::new(10.0, 60.0), Vec2::new(90.0, 10.0)),
        (Vec2::new(90.0, 60.0), Vec2::new(10.0, 10.0)),
        (Vec2::new(40.0, 10.0), Vec2::new(40.0, 90.0)),
    ] {
        let (top_left, size) = edge_box(start, end, HALF);
        let pad = Vec2::splat(HALF);
        let (a, b) = if edge_diagonal(start, end) > 0.0 {
            (top_left + pad, top_left + size - pad)
        } else {
            (
                Vec2::new(top_left.x + HALF, top_left.y + size.y - HALF),
                Vec2::new(top_left.x + size.x - HALF, top_left.y + HALF),
            )
        };
        let matches_forward = a.distance(start) < 1.0e-4 && b.distance(end) < 1.0e-4;
        let matches_reversed = a.distance(end) < 1.0e-4 && b.distance(start) < 1.0e-4;
        assert!(
            matches_forward || matches_reversed,
            "rebuilt {a}..{b} from the box, but the edge runs {start}..{end}"
        );
    }
}

fn lesson(column: f32, row: f32) -> Endpoint {
    Endpoint {
        centre: node_centre(column, row),
        radius: NODE_PX / 2.0,
    }
}

fn unit(column: f32, row: f32) -> Endpoint {
    Endpoint {
        centre: node_centre(column, row),
        radius: UNIT_PX / 2.0,
    }
}

#[test]
fn an_edge_leaves_the_bottom_of_a_parent_and_arrives_at_the_top_of_its_child() {
    // The tree flows downward, so a child directly below its parent is
    // the ordinary case: the line has to run down the gap between them
    // rather than out of either one's flank.
    let (from, to) = (lesson(2.5, 1.0), lesson(2.5, 2.0));
    let (start, end) = edge_span(from, to).expect("nodes a whole row apart");
    assert_eq!(start, from.centre + Vec2::Y * NODE_PX / 2.0);
    assert_eq!(end, to.centre - Vec2::Y * NODE_PX / 2.0);
}

#[test]
fn an_edge_points_the_way_its_centres_do() {
    // The invariant that keeps a line off the art it connects: whatever
    // direction the child lies in, both ends move along *that* line. A
    // child down and to the left is left by the parent's lower-left.
    let (from, to) = (lesson(2.5, 2.0), lesson(0.0, 3.0));
    let (start, end) = edge_span(from, to).expect("nodes a whole row apart");
    let along = (end - start).normalize();
    let centres = (to.centre - from.centre).normalize();
    assert!(
        along.distance(centres) < 1.0e-5,
        "edge runs {along} but its nodes lie {centres} apart",
    );
    assert!(start.x < from.centre.x, "the edge left the wrong side");
    assert!(start.y > from.centre.y, "the edge left the wrong side");
}

#[test]
fn each_end_is_clipped_by_the_radius_of_the_node_it_touches() {
    // A spine edge meets two unit rings; a unit branch meets a ring at
    // the top and a lesson at the bottom. One shared radius would leave
    // the wider node's end buried inside its own art.
    let (spine_start, spine_end) =
        edge_span(unit(0.0, 0.0), unit(7.0, 0.0)).expect("two units apart on the spine");
    assert_eq!(spine_start.x - node_centre(0.0, 0.0).x, UNIT_PX / 2.0);
    assert_eq!(node_centre(7.0, 0.0).x - spine_end.x, UNIT_PX / 2.0);

    let (branch_start, branch_end) =
        edge_span(unit(2.5, 0.0), lesson(2.5, 1.0)).expect("a unit above its root lesson");
    assert_eq!(branch_start.y - node_centre(2.5, 0.0).y, UNIT_PX / 2.0);
    assert_eq!(node_centre(2.5, 1.0).y - branch_end.y, NODE_PX / 2.0);
}

#[test]
fn a_dotted_edge_starts_and_ends_against_the_nodes_it_joins() {
    // A dot sits against each boundary, so a dotted branch reaches its
    // node exactly as far as a solid one does.
    let (start, end) = (Vec2::new(100.0, 100.0), Vec2::new(100.0, 240.0));
    let dots = dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX);
    assert!(dots.len() > 2, "expected a run of dots, got {}", dots.len());
    assert!((dots[0].distance(start) - EDGE_PX / 2.0).abs() < 1.0e-4);
    assert!((dots[dots.len() - 1].distance(end) - EDGE_PX / 2.0).abs() < 1.0e-4);
}

#[test]
fn dots_are_evenly_spaced_however_long_the_edge() {
    // Spacing is derived from the span rather than fixed, which is what
    // stops a short edge finishing on a ragged half-gap.
    for length in [20.0_f32, 58.0, 137.0, 394.0] {
        let (start, end) = (Vec2::ZERO, Vec2::new(length, 0.0));
        let dots = dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX);
        let gaps: Vec<f32> = dots.windows(2).map(|w| w[0].distance(w[1])).collect();
        let first = gaps[0];
        assert!(
            gaps.iter().all(|g| (g - first).abs() < 1.0e-3),
            "uneven spacing over {length}px: {gaps:?}"
        );
    }
}

#[test]
fn dots_follow_a_diagonal_edge() {
    // Every dot has to land *on* the line, not on a bounding box of it.
    let (start, end) = (Vec2::new(40.0, 40.0), Vec2::new(200.0, 160.0));
    let direction = (end - start).normalize();
    for dot in dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX) {
        let along = (dot - start).dot(direction);
        assert!(
            (start + direction * along).distance(dot) < 1.0e-3,
            "dot {dot} sits off the line"
        );
    }
}

#[test]
fn a_span_too_short_for_two_dots_draws_one() {
    let (start, end) = (Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
    assert_eq!(
        dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX),
        vec![Vec2::new(1.0, 0.0)]
    );
}

#[test]
fn nodes_too_close_to_separate_draw_no_edge() {
    // Nearer than their combined radii, the pull-backs would cross and
    // the segment would run backwards through both nodes.
    let touching = Endpoint {
        centre: Vec2::new(100.0, 100.0),
        radius: 40.0,
    };
    let overlapping = Endpoint {
        centre: Vec2::new(110.0, 100.0),
        radius: 40.0,
    };
    assert_eq!(edge_span(touching, overlapping), None);
    assert_eq!(edge_span(touching, touching), None);
}
