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
    let samples: Vec<f32> =
        (0..n).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin()).collect();
    let mut state = FftState::default();
    let pitches = detect_pitches(&samples, sample_rate, &mut state);
    assert!(!pitches.is_empty(), "expected at least one pitch");
    let a4 = pitches.iter().find(|p| p.note == "A" && p.octave == 4);
    assert!(a4.is_some(), "expected A4, got {:?}", pitches);
}

#[test]
fn harmonic_is_suppressed() {
    // 880 Hz is 2× 440 Hz and should be removed as a harmonic.
    let mut peaks = vec![(440.0f32, 1.0f32), (880.0, 0.5)];
    let retained = suppress_harmonics(&mut peaks);
    assert_eq!(retained, 1);
    assert!((peaks[0].0 - 440.0).abs() < 1.0);
}

#[test]
fn non_harmonic_peaks_both_kept() {
    // 440 Hz (A4) and 659 Hz (E5) are not harmonically related.
    let mut peaks = vec![(440.0f32, 1.0f32), (659.0, 0.8)];
    assert_eq!(suppress_harmonics(&mut peaks), 2);
}

#[test]
fn fft_peak_buffer_reuses_capacity_across_chunks() {
    let sr = 44_100;
    let tone = |hz: f32| {
        (0..4096).map(|i| (2.0 * PI * hz * i as f32 / sr as f32).sin()).collect::<Vec<_>>()
    };
    let mut state = FftState::default();
    let range = PitchRange::default();
    let first = analyze(&tone(440.0), sr, &mut state, PitchAlgorithm::Fft, range);
    assert!(first.pitches.iter().any(|p| p.midi == 69));
    let capacity = state.raw_peaks.capacity();
    let second = analyze(&tone(660.0), sr, &mut state, PitchAlgorithm::Fft, range);
    assert!(second.pitches.iter().any(|p| p.midi == 76));
    assert_eq!(state.raw_peaks.capacity(), capacity);
}

#[test]
fn yin_detects_440hz() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> =
        (0..n).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin()).collect();
    let f0 = yin_pitch(&samples, sample_rate, PitchRange::default(), &mut Vec::new())
        .expect("expected a pitch");
    assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
    assert_eq!(freq_to_note(f0), Some((69, "A".to_string(), 4)));
}

#[test]
fn yin_rejects_silence_and_noise() {
    // Flat silence: no period.
    assert_eq!(
        yin_pitch(&vec![0.0f32; 4096], 44100, PitchRange::default(), &mut Vec::new(),),
        None
    );
}

#[test]
fn simd_yin_difference_matches_scalar_reference() {
    let sr = 44_100;
    let range = PitchRange::default();
    let samples: Vec<f32> = (0..4096)
        .map(|i| {
            let t = i as f32 / sr as f32;
            0.6 * (2.0 * PI * 440.0 * t).sin()
                + 0.2 * (2.0 * PI * 880.0 * t).sin()
                + 0.01 * ((i * 73 % 101) as f32 / 50.0 - 1.0)
        })
        .collect();
    let mut simd = Vec::new();
    let (tau_min, tau_max) = yin_cmnd(&samples, sr, range, &mut simd).unwrap();
    let window = samples.len() - tau_max;
    let mut running = 0.0f32;
    for tau in 1..=tau_max {
        let sum = (0..window)
            .map(|j| {
                let diff = samples[j] - samples[j + tau];
                diff * diff
            })
            .sum::<f32>();
        running += sum;
        let scalar = if running > 0.0 { sum * tau as f32 / running } else { 1.0 };
        assert!((simd[tau] - scalar).abs() < 1e-4, "lag {tau}");
    }
    assert!(first_dip_below(&simd, tau_min, tau_max, YIN_THRESHOLD).is_some());
}

#[test]
fn pyin_detects_440hz() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> =
        (0..n).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin()).collect();
    let f0 =
        pyin_pitch(&samples, sample_rate, PitchRange::default(), &mut Vec::new(), &mut Vec::new())
            .expect("expected a pitch");
    assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
}

#[test]
fn pyin_rejects_silence() {
    assert_eq!(
        pyin_pitch(
            &vec![0.0f32; 4096],
            44100,
            PitchRange::default(),
            &mut Vec::new(),
            &mut Vec::new(),
        ),
        None
    );
}

