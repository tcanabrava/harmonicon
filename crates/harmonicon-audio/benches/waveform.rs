// SPDX-License-Identifier: MIT

//! Whole-file waveform preparation on the song-loading path. The OGG inputs
//! are real backing tracks (a short lesson loop and a full song) kept beside
//! the bench, since songs and lessons themselves now live in their own
//! repositories; WAV is a deterministic PCM backing track like the one
//! produced by MIDI import. `bucket_peaks` isolates the reduction used for
//! already-decoded or generated audio.
//!
//! Run with `cargo bench -p harmonicon-audio --bench waveform`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use harmonicon_audio::waveform::{
    WAVEFORM_BUCKETS, analyze_ogg_waveform, analyze_wav_waveform, bucket_peaks,
};
use harmonicon_core::wav::encode_wav;

const LESSON_OGG: &[u8] = include_bytes!("fixtures/short.ogg");
const SONG_OGG: &[u8] = include_bytes!("fixtures/long.ogg");

fn pcm(seconds: usize) -> Vec<f32> {
    let samples = seconds * 44_100;
    (0..samples)
        .map(|i| ((i % 441) as f32 / 441.0 - 0.5) * 0.5)
        .collect()
}

fn waveform(c: &mut Criterion) {
    let mut group = c.benchmark_group("waveform");
    for (label, bytes) in [("lesson_ogg", LESSON_OGG), ("song_ogg", SONG_OGG)] {
        assert!(analyze_ogg_waveform(bytes, WAVEFORM_BUCKETS).1 > 0.0);
        group.bench_function(BenchmarkId::new("decode_and_peaks", label), |b| {
            b.iter(|| black_box(analyze_ogg_waveform(black_box(bytes), WAVEFORM_BUCKETS)));
        });
    }

    for seconds in [10, 60] {
        let samples = pcm(seconds);
        let wav = encode_wav(&samples, 44_100);
        group.bench_function(BenchmarkId::new("bucket_peaks", seconds), |b| {
            b.iter(|| black_box(bucket_peaks(black_box(&samples), WAVEFORM_BUCKETS)));
        });
        group.bench_function(BenchmarkId::new("wav_and_peaks", seconds), |b| {
            b.iter(|| black_box(analyze_wav_waveform(black_box(&wav), WAVEFORM_BUCKETS)));
        });
    }
    group.finish();
}

criterion_group!(benches, waveform);
criterion_main!(benches);
