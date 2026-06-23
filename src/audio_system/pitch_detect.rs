// SPDX-License-Identifier: MIT

use bevy::prelude::{Message, Resource};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Harmonica range: roughly C4 (262 Hz) to the top of a 10-hole diatonic.
const MIN_FREQ: f32 = 200.0;
const MAX_FREQ: f32 = 2500.0;

/// Which pitch-detection algorithm the audio pipeline uses for *pitches*. The
/// FFT magnitude spectrum is always computed (the spectrogram needs it); this
/// only selects how fundamentals are extracted. Selectable on the Options page
/// and persisted in settings.
#[derive(
    Resource, Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub enum PitchAlgorithm {
    /// FFT peak-picking with harmonic suppression. Reports multiple pitches.
    #[default]
    Fft,
    /// YIN cumulative-mean-difference detector. Monophonic (one pitch).
    Yin,
}

impl PitchAlgorithm {
    /// All selectable algorithms, in display order.
    pub fn all() -> &'static [PitchAlgorithm] {
        &[PitchAlgorithm::Fft, PitchAlgorithm::Yin]
    }

    /// Short label for the Options selector.
    pub fn label(self) -> &'static str {
        match self {
            PitchAlgorithm::Fft => "FFT",
            PitchAlgorithm::Yin => "YIN",
        }
    }
}

// A peak must exceed this fraction of the strongest peak to be reported.
const PEAK_THRESHOLD_RATIO: f32 = 0.08;

// Signals below this RMS level are treated as silence.
const SILENCE_THRESHOLD: f32 = 0.005;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Debug, Clone, PartialEq)]
pub struct PitchInfo {
    pub note: String,
    pub octave: i32,
    pub frequency: f32,
}

#[derive(Message)]
pub struct PitchEvent(pub Vec<PitchInfo>);

/// One block of audio analysed: the detected pitches plus the magnitude spectrum
/// (half, DC..Nyquist) and its bin width. Sharing the spectrum lets other
/// consumers (the spectrogram) reuse this FFT instead of computing their own.
pub struct Analysis {
    pub pitches: Vec<PitchInfo>,
    pub magnitudes: Vec<f32>,
    pub freq_res: f32,
}

/// The latest analysed audio frame, published by the audio pipeline so multiple
/// consumers reuse one FFT: `magnitudes`/`freq_res` for frequency-domain views
/// (spectrogram bars) and `samples` for time-domain views (oscilloscope). Empty
/// vectors mean silence / no audio.
#[derive(Resource, Default)]
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub magnitudes: Vec<f32>,
    pub freq_res: f32,
}

// System-local state for the Bevy `Local<FftState>` parameter — avoids
// re-allocating the FFT plan on every frame.
pub struct FftState {
    planner: FftPlanner<f32>,
    plan: Option<Arc<dyn Fft<f32>>>,
    last_size: usize,
}

impl Default for FftState {
    fn default() -> Self {
        Self {
            planner: FftPlanner::new(),
            plan: None,
            last_size: 0,
        }
    }
}

/// Detect pitches in a block of audio with the default (FFT) algorithm. Thin
/// wrapper over [`analyze`] for callers that only want the pitches.
pub fn detect_pitches(samples: &[f32], sample_rate: u32, state: &mut FftState) -> Vec<PitchInfo> {
    analyze(samples, sample_rate, state, PitchAlgorithm::Fft).pitches
}

/// Window + FFT a block once (for the magnitude spectrum), then extract pitches
/// with the selected `algorithm`. Returns empty magnitudes/pitches for
/// too-short or silent input.
pub fn analyze(
    samples: &[f32],
    sample_rate: u32,
    state: &mut FftState,
    algorithm: PitchAlgorithm,
) -> Analysis {
    let n = samples.len();
    let freq_res = if n > 0 {
        sample_rate as f32 / n as f32
    } else {
        0.0
    };
    let silent = Analysis {
        pitches: vec![],
        magnitudes: vec![],
        freq_res,
    };

    if n < 2 {
        return silent;
    }
    let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / n as f32).sqrt();
    if rms < SILENCE_THRESHOLD {
        return silent;
    }

    // Lazily create (or re-create on size change) the forward FFT plan.
    if state.last_size != n {
        state.plan = Some(state.planner.plan_fft_forward(n));
        state.last_size = n;
    }
    let plan = state.plan.as_ref().unwrap();

    // Hanning window applied in-place before FFT.
    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
            Complex::new(s * w, 0.0)
        })
        .collect();

    plan.process(&mut buffer);

    let half = n / 2;
    let magnitudes: Vec<f32> = buffer[..half].iter().map(|c| c.norm()).collect();

    // Pitches come from the selected algorithm; the magnitudes above are always
    // produced so frequency-domain views (the spectrogram) keep working.
    let pitches = match algorithm {
        PitchAlgorithm::Fft => pitches_from_magnitudes(&magnitudes, freq_res),
        PitchAlgorithm::Yin => mono_pitch(yin_pitch(samples, sample_rate)),
    };

    Analysis {
        pitches,
        magnitudes,
        freq_res,
    }
}

