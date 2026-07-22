// SPDX-License-Identifier: MIT

//! The microphone pipeline's per-frame driver: reads chunks off the capture
//! channel, runs pitch detection, and publishes the results as a
//! [`PitchEvent`] message plus the shared [`AudioFrame`] resource
//! (visualizers reuse its FFT/waveform data instead of re-analysing).

use bevy::prelude::*;

use crate::settings::AudioSettings;

use super::audio_input;
use super::pitch_detect::{self, AudioFrame, PitchEvent, PitchRange};

/// Dev-only ("--features dev") raw-audio tap for `song_editor`'s "Debug
/// Recording" checkbox (`song_editor::debug_record`): accumulates the exact,
/// non-overlapping mono audio the mic captured while `recording` is set, so
/// a pitch-detection miss can be diagnosed against exactly what the mic
/// heard rather than just what the detector reported for it. Lives here
/// (not in `song_editor`) for the same reason `AudioFrame` does — a generic
/// resource in the audio pipeline that a higher-level feature taps, rather
/// than a second consumer of `AudioCapture::receiver` (each chunk only ever
/// goes to *one* receiver, so a second reader would steal chunks from this
/// one instead of seeing a copy).
#[cfg(feature = "dev")]
#[derive(Resource, Default)]
pub struct RawCaptureBuffer {
    pub recording: bool,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub fn process_audio(
    capture: Option<Res<audio_input::AudioCapture>>,
    settings: Res<AudioSettings>,
    range: Res<PitchRange>,
    mut writer: MessageWriter<PitchEvent>,
    mut frame: ResMut<AudioFrame>,
    mut fft: Local<pitch_detect::FftState>,
    #[cfg(feature = "dev")] mut raw_capture: Option<ResMut<RawCaptureBuffer>>,
) {
    let Some(capture) = capture else { return };
    while let Ok(samples) = capture.receiver.try_recv() {
        // Chunks arrive with 50% overlap (see `audio_input::push_chunks`), so
        // more than one can land in a single frame — a span per chunk (rather
        // than relying solely on the automatic per-system span this whole
        // function already gets) shows how many ran and how long each took.
        let _span = info_span!("process_audio_chunk", samples = samples.len()).entered();
        // One FFT per chunk for the spectrum; pitches use the chosen algorithm.
        let analysis = pitch_detect::analyze(
            &samples,
            capture.sample_rate,
            &mut fft,
            settings.pitch_algorithm,
            *range,
        );
        writer.write(PitchEvent(analysis.pitches));
        // Publish the frame so visualizers reuse this FFT (freq) or the raw
        // waveform (time) without re-analysing.
        frame.magnitudes = analysis.magnitudes;
        frame.freq_res = analysis.freq_res;

        // Only the newly-captured hop of each chunk (the first chunk in
        // full, every later one just its second half) goes into the debug
        // buffer — otherwise the 50% overlap above would duplicate half of
        // every chunk into a stuttering recording.
        #[cfg(feature = "dev")]
        if let Some(raw) = raw_capture.as_deref_mut()
            && raw.recording
        {
            raw.sample_rate = capture.sample_rate;
            if raw.samples.is_empty() {
                raw.samples.extend_from_slice(&samples);
            } else {
                let hop = samples.len() / 2;
                raw.samples.extend_from_slice(&samples[samples.len() - hop..]);
            }
        }

        // Recycle the buffer we're about to overwrite back to the capture
        // callback's pool instead of letting it deallocate here — see
        // `audio_input::AudioCapture::free_sender`.
        let previous = std::mem::replace(&mut frame.samples, samples);
        let _ = capture.free_sender.try_send(previous);
    }
}

/// Logs the detected pitches whenever they change during Playing, at
/// `debug` level rather than stdout — a diagnostic aid, not something every
/// player's console should be spammed with (enable with `RUST_LOG=debug` or
/// similar to see it).
pub fn log_pitches(mut reader: MessageReader<PitchEvent>, mut last: Local<Vec<String>>) {
    for event in reader.read() {
        let current: Vec<String> = event
            .0
            .iter()
            .map(|p| format!("{}{} ({:.1}Hz)", p.note, p.octave, p.frequency))
            .collect();

        if current == *last {
            continue;
        }

        if current.is_empty() {
            debug!("pitches: (silence)");
        } else {
            debug!("pitches: {}", current.join("  |  "));
        }
        *last = current;
    }
}
