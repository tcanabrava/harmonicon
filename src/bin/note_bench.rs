// SPDX-License-Identifier: MIT

//! Offline pitch-detection benchmark: replays every "debug recording"
//! (`song_editor::debug_record`, the Song Editor's dev-only "Debug
//! Recording" checkbox) under `assets/debug_songs/<song>/` through each of
//! the five selectable algorithms and prints per-algorithm hit/miss/phantom
//! counts, analysis time per chunk, and common confusion pairs, then notes
//! heard, frame precision/recall, exact-set chord precision/recall,
//! direction accuracy and median onset/release latency — raw and through
//! the live constraint — ending with one summary row per recording and
//! detector. Compares against
//! `expected.harpchart` — hand-annotated ground truth, placed via the Song
//! Editor's "Draw correct notes" mode (`song_editor::expected_notes`), not
//! `recorded.harpchart` (whatever the live detector produced when the take
//! was made — using that as the comparison target would let a detection
//! miss "confirm" itself).
//!
//! This is the "Immediate Next Step"/"Analyze the Errors" tooling
//! `Harmonicon Note Detection Roadmap.md` calls for — a reproducible
//! benchmark, meant to be run repeatedly as more debug recordings pile up,
//! *before* changing any detection algorithm. The comparison logic itself
//! lives in `harmonicon_bench::note_bench` (unit-tested there); this binary is
//! just file I/O and a printout.
//!
//! Usage: `cargo run --bin note_bench [-- <path> [tolerance_secs]]`, `path`
//! defaulting to `assets/debug_songs` and `tolerance_secs` to
//! [`DEFAULT_TIMING_TOLERANCE_SECS`] — how far early/late a note may land
//! against the chart's own clock and still count as "expected" at that
//! instant (see that constant's own doc comment for why a played-along
//! take needs this at all: it's never going to line up sample-accurately,
//! and this benchmark measures pitch detection, not rhythm).

use harmonicon_bench::note_bench::{
    AlgorithmReport, DEFAULT_TIMING_TOLERANCE_SECS, ExpectedNote, Frame, apply_live_constraints,
    compare, direction_accuracy, expected_notes_from_chart, median, note_timings, run_algorithm,
};
use harmonicon_core::wav::decode_wav_pcm16;
use harmonicon_dsp::{PITCH_RANGE_MARGIN_SEMITONES, PitchAlgorithm, PitchRange};
use harmonicon_song::song::chart::HarpChart;
use std::path::{Path, PathBuf};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().unwrap_or_else(|| "assets/debug_songs".to_string());
    let root = Path::new(&root);
    let tolerance_secs =
        args.next().and_then(|s| s.parse::<f64>().ok()).unwrap_or(DEFAULT_TIMING_TOLERANCE_SECS);

    // A missing directory just means no debug recording has been made yet
    // (the folder is only created on first save, see `song_editor::
    // debug_record::write_debug_recording_on_save`) — the expected,
    // friendly-message case, not a real error.
    let mut song_dirs: Vec<PathBuf> = match std::fs::read_dir(root) {
        Ok(entries) => {
            entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            println!("Couldn't read {}: {e}", root.display());
            return;
        }
    };
    song_dirs.sort();

    let mut summary = Vec::new();
    for song_dir in &song_dirs {
        // `expected.harpchart` (hand-annotated ground truth,
        // `song_editor::expected_notes`) is the comparison target — never
        // `recorded.harpchart` (whatever the live detector produced, which
        // would make a detection miss "confirm" itself).
        let chart_path = song_dir.join("expected.harpchart");
        let wav_path = song_dir.join("recording.wav");
        if !chart_path.is_file() || !wav_path.is_file() {
            continue;
        }
        run_one(song_dir, &chart_path, &wav_path, tolerance_secs, &mut summary);
    }

    if !summary.is_empty() {
        print_summary(&summary);
    } else {
        println!(
            "No annotated debug recordings found under {} yet. In the Song \
             Editor (cargo run --features dev): check \"Debug Recording\", \
             play a take, then use \"Draw correct notes\" mode to mark the \
             ground truth on top of it, and Save — that's what writes \
             expected.harpchart alongside the WAV. Then re-run this tool.",
            root.display()
        );
    }
}

/// One row of the closing summary: a recording (scenario) run through one
/// detector, raw or through the live constraint.
struct SummaryRow {
    scenario: String,
    detector: String,
    metrics: Metrics,
}

/// Everything printed for one detector on one recording.
struct Metrics {
    report: AlgorithmReport,
    direction: Option<f32>,
    onset_ms: Option<f64>,
    release_ms: Option<f64>,
    never_detected: u32,
    notes: usize,
}

impl Metrics {
    fn measure(
        harp: &harmonicon_core::harmonica::Harmonica,
        expected: &[ExpectedNote],
        frames: &[Frame],
        tolerance_secs: f64,
    ) -> Self {
        let timings = note_timings(expected, frames, tolerance_secs);
        Self {
            report: compare(expected, frames, tolerance_secs),
            direction: direction_accuracy(harp, expected, frames, tolerance_secs),
            onset_ms: median(&timings.onset_secs).map(|s| s * 1000.0),
            release_ms: median(&timings.release_secs).map(|s| s * 1000.0),
            never_detected: timings.never_detected,
            notes: expected.len(),
        }
    }

