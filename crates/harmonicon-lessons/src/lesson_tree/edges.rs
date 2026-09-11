// SPDX-License-Identifier: MIT

//! The skill tree's edges, as a `UiMaterial` on an axis-aligned node.
//!
//! **Why not a rotated rectangle.** `bevy_ui` clips a node by pushing its
//! four transformed corners inside the clip rect
//! (`bevy_ui_render`'s own comment: "this won't work with
//! rotation/scaling"). On an axis-aligned quad that is a correct clip; on a
//! rotated one it shears the quad rather than cutting it. The tree lives in
//! a scroll area, which clips, so every edge running off the viewport came
//! out skewed. Keeping the node axis-aligned and drawing the line in a
//! fragment shader puts clipping back on the path `bevy_ui` handles
//! properly.
//!
//! Same "custom shader for a shape a plain `Node` can't express" pattern as
//! `gameplay::note_tail_2d` and `music_score::tie_material`.
//!
//! **The material carries no endpoints.** A node here *is* its segment's
//! bounding box, padded by the half-thickness, and a segment always spans
//! its own bounding box corner to corner — so the shader rebuilds both ends
//! from the node's size, and only needs to be told which diagonal. Two
//! consequences worth having: a node that stretches still draws the right
//! line (so a sliding unit needs no material update), and edges sharing a
//! thickness, colour and diagonal share one handle rather than one per
//! edge.

use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use super::{ClusterMember, LayoutOwner, MovingEdge};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct LessonEdgeMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = half thickness in pixels; y = `+1` when the line runs top-left
    /// to bottom-right and `-1` when it runs bottom-left to top-right;
    /// z/w unused (`AsBindGroup` pads a uniform to `vec4` regardless).
    #[uniform(1)]
    pub params: Vec4,
}

impl UiMaterial for LessonEdgeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lesson_edge.wgsl".into()
    }
}

pub(crate) struct LessonEdgeMaterialPlugin;

impl Plugin for LessonEdgeMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<LessonEdgeMaterial>::default());
    }
}

/// The axis-aligned box a segment is drawn inside: its bounding box grown
/// by `half_thickness` on every side, so the stroke has room at the ends
/// and along a horizontal or vertical run.
///
/// Returns the box's top-left corner and its size. Pure, because it is the
/// half of edge drawing that can be wrong in a way a screenshot would not
/// obviously show.
pub(crate) fn edge_box(start: Vec2, end: Vec2, half_thickness: f32) -> (Vec2, Vec2) {
    let pad = Vec2::splat(half_thickness);
    let top_left = start.min(end) - pad;
    let size = (start - end).abs() + pad * 2.0;
    (top_left, size)
}

/// Which diagonal of [`edge_box`] the segment runs along: `1.0` from
/// top-left to bottom-right, `-1.0` from bottom-left to top-right.
///
/// A run with no extent on one axis lands on `1.0`, which is right either
/// way — both diagonals of a box that thin describe the same line.
pub(crate) fn edge_diagonal(start: Vec2, end: Vec2) -> f32 {
    let delta = end - start;
    if (delta.x >= 0.0) == (delta.y >= 0.0) {
        1.0
    } else {
        -1.0
    }
}

/// Clearance two node boundaries need before an edge between them is worth
/// drawing at all.
pub(crate) const EDGE_MIN_LENGTH_PX: f32 = 1.0;

/// Gap between the dots of an elective branch. A dot is the edge's own
/// thickness across, so a dotted line carries the same weight as a solid
/// one and only the continuity differs.
pub(crate) const EDGE_DOT_GAP_PX: f32 = 7.0;

/// One end of an edge: where a node sits, and how far its art reaches from
/// that centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Endpoint {
    pub centre: Vec2,
    pub radius: f32,
}

/// The visible run of an edge: the segment of the line joining two centres
/// that lies *between* the two nodes' boundaries.
///
/// Both ends are pulled back along that same line, each by its own radius,
/// so the edge points at what it connects from wherever that happens to be
/// — below, above, or off to one side. `None` when the boundaries already
/// meet and there is nothing left to draw.
pub(crate) fn edge_span(from: Endpoint, to: Endpoint) -> Option<(Vec2, Vec2)> {
    let offset = to.centre - from.centre;
    let gap = offset.length();
    // Tested against the gap rather than the resulting segment's length:
    // two nodes nearer than their combined radii would pull each end past
    // the other, and a backwards segment has a perfectly respectable
    // positive length while running through both nodes it claims to join.
    // This also covers two centres landing on the same point.
    if gap < from.radius + to.radius + EDGE_MIN_LENGTH_PX {
        return None;
    }
    let direction = offset / gap;
    Some((
        from.centre + direction * from.radius,
        to.centre - direction * to.radius,
    ))
}

