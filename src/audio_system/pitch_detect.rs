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
    /// Probabilistic YIN: aggregates YIN over a Beta-weighted range of
    /// thresholds. Monophonic (one pitch).
    Pyin,
    /// McLeod Pitch Method (MPM): normalized square difference function with
    /// key-maximum peak picking. Monophonic (one pitch).
    Mcleod,
    /// Template NMF: decompose the FFT spectrum onto a dictionary of harmonic
    /// note templates. Polyphonic — reports all notes of a chord.
    Nmf,
}

impl PitchAlgorithm {
    /// All selectable algorithms, in display order.
    pub fn all() -> &'static [PitchAlgorithm] {
        &[
            PitchAlgorithm::Fft,
            PitchAlgorithm::Yin,
            PitchAlgorithm::Pyin,
            PitchAlgorithm::Mcleod,
            PitchAlgorithm::Nmf,
        ]
    }

    /// Short label for the Options selector.
    pub fn label(self) -> &'static str {
        match self {
            PitchAlgorithm::Fft => "FFT",
            PitchAlgorithm::Yin => "YIN",
            PitchAlgorithm::Pyin => "pYIN",
            PitchAlgorithm::Mcleod => "MPM",
            PitchAlgorithm::Nmf => "NMF",
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
    /// Cached NMF note dictionary, rebuilt when the spectrum size / rate change.
    nmf_dict: Option<NmfDict>,
}

