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

use harmonicon_core::chart::{Action, HarpChart, tick_to_seconds};
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::harmonica_constraints::{
    HarmonicaNoteTracker, NoteTrackerConfig, plausible_notes, reachable_directions,
};
use harmonicon_core::midi::note_to_midi;
use harmonicon_dsp::{
    self as pitch_detect, CHUNK_SIZE, FftState, HOP_SIZE, PitchAlgorithm, PitchRange,
};

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
    /// Whether the note is played blowing (`false` for drawing) — what
    /// [`direction_accuracy`] checks a detection's breath against.
    pub blow: bool,
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
                blow: event.action == Action::Blow,
                label: format!("{sign}{}", event.hole),
            });
        }
    }
    notes
}

/// Default slack applied on both ends of every [`ExpectedNote`]'s window
/// before checking whether a detection instant falls inside it (see
/// [`expected_at`]) — a played-along-with-the-chart take is never sample-
/// accurate against the chart's own clock (nobody's timing is that tight,
/// least of all against a screen, not a metronome click track), and this
/// benchmark is measuring *pitch* detection, not rhythmic precision. Loose
/// enough to absorb ordinary human timing drift, tight enough that it
/// wouldn't paper over a genuinely different, adjacent note.
pub const DEFAULT_TIMING_TOLERANCE_SECS: f64 = 0.25;

/// The (deduplicated, sorted) MIDI pitches expected to be sounding at `t`
/// seconds — every [`ExpectedNote`] whose `[start_secs, end_secs)` window,
/// widened by `tolerance_secs` on both ends, contains it. Widening (rather
/// than requiring exact alignment) is what lets a take that nailed the
/// pitch but drifted a little on tempo still score as correct — see
/// [`DEFAULT_TIMING_TOLERANCE_SECS`]'s own doc comment for why that's the
/// right call for this benchmark, not a fudge.
pub fn expected_at(notes: &[ExpectedNote], t: f64, tolerance_secs: f64) -> Vec<u8> {
    let mut midis: Vec<u8> = notes
        .iter()
        .filter(|n| t >= n.start_secs - tolerance_secs && t < n.end_secs + tolerance_secs)
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
/// pipeline uses (`harmonicon_audio::audio_input`'s chunk writer), so an offline
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

/// Re-filters each of `frames`' detected pitch sets through
/// `harmonicon_core::harmonica_constraints::plausible_notes` — the "Harmonica
/// Constraint Solver" roadmap stage, applied here as an offline
/// post-process so its effect on `compare`'s hit/miss/phantom counts can be
/// measured directly against a detector's raw output, on the same
/// recording, before deciding whether it's worth wiring into the live
/// pipeline. Frame `time_secs` is untouched; only `detected` changes.
pub fn apply_constraints(harp: &Harmonica, frames: &[Frame]) -> Vec<Frame> {
    frames
        .iter()
        .map(|f| Frame {
            time_secs: f.time_secs,
            detected: plausible_notes(harp, &f.detected),
        })
        .collect()
}

/// Applies the same stateful direction/onset/release policy used by gameplay.
/// This is the constrained row to use when judging real recordings; the older
/// stateless helper remains useful for isolating the direction rule itself.
pub fn apply_live_constraints(harp: &Harmonica, frames: &[Frame]) -> Vec<Frame> {
    let mut tracker = HarmonicaNoteTracker::new(harp.clone(), NoteTrackerConfig::default());
    frames
        .iter()
        .map(|frame| Frame {
            time_secs: frame.time_secs,
            detected: tracker.update(&frame.detected).active,
        })
        .collect()
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
    /// Frames where a chord (two or more pitches) was expected.
    pub chord_frames_expected: u32,
    /// Frames where two or more pitches were detected.
    pub chord_frames_detected: u32,
    /// Frames where a chord was expected and the detected set was exactly
    /// it — no pitch missing, none extra. A chord is judged whole in play
    /// (`scoring::chord_is_sounding`), so per-note counts overstate how
    /// often one would actually score.
    pub chord_frames_exact: u32,
}

/// `part / whole`, or `None` when there was nothing to measure.
fn ratio(part: u32, whole: u32) -> Option<f32> {
    (whole > 0).then(|| part as f32 / whole as f32)
}

impl AlgorithmReport {
    /// Per-note precision: of the pitches detected, the share that were
    /// expected.
    pub fn precision(&self) -> Option<f32> {
        ratio(self.true_positive, self.true_positive + self.false_positive)
    }

    /// Per-note recall: of the pitches expected, the share detected.
    ///
    /// **Frame-level, and diluted by the timing tolerance.** A note counts
    /// as expected for `tolerance_secs` either side of its written window,
    /// so for short notes separated by silence this stays far below 100%
    /// even for a perfect detector (about 55% on the synthetic dataset's
    /// 0.45 s notes at the default ±0.25 s). Read it for *changes* between
    /// runs; [`NoteTimings::never_detected`] answers "was each note heard".
    pub fn recall(&self) -> Option<f32> {
        ratio(self.true_positive, self.true_positive + self.false_negative)
    }

    /// Exact-set chord recall: of the frames expecting a chord, the share
    /// where exactly that chord was detected.
    pub fn chord_recall(&self) -> Option<f32> {
        ratio(self.chord_frames_exact, self.chord_frames_expected)
    }

    /// Exact-set chord precision: of the frames reporting two or more
    /// pitches, the share that were exactly the chord expected.
    pub fn chord_precision(&self) -> Option<f32> {
        ratio(self.chord_frames_exact, self.chord_frames_detected)
    }
}

/// Builds an [`AlgorithmReport`] from `frames` (one algorithm's offline run,
/// see [`run_algorithm`]) against `expected` (see
/// [`expected_notes_from_chart`]), with [`expected_at`]'s timing tolerance
/// applied — pure, so it's directly testable without any real audio or
/// chart file.
pub fn compare(
    expected: &[ExpectedNote],
    frames: &[Frame],
    tolerance_secs: f64,
) -> AlgorithmReport {
    let mut report = AlgorithmReport::default();
    let mut confusion: HashMap<(Vec<u8>, Vec<u8>), u32> = HashMap::new();

    for frame in frames {
        let want = expected_at(expected, frame.time_secs, tolerance_secs);
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
        if want.len() >= 2 {
            report.chord_frames_expected += 1;
            if frame.detected == want {
                report.chord_frames_exact += 1;
            }
        }
        if frame.detected.len() >= 2 {
            report.chord_frames_detected += 1;
        }
        *confusion.entry((want, frame.detected.clone())).or_insert(0) += 1;
    }

    // Ties are ordered by pitch set, so repeated runs print the same table.
    let mut confusion: Vec<(Vec<u8>, Vec<u8>, u32)> =
        confusion.into_iter().map(|((e, d), c)| (e, d, c)).collect();
    confusion.sort_unstable_by(|a, b| b.2.cmp(&a.2).then_with(|| (&a.0, &a.1).cmp(&(&b.0, &b.1))));
    report.confusion = confusion;
    report
}

// ── Timing and direction ─────────────────────────────────────────────────────

/// How late each expected note was picked up and let go, in seconds, from
/// [`note_timings`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct NoteTimings {
    /// Per detected note: first detecting frame minus the note's start.
    /// Includes the analysis window, since that is what a player waits for.
    pub onset_secs: Vec<f64>,
    /// Per detected note whose release is unambiguous: first frame no longer
    /// detecting it minus the note's end. Negative when the detector lost
    /// the pitch before the note ended — a dropout mid-sustain.
    pub release_secs: Vec<f64>,
    /// Expected notes never detected inside their widened window.
    pub never_detected: u32,
}

