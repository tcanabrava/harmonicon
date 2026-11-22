// SPDX-License-Identifier: MIT
//! Streaming inference for the released JIT-MPE S/seed 1 immediate presence
//! head. Both asymmetric windows end at the current frame. Their durations
//! and log-frequency bands are expressed in physical units for native mic rates.

mod network;

use super::{PitchInfo, PitchRange, freq_to_note};
use harmonicon_core::midi::midi_to_freq_hz;
use network::Network;
use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use std::{f64::consts::PI, sync::Arc};

struct Window {
    weights: Vec<f32>,
    bands: Vec<(usize, Vec<f32>)>,
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex32>,
    scratch: Vec<Complex32>,
    magnitudes: Vec<f32>,
    bin_hz: f32,
}

impl Window {
    fn new(rate: u32, length_at_48k: usize, fall_ms: u32, planner: &mut FftPlanner<f32>) -> Self {
        let n = (length_at_48k as f64 * rate as f64 / 48000.).round() as usize;
        let fall = (fall_ms as f64 * rate as f64 / 1000.).round() as usize;
        let rise = n - fall;
        let mut weights: Vec<f32> = (0..n)
            .map(|i| {
                if i < rise {
                    (0.5 * (1. - (PI * (i as f64 + 0.5) / rise as f64).cos())) as f32
                } else {
                    (0.5 * (1. + (PI * ((i - rise) as f64 + 0.5) / fall as f64).cos())) as f32
                }
            })
            .collect();
        let sum: f32 = weights.iter().sum();
        for w in &mut weights {
            *w /= sum;
        }
        let nfft = n.next_power_of_two();
        let fft = planner.plan_fft_forward(nfft);
        let bin_hz = rate as f32 / nfft as f32;
        let band_bin = |band: f32| midi_to_freq_hz(21. + (band - 1.) / 3.) / bin_hz;
        let bands = (0..300)
            .map(|band| {
                let center = band_bin(band as f32);
                let lo = band_bin(band as f32 - 1.);
                let hi = band_bin(band as f32 + 1.);
                if center >= (nfft / 2) as f32 {
                    return (0, vec![]);
                }
                if hi - lo <= 2. {
                    let fraction = center.fract();
                    return (center as usize, vec![1. - fraction, fraction]);
                }
                let start = lo.ceil() as usize;
                let mut triangle: Vec<_> = (start..=(hi as usize).min(nfft / 2))
                    .map(|i| ((i as f32 - lo) / (center - lo)).min((hi - i as f32) / (hi - center)))
                    .collect();
                let sum: f32 = triangle.iter().sum();
                for w in &mut triangle {
                    *w /= sum;
                }
                (start, triangle)
            })
            .collect();
        Self {
            weights,
            bands,
            buffer: vec![Complex32::default(); nfft],
            scratch: vec![Complex32::default(); fft.get_inplace_scratch_len()],
            magnitudes: vec![0.; nfft / 2 + 1],
            fft,
            bin_hz,
        }
    }

    fn features(&mut self, history: &[f32], out: &mut [f32]) {
        let input = &history[history.len() - self.weights.len()..];
        for ((bin, x), w) in self.buffer.iter_mut().zip(input).zip(&self.weights) {
            *bin = Complex32::new(x * w, 0.);
        }
        self.buffer[input.len()..].fill(Complex32::default());
        self.fft.process_with_scratch(&mut self.buffer, &mut self.scratch);
        for (mag, bin) in self.magnitudes.iter_mut().zip(&self.buffer) {
            *mag = bin.norm();
        }
        for (feature, (start, weights)) in out.iter_mut().zip(&self.bands) {
            let amplitude: f32 =
                self.magnitudes[*start..].iter().zip(weights).map(|(a, w)| a * w).sum();
            *feature = (1e4 * amplitude).ln_1p();
        }
    }

    // Continuous frequency for the tuner and bends. Use interpolated partial
    // peaks within half a semitone, weighted by their measured amplitudes.
    fn frequency(&self, nominal: f32) -> f32 {
        let mut total = 0.;
        let mut cents = 0.;
        for h in 1..=4 {
            let center = nominal * h as f32 / self.bin_hz;
            let width = (center * 0.0353).max(1.);
            let lo = ((center - width).round() as usize).max(1);
            let hi = ((center + width).round() as usize).min(self.magnitudes.len() - 2);
            let Some(i) =
                (lo..=hi).max_by(|&a, &b| self.magnitudes[a].total_cmp(&self.magnitudes[b]))
            else {
                continue;
            };
            let m = self.magnitudes[i];
            if m < 1.12e-5 || m < self.magnitudes[i - 1] || m < self.magnitudes[i + 1] {
                continue;
            }
            let [a, b, c] = [i - 1, i, i + 1].map(|j| self.magnitudes[j].max(1e-15).ln());
            let curvature = a - 2. * b + c;
            if curvature >= 0. {
                continue;
            }
            let peak = i as f32 + 0.5 * (a - c) / curvature;
            cents += 1200. * (peak / center).log2() * m;
            total += m;
        }
        if total > 0. {
            nominal * 2f32.powf((cents / total).clamp(-49., 49.) / 1200.)
        } else {
            nominal
        }
    }
}

