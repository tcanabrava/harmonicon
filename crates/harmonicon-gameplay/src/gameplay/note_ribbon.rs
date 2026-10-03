// SPDX-License-Identifier: MIT

//! What a note ribbon draws for a note's techniques — shared by the 2D and
//! 3D ribbons, so one note reads the same in both modes.

use bevy::math::Vec4;
use harmonicon_core::chart::Modifier;

/// The ribbon shaders' `technique` uniform for a note's modifiers:
///
/// - `x` — mode: 0 plain, 1 bend, 2 vibrato, 3 wah, 4 pitch-up shift
///   (overblow, overdraw, slide).
/// - `y` — the charted oscillation rate in Hz (vibrato/wah), 0 otherwise.
///   The shaders space crests by it along the note, so they cross the hit
///   line at exactly this rate — the wobble to copy.
/// - `z` — how far the core leans across the lane, signed: a deeper bend
///   leans further, down-bends one way and pitch-up shifts the other.
/// - `w` — 1 when a vibrato rides on a bend or shift, which keeps the bend
///   as the mode and draws the vibrato on top.
pub fn ribbon_technique(modifiers: &[Modifier]) -> Vec4 {
    let bend = modifiers.iter().find_map(|m| match m {
        Modifier::Bend { semitones, .. } => Some(*semitones),
        _ => None,
    });
    let up = modifiers
        .iter()
        .any(|m| matches!(m, Modifier::Overblow | Modifier::Overdraw | Modifier::Slide));
    let wobble = modifiers.iter().find_map(|m| match m {
        Modifier::Vibrato { oscillation_hz, .. } => Some((false, *oscillation_hz)),
        Modifier::WahWah { oscillation_hz, .. } => Some((true, *oscillation_hz)),
        _ => None,
    });

    let (mut mode, mut lean) = (0.0, 0.0);
    if let Some(semitones) = bend {
        mode = 1.0;
        let depth = semitones.abs().clamp(1.0, 3.0);
        lean = (0.12 + 0.08 * depth).copysign(semitones);
    } else if up {
        mode = 4.0;
        lean = 0.2;
    }
    let (mut hz, mut vibrato_on_shift) = (0.0, 0.0);
    if let Some((is_wah, rate)) = wobble {
        hz = rate;
        if mode == 0.0 {
            mode = if is_wah { 3.0 } else { 2.0 };
        } else if !is_wah {
            vibrato_on_shift = 1.0;
        }
    }
    Vec4::new(mode, hz, lean, vibrato_on_shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bend(semitones: f32) -> Modifier {
        Modifier::Bend { semitones, intensity: None }
    }

    #[test]
    fn a_wobble_carries_its_charted_rate() {
        let vibrato =
            ribbon_technique(&[Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }]);
        assert_eq!((vibrato.x, vibrato.y), (2.0, 5.0));
        let wah = ribbon_technique(&[Modifier::WahWah { oscillation_hz: 3.0, intensity: None }]);
        assert_eq!((wah.x, wah.y), (3.0, 3.0));
    }

    #[test]
    fn a_deeper_bend_leans_further_and_pitch_up_leans_the_other_way() {
        let (half, whole) = (ribbon_technique(&[bend(-1.0)]), ribbon_technique(&[bend(-2.0)]));
        assert_eq!(half.x, 1.0);
        assert!(half.z < 0.0 && whole.z < half.z);
        let up = ribbon_technique(&[Modifier::Overblow]);
        assert_eq!(up.x, 4.0);
        assert!(up.z > 0.0);
        assert_eq!(ribbon_technique(&[]), Vec4::ZERO);
    }

    #[test]
    fn a_vibrato_on_a_bend_keeps_the_bend_and_rides_on_it() {
        let t = ribbon_technique(&[
            Modifier::Vibrato { oscillation_hz: 5.0, intensity: None },
            bend(-1.0),
        ]);
        assert_eq!((t.x, t.w), (1.0, 1.0));
    }
}
