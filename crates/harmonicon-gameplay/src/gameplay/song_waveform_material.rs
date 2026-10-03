// SPDX-License-Identifier: MIT

//! The song-progress bar's audio waveform: a continuous, smoothed, mirrored
//! silhouette drawn by a UI material shader (one node, not one per bucket) —
//! the same "pack N samples into a `[Vec4; N/4]` uniform, let the shader
//! interpolate between them" pattern `spectrogram::oscilloscope` already
//! uses for the pitch-detector's live trace, adapted from a signed line
//! trace to an unsigned, mirrored, filled envelope. Unlike that trace, the
//! song waveform is static for the whole song — the uniform is written once
//! at spawn time, never updated per frame.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

use harmonicon_audio::waveform::WAVEFORM_BUCKETS;

/// Amplitudes packed four-per-`Vec4` for the uniform array.
const PACKED: usize = WAVEFORM_BUCKETS / 4;

/// UI material fed the song's precomputed waveform buckets; the shader
/// renders them as a smoothed, mirrored envelope instead of a bar chart.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SongWaveformMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub amplitudes: [Vec4; PACKED],
}

impl UiMaterial for SongWaveformMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/song_waveform.wesl".into()
    }
}

/// Packs `waveform` (0..1 peak amplitudes, `song::loader` always analyzes
/// exactly [`WAVEFORM_BUCKETS`] of them when there's any music to analyze,
/// but a music-less song, per `SongManifest::music: None`, hands this an
/// empty slice) into the fixed-size uniform array — floored, so silence
/// still reads as a continuous shape rather than a gap. Deliberately no
/// data-side smoothing beyond that floor: a moving average rounds the
/// silhouette off until it stops reading as an audio waveform. The
/// shader's inter-bucket interpolation (`song_waveform.wesl`) is the only
/// smoothing.
pub fn pack_amplitudes(waveform: &[f32], floor: f32) -> [Vec4; PACKED] {
    let mut amplitudes = [Vec4::ZERO; PACKED];
    for (i, &amplitude) in waveform.iter().take(WAVEFORM_BUCKETS).enumerate() {
        amplitudes[i / 4][i % 4] = amplitude.clamp(0.0, 1.0).max(floor);
    }
    amplitudes
}

/// `pub(super)` — folded into `song_progress_overlay::SongProgressPlugin`
/// (see that module), the same "material's own plugin, folded into its
/// single consumer's plugin" shape `music_score::tie_material::
/// TieMaterialPlugin` already uses.
pub(super) struct SongWaveformMaterialPlugin;

impl Plugin for SongWaveformMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<SongWaveformMaterial>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_amplitudes_floors_silence() {
        let packed = pack_amplitudes(&[0.0; WAVEFORM_BUCKETS], 0.04);
        assert_eq!(packed[0][0], 0.04);
    }

    #[test]
    fn pack_amplitudes_clamps_and_preserves_loud_values() {
        let mut waveform = vec![0.0; WAVEFORM_BUCKETS];
        waveform[1] = 0.8;
        waveform[5] = 5.0; // out-of-range input should clamp to 1.0
        let packed = pack_amplitudes(&waveform, 0.04);
        assert!((packed[0][1] - 0.8).abs() < 1e-6);
        assert_eq!(packed[1][1], 1.0);
    }

    #[test]
    fn pack_amplitudes_handles_a_shorter_than_expected_slice() {
        // A music-less song's manifest never analyzed any waveform data.
        let packed = pack_amplitudes(&[], 0.04);
        assert_eq!(packed[0][0], 0.0);
    }
}
