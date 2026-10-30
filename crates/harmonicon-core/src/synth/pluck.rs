// SPDX-License-Identifier: MIT

//! A plucked string, by Karplus–Strong: a delay line one period long, filled
//! with a burst of noise, fed back through a two-point average. Each pass
//! round the loop the average takes a little off the top of the spectrum, so
//! the burst settles into a pitched, decaying tone the way a struck string
//! does, bright at the attack and mellowing as it rings.
//!
//! **Tuning needs a fractional delay.** The loop's period is its integer
//! length plus the average's half sample, so a whole-sample line can only
//! land on pitches whose period is a whole number plus a half — high up the
//! neck that can be tens of cents out. A first-order all-pass in the loop
//! supplies the rest of the fraction (Jaffe and Smith's tuning), keeping
//! every note within a few cents.
//!
//! Expression (vibrato, wah) isn't modelled: a string's vibrato bends its
//! length, which a fixed delay line can't do.

use super::SAMPLE_RATE;

/// Loss per trip round the loop, on top of the average's own: low strings
/// ring for seconds, high ones fade fast, as a guitar's do.
const LOOP_GAIN: f32 = 0.996;

/// How long the string takes to stop once the note ends — a fretting finger
/// lifting, not an instant cut.
const RELEASE_SECS: f32 = 0.06;

/// Adds one plucked note to `buf`: `freq` Hz, from sample `start` for `dur`
/// samples plus its release, at `level`. `seed` makes the noise burst, and so
/// the note, deterministic.
pub(super) fn add_pluck(
    buf: &mut [f32],
    start: usize,
    dur: usize,
    freq: f32,
    seed: u32,
    level: f32,
) {
    if freq <= 0.0 {
        return;
    }
    let period = SAMPLE_RATE as f32 / freq;
    // Integer length plus the average's half sample plus the all-pass's
    // fraction makes the period. The fraction is kept in 0.1..1.1, where the
    // all-pass stays well-behaved, by taking the integer part low.
    let length = ((period - 0.6).floor() as usize).max(2);
    let fraction = period - 0.5 - length as f32;
    let coefficient = (1.0 - fraction) / (1.0 + fraction);

    let mut rng = seed;
    let mut noise = || {
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (rng as i32) as f32 / i32::MAX as f32
    };
    let mut line: Vec<f32> = (0..length).map(|_| noise()).collect();
    // A burst with a DC offset would ring as a slowly-dying shift of the
    // whole waveform rather than as pitch.
    let mean = line.iter().sum::<f32>() / length as f32;
    for x in &mut line {
        *x -= mean;
    }

    let release = (SAMPLE_RATE as f32 * RELEASE_SECS) as usize;
    let (mut index, mut last, mut all_pass_in, mut all_pass_out) = (0, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..dur + release {
        let Some(slot) = buf.get_mut(start + i) else {
            break;
        };
        let oldest = line[index];
        let averaged = 0.5 * (oldest + last) * LOOP_GAIN;
        last = oldest;
        let out = coefficient * averaged + all_pass_in - coefficient * all_pass_out;
        all_pass_in = averaged;
        all_pass_out = out;
        line[index] = out;
        index = (index + 1) % length;

        let env = if i < dur { 1.0 } else { 1.0 - (i - dur) as f32 / release as f32 };
        *slot += level * env * out;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Expr, PhraseNote, Voice, render_pcm_with};
    use super::*;

    /// One `secs`-long plucked note at `freq`, on a millisecond tick grid.
    fn pluck(freq: f32, secs: f32) -> Vec<f32> {
        let len = (secs * 1000.0) as usize;
        let note = PhraseNote { tick: 0, len, freq: Some(freq), expr: Expr::None };
        render_pcm_with(&[note], 0.001, Voice::PluckedString)
    }

    /// The period in samples where `samples` best matches itself, near the
    /// period `expected` implies, refined between lags by a parabola.
    fn measured_hz(samples: &[f32], expected: f32) -> f32 {
        let period = SAMPLE_RATE as f32 / expected;
        let correlation =
            |lag: usize| -> f32 { samples.iter().zip(&samples[lag..]).map(|(a, b)| a * b).sum() };
        let low = (period * 0.9) as usize;
        let high = (period * 1.1).ceil() as usize;
        let best = (low..=high).max_by(|&a, &b| correlation(a).total_cmp(&correlation(b))).unwrap();
        let (l, c, r) = (correlation(best - 1), correlation(best), correlation(best + 1));
        let shift = 0.5 * (l - r) / (l - 2.0 * c + r);
        SAMPLE_RATE as f32 / (best as f32 + shift)
    }

    fn cents(measured: f32, wanted: f32) -> f32 {
        1200.0 * (measured / wanted).log2()
    }

    /// Across a guitar's range, open low E to the high E's twelfth fret,
    /// every note lands within a few cents. A whole-sample delay line alone
    /// can be up to half a sample out, about 26 cents at 1.3 kHz.
    #[test]
    fn every_pitch_on_the_neck_is_in_tune() {
        for wanted in [82.41, 110.0, 196.0, 329.63, 440.0, 659.26, 1318.51] {
            let pcm = pluck(wanted, 0.6);
            // Skip the attack, where the burst hasn't settled into a tone.
            let settled = &pcm[2_000..20_000];
            let off = cents(measured_hz(settled, wanted), wanted);
            assert!(off.abs() < 5.0, "{wanted} Hz is {off:.1} cents out");
        }
    }

    #[test]
    fn a_plucked_note_rings_down() {
        let pcm = pluck(196.0, 1.5);
        let rms = |from: usize| {
            let window = &pcm[from..from + 4_410];
            (window.iter().map(|x| x * x).sum::<f32>() / window.len() as f32).sqrt()
        };
        assert!(rms(2_000) > rms(30_000) * 1.5, "louder at the start than a second later");
        assert!(rms(30_000) > 0.0, "still ringing");
    }

    #[test]
    fn the_string_stops_soon_after_the_note_ends() {
        let pcm = pluck(196.0, 0.5);
        let after = (0.5 + RELEASE_SECS) * SAMPLE_RATE as f32;
        assert!(pcm[after as usize + 10..].iter().all(|x| *x == 0.0));
    }

    #[test]
    fn the_same_note_always_sounds_the_same() {
        assert_eq!(pluck(330.0, 0.2), pluck(330.0, 0.2));
    }

    #[test]
    fn the_harmonica_voice_is_the_default() {
        let note = PhraseNote { tick: 0, len: 100, freq: Some(440.0), expr: Expr::None };
        assert_eq!(
            super::super::render_pcm(&[note], 0.001),
            render_pcm_with(&[note], 0.001, Voice::default())
        );
        assert_ne!(
            render_pcm_with(&[note], 0.001, Voice::Harmonica),
            render_pcm_with(&[note], 0.001, Voice::PluckedString)
        );
    }
}
