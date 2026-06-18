// SPDX-License-Identifier: MIT

// Button hover state: smoke picks up speed and brightness, like a fire
// catching a breeze — clearly visible but not overwhelming.
// The smoke is white; the button base shifts to a slightly brighter blue-grey.
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
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i + vec2<f32>(0.0, 0.0)), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

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

fn smoke(uv: vec2<f32>, t: f32) -> f32 {
    // Faster drift — smoke reacts to the hover.
    let drifted = uv + vec2<f32>(0.0, -t * 0.30);

    // Add a slow horizontal sway to make the cursor feel like a heat source.
    let sway = vec2<f32>(sin(t * 0.4) * 0.05, 0.0);

    let q = vec2<f32>(
        fbm(drifted + sway + vec2<f32>(0.0,  0.0)),
        fbm(drifted + sway + vec2<f32>(5.2,  1.3)),
    );
    let r = vec2<f32>(
        fbm(drifted + 1.2 * q + vec2<f32>(1.7, 9.2)),
        fbm(drifted + 1.2 * q + vec2<f32>(8.3, 2.8)),
    );

    return fbm(drifted + 1.2 * r);
}

// ── Fragment ──────────────────────────────────────────────────────────────────

const BG: vec3<f32>    = vec3<f32>(0.14, 0.14, 0.22); // matches btn_default
const SMOKE: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let d = smoke(uv * vec2<f32>(1.8, 2.5), time);

    // Wider wisp band: more smoke mass visible than idle.
    let wisp = smoothstep(0.40, 0.62, d);

    // Smoke reaches higher than idle — slightly shallower vertical curve.
    let vert = pow(uv.y, 0.85);

    let edge = smoothstep(0.0, 0.08, uv.x) * smoothstep(1.0, 0.92, uv.x);

    // A gentle inner glow near the bottom-centre, as if the cursor is a heat source.
    let cx    = 0.5 - uv.x;
    let glow  = exp(-(cx * cx * 18.0 + (1.0 - uv.y) * (1.0 - uv.y) * 6.0)) * 0.18;

    let intensity = (wisp * vert * edge + glow) * 0.65;

    return vec4<f32>(mix(BG, SMOKE, clamp(intensity, 0.0, 1.0)), 1.0);
}
