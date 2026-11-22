// SPDX-License-Identifier: MIT
use super::*;
use harmonicon_audio::{
    audio_input::{CHUNK_SIZE, HOP_SIZE},
    pitch_detect::{FftState, analyze_stream},
};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[test]
#[ignore = "run scripts/guitar_chord_trainer_benchmark.py to supply the recordings"]
fn recorded_open_chords() {
    let dir = PathBuf::from(std::env::var_os("HARMONICON_CHORD_RECORDINGS").unwrap());
    let cases: BTreeMap<String, Option<String>> =
        serde_json::from_slice(&fs::read(dir.join("chords.json")).unwrap()).unwrap();
    let guitar = Guitar::standard(GuitarBody::Acoustic);
    let (lo, hi) = guitar.range().unwrap();
    let range = PitchRange::from_freqs(
        [lo, hi].map(|m| midi_to_freq_hz(f32::from(m))),
        PITCH_RANGE_MARGIN_SEMITONES,
    );
    let settings = BendingTrainerSettings::default();
    let targets: Vec<_> = SHAPES.iter().map(|s| s.pitches(&guitar)).collect();
    let (mut hits, mut positives, mut false_accepts, mut negatives) = (0, 0, 0, 0);
    for (id, label) in cases {
        let wav = fs::read(dir.join(format!("{id}.wav"))).unwrap();
        let (samples, channels, rate) = harmonicon_core::wav::decode_wav_pcm16(&wav).unwrap();
        assert_eq!((channels, rate), (1, 44100));
        let mut fft = FftState::default();
        let mut trials: Vec<_> = targets.iter().map(|_| ChordPractice::default()).collect();
        for (i, chunk) in samples.windows(CHUNK_SIZE).step_by(HOP_SIZE).enumerate() {
            let frame = analyze_stream(
                chunk,
                rate,
                &mut fft,
                PitchAlgorithm::JitMpe,
                range.0,
                None,
                (i * HOP_SIZE + CHUNK_SIZE) as u64,
            );
            let heard: Vec<_> = frame
                .pitches
                .iter()
                .map(|p| 69. + 12. * (p.frequency / settings.a4_hz).log2())
                .collect();
            for (trial, target) in trials.iter_mut().zip(&targets) {
                trial.listen(
                    target,
                    &heard,
                    HOP_SIZE as f32 / rate as f32,
                    settings.tolerance_cents,
                    settings.hold_secs,
                    true,
                );
            }
        }
        for (shape, trial) in SHAPES.iter().zip(trials) {
            let accepted = trial.landed > 0;
            if Some(shape.name) == label.as_deref() {
                positives += 1;
                hits += usize::from(accepted);
                if !accepted {
                    println!("Missed {id} {}", shape.name);
                }
            } else {
                negatives += 1;
                false_accepts += usize::from(accepted);
                if accepted {
                    println!("Accepted {id} {label:?} as {}", shape.name);
                }
            }
        }
    }
    println!(
        "Accepted {hits}/{positives} target strums and {false_accepts}/{negatives} wrong targets"
    );
    assert_eq!((positives, negatives), (17, 423), "pinned corpus coverage");
    assert!(hits >= 9, "regression in recognizing target strums");
    assert!(false_accepts <= 1, "regression in rejecting wrong targets");
}
