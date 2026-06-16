// SPDX-License-Identifier: MIT

//! midi-to-chart — turn a MIDI track into a Harmonicon `chart.hpchart`.
//!
//! Usage:
//!   midi-to-chart <file.mid>                 # list the tracks in the file
//!   midi-to-chart <file.mid> "<track name>"  # generate chart.hpchart from that
//!                                            # track, then write
//!                                            # <file>_processed.midi without it
//!
//! The chart is validated against assets/song_schema.dtd.json before being
//! written. MIDI pitches are mapped onto a standard C richter diatonic harp;
//! pitches that aren't directly available are reached with a draw/blow bend
//! where possible, otherwise snapped to the nearest playable note.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use harmonicon::audio_system::midi::{midi_to_note, note_to_midi};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use serde_json::{Value, json};

// Standard C richter diatonic layout (holes 1..=10), matching the in-repo charts.
const BLOW: [&str; 10] = ["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"];
const DRAW: [&str; 10] = ["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"];

const DEFAULT_TEMPO_US: u32 = 500_000; // 120 BPM if the file specifies none

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} <file.mid> [\"track name\"]", args[0]);
        std::process::exit(2);
    }
    let midi_path = PathBuf::from(&args[1]);
    let track_name = args.get(2).cloned();

    let bytes = match std::fs::read(&midi_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", midi_path.display());
            std::process::exit(1);
        }
    };
    let mut smf = match Smf::parse(&bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: not a valid MIDI file: {e}");
            std::process::exit(1);
        }
    };

    let ticks_per_quarter = match smf.header.timing {
        Timing::Metrical(tpq) => tpq.as_int() as u32,
        Timing::Timecode(..) => {
            eprintln!("error: timecode-based MIDI timing is not supported (need metrical)");
            std::process::exit(1);
        }
    };

    match track_name {
        None => list_tracks(&smf),
        Some(name) => {
            // Accept either an exact track name or, since MIDI track names are
            // often non-unique, a numeric track index.
            let by_index = name.parse::<usize>().ok().filter(|&i| i < smf.tracks.len());
            let Some(idx) = by_index.or_else(|| find_track(&smf, &name)) else {
                eprintln!("error: no track named {name:?}. Available tracks:");
                list_tracks(&smf);
                std::process::exit(1);
            };
            // Use the track's real name (not the raw selector) in the chart.
            let display = track_name_of(&smf.tracks[idx]).unwrap_or_else(|| format!("track {idx}"));
            process_track(&smf, idx, &display, ticks_per_quarter, &midi_path);

            // Remove the track and write the leftover MIDI.
            smf.tracks.remove(idx);
            let out = processed_path(&midi_path);
            match smf.save(&out) {
                Ok(()) => println!("Wrote {} (track {idx} removed)", out.display()),
                Err(e) => {
                    eprintln!("error: failed to write {}: {e}", out.display());
                    std::process::exit(1);
                }
            }
        }
    }
}

/// The display name of a track, from its first TrackName meta event.
fn track_name_of(track: &[midly::TrackEvent]) -> Option<String> {
    track.iter().find_map(|ev| match ev.kind {
        TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
        _ => None,
    })
}

fn note_on_count(track: &[midly::TrackEvent]) -> usize {
    track
        .iter()
        .filter(|ev| {
            matches!(
                ev.kind,
                TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0
            )
        })
        .count()
}

fn list_tracks(smf: &Smf) {
    println!("{} track(s):", smf.tracks.len());
    for (i, track) in smf.tracks.iter().enumerate() {
        let name = track_name_of(track).unwrap_or_else(|| "<unnamed>".to_string());
        println!("  [{i}] {name}  ({} notes)", note_on_count(track));
    }
}

fn find_track(smf: &Smf, name: &str) -> Option<usize> {
    smf.tracks
        .iter()
        .position(|t| track_name_of(t).as_deref() == Some(name))
}

fn processed_path(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("song");
    let mut out = original.to_path_buf();
    out.set_file_name(format!("{stem}_processed.midi"));
    out
}

// ── Tempo handling ──────────────────────────────────────────────────────────

/// All tempo changes across the file as `(absolute_tick, microseconds_per_quarter)`,
/// sorted, with an implicit 120 BPM entry at tick 0 if none is given there.
fn collect_tempo_map(smf: &Smf) -> Vec<(u64, u32)> {
    let mut changes: Vec<(u64, u32)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = ev.kind {
                changes.push((tick, us.as_int()));
            }
        }
    }
    changes.sort_by_key(|&(t, _)| t);
    if changes.first().map(|&(t, _)| t) != Some(0) {
        changes.insert(0, (0, DEFAULT_TEMPO_US));
    }
    changes
}

