// SPDX-License-Identifier: MIT

use harmonicon_core::wav::decode_wav_pcm16;
use harmonicon_dsp::{CHUNK_SIZE, FftState, HOP_SIZE, PitchAlgorithm, PitchRange, analyze_stream};

#[test]
fn tracks_octave_doubled_notes_as_a_recorded_guitar_chord_rings() {
    let (samples, _, rate) =
        decode_wav_pcm16(include_bytes!("fixtures/guitarset-e-major.wav")).unwrap();
    let mut state = FftState::default();
    let mut four_notes = 0;
    let mut five_notes = 0;
    for (i, window) in samples.windows(CHUNK_SIZE).step_by(HOP_SIZE).enumerate() {
        let end = i * HOP_SIZE + CHUNK_SIZE;
        let analysis = analyze_stream(
            window,
            rate,
            &mut state,
            PitchAlgorithm::JitMpe,
            PitchRange::from_freqs([82.41, 1046.5], 1.),
            None,
            end as u64,
        );
        let mut heard: Vec<_> = analysis.pitches.iter().map(|pitch| pitch.midi).collect();
        heard.sort_unstable();
        let t = end as f32 / rate as f32;
        if (0.237..0.773).contains(&t) && heard == [40, 47, 52, 56] {
            four_notes += 1;
        }
        if t >= 0.773 && heard == [40, 47, 52, 56, 59] {
            five_notes += 1;
        }
    }
    assert!(four_notes >= 4, "four-note voicing sustained for {four_notes} exact frames");
    assert!(five_notes >= 2, "added B3 detected for {five_notes} exact frames");
}
