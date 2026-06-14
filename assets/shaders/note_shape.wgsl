// SPDX-License-Identifier: MIT

// Comet tail for a note, drawn as a UI material. The round head is a separate
// image (`assets/notes/circular.png`) layered on top of this tail's base, so
// this shader only has to draw the trailing tail: full width where it meets the
// head, tapering to a point at the tip, fading out as it goes. The tail carries
// the technique displacement so a note still reads as bent / vibrato / wah:
//   * sine  -> vibrato (tail wiggles)
//   * arc   -> bend    (tail curves to one side)
//   * pulse -> wah     (tail width breathes)
//
// The tail node fills the whole note; its lower (head-sized) square is hidden
// behind the head image, which also hides the seam. uv.y = 0 is the tip (top),
// uv.y = 1 is the base by the head (bottom).

#import bevy_ui::ui_vertex_output::UiVertexOutput

// params: x = vibrato amplitude (fraction of width)
//         y = vibrato cycles down the tail
//         z = unused (legacy body half-width)
//         w = bend amplitude (fraction of width; signed direction baked in)
// wah:    x = wah depth (0 = none .. ~0.7 = pinches nearly shut)
//         y = wah cycles (open/close pulses down the tail)
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> params: vec4<f32>;
@group(1) @binding(2) var<uniform> wah: vec4<f32>;

const TAU: f32 = 6.2831853;

// Smootherstep: holds flat near the head, slides through, settles at the tip —
// a note bending from one pitch to another along the tail.
fn bend_ease(t: f32) -> f32 {
    let x = clamp(t, 0.0, 1.0);
    return x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let s = clamp(1.0 - uv.y, 0.0, 1.0); // 0 at the head (base), 1 at the tip

    let vibrato = params.x * sin(s * params.y * TAU) * s; // grows away from head
    let bend = params.w * bend_ease(s);
    let center = 0.5 + vibrato + bend;

    let breath = 1.0 - wah.x * (0.5 - 0.5 * cos(s * wah.y * TAU));
    let half = 0.5 * pow(1.0 - s, 1.3) * breath; // full at head, points at tip

    let dist = abs(uv.x - center);
    let aa = fwidth(dist) + 0.0015;
    let body = 1.0 - smoothstep(half - aa, half + aa, dist);
    let side_glow = exp(-max(dist - half, 0.0) * 14.0) * 0.4;
    let fade = pow(1.0 - s, 0.6); // dim toward the tip

    let alpha = clamp(max(body, side_glow) * fade, 0.0, 1.0);
    let hot = body * (1.0 - s) * 0.4; // brighten the core near the head
    let rgb = mix(color.rgb, vec3<f32>(1.0, 1.0, 1.0), hot);

    return vec4<f32>(rgb, alpha * color.a);
}
