// SPDX-License-Identifier: MIT

//! Synthesizes a benchmark dataset in the same on-disk shape
//! `song_editor::debug_record` writes under `assets/debug_songs/<name>/`
//! (`recording.wav` + `expected.harpchart`), so `note_bench` has something
//! reproducible to run against without a real harmonica/microphone take.
//! Driven by the `gen_synthetic_dataset` binary
//! (`src/bin/gen_synthetic_dataset.rs`).
//!
//! This is a stand-in for the real thing, not a replacement: `Harmonica
//! Note Detection Roadmap.md`'s "Immediate Next Step" calls for *recorded*
//! coverage of every note/bend/chord/octave, and a synthesized reed is far
//! cleaner than a real one (no room, no mic coloration, no embouchure
//! noise) — this only exercises the benchmark *pipeline* and gives
//! detector algorithms a first, easy pass/fail signal on material with
//! perfectly known ground truth. Real debug recordings still need to
//! happen once a harmonica is back in hand.
//!
//! Reuses the same additive harmonica voice (`harmonicon_core::synth`) that
//! already renders the Song Editor's Play/Practice preview and
//! `gameplay::call_response`'s call demo, rather than a separate tone
//! generator, so the synthetic audio at least carries this crate's own
//! harmonica timbre (breath noise, attack/release, harmonic stack).

use std::path::{Path, PathBuf};

use crate::note_bench::DEFAULT_TIMING_TOLERANCE_SECS;
use harmonicon_core::chart::{
    Action, CURRENT_FORMAT_VERSION, Difficulty, HarpChart, Metadata, NoteEvent, PlayMode, Scoring,
    Song, TempoPoint, Timing, TrackItem,
};
use harmonicon_core::harmonica::{Harmonica, hole_notes, richter_harp};
use harmonicon_core::midi::{midi_to_freq_hz, midi_to_note, note_to_midi};
use harmonicon_core::synth::{Expr, PhraseNote, SAMPLE_RATE, render_pcm};
use harmonicon_core::wav::encode_wav;

/// Sustain per synthesized note.
const NOTE_DUR_SECS: f64 = 0.45;
/// Silence between notes. Long enough for two independent reasons: the
/// pipeline's RMS silence gate (`harmonicon_dsp`'s `SILENCE_THRESHOLD`) needs a clean gap to separate one note's analysis
/// frames from the next, and — the tighter constraint —
/// `note_bench::expected_at` widens each note's window by
/// `DEFAULT_TIMING_TOLERANCE_SECS` (±0.25s) on *both* ends before checking
/// what's "expected" at an instant. If the gap is shorter than twice that
/// tolerance, consecutive notes' widened windows overlap and swallow the
/// silence between them entirely — real silence then gets scored as a
/// missed detection for both neighboring pitches instead of correctly
/// contributing nothing, inflating every algorithm's miss count
/// (including otherwise-solid monophonic ones) for reasons that have
/// nothing to do with detection quality. Comfortably over
/// `2 * DEFAULT_TIMING_TOLERANCE_SECS` (0.5s) leaves genuine untouched
/// silence on both sides.
const GAP_SECS: f64 = 0.65;

const _GAP_LEAVES_GENUINE_SILENCE: () = assert!(GAP_SECS > 2.0 * DEFAULT_TIMING_TOLERANCE_SECS);
const STEP_SECS: f64 = NOTE_DUR_SECS + GAP_SECS;

/// Ticks per second for the internal tick grid handed to `render_pcm` — an
/// arbitrarily fine resolution (1 ms) for placing notes, unrelated to any
/// chart's own musical tick resolution (this dataset's `HarpChart`s place
/// every `TrackItem` by `time` in seconds, never `tick`).
const TICK_HZ: f64 = 1000.0;

fn to_midi_u8(note: &str) -> Option<u8> {
    u8::try_from(note_to_midi(note)?).ok()
}

struct SyntheticEvent {
    hole: u8,
    action: Action,
    midi: u8,
}

struct SyntheticItem {
    start_secs: f64,
    duration_secs: f64,
    events: Vec<SyntheticEvent>,
}