/// Absolute time in seconds of a tick, integrating across tempo changes.
fn tick_to_seconds(tick: u64, tpq: u32, tempo: &[(u64, u32)]) -> f64 {
    let mut seconds = 0.0;
    let mut prev_tick = 0u64;
    let mut us = tempo.first().map(|&(_, u)| u).unwrap_or(DEFAULT_TEMPO_US);
    for &(change_tick, change_us) in tempo {
        if change_tick >= tick {
            break;
        }
        let span = change_tick - prev_tick;
        seconds += span as f64 * us as f64 / tpq as f64 / 1_000_000.0;
        prev_tick = change_tick;
        us = change_us;
    }
    seconds += (tick - prev_tick) as f64 * us as f64 / tpq as f64 / 1_000_000.0;
    seconds
}

fn first_time_signature(smf: &Smf) -> (u8, u8) {
    for track in &smf.tracks {
        for ev in track {
            if let TrackEventKind::Meta(MetaMessage::TimeSignature(num, denom_pow, _, _)) = ev.kind
            {
                return (num, 1u8 << denom_pow);
            }
        }
    }
    (4, 4)
}

// ── Note extraction & harmonica mapping ──────────────────────────────────────

struct Note {
    start_tick: u64,
    dur_ticks: u64,
    key: u8,
}

