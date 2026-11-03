// SPDX-License-Identifier: MIT

// A 2D note: one ribbon down its lane, as long as the note lasts — the UI
// twin of `note_ribbon_3d.wesl`. The bottom is the attack: a bright cap
// (which carries the tab label) with a white timing edge that reaches the
// hit line on time.
//
// The technique is drawn in *note time* — seconds back from the attack —
// so it is right at any window size: a vibrato's crests and a wah's
// open/close pinches are spaced so they cross the hit line at exactly the
// charted rate as the note scrolls down, and a bend leans the ribbon's
// core over its first fifth of a second, further for a deeper bend.
//
// On a vertical highway uv.y = 1 is the attack (bottom) and uv.y = 0 the
// end of the note (top). On a horizontal one (axis.x = 1, a guitar's tab
// highway) the ribbon lies on its side: uv.x = 0 is the attack (left) and
// uv.y runs across it, so `back`/`across` below stand in for both.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

// technique: x = mode (0 plain, 1 bend, 2 vibrato, 3 wah, 4 pitch-up shift),
//            y = oscillation rate (Hz), z = lean across the lane at full
//            bend, signed (-0.5..0.5), w = 1 when a vibrato rides a bend
// shape:     x = note duration (seconds), y = cap height (px), z = animation
//            time (seconds), w = 1 to scroll the pattern with that time (the
//            legend's static previews), 0 for notes, which scroll themselves
// hold:      x = pitch sounding now (1/0), y = hold integrity so far (0..1),
//            z = technique status (1 confirmed, -1 not heard, 0 n/a), w = 1
//            once hit — all zero until then
// axis:      x = 1 on a horizontal highway; y = seconds of the note already
//            consumed by the hit line, so the drawn ribbon starts that far
//            into the note (0 for the legend's previews)
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> technique: vec4<f32>;
@group(1) @binding(2) var<uniform> shape: vec4<f32>;
@group(1) @binding(3) var<uniform> hold: vec4<f32>;
@group(1) @binding(4) var<uniform> axis: vec4<f32>;

const TAU: f32 = 6.2831853;
/// Seconds over which a bend's lean ramps in from the attack.
const BEND_RAMP_SECS: f32 = 0.2;
/// Height of the white timing edge at the very bottom, in px.
const EDGE_PX: f32 = 3.0;

fn ease(t: f32) -> f32 {
    let x = clamp(t, 0.0, 1.0);
    return x * x * (3.0 - 2.0 * x);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let horizontal = axis.x > 0.5;
    // 0 at the attack, 1 at the end of the note.
    let back = select(1.0 - in.uv.y, in.uv.x, horizontal);
    // Across the ribbon, 0..1 from one edge to the other.
    let across = select(in.uv.x, in.uv.y, horizontal);
    let length_px = select(in.size.y, in.size.x, horizontal);
    let d_px = back * length_px;
    let consumed = axis.y;
    let since_attack = consumed + back * max(shape.x - consumed, 0.0);
    let t = shape.z;
    let secs = since_attack + shape.w * t;

    let mode = technique.x;
    let hz = technique.y;
    let lean = technique.z;
    let is_bend = f32(mode > 0.5 && mode < 1.5);
    let is_vib = clamp(f32(mode > 1.5 && mode < 2.5) + technique.w, 0.0, 1.0);
    let is_wah = f32(mode > 2.5 && mode < 3.5);
    let is_up = f32(mode > 3.5);

    let wave = sin(secs * hz * TAU);
    let shift = (is_bend + is_up) * lean * ease(since_attack / BEND_RAMP_SECS);
    let centre = 0.5 + shift + is_vib * 0.16 * wave;
    let open = 0.5 + 0.5 * cos(secs * hz * TAU);
    let width = mix(1.0, 0.35 + 0.65 * open, is_wah);

    let aa = fwidth(across) + 0.002;
    let body_half = 0.5 * width;
    let body = 1.0 - smoothstep(body_half - aa, body_half + aa, abs(across - 0.5));
    let core_half = mix(0.22, 0.07, f32(mode > 0.5));
    let core = 1.0 - smoothstep(core_half - aa, core_half + aa, abs(across - centre));

    let cap = f32(d_px < shape.y);
    let edge = f32(d_px < EDGE_PX);

    // Hold state, once hit (hold is all zero before): gold while the
    // expected pitch is sounding, paler for hold lost so far, dim grey the
    // moment it drops; a confirmed sustained technique shimmers the gold
    // and one not heard yet flattens it. Arithmetic masks, not branches:
    // `fwidth` above needs uniform control flow.
    let held = f32(hold.x > 0.5);
    let hit = f32(hold.w > 0.5);
    let confirmed = f32(hold.z > 0.5);
    let unheard = f32(hold.z < -0.5);
    let shimmer = 0.5 + 0.5 * sin(since_attack * 12.0 * TAU - t * 9.0);
    let gold = vec3<f32>(1.0, 0.86, 0.35);
    let gold_lift = (0.45 + 0.35 * hold.y) + 0.2 * shimmer * confirmed - 0.3 * unheard;
    let held_rgb = mix(color.rgb, gold, clamp(gold_lift, 0.0, 1.0));
    let dropped_rgb = mix(color.rgb, vec3<f32>(0.42, 0.42, 0.48), 0.75);
    let base_rgb = mix(color.rgb, mix(dropped_rgb, held_rgb, held), hit);

    let core_rgb = mix(base_rgb, vec3<f32>(1.0, 1.0, 1.0), 0.45);
    let cap_rgb = mix(mix(base_rgb, vec3<f32>(1.0, 1.0, 1.0), 0.55), gold, hit);

    var rgb = mix(base_rgb * 0.8, core_rgb, core);
    rgb = mix(rgb, cap_rgb, cap);
    rgb = mix(rgb, vec3<f32>(1.0, 1.0, 1.0), edge);

    let body_alpha = mix(0.55, 0.95, core);
    let hold_alpha = mix(1.0, mix(0.5, 1.0, held), hit);
    let alpha = max(body * body_alpha, cap) * hold_alpha;

    return vec4<f32>(rgb, alpha * color.a);
}
