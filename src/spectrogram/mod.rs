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

use crate::audio_system::pitch_detect::LiveSpectrum;
use crate::menu::AppState;

/// Number of frequency bands the spectrum is reduced to.
pub const NUM_BANDS: usize = 32;

// Display range — the harmonica's useful band.
const MIN_FREQ: f32 = 150.0;
const MAX_FREQ: f32 = 4000.0;

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

/// Reduces a precomputed magnitude spectrum (half, DC..Nyquist, `freq_res` Hz per
/// bin) to `NUM_BANDS` log-spaced, per-frame-normalized band levels in 0..1.
/// Empty input (silence) yields all-zero bands. Reuses the audio pipeline's FFT.
pub fn bands_from_magnitudes(magnitudes: &[f32], freq_res: f32) -> Vec<f32> {
    let half = magnitudes.len();
    if half == 0 || freq_res <= 0.0 {
        return vec![0.0; NUM_BANDS];
    }

    // Log-spaced band edges; each band takes the peak magnitude in its range.
    let ratio = MAX_FREQ / MIN_FREQ;
    let mut bands = vec![0.0f32; NUM_BANDS];
    for (b, slot) in bands.iter_mut().enumerate() {
        let lo = MIN_FREQ * ratio.powf(b as f32 / NUM_BANDS as f32);
        let hi = MIN_FREQ * ratio.powf((b + 1) as f32 / NUM_BANDS as f32);
        let bin_lo = ((lo / freq_res) as usize).max(1).min(half - 1);
        let bin_hi = ((hi / freq_res).ceil() as usize).clamp(bin_lo + 1, half);
        let mut peak = 0.0f32;
        for &m in &magnitudes[bin_lo..bin_hi] {
            peak = peak.max(m);
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

/// Reduces the published [`LiveSpectrum`] to bands and smooths them into
/// [`Spectrum`] (fast attack, slow decay) so the visualization rises sharply and
/// falls gracefully.
fn analyze_audio(live: Res<LiveSpectrum>, time: Res<Time>, mut spectrum: ResMut<Spectrum>) {
    let target = bands_from_magnitudes(&live.magnitudes, live.freq_res);

    let dt = time.delta_secs();
    let attack = 1.0 - (-dt * 30.0).exp();
    let decay = 1.0 - (-dt * 8.0).exp();
    for (cur, &tgt) in spectrum.bands.iter_mut().zip(target.iter()) {
        let k = if tgt > *cur { attack } else { decay };
        *cur += (tgt - *cur) * k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band_index_for(freq: f32) -> usize {
        (NUM_BANDS as f32 * (freq / MIN_FREQ).ln() / (MAX_FREQ / MIN_FREQ).ln()) as usize
    }

    #[test]
    fn empty_spectrum_yields_zero_bands() {
        let bands = bands_from_magnitudes(&[], 0.0);
        assert_eq!(bands.len(), NUM_BANDS);
        assert!(bands.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn peak_lights_the_expected_band() {
        // A magnitude spectrum with a single spike at the 440 Hz bin should make
        // the band covering 440 Hz the loudest.
        let freq_res = 10.0; // Hz per bin
        let half = 512; // covers up to 5120 Hz
        let mut mags = vec![0.001f32; half];
        mags[(440.0 / freq_res) as usize] = 1.0;

        let bands = bands_from_magnitudes(&mags, freq_res);
        let loudest = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let expected = band_index_for(440.0);
        assert!(
            (loudest as i32 - expected as i32).abs() <= 1,
            "loudest band {loudest}, expected ~{expected}"
        );
    }
}
