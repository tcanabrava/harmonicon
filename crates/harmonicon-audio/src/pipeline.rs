// SPDX-License-Identifier: MIT

//! The microphone pipeline's per-frame driver: reads chunks off the capture
//! channel, runs pitch detection, and publishes the results as a
//! [`PitchEvent`] message plus the shared [`AudioFrame`] resource
//! (visualizers reuse its FFT/waveform data instead of re-analysing).

use bevy::prelude::*;

use crate::AudioSettings;

use super::audio_input;
#[cfg(feature = "dev")]
use super::pitch_detect::PitchAlgorithm;
use super::pitch_detect::{self, AudioFrame, PitchEvent, PitchRange};

/// Dev-only ("--features dev") raw-audio tap for `song_editor`'s "Debug
/// Recording" checkbox (`song_editor::debug_record`): accumulates the
/// exact, non-overlapping mono audio the mic captured while `recording` is
/// set, so a pitch-detection miss can be diagnosed against what the mic
/// actually heard, not just what the detector reported. Lives here (not in
/// `song_editor`), same as `AudioFrame` — a second consumer of
/// `AudioCapture::receiver` would steal chunks from this one instead of
/// seeing a copy, since each chunk only ever goes to one receiver.
#[cfg(feature = "dev")]
#[derive(Resource, Default)]
pub struct RawCaptureBuffer {
    pub recording: bool,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Which algorithm `settings.pitch_algorithm` was set to while
    /// capturing — refreshed every chunk like `sample_rate`, so a
    /// detection miss can be reproduced against the exact algorithm that
    /// missed it, not just guessed at.
    pub algorithm: PitchAlgorithm,
    /// The detected-pitch log for this take: (seconds since the take's
    /// first sample, the same formatted note labels `log_pitches` prints)
    /// — one entry per *change*, not per chunk, so a multi-minute take
    /// doesn't produce one entry per ~46ms chunk. Cleared alongside
    /// `samples` at the start of a fresh take (`song_editor::
    /// debug_record::sync_raw_capture`).
    pub detected_notes: Vec<(f32, Vec<String>)>,
}

pub fn process_audio(
    capture: Option<Res<audio_input::AudioCapture>>,
    status: Option<Res<audio_input::MicStatus>>,
    time: Res<Time<Real>>,
    mut last_received: Local<Option<std::time::Duration>>,
    settings: Res<AudioSettings>,
    range: Res<PitchRange>,
    mut writer: MessageWriter<PitchEvent>,
    mut frame: ResMut<AudioFrame>,
    mut fft: Local<pitch_detect::FftState>,
    #[cfg(feature = "dev")] mut raw_capture: Option<ResMut<RawCaptureBuffer>>,
) {
    let connected = status.as_ref().is_none_or(|status| status.is_connected());
    if capture.is_none() || !connected {
        if let Some(capture) = capture.as_ref() {
            for samples in capture.receiver.try_iter() {
                let _ = capture.free_sender.try_send(samples);
            }
        }
        *last_received = None;
        clear_detection(&mut frame, &mut writer);
        return;
    }
    let capture = capture.unwrap();
    while let Ok(samples) = capture.receiver.try_recv() {
        *last_received = Some(time.elapsed());
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
            **range,
        );
        // Placed before `analysis.pitches` moves into the `PitchEvent` below
        // — this needs to read it first.
        #[cfg(feature = "dev")]
        if let Some(raw) = raw_capture.as_deref_mut()
            && raw.recording
        {
            raw.sample_rate = capture.sample_rate;
            raw.algorithm = settings.pitch_algorithm;
            // Only the newly-captured hop of each chunk (the first chunk in
            // full, every later one just its second half) goes into the
            // debug buffer — otherwise the 50% overlap above would
            // duplicate half of every chunk into a stuttering recording.
            if raw.samples.is_empty() {
                raw.samples.extend_from_slice(&samples);
            } else {
                let hop = samples.len() / 2;
                raw.samples
                    .extend_from_slice(&samples[samples.len() - hop..]);
            }
            let elapsed = raw.samples.len() as f32 / raw.sample_rate.max(1) as f32;
            let current: Vec<String> = analysis
                .pitches
                .iter()
                .map(|p| format!("{}{} ({:.1}Hz)", p.note, p.octave, p.frequency))
                .collect();
            if raw.detected_notes.last().map(|(_, n)| n) != Some(&current) {
                raw.detected_notes.push((elapsed, current));
            }
        }

        writer.write(PitchEvent(analysis.pitches));
        // Publish the frame so visualizers reuse this FFT (freq) or the raw
        // waveform (time) without re-analysing.
        frame.magnitudes = analysis.magnitudes;
        frame.freq_res = analysis.freq_res;

        // Recycle the buffer we're about to overwrite back to the capture
        // callback's pool instead of letting it deallocate here — see
        // `audio_input::AudioCapture::free_sender`.
        let previous = std::mem::replace(&mut frame.samples, samples);
        let _ = capture.free_sender.try_send(previous);
    }
    if last_received.is_none_or(|last| time.elapsed().saturating_sub(last) >= CAPTURE_TIMEOUT) {
        clear_detection(&mut frame, &mut writer);
    }
}

const CAPTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

fn clear_detection(frame: &mut AudioFrame, writer: &mut MessageWriter<PitchEvent>) {
    frame.samples.clear();
    frame.magnitudes.clear();
    frame.freq_res = 0.0;
    // Publish silence even while disconnected so a consumer resuming after
    // a pause cannot retain an old pitch whose clear message has expired.
    writer.write(PitchEvent(Vec::new()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    fn app() -> (App, crossbeam_channel::Sender<Vec<f32>>) {
        let mut app = App::new();
        let (tx, receiver) = bounded(4);
        let (free_sender, _) = bounded(4);
        let (_, errors) = bounded(4);
        app.insert_resource(audio_input::AudioCapture {
            receiver,
            free_sender,
            errors,
            sample_rate: 44100,
            device_name: "test".into(),
        })
        .insert_resource(audio_input::MicStatus::Connected {
            device_name: "test".into(),
        })
        .init_resource::<Time<Real>>()
        .init_resource::<AudioSettings>()
        .init_resource::<PitchRange>()
        .init_resource::<AudioFrame>()
        .add_message::<PitchEvent>()
        .add_systems(Update, process_audio);
        (app, tx)
    }

    fn assert_silent(app: &mut App) {
        let frame = app.world().resource::<AudioFrame>();
        assert!(frame.samples.is_empty());
        assert!(frame.magnitudes.is_empty());
        let messages: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PitchEvent>>()
            .drain()
            .collect();
        assert!(messages.last().unwrap().0.is_empty());
    }

    #[test]
    fn failed_capture_discards_queued_audio_and_publishes_silence() {
        let (mut app, tx) = app();
        tx.send(vec![0.1; 4096]).unwrap();
        app.world_mut()
            .insert_resource(audio_input::MicStatus::Failed {
                reason: "unplugged".into(),
            });
        app.update();
        assert_silent(&mut app);
        assert!(tx.is_empty());
    }

    #[test]
    fn capture_timeout_clears_frame_and_recovers_on_new_audio() {
        let (mut app, tx) = app();
        tx.send(vec![0.0; 4096]).unwrap();
        app.update();
        assert_eq!(app.world().resource::<AudioFrame>().samples.len(), 4096);
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(CAPTURE_TIMEOUT);
        app.update();
        assert_silent(&mut app);
        tx.send(vec![0.0; 4096]).unwrap();
        app.update();
        assert_eq!(app.world().resource::<AudioFrame>().samples.len(), 4096);
    }

    #[test]
    fn missing_capture_repeatedly_publishes_silence_for_resuming_consumers() {
        let (mut app, _) = app();
        app.world_mut()
            .remove_resource::<audio_input::AudioCapture>();
        for _ in 0..4 {
            app.update();
            assert_silent(&mut app);
        }
    }
}
