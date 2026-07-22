// SPDX-License-Identifier: MIT

//! Decodes a whole song's audio into a coarse peak-amplitude waveform, for
//! display on the gameplay progress bar (`gameplay::song_progress_overlay`).
//! Runs once, at song-asset load time — see `song::loader` — so gameplay
//! setup just reads the finished `Vec<f32>` off `SongManifest`.

use std::io::Cursor;

use bevy::log::info_span;
use rodio::Source;

/// How many bars the waveform is reduced to — independent of song length or
/// screen width; the display just divides its width evenly among them.
pub const WAVEFORM_BUCKETS: usize = 300;

/// Downmixes interleaved multi-channel samples to mono by averaging each
/// frame. `channels <= 1` returns the input unchanged (already mono).
fn downmix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Splits mono samples into `buckets` equal-width windows and takes the peak
/// absolute amplitude (0..1) in each. Empty input yields an all-zero
/// waveform of the requested length; a zero bucket count yields an empty one.
pub fn bucket_peaks(samples: &[f32], buckets: usize) -> Vec<f32> {
    if buckets == 0 || samples.is_empty() {
        return vec![0.0; buckets];
    }
    let len = samples.len();
    (0..buckets)
        .map(|i| {
            let start = i * len / buckets;
            let end = ((i + 1) * len / buckets).max(start + 1).min(len);
            samples[start..end]
                .iter()
                .fold(0.0f32, |peak, &s| peak.max(s.abs()))
                .clamp(0.0, 1.0)
        })
        .collect()
}

/// Decodes an in-memory audio file (the song's `music.ogg`) into a
/// `buckets`-wide peak-amplitude waveform, plus the file's real duration in
/// seconds — the timescale the waveform is laid out on; anything positioned
/// over it should use this same duration. Returns an all-zero, zero-duration
/// waveform on a decode failure rather than erroring.
pub fn analyze_ogg_waveform(bytes: &[u8], buckets: usize) -> (Vec<f32>, f64) {
    // Two call sites, both invisible to Bevy's per-system spans: the async
    // `SongChartLoader` (runs on the AssetServer's IO task pool, entirely
    // outside the ECS schedule) and `song_editor::waveform`'s synchronous
    // main-thread decode (inside a system, but a whole-file decode is exactly
    // the kind of hot inner-loop work worth breaking out from that system's
    // own total time).
    let _span = info_span!("analyze_ogg_waveform", bytes = bytes.len()).entered();
    let Ok(decoder) = rodio::Decoder::new(Cursor::new(bytes.to_vec())) else {
        return (vec![0.0; buckets], 0.0);
    };
    let channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get() as f64;
    let mono = downmix_to_mono(&decoder.collect::<Vec<f32>>(), channels);
    let duration_secs = mono.len() as f64 / sample_rate;
    (bucket_peaks(&mono, buckets), duration_secs)
}

/// Same as [`analyze_ogg_waveform`], but for a `song/music.wav` backing
/// track — the Song Editor's MIDI import writes one of these (a synthesized
/// mixdown, see `song_editor::midi_import::render_backing_pcm`) since the
/// engine can't play a raw MIDI file and no OGG encoder is in the
/// dependency tree. Uses [`crate::audio_system::wav::decode_wav_pcm16`]
/// rather than `rodio::Decoder` (whose WAV support isn't enabled — see
/// `Cargo.toml`'s comment on the `rodio` dependency) since the only WAV
/// files this ever needs to read are ones this same codebase wrote.
pub fn analyze_wav_waveform(bytes: &[u8], buckets: usize) -> (Vec<f32>, f64) {
    // Same off-schedule/hot-loop reasoning as `analyze_ogg_waveform` above.
    let _span = info_span!("analyze_wav_waveform", bytes = bytes.len()).entered();
    let Some((samples, channels, sample_rate)) = crate::audio_system::wav::decode_wav_pcm16(bytes)
    else {
        return (vec![0.0; buckets], 0.0);
    };
    let mono = downmix_to_mono(&samples, channels as usize);
    let duration_secs = mono.len() as f64 / sample_rate as f64;
    (bucket_peaks(&mono, buckets), duration_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_leaves_mono_untouched() {
        let samples = [0.1, -0.2, 0.3];
        assert_eq!(downmix_to_mono(&samples, 1), samples);
    }

    #[test]
    fn downmix_averages_interleaved_channels() {
        // Stereo: left=1.0, right=-1.0 should average to 0.0 in every frame.
        let samples = [1.0, -1.0, 0.5, -0.5];
        assert_eq!(downmix_to_mono(&samples, 2), vec![0.0, 0.0]);
    }

    #[test]
    fn bucket_peaks_empty_input_is_all_zero() {
        assert_eq!(bucket_peaks(&[], 4), vec![0.0; 4]);
    }

    #[test]
    fn bucket_peaks_zero_buckets_is_empty() {
        assert!(bucket_peaks(&[0.1, 0.2], 0).is_empty());
    }

    #[test]
    fn bucket_peaks_finds_the_loudest_sample_per_window() {
        // Four windows of two samples each; the peak (abs) of each pair.
        let samples = [0.1, -0.9, 0.2, 0.3, -0.4, 0.05, 0.6, 0.6];
        let peaks = bucket_peaks(&samples, 4);
        assert_eq!(peaks.len(), 4);
        assert!((peaks[0] - 0.9).abs() < 1e-6);
        assert!((peaks[1] - 0.3).abs() < 1e-6);
        assert!((peaks[2] - 0.4).abs() < 1e-6);
        assert!((peaks[3] - 0.6).abs() < 1e-6);
    }

    #[test]
    fn bucket_peaks_covers_every_sample_even_when_buckets_dont_divide_evenly() {
        // 7 samples into 3 buckets shouldn't drop the tail samples.
        let samples = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.9];
        let peaks = bucket_peaks(&samples, 3);
        assert_eq!(peaks.len(), 3);
        assert!((peaks[2] - 0.9).abs() < 1e-6);
    }

    #[test]
    fn analyze_ogg_waveform_degrades_gracefully_on_bad_bytes() {
        // Not a real ogg file — decoding fails, so we get a flat, sized
        // waveform back (and a zero duration) instead of a panic or an error
        // the caller must handle.
        let (waveform, duration) = analyze_ogg_waveform(b"not an ogg file", 16);
        assert_eq!(waveform, vec![0.0; 16]);
        assert_eq!(duration, 0.0);
    }

    #[test]
    fn analyze_wav_waveform_reads_a_real_wav_and_its_duration() {
        let samples: Vec<f32> = (0..44_100)
            .map(|i| ((i % 100) as f32 / 50.0) - 1.0)
            .collect();
        let wav = crate::audio_system::wav::encode_wav(&samples, 44_100);
        let (waveform, duration) = analyze_wav_waveform(&wav, 8);
        assert_eq!(waveform.len(), 8);
        assert!((duration - 1.0).abs() < 1e-6);
        assert!(waveform.iter().any(|&p| p > 0.0), "should be audible");
    }

    #[test]
    fn analyze_wav_waveform_degrades_gracefully_on_bad_bytes() {
        let (waveform, duration) = analyze_wav_waveform(b"not a wav file", 16);
        assert_eq!(waveform, vec![0.0; 16]);
        assert_eq!(duration, 0.0);
    }
}