#[test]
fn pyin_reuses_buffers_without_retaining_previous_probabilities() {
    let sr = 44_100;
    let tone = |hz: f32| {
        (0..4096).map(|i| (2.0 * PI * hz * i as f32 / sr as f32).sin()).collect::<Vec<_>>()
    };
    let a = tone(440.0);
    let b = tone(660.0);
    let mut cmnd = Vec::new();
    let mut prob = Vec::new();
    let range = PitchRange::default();
    let first = pyin_pitch(&a, sr, range, &mut cmnd, &mut prob).unwrap();
    let capacity = (cmnd.capacity(), prob.capacity());
    let second = pyin_pitch(&b, sr, range, &mut cmnd, &mut prob).unwrap();
    let again = pyin_pitch(&a, sr, range, &mut cmnd, &mut prob).unwrap();
    assert!((first - 440.0).abs() < 5.0);
    assert!((second - 660.0).abs() < 5.0);
    assert_eq!(again, first);
    assert_eq!((cmnd.capacity(), prob.capacity()), capacity);
}

#[test]
fn mpm_detects_440hz() {
    let sample_rate = 44100u32;
    let n = 4096;
    let samples: Vec<f32> =
        (0..n).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin()).collect();
    let f0 = mpm_pitch(&samples, sample_rate, PitchRange::default(), &mut Vec::new())
        .expect("expected a pitch");
    assert!((f0 - 440.0).abs() < 5.0, "expected ~440 Hz, got {f0}");
}

#[test]
fn mpm_rejects_silence() {
    assert_eq!(
        mpm_pitch(&vec![0.0f32; 4096], 44100, PitchRange::default(), &mut Vec::new(),),
        None
    );
}

#[test]
fn mpm_peak_scan_skips_zero_lag_and_out_of_range_humps() {
    let nsdf = [1.0, 0.5, -0.1, 0.4, 0.5, -0.2, 0.7, 0.6, -0.1, 0.9, 0.8];
    assert_eq!(mpm_key_maxima(&nsdf, 5).collect::<Vec<_>>(), vec![6, 9]);
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
    assert_eq!(mpm_pitch(&samples, 44100, PitchRange::default(), &mut Vec::new()), None);
}

#[test]
fn mpm_reuses_lag_buffer_across_pitches() {
    let sr = 44_100;
    let tone = |hz: f32| {
        (0..4096).map(|i| (2.0 * PI * hz * i as f32 / sr as f32).sin()).collect::<Vec<_>>()
    };
    let mut nsdf = Vec::new();
    let range = PitchRange::default();
    let first = mpm_pitch(&tone(440.0), sr, range, &mut nsdf).unwrap();
    let capacity = nsdf.capacity();
    let second = mpm_pitch(&tone(660.0), sr, range, &mut nsdf).unwrap();
    let again = mpm_pitch(&tone(440.0), sr, range, &mut nsdf).unwrap();
    assert!((first - 440.0).abs() < 5.0);
    assert!((second - 660.0).abs() < 5.0);
    assert_eq!(again, first);
    assert_eq!(nsdf.capacity(), capacity);
}

#[test]
fn simd_mpm_difference_matches_scalar_reference() {
    let sr = 44_100;
    let samples: Vec<f32> = (0..4096)
        .map(|i| {
            let t = i as f32 / sr as f32;
            0.7 * (2.0 * PI * 440.0 * t).sin() + 0.1 * (2.0 * PI * 660.0 * t).sin()
        })
        .collect();
    let mut nsdf = Vec::new();
    mpm_pitch(&samples, sr, PitchRange::default(), &mut nsdf);
    for tau in 0..nsdf.len() {
        let mut acf = 0.0f32;
        let mut norm = 0.0f32;
        for j in 0..samples.len() - tau {
            let (a, b) = (samples[j], samples[j + tau]);
            acf += a * b;
            norm += a * a + b * b;
        }
        let scalar = if norm > 0.0 { 2.0 * acf / norm } else { 0.0 };
        assert!((nsdf[tau] - scalar).abs() < 1e-4, "lag {tau}");
    }
}

// Render the FFT magnitude spectrum of a sum of sine tones, the way
// `analyze` would, so the NMF detector can be tested directly.
fn magnitudes_of(freqs: &[f32], sample_rate: u32, n: usize) -> Vec<f32> {
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            freqs.iter().map(|&f| 0.4 * (2.0 * PI * f * i as f32 / sample_rate as f32).sin()).sum()
        })
        .collect();
    let mut state = FftState::default();
    analyze(&samples, sample_rate, &mut state, PitchAlgorithm::Fft, PitchRange::default())
        .magnitudes
}

