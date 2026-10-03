// SPDX-License-Identifier: MIT

//! `harmonicon-core` functions that run on a live path:
//!
//! - `HarmonicaNoteTracker::update` runs once per detector hop whenever
//!   harmonica filtering is on. Each iteration feeds it a 64-hop stream, so
//!   onsets, releases and breath-direction changes all happen as they would
//!   live. The throughput figure is per hop.
//! - `chart::tick_to_seconds` / `seconds_to_tick` walk the tempo map
//!   linearly, and the 2D beat guides call them for every visible guide on
//!   every frame. Measured on maps of 1, 16 and 256 tempo points, 64 queries
//!   spread over the song per iteration.
//! - `synth::render_pcm` renders synchronously, both for the
//!   call-and-response demo at song setup and in the Song Editor's audition
//!   and preview. Measured on an 8-note call phrase and a 200-note song.
//!
//! Run with `cargo bench -p harmonicon-bench --bench core_hot_paths`.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use harmonicon_core::chart::{TempoPoint, seconds_to_tick, tick_to_seconds};
use harmonicon_core::harmonica::richter_harp;
use harmonicon_core::harmonica_constraints::{HarmonicaNoteTracker, NoteTrackerConfig};
use harmonicon_core::midi::midi_to_freq_hz;
use harmonicon_core::synth::{Expr, PhraseNote, TICKS_PER_BEAT, render_pcm};

const HOPS: usize = 64;

/// Candidate streams on a C harp, `HOPS` long.
fn tracker_streams() -> [(&'static str, Vec<Vec<u8>>); 3] {
    // Hole 4 blow held throughout.
    let steady = vec![vec![72]; HOPS];
    // Hole 4 blow and draw (C5/D5), switching every four hops.
    let alternating = (0..HOPS).map(|i| vec![if (i / 4) % 2 == 0 { 72 } else { 74 }]).collect();
    // Up to six candidates per hop, mixing blow, draw and unplayable pitches.
    let pool = [60u8, 62, 64, 65, 67, 71, 72, 74, 76, 61, 70, 90];
    let mut state: u32 = 0x9e37_79b9;
    let noisy = (0..HOPS)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let count = 1 + (state >> 29) as usize % 6;
            (0..count).map(|k| pool[((state >> (k * 4)) as usize + k) % pool.len()]).collect()
        })
        .collect();
    [("steady", steady), ("alternating", alternating), ("noisy", noisy)]
}

fn tracker_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("tracker_update");
    group.throughput(Throughput::Elements(HOPS as u64));
    for (label, stream) in tracker_streams() {
        group.bench_function(label, |b| {
            b.iter_batched_ref(
                || HarmonicaNoteTracker::new(richter_harp("C"), NoteTrackerConfig::default()),
                |tracker| {
                    for candidates in &stream {
                        black_box(tracker.update(black_box(candidates)));
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

const RESOLUTION: u32 = 480;
/// Ticks between tempo changes, and the song length is `points` of them.
const SEGMENT_TICKS: u64 = RESOLUTION as u64 * 16;

fn tempo_map(points: usize) -> Vec<TempoPoint> {
    (0..points)
        .map(|i| TempoPoint { tick: i as u64 * SEGMENT_TICKS, bpm: 90.0 + (i % 7) as f32 * 5.0 })
        .collect()
}

fn tempo_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("tempo_map");
    group.throughput(Throughput::Elements(HOPS as u64));
    for points in [1usize, 16, 256] {
        let map = tempo_map(points);
        let song_ticks = points as u64 * SEGMENT_TICKS;
        let ticks: Vec<u64> = (0..HOPS as u64).map(|i| i * song_ticks / HOPS as u64).collect();
        let secs: Vec<f64> = ticks.iter().map(|&t| tick_to_seconds(t, RESOLUTION, &map)).collect();
        group.bench_with_input(BenchmarkId::new("tick_to_seconds", points), &ticks, |b, ticks| {
            b.iter(|| {
                for &t in ticks {
                    black_box(tick_to_seconds(black_box(t), RESOLUTION, &map));
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("seconds_to_tick", points), &secs, |b, secs| {
            b.iter(|| {
                for &s in secs {
                    black_box(seconds_to_tick(black_box(s), RESOLUTION, &map));
                }
            });
        });
    }
    group.finish();
}

/// `count` eighth notes walking a C major scale, every fourth one with
/// vibrato so that path is exercised too.
fn phrase(count: usize) -> Vec<PhraseNote> {
    let scale = [60u8, 62, 64, 65, 67, 69, 71, 72];
    let eighth = TICKS_PER_BEAT / 2;
    (0..count)
        .map(|i| PhraseNote {
            tick: i * eighth,
            len: eighth,
            freq: Some(midi_to_freq_hz(scale[i % scale.len()] as f32)),
            expr: if i % 4 == 3 { Expr::Vibrato(5.0) } else { Expr::None },
        })
        .collect()
}

fn synth_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_pcm");
    // 100 BPM.
    let secs_per_tick = 60.0 / 100.0 / TICKS_PER_BEAT as f32;
    for count in [8usize, 200] {
        let notes = phrase(count);
        group.bench_with_input(BenchmarkId::from_parameter(count), &notes, |b, notes| {
            b.iter(|| black_box(render_pcm(black_box(notes), secs_per_tick)));
        });
    }
    group.finish();
}

criterion_group!(benches, tracker_update, tempo_conversions, synth_render);
criterion_main!(benches);
