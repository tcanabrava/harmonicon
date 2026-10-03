// SPDX-License-Identifier: MIT

//! Deterministic generated-band microtiming and dynamics.

use super::{Genre, SAMPLE_RATE};

/// Maximum generated-band layback. Variation stays inside its eighth-note
/// slot, so it can loosen repeated attacks without moving a bar boundary.
const MAX_HUMANIZE_SECS: f32 = 0.006;

#[derive(Clone, Copy)]
pub(super) enum GrooveRole {
    Bass = 0,
    Drums = 1,
    Comping = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PerformanceVariation {
    delay_secs: f32,
    gain: f32,
}

/// A tiny deterministic performance variation for one event. Quarter-note
/// downbeats remain anchored; the other attacks can sit up to 6 ms behind
/// the grid and vary by at most 6% in level. The role is part of the seed so
/// the three players do not move as one machine-like block.
pub(super) fn performance_variation(
    seed: u64,
    genre: Genre,
    role: GrooveRole,
    chorus: usize,
    bar: usize,
    slot: usize,
) -> PerformanceVariation {
    let mut hash = (seed as u32 ^ (seed >> 32) as u32)
        .wrapping_add(genre as u32 + 1)
        .wrapping_mul(0x9e37_79b9);
    for value in [role as u32, chorus as u32, bar as u32, slot as u32] {
        hash ^= value.wrapping_add(0x9e37_79b9).wrapping_add(hash << 6).wrapping_add(hash >> 2);
    }
    let unit = |bits: u32| bits as f32 / u16::MAX as f32;
    let delay_secs =
        if slot.is_multiple_of(2) { 0.0 } else { unit(hash & 0xffff) * MAX_HUMANIZE_SECS };
    let gain = 0.94 + unit(hash >> 16) * 0.12;
    PerformanceVariation { delay_secs, gain }
}

pub(super) fn varied_slot(
    mut sound: Vec<f32>,
    slot_secs: f32,
    variation: PerformanceVariation,
) -> Vec<f32> {
    let total = (slot_secs * SAMPLE_RATE as f32).max(1.0) as usize;
    let delay = ((variation.delay_secs * SAMPLE_RATE as f32) as usize).min(total);
    sound.truncate(total.saturating_sub(delay));
    let len = sound.len();
    sound.resize(total, 0.0);
    sound.copy_within(0..len, delay);
    sound[..delay].fill(0.0);
    for sample in &mut sound[delay..delay + len] {
        *sample *= variation.gain;
    }
    sound
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variation_is_deterministic_bounded_and_role_specific() {
        let bass = performance_variation(17, Genre::Blues, GrooveRole::Bass, 2, 6, 3);
        assert_eq!(bass, performance_variation(17, Genre::Blues, GrooveRole::Bass, 2, 6, 3));
        assert!((0.0..=MAX_HUMANIZE_SECS).contains(&bass.delay_secs));
        assert!((0.94..=1.06).contains(&bass.gain));
        assert_ne!(bass, performance_variation(17, Genre::Blues, GrooveRole::Drums, 2, 6, 3));
        assert_eq!(
            performance_variation(17, Genre::Blues, GrooveRole::Comping, 2, 6, 2).delay_secs,
            0.0
        );
        assert_ne!(bass, performance_variation(18, Genre::Blues, GrooveRole::Bass, 2, 6, 3));
    }

    #[test]
    fn varied_slot_delays_sound_without_changing_slot_length() {
        let variation = PerformanceVariation { delay_secs: 0.003, gain: 0.5 };
        let slot = varied_slot(vec![1.0; 100], 0.01, variation);
        assert_eq!(slot.len(), (0.01 * SAMPLE_RATE as f32) as usize);
        let delay = (variation.delay_secs * SAMPLE_RATE as f32) as usize;
        assert!(slot[..delay].iter().all(|sample| *sample == 0.0));
        assert_eq!(slot[delay], 0.5);
    }
}
