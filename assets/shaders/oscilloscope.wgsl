// SPDX-License-Identifier: MIT

// Oscilloscope trace: a continuous glowing line plotting the audio waveform.
// The waveform (128 samples in -1..1) is packed into a uniform array of vec4.
// For each pixel we light it by distance to the line segment spanning the two
// samples that bracket its x, with a soft glow falloff — like a CRT scope.

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> wave: array<vec4<f32>, 32>; // 128 samples, packed

const N: f32 = 128.0;
const DEFLECT: f32 = 0.18; // fraction of half-height a full-scale sample reaches
const CORE_PX: f32 = 1.5; // bright core half-thickness, pixels
const GLOW_PX: f32 = 16.0; // glow falloff radius, pixels

fn sample(i: u32) -> f32 {
    return wave[i >> 2u][i & 3u];
}

// y position (uv space, 0=top) of waveform value v.
fn y_of(v: f32) -> f32 {
    return 0.5 - v * 0.5 * DEFLECT;
}

// Distance from p to segment a-b.
fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = in.size; // node size in pixels
    let px = in.uv * size;

    // The two waveform samples bracketing this x.
    let fx = clamp(in.uv.x * (N - 1.0), 0.0, N - 1.0);
    let i0 = u32(floor(fx));
    let i1 = min(i0 + 1u, u32(N) - 1u);

    let ax = (f32(i0) / (N - 1.0)) * size.x;
    let bx = (f32(i1) / (N - 1.0)) * size.x;
    let a = vec2<f32>(ax, y_of(sample(i0)) * size.y);
    let b = vec2<f32>(bx, y_of(sample(i1)) * size.y);

    let d = seg_dist(px, a, b);
    let core = smoothstep(CORE_PX, 0.0, d);
    let glow = exp(-(d * d) / (GLOW_PX * GLOW_PX));
    let alpha = clamp(core + glow * 0.8, 0.0, 1.0);

    // Brighten the core toward white so it reads like a hot CRT line.
    let rgb = mix(color.rgb, vec3<f32>(1.0), core * 0.7);
    return vec4<f32>(rgb, alpha);
}