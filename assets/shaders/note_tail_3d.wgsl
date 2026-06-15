// SPDX-License-Identifier: MIT

// 3D comet tail — the mesh-material twin of `note_shape.wgsl`. Drawn on a flat
// ribbon trailing behind the cube head; the alpha carves the tapering, animated
// tail shape onto the quad, so the technique animations match the 2D notes.
//
// `wah.z` selects the per-technique animation and `params.z` is the live gameplay
// clock. uv.y = 0 is the tip, uv.y = 1 is the base by the head.

#import bevy_pbr::forward_io::VertexOutput

// params: x = vibrato amplitude, y = vibrato cycles, z = animation time (seconds),
//         w = bend amplitude (signed)
// wah:    x = wah depth, y = wah cycles, z = animation mode, w = per-note phase
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> wah: vec4<f32>;

const TAU: f32 = 6.2831853;
const PI: f32 = 3.1415927;

fn bend_ease(t: f32) -> f32 {
    let x = clamp(t, 0.0, 1.0);
    return x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
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

    var center_off = 0.0;
    var width_mul = 1.0;
    var pulse = 1.0;

    if (mode < 0.5) {
        let band = 0.5 + 0.5 * sin(s * 2.0 * TAU - t * 1.6 + phase);
        pulse = 0.75 + 0.25 * band;
    } else if (mode < 1.5) {
        let dir = sign(bend_amp + 0.0001);
        let lean = (abs(bend_amp) + 0.14 + 0.10 * sin(t * 2.5 + phase)) * dir;
        center_off = lean * bend_ease(s) + 0.05 * sin(s * 2.0 * TAU - t * 4.0 + phase) * s;
        pulse = 0.45 + 0.75 * (0.5 + 0.5 * sin(s * 4.0 * TAU - t * 4.0 + phase));
    } else if (mode < 2.5) {
        let amp = max(vib_amp, 0.12);
        let cyc = max(vib_cycles, 3.0);
        center_off = amp * sin(s * cyc * TAU - t * 8.0 + phase) * s;
        pulse = 0.60 + 0.50 * (0.5 + 0.5 * sin(s * 4.0 * TAU - t * 6.0 + phase));
    } else if (mode < 3.5) {
        let cyc = max(wah_cycles, 4.0);
        let nodes = abs(sin((s * cyc - t * 1.5 + phase) * PI));
        width_mul = 0.08 + 0.92 * nodes;
        pulse = 0.50 + 0.60 * nodes;
    } else if (mode < 4.5) {
        let breath = 0.5 + 0.5 * sin(t * 2.4 + phase);
        width_mul = 0.55 + 0.45 * breath;
        pulse = 0.30 + 1.00 * breath;
    } else if (mode < 5.5) {
        let flare = pow(0.5 + 0.5 * sin(s * 5.0 * TAU - t * 13.0 + phase), 3.0);
        center_off = bend_amp * bend_ease(s);
        width_mul = 0.50 + 0.50 * flare;
        pulse = 0.25 + 1.30 * flare;
    } else {
        let flare = pow(0.5 + 0.5 * sin(s * 4.0 * TAU + t * 10.0 + phase), 3.0);
        center_off = bend_amp * bend_ease(s) + 0.13 * sin(s * 3.0 * TAU + t * 7.0 + phase) * s;
        width_mul = 0.55 + 0.45 * flare;
        pulse = 0.30 + 1.20 * flare;
    }

    let center = 0.5 + center_off;
    let half = 0.5 * pow(1.0 - s, 1.3) * width_mul;

    let dist = abs(uv.x - center);
    let aa = fwidth(dist) + 0.0015;
    let body = 1.0 - smoothstep(half - aa, half + aa, dist);
    let side_glow = exp(-max(dist - half, 0.0) * 14.0) * 0.4;
    let fade = pow(1.0 - s, 0.6);

    let alpha = clamp(max(body, side_glow) * fade * pulse, 0.0, 1.0);
    let hot = body * (1.0 - s) * 0.4;
    let rgb = mix(color.rgb, vec3<f32>(1.0, 1.0, 1.0), hot);

    return vec4<f32>(rgb, alpha * color.a);
}
