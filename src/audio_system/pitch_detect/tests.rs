// SPDX-License-Identifier: MIT

use super::*;
use std::f32::consts::PI;

// ── PitchAlgorithm::label / from_label ────────────────────────────────────

#[test]
fn every_algorithm_label_round_trips() {
    for &algo in PitchAlgorithm::all() {
        assert_eq!(PitchAlgorithm::from_label(algo.label()), Some(algo));
    }
}

#[test]
fn unknown_label_is_none() {
    assert_eq!(PitchAlgorithm::from_label("nonsense"), None);
}

#[test]
fn silence_returns_empty() {
    let samples = vec![0.0f32; 4096];
    let mut state = FftState::default();
    assert!(detect_pitches(&samples, 44100, &mut state).is_empty());
}

#[test]
fn too_short_returns_empty() {
    let mut state = FftState::default();
    assert!(detect_pitches(&[], 44100, &mut state).is_empty());
    assert!(detect_pitches(&[0.5], 44100, &mut state).is_empty());
}

#[test]
fn sine_440hz_detected_as_a4() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let mut state = FftState::default();
    let pitches = detect_pitches(&samples, sample_rate, &mut state);
    assert!(!pitches.is_empty(), "expected at least one pitch");
    let a4 = pitches.iter().find(|p| p.note == "A" && p.octave == 4);
    assert!(a4.is_some(), "expected A4, got {:?}", pitches);
}

#[test]
fn harmonic_is_suppressed() {
    // 880 Hz is 2× 440 Hz and should be removed as a harmonic.
    let peaks = vec![(440.0f32, 1.0f32), (880.0, 0.5)];
    let result = suppress_harmonics(&peaks);
    assert_eq!(result.len(), 1);
    assert!((result[0].0 - 440.0).abs() < 1.0);
}

#[test]
fn non_harmonic_peaks_both_kept() {
    // 440 Hz (A4) and 659 Hz (E5) are not harmonically related.
    let peaks = vec![(440.0f32, 1.0f32), (659.0, 0.8)];
    let result = suppress_harmonics(&peaks);
    assert_eq!(result.len(), 2);
}

#[test]
fn yin_detects_440hz() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let f0 = yin_pitch(&samples, sample_rate, PitchRange::default()).expect("expected a pitch");
    assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
    assert_eq!(freq_to_note(f0), Some((69, "A".to_string(), 4)));
}

#[test]
fn yin_rejects_silence_and_noise() {
    // Flat silence: no period.
    assert_eq!(
        yin_pitch(&vec![0.0f32; 4096], 44100, PitchRange::default()),
        None
    );
}

#[test]
fn pyin_detects_440hz() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let f0 = pyin_pitch(&samples, sample_rate, PitchRange::default()).expect("expected a pitch");
    assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
}

#[test]
fn pyin_rejects_silence() {
    assert_eq!(
        pyin_pitch(&vec![0.0f32; 4096], 44100, PitchRange::default()),
        None
    );
}

#[test]
fn mpm_detects_440hz() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let f0 = mpm_pitch(&samples, sample_rate, PitchRange::default()).expect("expected a pitch");
    assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
}

#[test]
fn mpm_rejects_silence() {
    assert_eq!(
        mpm_pitch(&vec![0.0f32; 4096], 44100, PitchRange::default()),
        None
    );
}

#[test]
fn mpm_rejects_unpitched_noise() {
    // Deterministic white-ish noise (LCG), loud enough that only the
    // absolute clarity floor — not the RMS silence gate — can reject it.
    // Models breath noise into the mic, which must not read as a note.
    let mut seed = 0x12345678u32;
    let samples: Vec<f32> = (0..4096)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
        })
        .collect();
    assert_eq!(mpm_pitch(&samples, 44100, PitchRange::default()), None);
}

// Render the FFT magnitude spectrum of a sum of sine tones, the way
// `analyze` would, so the NMF detector can be tested directly.
fn magnitudes_of(freqs: &[f32], sample_rate: u32, n: usize) -> Vec<f32> {
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            freqs
                .iter()
                .map(|&f| 0.4 * (2.0 * PI * f * i as f32 / sample_rate as f32).sin())
                .sum()
        })
        .collect();
    let mut state = FftState::default();
    analyze(
        &samples,
        sample_rate,
        &mut state,
        PitchAlgorithm::Fft,
        PitchRange::default(),
    )
    .magnitudes
}

