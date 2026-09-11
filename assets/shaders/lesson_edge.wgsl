// SPDX-License-Identifier: MIT

// A prerequisite edge for the lesson skill tree, drawn as a line *inside*
// an axis-aligned node rather than as a rotated rectangle.
//
// The rotation is the whole point. `bevy_ui` clips a node by moving its
// four transformed corners inside the clip rect — correct for an
// axis-aligned quad, but on a rotated one it shears the quad instead of
// cutting it (bevy_ui_render's own comment says so where it happens). A
// scroll area clips, so every edge crossing the viewport's boundary came
// out skewed. Here the node stays axis-aligned and the shader draws the
// line, so clipping behaves exactly as `bevy_ui` intends.
//
// No endpoints are passed in: the node *is* the segment's bounding box,
// padded by the line's half-thickness, and a segment always spans its own
// bounding box corner to corner. So the shader reconstructs both ends from
// `in.size` alone, which also means a node that later stretches still
// draws the right line. `params.y` picks which of the two diagonals.

#import bevy_ui::ui_vertex_output::UiVertexOutput

// x = half thickness, in pixels.
// y = +1 when the line runs top-left to bottom-right, -1 when it runs
//     bottom-left to top-right.
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> params: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let half = max(params.x, 0.0001);
    let point = in.uv * in.size;

    // Inset by the half-thickness the node was padded with, so the line's
    // ends land on the real endpoints rather than the padded corners.
    var start = vec2<f32>(half, half);
    var end = in.size - vec2<f32>(half, half);
    if params.y < 0.0 {
        start = vec2<f32>(half, in.size.y - half);
        end = vec2<f32>(in.size.x - half, half);
    }

    // Distance from this fragment to the segment. A purely horizontal or
    // vertical edge collapses one axis of the box to a single
    // half-thickness, which this handles without a special case: the
    // segment simply has no extent on that axis.
    let along = end - start;
    let length_squared = max(dot(along, along), 0.0001);
    let t = clamp(dot(point - start, along) / length_squared, 0.0, 1.0);
    let distance_px = distance(point, start + along * t);

    // One pixel of feathering, which is what keeps a diagonal from
    // staircasing — the reason the rotated-rectangle version needed no
    // antialiasing of its own is that the rasterizer did it.
    let alpha = 1.0 - smoothstep(half - 1.0, half, distance_px);
    return vec4<f32>(color.rgb, color.a * alpha);
}
