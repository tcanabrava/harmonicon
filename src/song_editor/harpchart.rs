// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::playback::{
    C_BLOW, C_BLOW_CHROMATIC, C_BLOW_SLIDE_CHROMATIC, C_DRAW, C_DRAW_CHROMATIC,
    C_DRAW_SLIDE_CHROMATIC, key_offset,
};
use super::state::{
    Dir, EditorState, Expr, GridNote, HARP_KEYS, HarmonicaKind, POSITIONS, Pitch, Scroll,
};
use super::{LOAD_PURPOSE, MUSIC_PURPOSE, SAVE_PURPOSE, TICKS_PER_BEAT};
use crate::audio_system::midi::{midi_to_note, note_to_midi};
use crate::dialogs::file_dialog::FileChosen;

// ── Serialisation ────────────────────────────────────────────────────────────

/// Resolves `n`'s note name, choosing the diatonic or chromatic reference
/// layout (and the slide table when `n.pitch` is `Pitch::Slide`) to match
/// `kind` — mirrors `playback::note_freq`'s table selection, but returns the
/// note name for the chart's `events[].note` field rather than a frequency.
fn note_name_for(n: &GridNote, key_offset: i32, kind: HarmonicaKind) -> String {
    let idx = (n.hole as usize).saturating_sub(1);
    let base = match (kind, n.dir, n.pitch) {
        (HarmonicaKind::Chromatic, Dir::Blow, Pitch::Slide) => {
            C_BLOW_SLIDE_CHROMATIC.get(idx).copied()
        }
        (HarmonicaKind::Chromatic, Dir::Draw, Pitch::Slide) => {
            C_DRAW_SLIDE_CHROMATIC.get(idx).copied()
        }
        (HarmonicaKind::Chromatic, Dir::Blow, _) => C_BLOW_CHROMATIC.get(idx).copied(),
        (HarmonicaKind::Chromatic, Dir::Draw, _) => C_DRAW_CHROMATIC.get(idx).copied(),
        (HarmonicaKind::Diatonic, Dir::Blow, _) => C_BLOW.get(idx).copied(),
        (HarmonicaKind::Diatonic, Dir::Draw, _) => C_DRAW.get(idx).copied(),
    }
    .unwrap_or("C4");
    let midi = note_to_midi(base).unwrap_or(60) + key_offset;
    midi_to_note(midi)
}

