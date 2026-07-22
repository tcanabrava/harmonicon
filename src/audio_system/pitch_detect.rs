// SPDX-License-Identifier: MIT

use bevy::log::info_span;
use bevy::prelude::{Message, Resource};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::midi::NOTE_NAMES;

/// Frequency bounds (Hz) the pitch detectors search within. Defaults to
/// roughly a standard-key 10-hole diatonic's range; gameplay narrows/widens
/// this from the loaded chart's harmonica layout (or the bend trainer's
/// current key) in [`PitchRange::from_freqs`], so a Low-F/Low-D harp's
/// hole-1 notes aren't cut off by a floor tuned for standard keys.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct PitchRange {
    pub min_freq: f32,
    pub max_freq: f32,
}

impl Default for PitchRange {
    fn default() -> Self {
        // Roughly C4 (262 Hz) to the top of a 10-hole diatonic.
        Self {
            min_freq: 200.0,
            max_freq: 2500.0,
        }
    }
}

/// Semitone margin added on each side of a harmonica's natural range when
/// sizing [`PitchRange`] from it — covers bends/overblows landing just past
/// a charted note plus a little slop before a clean attack. Shared by
/// gameplay's chart-driven range, the bend trainer's key-driven one, and
/// the song editor's record mode.
pub const PITCH_RANGE_MARGIN_SEMITONES: f32 = 1.0;

impl PitchRange {
    /// Bounds spanning `freqs`, widened by `margin_semitones` on each side so
    /// a bend or attack landing just past a charted note still detects.
    /// Falls back to [`PitchRange::default`] when `freqs` is empty.
    pub fn from_freqs(freqs: impl IntoIterator<Item = f32>, margin_semitones: f32) -> Self {
        let (lo, hi) = freqs
            .into_iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), f| (lo.min(f), hi.max(f)));
        if lo > hi {
            return Self::default();
        }
        let ratio = 2f32.powf(margin_semitones / 12.0);
        Self {
            min_freq: lo / ratio,
            max_freq: hi * ratio,
        }
    }
}

/// Which pitch-detection algorithm the audio pipeline uses for *pitches*. The
/// FFT magnitude spectrum is always computed (the spectrogram needs it); this
/// only selects how fundamentals are extracted. Selectable on the Options page
/// and persisted in settings.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
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

    /// Inverse of [`label`](Self::label) — for UI that deals in plain
    /// strings (e.g. `dialogs::combobox`'s selection event) rather than the
    /// enum itself. `None` for anything that isn't one of [`Self::all`]'s
    /// labels.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().iter().copied().find(|a| a.label() == label)
    }

    /// A short, player-facing explanation shown next to the selector.
    pub fn description(self) -> &'static str {
        match self {
            PitchAlgorithm::Fft => {
                "Fast Fourier Transform peak-picking with harmonic suppression. \
                 Reports several notes at once (polyphonic-ish) and is cheap, but \
                 less precise on a single bent note. A solid all-round default."
            }
            PitchAlgorithm::Yin => {
                "YIN: a time-domain detector that finds the period that best makes \
                 the signal repeat. Monophonic (one note) and very accurate on \
                 clean single notes, with few octave errors — good for bends."
            }
            PitchAlgorithm::Pyin => {
                "Probabilistic YIN: runs YIN over many thresholds weighted by a \
                 prior and keeps the most likely pitch. Monophonic; steadier than \
                 plain YIN on quiet or noisy input, at a little more cost."
            }
            PitchAlgorithm::Mcleod => {
                "McLeod Pitch Method (MPM): normalized square-difference with \
                 clarity-based peak picking. Monophonic; fast and robust against \
                 octave jumps, a strong choice for live single-note playing."
            }
            PitchAlgorithm::Nmf => {
                "Template NMF: matches the spectrum against harmonic templates for \
                 every note. Polyphonic — it can report the notes of a chord — but \
                 heavier and the most experimental option here."
            }
        }
    }
}

// A peak must exceed this fraction of the strongest peak to be reported.
const PEAK_THRESHOLD_RATIO: f32 = 0.08;

/// The window function `analyze` applies before the FFT — a single named
/// constant (rather than a magic string wherever this needs describing) so
/// it can't drift out of sync with the actual formula below if that ever
/// changes. Referenced by `song_editor::debug_record`'s recording metadata.
pub const WINDOW_FUNCTION: &str = "Hann";

// Signals below this RMS level are treated as silence.
const SILENCE_THRESHOLD: f32 = 0.005;

