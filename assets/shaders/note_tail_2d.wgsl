// SPDX-License-Identifier: MIT

// Comet tail for a note, drawn as a UI material. The round head is a separate
// image; this shader draws the trailing tail: full width where it meets the head,
// tapering to a point at the tip, fading as it goes.
//
// The tail is *animated*, and the animation differs per harmonica technique so
// each note reads at a glance and the highway feels alive. `wah.z` selects the
// mode and `params.z` is a live clock (the gameplay time, so motion flows with
// the song and freezes on pause):
//   0 none     – gentle glow bands drifting off the head (sustained breath)
//   1 bend     – the pitch arc, leaning and wavering as it slides
//   2 vibrato  – a wiggle travelling down the tail
//   3 wah-wah  – the width breathing open and closed
//   4 overblow – fast, bright flares shooting up the tail (high energy)
//   5 overdraw – energetic flares with a twist, travelling the other way
//
// uv.y = 0 is the tip (top); uv.y = 1 is the base by the head (bottom).

#import bevy_ui::ui_vertex_output::UiVertexOutput

// params: x = vibrato amplitude, y = vibrato cycles, z = animation time (seconds),
//         w = bend amplitude (fraction of width; signed direction baked in)
// wah:    x = wah depth, y = wah cycles, z = animation mode, w = per-note phase
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> params: vec4<f32>;
@group(1) @binding(2) var<uniform> wah: vec4<f32>;

const TAU: f32 = 6.2831853;
const PI: f32 = 3.1415927;

// Smootherstep arc: flat near the head, slides through, settles at the tip.
fn bend_ease(t: f32) -> f32 {
    let x = clamp(t, 0.0, 1.0);
    return x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let s = clamp(1.0 - uv.y, 0.0, 1.0); // 0 at the head (base), 1 at the tip

    let vib_amp = params.x;
    let vib_cycles = params.y;
    let bend_amp = params.w;
    let wah_depth = wah.x;
    let wah_cycles = wah.y;
    let t = params.z;
    let mode = wah.z;
    let phase = wah.w;

    // Per-technique animation: a horizontal centerline offset, a width multiplier,
    // and a brightness pulse. Computed in uniform control flow (no derivatives in
    // the branches) so `fwidth` below stays valid.
    var center_off = 0.0;
    var width_mul = 1.0;
    var pulse = 1.0;

    if (mode < 0.5) {
        // none: a calm, slow glow drifting up the tail — quiet by design.
        let band = 0.5 + 0.5 * sin(s * 2.0 * TAU - t * 1.6 + phase);
        pulse = 0.75 + 0.25 * band;
    } else if (mode < 1.5) {
        // bend: a pronounced leaning arc that sways side to side over time, with a
        // wave riding along it.
        let dir = sign(bend_amp + 0.0001);
        let lean = (abs(bend_amp) + 0.14 + 0.10 * sin(t * 2.5 + phase)) * dir;
        center_off = lean * bend_ease(s) + 0.05 * sin(s * 2.0 * TAU - t * 4.0 + phase) * s;
        pulse = 0.45 + 0.75 * (0.5 + 0.5 * sin(s * 4.0 * TAU - t * 4.0 + phase));
    } else if (mode < 2.5) {
        // vibrato: a wiggle travelling down the tail.
        let amp = max(vib_amp, 0.12);
        let cyc = max(vib_cycles, 3.0);
        center_off = amp * sin(s * cyc * TAU - t * 8.0 + phase) * s;
        pulse = 0.60 + 0.50 * (0.5 + 0.5 * sin(s * 4.0 * TAU - t * 6.0 + phase));
    } else if (mode < 3.5) {
        // wah-wah: the tail becomes an audio-wave envelope — a string of spindle
        // bulges pinched to near-points at the nodes — that drifts down the note
        // as the cupped hand works. `abs(sin)` makes the pinch-to-a-point nodes.
        let cyc = max(wah_cycles, 4.0);
        let nodes = abs(sin((s * cyc - t * 1.5 + phase) * PI)); // 1 at bulges, 0 at nodes
        width_mul = 0.08 + 0.92 * nodes;
        pulse = 0.50 + 0.60 * nodes;
    } else if (mode < 4.5) {
        // overblow: fast, hard flares shooting up the tail.
        let flare = pow(0.5 + 0.5 * sin(s * 5.0 * TAU - t * 13.0 + phase), 3.0);
        center_off = bend_amp * bend_ease(s);
        width_mul = 0.50 + 0.50 * flare;
        pulse = 0.25 + 1.30 * flare;
    } else {
        // overdraw: a twisting sway plus flares travelling the other way.
        let flare = pow(0.5 + 0.5 * sin(s * 4.0 * TAU + t * 10.0 + phase), 3.0);
        center_off = bend_amp * bend_ease(s) + 0.13 * sin(s * 3.0 * TAU + t * 7.0 + phase) * s;
        width_mul = 0.55 + 0.45 * flare;
        pulse = 0.30 + 1.20 * flare;
    }

    let center = 0.5 + center_off;
    let half = 0.5 * pow(1.0 - s, 1.3) * width_mul; // full at head, point at tip

    let dist = abs(uv.x - center);
    let aa = fwidth(dist) + 0.0015;
    let body = 1.0 - smoothstep(half - aa, half + aa, dist);
    let side_glow = exp(-max(dist - half, 0.0) * 14.0) * 0.4;
    let fade = pow(1.0 - s, 0.6); // dim toward the tip

    let alpha = clamp(max(body, side_glow) * fade * pulse, 0.0, 1.0);
    let hot = body * (1.0 - s) * 0.4; // brighten the core near the head
    let rgb = mix(color.rgb, vec3<f32>(1.0, 1.0, 1.0), hot);

    return vec4<f32>(rgb, alpha * color.a);
}