pub(super) fn serialize_harpchart(state: &EditorState) -> String {
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    let bpm: f32 = state.tempo.parse().unwrap_or(120.0);
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let k_off = key_offset(&state.key);

    let mut by_tick: BTreeMap<usize, Vec<&GridNote>> = BTreeMap::new();
    for n in &state.notes {
        by_tick.entry(n.tick).or_default().push(n);
    }

    let track: Vec<Value> = by_tick
        .iter()
        .enumerate()
        .map(|(idx, (&tick, notes))| {
            let max_len = notes.iter().map(|n| n.len).max().unwrap_or(1);
            let duration_secs = max_len as f64 * secs_per_tick as f64;
            let play_mode = if notes.len() == 1 { "single" } else { "chord" };

            let events: Vec<Value> = notes
                .iter()
                .map(|n| {
                    let action = match n.dir {
                        Dir::Blow => "blow",
                        Dir::Draw => "draw",
                    };
                    let note_name = note_name_for(n, k_off, state.harmonica_kind);
                    let mut modifiers: Vec<Value> = Vec::new();
                    match n.pitch {
                        Pitch::Bend(a) => {
                            modifiers.push(json!({ "type": "bend", "semitones": -(a as f64) }));
                        }
                        Pitch::Overblow => modifiers.push(json!({ "type": "overblow" })),
                        Pitch::Overdraw => modifiers.push(json!({ "type": "overdraw" })),
                        Pitch::Slide => modifiers.push(json!({ "type": "slide" })),
                        Pitch::Normal => {}
                    }
                    match n.expr {
                        Expr::Vibrato(hz) => modifiers.push(
                            json!({ "type": "vibrato", "oscillation_hz": hz, "intensity": 0.5 }),
                        ),
                        Expr::Wah(hz) => modifiers.push(
                            json!({ "type": "wah-wah", "oscillation_hz": hz, "intensity": 0.5 }),
                        ),
                        Expr::None => {}
                    }
                    let mut event = json!({
                        "hole": n.hole,
                        "action": action,
                        "note": note_name,
                    });
                    if !modifiers.is_empty() {
                        event["modifiers"] = Value::Array(modifiers);
                    }
                    event
                })
                .collect();

            json!({
                "id": format!("phrase_{:02}", idx + 1),
                "tick": tick,
                "duration": (duration_secs * 1000.0).round() / 1000.0,
                "play_mode": play_mode,
                "events": events,
            })
        })
        .collect();

    let title = if state.name.is_empty() {
        "Untitled"
    } else {
        &state.name
    };
    let artist = if state.author.is_empty() {
        "Unknown Artist"
    } else {
        &state.author
    };
    let last_phrase = track.len().saturating_sub(1);

    let harmonica = match state.harmonica_kind {
        HarmonicaKind::Diatonic => json!({
            "type": "diatonic",
            "holes": 10,
            "position": state.position,
            "bending_profile": "richter_standard",
            "layout": { "blow": C_BLOW, "draw": C_DRAW }
        }),
        HarmonicaKind::Chromatic => json!({
            "type": "chromatic",
            "holes": 12,
            "position": state.position,
            "layout": {
                "blow": C_BLOW_CHROMATIC,
                "draw": C_DRAW_CHROMATIC,
                "blow_slide": C_BLOW_SLIDE_CHROMATIC,
                "draw_slide": C_DRAW_SLIDE_CHROMATIC
            }
        }),
    };

    let chart = json!({
        "metadata": {
            "format_version": "1.0.0",
            "author": artist,
            "description": "Created with Harmonicon Song Editor 2",
            "audio_file": state.music.trim()
        },
        "song": {
            "title": title,
            "artist": artist,
            "tempo_bpm": bpm,
            "key": state.key,
            "time_signature": "4/4",
            "difficulty": "intermediate"
        },
        "timing": {
            "resolution": TICKS_PER_BEAT,
            "tempo_map": [{ "tick": 0, "bpm": bpm }]
        },
        "harmonica": harmonica,
        "track": track,
        "loop": {
            "type": "full",
            "repeat": false,
            "start_index": 0,
            "end_index": last_phrase
        },
        "scoring": {
            "perfect_window_ms": 60,
            "good_window_ms": 120,
            "miss_window_ms": 220,
            "combo": {
                "enabled": true,
                "base_multiplier": 1.0,
                "step_multiplier": 0.1,
                "max_multiplier": 4.0,
                "decay_ms": 2000
            },
            "style_bonus": { "bend": 50, "vibrato": 25, "wah-wah": 40 }
        }
    });

    serde_json::to_string_pretty(&chart).unwrap_or_default()
}

// ── Parsing ───────────────────────────────────────────────────────────────────

pub(super) fn parse_pitch_expr(modifiers: &[serde_json::Value]) -> (Pitch, Expr) {
    let mut pitch = Pitch::Normal;
    let mut expr = Expr::None;
    for m in modifiers {
        match m["type"].as_str().unwrap_or("") {
            "bend" => {
                let s = m["semitones"].as_f64().unwrap_or(0.0) as f32;
                pitch = Pitch::Bend(-s);
            }
            "overblow" => pitch = Pitch::Overblow,
            "overdraw" => pitch = Pitch::Overdraw,
            "slide" => pitch = Pitch::Slide,
            // Default to the old fixed rates for charts saved before
            // `oscillation_hz` was per-note; clamp away non-positive/absurd
            // values so a hand-edited chart can't divide-by-zero the preview
            // synth's phase integration.
            "vibrato" => {
                let hz = m["oscillation_hz"].as_f64().unwrap_or(5.5) as f32;
                expr = Expr::Vibrato(hz.max(0.5));
            }
            "wah-wah" => {
                let hz = m["oscillation_hz"].as_f64().unwrap_or(4.0) as f32;
                expr = Expr::Wah(hz.max(0.5));
            }
            _ => {}
        }
    }
    (pitch, expr)
}

