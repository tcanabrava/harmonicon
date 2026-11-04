// SPDX-License-Identifier: MIT

//! A struck piano string, additively: a handful of partials, each a little
//! sharp of its harmonic (a stiff string's inharmonicity, which is much of
//! why a piano doesn't sound like an organ), each decaying on its own clock
//! with the upper ones dying first, so the note is bright at the hammer and
//! mellows as it rings. Lower notes ring longer than high ones. Lifting the
//! key drops the damper, a short release rather than a cut.
//!
//! Every partial's phase is accumulated from its own fixed frequency, the
//! rule for any oscillator here (`synth`'s vibrato note); a piano has no
//! expression to modulate it, so none is modelled.

use std::f32::consts::TAU;

use super::SAMPLE_RATE;

/// Partials summed per note.
const PARTIALS: usize = 8;
/// How stiff the string is: partial `k` sits at `k · f · √(1 + B·k²)`.
const INHARMONICITY: f32 = 0.0004;
/// The hammer's attack, quick but not a click.
const ATTACK_SECS: f32 = 0.004;
/// The damper's release once the key lifts.
const RELEASE_SECS: f32 = 0.12;
/// The fundamental's decay rate, in 1/s, at middle C; higher partials and
/// higher notes decay faster.
const BASE_DECAY: f32 = 0.9;

/// The frequency of partial `k` (from 1) of a string sounding `freq`.
pub(super) fn partial_freq(freq: f32, k: usize) -> f32 {
    let k = k as f32;
    k * freq * (1.0 + INHARMONICITY * k * k).sqrt()
}

/// How fast partial `k` of a `freq` note decays, in 1/s.
fn partial_decay(freq: f32, k: usize) -> f32 {
    BASE_DECAY * (freq / 261.63).max(0.25).sqrt() * (1.0 + 0.45 * (k as f32 - 1.0))
}

/// Adds one struck note to `buf`: `freq` Hz from sample `start`, held for
/// `dur` samples and then damped, at `level`.
pub(super) fn add_piano(buf: &mut [f32], start: usize, dur: usize, freq: f32, level: f32) {
    if freq <= 0.0 {
        return;
    }
    let rate = SAMPLE_RATE as f32;
    let attack = (ATTACK_SECS * rate) as usize;
    let release = (RELEASE_SECS * rate) as usize;
    // Partials above the Nyquist limit would alias back down as noise.
    let partials: Vec<(f32, f32, f32)> = (1..=PARTIALS)
        .map(|k| (partial_freq(freq, k), 1.0 / (k as f32).powf(1.3), partial_decay(freq, k)))
        .filter(|&(f, _, _)| f < rate * 0.45)
        .collect();
    let norm = 1.0 / partials.iter().map(|p| p.1).sum::<f32>().max(1.0);
    let mut phases = vec![0.0f32; partials.len()];
    for i in 0..dur + release {
        let s = start + i;
        if s >= buf.len() {
            break;
        }
        let t = i as f32 / rate;
        let onset = if i < attack { i as f32 / attack.max(1) as f32 } else { 1.0 };
        let damper = if i < dur { 1.0 } else { 1.0 - (i - dur) as f32 / release.max(1) as f32 };
        let mut sample = 0.0;
        for ((f, amp, decay), phase) in partials.iter().zip(&mut phases) {
            sample += amp * (-decay * t).exp() * phase.sin();
            *phase = (*phase + TAU * f / rate) % TAU;
        }
        buf[s] += level * norm * onset * damper * sample;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stiff_strings_partials_run_sharp_of_their_harmonics() {
        assert_eq!(partial_freq(100.0, 1), 100.0 * (1.0 + INHARMONICITY).sqrt());
        let eighth = partial_freq(100.0, 8);
        assert!(eighth > 800.0 && eighth < 820.0, "{eighth}");
    }

    #[test]
    fn a_note_decays_while_held_and_stops_once_damped() {
        let rate = SAMPLE_RATE as usize;
        let mut buf = vec![0.0; rate * 3];
        add_piano(&mut buf, 0, rate * 2, 220.0, 1.0);
        let level = |from: usize| buf[from..from + rate / 10].iter().map(|x| x.abs()).sum::<f32>();
        assert!(level(rate / 10) > level(rate * 3 / 2), "it rings down while held");
        let after = 2 * rate + (RELEASE_SECS * SAMPLE_RATE as f32) as usize + 10;
        assert!(buf[after..].iter().all(|&x| x == 0.0), "silent after the damper");
    }

    #[test]
    fn a_low_note_rings_longer_than_a_high_one() {
        assert!(partial_decay(65.0, 1) < partial_decay(1046.0, 1));
        assert!(partial_decay(261.63, 1) < partial_decay(261.63, 4), "upper partials go first");
    }
}
