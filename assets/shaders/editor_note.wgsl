// SPDX-License-Identifier: MIT

// Editor note material for Vibrato (~~~~~) and Wah (OoOoOo) expressions.
// Unlike the gameplay tail shader this one runs on the horizontal axis of the
// note tile, so the pattern travels left-to-right along the note's duration.
//
// uv.x = 0 is the note's left edge, 1 is the right edge (along duration).
// uv.y = 0 is the top, 1 is the bottom (across the note's height).
//
// params.x = mode:  0 = vibrato (sine wave ribbon),  1 = wah (thick/thin band)

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> params: vec4<f32>;

const TAU: f32 = 6.2831853;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv  = in.uv;
    let mode = params.x;

    if (mode < 0.5) {
        // Vibrato: a sine-wave ribbon running horizontally (~~~~~).
        // The wave rises and dips in Y as the eye travels left to right in X.
        let amp    = 0.28;
        let cycles = 3.0 + uv.x * 0.5;   // slight stretch toward the right
        let center = 0.5 + amp * sin(uv.x * cycles * TAU);

        let dist      = abs(uv.y - center);
        let ribbon_r  = 0.07;
        let aa        = fwidth(dist) * 1.5;
        let core      = 1.0 - smoothstep(ribbon_r - aa, ribbon_r + aa, dist);
        let glow      = exp(-dist * 13.0) * 0.35;
        let alpha     = clamp(max(core, glow), 0.0, 1.0);
        return vec4<f32>(color.rgb, alpha * color.a);
    }

    // Wah: alternating thick/thin band running horizontally (OoOoOo).
    // The vertical extent at each X position oscillates between wide (O) and
    // narrow (o), producing a string of lens-shaped bulges and pinch points.
    let cycles = 3.5;
    let bulge  = abs(sin(uv.x * cycles * TAU));  // 1 at O, 0 at o
    let half_h = 0.10 + 0.35 * bulge;

    let dist   = abs(uv.y - 0.5);
    let aa     = fwidth(dist) * 1.5;
    let body   = 1.0 - smoothstep(half_h - aa, half_h + aa, dist);
    let glow   = exp(-max(dist - half_h, 0.0) * 16.0) * 0.30;
    let alpha  = clamp(max(body, glow), 0.0, 1.0);
    return vec4<f32>(color.rgb, alpha * color.a);
}