pub(super) fn load_harpchart(v: &serde_json::Value, state: &mut EditorState, scroll: &mut Scroll) {
    if let Some(song) = v.get("song") {
        if let Some(t) = song["title"].as_str() {
            state.name = t.to_string();
        }
        if let Some(a) = song["artist"].as_str() {
            state.author = a.to_string();
        }
        if let Some(b) = song["tempo_bpm"].as_f64() {
            state.tempo = format!("{}", b.round() as u32);
        }
        if let Some(k) = song["key"].as_str()
            && HARP_KEYS.contains(&k)
        {
            state.key = k.to_string();
        }
    }
    if let Some(p) = v["harmonica"]["position"].as_str()
        && POSITIONS.contains(&p)
    {
        state.position = p.to_string();
    }
    // The editor only models a 10-hole diatonic or 12-hole chromatic harp
    // (`HarmonicaKind::hole_count`); a chart declaring a 16-hole chromatic
    // harp still loads as 12-hole chromatic, so any of its holes 13–16 are
    // dropped below rather than rejecting the whole chart.
    state.harmonica_kind = if v["harmonica"]["type"].as_str() == Some("chromatic") {
        HarmonicaKind::Chromatic
    } else {
        HarmonicaKind::Diatonic
    };
    if let Some(meta) = v.get("metadata")
        && let Some(audio) = meta["audio_file"].as_str()
        && !audio.is_empty()
    {
        state.music = audio.to_string();
    }

    let bpm: f32 = state.tempo.parse().unwrap_or(120.0);
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;

    let mut notes: Vec<GridNote> = Vec::new();
    let mut next_id = 0u32;
    let empty = vec![];
    let hole_count = state.hole_count();

    if let Some(track) = v["track"].as_array() {
        for phrase in track {
            let start_tick = if let Some(t) = phrase["tick"].as_u64() {
                t as usize
            } else if let Some(t) = phrase["time"].as_f64() {
                (t as f32 / secs_per_tick).round() as usize
            } else {
                continue;
            };

            let duration_secs = phrase["duration"]
                .as_f64()
                .unwrap_or(secs_per_tick as f64 * TICKS_PER_BEAT as f64)
                as f32;
            let len = ((duration_secs / secs_per_tick).round() as usize).max(1);

            let events = phrase["events"].as_array().unwrap_or(&empty);
            for event in events {
                let hole = event["hole"].as_u64().unwrap_or(1) as u8;
                if !(1..=hole_count).contains(&hole) {
                    continue;
                }
                let dir = if event["action"].as_str() == Some("draw") {
                    Dir::Draw
                } else {
                    Dir::Blow
                };
                let mods_empty = vec![];
                let mods = event["modifiers"].as_array().unwrap_or(&mods_empty);
                let (pitch, expr) = parse_pitch_expr(mods);
                notes.push(GridNote {
                    id: next_id,
                    hole,
                    tick: start_tick,
                    len,
                    dir,
                    pitch,
                    expr,
                });
                next_id += 1;
            }
        }
    }

    state.notes = notes;
    state.next_id = next_id;
    state.selected = None;
    state.dragging = None;
    state.focus = None;
    state.scroll_beat = 0;
    scroll.px = 0.0;
}

// ── Systems ───────────────────────────────────────────────────────────────────

pub(super) fn handle_load_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut state: ResMut<EditorState>,
    mut scroll: ResMut<Scroll>,
) {
    for ev in chosen.read() {
        if ev.purpose != LOAD_PURPOSE {
            continue;
        }
        let text = match std::fs::read_to_string(&ev.path) {
            Ok(t) => t,
            Err(e) => {
                println!("Load failed (read): {e}");
                continue;
            }
        };
        let v: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                println!("Load failed (parse): {e}");
                continue;
            }
        };
        load_harpchart(&v, &mut state, &mut scroll);
        println!("Loaded: {}", ev.path.display());
    }
}

pub(super) fn handle_music_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut state: ResMut<EditorState>,
) {
    for ev in chosen.read() {
        if ev.purpose != MUSIC_PURPOSE {
            continue;
        }
        state.music = ev.path.to_string_lossy().into_owned();
    }
}

pub(super) fn handle_save_chosen(mut chosen: MessageReader<FileChosen>, state: Res<EditorState>) {
    for ev in chosen.read() {
        if ev.purpose != SAVE_PURPOSE {
            continue;
        }
        let json = serialize_harpchart(&state);
        if let Some(parent) = ev.path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            println!("Save failed (mkdir): {e}");
            continue;
        }
        match std::fs::write(&ev.path, json.as_bytes()) {
            Ok(()) => println!("Saved: {}", ev.path.display()),
            Err(e) => println!("Save failed (write): {e}"),
        }
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

pub(super) fn safe_path_segment(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
