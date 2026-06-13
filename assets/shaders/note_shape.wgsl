// Note-body shape for technique notes, drawn as a UI material so the silhouette
// is smooth and antialiased. The filled band has straight horizontal top/bottom
// edges; its centerline is displaced horizontally down the note's length by:
//   * a sine    -> vibrato (oscillating pitch)
//   * an arc    -> bend    (pitch sliding off in one direction)
// A note may carry both at once.

#import bevy_ui::ui_vertex_output::UiVertexOutput

// params: x = vibrato amplitude (fraction of width)
//         y = vibrato cycles down the note
//         z = body half-width (fraction of width)
//         w = bend amplitude (fraction of width; signed direction baked in)
// wah:    x = wah depth (0 = none .. ~0.7 = pinches nearly shut)
//         y = wah cycles (open/close pulses down the note)
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> params: vec4<f32>;
@group(1) @binding(2) var<uniform> wah: vec4<f32>;

const TAU: f32 = 6.2831853;

// Sigmoid bend profile: holds flat, transitions sharply through the middle, then
// holds flat again — a note bending from one pitch to another. Steepened
// smootherstep (transition compressed into the central band).
fn bend_ease(t: f32) -> f32 {
    let x = clamp((t - 0.5) * 2.0 + 0.5, 0.0, 1.0);
    return x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let vib_amp = params.x;
    let cycles = params.y;
    let body_half = params.z;
    let bend_amp = params.w;

    // Vibrato oscillates; the bend is a one-way S-curve down the note length.
    let vibrato = vib_amp * sin(uv.y * cycles * TAU);
    let bend = bend_amp * bend_ease(uv.y);
    let center = 0.5 + vibrato + bend;

    // Wah-wah: the body breathes — its width pinches and swells down the note,
    // like the hands cupping and releasing. Starts open (cos=1 at uv.y=0).
    let wah_depth = wah.x;
    let wah_cycles = wah.y;
    let breath = 1.0 - wah_depth * (0.5 - 0.5 * cos(uv.y * wah_cycles * TAU));
    let half = body_half * breath;

    let dist = abs(uv.x - center);
    let aa = fwidth(dist) + 0.0015;
    let fill = 1.0 - smoothstep(half - aa, half + aa, dist);

    return vec4<f32>(color.rgb, color.a * fill);
}
