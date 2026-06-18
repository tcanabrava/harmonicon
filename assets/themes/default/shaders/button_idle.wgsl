// SPDX-License-Identifier: MIT

// Button idle state: slow, lazy smoke wisps drifting upward from the bottom.
// The smoke is white; the button base is very dark blue-grey.
// Time uniform is expected from the theme material each frame.

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> time: f32;

// ── Noise ─────────────────────────────────────────────────────────────────────

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(127.1, 311.7));
    q += dot(q, q.yx + vec2<f32>(19.19, 19.19));
    return fract((q.x + q.y) * q.x);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep
    return mix(
        mix(hash(i + vec2<f32>(0.0, 0.0)), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

// 5-octave FBM.
fn fbm(p: vec2<f32>) -> f32 {
    var v   = 0.0;
    var amp = 0.5;
    var pp  = p;
    for (var i: i32 = 0; i < 5; i++) {
        v   += amp * vnoise(pp);
        amp *= 0.5;
        pp   = pp * 2.1 + vec2<f32>(0.37, 0.71);
    }
    return v;
}

// Domain-warped FBM: two layers of warping give organic, billowing shapes.
fn smoke(uv: vec2<f32>, t: f32) -> f32 {
    // Slow upward drift (uv.y=0 is top in Bevy UI, so subtract to move up).
    let drifted = uv + vec2<f32>(0.0, -t * 0.12);

    // First warp layer.
    let q = vec2<f32>(
        fbm(drifted + vec2<f32>(0.0,  0.0)),
        fbm(drifted + vec2<f32>(5.2,  1.3)),
    );

    // Second warp layer using the first.
    let r = vec2<f32>(
        fbm(drifted + 1.0 * q + vec2<f32>(1.7, 9.2)),
        fbm(drifted + 1.0 * q + vec2<f32>(8.3, 2.8)),
    );

    return fbm(drifted + 1.0 * r);
}

// ── Fragment ──────────────────────────────────────────────────────────────────

const BG: vec3<f32>    = vec3<f32>(0.08, 0.08, 0.14);
const SMOKE: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Sample smoke density at 1.5× zoom so wisps are wider than the button.
    let d = smoke(uv * vec2<f32>(1.4, 2.2), time);

    // Wisp threshold: extract thin bands, not a flat fog.
    let wisp = smoothstep(0.44, 0.60, d);

    // Vertical gradient: smoke is born at the bottom (uv.y=1), fades upward.
    let vert = pow(uv.y, 1.2);

    // Soft horizontal edge so smoke doesn't clip hard at button borders.
    let edge = smoothstep(0.0, 0.10, uv.x) * smoothstep(1.0, 0.90, uv.x);

    // Clamp max opacity low — idle should be barely noticeable.
    let intensity = wisp * vert * edge * 0.30;

    return vec4<f32>(mix(BG, SMOKE, intensity), 1.0);
}
