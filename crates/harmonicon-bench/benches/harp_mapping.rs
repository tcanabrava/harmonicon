// SPDX-License-Identifier: MIT

//! Harmonica mapping on score import and live pitch events. `suggested_harp`
//! searches keys and instrument types for an imported part; `map_pitch_playable`
//! resolves each note during conversion and each detected pitch during play.
//! The pitches mix natural notes, bends and notes outside a C harp's range.
//!
//! Run with `cargo bench -p harmonicon-bench --bench harp_mapping`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use harmonicon_core::harmonica::richter_harp;
use harmonicon_core::pitch_map::{HarpKind, map_pitch_playable, suggest_key};
use harmonicon_score::convert::suggested_harp;

fn pitches(count: usize) -> Vec<u8> {
    const PHRASE: [u8; 16] = [60, 62, 64, 65, 67, 68, 70, 72, 74, 76, 77, 79, 81, 83, 48, 88];
    (0..count).map(|i| PHRASE[i % PHRASE.len()]).collect()
}

fn harp_mapping(c: &mut Criterion) {
    let mut group = c.benchmark_group("harp_mapping");
    let harp = richter_harp("C");
    let phrase = pitches(64);
    group.bench_function("resolve_64_pitches", |b| {
        b.iter(|| {
            for &pitch in black_box(&phrase) {
                black_box(map_pitch_playable(black_box(pitch), &harp));
            }
        });
    });
    for count in [256, 2048] {
        let part = pitches(count);
        group.bench_function(BenchmarkId::new("suggest_key", count), |b| {
            b.iter(|| black_box(suggest_key(black_box(&part), HarpKind::Diatonic)));
        });
        group.bench_function(BenchmarkId::new("suggested_harp", count), |b| {
            b.iter(|| black_box(suggested_harp(black_box(&part))));
        });
    }
    group.finish();
}

criterion_group!(benches, harp_mapping);
criterion_main!(benches);