#[derive(Debug, Clone, PartialEq)]
pub struct PitchInfo {
    /// MIDI note number (0-127), rounded to the nearest semitone — the
    /// canonical identity used for scoring comparisons (see
    /// `gameplay::PitchGate`/`ValidHarpNotes`). `note`/`octave` are kept
    /// alongside it purely for display; they're the same pitch, just spelled
    /// out (and always sharp-spelled, never flat — see `midi_to_note`).
    pub midi: u8,
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

/// Detect pitches in a block of audio with the default (FFT) algorithm and the
/// default [`PitchRange`]. Thin wrapper over [`analyze`] for callers that only
/// want the pitches.
pub fn detect_pitches(samples: &[f32], sample_rate: u32, state: &mut FftState) -> Vec<PitchInfo> {
    analyze(
        samples,
        sample_rate,
        state,
        PitchAlgorithm::Fft,
        PitchRange::default(),
    )
    .pitches
}

/// Window + FFT a block once (for the magnitude spectrum), then extract pitches
/// with the selected `algorithm`, searched within `range`. Returns empty
/// magnitudes/pitches for too-short or silent input.
pub fn analyze(
    samples: &[f32],
    sample_rate: u32,
    state: &mut FftState,
    algorithm: PitchAlgorithm,
    range: PitchRange,
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

    let magnitudes = {
        let _span = info_span!("fft_window_and_transform", n).entered();
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
                let w =
                    0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
                Complex::new(s * w, 0.0)
            })
            .collect();

        plan.process(&mut buffer);

        let half = n / 2;
        buffer[..half]
            .iter()
            .map(|c| c.norm())
            .collect::<Vec<f32>>()
    };

    // Pitches come from the selected algorithm; the magnitudes above are always
    // produced so frequency-domain views (the spectrogram) keep working. The
    // algorithms cost wildly different amounts (a plain FFT peak-pick vs.
    // NMF's dictionary matching), hence one span per branch rather than a
    // single span with an `algorithm` field.
    let pitches = match algorithm {
        PitchAlgorithm::Fft => {
            let _span = info_span!("pitches_from_magnitudes").entered();
            pitches_from_magnitudes(&magnitudes, freq_res, range)
        }
        PitchAlgorithm::Yin => {
            let _span = info_span!("yin_pitch").entered();
            mono_pitch(yin_pitch(samples, sample_rate, range))
        }
        PitchAlgorithm::Pyin => {
            let _span = info_span!("pyin_pitch").entered();
            mono_pitch(pyin_pitch(samples, sample_rate, range))
        }
        PitchAlgorithm::Mcleod => {
            let _span = info_span!("mpm_pitch").entered();
            mono_pitch(mpm_pitch(samples, sample_rate, range))
        }
        PitchAlgorithm::Nmf => {
            let _span = info_span!("nmf_pitches").entered();
            let n_bins = magnitudes.len();
            let stale = match &state.nmf_dict {
                Some(d) => d.n_bins != n_bins || d.sample_rate != sample_rate || d.range != range,
                None => true,
            };
            if stale {
                state.nmf_dict = Some(build_nmf_dict(sample_rate, n_bins, range));
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
        freq_to_note(f).map(|(midi, note, octave)| PitchInfo {
            midi,
            note,
            octave,
            frequency: f,
        })
    })
    .into_iter()
    .collect()
}