#[test]
fn nmf_detects_a_two_note_chord() {
    let sample_rate = 44100u32;
    let n = 4096;
    // A4 (440) + C#5 (554.37): a major third.
    let mags = magnitudes_of(&[440.0, 554.37], sample_rate, n);
    let dict = build_nmf_dict(sample_rate, mags.len(), PitchRange::default());
    let pitches = nmf_pitches(&mags, &dict);
    assert!(
        pitches.iter().any(|p| p.note == "A" && p.octave == 4),
        "expected A4, got {pitches:?}"
    );
    assert!(
        pitches.iter().any(|p| p.note == "C#" && p.octave == 5),
        "expected C#5, got {pitches:?}"
    );
}

#[test]
fn nmf_single_tone_is_one_note() {
    let sample_rate = 44100u32;
    let mags = magnitudes_of(&[440.0], sample_rate, 4096);
    let dict = build_nmf_dict(sample_rate, mags.len(), PitchRange::default());
    let pitches = nmf_pitches(&mags, &dict);
    assert!(
        pitches.iter().any(|p| p.note == "A" && p.octave == 4),
        "expected A4 among {pitches:?}"
    );
}

#[test]
fn all_algorithms_agree_on_a_clean_tone() {
    // A 330 Hz tone (E4) should read the same through every detector.
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> = (0..n)
        .map(|i| 0.4 * (2.0 * PI * 330.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let mut state = FftState::default();
    for algo in PitchAlgorithm::all() {
        let pitches = analyze(
            &samples,
            sample_rate,
            &mut state,
            *algo,
            PitchRange::default(),
        )
        .pitches;
        assert!(
            pitches.iter().any(|p| p.note == "E" && p.octave == 4),
            "{:?} should detect E4, got {:?}",
            algo,
            pitches
        );
    }
}

#[test]
fn yin_via_analyze_uses_selected_algorithm() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();
    let mut state = FftState::default();
    let pitches = analyze(
        &samples,
        sample_rate,
        &mut state,
        PitchAlgorithm::Yin,
        PitchRange::default(),
    )
    .pitches;
    assert_eq!(pitches.len(), 1, "YIN is monophonic: one pitch");
    assert_eq!(pitches[0].note, "A");
    assert_eq!(pitches[0].octave, 4);
}

// ── PitchRange::from_freqs ───────────────────────────────────────────────

#[test]
fn pitch_range_from_freqs_falls_back_to_default_when_empty() {
    assert_eq!(
        PitchRange::from_freqs(std::iter::empty(), 1.0),
        PitchRange::default()
    );
}

#[test]
fn pitch_range_from_freqs_spans_min_and_max_with_margin() {
    // G3 (~196 Hz) to G6 (~1568 Hz), the range of a low-G 10-hole diatonic.
    let range = PitchRange::from_freqs([196.0, 1568.0], 1.0);
    // A semitone below G3 / above G6.
    assert!(
        range.min_freq < 196.0,
        "min {} should be below 196",
        range.min_freq
    );
    assert!(
        range.max_freq > 1568.0,
        "max {} should be above 1568",
        range.max_freq
    );
    // And within ~1 semitone (ratio 2^(1/12) ≈ 1.0595) of the source notes.
    assert!((range.min_freq - 196.0 / 2f32.powf(1.0 / 12.0)).abs() < 0.01);
    assert!((range.max_freq - 1568.0 * 2f32.powf(1.0 / 12.0)).abs() < 0.01);
}

#[test]
fn pitch_range_from_freqs_lets_a_low_g_harp_hole_1_blow_through() {
    // Hole-1 blow on a key-of-G diatonic is G3 ≈ 196 Hz, below the
    // default detector floor of 200 Hz — this harp needs its own
    // derived range, not the fixed default.
    let range = PitchRange::from_freqs([196.0, 1568.0], 1.0);
    assert!(range.min_freq < 196.0);
    assert!(
        196.0 < PitchRange::default().min_freq,
        "sanity: below the default fixed floor"
    );
}

#[test]
fn freq_to_note_a440() {
    assert_eq!(freq_to_note(440.0), Some((69, "A".to_string(), 4)));
}

#[test]
fn freq_to_note_middle_c() {
    // C4 ≈ 261.63 Hz
    assert_eq!(freq_to_note(261.63), Some((60, "C".to_string(), 4)));
}

#[test]
fn freq_to_note_zero_or_negative_returns_none() {
    assert_eq!(freq_to_note(0.0), None);
    assert_eq!(freq_to_note(-1.0), None);
}

#[test]
fn freq_to_note_rejects_absurdly_high_frequencies() {
    // Comfortably beyond MIDI 127 (~12.5kHz) — a bogus detection, not a
    // real harmonica note; must not silently produce a garbage octave.
    assert_eq!(freq_to_note(50_000.0), None);
}
