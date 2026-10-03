// SPDX-License-Identifier: MIT

//! Pure preparation performed when the music-score window is rebuilt.
//! A dense score includes two-note chords and tied continuations; the
//! ordinary case uses one note per eighth beat. These cases expose how
//! preparation scales with total song length. Tie lookup uses the final 64
//! beats, approximating a visible window late in the song.
//!
//! Run with `cargo bench -p harmonicon-ui --bench notation`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use harmonicon_ui::music_score::{Clef, NotationNote, accidentals, stem_roles, tied_from};

fn notes(count: usize, chord: bool) -> Vec<NotationNote> {
    let mut result = Vec::with_capacity(count * if chord { 2 } else { 1 });
    for i in 0..count {
        let start_beat = i as f64 * 0.5;
        let tied = i > 0 && i % 16 == 0;
        result.push(NotationNote {
            start_beat,
            duration_beats: 0.5,
            midi: if tied { 72 } else { [60, 62, 64, 65, 67, 69, 71, 72][i % 8] },
            tied_from_previous: tied,
            highlighted: false,
        });
        if chord {
            result.push(NotationNote {
                start_beat,
                duration_beats: 0.5,
                midi: if tied { 76 } else { [64, 65, 67, 69, 71, 72, 74, 76][i % 8] },
                tied_from_previous: tied,
                highlighted: false,
            });
        }
    }
    result
}

fn notation(c: &mut Criterion) {
    let mut group = c.benchmark_group("notation_rebuild");
    for (label, count, chord) in
        [("melody_256", 256, false), ("chords_256", 256, true), ("chords_2048", 2048, true)]
    {
        let notes = notes(count, chord);
        group.bench_function(BenchmarkId::new("stem_roles", label), |b| {
            b.iter(|| black_box(stem_roles(black_box(&notes), Clef::Treble)));
        });
        group.bench_function(BenchmarkId::new("accidentals", label), |b| {
            b.iter(|| black_box(accidentals(black_box(&notes), Clef::Treble, 4.0)));
        });
        let visible_start = (count.saturating_sub(128)) * if chord { 2 } else { 1 };
        let tied: Vec<usize> = notes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (i >= visible_start && n.tied_from_previous).then_some(i))
            .collect();
        assert!(!tied.is_empty());
        assert!(tied.iter().all(|&i| tied_from(&notes, i).is_some()));
        group.bench_function(BenchmarkId::new("tie_lookup", label), |b| {
            b.iter(|| {
                for &i in black_box(&tied) {
                    black_box(tied_from(&notes, i));
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, notation);
criterion_main!(benches);
