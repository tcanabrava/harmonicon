// SPDX-License-Identifier: MIT

//! Native chart loading costs: JSON parsing and the migration, schema
//! validation and typed deserialization performed by the real loader.
//! The validator is warmed before timing, as it is cached process-wide.
//!
//! Run with `cargo bench -p harmonicon-song --bench chart_validation`.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use harmonicon_song::song::validate_and_migrate_chart;
use serde_json::Value;

const SHORT: &[u8] = include_bytes!(
    "../../../tests/fixtures/song-pack/Traditional/Amazing Grace/song/chart.harpchart"
);
const LONG: &[u8] = include_bytes!(
    "../../../tests/fixtures/song-pack/Traditional/O Pulo da Gaita/song/chart.harpchart"
);

fn chart_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("chart_validation");
    for (label, bytes) in [("short", SHORT), ("long", LONG)] {
        let value: Value = serde_json::from_slice(bytes).expect("bundled chart parses");
        validate_and_migrate_chart(&mut value.clone()).expect("bundled chart validates");

        group.bench_function(BenchmarkId::new("parse_json", label), |b| {
            b.iter(|| black_box(serde_json::from_slice::<Value>(black_box(bytes)).unwrap()));
        });
        group.bench_function(BenchmarkId::new("validate_and_deserialize", label), |b| {
            b.iter_batched(
                || value.clone(),
                |mut chart| black_box(validate_and_migrate_chart(&mut chart).unwrap()),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, chart_validation);
criterion_main!(benches);