pub(super) struct Detector {
    pub(super) rate: u32,
    windows: [Window; 2],
    history: Vec<f32>,
    pending: Vec<f32>,
    hop: usize,
    network: Network,
    logits: Vec<f32>,
    last_end: Option<u64>,
}

impl Detector {
    pub(super) fn new(rate: u32) -> Self {
        let mut planner = FftPlanner::new();
        let windows =
            [Window::new(rate, 4096, 10, &mut planner), Window::new(rate, 16384, 20, &mut planner)];
        let history = vec![0.; windows[1].weights.len()];
        Self {
            rate,
            windows,
            history,
            pending: vec![],
            hop: (rate as f64 / 50.).round() as usize,
            network: Network::default(),
            logits: vec![],
            last_end: None,
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.last_end = None;
    }

    pub(super) fn analyze(
        &mut self,
        samples: &[f32],
        end: Option<u64>,
        range: PitchRange,
    ) -> Vec<PitchInfo> {
        // The capture queue drops old windows under load. Only reuse history
        // if all samples since the previous endpoint are present in this one.
        let advance = self
            .last_end
            .zip(end)
            .and_then(|(old, new)| new.checked_sub(old))
            .filter(|&n| n <= samples.len() as u64);
        let fresh = if let Some(n) = advance {
            &samples[samples.len() - n as usize..]
        } else {
            self.history.fill(0.);
            self.pending.clear();
            self.network.reset();
            self.logits.clear();
            samples
        };
        self.last_end = end;
        self.pending.extend_from_slice(fresh);
        let mut features = [0.; 600];
        for next in self.pending.chunks_exact(self.hop) {
            self.history.copy_within(self.hop.., 0);
            let start = self.history.len() - self.hop;
            self.history[start..].copy_from_slice(next);
            for (window, out) in self.windows.iter_mut().zip(features.as_chunks_mut::<300>().0) {
                window.features(&self.history, out);
            }
            self.logits = self.network.step(&features);
        }
        self.pending.drain(..self.pending.len() / self.hop * self.hop);
        let mut active: Vec<_> = (0..self.logits.len()).filter(|&i| self.logits[i] >= 0.).collect();
        active.sort_unstable_by(|&a, &b| self.logits[b].total_cmp(&self.logits[a]));
        active
            .into_iter()
            .filter_map(|i| {
                let frequency = self.windows[1].frequency(midi_to_freq_hz((21 + i) as f32));
                if !(range.min_freq..=range.max_freq).contains(&frequency) {
                    return None;
                }
                let (midi, note, octave) = freq_to_note(frequency)?;
                Some(PitchInfo { midi, note, octave, frequency })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_head_matches_upstream_on_streaming_audio() {
        // Calibrated PyTorch logits, S-s1, at 20, 80 and 220 ms. Check both
        // sides of the decision boundary to catch layout and cache mistakes.
        let reference = [
            (1, [-1.19580, -0.59590, -1.08783, -0.93635, -0.81881, -1.67298]),
            (4, [-3.32292, 1.91466, -2.48122, 1.58145, -2.70492, -1.14767]),
            (11, [-5.22189, 3.69075, -3.68834, 2.73159, -4.66380, -1.22146]),
        ];
        let samples: Vec<_> = (0..11 * 882)
            .map(|i| {
                let t = i as f64 / 44100.;
                let phase = 2. * PI * t * 260.;
                ((-2. * t).exp()
                    * (0.6 * phase.sin() + 0.2 * (2. * phase).sin() + 0.1 * (3. * phase).sin())
                    + 0.3 * (2. * PI * 392. * t).sin()) as f32
            })
            .collect();
        let mut d = Detector::new(44100);
        for (frame, expected) in reference {
            let end = frame * 882;
            d.analyze(&samples[..end], Some(end as u64), PitchRange::default());
            for (i, want) in [27, 39, 40, 46, 47, 51].into_iter().zip(expected) {
                assert!(
                    (d.logits[i] - want).abs() < 0.001,
                    "frame {frame}, index {i}: {} vs {want}",
                    d.logits[i]
                );
            }
        }
        let output = d.logits.clone();
        d.analyze(&samples, Some(samples.len() as u64), PitchRange::default());
        assert_eq!(d.logits, output, "duplicate capture");
        d.analyze(&samples[..882], Some(882), PitchRange::default());
        assert!((d.logits[39] - reference[0].1[1]).abs() < 0.001, "capture restart");
    }
}