impl Default for FftState {
    fn default() -> Self {
        Self {
            planner: FftPlanner::new(),
            plan: None,
            last_size: 0,
            nmf_dict: None,
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
        PitchAlgorithm::Pyin => mono_pitch(pyin_pitch(samples, sample_rate)),
        PitchAlgorithm::Mcleod => mono_pitch(mpm_pitch(samples, sample_rate)),
        PitchAlgorithm::Nmf => {
            let n_bins = magnitudes.len();
            let stale = match &state.nmf_dict {
                Some(d) => d.n_bins != n_bins || d.sample_rate != sample_rate,
                None => true,
            };
            if stale {
                state.nmf_dict = Some(build_nmf_dict(sample_rate, n_bins));
            }
            nmf_pitches(&magnitudes, state.nmf_dict.as_ref().unwrap())
        }
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
    let (cmnd, tau_min, tau_max) = yin_cmnd(samples, sample_rate)?;
    // First τ whose d' dips below the absolute threshold (no dip → unvoiced).
    let tau = first_dip_below(&cmnd, tau_min, tau_max, YIN_THRESHOLD)?;
    cmnd_to_freq(&cmnd, tau, sample_rate)
}

/// Build YIN's cumulative-mean-normalized difference function d'(τ) over the
/// harmonica's τ range. Returns `(d', tau_min, tau_max)`, or `None` if the
/// block is too short. Shared by YIN and pYIN.
fn yin_cmnd(samples: &[f32], sample_rate: u32) -> Option<(Vec<f32>, usize, usize)> {
    let sr = sample_rate as f32;
    let tau_min = ((sr / MAX_FREQ).floor() as usize).max(2);
    let tau_max = (sr / MIN_FREQ).ceil() as usize;
    let n = samples.len();
    if tau_max < tau_min || n < tau_max + 2 {
        return None;
    }
    // Comparison window: each τ compares this many sample pairs.
    let w = n - tau_max;

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
    Some((cmnd, tau_min, tau_max))
}

/// First τ in `[tau_min, tau_max]` whose d' dips below `threshold`, descended to
/// the bottom of that dip. `None` if it never crosses the threshold.
fn first_dip_below(cmnd: &[f32], tau_min: usize, tau_max: usize, threshold: f32) -> Option<usize> {
    let mut tau = tau_min;
    while tau <= tau_max {
        if cmnd[tau] < threshold {
            while tau + 1 <= tau_max && cmnd[tau + 1] < cmnd[tau] {
                tau += 1;
            }
            return Some(tau);
        }
        tau += 1;
    }
    None
}

/// Parabolic-refine the chosen lag and convert to a frequency, gated to the
/// harmonica range.
fn cmnd_to_freq(cmnd: &[f32], tau: usize, sample_rate: u32) -> Option<f32> {
    let refined = parabolic_vertex(cmnd, tau);
    let f0 = sample_rate as f32 / refined;
    (MIN_FREQ..=MAX_FREQ).contains(&f0).then_some(f0)
}

// ── pYIN (probabilistic YIN) ──────────────────────────────────────────────────
//
// pYIN (Mauch & Dixon, 2014) runs YIN under many thresholds rather than one,
// weighting each by a prior, to get a distribution over pitch candidates. The
// full method then tracks the best path across frames with an HMM; here
// `analyze` is stateless per audio chunk, so we keep pYIN's per-frame core
// (Beta-weighted threshold sweep) and pick the most probable candidate.

// Number of thresholds swept across (0, 1).
const PYIN_THRESHOLDS: usize = 100;

/// Estimate f0 with the per-frame pYIN threshold sweep, or `None` if no
/// candidate accumulates any probability.
fn pyin_pitch(samples: &[f32], sample_rate: u32) -> Option<f32> {
    let (cmnd, tau_min, tau_max) = yin_cmnd(samples, sample_rate)?;

    // Accumulate prior probability onto whichever τ each threshold selects.
    let mut prob = vec![0.0f32; tau_max + 1];
    for k in 0..PYIN_THRESHOLDS {
        let threshold = (k as f32 + 0.5) / PYIN_THRESHOLDS as f32;
        if let Some(tau) = first_dip_below(&cmnd, tau_min, tau_max, threshold) {
            prob[tau] += beta_weight(threshold);
        }
    }

    let best = (tau_min..=tau_max)
        .max_by(|&a, &b| prob[a].partial_cmp(&prob[b]).unwrap_or(std::cmp::Ordering::Equal))?;
    if prob[best] <= 0.0 {
        return None;
    }
    cmnd_to_freq(&cmnd, best, sample_rate)
}

/// Unnormalized Beta(2, 18) density — pYIN's default prior over the YIN
/// threshold (mean ≈ 0.1). The B(a,b) constant is dropped since only relative
/// weights matter.
fn beta_weight(s: f32) -> f32 {
    s * (1.0 - s).powi(17)
}

/// Sub-sample lag refinement: fit a parabola to `c[τ-1..τ+1]` and return its
/// vertex's abscissa (works for a dip in YIN/pYIN or a peak in MPM).
fn parabolic_vertex(c: &[f32], tau: usize) -> f32 {
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

// ── McLeod Pitch Method (MPM) ─────────────────────────────────────────────────
//
// MPM (McLeod & Wyvill, 2005): the normalized square difference function
//   n(τ) = 2·Σ x[j]·x[j+τ] / Σ (x[j]² + x[j+τ]²)   ∈ [-1, 1]
// peaks at periodic lags. We collect the "key maxima" (the highest point of each
// positive hump after the τ=0 lobe) and pick the first whose clarity reaches
// `MPM_CLARITY` of the strongest — favouring the fundamental over its octave.

const MPM_CLARITY: f32 = 0.9;

/// Estimate f0 with MPM, or `None` if the block is too short or no key maximum
/// lands a clear pitch in the harmonica range.
fn mpm_pitch(samples: &[f32], sample_rate: u32) -> Option<f32> {
    let sr = sample_rate as f32;
    let tau_min = ((sr / MAX_FREQ).floor() as usize).max(2);
    let tau_max = (sr / MIN_FREQ).ceil() as usize;
    let n = samples.len();
    if tau_max < tau_min || n < tau_max + 2 {
        return None;
    }

    // Normalized square difference function over the τ range.
    let mut nsdf = vec![0.0f32; tau_max + 1];
    for tau in 0..=tau_max {
        let mut acf = 0.0f32; // Σ x[j]·x[j+τ]
        let mut norm = 0.0f32; // Σ x[j]² + x[j+τ]²
        for j in 0..(n - tau) {
            let (a, b) = (samples[j], samples[j + tau]);
            acf += a * b;
            norm += a * a + b * b;
        }
        nsdf[tau] = if norm > 0.0 { 2.0 * acf / norm } else { 0.0 };
    }

    // Key maxima: skip the τ=0 lobe, then take the peak of each positive hump.
    let mut maxima: Vec<usize> = Vec::new();
    let mut tau = 1;
    while tau <= tau_max && nsdf[tau] > 0.0 {
        tau += 1; // descend off the τ=0 lobe to the first negative
    }
    while tau <= tau_max {
        while tau <= tau_max && nsdf[tau] <= 0.0 {
            tau += 1; // skip to the next positive zero crossing
        }
        let mut best = tau;
        while tau <= tau_max && nsdf[tau] > 0.0 {
            if nsdf[tau] > nsdf[best] {
                best = tau;
            }
            tau += 1;
        }
        if best >= tau_min && best <= tau_max {
            maxima.push(best);
        }
    }

    // Pick the first key maximum within MPM_CLARITY of the strongest one.
    let strongest = maxima.iter().map(|&t| nsdf[t]).fold(f32::MIN, f32::max);
    if strongest <= 0.0 {
        return None;
    }
    let threshold = MPM_CLARITY * strongest;
    let chosen = *maxima.iter().find(|&&t| nsdf[t] >= threshold)?;

    let refined = parabolic_vertex(&nsdf, chosen);
    let f0 = sr / refined;
    (MIN_FREQ..=MAX_FREQ).contains(&f0).then_some(f0)
}

// ── Template NMF (polyphonic) ─────────────────────────────────────────────────
//
// Decompose the observed magnitude spectrum y onto a fixed dictionary D whose
// columns are synthetic harmonic templates, one per chromatic note in the
// harmonica range: y ≈ D·a, a ≥ 0. The notes whose activations a are strongest
// are the ones sounding — so unlike the YIN/MPM family this reports a whole
// chord. Activations are solved with non-negative multiplicative updates (NMF
// with a fixed basis), which need no model files and run on the FFT we already
// compute. The harmonic templates also damp octave confusion: a low note
// explains its own upper partials, so energy isn't double-counted as a 2nd note.

// Harmonics per note template, amplitude rolling off as 1/h.
const NMF_HARMONICS: usize = 8;
// Iterations of the multiplicative activation update per frame.
const NMF_ITERS: usize = 50;
// A note is "playing" when its activation reaches this fraction of the loudest.
const NMF_ACTIVATION_RATIO: f32 = 0.22;
// Cap on simultaneously reported notes (a harmonica chord is only so wide).
const NMF_MAX_NOTES: usize = 6;

/// A fixed harmonic-template dictionary over the chromatic notes in range, with
/// `DᵀD` precomputed for the activation updates. Tied to one spectrum size +
/// sample rate (rebuilt when those change).
struct NmfDict {
    sample_rate: u32,
    n_bins: usize,
    n_notes: usize,
    /// Fundamental frequency of each note (dictionary column).
    freqs: Vec<f32>,
    /// `n_notes` columns, each a length-`n_bins` magnitude template.
    columns: Vec<Vec<f32>>,
    /// `DᵀD` (`n_notes × n_notes`), the constant part of the update denominator.
    dtd: Vec<Vec<f32>>,
}

/// Build the harmonic-template dictionary for a given spectrum size / rate.
fn build_nmf_dict(sample_rate: u32, n_bins: usize) -> NmfDict {
    let nyquist = sample_rate as f32 / 2.0;
    // magnitudes span DC..Nyquist over n_bins, so each bin is this wide.
    let freq_res = nyquist / n_bins.max(1) as f32;

    let midi_lo = (12.0 * (MIN_FREQ / 440.0).log2() + 69.0).ceil() as i32;
    let midi_hi = (12.0 * (MAX_FREQ / 440.0).log2() + 69.0).floor() as i32;

    let mut freqs = Vec::new();
    let mut columns: Vec<Vec<f32>> = Vec::new();
    for midi in midi_lo..=midi_hi {
        let f = 440.0 * 2f32.powf((midi - 69) as f32 / 12.0);
        let mut col = vec![0.0f32; n_bins];
        for h in 1..=NMF_HARMONICS {
            let fh = f * h as f32;
            if fh >= nyquist {
                break;
            }
            let center = fh / freq_res;
            let amp = 1.0 / h as f32;
            // Gaussian smear (~Hanning main-lobe width) so slight detuning still hits.
            let lo = (center.floor() as i32 - 2).max(0);
            let hi = (center.ceil() as i32 + 2).min(n_bins as i32 - 1);
            for b in lo..=hi {
                let d = b as f32 - center;
                col[b as usize] += amp * (-(d * d) / 2.0).exp();
            }
        }
        // L2-normalize so DᵀD has a unit diagonal (notes compete fairly).
        let norm = col.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in col.iter_mut() {
                *v /= norm;
            }
        }
        freqs.push(f);
        columns.push(col);
    }

    let n_notes = columns.len();
    let mut dtd = vec![vec![0.0f32; n_notes]; n_notes];
    for i in 0..n_notes {
        for j in i..n_notes {
            let s: f32 = (0..n_bins).map(|b| columns[i][b] * columns[j][b]).sum();
            dtd[i][j] = s;
            dtd[j][i] = s;
        }
    }

    NmfDict {
        sample_rate,
        n_bins,
        n_notes,
        freqs,
        columns,
        dtd,
    }
}

/// Solve `y ≈ D·a` (a ≥ 0) with multiplicative updates and report the notes
/// whose activation clears the threshold, strongest first.
fn nmf_pitches(magnitudes: &[f32], dict: &NmfDict) -> Vec<PitchInfo> {
    let n = dict.n_notes;
    if n == 0 || dict.n_bins == 0 {
        return vec![];
    }

    // Dᵀy: correlation of each template with the observed spectrum.
    let dty: Vec<f32> = (0..n)
        .map(|k| {
            (0..dict.n_bins)
                .map(|b| dict.columns[k][b] * magnitudes[b])
                .sum()
        })
        .collect();

    // a ← a · (Dᵀy) / (DᵀD·a). Start uniform-positive so every note can grow.
    let mut a = vec![1.0f32; n];
    for _ in 0..NMF_ITERS {
        let dtda: Vec<f32> = (0..n)
            .map(|k| (0..n).map(|j| dict.dtd[k][j] * a[j]).sum::<f32>())
            .collect();
        for k in 0..n {
            a[k] *= dty[k] / (dtda[k] + 1e-9);
        }
    }

    let max_a = a.iter().cloned().fold(0.0f32, f32::max);
    if max_a <= 0.0 {
        return vec![];
    }
    let threshold = max_a * NMF_ACTIVATION_RATIO;

    let mut active: Vec<(f32, f32)> = (0..n)
        .filter(|&k| a[k] >= threshold)
        .map(|k| (dict.freqs[k], a[k]))
        .collect();
    active.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
    active.truncate(NMF_MAX_NOTES);

    active
        .into_iter()
        .filter_map(|(f, _)| {
            freq_to_note(f).map(|(note, octave)| PitchInfo {
                note,
                octave,
                frequency: f,
            })
        })
        .collect()
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
    fn pyin_detects_440hz() {
        let sample_rate = 44100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let f0 = pyin_pitch(&samples, sample_rate).expect("expected a pitch");
        assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
    }

    #[test]
    fn pyin_rejects_silence() {
        assert_eq!(pyin_pitch(&vec![0.0f32; 4096], 44100), None);
    }

    #[test]
    fn mpm_detects_440hz() {
        let sample_rate = 44100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let f0 = mpm_pitch(&samples, sample_rate).expect("expected a pitch");
        assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
    }

    #[test]
    fn mpm_rejects_silence() {
        assert_eq!(mpm_pitch(&vec![0.0f32; 4096], 44100), None);
    }

    // Render the FFT magnitude spectrum of a sum of sine tones, the way
    // `analyze` would, so the NMF detector can be tested directly.
    fn magnitudes_of(freqs: &[f32], sample_rate: u32, n: usize) -> Vec<f32> {
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                freqs
                    .iter()
                    .map(|&f| 0.4 * (2.0 * PI * f * i as f32 / sample_rate as f32).sin())
                    .sum()
            })
            .collect();
        let mut state = FftState::default();
        analyze(&samples, sample_rate, &mut state, PitchAlgorithm::Fft).magnitudes
    }