/// Repositions a moving edge's node.
///
/// Only the box moves: the shader rebuilds the line from the node's own
/// size, so a translating or stretching edge needs no material update. The
/// diagonal *is* baked into the material at spawn, which holds because an
/// edge either slides rigidly (both ends belong to one unit) or stretches
/// along the horizontal spine — neither flips which way it runs.
pub(crate) fn set_edge_geometry(
    node: &mut Node,
    from: Endpoint,
    to: Endpoint,
    thickness: f32,
) -> bool {
    let Some((start, end)) = edge_span(from, to) else {
        return false;
    };
    let (top_left, size) = edge_box(start, end, thickness / 2.0);
    node.left = Val::Px(top_left.x);
    node.top = Val::Px(top_left.y);
    node.width = Val::Px(size.x);
    node.height = Val::Px(size.y);
    true
}

/// How an edge is drawn, as opposed to where it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EdgeStyle {
    pub thickness: f32,
    pub color: Color,
    /// Elective branches are dotted: the conventional way a dependency
    /// diagram says "you may skip this" without spending a second meaning
    /// on brightness, which here already distinguishes locked from open.
    pub dotted: bool,
}

/// Centres of the dots making up an elective branch, evenly spread so one
/// sits against each node's boundary and the spacing comes out equal.
///
/// Spacing is derived from the span rather than fixed, which is what stops
/// a short edge ending in a ragged half-gap. `diameter` is the dot size, so
/// the first and last centres are inset by half of it.
pub(crate) fn dot_centres(start: Vec2, end: Vec2, diameter: f32, gap: f32) -> Vec<Vec2> {
    let delta = end - start;
    let length = delta.length();
    // Centre-to-centre distance available once both end dots are inset.
    let travel = length - diameter;
    if travel <= 0.0 {
        return vec![(start + end) / 2.0];
    }
    let direction = delta / length;
    let count = ((travel / (diameter + gap)).round() as usize + 1).max(2);
    let spacing = travel / (count - 1) as f32;
    (0..count)
        .map(|i| start + direction * (diameter / 2.0 + spacing * i as f32))
        .collect()
}

/// An elective branch, as a run of round dots.
///
/// Each dot is its own entity carrying [`LayoutOwner`], not one
/// [`MovingEdge`]: a dotted edge is always inside a single cluster (the
/// spine is never elective), so both its endpoints take the same slide
/// offset and the whole run translates rigidly. `animate_unit_slides`
/// writes only `translation`, so each dot keeps the rotation set here.
pub(crate) fn spawn_dotted_edge(
    parent: &mut ChildSpawnerCommands,
    start: Vec2,
    end: Vec2,
    style: EdgeStyle,
    unit_id: Option<&str>,
) {
    let diameter = style.thickness;
    for centre in dot_centres(start, end, diameter, EDGE_DOT_GAP_PX) {
        let mut dot = parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x - diameter / 2.0),
                top: Val::Px(centre.y - diameter / 2.0),
                width: Val::Px(diameter),
                height: Val::Px(diameter),
                // A square with a maximal radius is a circle.
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(style.color),
        ));
        if let Some(unit_id) = unit_id {
            dot.insert((
                ClusterMember(unit_id.to_string()),
                LayoutOwner(unit_id.to_string()),
            ));
        }
    }
}

/// One edge, as a single rotated rectangle.
///
/// A straight run needs no sampling and no joins, so there is exactly one
/// node per edge and no seam anywhere along it.
pub(crate) fn spawn_edge(
    parent: &mut ChildSpawnerCommands,
    from: Endpoint,
    to: Endpoint,
    style: EdgeStyle,
    unit_id: Option<&str>,
    owners: Option<(&str, &str)>,
    materials: &mut Assets<LessonEdgeMaterial>,
) {
    let Some((start, end)) = edge_span(from, to) else {
        return;
    };
    let EdgeStyle {
        thickness, color, ..
    } = style;
    if style.dotted {
        spawn_dotted_edge(parent, start, end, style, unit_id);
        return;
    }
    let (top_left, size) = edge_box(start, end, thickness / 2.0);

    let mut segment = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(top_left.x),
            top: Val::Px(top_left.y),
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            ..default()
        },
        // The node stays axis-aligned and the shader draws the line inside
        // it — see `edge_material`. A rotated node would be sheared rather
        // than clipped wherever it left the scroll viewport.
        MaterialNode(materials.add(LessonEdgeMaterial {
            color: color.into(),
            params: Vec4::new(thickness / 2.0, edge_diagonal(start, end), 0.0, 0.0),
        })),
    ));
    if let Some((from_unit, to_unit)) = owners {
        segment.insert(MovingEdge {
            from,
            to,
            from_unit: from_unit.to_string(),
            to_unit: to_unit.to_string(),
            thickness,
        });
    }
    if let Some(unit_id) = unit_id {
        segment.insert(ClusterMember(unit_id.to_string()));
    }
}

#[cfg(test)]
mod tests;
