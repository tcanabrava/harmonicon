// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

/// UI material for a note's comet **tail**. The round head is a separate image
/// layered on top of the tail's base; this material draws the tapered, fading
/// trail, shaped by the note's techniques and *animated* per technique. Drawn in
/// `assets/shaders/note_tail_2d.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct NoteTail2dMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = vibrato amplitude, y = vibrato cycles, z = animation time in seconds
    /// (refreshed each frame by `animate_note_tails`), w = bend amplitude.
    /// Amplitudes are fractions of the note width.
    #[uniform(1)]
    pub params: Vec4,
    /// x = wah depth (0..~0.7), y = wah cycles, z = animation mode (which technique
    /// animation to run; see the shader), w = per-note phase offset.
    #[uniform(2)]
    pub wah: Vec4,
}

impl UiMaterial for NoteTail2dMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/note_tail_2d.wgsl".into()
    }
}

/// Builds the two shader param vectors `(params, wah)` for a shaped note from its
/// techniques. `vibrato` is the modifier intensity (0..1); `shift` is the pitch
/// shift in semitones — negative bends the note down, positive (overblow/overdraw)
/// bends it up; `wah` is the wah-wah intensity (0..1). The bend arc depth is
/// proportional to `|shift|` and its sign sets the lean direction; the body
/// half-width shrinks with the total centerline sway; wah pulses the width.
pub fn tail_params(
    h_pct: f32,
    vibrato: Option<f32>,
    shift: Option<f32>,
    wah: Option<f32>,
) -> (Vec4, Vec4) {
    let vib_amp = vibrato.map_or(0.0, |i| 0.12 + 0.14 * i.clamp(0.0, 1.0));

    // Proportional to depth (normalised against a 3-semitone max), with only a
    // small floor so even a half-step is visibly a bend.
    let shift = shift.unwrap_or(0.0);
    let bend_mag = if shift == 0.0 {
        0.0
    } else {
        0.05 + 0.27 * (shift.abs() / 3.0).clamp(0.0, 1.0)
    };

    // Cap the combined sway so a minimum solid body always remains.
    let total = vib_amp + bend_mag;
    let (vib_amp, bend_mag) = if total > 0.40 {
        let k = 0.40 / total;
        (vib_amp * k, bend_mag * k)
    } else {
        (vib_amp, bend_mag)
    };

    let cycles = (h_pct / 6.5).clamp(1.0, 6.0);
    let body_half = 0.5 - (vib_amp + bend_mag);
    // Sign the bend amplitude so the shader leans the arc the right way.
    let bend_signed = bend_mag * if shift < 0.0 { -1.0 } else { 1.0 };
    let params = Vec4::new(vib_amp, cycles, body_half, bend_signed);

    // Wah breathes the width: deeper intensity pinches harder; the pulse is
    // slower than vibrato (a rhythmic open/close, not a fast wobble).
    let wah_depth = wah.map_or(0.0, |i| 0.30 + 0.40 * i.clamp(0.0, 1.0));
    let wah_cycles = (h_pct / 9.0).clamp(1.0, 4.0);
    let wah = Vec4::new(wah_depth, wah_cycles, 0.0, 0.0);

    (params, wah)
}

pub struct NoteTail2dPlugin;

impl Plugin for NoteTail2dPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<NoteTail2dMaterial>::default());
    }
}

/// Drives every 2D note tail's animation clock (`params.z`) from the gameplay
/// clock, so the tails flow in time with the song and freeze when paused.
pub fn animate_note_tails(
    clock: Res<super::GameplayClock>,
    mut materials: ResMut<Assets<NoteTail2dMaterial>>,
) {
    let t = clock.get() as f32;
    for (_, material) in materials.iter_mut() {
        material.params.z = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_techniques_is_all_body() {
        let (p, wah) = tail_params(10.0, None, None, None);
        assert_eq!(p.x, 0.0); // vibrato amp
        assert_eq!(p.w, 0.0); // bend amp
        assert_eq!(p.z, 0.5); // full half-width body
        assert_eq!(wah.x, 0.0); // no wah pulse
    }

    #[test]
    fn bend_arc_depth_scales_with_semitones() {
        let shallow = tail_params(10.0, None, Some(-0.5), None).0;
        let deep = tail_params(10.0, None, Some(-3.0), None).0;
        assert!(deep.w.abs() > shallow.w.abs(), "deeper bend leans more");
        assert!(shallow.w.abs() > 0.0, "even a half-step is visibly a bend");
    }

    #[test]
    fn bend_direction_follows_sign() {
        // Pitch down (negative) and up (positive) lean opposite ways.
        assert!(tail_params(10.0, None, Some(-1.0), None).0.w < 0.0);
        assert!(tail_params(10.0, None, Some(1.0), None).0.w > 0.0);
    }

    #[test]
    fn body_stays_solid_when_both_techniques_present() {
        let (p, _) = tail_params(10.0, Some(1.0), Some(-3.0), None);
        // half-width never collapses below the 0.10 floor implied by the 0.40 cap.
        assert!(p.z >= 0.10 - 1e-6, "got {}", p.z);
        // |amplitudes| plus half-width fill exactly half the tile (band touches edges).
        assert!((p.x + p.w.abs() + p.z - 0.5).abs() < 1e-6);
    }

    #[test]
    fn wah_depth_scales_and_stays_open_to_pinch() {
        let (_, none) = tail_params(10.0, None, None, None);
        assert_eq!(none.x, 0.0);
        let (_, soft) = tail_params(10.0, None, None, Some(0.0));
        let (_, hard) = tail_params(10.0, None, None, Some(1.0));
        assert!(hard.x > soft.x, "more intensity pinches harder");
        assert!(hard.x < 1.0, "never fully shut");
    }
}