/// Onset and release latency for every expected note in `frames`.
///
/// **Onset** is the first frame inside the note's window, widened by
/// `tolerance_secs` at both ends, that detects its pitch. **Release** walks
/// on from there to the first frame without the pitch. A release is skipped
/// when another expected note of the same pitch starts before this one's
/// widened end, since the pitch continuing is then correct rather than
/// late, and when the recording ends with the pitch still detected.
pub fn note_timings(
    expected: &[ExpectedNote],
    frames: &[Frame],
    tolerance_secs: f64,
) -> NoteTimings {
    let mut timings = NoteTimings::default();
    for note in expected {
        let window = (note.start_secs - tolerance_secs)..(note.end_secs + tolerance_secs);
        let Some(onset) = frames
            .iter()
            .position(|f| window.contains(&f.time_secs) && f.detected.contains(&note.midi))
        else {
            timings.never_detected += 1;
            continue;
        };
        timings
            .onset_secs
            .push(frames[onset].time_secs - note.start_secs);

        let continued = expected.iter().any(|other| {
            other.midi == note.midi
                && other.start_secs > note.start_secs
                && other.start_secs < note.end_secs + tolerance_secs
        });
        if continued {
            continue;
        }
        if let Some(release) = frames[onset..]
            .iter()
            .find(|f| !f.detected.contains(&note.midi))
        {
            timings.release_secs.push(release.time_secs - note.end_secs);
        }
    }
    timings
}

