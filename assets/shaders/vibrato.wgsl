// Vibrato note body: a filled band with straight horizontal top/bottom edges and
// sine-curved left/right edges. Drawn as a UI material so the shape is smooth and
// antialiased (no stacked rectangles), while the node itself is laid out and
// scrolled like any other note tile.

#import bevy_ui::ui_vertex_output::UiVertexOutput

// params: x = sway amplitude (fraction of width, 0..0.5)
//         y = wave cycles down the note
//         z = body half-width (fraction of width, 0..0.5)
//         w = phase offset (radians)
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> params: vec4<f32>;

const TAU: f32 = 6.2831853;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let amp = params.x;
    let cycles = params.y;
    let body_half = params.z;
    let phase = params.w;

    // The band sways horizontally as a sine of vertical position. Because it
    // spans the full height, its top (uv.y=0) and bottom (uv.y=1) edges are
    // straight horizontal lines; only the sides follow the curve.
    let center = 0.5 + amp * sin(uv.y * cycles * TAU + phase);
    let dist = abs(uv.x - center);

    // Antialias the curved sides using the screen-space derivative of `dist`.
    let aa = fwidth(dist) + 0.0015;
    let fill = 1.0 - smoothstep(body_half - aa, body_half + aa, dist);

    return vec4<f32>(color.rgb, color.a * fill);
}