/// Peak-picks fundamentals from a precomputed magnitude spectrum.
fn pitches_from_magnitudes(magnitudes: &[f32], freq_res: f32, range: PitchRange) -> Vec<PitchInfo> {
    let n_bins = magnitudes.len();
    let max_mag = magnitudes.iter().cloned().fold(0.0f32, f32::max);
    if max_mag < 1e-9 || freq_res <= 0.0 {
        return vec![];
    }

    let threshold = max_mag * PEAK_THRESHOLD_RATIO;
    let min_bin = (range.min_freq / freq_res) as usize;
    let max_bin = ((range.max_freq / freq_res) as usize).min(n_bins.saturating_sub(2));

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
            freq_to_note(freq).map(|(midi, note, octave)| PitchInfo {
                midi,
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
/// block is too short or has no clear pitch within `range`.
fn yin_pitch(samples: &[f32], sample_rate: u32, range: PitchRange) -> Option<f32> {
    let (cmnd, tau_min, tau_max) = yin_cmnd(samples, sample_rate, range)?;
    // First τ whose d' dips below the absolute threshold (no dip → unvoiced).
    let tau = first_dip_below(&cmnd, tau_min, tau_max, YIN_THRESHOLD)?;
    cmnd_to_freq(&cmnd, tau, sample_rate, range)
}

/// Build YIN's cumulative-mean-normalized difference function d'(τ) over
/// `range`'s τ span. Returns `(d', tau_min, tau_max)`, or `None` if the
/// block is too short. Shared by YIN and pYIN.
fn yin_cmnd(
    samples: &[f32],
    sample_rate: u32,
    range: PitchRange,
) -> Option<(Vec<f32>, usize, usize)> {
    let sr = sample_rate as f32;
    let tau_min = ((sr / range.max_freq).floor() as usize).max(2);
    let tau_max = (sr / range.min_freq).ceil() as usize;
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
            while tau < tau_max && cmnd[tau + 1] < cmnd[tau] {
                tau += 1;
            }
            return Some(tau);
        }
        tau += 1;
    }
    None
}

/// Parabolic-refine the chosen lag and convert to a frequency, gated to
/// `range`.
fn cmnd_to_freq(cmnd: &[f32], tau: usize, sample_rate: u32, range: PitchRange) -> Option<f32> {
    let refined = parabolic_vertex(cmnd, tau);
    let f0 = sample_rate as f32 / refined;
    (range.min_freq..=range.max_freq)
        .contains(&f0)
        .then_some(f0)
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
fn pyin_pitch(samples: &[f32], sample_rate: u32, range: PitchRange) -> Option<f32> {
    let (cmnd, tau_min, tau_max) = yin_cmnd(samples, sample_rate, range)?;

    // Accumulate prior probability onto whichever τ each threshold selects.
    let mut prob = vec![0.0f32; tau_max + 1];
    for k in 0..PYIN_THRESHOLDS {
        let threshold = (k as f32 + 0.5) / PYIN_THRESHOLDS as f32;
        if let Some(tau) = first_dip_below(&cmnd, tau_min, tau_max, threshold) {
            prob[tau] += beta_weight(threshold);
        }
    }

    let best = (tau_min..=tau_max).max_by(|&a, &b| {
        prob[a]
            .partial_cmp(&prob[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    if prob[best] <= 0.0 {
        return None;
    }
    cmnd_to_freq(&cmnd, best, sample_rate, range)
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

// A frame is only voiced when its best NSDF value reaches this. The 0.9
// `MPM_CLARITY` above is *relative* (which key maximum to pick); without an
// absolute floor too, an unpitched frame — breath noise loud enough to pass
// the RMS silence gate — still reports whichever lag happens to score
// highest, however weakly periodic. A cleanly played harmonica note scores
// well above this; broadband noise well below.
const MPM_MIN_CLARITY: f32 = 0.6;

/// Estimate f0 with MPM, or `None` if the block is too short or no key maximum
/// lands a clear pitch within `range`.
fn mpm_pitch(samples: &[f32], sample_rate: u32, range: PitchRange) -> Option<f32> {
    let sr = sample_rate as f32;
    let tau_min = ((sr / range.max_freq).floor() as usize).max(2);
    let tau_max = (sr / range.min_freq).ceil() as usize;
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

    // Pick the first key maximum within MPM_CLARITY of the strongest one —
    // but only if the frame is periodic enough to be voiced at all.
    let strongest = maxima.iter().map(|&t| nsdf[t]).fold(f32::MIN, f32::max);
    if strongest < MPM_MIN_CLARITY {
        return None;
    }
    let threshold = MPM_CLARITY * strongest;
    let chosen = *maxima.iter().find(|&&t| nsdf[t] >= threshold)?;

    let refined = parabolic_vertex(&nsdf, chosen);
    let f0 = sr / refined;
    (range.min_freq..=range.max_freq)
        .contains(&f0)
        .then_some(f0)
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
    range: PitchRange,
    n_notes: usize,
    /// Fundamental frequency of each note (dictionary column).
    freqs: Vec<f32>,
    /// `n_notes` columns, each a length-`n_bins` magnitude template.
    columns: Vec<Vec<f32>>,
    /// `DᵀD` (`n_notes × n_notes`), the constant part of the update denominator.
    dtd: Vec<Vec<f32>>,
}

/// Build the harmonic-template dictionary for a given spectrum size / rate /
/// pitch range.
fn build_nmf_dict(sample_rate: u32, n_bins: usize, range: PitchRange) -> NmfDict {
    // Rebuilt only when the range/rate/bin-count actually goes stale (see the
    // `Nmf` match arm above), but that can still land on a frame the player is
    // actively playing through (e.g. a song-start range change) — worth its
    // own span to distinguish a genuine hitch here from the steady-state
    // per-chunk cost.
    let _span = info_span!("build_nmf_dict", n_bins).entered();
    let nyquist = sample_rate as f32 / 2.0;
    // magnitudes span DC..Nyquist over n_bins, so each bin is this wide.
    let freq_res = nyquist / n_bins.max(1) as f32;

    let midi_lo = (12.0 * (range.min_freq / 440.0).log2() + 69.0).ceil() as i32;
    let midi_hi = (12.0 * (range.max_freq / 440.0).log2() + 69.0).floor() as i32;

    let mut freqs = Vec::new();
    let mut columns: Vec<Vec<f32>> = Vec::new();
    for midi in midi_lo..=midi_hi {
        let f = crate::audio_system::midi::midi_to_freq_hz(midi as f32);
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
        range,
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
            freq_to_note(f).map(|(midi, note, octave)| PitchInfo {
                midi,
                note,
                octave,
                frequency: f,
            })
        })
        .collect()
}

/// Rounds `freq` to the nearest MIDI semitone, returning `(midi, note_name,
/// octave)` — `None` outside the valid MIDI range (0-127), which also
/// catches non-positive/nonsensical input rather than producing a bogus
/// octave number.
fn freq_to_note(freq: f32) -> Option<(u8, String, i32)> {
    if freq <= 0.0 {
        return None;
    }
    let midi = 12.0 * (freq / 440.0).log2() + 69.0;
    let midi_rounded = midi.round() as i32;
    if !(0..=127).contains(&midi_rounded) {
        return None;
    }
    let octave = midi_rounded / 12 - 1;
    let note_idx = (midi_rounded % 12) as usize;
    Some((midi_rounded as u8, NOTE_NAMES[note_idx].to_string(), octave))
}

#[cfg(test)]
mod tests;
