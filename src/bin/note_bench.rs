// SPDX-License-Identifier: MIT

//! Offline pitch-detection benchmark: replays every "debug recording"
//! (`song_editor::debug_record`, the Song Editor's dev-only "Debug
//! Recording" checkbox) under `assets/debug_songs/<song>/` through each of
//! the five selectable algorithms and prints a per-algorithm hit/miss/
//! phantom summary plus the most common confusion pairs. Compares against
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
//! lives in `harmonicon::note_bench` (unit-tested there); this binary is
//! just file I/O and a printout.
//!
//! Usage: `cargo run --bin note_bench [-- <path> [tolerance_secs]]`, `path`
//! defaulting to `assets/debug_songs` and `tolerance_secs` to
//! [`DEFAULT_TIMING_TOLERANCE_SECS`] — how far early/late a note may land
//! against the chart's own clock and still count as "expected" at that
//! instant (see that constant's own doc comment for why a played-along
//! take needs this at all: it's never going to line up sample-accurately,
//! and this benchmark measures pitch detection, not rhythm).

use harmonicon::audio_system::pitch_detect::{
    PITCH_RANGE_MARGIN_SEMITONES, PitchAlgorithm, PitchRange,
};
use harmonicon::audio_system::wav::decode_wav_pcm16;
use harmonicon::note_bench::{
    DEFAULT_TIMING_TOLERANCE_SECS, compare, expected_notes_from_chart, run_algorithm,
};
use harmonicon::song::chart::HarpChart;
use std::path::{Path, PathBuf};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .unwrap_or_else(|| "assets/debug_songs".to_string());
    let root = Path::new(&root);
    let tolerance_secs = args
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(DEFAULT_TIMING_TOLERANCE_SECS);

    // A missing directory just means no debug recording has been made yet
    // (the folder is only created on first save, see `song_editor::
    // debug_record::write_debug_recording_on_save`) — the expected,
    // friendly-message case, not a real error.
    let mut song_dirs: Vec<PathBuf> = match std::fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            println!("Couldn't read {}: {e}", root.display());
            return;
        }
    };
    song_dirs.sort();

    let mut ran_any = false;
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
        ran_any = true;
        run_one(song_dir, &chart_path, &wav_path, tolerance_secs);
    }

    if !ran_any {
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

fn run_one(song_dir: &Path, chart_path: &Path, wav_path: &Path, tolerance_secs: f64) {
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
        let frames = run_algorithm(&samples, sample_rate, algorithm, range);
        let report = compare(&expected, &frames, tolerance_secs);
        println!(
            "  {:>5}: hit {:>5}  miss {:>5}  phantom {:>5}",
            algorithm.label(),
            report.true_positive,
            report.false_negative,
            report.false_positive,
        );
        for (want, got, count) in report.confusion.iter().filter(|(w, d, _)| w != d).take(5) {
            println!("        {count:>4}x  played {want:?} -> detected {got:?}");
        }
    }
}