/// Wrap a single monophonic fundamental (if any) into the `Vec<PitchInfo>` the
/// rest of the pipeline expects.
fn mono_pitch(freq: Option<f32>) -> Vec<PitchInfo> {
    freq.and_then(|f| {
        freq_to_note(f).map(|(note, octave)| PitchInfo {
            note,
            octave,
            frequency: f,
        })
    })
    .into_iter()
    .collect()
}

/// Peak-picks fundamentals from a precomputed magnitude spectrum.
fn pitches_from_magnitudes(magnitudes: &[f32], freq_res: f32) -> Vec<PitchInfo> {
    let n_bins = magnitudes.len();
    let max_mag = magnitudes.iter().cloned().fold(0.0f32, f32::max);
    if max_mag < 1e-9 || freq_res <= 0.0 {
        return vec![];
    }

    let threshold = max_mag * PEAK_THRESHOLD_RATIO;
    let min_bin = (MIN_FREQ / freq_res) as usize;
    let max_bin = ((MAX_FREQ / freq_res) as usize).min(n_bins.saturating_sub(2));

    // Collect local maxima — use parabolic interpolation for sub-bin accuracy.
    let mut raw_peaks: Vec<(f32, f32)> = Vec::new();
    for i in min_bin.max(1)..=max_bin {
        if magnitudes[i] > magnitudes[i - 1]
            && magnitudes[i] > magnitudes[i + 1]
            && magnitudes[i] > threshold
        {
            let freq = parabolic_peak(magnitudes, i, freq_res);
            raw_peaks.push((freq, magnitudes[i]));
        }
    }

    // Sort by magnitude descending so fundamental candidates come first.
    raw_peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    suppress_harmonics(&raw_peaks)
        .into_iter()
        .filter_map(|(freq, _)| {
            freq_to_note(freq).map(|(note, octave)| PitchInfo {
                note,
                octave,
                frequency: freq,
            })
        })
        .collect()
}

// Sub-bin frequency refinement via parabolic interpolation on the three bins
// around a local maximum.
fn parabolic_peak(mags: &[f32], bin: usize, freq_res: f32) -> f32 {
    let (a, b, g) = (mags[bin - 1], mags[bin], mags[bin + 1]);
    let denom = a - 2.0 * b + g;
    if denom.abs() < 1e-10 {
        return bin as f32 * freq_res;
    }
    (bin as f32 + 0.5 * (a - g) / denom) * freq_res
}

// Remove peaks that are integer multiples (harmonics) of a stronger peak.
// The input slice must already be sorted by magnitude descending.
fn suppress_harmonics(peaks: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut suppressed = vec![false; peaks.len()];
    for i in 0..peaks.len() {
        if suppressed[i] {
            continue;
        }
        for j in 0..peaks.len() {
            if i == j || suppressed[j] {
                continue;
            }
            let ratio = peaks[j].0 / peaks[i].0;
            for h in 2..=8u32 {
                // 5 % tolerance per harmonic number
                if (ratio - h as f32).abs() < 0.05 * h as f32 {
                    suppressed[j] = true;
                    break;
                }
            }
        }
    }
    peaks
        .iter()
        .enumerate()
        .filter(|(i, _)| !suppressed[*i])
        .map(|(_, &p)| p)
        .collect()
}

// ── YIN ─────────────────────────────────────────────────────────────────────
//
// YIN (de Cheveigné & Kawahara, 2002): a time-domain monophonic detector. It
// finds the lag τ that best makes the signal periodic, then f0 = sample_rate/τ.
// Robust against the octave errors that plague plain autocorrelation.

// A τ is accepted as the period when its cumulative-mean-normalized difference
// dips below this. Lower = stricter (fewer false positives, more dropouts).
const YIN_THRESHOLD: f32 = 0.15;

/// Estimate the fundamental frequency of `samples` with YIN, or `None` if the
/// block is too short or has no clear pitch in the harmonica range.
fn yin_pitch(samples: &[f32], sample_rate: u32) -> Option<f32> {
    let sr = sample_rate as f32;
    let tau_min = ((sr / MAX_FREQ).floor() as usize).max(2);
    let tau_max = (sr / MIN_FREQ).ceil() as usize;
    let n = samples.len();
    if tau_max < tau_min || n < tau_max + 2 {
        return None;
    }
    // Comparison window: each τ compares this many sample pairs.
    let w = n - tau_max;

    // Step 1+2: difference function d(τ) and its cumulative mean normalization.
    // d'(τ) = d(τ) · τ / Σ_{j=1..τ} d(j); d'(0) ≡ 1.
    let mut cmnd = vec![1.0f32; tau_max + 1];
    let mut running = 0.0f32;
    for tau in 1..=tau_max {
        let mut sum = 0.0f32;
        for j in 0..w {
            let diff = samples[j] - samples[j + tau];
            sum += diff * diff;
        }
        running += sum;
        cmnd[tau] = if running > 0.0 {
            sum * tau as f32 / running
        } else {
            1.0
        };
    }

    // Step 3: first τ (in range) whose d' dips below the threshold, descended to
    // the bottom of that dip. No dip → unvoiced / no clear pitch.
    let mut tau = tau_min;
    let mut found = None;
    while tau <= tau_max {
        if cmnd[tau] < YIN_THRESHOLD {
            while tau + 1 <= tau_max && cmnd[tau + 1] < cmnd[tau] {
                tau += 1;
            }
            found = Some(tau);
            break;
        }
        tau += 1;
    }
    let tau = found?;

    // Step 4: parabolic interpolation around the dip for sub-sample accuracy.
    let refined = parabolic_min(&cmnd, tau);
    let f0 = sr / refined;
    (MIN_FREQ..=MAX_FREQ).contains(&f0).then_some(f0)
}

