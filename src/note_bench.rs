// SPDX-License-Identifier: MIT

//! Offline pitch-detection benchmark, driven by the `note_bench` binary
//! (`src/bin/note_bench.rs`): replays a "debug recording" (`song_editor::
//! debug_record`, `--features dev`) through each of the five selectable
//! algorithms and compares the result against the chart's own expected
//! notes over time.
//!
//! This is the "Immediate Next Step"/"Analyze the Errors" tooling
//! `Harmonicon Note Detection Roadmap.md` calls for — a reproducible
//! benchmark, run *before* any change to the detection algorithms
//! themselves. The pure comparison logic lives here (not in the bin) so
//! it's directly unit-testable without needing a real WAV file; the bin
//! only handles file I/O and printing.

use std::collections::HashMap;

use crate::audio_system::audio_input::{CHUNK_SIZE, HOP_SIZE};
use crate::audio_system::midi::note_to_midi;
use crate::audio_system::pitch_detect::{self, FftState, PitchAlgorithm, PitchRange};
use crate::song::chart::{Action, HarpChart, tick_to_seconds};

// ── Ground truth ─────────────────────────────────────────────────────────────

/// One chart note's ground truth: when it sounds (chart-time seconds) and
/// which MIDI pitch it's expected to produce. A chord/split `TrackItem`
/// yields one `ExpectedNote` per event, all sharing the same time window —
/// same "one scheduled note per event" shape `gameplay::notes::
/// ScheduledNote` already uses for the same reason (a chord is several
/// simultaneous expectations, not one).
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedNote {
    pub start_secs: f64,
    pub end_secs: f64,
    pub midi: u8,
    /// Hole/action tab label (e.g. `"-4"` for a hole-4 draw, following the
    /// roadmap's own notation) — display only, never compared against.
    pub label: String,
}

/// Resolves every `TrackItem`'s events into [`ExpectedNote`]s, in chart
/// time (seconds) via the chart's own tempo map — `event.note` is already
/// the fully-resolved *sounded* pitch (bend/overblow/overdraw/slide already
/// applied, see `song_editor::harpchart::note_name_for`), so this needs no
/// harmonica-layout lookup of its own. Skips an event with no resolvable
/// `note` name or an unparseable one (shouldn't happen for a chart that
/// loaded successfully at all, but this is offline analysis tooling, not
/// the game itself — degrade by skipping rather than panicking on a
/// hand-edited or malformed chart).
pub fn expected_notes_from_chart(chart: &HarpChart) -> Vec<ExpectedNote> {
    let resolution = chart.timing.resolution.max(1);
    let tempo_map = &chart.timing.tempo_map;
    let mut notes = Vec::new();
    for item in &chart.track {
        let start_secs = match (item.tick, item.time) {
            (Some(tick), _) => tick_to_seconds(tick, resolution, tempo_map),
            (None, Some(time)) => time,
            (None, None) => continue,
        };
        let end_secs = start_secs + item.duration;
        for event in &item.events {
            let Some(note_name) = event.note.as_deref() else {
                continue;
            };
            let Some(midi) = note_to_midi(note_name) else {
                continue;
            };
            let Ok(midi) = u8::try_from(midi) else {
                continue;
            };
            let sign = match event.action {
                Action::Draw => "-",
                Action::Blow => "",
            };
            notes.push(ExpectedNote {
                start_secs,
                end_secs,
                midi,
                label: format!("{sign}{}", event.hole),
            });
        }
    }
    notes
}

/// The (deduplicated, sorted) MIDI pitches expected to be sounding at `t`
/// seconds — every [`ExpectedNote`] whose `[start_secs, end_secs)` window
/// contains it.
pub fn expected_at(notes: &[ExpectedNote], t: f64) -> Vec<u8> {
    let mut midis: Vec<u8> = notes
        .iter()
        .filter(|n| t >= n.start_secs && t < n.end_secs)
        .map(|n| n.midi)
        .collect();
    midis.sort_unstable();
    midis.dedup();
    midis
}

// ── Running a detector offline ───────────────────────────────────────────────

/// One analysis frame's result: `time_secs` is this chunk's end position in
/// the recording, `detected` the (deduplicated, sorted) MIDI pitches the
/// algorithm reported for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub time_secs: f64,
    pub detected: Vec<u8>,
}