/// Of the frames with both an expectation and a detection, the share whose
/// every detected pitch can be played in the expected breath direction on
/// `harp` — whether the detector keeps blow and draw apart, whatever it
/// makes of the exact pitch.
///
/// A pitch reachable both ways (a C harp's G4 is blow 3 and draw 2) counts
/// as matching either. Frames whose expectation mixes directions are
/// skipped: no single breath is right there. `None` when no frame counted.
pub fn direction_accuracy(
    harp: &Harmonica,
    expected: &[ExpectedNote],
    frames: &[Frame],
    tolerance_secs: f64,
) -> Option<f32> {
    let (mut counted, mut matched) = (0u32, 0u32);
    for frame in frames.iter().filter(|f| !f.detected.is_empty()) {
        let t = frame.time_secs;
        let mut due = expected
            .iter()
            .filter(|n| t >= n.start_secs - tolerance_secs && t < n.end_secs + tolerance_secs)
            .map(|n| n.blow);
        let Some(blow) = due.next() else {
            continue;
        };
        if due.any(|other| other != blow) {
            continue;
        }
        counted += 1;
        let fits = frame.detected.iter().all(|&midi| {
            let (by_blow, by_draw) = reachable_directions(harp, midi);
            if blow { by_blow } else { by_draw }
        });
        if fits {
            matched += 1;
        }
    }
    ratio(matched, counted)
}

