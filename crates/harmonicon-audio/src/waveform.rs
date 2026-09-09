// SPDX-License-Identifier: MIT

//! Decodes a whole song's audio into a coarse peak-amplitude waveform, for
//! display on the gameplay progress bar (`gameplay::song_progress_overlay`).
//! Runs once, at song-asset load time — see `song::loader` — so gameplay
//! setup just reads the finished `Vec<f32>` off `SongManifest`.

use std::{io::Cursor, sync::Arc};

use bevy::log::info_span;
use rodio::Source;

/// How many bars the waveform is reduced to — independent of song length or
/// screen width; the display just divides its width evenly among them.
pub const WAVEFORM_BUCKETS: usize = 300;

/// Downmixes interleaved multi-channel samples to mono by averaging each
/// frame. `channels <= 1` returns the input unchanged (already mono).
#[cfg(test)]
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
    let bytes: Arc<[u8]> = bytes.into();
    let Ok(decoder) = rodio::Decoder::new(Cursor::new(bytes.clone())) else {
        return (vec![0.0; buckets], 0.0);
    };
    let channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get() as f64;
    // Count first so buckets cover the exact duration, even when container
    // metadata has no duration. Both passes retain only decoder working memory.
    let frames = decoder.count() / channels;
    let Ok(decoder) = rodio::Decoder::new(Cursor::new(bytes)) else {
        return (vec![0.0; buckets], 0.0);
    };
    (
        stream_peaks(decoder, channels, frames, buckets),
        frames as f64 / sample_rate,
    )
}

/// Same as [`analyze_ogg_waveform`], but for a `song/music.wav` backing
/// track — the Song Editor's MIDI import writes one of these (a synthesized
/// mixdown, see `song_editor::midi_import::render_backing_pcm`) since the
/// engine can't play a raw MIDI file and no OGG encoder is in the
/// dependency tree. Uses [`harmonicon_core::wav::decode_wav_pcm16`]
/// rather than `rodio::Decoder` (whose WAV support isn't enabled — see
/// `Cargo.toml`'s comment on the `rodio` dependency) since the only WAV
/// files this ever needs to read are ones this same codebase wrote.
pub fn analyze_wav_waveform(bytes: &[u8], buckets: usize) -> (Vec<f32>, f64) {
    // Same off-schedule/hot-loop reasoning as `analyze_ogg_waveform` above.
    let _span = info_span!("analyze_wav_waveform", bytes = bytes.len()).entered();
    let Some((data, channels, sample_rate)) = harmonicon_core::wav::wav_pcm16_data(bytes) else {
        return (vec![0.0; buckets], 0.0);
    };
    let frames = data.len() / (usize::from(channels) * 2);
    let samples = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b) as f32 / 32768.0);
    (
        stream_peaks(samples, usize::from(channels), frames, buckets),
        frames as f64 / f64::from(sample_rate),
    )
}

/// Reduce an interleaved stream with O(buckets + channels) working memory.
fn stream_peaks(
    mut samples: impl Iterator<Item = f32>,
    channels: usize,
    frames: usize,
    buckets: usize,
) -> Vec<f32> {
    if frames == 0 || channels == 0 {
        return vec![0.0; buckets];
    }
    let mut consumed = 0;
    let mut last = 0.0f32;
    (0..buckets)
        .map(|i| {
            let start = i * frames / buckets;
            let end = ((i + 1) * frames / buckets).max(start + 1).min(frames);
            // More buckets than frames repeats the corresponding sample, just
            // like bucket_peaks, without buffering the whole decoded stream.
            let mut peak = if start < consumed { last.abs() } else { 0.0 };
            while consumed < end {
                last = (0..channels)
                    .map(|_| samples.next().unwrap_or(0.0))
                    .sum::<f32>()
                    / channels as f32;
                peak = peak.max(last.abs());
                consumed += 1;
            }
            peak.clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_matches_reference_for_short_uneven_and_multichannel_input() {
        for frames in [0, 1, 2, 7, 31, 1000] {
            for channels in [1, 2, 6] {
                let samples: Vec<f32> = (0..frames * channels)
                    .map(|i| (i % 17) as f32 / 8.0 - 1.0)
                    .collect();
                for buckets in [0, 1, 3, 16, 300] {
                    let expected = bucket_peaks(&downmix_to_mono(&samples, channels), buckets);
                    assert_eq!(
                        stream_peaks(samples.iter().copied(), channels, frames, buckets),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn long_stream_needs_no_full_song_buffer() {
        let peaks = stream_peaks(std::iter::repeat_n(0.5, 2_000_000), 2, 1_000_000, 300);
        assert_eq!(peaks, vec![0.5; 300]);
    }

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
        let wav = harmonicon_core::wav::encode_wav(&samples, 44_100);
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