    /// The metric columns, shared by the per-recording lines and the summary.
    /// `P`/`R` are frame-level (see `AlgorithmReport::recall` on why R stays
    /// low); `notes` is how many expected notes were heard at all.
    fn columns(&self) -> String {
        let r = &self.report;
        format!(
            "notes {:>3}/{:<3}  frame P {} R {}  chord P/R {}/{}  dir {}  onset {}  release {}",
            self.notes - self.never_detected as usize,
            self.notes,
            share(r.precision()),
            share(r.recall()),
            share(r.chord_precision()),
            share(r.chord_recall()),
            share(self.direction),
            millis(self.onset_ms),
            millis(self.release_ms),
        )
    }
}

/// A 0..1 share as a percentage, or a dash when there was nothing to measure.
fn share(value: Option<f32>) -> String {
    value.map_or_else(|| "  —".to_string(), |v| format!("{:>3.0}%", v * 100.0))
}

/// A median latency in milliseconds, or a dash.
fn millis(value: Option<f64>) -> String {
    value.map_or_else(|| "    —".to_string(), |ms| format!("{ms:>4.0}ms"))
}

fn print_summary(rows: &[SummaryRow]) {
    println!();
    println!("== summary: per recording, per detector ==");
    let width = rows.iter().map(|r| r.scenario.len()).max().unwrap_or(0);
    for row in rows {
        println!("  {:<width$}  {:<7}  {}", row.scenario, row.detector, row.metrics.columns());
    }
}

fn run_one(
    song_dir: &Path,
    chart_path: &Path,
    wav_path: &Path,
    tolerance_secs: f64,
    summary: &mut Vec<SummaryRow>,
) {
    let name = song_dir.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    println!("== {name} == (timing tolerance ±{tolerance_secs:.2}s)");

    let chart_json = match std::fs::read_to_string(chart_path) {
        Ok(s) => s,
        Err(e) => {
            println!("  chart read failed: {e}");
            return;
        }
    };
    let chart: HarpChart = match serde_json::from_str(&chart_json) {
        Ok(c) => c,
        Err(e) => {
            println!("  chart parse failed: {e}");
            return;
        }
    };
    let wav_bytes = match std::fs::read(wav_path) {
        Ok(b) => b,
        Err(e) => {
            println!("  wav read failed: {e}");
            return;
        }
    };
    let Some((samples, _channels, sample_rate)) = decode_wav_pcm16(&wav_bytes) else {
        println!("  wav decode failed (not a 16-bit PCM WAV?)");
        return;
    };

    let expected = expected_notes_from_chart(&chart);
    if expected.is_empty() {
        println!("  chart has no expected notes — skipping");
        return;
    }

    // Narrowed to the harp actually being played, same as gameplay/live
    // recording both do — fewer candidates for every algorithm.
    let range = chart
        .harmonica
        .frequency_range()
        .map(|(lo, hi)| PitchRange::from_freqs([lo, hi], PITCH_RANGE_MARGIN_SEMITONES))
        .unwrap_or_default();

    for &algorithm in PitchAlgorithm::all() {
        let started = std::time::Instant::now();
        let frames = run_algorithm(&samples, sample_rate, algorithm, range);
        let micros_per_chunk = started.elapsed().as_secs_f64() * 1e6 / frames.len().max(1) as f64;
        let raw = Metrics::measure(&chart.harmonica, &expected, &frames, tolerance_secs);
        println!(
            "  {:>5}: hit {:>5}  miss {:>5}  phantom {:>5}  {:>7.1} µs/chunk",
            algorithm.label(),
            raw.report.true_positive,
            raw.report.false_negative,
            raw.report.false_positive,
            micros_per_chunk,
        );
        println!("         {}", raw.columns());
        for (want, got, count) in raw.report.confusion.iter().filter(|(w, d, _)| w != d).take(5) {
            println!("        {count:>4}x  played {want:?} -> detected {got:?}");
        }

        // Same frames, re-filtered through exactly the policy live gameplay
        // applies (`harmonicon_core::harmonica_constraints`'s
        // `HarmonicaNoteTracker`: one wind direction at a time, plus onset
        // confirmation and release) — printed alongside the raw algorithm so
        // the cost and benefit of that stage are visible side by side.
        let constrained = apply_live_constraints(&chart.harmonica, &frames);
        let live = Metrics::measure(&chart.harmonica, &expected, &constrained, tolerance_secs);
        println!(
            "  {:>5}+HC: hit {:>5}  miss {:>5}  phantom {:>5}",
            algorithm.label(),
            live.report.true_positive,
            live.report.false_negative,
            live.report.false_positive,
        );
        println!("         {}", live.columns());

        summary.push(SummaryRow {
            scenario: name.to_string(),
            detector: algorithm.label().to_string(),
            metrics: raw,
        });
        summary.push(SummaryRow {
            scenario: name.to_string(),
            detector: format!("{}+HC", algorithm.label()),
            metrics: live,
        });
    }
}
