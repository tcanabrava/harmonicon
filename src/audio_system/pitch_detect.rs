use bevy::prelude::Message;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;

// Harmonica range: roughly C4 (262 Hz) to the top of a 10-hole diatonic.
const MIN_FREQ: f32 = 200.0;
const MAX_FREQ: f32 = 2500.0;

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

pub fn detect_pitches(samples: &[f32], sample_rate: u32, state: &mut FftState) -> Vec<PitchInfo> {
    let n = samples.len();
    if n < 2 {
        return vec![];
    }

    let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / n as f32).sqrt();
    if rms < SILENCE_THRESHOLD {
        return vec![];
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
            let w =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
            Complex::new(s * w, 0.0)
        })
        .collect();

    plan.process(&mut buffer);

    let half = n / 2;
    let magnitudes: Vec<f32> = buffer[..half].iter().map(|c| c.norm()).collect();

    let max_mag = magnitudes.iter().cloned().fold(0.0f32, f32::max);
    if max_mag < 1e-9 {
        return vec![];
    }

    let threshold = max_mag * PEAK_THRESHOLD_RATIO;
    let freq_res = sample_rate as f32 / n as f32;

    let min_bin = (MIN_FREQ / freq_res) as usize;
    let max_bin = ((MAX_FREQ / freq_res) as usize).min(half.saturating_sub(2));

    // Collect local maxima — use parabolic interpolation for sub-bin accuracy.
    let mut raw_peaks: Vec<(f32, f32)> = Vec::new();
    for i in min_bin.max(1)..=max_bin {
        if magnitudes[i] > magnitudes[i - 1]
            && magnitudes[i] > magnitudes[i + 1]
            && magnitudes[i] > threshold
        {
            let freq = parabolic_peak(&magnitudes, i, freq_res);
            raw_peaks.push((freq, magnitudes[i]));
        }
    }

    // Sort by magnitude descending so fundamental candidates come first.
    raw_peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    suppress_harmonics(&raw_peaks)
        .into_iter()
        .filter_map(|(freq, _)| freq_to_note(freq).map(|(note, octave)| PitchInfo { note, octave, frequency: freq }))
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