    #[test]
    fn nmf_detects_a_two_note_chord() {
        let sample_rate = 44100u32;
        let n = 4096;
        // A4 (440) + C#5 (554.37): a major third.
        let mags = magnitudes_of(&[440.0, 554.37], sample_rate, n);
        let dict = build_nmf_dict(sample_rate, mags.len());
        let pitches = nmf_pitches(&mags, &dict);
        assert!(
            pitches.iter().any(|p| p.note == "A" && p.octave == 4),
            "expected A4, got {pitches:?}"
        );
        assert!(
            pitches.iter().any(|p| p.note == "C#" && p.octave == 5),
            "expected C#5, got {pitches:?}"
        );
    }

    #[test]
    fn nmf_single_tone_is_one_note() {
        let sample_rate = 44100u32;
        let mags = magnitudes_of(&[440.0], sample_rate, 4096);
        let dict = build_nmf_dict(sample_rate, mags.len());
        let pitches = nmf_pitches(&mags, &dict);
        assert!(
            pitches.iter().any(|p| p.note == "A" && p.octave == 4),
            "expected A4 among {pitches:?}"
        );
    }

    #[test]
    fn all_algorithms_agree_on_a_clean_tone() {
        // A 330 Hz tone (E4) should read the same through every detector.
        let sample_rate = 44100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| 0.4 * (2.0 * PI * 330.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let mut state = FftState::default();
        for algo in PitchAlgorithm::all() {
            let pitches = analyze(&samples, sample_rate, &mut state, *algo).pitches;
            assert!(
                pitches.iter().any(|p| p.note == "E" && p.octave == 4),
                "{:?} should detect E4, got {:?}",
                algo,
                pitches
            );
        }
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
