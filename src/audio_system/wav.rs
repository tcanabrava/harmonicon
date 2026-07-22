// SPDX-License-Identifier: MIT

//! Minimal mono 16-bit PCM WAV encoding, shared by every in-app synthesis
//! feature that needs to hand Bevy's audio system a real `AudioSource`
//! (the song editor's preview/practice playback, the Bending Trainer's
//! reference-tone "Listen" button, ...).

/// Encode `samples` (mono, in `[-1.0, 1.0]`) as a 16-bit PCM WAV file.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    // WAV/RIFF layout per the PCM subset of the WAVE spec:
    //   RIFF chunk (12 bytes) + fmt subchunk (24 bytes) + data subchunk header (8 bytes)
    //   = 44 bytes of header, followed by raw 16-bit LE PCM samples.
    let channels: u16 = 1; // mono
    let bits_per_sample: u16 = 16; // 16-bit PCM
    let bytes_per_sample = (bits_per_sample / 8) as u32;
    let data_len = (samples.len() as u32) * bytes_per_sample;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample;
    let block_align = (channels as u32 * bytes_per_sample) as u16;

    let mut v = Vec::with_capacity(44 + data_len as usize);
    // RIFF chunk descriptor
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes()); // total file size minus 8
    v.extend_from_slice(b"WAVE");
    // fmt subchunk (16 bytes for PCM)
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes()); // subchunk size for PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = PCM (no compression)
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&byte_rate.to_le_bytes());
    v.extend_from_slice(&block_align.to_le_bytes());
    v.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data subchunk
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let q = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        v.extend_from_slice(&q.to_le_bytes());
    }
    v
}

/// Decodes a 16-bit PCM WAV file — the format [`encode_wav`] itself
/// produces — into `(samples, channels, sample_rate)`, normalizing samples
/// to `[-1.0, 1.0]`. `None` for anything that isn't `RIFF`/`WAVE`/16-bit
/// PCM. Deliberately not a general-purpose WAV decoder (no float/24-bit/
/// ADPCM support): just enough to read back what this module writes, for
/// `audio_system::waveform::analyze_wav_waveform`'s progress-bar analysis of
/// a Song Editor MIDI import's synthesized `song/music.wav` backing track.
pub fn decode_wav_pcm16(bytes: &[u8]) -> Option<(Vec<f32>, u16, u32)> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12;
    let mut channels = 1u16;
    let mut sample_rate = 44_100u32;
    let mut bits_per_sample = 16u16;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body_start = pos + 8;
        let body_end = (body_start + size).min(bytes.len());
        let body = &bytes[body_start..body_end];
        match id {
            b"fmt " if body.len() >= 16 => {
                channels = u16::from_le_bytes(body[2..4].try_into().ok()?);
                sample_rate = u32::from_le_bytes(body[4..8].try_into().ok()?);
                bits_per_sample = u16::from_le_bytes(body[14..16].try_into().ok()?);
            }
            b"data" => data = Some(body),
            _ => {}
        }
        // Chunks are word-aligned; an odd-sized chunk has one pad byte.
        pos = body_start + size + (size % 2);
    }
    if bits_per_sample != 16 {
        return None;
    }
    let samples = data?
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect();
    Some((samples, channels.max(1), sample_rate.max(1)))
}

/// Resamples mono `samples` from `from_rate` to `to_rate` by straight linear
/// interpolation — deliberately simple (no anti-aliasing low-pass filter),
/// so it's not broadcast-quality, but is plenty for tooling that just wants
/// every output file at a fixed rate regardless of whatever a capture
/// device happened to use (`song_editor::debug_record`'s WAV dump). A no-op
/// copy when the rates already match, or when there's nothing to resample.
pub fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if samples.is_empty() || from_rate == 0 || from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = (samples.len() as f64 * ratio).round() as usize;
    (0..out_len)
        .map(|i| {
            let src_pos = i as f64 / ratio;
            let i0 = src_pos.floor() as usize;
            let frac = (src_pos - i0 as f64) as f32;
            let s0 = samples.get(i0).copied().unwrap_or(0.0);
            let s1 = samples.get(i0 + 1).copied().unwrap_or(s0);
            s0 + (s1 - s0) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_data_length_match_sample_count() {
        let samples = vec![0.0f32; 100];
        let wav = encode_wav(&samples, 44_100);
        assert_eq!(wav.len(), 44 + 100 * 2);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn clamps_out_of_range_samples() {
        let wav = encode_wav(&[2.0, -2.0], 44_100);
        let s0 = i16::from_le_bytes([wav[44], wav[45]]);
        let s1 = i16::from_le_bytes([wav[46], wav[47]]);
        assert_eq!(s0, i16::MAX);
        assert_eq!(s1, -i16::MAX);
    }

    #[test]
    fn decode_wav_pcm16_round_trips_encode_wav() {
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let wav = encode_wav(&samples, 22_050);
        let (decoded, channels, sample_rate) = decode_wav_pcm16(&wav).unwrap();
        assert_eq!(channels, 1);
        assert_eq!(sample_rate, 22_050);
        assert_eq!(decoded.len(), samples.len());
        for (a, b) in decoded.iter().zip(&samples) {
            assert!((a - b).abs() < 1e-3, "{a} != {b}");
        }
    }

    #[test]
    fn decode_wav_pcm16_rejects_non_riff_bytes() {
        assert!(decode_wav_pcm16(b"not a wav file at all").is_none());
    }

    #[test]
    fn decode_wav_pcm16_rejects_a_truncated_header() {
        assert!(decode_wav_pcm16(b"RIFF").is_none());
    }

    // ── resample_linear ──────────────────────────────────────────────────────

    #[test]
    fn matching_rates_are_a_no_op() {
        let samples = vec![0.1, 0.2, -0.3];
        assert_eq!(resample_linear(&samples, 44_100, 44_100), samples);
    }

    #[test]
    fn upsampling_doubles_the_length_for_a_2x_ratio() {
        let samples = vec![0.0; 100];
        assert_eq!(resample_linear(&samples, 24_000, 48_000).len(), 200);
    }

    #[test]
    fn downsampling_halves_the_length_for_a_half_ratio() {
        let samples = vec![0.0; 100];
        assert_eq!(resample_linear(&samples, 48_000, 24_000).len(), 50);
    }

    #[test]
    fn interpolates_between_neighbouring_samples() {
        // 0.0 -> 1.0 over 2 input samples at 2x: the midpoint output sample
        // should land halfway between them.
        let samples = vec![0.0, 1.0];
        let out = resample_linear(&samples, 1, 2);
        assert!((out[1] - 0.5).abs() < 1e-6, "{:?}", out);
    }

    #[test]
    fn empty_input_stays_empty() {
        assert!(resample_linear(&[], 44_100, 48_000).is_empty());
    }
}