#[test]
fn nmf_detects_a_two_note_chord() {
    let sample_rate = 44100u32;
    let n = 4096;
    // A4 (440) + C#5 (554.37): a major third.
    let mags = magnitudes_of(&[440.0, 554.37], sample_rate, n);
    let dict = build_nmf_dict(sample_rate, mags.len(), PitchRange::default());
    let pitches = nmf_pitches(&mags, &dict, &mut NmfScratch::default());
    assert!(pitches.iter().any(|p| p.note == "A" && p.octave == 4), "expected A4, got {pitches:?}");
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
    let pitches = nmf_pitches(&mags, &dict, &mut NmfScratch::default());
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
    let samples: Vec<f32> =
        (0..n).map(|i| 0.4 * (2.0 * PI * 330.0 * i as f32 / sample_rate as f32).sin()).collect();
    let mut state = FftState::default();
    for algo in PitchAlgorithm::all() {
        let pitches =
            analyze(&samples, sample_rate, &mut state, *algo, PitchRange::default()).pitches;
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
    let samples: Vec<f32> =
        (0..n).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin()).collect();
    let mut state = FftState::default();
    let pitches =
        analyze(&samples, sample_rate, &mut state, PitchAlgorithm::Yin, PitchRange::default())
            .pitches;
    assert_eq!(pitches.len(), 1, "YIN is monophonic: one pitch");
    assert_eq!(pitches[0].note, "A");
    assert_eq!(pitches[0].octave, 4);
}

// ── PitchRange::from_freqs ───────────────────────────────────────────────

#[test]
fn pitch_range_from_freqs_falls_back_to_default_when_empty() {
    assert_eq!(PitchRange::from_freqs(std::iter::empty(), 1.0), PitchRange::default());
}

#[test]
fn pitch_range_from_freqs_spans_min_and_max_with_margin() {
    // G3 (~196 Hz) to G6 (~1568 Hz), the range of a low-G 10-hole diatonic.
    let range = PitchRange::from_freqs([196.0, 1568.0], 1.0);
    // A semitone below G3 / above G6.
    assert!(range.min_freq < 196.0, "min {} should be below 196", range.min_freq);
    assert!(range.max_freq > 1568.0, "max {} should be above 1568", range.max_freq);
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
    assert!(196.0 < PitchRange::default().min_freq, "sanity: below the default fixed floor");
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

// ── Chord (polyphonic) detection ──────────────────────────────────────────
//
// `scoring::chord_is_sounding` only counts a chord as played when *every*
// one of its pitches is present at once, so whether a chord in a chart is
// hittable at all depends entirely on the selected algorithm. These tests
// pin down which ones can do it, because the answer is not obvious from the
// UI and picking the wrong one silently makes those notes impossible.

/// Two simultaneous sine tones at `freqs`, one analysis window long.
fn chord_samples(freqs: &[f32], sample_rate: u32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            freqs.iter().map(|f| 0.4 * (2.0 * PI * f * t).sin()).sum::<f32>()
        })
        .collect()
}

/// D4 + G4 — the interval a real chart in the wild actually uses (the
/// Windy City Swing chord items).
const CHORD_D4_G4: [f32; 2] = [293.66, 392.00];

fn hears_both(algo: PitchAlgorithm) -> bool {
    let sr = 44100u32;
    let samples = chord_samples(&CHORD_D4_G4, sr, 4096);
    let mut state = FftState::default();
    let out = analyze(&samples, sr, &mut state, algo, PitchRange::default());
    let d4 = out.pitches.iter().any(|p| p.note == "D" && p.octave == 4);
    let g4 = out.pitches.iter().any(|p| p.note == "G" && p.octave == 4);
    d4 && g4
}

#[test]
fn the_polyphonic_algorithms_hear_both_notes_of_a_chord() {
    for algo in [PitchAlgorithm::Fft, PitchAlgorithm::Nmf] {
        assert!(
            hears_both(algo),
            "{} is documented as reporting multiple pitches, but missed one \
             of D4/G4 — chord items in a chart would be unhittable with it",
            algo.label()
        );
    }
}