/// The middle value, or `None` for no values.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::chart::{
        Difficulty, HarpChart, NoteEvent, PlayMode, Scoring, Song, TempoPoint, Timing, TrackItem,
    };
    use harmonicon_core::harmonica::richter_harp;

    // ── apply_constraints ────────────────────────────────────────────────────

    #[test]
    fn apply_constraints_drops_a_minority_wind_direction_phantom() {
        let harp = richter_harp("C");
        // Hole-1 blow (60) and hole-2 blow (64) outnumber hole-1 draw (62) —
        // the same "two blow notes really were played, 62 is a phantom
        // detection" scenario the roadmap's own confusion-matrix example
        // calls out.
        let frames = vec![frame(0.1, &[60, 64, 62])];
        let constrained = apply_constraints(&harp, &frames);
        assert_eq!(constrained[0].detected, vec![60, 64]);
        assert_eq!(constrained[0].time_secs, frames[0].time_secs);
    }

    #[test]
    fn apply_constraints_can_turn_a_phantom_false_positive_into_a_clean_true_positive() {
        let harp = richter_harp("C");
        let expected = vec![expected(60), expected(64)];
        let frames = vec![frame(0.1, &[60, 64, 62])];
        let raw_report = compare(&expected, &frames, 0.0);
        assert_eq!(raw_report.false_positive, 1);

        let constrained = apply_constraints(&harp, &frames);
        let constrained_report = compare(&expected, &constrained, 0.0);
        assert_eq!(constrained_report.true_positive, 2);
        assert_eq!(constrained_report.false_positive, 0);
    }

    #[test]
    fn apply_constraints_leaves_a_legal_chord_untouched() {
        let harp = richter_harp("C");
        // Holes 1-3 blow together — all blow-family, nothing to drop.
        let frames = vec![frame(0.1, &[60, 64, 67])];
        let constrained = apply_constraints(&harp, &frames);
        assert_eq!(constrained[0].detected, vec![60, 64, 67]);
    }

    #[test]
    fn live_constraints_confirm_over_two_frames_and_release_on_the_first_silent_one() {
        let harp = richter_harp("C");
        let frames = vec![
            frame(0.1, &[60]),
            frame(0.2, &[60]),
            frame(0.3, &[]),
            frame(0.4, &[60]),
            frame(0.5, &[60]),
        ];
        let constrained = apply_live_constraints(&harp, &frames);
        assert!(constrained[0].detected.is_empty());
        assert_eq!(constrained[1].detected, vec![60]);
        assert!(constrained[2].detected.is_empty());
        // The re-attack pays the confirmation cost again, rather than being
        // swallowed by a release grace still holding the first note open.
        assert!(constrained[3].detected.is_empty());
        assert_eq!(constrained[4].detected, vec![60]);
    }

    fn flat_chart(track: Vec<TrackItem>) -> HarpChart {
        HarpChart {
            metadata: None,
            song: Song {
                title: "Test".into(),
                artist: "Test".into(),
                genre: "Test".into(),
                tempo_bpm: 120.0,
                key: "C".into(),
                time_signature: None,
                difficulty: Difficulty::Easy,
                feel: None,
            },
            timing: Timing {
                resolution: 480,
                tempo_map: vec![TempoPoint {
                    tick: 0,
                    bpm: 120.0,
                }],
                time_signature_map: None,
                pickup_ticks: None,
                repeats: Vec::new(),
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
            chord: None,
            play_mode: Some(PlayMode::Single),
            call: false,
            lyric: None,
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
        assert!(expected_at(&notes, 0.5, 0.0).is_empty());
        assert_eq!(expected_at(&notes, 1.2, 0.0), vec![60]); // C4 = MIDI 60
        assert!(expected_at(&notes, 1.5, 0.0).is_empty()); // end is exclusive
    }

    #[test]
    fn tolerance_widens_the_window_on_both_ends() {
        let chart = flat_chart(vec![note_item(1.0, 0.5, 1, Action::Blow, "C4")]);
        let notes = expected_notes_from_chart(&chart);
        // Exactly on the boundary is excluded with zero tolerance...
        assert!(expected_at(&notes, 0.9, 0.0).is_empty());
        assert!(expected_at(&notes, 1.6, 0.0).is_empty());
        // ...but included once played a little early or a little late.
        assert_eq!(expected_at(&notes, 0.9, 0.25), vec![60]);
        assert_eq!(expected_at(&notes, 1.6, 0.25), vec![60]);
    }

    // ── compare ───────────────────────────────────────────────────────────────

    fn expected(midi: u8) -> ExpectedNote {
        ExpectedNote {
            start_secs: 0.0,
            end_secs: 1.0,
            midi,
            blow: true,
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
        let report = compare(&expected, &frames, 0.0);
        assert_eq!(report.true_positive, 1);
        assert_eq!(report.false_negative, 0);
        assert_eq!(report.false_positive, 0);
    }

    #[test]
    fn a_missed_note_is_a_false_negative() {
        let expected = vec![expected(60)];
        let frames = vec![frame(0.5, &[])];
        let report = compare(&expected, &frames, 0.0);
        assert_eq!(report.true_positive, 0);
        assert_eq!(report.false_negative, 1);
        assert_eq!(report.false_positive, 0);
    }

    #[test]
    fn a_detection_with_nothing_expected_is_a_phantom_false_positive() {
        let expected: Vec<ExpectedNote> = vec![];
        let frames = vec![frame(0.5, &[60])];
        let report = compare(&expected, &frames, 0.0);
        assert_eq!(report.false_positive, 1);
        assert_eq!(report.true_positive, 0);
    }

    #[test]
    fn extra_notes_alongside_a_correct_one_still_count_as_phantom() {
        // e.g. NMF's dictionary matching reporting neighboring semitones as
        // "also active" for what's actually one clean note.
        let expected = vec![expected(60)];
        let frames = vec![frame(0.5, &[59, 60, 61])];
        let report = compare(&expected, &frames, 0.0);
        assert_eq!(report.true_positive, 1);
        assert_eq!(report.false_negative, 0);
        assert_eq!(report.false_positive, 2);
    }

    #[test]
    fn confusion_pairs_are_sorted_most_frequent_first() {
        let expected = vec![expected(60)];
        let frames = vec![frame(0.1, &[60]), frame(0.2, &[60]), frame(0.3, &[62])];
        let report = compare(&expected, &frames, 0.0);
        // (want=[60], got=[60]) occurred twice; (want=[60], got=[62]) once.
        assert_eq!(report.confusion[0], (vec![60], vec![60], 2));
        assert_eq!(report.confusion[1], (vec![60], vec![62], 1));
    }

    #[test]
    fn tied_confusion_pairs_are_ordered_by_pitch_set() {
        let expected = vec![expected(60)];
        let frames = vec![frame(0.1, &[64]), frame(0.2, &[62]), frame(0.3, &[])];
        let report = compare(&expected, &frames, 0.0);
        let detected: Vec<&[u8]> = report.confusion.iter().map(|c| c.1.as_slice()).collect();
        assert_eq!(detected, vec![&[][..], &[62], &[64]]);
    }

    // ── precision, recall and chords ─────────────────────────────────────────

    #[test]
    fn precision_and_recall_come_from_the_per_note_counts() {
        let expected = vec![expected(60), expected(64)];
        // One of two found, plus one phantom.
        let report = compare(&expected, &[frame(0.5, &[60, 62])], 0.0);
        assert_eq!(report.precision(), Some(0.5));
        assert_eq!(report.recall(), Some(0.5));
        assert_eq!(compare(&[], &[], 0.0).recall(), None, "nothing to measure");
    }

    #[test]
    fn a_chord_counts_only_when_the_detected_set_is_exactly_it() {
        let chord = vec![expected(60), expected(64), expected(67)];
        let frames = vec![
            frame(0.1, &[60, 64, 67]),     // exact
            frame(0.2, &[60, 64]),         // one missing
            frame(0.3, &[60, 64, 67, 72]), // one extra
        ];
        let report = compare(&chord, &frames, 0.0);
        assert_eq!(report.chord_frames_expected, 3);
        assert_eq!(report.chord_frames_detected, 3);
        assert_eq!(report.chord_frames_exact, 1);
        // Per-note recall is 8/9 here, far kinder than a chord actually
        // scoring one frame in three.
        assert!(report.recall().unwrap() > 0.85);
        assert!((report.chord_recall().unwrap() - 1.0 / 3.0).abs() < 1e-6);
    }

    // ── timing ───────────────────────────────────────────────────────────────

    fn note(midi: u8, start: f64, end: f64) -> ExpectedNote {
        ExpectedNote {
            start_secs: start,
            end_secs: end,
            midi,
            blow: true,
            label: String::new(),
        }
    }

    /// A 10 ms frame grid where `midi` is detected over `[on, off)`.
    fn detected_between(midi: u8, on: f64, off: f64, until: f64) -> Vec<Frame> {
        (0..=(until * 100.0) as usize)
            .map(|i| {
                let t = i as f64 / 100.0;
                let d: &[u8] = if t >= on - 1e-9 && t < off - 1e-9 {
                    &[midi]
                } else {
                    &[]
                };
                frame(t, d)
            })
            .collect()
    }

    #[test]
    fn onset_and_release_latency_are_measured_from_the_note_edges() {
        let frames = detected_between(60, 1.05, 2.08, 3.0);
        let timings = note_timings(&[note(60, 1.0, 2.0)], &frames, 0.25);
        assert_eq!(timings.never_detected, 0);
        assert!((timings.onset_secs[0] - 0.05).abs() < 1e-9);
        assert!((timings.release_secs[0] - 0.08).abs() < 1e-9);
    }

    #[test]
    fn a_dropout_mid_note_reads_as_an_early_release() {
        let frames = detected_between(60, 1.05, 1.60, 3.0);
        let timings = note_timings(&[note(60, 1.0, 2.0)], &frames, 0.25);
        assert!((timings.release_secs[0] + 0.40).abs() < 1e-9);
    }

    #[test]
    fn a_repeated_pitch_has_no_release_to_measure_between_the_two() {
        let frames = detected_between(60, 1.05, 3.10, 4.0);
        let expected = [note(60, 1.0, 2.0), note(60, 2.0, 3.0)];
        let timings = note_timings(&expected, &frames, 0.25);
        assert_eq!(timings.onset_secs.len(), 2);
        assert_eq!(timings.release_secs.len(), 1, "only the second note's");
    }

    #[test]
    fn a_missed_note_is_counted_not_timed() {
        let frames = detected_between(62, 1.0, 2.0, 3.0);
        let timings = note_timings(&[note(60, 1.0, 2.0)], &frames, 0.25);
        assert_eq!(timings.never_detected, 1);
        assert!(timings.onset_secs.is_empty());
    }

    #[test]
    fn the_median_takes_the_middle_or_the_mean_of_the_two_middles() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
    }

    // ── direction ────────────────────────────────────────────────────────────

    #[test]
    fn direction_accuracy_asks_whether_the_breath_is_right_not_the_pitch() {
        let harp = richter_harp("C");
        // Hole 4 blow (C5) expected.
        let blow = vec![note(72, 0.0, 1.0)];
        let frames = vec![
            frame(0.1, &[72]), // right
            frame(0.2, &[76]), // wrong pitch, but hole 5 blow: right breath
            frame(0.3, &[74]), // hole 4 draw: wrong breath
            frame(0.4, &[]),   // nothing detected: not counted
        ];
        let accuracy = direction_accuracy(&harp, &blow, &frames, 0.0).unwrap();
        assert!((accuracy - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn a_pitch_on_both_breaths_matches_either() {
        let harp = richter_harp("C");
        // G4 is hole 3 blow and hole 2 draw on a C harp.
        let mut draw = note(67, 0.0, 1.0);
        draw.blow = false;
        let accuracy = direction_accuracy(&harp, &[draw], &[frame(0.5, &[67])], 0.0);
        assert_eq!(accuracy, Some(1.0));
    }
}