struct SyntheticScenario {
    name: &'static str,
    items: Vec<SyntheticItem>,
}

// ── Scenario builders (pure — no audio, no file I/O) ────────────────────────

/// Every natural blow/draw note on `harp`, one at a time.
fn single_notes_scenario(harp: &Harmonica) -> SyntheticScenario {
    let mut items = Vec::new();
    let mut t = GAP_SECS;
    for hole in 1..=harp.hole_count() {
        let hn = hole_notes(harp, hole);
        for (action, note) in [(Action::Blow, &hn.blow), (Action::Draw, &hn.draw)] {
            let Some(midi) = note.as_deref().and_then(to_midi_u8) else {
                continue;
            };
            items.push(SyntheticItem {
                start_secs: t,
                duration_secs: NOTE_DUR_SECS,
                events: vec![SyntheticEvent { hole, action, midi }],
            });
            t += STEP_SECS;
        }
    }
    SyntheticScenario { name: "single_notes", items }
}

/// Every bend on `harp` (draw bends on holes 1-6, blow bends on 7-10 — see
/// `harmonicon_core::harmonica::hole_notes`), one at a time.
fn bends_scenario(harp: &Harmonica) -> SyntheticScenario {
    let mut items = Vec::new();
    let mut t = GAP_SECS;
    for hole in 1..=harp.hole_count() {
        let hn = hole_notes(harp, hole);
        let action = if hole <= 6 { Action::Draw } else { Action::Blow };
        for bend in &hn.bends {
            let Some(midi) = to_midi_u8(bend) else {
                continue;
            };
            items.push(SyntheticItem {
                start_secs: t,
                duration_secs: NOTE_DUR_SECS,
                events: vec![SyntheticEvent { hole, action, midi }],
            });
            t += STEP_SECS;
        }
    }
    SyntheticScenario { name: "bends", items }
}

/// Every overblow/overdraw `harp` supports, one at a time.
fn overblows_overdraws_scenario(harp: &Harmonica) -> SyntheticScenario {
    let mut items = Vec::new();
    let mut t = GAP_SECS;
    for hole in 1..=harp.hole_count() {
        let hn = hole_notes(harp, hole);
        let Some(midi) = hn.over.as_deref().and_then(to_midi_u8) else {
            continue;
        };
        let action = if matches!(hole, 1 | 4 | 5 | 6) { Action::Blow } else { Action::Draw };
        items.push(SyntheticItem {
            start_secs: t,
            duration_secs: NOTE_DUR_SECS,
            events: vec![SyntheticEvent { hole, action, midi }],
        });
        t += STEP_SECS;
    }
    SyntheticScenario { name: "overblows_overdraws", items }
}

/// A couple of representative same-direction adjacent-hole chords (e.g. the
/// classic holes-1-2-3 blow "train" chord) — legal since one breath can
/// sound several reeds in the same direction at once; only mixing blow and
/// draw simultaneously is physically impossible (see
/// `harmonicon_core::harmonica_constraints`).
fn chords_scenario(harp: &Harmonica) -> SyntheticScenario {
    let mut items = Vec::new();
    let mut t = GAP_SECS;
    let hole_count = harp.hole_count();
    let groups: [(Action, [u8; 3]); 2] = [(Action::Blow, [1, 2, 3]), (Action::Draw, [2, 3, 4])];
    for (action, holes) in groups {
        if holes.iter().any(|&h| h > hole_count) {
            continue;
        }
        let mut events = Vec::new();
        for &hole in &holes {
            let hn = hole_notes(harp, hole);
            let note = match action {
                Action::Blow => &hn.blow,
                Action::Draw => &hn.draw,
            };
            if let Some(midi) = note.as_deref().and_then(to_midi_u8) {
                events.push(SyntheticEvent { hole, action, midi });
            }
        }
        if events.len() < 2 {
            continue;
        }
        items.push(SyntheticItem { start_secs: t, duration_secs: NOTE_DUR_SECS, events });
        t += STEP_SECS;
    }
    SyntheticScenario { name: "chords", items }
}