#[test]
fn the_default_algorithm_can_hear_a_chord() {
    // Separately from the loop above, because *this* is the one a player who
    // never opens Options is using.
    assert!(
        hears_both(PitchAlgorithm::default()),
        "the default algorithm must be able to score a chord"
    );
}

#[test]
fn the_monophonic_algorithms_cannot_hear_a_chord() {
    // Not a bug to fix in the detectors — YIN/pYIN/MPM estimate a single
    // fundamental by construction. Pinned so the limitation is recorded
    // rather than rediscovered, and so `is_polyphonic` can't drift away
    // from the measured truth.
    for algo in [PitchAlgorithm::Yin, PitchAlgorithm::Pyin, PitchAlgorithm::Mcleod] {
        assert!(
            !hears_both(algo),
            "{} unexpectedly resolved both chord notes — if that is now real, \
             update is_polyphonic() to match",
            algo.label()
        );
    }
}

#[test]
fn a_monophonic_detector_on_a_chord_can_report_a_note_nobody_played() {
    // Worth stating outright: pYIN and MPM don't just miss one note of
    // D4+G4, they resolve F4 — a pitch absent from the signal, roughly
    // between the two. So the failure mode isn't only "chord unhittable";
    // it's a wrong note entering scoring.
    let sr = 44100u32;
    let samples = chord_samples(&CHORD_D4_G4, sr, 4096);
    for algo in [PitchAlgorithm::Pyin, PitchAlgorithm::Mcleod] {
        let mut state = FftState::default();
        let out = analyze(&samples, sr, &mut state, algo, PitchRange::default());
        let phantom = out.pitches.iter().any(|p| p.note != "D" && p.note != "G");
        assert!(
            phantom,
            "{} no longer reports a phantom pitch for D4+G4 — good, but this \
             test documents the opposite; re-measure and update it",
            algo.label()
        );
    }
}

/// `is_polyphonic` must agree with what the detectors actually do above,
/// since the UI uses it to warn players away from an unusable combination.
#[test]
fn is_polyphonic_matches_measured_behaviour() {
    for &algo in PitchAlgorithm::all() {
        assert_eq!(
            algo.is_polyphonic(),
            hears_both(algo),
            "is_polyphonic() disagrees with measured chord detection for {}",
            algo.label()
        );
    }
}

/// End to end through the *real* synth rather than bare sine tones: a chord
/// as `render_pcm` actually produces it, analysed by the default detector.
///
/// The sine-tone tests above prove the detectors can resolve two pitches in
/// principle. This one proves the thing a player depends on — that a chord
/// the game itself sounds (call-and-response demos, the Song Editor's
/// audition and Play button) comes back out as both of its notes. A synth
/// change that made chord voices cancel, or clip, or share one envelope
/// would pass every test above and fail this one.
#[test]
fn a_chord_from_the_real_synth_is_heard_as_both_notes() {
    use harmonicon_core::synth::{Expr, PhraseNote, SAMPLE_RATE, render_pcm};

    // Same tick, same length: `render_pcm` sums voices into one buffer, so
    // this is a genuine simultaneity, not two notes in sequence.
    let chord = [
        PhraseNote { tick: 0, len: 8, freq: Some(CHORD_D4_G4[0]), expr: Expr::None },
        PhraseNote { tick: 0, len: 8, freq: Some(CHORD_D4_G4[1]), expr: Expr::None },
    ];
    let pcm = render_pcm(&chord, 0.05);
    assert!(pcm.len() > 4096, "synth produced too little audio to analyse");

    // Skip the attack: the envelope's opening ramp is a transient, and pitch
    // detection wants the steady state — the same reason the live path
    // analyses mid-note windows rather than onsets.
    let start = (SAMPLE_RATE as f32 * 0.05) as usize;
    let window = &pcm[start..start + 4096];

    let mut state = FftState::default();
    let out =
        analyze(window, SAMPLE_RATE, &mut state, PitchAlgorithm::default(), PitchRange::default());
    let names: Vec<String> =
        out.pitches.iter().map(|p| format!("{}{}", p.note, p.octave)).collect();
    assert!(
        out.pitches.iter().any(|p| p.note == "D" && p.octave == 4),
        "expected D4 from the synthesized chord, got {names:?}"
    );
    assert!(
        out.pitches.iter().any(|p| p.note == "G" && p.octave == 4),
        "expected G4 from the synthesized chord, got {names:?}"
    );
}