/// Replays `samples` (mono, `sample_rate` Hz) through `algorithm` using the
/// same [`CHUNK_SIZE`]/50%-overlap ([`HOP_SIZE`]) chunking the live mic
/// pipeline uses (`audio_system::audio_input::push_chunks`), so an offline
/// benchmark run sees exactly what the real-time detector would have seen
/// — same window, same hop, same algorithm dispatch (`pitch_detect::
/// analyze`). `range` should normally be narrowed to the harp actually
/// being played (`Harmonica::frequency_range`), the same narrowing
/// gameplay/recording both apply — see `note_bench` binary's own call site.
pub fn run_algorithm(
    samples: &[f32],
    sample_rate: u32,
    algorithm: PitchAlgorithm,
    range: PitchRange,
) -> Vec<Frame> {
    let mut fft = FftState::default();
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos + CHUNK_SIZE <= samples.len() {
        let chunk = &samples[pos..pos + CHUNK_SIZE];
        let analysis = pitch_detect::analyze(chunk, sample_rate, &mut fft, algorithm, range);
        let time_secs = (pos + CHUNK_SIZE) as f64 / sample_rate.max(1) as f64;
        let mut detected: Vec<u8> = analysis.pitches.iter().map(|p| p.midi).collect();
        detected.sort_unstable();
        detected.dedup();
        frames.push(Frame {
            time_secs,
            detected,
        });
        pos += HOP_SIZE;
    }
    frames
}

// ── Comparison / confusion matrix ────────────────────────────────────────────

/// One algorithm's aggregate performance against a chart's expected notes:
/// how many expected note-*instances* (one per frame an expected note is
/// due, not per note-event) were actually detected (`true_positive`), how
/// many weren't (`false_negative`), plus how many frames reported a pitch
/// with *nothing* expected at that instant (`false_positive` — a "phantom"
/// detection, the roadmap's own term). `confusion` counts how often a given
/// (expected set, detected set) pairing occurred, most frequent first — the
/// roadmap's example table, generalized from single notes to overlapping
/// sets (a chord/split item expects several at once).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct AlgorithmReport {
    pub true_positive: u32,
    pub false_negative: u32,
    pub false_positive: u32,
    pub confusion: Vec<(Vec<u8>, Vec<u8>, u32)>,
}

