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
    let channels: u16 = 1;            // mono
    let bits_per_sample: u16 = 16;    // 16-bit PCM
    let bytes_per_sample = (bits_per_sample / 8) as u32;
    let data_len  = (samples.len() as u32) * bytes_per_sample;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample;
    let block_align = (channels as u32 * bytes_per_sample) as u16;

    let mut v = Vec::with_capacity(44 + data_len as usize);
    // RIFF chunk descriptor
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes()); // total file size minus 8
    v.extend_from_slice(b"WAVE");
    // fmt subchunk (16 bytes for PCM)
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());            // subchunk size for PCM
    v.extend_from_slice(&1u16.to_le_bytes());             // AudioFormat = PCM (no compression)
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
}
