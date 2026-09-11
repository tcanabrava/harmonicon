// SPDX-License-Identifier: MIT

use super::*;

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