/// Up to 4 same-direction hole pairs a full octave apart (e.g. holes 1 & 4
/// blow on a Richter harp), found by scanning every hole pair rather than
/// hardcoding one tuning's layout.
fn octaves_scenario(harp: &Harmonica) -> SyntheticScenario {
    let mut items = Vec::new();
    let mut t = GAP_SECS;
    let hole_count = harp.hole_count();
    let mut found = 0u32;
    'outer: for hole_a in 1..=hole_count {
        let hn_a = hole_notes(harp, hole_a);
        for hole_b in (hole_a + 1)..=hole_count {
            let hn_b = hole_notes(harp, hole_b);
            for (action, note_a, note_b) in
                [(Action::Blow, &hn_a.blow, &hn_b.blow), (Action::Draw, &hn_a.draw, &hn_b.draw)]
            {
                let (Some(ma), Some(mb)) = (
                    note_a.as_deref().and_then(to_midi_u8),
                    note_b.as_deref().and_then(to_midi_u8),
                ) else {
                    continue;
                };
                if mb as i32 - ma as i32 != 12 {
                    continue;
                }
                items.push(SyntheticItem {
                    start_secs: t,
                    duration_secs: NOTE_DUR_SECS,
                    events: vec![
                        SyntheticEvent { hole: hole_a, action, midi: ma },
                        SyntheticEvent { hole: hole_b, action, midi: mb },
                    ],
                });
                t += STEP_SECS;
                found += 1;
                if found >= 4 {
                    break 'outer;
                }
            }
        }
    }
    SyntheticScenario { name: "octaves", items }
}

fn build_scenarios(harp: &Harmonica) -> Vec<SyntheticScenario> {
    vec![
        single_notes_scenario(harp),
        bends_scenario(harp),
        overblows_overdraws_scenario(harp),
        chords_scenario(harp),
        octaves_scenario(harp),
    ]
}

// ── Rendering (pure: scenario -> PCM + HarpChart) ────────────────────────────

fn render_scenario(
    key: &str,
    harp: &Harmonica,
    scenario: &SyntheticScenario,
) -> (Vec<f32>, HarpChart) {
    let mut phrase_notes = Vec::new();
    let mut track = Vec::new();
    for item in &scenario.items {
        let tick = (item.start_secs * TICK_HZ).round() as usize;
        let len = (item.duration_secs * TICK_HZ).round() as usize;
        let mut events = Vec::new();
        for ev in &item.events {
            phrase_notes.push(PhraseNote {
                tick,
                len,
                freq: Some(midi_to_freq_hz(ev.midi as f32)),
                expr: Expr::None,
            });
            events.push(NoteEvent {
                hole: ev.hole,
                action: ev.action,
                note: Some(midi_to_note(ev.midi as i32)),
                modifiers: None,
            });
        }
        track.push(TrackItem {
            id: None,
            time: Some(item.start_secs),
            tick: None,
            duration: item.duration_secs,
            phrase: None,
            groove: None,
            chord: None,
            play_mode: Some(if events.len() > 1 { PlayMode::Chord } else { PlayMode::Single }),
            call: false,
            lyric: None,
            events,
        });
    }

    let samples = render_pcm(&phrase_notes, (1.0 / TICK_HZ) as f32);
    let chart = HarpChart {
        metadata: Some(Metadata {
            format_version: Some(CURRENT_FORMAT_VERSION.to_string()),
            author: Some("gen_synthetic_dataset".into()),
            source: Some(
                "Programmatically synthesized (src/synthetic_dataset.rs) — not a real \
                 recording; a stand-in for note_bench pipeline testing until a real debug \
                 recording is made."
                    .into(),
            ),
            license: None,
            description: Some(format!("Synthetic '{}' scenario, {} harp", scenario.name, key)),
        }),
        song: Song {
            title: format!("Synthetic {} — {}", key, scenario.name),
            artist: "note_bench synthetic".into(),
            genre: "Synthetic".into(),
            tempo_bpm: 120.0,
            key: key.to_string(),
            time_signature: None,
            difficulty: Difficulty::Easy,
            feel: None,
        },
        timing: Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint { tick: 0, bpm: 120.0 }],
            time_signature_map: None,
            pickup_ticks: None,
            repeats: Vec::new(),
        },
        harmonica: harp.clone(),
        track,
        loop_section: None,
        scoring: Scoring {
            perfect_window_ms: 60,
            good_window_ms: 120,
            miss_window_ms: 220,
            combo: None,
            style_bonus: None,
        },
    };
    (samples, chart)
}

