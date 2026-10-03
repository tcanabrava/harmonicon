// SPDX-License-Identifier: MIT

//! Note extraction from an imported MIDI score as its tempo map grows.
//! Every note start and end is converted to seconds against that map, so
//! this measures the full path rather than the tick conversion alone.
//!
//! Run with `cargo bench -p harmonicon-bench --bench midi_tempo`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use harmonicon_score::{ScoreFile, midi::MidiScore};
use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

const NOTES: usize = 2048;

fn midi_file(tempo_points: usize) -> Vec<u8> {
    let mut tempos = Vec::with_capacity(tempo_points + 1);
    for i in 0..tempo_points {
        tempos.push(TrackEvent {
            delta: u28::from(if i == 0 { 0 } else { (NOTES / tempo_points * 480) as u32 }),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(
                450_000 + (i % 7) as u32 * 15_000,
            ))),
        });
    }
    tempos.push(TrackEvent {
        delta: u28::from(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let mut notes = Vec::with_capacity(NOTES * 2 + 1);
    for i in 0..NOTES {
        let key = u7::from([60, 62, 64, 65, 67, 69, 71, 72][i % 8]);
        notes.push(TrackEvent {
            delta: u28::from(if i == 0 { 0 } else { 240 }),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn { key, vel: u7::from(100) },
            },
        });
        notes.push(TrackEvent {
            delta: u28::from(240),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff { key, vel: u7::from(0) },
            },
        });
    }
    notes.push(TrackEvent {
        delta: u28::from(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let file = Smf {
        header: Header { format: Format::Parallel, timing: Timing::Metrical(u15::from(480)) },
        tracks: vec![tempos, notes],
    };
    let mut bytes = Vec::new();
    file.write_std(&mut bytes).expect("synthetic MIDI encodes");
    bytes
}

fn midi_tempo(c: &mut Criterion) {
    let mut group = c.benchmark_group("midi_notes_tempo");
    group.throughput(Throughput::Elements(NOTES as u64));
    for points in [1, 64, 256] {
        let score = MidiScore::parse(midi_file(points)).expect("synthetic MIDI parses");
        assert_eq!(score.notes(1).unwrap().len(), NOTES);
        group.bench_function(BenchmarkId::from_parameter(points), |b| {
            b.iter(|| black_box(score.notes(black_box(1)).unwrap()));
        });
    }
    group.finish();
}

criterion_group!(benches, midi_tempo);
criterion_main!(benches);
