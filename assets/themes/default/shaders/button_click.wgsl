// SPDX-License-Identifier: MIT

// Button click (pressed) state: a burst of bright, turbulent smoke — like
// pressing a button on a smoke machine. The instant of contact produces a
// hot white core that fades outward into fast-churning wisps.
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
    // Fast upward rush — the button is actively pressed.
    let drifted = uv + vec2<f32>(0.0, -t * 0.65);

    // Strong horizontal turbulence: the burst expands outward.
    let turb = vec2<f32>(sin(t * 1.2 + uv.y * 4.0) * 0.08, 0.0);

    let q = vec2<f32>(
        fbm(drifted + turb + vec2<f32>(0.0, 0.0)),
        fbm(drifted + turb + vec2<f32>(5.2, 1.3)),
    );
    let r = vec2<f32>(
        fbm(drifted + 1.5 * q + vec2<f32>(1.7, 9.2)),
        fbm(drifted + 1.5 * q + vec2<f32>(8.3, 2.8)),
    );

    return fbm(drifted + 1.5 * r);
}

// ── Fragment ──────────────────────────────────────────────────────────────────

const BG: vec3<f32>    = vec3<f32>(0.20, 0.20, 0.34); // brighter than hover
const SMOKE: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let d = smoke(uv * vec2<f32>(2.2, 3.0), time);

    // Wide wisp band — lots of smoke mass during a click.
    let wisp = smoothstep(0.36, 0.64, d);

    // Click smoke fills the whole button, not just the bottom half.
    let vert = pow(uv.y, 0.5);

    let edge = smoothstep(0.0, 0.06, uv.x) * smoothstep(1.0, 0.94, uv.x);

    // Hot core: a bright oval centred slightly below mid-button, pulsing fast.
    let cx    = 0.5 - uv.x;
    let cy    = 0.6 - uv.y; // shifted down (uv.y=1 is bottom)
    let pulse = 0.5 + 0.5 * sin(time * 8.0);
    let core  = exp(-(cx * cx * 24.0 + cy * cy * 12.0)) * (0.55 + 0.25 * pulse);

    // A fast shimmer band sweeping up the button on each press cycle.
    let sweep_y  = fract(uv.y - time * 1.8);
    let shimmer  = smoothstep(0.0, 0.08, sweep_y) * smoothstep(0.16, 0.08, sweep_y) * 0.22;

    let intensity = clamp(wisp * vert * edge * 0.90 + core + shimmer, 0.0, 1.0);

    // Slightly warm the smoke towards a blue-white during the click.
    let smoke_tint = vec3<f32>(0.90, 0.93, 1.0);
    return vec4<f32>(mix(BG, smoke_tint, intensity), 1.0);
}
