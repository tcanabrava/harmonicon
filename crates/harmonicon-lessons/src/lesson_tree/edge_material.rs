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

#[cfg(test)]
mod tests;
