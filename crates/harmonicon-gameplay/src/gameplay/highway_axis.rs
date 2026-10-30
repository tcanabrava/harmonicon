// SPDX-License-Identifier: MIT

//! Which `Node` edges mean what on a highway scrolling along `axis`.
//!
//! Everything on the highway is placed by three things: where it sits
//! *along* the scroll (how far from the hit line's end), how long it is
//! along it, and which span *across* the highway it covers (its lane, or the
//! whole width). A vertical highway scrolls notes down columns, so along is
//! `bottom`/`height` and across is `left`/`width`; a horizontal one (the
//! guitar's tab highway) scrolls them right to left along rows, so along is
//! `left`/`width` and across is `top`/`height`.
//!
//! Every system that places something on the highway goes through these
//! instead of naming an edge, so the two orientations can't drift apart.

use bevy::prelude::*;

use harmonicon_core::instrument::ScrollAxis;

/// Puts `node`'s near edge `pct` percent along the scroll, measured from the
/// end the hit line is at (the bottom, or the left).
pub(super) fn set_along(node: &mut Node, axis: ScrollAxis, pct: f32) {
    let value = Val::Percent(pct);
    match axis {
        ScrollAxis::Vertical => node.bottom = value,
        ScrollAxis::Horizontal => node.left = value,
    }
}

/// Sets how long `node` is along the scroll.
pub(super) fn set_along_size(node: &mut Node, axis: ScrollAxis, size: Val) {
    match axis {
        ScrollAxis::Vertical => node.height = size,
        ScrollAxis::Horizontal => node.width = size,
    }
}

/// `node`'s length along the scroll, as last set.
pub(super) fn along_size(node: &Node, axis: ScrollAxis) -> Val {
    match axis {
        ScrollAxis::Vertical => node.height,
        ScrollAxis::Horizontal => node.width,
    }
}

/// Spans `node` across the highway from `start_pct`, `size` wide (or tall).
/// Lanes are counted from the left, or from the top as tab's string 1 is.
pub(super) fn set_across(node: &mut Node, axis: ScrollAxis, start_pct: f32, size: Val) {
    match axis {
        ScrollAxis::Vertical => {
            node.left = Val::Percent(start_pct);
            node.width = size;
        }
        ScrollAxis::Horizontal => {
            node.top = Val::Percent(start_pct);
            node.height = size;
        }
    }
}

/// `node`'s span across the highway, as last set: `(start, size)`.
pub(super) fn across(node: &Node, axis: ScrollAxis) -> (Val, Val) {
    match axis {
        ScrollAxis::Vertical => (node.left, node.width),
        ScrollAxis::Horizontal => (node.top, node.height),
    }
}

/// A highway of `size` px, measured along the scroll.
pub(super) fn along_px(axis: ScrollAxis, size: Vec2) -> f32 {
    match axis {
        ScrollAxis::Vertical => size.y,
        ScrollAxis::Horizontal => size.x,
    }
}

/// An absolutely-positioned node placed `along_pct` along the scroll and
/// `along_len` long, spanning `start_pct` and `across_len` across it.
pub(super) fn placed(
    axis: ScrollAxis,
    along_pct: f32,
    along_len: Val,
    start_pct: f32,
    across_len: Val,
) -> Node {
    let mut node = Node { position_type: PositionType::Absolute, ..default() };
    set_along(&mut node, axis, along_pct);
    set_along_size(&mut node, axis, along_len);
    set_across(&mut node, axis, start_pct, across_len);
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vertical_highway_scrolls_notes_up_from_the_bottom() {
        let node = placed(ScrollAxis::Vertical, 30.0, Val::Px(40.0), 20.0, Val::Percent(10.0));
        assert_eq!((node.bottom, node.height), (Val::Percent(30.0), Val::Px(40.0)));
        assert_eq!((node.left, node.width), (Val::Percent(20.0), Val::Percent(10.0)));
        assert_eq!(node.top, Val::Auto);
    }

    #[test]
    fn a_horizontal_highway_scrolls_notes_in_from_the_left_along_rows() {
        let node = placed(ScrollAxis::Horizontal, 30.0, Val::Px(40.0), 20.0, Val::Percent(10.0));
        assert_eq!((node.left, node.width), (Val::Percent(30.0), Val::Px(40.0)));
        assert_eq!((node.top, node.height), (Val::Percent(20.0), Val::Percent(10.0)));
        assert_eq!(node.bottom, Val::Auto);
    }

    #[test]
    fn reading_back_agrees_with_what_was_set() {
        for axis in [ScrollAxis::Vertical, ScrollAxis::Horizontal] {
            let node = placed(axis, 5.0, Val::Px(12.0), 40.0, Val::Percent(15.0));
            assert_eq!(along_size(&node, axis), Val::Px(12.0));
            assert_eq!(across(&node, axis), (Val::Percent(40.0), Val::Percent(15.0)));
        }
        assert_eq!(along_px(ScrollAxis::Vertical, Vec2::new(800.0, 600.0)), 600.0);
        assert_eq!(along_px(ScrollAxis::Horizontal, Vec2::new(800.0, 600.0)), 800.0);
    }
}