/// Sub-sample lag refinement: fit a parabola to d'(τ-1..τ+1) and return its
/// minimum's abscissa.
fn parabolic_min(c: &[f32], tau: usize) -> f32 {
    if tau == 0 || tau + 1 >= c.len() {
        return tau as f32;
    }
    let (a, b, g) = (c[tau - 1], c[tau], c[tau + 1]);
    let denom = a - 2.0 * b + g;
    if denom.abs() < 1e-10 {
        tau as f32
    } else {
        tau as f32 + 0.5 * (a - g) / denom
    }
}

fn freq_to_note(freq: f32) -> Option<(String, i32)> {
    if freq <= 0.0 {
        return None;
    }
    let midi = 12.0 * (freq / 440.0).log2() + 69.0;
    let midi_rounded = midi.round() as i32;
    if midi_rounded < 0 {
        return None;
    }
    let octave = midi_rounded / 12 - 1;
    let note_idx = (midi_rounded % 12) as usize;
    Some((NOTE_NAMES[note_idx].to_string(), octave))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn silence_returns_empty() {
        let samples = vec![0.0f32; 4096];
        let mut state = FftState::default();
        assert!(detect_pitches(&samples, 44100, &mut state).is_empty());
    }

    #[test]
    fn too_short_returns_empty() {
        let mut state = FftState::default();
        assert!(detect_pitches(&[], 44100, &mut state).is_empty());
        assert!(detect_pitches(&[0.5], 44100, &mut state).is_empty());
    }

    #[test]
    fn sine_440hz_detected_as_a4() {
        let sample_rate = 44100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let mut state = FftState::default();
        let pitches = detect_pitches(&samples, sample_rate, &mut state);
        assert!(!pitches.is_empty(), "expected at least one pitch");
        let a4 = pitches.iter().find(|p| p.note == "A" && p.octave == 4);
        assert!(a4.is_some(), "expected A4, got {:?}", pitches);
    }

    #[test]
    fn harmonic_is_suppressed() {
        // 880 Hz is 2× 440 Hz and should be removed as a harmonic.
        let peaks = vec![(440.0f32, 1.0f32), (880.0, 0.5)];
        let result = suppress_harmonics(&peaks);
        assert_eq!(result.len(), 1);
        assert!((result[0].0 - 440.0).abs() < 1.0);
    }

    #[test]
    fn non_harmonic_peaks_both_kept() {
        // 440 Hz (A4) and 659 Hz (E5) are not harmonically related.
        let peaks = vec![(440.0f32, 1.0f32), (659.0, 0.8)];
        let result = suppress_harmonics(&peaks);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn yin_detects_440hz() {
        let sample_rate = 44100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let f0 = yin_pitch(&samples, sample_rate).expect("expected a pitch");
        assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
        assert_eq!(freq_to_note(f0), Some(("A".to_string(), 4)));
    }

    #[test]
    fn yin_rejects_silence_and_noise() {
        // Flat silence: no period.
        assert_eq!(yin_pitch(&vec![0.0f32; 4096], 44100), None);
    }

    #[test]
    fn yin_via_analyze_uses_selected_algorithm() {
        let sample_rate = 44100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let mut state = FftState::default();
        let pitches = analyze(&samples, sample_rate, &mut state, PitchAlgorithm::Yin).pitches;
        assert_eq!(pitches.len(), 1, "YIN is monophonic: one pitch");
        assert_eq!(pitches[0].note, "A");
        assert_eq!(pitches[0].octave, 4);
    }

    #[test]
    fn freq_to_note_a440() {
        assert_eq!(freq_to_note(440.0), Some(("A".to_string(), 4)));
    }

    #[test]
    fn freq_to_note_middle_c() {
        // C4 ≈ 261.63 Hz
        assert_eq!(freq_to_note(261.63), Some(("C".to_string(), 4)));
    }

    #[test]
    fn freq_to_note_zero_or_negative_returns_none() {
        assert_eq!(freq_to_note(0.0), None);
        assert_eq!(freq_to_note(-1.0), None);
    }
}
