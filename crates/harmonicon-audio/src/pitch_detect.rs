// SPDX-License-Identifier: MIT

//! The ECS-facing side of pitch detection. The algorithms themselves are in
//! `harmonicon-dsp`, which is Bevy-free so the detection loop can be
//! iterated on without an engine build; this module is what makes their
//! inputs and outputs addressable from a Bevy schedule.

use bevy::prelude::{Deref, DerefMut, Message, Resource};

pub use harmonicon_dsp::{
    Analysis, FftState, PITCH_RANGE_MARGIN_SEMITONES, PitchAlgorithm, PitchInfo, WINDOW_FUNCTION,
    analyze,
};

/// The frequency window pitch detection searches, as an ECS resource.
///
/// A newtype over `harmonicon_dsp::PitchRange` rather than the type itself:
/// `Resource` is Bevy's trait and the inner type is `harmonicon-dsp`'s, so
/// the orphan rule leaves no way to implement one for the other. `Deref`
/// keeps `range.min_freq` reading as before.
#[derive(Resource, Clone, Copy, PartialEq, Debug, Default, Deref, DerefMut)]
pub struct PitchRange(pub harmonicon_dsp::PitchRange);

impl PitchRange {
    /// Same chart-derived construction as the inner type, wrapped.
    pub fn from_freqs(freqs: impl IntoIterator<Item = f32>, margin_semitones: f32) -> Self {
        Self(harmonicon_dsp::PitchRange::from_freqs(freqs, margin_semitones))
    }
}

/// The pitches detected in one analysed block.
#[derive(Message)]
pub struct PitchEvent(pub Vec<PitchInfo>);

/// The latest analysed audio frame, published by the audio pipeline so
/// multiple consumers reuse one FFT: `magnitudes`/`freq_res` for
/// frequency-domain views (spectrogram bars) and `samples` for time-domain
/// views (oscilloscope). Empty vectors mean silence / no audio.
#[derive(Resource, Default)]
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub magnitudes: Vec<f32>,
    pub freq_res: f32,
}
