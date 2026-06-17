// SPDX-License-Identifier: MIT

//! Audio spectrogram visualizations.
//!
//! The base here owns the shared analysis: it turns the latest captured audio
//! into both a frequency view (normalized, smoothed `bands`) and a time view
//! (a triggered `waveform`) in the [`Spectrum`] resource. Individual
//! visualizations (bars, oscilloscope, fluid, circular, …) are pluggable — each
//! lives in its own sub-module with a `spawn` function and an `update` system
//! gated on the active [`SpectrogramStyle`]. Press **V** to cycle styles.
//!
//! Adding another means: a sub-module, a `SpectrogramStyle` variant, an arm in
//! [`spawn_content`], and registering its update system in [`SpectrogramPlugin`].

mod bars;
mod oscilloscope;

use bevy::prelude::*;
use bevy::ui_render::prelude::UiMaterialPlugin;

use crate::audio_system::pitch_detect::AudioFrame;
use crate::menu::AppState;

pub use oscilloscope::OscMaterial;
use oscilloscope::OscilloscopeMaterial;

/// Number of frequency bands the spectrum is reduced to.
pub const NUM_BANDS: usize = 32;
/// Number of points sampled for the oscilloscope trace.
pub const WAVE_POINTS: usize = 128;

// Display range — the harmonica's useful band.
const MIN_FREQ: f32 = 150.0;
const MAX_FREQ: f32 = 4000.0;
// Peak amplitude below which the input is treated as silence.
const SILENCE_PEAK: f32 = 0.01;

/// Analysis ready for rendering: per-band levels (0..1, smoothed) for frequency
/// views, and a normalized, trigger-aligned waveform (-1..1) for time views.
#[derive(Resource)]
pub struct Spectrum {
    pub bands: Vec<f32>,
    pub waveform: Vec<f32>,
}

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            bands: vec![0.0; NUM_BANDS],
            waveform: vec![0.0; WAVE_POINTS],
        }
    }
}

/// Which visualization is currently shown. Cycle with [`SpectrogramStyle::next`].
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SpectrogramStyle {
    #[default]
    Bars,
    Oscilloscope,
    // Fluid,    // not yet implemented
    // Circular, // not yet implemented
}

impl SpectrogramStyle {
    fn next(self) -> Self {
        match self {
            Self::Bars => Self::Oscilloscope,
            Self::Oscilloscope => Self::Bars,
        }
    }
}

/// Marks the container that holds the active visualization, so it can be rebuilt
/// in place when the style is switched.
#[derive(Component)]
struct SpectrogramRoot;

/// Spawns the spectrogram (a filling container hosting the active style) as a
/// child of `parent`. `osc` is the shared oscilloscope material handle.
pub fn spawn_spectrogram(
    parent: &mut ChildSpawnerCommands,
    style: SpectrogramStyle,
    osc: &Handle<OscilloscopeMaterial>,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            SpectrogramRoot,
        ))
        .with_children(|root| spawn_content(root, style, osc));
}

/// Builds the active style's nodes into `parent`.
fn spawn_content(
    parent: &mut ChildSpawnerCommands,
    style: SpectrogramStyle,
    osc: &Handle<OscilloscopeMaterial>,
) {
    match style {
        SpectrogramStyle::Bars => bars::spawn(parent),
        SpectrogramStyle::Oscilloscope => oscilloscope::spawn(parent, osc),
    }
}

pub struct SpectrogramPlugin;

impl Plugin for SpectrogramPlugin {
    fn build(&self, app: &mut App) {
        let playing = || in_state(AppState::Playing);
        app.add_plugins(UiMaterialPlugin::<OscilloscopeMaterial>::default())
            .init_resource::<Spectrum>()
            .init_resource::<SpectrogramStyle>()
            .add_systems(Startup, oscilloscope::init_material)
            .add_systems(Update, analyze_audio.run_if(in_state(AppState::Playing)))
            .add_systems(
                Update,
                switch_visualization.run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                bars::update_bars.run_if(
                    playing().and_then(|s: Res<SpectrogramStyle>| *s == SpectrogramStyle::Bars),
                ),
            )
            .add_systems(
                Update,
                oscilloscope::update_scope.run_if(
                    playing()
                        .and_then(|s: Res<SpectrogramStyle>| *s == SpectrogramStyle::Oscilloscope),
                ),
            );
    }
}

