// SPDX-License-Identifier: MIT

//! Carries scored state across the note-list rebuild caused by a mid-song
//! adaptive-difficulty change. A second case inserts newly unlocked notes,
//! which have no old match and exercise the expensive search path.
//!
//! Run with `cargo bench -p harmonicon-gameplay --bench adaptive_rebuild`.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use harmonicon_gameplay::gameplay::{ScheduledNote, carry_over_note_state};

fn note(i: usize, time: f64) -> ScheduledNote {
    ScheduledNote {
        time,
        duration: 0.2,
        hole: (i % 10 + 1) as u8,
        is_blow: i.is_multiple_of(2),
        expected_pitch: Some(60 + (i % 12) as u8),
        hit: i.is_multiple_of(3),
        missed: i % 3 == 1,
        held: 0.1,
        sustain_scored: i.is_multiple_of(3),
        phrase_section: i / 16,
        ..Default::default()
    }
}

fn adaptive_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("carry_over_note_state");
    for count in [500, 3000] {
        let old: Vec<_> = (0..count).map(|i| note(i, i as f64 * 0.25)).collect();
        let same = old.clone();
        let mut unlocked = Vec::with_capacity(count + count / 4);
        for (i, original) in old.iter().enumerate() {
            unlocked.push(original.clone());
            if i.is_multiple_of(4) {
                unlocked.push(note(i, original.time + 0.125));
            }
        }
        for (label, template) in [("same", same), ("new_notes", unlocked)] {
            group.bench_function(BenchmarkId::new(label, count), |b| {
                b.iter_batched_ref(
                    || template.clone(),
                    |new| carry_over_note_state(black_box(&old), black_box(new)),
                    BatchSize::SmallInput,
                );
            });
        }
    }
    group.finish();
}

criterion_group!(benches, adaptive_rebuild);
criterion_main!(benches);