/// Builds an [`AlgorithmReport`] from `frames` (one algorithm's offline run,
/// see [`run_algorithm`]) against `expected` (see
/// [`expected_notes_from_chart`]) — pure, so it's directly testable without
/// any real audio or chart file.
pub fn compare(expected: &[ExpectedNote], frames: &[Frame]) -> AlgorithmReport {
    let mut report = AlgorithmReport::default();
    let mut confusion: HashMap<(Vec<u8>, Vec<u8>), u32> = HashMap::new();

    for frame in frames {
        let want = expected_at(expected, frame.time_secs);
        *confusion
            .entry((want.clone(), frame.detected.clone()))
            .or_insert(0) += 1;

        for &m in &want {
            if frame.detected.contains(&m) {
                report.true_positive += 1;
            } else {
                report.false_negative += 1;
            }
        }
        // Every detected pitch *not* in `want` is phantom — including one
        // sitting alongside an otherwise-correct detection (e.g. NMF's
        // dictionary matching reporting neighboring semitones as "also
        // active" for what's actually one clean note): a real detection
        // isn't fully correct just because *a* correct pitch happened to be
        // among several reported ones.
        for &m in &frame.detected {
            if !want.contains(&m) {
                report.false_positive += 1;
            }
        }
    }

    let mut confusion: Vec<(Vec<u8>, Vec<u8>, u32)> =
        confusion.into_iter().map(|((e, d), c)| (e, d, c)).collect();
    confusion.sort_by_key(|entry| std::cmp::Reverse(entry.2));
    report.confusion = confusion;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::chart::{
        Difficulty, HarpChart, NoteEvent, PlayMode, Scoring, Song, TempoPoint, Timing, TrackItem,
    };
    use crate::song::harmonica::richter_harp;

    fn flat_chart(track: Vec<TrackItem>) -> HarpChart {
        HarpChart {
            metadata: None,
            song: Song {
                title: "Test".into(),
                artist: "Test".into(),
                tempo_bpm: 120.0,
                key: "C".into(),
                time_signature: None,
                difficulty: Difficulty::Easy,
                feel: None,
            },
            timing: Timing {
                resolution: 480,
                tempo_map: vec![TempoPoint { tick: 0, bpm: 120.0 }],
                time_signature_map: None,
            },
            harmonica: richter_harp("C"),
            track,
            loop_section: None,
            scoring: Scoring {
                perfect_window_ms: 60,
                good_window_ms: 120,
                miss_window_ms: 220,
                combo: None,
                style_bonus: None,
            },
        }
    }

    fn note_item(time: f64, duration: f64, hole: u8, action: Action, note: &str) -> TrackItem {
        TrackItem {
            id: None,
            time: Some(time),
            tick: None,
            duration,
            phrase: None,
            groove: None,
            play_mode: Some(PlayMode::Single),
            call: false,
            events: vec![NoteEvent {
                hole,
                action,
                note: Some(note.to_string()),
                modifiers: None,
            }],
        }
    }

    // ── expected_notes_from_chart / expected_at ──────────────────────────────

    #[test]
    fn resolves_a_single_note_events_time_window_and_label() {
        let chart = flat_chart(vec![note_item(1.0, 0.5, 1, Action::Blow, "C4")]);
        let notes = expected_notes_from_chart(&chart);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].start_secs, 1.0);
        assert_eq!(notes[0].end_secs, 1.5);
        assert_eq!(notes[0].label, "1");
    }

    #[test]
    fn a_draw_note_gets_a_minus_sign_label() {
        let chart = flat_chart(vec![note_item(0.0, 0.5, 4, Action::Draw, "D4")]);
        let notes = expected_notes_from_chart(&chart);
        assert_eq!(notes[0].label, "-4");
    }

    #[test]
    fn an_event_with_no_note_name_is_skipped() {
        let mut item = note_item(0.0, 0.5, 1, Action::Blow, "C4");
        item.events[0].note = None;
        let chart = flat_chart(vec![item]);
        assert!(expected_notes_from_chart(&chart).is_empty());
    }

    #[test]
    fn a_chord_items_events_all_share_one_time_window() {
        let mut item = note_item(0.0, 1.0, 1, Action::Blow, "C4");
        item.events.push(NoteEvent {
            hole: 2,
            action: Action::Blow,
            note: Some("E4".into()),
            modifiers: None,
        });
        let chart = flat_chart(vec![item]);
        let notes = expected_notes_from_chart(&chart);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].start_secs, notes[1].start_secs);
        assert_eq!(notes[0].end_secs, notes[1].end_secs);
    }

    #[test]
    fn expected_at_only_returns_notes_whose_window_contains_t() {
        let chart = flat_chart(vec![note_item(1.0, 0.5, 1, Action::Blow, "C4")]);
        let notes = expected_notes_from_chart(&chart);
        assert!(expected_at(&notes, 0.5).is_empty());
        assert_eq!(expected_at(&notes, 1.2), vec![60]); // C4 = MIDI 60
        assert!(expected_at(&notes, 1.5).is_empty()); // end is exclusive
    }

    // ── compare ───────────────────────────────────────────────────────────────

    fn expected(midi: u8) -> ExpectedNote {
        ExpectedNote {
            start_secs: 0.0,
            end_secs: 1.0,
            midi,
            label: String::new(),
        }
    }

    fn frame(time_secs: f64, detected: &[u8]) -> Frame {
        Frame {
            time_secs,
            detected: detected.to_vec(),
        }
    }

    #[test]
    fn an_exact_match_counts_as_a_true_positive_with_no_confusion() {
        let expected = vec![expected(60)];
        let frames = vec![frame(0.5, &[60])];
        let report = compare(&expected, &frames);
        assert_eq!(report.true_positive, 1);
        assert_eq!(report.false_negative, 0);
        assert_eq!(report.false_positive, 0);
    }

    #[test]
    fn a_missed_note_is_a_false_negative() {
        let expected = vec![expected(60)];
        let frames = vec![frame(0.5, &[])];
        let report = compare(&expected, &frames);
        assert_eq!(report.true_positive, 0);
        assert_eq!(report.false_negative, 1);
        assert_eq!(report.false_positive, 0);
    }

    #[test]
    fn a_detection_with_nothing_expected_is_a_phantom_false_positive() {
        let expected: Vec<ExpectedNote> = vec![];
        let frames = vec![frame(0.5, &[60])];
        let report = compare(&expected, &frames);
        assert_eq!(report.false_positive, 1);
        assert_eq!(report.true_positive, 0);
    }

    #[test]
    fn extra_notes_alongside_a_correct_one_still_count_as_phantom() {
        // e.g. NMF's dictionary matching reporting neighboring semitones as
        // "also active" for what's actually one clean note.
        let expected = vec![expected(60)];
        let frames = vec![frame(0.5, &[59, 60, 61])];
        let report = compare(&expected, &frames);
        assert_eq!(report.true_positive, 1);
        assert_eq!(report.false_negative, 0);
        assert_eq!(report.false_positive, 2);
    }

    #[test]
    fn confusion_pairs_are_sorted_most_frequent_first() {
        let expected = vec![expected(60)];
        let frames = vec![
            frame(0.1, &[60]),
            frame(0.2, &[60]),
            frame(0.3, &[62]),
        ];
        let report = compare(&expected, &frames);
        // (want=[60], got=[60]) occurred twice; (want=[60], got=[62]) once.
        assert_eq!(report.confusion[0], (vec![60], vec![60], 2));
        assert_eq!(report.confusion[1], (vec![60], vec![62], 1));
    }
}