/// Cycles the visualization style on **V** and rebuilds the spectrogram in place.
fn switch_visualization(
    keys: Res<ButtonInput<KeyCode>>,
    mut style: ResMut<SpectrogramStyle>,
    osc: Res<OscMaterial>,
    roots: Query<(Entity, Option<&Children>), With<SpectrogramRoot>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    *style = style.next();
    let style = *style; // Copy, so each closure below can capture it freely.
    let handle = osc.0.clone();
    for (root, children) in &roots {
        if let Some(children) = children {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        let handle = handle.clone();
        commands
            .entity(root)
            .with_children(move |c| spawn_content(c, style, &handle));
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

/// Decimates a block of samples into a `points`-long, trigger-aligned waveform
/// in -1..1 (auto-gained to fill). Triggering on the first rising zero crossing
/// keeps the trace stationary between frames. Silence yields a flat (zero) line.
pub fn compute_waveform(samples: &[f32], points: usize) -> Vec<f32> {
    if samples.len() < points * 2 {
        return vec![0.0; points];
    }
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    if peak < SILENCE_PEAK {
        return vec![0.0; points];
    }

    // Trigger: first rising zero crossing in the first half of the block.
    let mut start = 0;
    for i in 1..samples.len() / 2 {
        if samples[i - 1] < 0.0 && samples[i] >= 0.0 {
            start = i;
            break;
        }
    }

    let window = &samples[start..];
    let step = (window.len() / points).max(1);
    let gain = 1.0 / peak; // auto-gain so the trace fills vertically
    (0..points)
        .map(|i| (window[(i * step).min(window.len() - 1)] * gain).clamp(-1.0, 1.0))
        .collect()
}

/// Smooth one band level toward `target`: fast `attack` when rising, slow `decay`
/// when falling, so bands jump up with the sound and ease back down. `attack` and
/// `decay` are per-frame interpolation factors in 0..1.
fn smooth_toward(current: f32, target: f32, attack: f32, decay: f32) -> f32 {
    let k = if target > current { attack } else { decay };
    current + (target - current) * k
}

/// Reduces the published [`AudioFrame`] into [`Spectrum`]: bands are smoothed
/// (fast attack, slow decay) so they rise sharply and fall gracefully; the
/// waveform is taken instantaneously (the trigger keeps it steady). Reuses the
/// audio pipeline's FFT via [`AudioFrame`] — it never runs its own.
fn analyze_audio(frame: Res<AudioFrame>, time: Res<Time<Real>>, mut spectrum: ResMut<Spectrum>) {
    let target = bands_from_magnitudes(&frame.magnitudes, frame.freq_res);

    let dt = time.delta_secs();
    let attack = 1.0 - (-dt * 30.0).exp();
    let decay = 1.0 - (-dt * 8.0).exp();
    for (cur, &tgt) in spectrum.bands.iter_mut().zip(target.iter()) {
        *cur = smooth_toward(*cur, tgt, attack, decay);
    }

    spectrum.waveform = compute_waveform(&frame.samples, WAVE_POINTS);
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

    #[test]
    fn silence_yields_flat_waveform() {
        let wave = compute_waveform(&[0.0; 4096], WAVE_POINTS);
        assert_eq!(wave.len(), WAVE_POINTS);
        assert!(wave.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn waveform_is_triggered_and_auto_gained() {
        let sr = 44_100.0;
        let samples: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr).sin() * 0.25)
            .collect();
        let wave = compute_waveform(&samples, WAVE_POINTS);
        assert_eq!(wave.len(), WAVE_POINTS);
        // Trigger is a rising zero crossing, so the trace starts near zero rising.
        assert!(
            wave[0].abs() < 0.2,
            "starts near the trigger, got {}",
            wave[0]
        );
        assert!(wave[1] >= wave[0], "rising after the trigger");
        // Auto-gain: a quarter-scale sine should still reach near full deflection.
        let peak = wave.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!(peak > 0.8, "auto-gain should fill the trace, peak {peak}");
    }

    // ── style switching (the V key) ───────────────────────────────────────────

    #[test]
    fn style_next_cycles_through_all_styles() {
        // V cycles forward and returns to the start — every style is reachable and
        // the cycle never gets stuck on one.
        let start = SpectrogramStyle::default();
        assert_eq!(start, SpectrogramStyle::Bars);
        assert_eq!(start.next(), SpectrogramStyle::Oscilloscope);
        assert_eq!(start.next().next(), start, "cycle returns to the start");
    }

    // ── band smoothing (reacts, doesn't freeze) ───────────────────────────────

    #[test]
    fn smoothing_rises_fast_and_falls_slow() {
        // Same distance, but attack > decay: rising moves further than falling.
        let up = smooth_toward(0.0, 1.0, 0.5, 0.1);
        let down = smooth_toward(1.0, 0.0, 0.5, 0.1);
        assert!((up - 0.5).abs() < 1e-6, "attack moves halfway up, got {up}");
        assert!(
            (down - 0.9).abs() < 1e-6,
            "decay eases down slowly, got {down}"
        );
    }

    #[test]
    fn smoothing_converges_toward_the_target() {
        // Repeated frames keep moving toward the target (the bars never freeze).
        let mut v = 0.0;
        let mut prev = -1.0;
        for _ in 0..20 {
            v = smooth_toward(v, 1.0, 0.3, 0.1);
            assert!(v > prev, "must keep rising toward the target");
            prev = v;
        }
        assert!(
            v > 0.9,
            "approaches the target after enough frames, got {v}"
        );
    }

    // ── analyze_audio: reacts to the shared frame, reuses its FFT ──────────────

    #[test]
    fn analyze_audio_reacts_to_the_shared_audio_frame() {
        use std::time::Duration;

        // A published frame: a magnitude spike (no FFT done here — the system
        // consumes the pipeline's spectrum) plus a sine block for the waveform.
        let freq_res = 10.0;
        let mut magnitudes = vec![0.001f32; 512];
        magnitudes[(440.0 / freq_res) as usize] = 1.0;
        let samples: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f32::consts::PI * 200.0 * i as f32 / 44_100.0).sin() * 0.3)
            .collect();

        let mut world = World::new();
        world.insert_resource(AudioFrame {
            samples,
            magnitudes,
            freq_res,
        });
        world.insert_resource(Spectrum::default());
        let mut t = Time::<Real>::default();
        t.advance_by(Duration::from_millis(16));
        world.insert_resource(t);

        let mut schedule = Schedule::default();
        schedule.add_systems(analyze_audio);
        schedule.run(&mut world);

        let spectrum = world.resource::<Spectrum>();
        // Bands moved off zero toward the spike (reacts to the captured audio).
        assert!(
            spectrum.bands.iter().any(|&b| b > 0.0),
            "bands should react to the published spectrum"
        );
        // The waveform was filled from the frame's samples.
        assert!(
            spectrum.waveform.iter().any(|&v| v != 0.0),
            "waveform should react to the published samples"
        );
    }
}