// ── File I/O ──────────────────────────────────────────────────────────────────

/// Renders and writes one scenario as `<out_root>/synthetic_<key>_<scenario>/
/// {recording.wav, expected.harpchart}` — the two files `note_bench`
/// actually reads (see `src/bin/note_bench.rs`); no `recorded.harpchart`/
/// `recording.json` since nothing here ever ran a live detector to produce
/// them.
fn write_scenario(
    out_root: &Path,
    key: &str,
    harp: &Harmonica,
    scenario: &SyntheticScenario,
) -> std::io::Result<PathBuf> {
    let (samples, chart) = render_scenario(key, harp, scenario);
    let dir = out_root.join(format!("synthetic_{}_{}", key.to_lowercase(), scenario.name));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("recording.wav"), encode_wav(&samples, SAMPLE_RATE))?;
    let chart_json = serde_json::to_string_pretty(&chart)
        .expect("HarpChart serialization is infallible for well-formed field values");
    std::fs::write(dir.join("expected.harpchart"), chart_json)?;
    Ok(dir)
}

/// Generates and writes every scenario for every harp this module covers
/// (currently just a standard C Richter diatonic — the common case; add
/// more `(key, Harmonica)` pairs here to widen coverage) under `out_root`.
/// Returns the directories written, skipping any scenario that ended up
/// with nothing in it for a given harp.
pub fn write_all(out_root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (key, harp) in [("C", richter_harp("C"))] {
        for scenario in build_scenarios(&harp) {
            if scenario.items.is_empty() {
                continue;
            }
            written.push(write_scenario(out_root, key, &harp, &scenario)?);
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_notes_covers_every_hole_blow_and_draw_on_a_richter_harp() {
        let harp = richter_harp("C");
        let scenario = single_notes_scenario(&harp);
        // 10 holes * blow + draw, all present on a standard Richter layout.
        assert_eq!(scenario.items.len(), 20);
        assert!(scenario.items.iter().all(|i| i.events.len() == 1));
    }

    #[test]
    fn bends_scenario_is_nonempty_on_a_richter_harp() {
        let harp = richter_harp("C");
        let scenario = bends_scenario(&harp);
        assert!(!scenario.items.is_empty());
    }

    #[test]
    fn chords_scenario_produces_multi_event_items() {
        let harp = richter_harp("C");
        let scenario = chords_scenario(&harp);
        assert_eq!(scenario.items.len(), 2);
        assert!(scenario.items.iter().all(|i| i.events.len() == 3));
    }

    #[test]
    fn octaves_scenario_finds_an_exact_octave_pair() {
        let harp = richter_harp("C");
        let scenario = octaves_scenario(&harp);
        assert!(!scenario.items.is_empty());
        for item in &scenario.items {
            assert_eq!(item.events.len(), 2);
            let diff = item.events[1].midi as i32 - item.events[0].midi as i32;
            assert_eq!(diff, 12);
        }
    }

    #[test]
    fn render_scenario_produces_one_track_item_per_scenario_item() {
        let harp = richter_harp("C");
        let scenario = single_notes_scenario(&harp);
        let (samples, chart) = render_scenario("C", &harp, &scenario);
        assert_eq!(chart.track.len(), scenario.items.len());
        assert!(!samples.is_empty());
    }

    #[test]
    fn render_scenario_output_round_trips_through_expected_notes_from_chart() {
        use crate::note_bench::expected_notes_from_chart;
        let harp = richter_harp("C");
        let scenario = chords_scenario(&harp);
        let (_, chart) = render_scenario("C", &harp, &scenario);
        let expected = expected_notes_from_chart(&chart);
        // Each chord item contributes 3 expected notes (one per event).
        assert_eq!(expected.len(), scenario.items.len() * 3);
    }
}