/// Pairs NoteOn/NoteOff into `Note`s, ordered by start tick.
fn extract_notes(track: &[midly::TrackEvent]) -> Vec<Note> {
    let mut open: HashMap<u8, u64> = HashMap::new(); // key -> start tick
    let mut notes: Vec<Note> = Vec::new();
    let mut tick: u64 = 0;
    for ev in track {
        tick += ev.delta.as_int() as u64;
        if let TrackEventKind::Midi { message, .. } = ev.kind {
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    open.insert(key.as_int(), tick);
                }
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    if let Some(start) = open.remove(&key.as_int()) {
                        notes.push(Note {
                            start_tick: start,
                            dur_ticks: tick.saturating_sub(start),
                            key: key.as_int(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    notes.sort_by_key(|n| (n.start_tick, n.key));
    notes
}

/// A resolved playing instruction for one MIDI pitch.
struct Mapped {
    hole: u8,
    action: &'static str,
    /// The harmonica's natural note for that hole/action (e.g. the un-bent draw).
    natural: u8,
    /// Bend depth in semitones (negative), if the pitch needs a bend.
    bend: Option<i32>,
}

fn build_pitch_maps() -> (HashMap<u8, u8>, HashMap<u8, u8>) {
    let mut blow = HashMap::new();
    let mut draw = HashMap::new();
    for (i, n) in BLOW.iter().enumerate() {
        if let Some(m) = note_to_midi(n) {
            blow.insert(m as u8, (i + 1) as u8);
        }
    }
    for (i, n) in DRAW.iter().enumerate() {
        if let Some(m) = note_to_midi(n) {
            draw.insert(m as u8, (i + 1) as u8);
        }
    }
    (blow, draw)
}

fn map_pitch(target: u8, blow: &HashMap<u8, u8>, draw: &HashMap<u8, u8>) -> Mapped {
    // Directly playable.
    if let Some(&hole) = blow.get(&target) {
        return Mapped {
            hole,
            action: "blow",
            natural: target,
            bend: None,
        };
    }
    if let Some(&hole) = draw.get(&target) {
        return Mapped {
            hole,
            action: "draw",
            natural: target,
            bend: None,
        };
    }
    // Draw bend: holes 1..=6 can bend the draw note down by 1..=3 semitones.
    for k in 1..=3u8 {
        if let Some(&hole) = draw.get(&(target + k)) {
            if (1..=6).contains(&hole) {
                return Mapped {
                    hole,
                    action: "draw",
                    natural: target + k,
                    bend: Some(-(k as i32)),
                };
            }
        }
    }
    // Blow bend: high holes 8..=10 can bend the blow note down.
    for k in 1..=3u8 {
        if let Some(&hole) = blow.get(&(target + k)) {
            if (8..=10).contains(&hole) {
                return Mapped {
                    hole,
                    action: "blow",
                    natural: target + k,
                    bend: Some(-(k as i32)),
                };
            }
        }
    }
    // Fallback: snap to the nearest playable natural note.
    let mut best_action = "blow";
    let mut best_natural = target;
    let mut best_dist = u8::MAX;
    for (map, action) in [(blow, "blow"), (draw, "draw")] {
        for &m in map.keys() {
            let d = m.abs_diff(target);
            if d < best_dist {
                best_dist = d;
                best_action = action;
                best_natural = m;
            }
        }
    }
    let hole = if best_action == "blow" {
        blow[&best_natural]
    } else {
        draw[&best_natural]
    };
    Mapped {
        hole,
        action: best_action,
        natural: best_natural,
        bend: None,
    }
}

// ── Chart generation ─────────────────────────────────────────────────────────

fn process_track(smf: &Smf, idx: usize, name: &str, tpq: u32, midi_path: &Path) {
    let tempo = collect_tempo_map(smf);
    let (ts_num, ts_den) = first_time_signature(smf);
    let initial_bpm = (60_000_000.0 / tempo[0].1 as f64).clamp(20.0, 300.0);

    let notes = extract_notes(&smf.tracks[idx]);
    if notes.is_empty() {
        eprintln!("error: track {name:?} has no notes");
        std::process::exit(1);
    }

    let (blow, draw) = build_pitch_maps();

    // Group notes that start on the same tick into one chart item (a chord).
    let mut items: Vec<Value> = Vec::new();
    let mut i = 0;
    while i < notes.len() {
        let start = notes[i].start_tick;
        let mut group_end = i;
        while group_end < notes.len() && notes[group_end].start_tick == start {
            group_end += 1;
        }
        let group = &notes[i..group_end];
        i = group_end;

        let time = tick_to_seconds(start, tpq, &tempo);
        let max_dur_ticks = group.iter().map(|n| n.dur_ticks).max().unwrap_or(0);
        let dur = (tick_to_seconds(start + max_dur_ticks, tpq, &tempo) - time).max(0.05);

        let events: Vec<Value> = group
            .iter()
            .map(|n| {
                let m = map_pitch(n.key, &blow, &draw);
                let mut ev = json!({
                    "hole": m.hole,
                    "action": m.action,
                    "note": midi_to_note(m.natural as i32),
                });
                if let Some(semis) = m.bend {
                    ev["modifiers"] = json!([{ "type": "bend", "semitones": semis }]);
                }
                ev
            })
            .collect();

        items.push(json!({
            "id": format!("n{:04}", items.len() + 1),
            "time": round3(time),
            "duration": round3(dur),
            "play_mode": if events.len() > 1 { "chord" } else { "single" },
            "events": events,
        }));
    }

    let tempo_map: Vec<Value> = tempo
        .iter()
        .map(|&(tick, us)| json!({ "tick": tick, "bpm": round3((60_000_000.0 / us as f64).clamp(20.0, 300.0)) }))
        .collect();

    let song_title = midi_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let last_index = items.len() - 1;
    let chart = json!({
        "metadata": {
            "format_version": "1.0.0",
            "author": "midi-to-chart",
            "source": midi_path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            "license": "Generated from MIDI",
            "description": format!("Generated from MIDI track '{name}'."),
        },
        "song": {
            "title": song_title,
            "artist": "Unknown",
            "tempo_bpm": round3(initial_bpm),
            "key": "C",
            "time_signature": format!("{ts_num}/{ts_den}"),
            "difficulty": "intermediate",
        },
        "timing": {
            "resolution": tpq,
            "tempo_map": tempo_map,
        },
        "harmonica": {
            "type": "diatonic",
            "holes": 10,
            "position": "1st",
            "bending_profile": "richter_standard",
            "layout": { "blow": BLOW, "draw": DRAW },
        },
        "track": items,
        "loop": { "type": "full", "repeat": false, "start_index": 0, "end_index": last_index },
        "scoring": {
            "perfect_window_ms": 60,
            "good_window_ms": 120,
            "miss_window_ms": 220,
            "combo": { "enabled": true, "base_multiplier": 1.0, "step_multiplier": 0.1, "max_multiplier": 4.0, "decay_ms": 2000 },
            "style_bonus": { "bend": 50, "vibrato": 25, "wah-wah": 40 },
        },
        "fx_mapping": { "bend": "pitch_bend", "vibrato": "pitch_lfo", "wah-wah": "filter_lfo" },
    });

    validate(&chart);

    let out = PathBuf::from("chart.hpchart");
    match std::fs::write(&out, serde_json::to_string_pretty(&chart).unwrap()) {
        Ok(()) => println!(
            "Wrote {} ({} notes -> {} items, {:.1} BPM, {ts_num}/{ts_den})",
            out.display(),
            notes.len(),
            last_index + 1,
            initial_bpm,
        ),
        Err(e) => {
            eprintln!("error: failed to write {}: {e}", out.display());
            std::process::exit(1);
        }
    }
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Validate the chart against the schema if it can be found; warn (don't fail)
/// when the schema isn't reachable from the working directory.
fn validate(chart: &Value) {
    let Ok(schema_text) = std::fs::read_to_string("assets/song_schema.dtd.json") else {
        eprintln!("warning: assets/song_schema.dtd.json not found; skipping validation");
        return;
    };
    let schema: Value = serde_json::from_str(&schema_text).expect("schema is valid JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(chart)
        .map(|e| format!("  - {e} (at /{})", e.instance_path))
        .collect();
    if !errors.is_empty() {
        eprintln!("error: generated chart failed schema validation:");
        eprintln!("{}", errors.join("\n"));
        std::process::exit(1);
    }
}
