// SPDX-License-Identifier: MIT

//! Production-path mixes for the generated-band listening review.

use harmonicon_core::harmonica::Progression;

use super::{BandEnergy, Genre, SAMPLE_RATE, generate_backing_stems};

/// Renders the first chorus of the production backing as one mono mix for
/// the repository's listening-review example. It deliberately applies no
/// normalization: the exported file must expose the same role balance and
/// headroom a player hears in the app.
pub fn generate_listening_preview(
    key: &str,
    bpm: f32,
    progression: Progression,
    genre: Genre,
    energy: BandEnergy,
    seed: u64,
) -> Vec<f32> {
    let stems = generate_backing_stems(key, bpm, progression, genre, energy, seed);
    let chorus_samples = (12.0 * 4.0 * 60.0 / bpm.max(1.0) * SAMPLE_RATE as f32) as usize;
    let len = chorus_samples.min(stems[0].1.len());
    (0..len).map(|sample| stems.iter().map(|(_, pcm)| pcm[sample]).sum()).collect()
}
