//! Audio spectrogram visualizations.
//!
//! The base here owns the shared analysis: it turns the latest captured audio
//! into a normalized, smoothed set of frequency `bands` (the [`Spectrum`]
//! resource). Individual visualizations (bars, fluid, circular, …) are pluggable
//! — each lives in its own sub-module with a spawn function and an update system
//! gated on the active [`SpectrogramStyle`]. Only the bar style is implemented;
//! adding another means: a sub-module, a `SpectrogramStyle` variant, a `spawn_*`
//! arm below, and registering its update system in [`SpectrogramPlugin`].

mod bars;

use bevy::prelude::*;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

use crate::audio_system::audio_input::LatestSamples;
use crate::menu::AppState;

/// Number of frequency bands the spectrum is reduced to.
pub const NUM_BANDS: usize = 32;

// Analysis range — the harmonica's useful band (matches pitch_detect).
const MIN_FREQ: f32 = 150.0;
const MAX_FREQ: f32 = 4000.0;
const SILENCE_RMS: f32 = 0.005;

/// Latest normalized (0..1) magnitude per frequency band, smoothed over time.
#[derive(Resource)]
pub struct Spectrum {
    pub bands: Vec<f32>,
}

impl Default for Spectrum {
    fn default() -> Self {
        Self { bands: vec![0.0; NUM_BANDS] }
    }
}

/// Which visualization is currently shown. Switch this to swap renderers.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SpectrogramStyle {
    #[default]
    Bars,
    // Fluid,    // not yet implemented
    // Circular, // not yet implemented
}

/// Spawns the active visualization as a child of `parent`, filling its box.
pub fn spawn_spectrogram(parent: &mut ChildSpawnerCommands, style: SpectrogramStyle) {
    match style {
        SpectrogramStyle::Bars => bars::spawn(parent),
    }
}

pub struct SpectrogramPlugin;

impl Plugin for SpectrogramPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Spectrum>()
            .init_resource::<SpectrogramStyle>()
            .add_systems(Update, analyze_audio.run_if(in_state(AppState::Playing)))
            .add_systems(
                Update,
                bars::update_bars.run_if(
                    in_state(AppState::Playing)
                        .and_then(|s: Res<SpectrogramStyle>| *s == SpectrogramStyle::Bars),
                ),
            );
    }
}

// ── Analysis ────────────────────────────────────────────────────────────────

/// Reduces a block of samples to `NUM_BANDS` log-spaced, per-frame-normalized
/// band levels in 0..1. Returns all-zero during silence. Pure except for the
/// reusable FFT plan in `state`.
pub fn compute_bands(samples: &[f32], sample_rate: u32, state: &mut FftState) -> Vec<f32> {
    let n = samples.len();
    if n < 2 || sample_rate == 0 {
        return vec![0.0; NUM_BANDS];
    }

    let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / n as f32).sqrt();
    if rms < SILENCE_RMS {
        return vec![0.0; NUM_BANDS];
    }

    if state.last_size != n {
        state.plan = Some(state.planner.plan_fft_forward(n));
        state.last_size = n;
    }
    let plan = state.plan.as_ref().unwrap();

    // Hanning window then forward FFT.
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
    let freq_res = sample_rate as f32 / n as f32;

    // Log-spaced band edges; each band takes the peak magnitude in its range.
    let ratio = MAX_FREQ / MIN_FREQ;
    let mut bands = vec![0.0f32; NUM_BANDS];
    for (b, slot) in bands.iter_mut().enumerate() {
        let lo = MIN_FREQ * ratio.powf(b as f32 / NUM_BANDS as f32);
        let hi = MIN_FREQ * ratio.powf((b + 1) as f32 / NUM_BANDS as f32);
        let bin_lo = ((lo / freq_res) as usize).max(1);
        let bin_hi = ((hi / freq_res).ceil() as usize).clamp(bin_lo + 1, half);
        let mut peak = 0.0f32;
        for c in &buffer[bin_lo..bin_hi] {
            peak = peak.max(c.norm());
        }
        *slot = peak;
    }

    // Per-frame normalize to the spectral shape, with a gamma to lift detail.
    let max = bands.iter().cloned().fold(0.0f32, f32::max);
    if max > 1e-9 {
        for v in &mut bands {
            *v = (*v / max).powf(0.6);
        }
    }
    bands
}

/// Smooths the freshly computed bands into [`Spectrum`] (fast attack, slow
/// decay) so the visualization rises sharply and falls gracefully.
fn analyze_audio(
    latest: Option<Res<LatestSamples>>,
    time: Res<Time>,
    mut spectrum: ResMut<Spectrum>,
    mut state: Local<FftState>,
) {
    let target = match latest {
        Some(l) if !l.samples.is_empty() => compute_bands(&l.samples, l.sample_rate, &mut state),
        _ => vec![0.0; NUM_BANDS],
    };

    let dt = time.delta_secs();
    let attack = 1.0 - (-dt * 30.0).exp();
    let decay = 1.0 - (-dt * 8.0).exp();
    for (cur, &tgt) in spectrum.bands.iter_mut().zip(target.iter()) {
        let k = if tgt > *cur { attack } else { decay };
        *cur += (tgt - *cur) * k;
    }
}

// ── Reusable FFT plan (mirrors pitch_detect::FftState) ───────────────────────

pub struct FftState {
    planner: FftPlanner<f32>,
    plan: Option<Arc<dyn Fft<f32>>>,
    last_size: usize,
}

impl Default for FftState {
    fn default() -> Self {
        Self { planner: FftPlanner::new(), plan: None, last_size: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_yields_zero_bands() {
        let mut st = FftState::default();
        let bands = compute_bands(&[0.0; 2048], 44_100, &mut st);
        assert_eq!(bands.len(), NUM_BANDS);
        assert!(bands.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn tone_peaks_in_the_expected_band() {
        // 440 Hz sine should light a band, and the strongest band should sit at
        // the log position for 440 Hz within [MIN_FREQ, MAX_FREQ].
        let sr = 44_100u32;
        let n = 4096;
        let samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        let mut st = FftState::default();
        let bands = compute_bands(&samples, sr, &mut st);

        let loudest = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let expected = (NUM_BANDS as f32
            * (440.0f32 / MIN_FREQ).ln()
            / (MAX_FREQ / MIN_FREQ).ln()) as usize;
        assert!(
            (loudest as i32 - expected as i32).abs() <= 1,
            "loudest band {loudest}, expected ~{expected}"
        );
    }
}
