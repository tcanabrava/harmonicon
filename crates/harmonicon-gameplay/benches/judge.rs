// SPDX-License-Identifier: MIT

//! What scoring costs: `score_notes` every frame, and `build_scheduled_notes`
//! whenever the note list is rebuilt, which includes a mid-song rebuild when
//! adaptive difficulty is toggled from the pause menu.
//!
//! `score_frame` runs the real `score_notes` system on a minimal `World`,
//! halfway through a synthetic song with a note every quarter second. Every
//! note well behind the clock is already resolved and the cursor is past
//! them, as in live play. Each iteration starts from the same note state,
//! so a `hit` iteration always scores one fresh attack. Song lengths of 500
//! and 3000 notes should cost the same: the cursor and the early break
//! bound the scan to the hit window.
//!
//! `build_scheduled_notes` uses the longest bundled chart (O Pulo da Gaita,
//! 239 items) and the same chart repeated eight times.
//!
//! Run with `cargo bench -p harmonicon-gameplay --bench judge`.

use std::collections::HashSet;
use std::hint::black_box;
use std::path::PathBuf;

use bevy::prelude::*;
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

use harmonicon_app::app::EffectiveHarmonica;
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::{AudioFrame, PitchInfo};
use harmonicon_core::chart::HarpChart;
use harmonicon_core::harmonica::richter_harp;
use harmonicon_core::midi::midi_to_freq_hz;
use harmonicon_gameplay::gameplay::{
    ActivePitches, AdaptiveDifficulty, GameplayClock, HitFeedback, NoteScored, PitchGate,
    PlayedHarp, ScheduledNote, Score, ScoringConfig, SongNotes, SongStats, ValidHarpNotes,
    build_scheduled_notes, score_notes,
};

const NOTE_SPACING_SECS: f64 = 0.25;
/// Holes 1–4 blow on a C harp.
const PITCHES: [u8; 4] = [60, 64, 67, 72];

fn note(i: usize) -> ScheduledNote {
    ScheduledNote {
        time: i as f64 * NOTE_SPACING_SECS,
        duration: NOTE_SPACING_SECS * 0.8,
        hole: (i % PITCHES.len()) as u8 + 1,
        is_blow: true,
        expected_pitch: Some(PITCHES[i % PITCHES.len()]),
        hit: false,
        missed: false,
        held: 0.0,
        sustain_scored: false,
        modifiers: Vec::new(),
        pitch_samples: Vec::new(),
        amp_samples: Vec::new(),
        phrase_section: 0,
        chord_pitches: Vec::new(),
        playable: true,
        miss_evidence: None,
        force_wait: false,
    }
}

/// `count` notes with the clock on the middle one; everything more than a
/// second behind it is resolved and skipped by the cursor.
fn mid_song(count: usize) -> (SongNotes, f64, u8) {
    let due = count / 2;
    let clock = due as f64 * NOTE_SPACING_SECS;
    let mut notes: Vec<ScheduledNote> = (0..count).map(note).collect();
    let mut cursor = 0;
    for n in notes.iter_mut().filter(|n| n.time < clock - 1.0) {
        n.missed = true;
        cursor += 1;
    }
    (SongNotes { notes, cursor }, clock, PITCHES[due % PITCHES.len()])
}

fn world(clock: f64, sounding: Option<u8>) -> World {
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(clock));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(
        sounding
            .map(|midi| PitchInfo {
                midi,
                note: String::new(),
                octave: 0,
                frequency: midi_to_freq_hz(midi as f32),
            })
            .into_iter()
            .collect(),
    ));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from(PITCHES)));
    world.insert_resource(PlayedHarp(Some(richter_harp("C"))));
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.init_resource::<Messages<NoteScored>>();
    world
}

fn score_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("score_frame");
    for count in [500usize, 3000] {
        let (song, clock, due_pitch) = mid_song(count);
        for (label, sounding) in [("silent", None), ("hit", Some(due_pitch))] {
            let mut world = world(clock, sounding);
            let mut schedule = Schedule::default();
            schedule.add_systems(score_notes);
            group.bench_function(BenchmarkId::new(label, count), |b| {
                b.iter_batched(
                    || SongNotes { notes: song.notes.clone(), cursor: song.cursor },
                    |notes| {
                        world.insert_resource(notes);
                        world.insert_resource(PitchGate::default());
                        schedule.run(&mut world);
                        world.resource_mut::<Messages<NoteScored>>().clear();
                        // Returned, so Criterion drops the song outside the
                        // timed section.
                        world.remove_resource::<SongNotes>()
                    },
                    BatchSize::LargeInput,
                );
            });
        }
    }
    group.finish();
}

fn fixture_chart() -> HarpChart {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/song-pack/Traditional/O Pulo da Gaita/song/chart.harpchart");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture chart parses")
}

/// The chart played `times` times back to back.
fn tiled(chart: &HarpChart, times: usize) -> HarpChart {
    let length = chart
        .track
        .iter()
        .filter_map(|item| item.time.map(|t| t + item.duration))
        .fold(0.0, f64::max);
    let mut out = chart.clone();
    out.track = (0..times)
        .flat_map(|k| {
            chart.track.iter().map(move |item| {
                let mut item = item.clone();
                item.time = item.time.map(|t| t + k as f64 * length);
                item
            })
        })
        .collect();
    out
}

fn build_notes(c: &mut Criterion) {
    let chart = fixture_chart();
    let effective = EffectiveHarmonica::default();
    let adaptive = AdaptiveDifficulty::default();
    let mut group = c.benchmark_group("build_scheduled_notes");
    for times in [1usize, 8] {
        let chart = tiled(&chart, times);
        group.bench_with_input(
            BenchmarkId::from_parameter(chart.track.len()),
            &chart,
            |b, chart| {
                b.iter(|| {
                    black_box(build_scheduled_notes(&effective, black_box(chart), &adaptive))
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, score_frame, build_notes);
criterion_main!(benches);
