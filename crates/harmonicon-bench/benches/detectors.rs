// SPDX-License-Identifier: MIT

//! What one `analyze` call costs per detection algorithm — the work the live
//! mic pipeline does once per hop (every `HOP_SIZE` samples, ~46 ms at
//! 44.1 kHz), on one `CHUNK_SIZE` window.
//!
//! Inputs come from `harmonicon_core::synth`, the same harmonica voice the
//! synthetic dataset uses: a single held note, a three-hole blow chord (what
//! the polyphonic FFT/NMF detectors exist for), white noise (loud enough to
//! pass the silence gate but pitchless), and silence (the RMS early-out).
//! Each window is cut from the middle of the note, clear of attack and
//! release.
//!
//! `analyze_warm` reuses one `FftState`, as the pipeline does. `analyze_cold`
//! starts from a fresh one each call, so it also pays for the FFT plan and,
//! for NMF, the note dictionary — the cost of a detection-range change.
//!
//! Run with `cargo bench -p harmonicon-bench --bench detectors`.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

use harmonicon_core::midi::midi_to_freq_hz;
use harmonicon_core::synth::{Expr, PhraseNote, SAMPLE_RATE, render_pcm};
use harmonicon_dsp::{CHUNK_SIZE, FftState, PitchAlgorithm, PitchRange, analyze};

/// One `CHUNK_SIZE` window of the synth voice sounding `midis` together.
fn synth_window(midis: &[u8]) -> Vec<f32> {
    // One tick per millisecond; a one-second note leaves plenty of steady
    // sustain around its midpoint.
    let notes: Vec<PhraseNote> = midis
        .iter()
        .map(|&m| PhraseNote {
            tick: 0,
            len: 1000,
            freq: Some(midi_to_freq_hz(m as f32)),
            expr: Expr::None,
        })
        .collect();
    let pcm = render_pcm(&notes, 0.001);
    let start = SAMPLE_RATE as usize / 2 - CHUNK_SIZE / 2;
    pcm[start..start + CHUNK_SIZE].to_vec()
}

/// Deterministic white noise at roughly the synth's level.
fn noise_window() -> Vec<f32> {
    let mut state: u32 = 0x1234_5678;
    (0..CHUNK_SIZE)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 24) as f32 * 0.5 - 0.25
        })
        .collect()
}

fn signals() -> [(&'static str, Vec<f32>); 4] {
    [
        // Hole 4 blow on a C harp.
        ("note", synth_window(&[72])),
        // Holes 1–3 blow on a C harp: C4 E4 G4.
        ("chord", synth_window(&[60, 64, 67])),
        ("noise", noise_window()),
        ("silence", vec![0.0; CHUNK_SIZE]),
    ]
}

fn analyze_warm(c: &mut Criterion) {
    let signals = signals();
    let mut group = c.benchmark_group("analyze_warm");
    for &algorithm in PitchAlgorithm::all() {
        for (label, samples) in &signals {
            let mut state = FftState::default();
            // Build the plan and any dictionary before timing.
            analyze(samples, SAMPLE_RATE, &mut state, algorithm, PitchRange::default());
            group.bench_with_input(
                BenchmarkId::new(algorithm.label(), label),
                samples,
                |b, samples| {
                    b.iter(|| {
                        black_box(analyze(
                            black_box(samples),
                            SAMPLE_RATE,
                            &mut state,
                            algorithm,
                            PitchRange::default(),
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

fn analyze_cold(c: &mut Criterion) {
    let samples = synth_window(&[72]);
    let mut group = c.benchmark_group("analyze_cold");
    for &algorithm in PitchAlgorithm::all() {
        group.bench_function(algorithm.label(), |b| {
            b.iter_batched_ref(
                FftState::default,
                |state| {
                    black_box(analyze(
                        black_box(&samples),
                        SAMPLE_RATE,
                        state,
                        algorithm,
                        PitchRange::default(),
                    ))
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, analyze_warm, analyze_cold);
criterion_main!(benches);
