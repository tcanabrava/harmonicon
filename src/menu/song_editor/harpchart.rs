// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use crate::audio_system::midi::{midi_to_note, note_to_midi};
use crate::dialogs::file_dialog::FileChosen;
use super::{LOAD_PURPOSE, MUSIC_PURPOSE, SAVE_PURPOSE, TICKS_PER_BEAT};
use super::state::{Dir, EditorState, Expr, GridNote, Pitch, Scroll, HARP_KEYS};
use super::playback::{C_BLOW, C_DRAW, key_offset};

// ── Serialisation ────────────────────────────────────────────────────────────

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
                    let note_name = {
                        let idx = (n.hole as usize).saturating_sub(1);
                        let base = match n.dir {
                            Dir::Blow => C_BLOW.get(idx).copied().unwrap_or("C4"),
                            Dir::Draw => C_DRAW.get(idx).copied().unwrap_or("D4"),
                        };
                        let midi = note_to_midi(base).unwrap_or(60) + k_off;
                        midi_to_note(midi)
                    };
                    let mut modifiers: Vec<Value> = Vec::new();
                    match n.pitch {
                        Pitch::Bend(a) => {
                            modifiers.push(json!({ "type": "bend", "semitones": -(a as f64) }));
                        }
                        Pitch::Overblow => modifiers.push(json!({ "type": "overblow" })),
                        Pitch::Overdraw => modifiers.push(json!({ "type": "overdraw" })),
                        Pitch::Normal => {}
                    }
                    match n.expr {
                        Expr::Vibrato => modifiers.push(
                            json!({ "type": "vibrato", "oscillation_hz": 5.5, "intensity": 0.5 }),
                        ),
                        Expr::Wah => modifiers.push(
                            json!({ "type": "wah-wah", "oscillation_hz": 4.0, "intensity": 0.5 }),
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

    let title  = if state.name.is_empty()   { "Untitled"        } else { &state.name   };
    let artist = if state.author.is_empty() { "Unknown Artist"  } else { &state.author };
    let last_phrase = track.len().saturating_sub(1);

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
        "harmonica": {
            "type": "diatonic",
            "holes": 10,
            "position": "2nd",
            "bending_profile": "richter_standard",
            "layout": { "blow": C_BLOW, "draw": C_DRAW }
        },
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
        },
        "fx_mapping": {
            "bend": "pitch_bend",
            "vibrato": "pitch_lfo",
            "wah-wah": "filter_lfo"
        }
    });

    serde_json::to_string_pretty(&chart).unwrap_or_default()
}

// ── Parsing ───────────────────────────────────────────────────────────────────

pub(super) fn parse_pitch_expr(modifiers: &[serde_json::Value]) -> (Pitch, Expr) {
    let mut pitch = Pitch::Normal;
    let mut expr  = Expr::None;
    for m in modifiers {
        match m["type"].as_str().unwrap_or("") {
            "bend"     => {
                let s = m["semitones"].as_f64().unwrap_or(0.0) as f32;
                pitch = Pitch::Bend(-s);
            }
            "overblow" => pitch = Pitch::Overblow,
            "overdraw" => pitch = Pitch::Overdraw,
            "vibrato"  => expr  = Expr::Vibrato,
            "wah-wah"  => expr  = Expr::Wah,
            _ => {}
        }
    }
    (pitch, expr)
}

pub(super) fn load_harpchart(v: &serde_json::Value, state: &mut EditorState, scroll: &mut Scroll) {
    if let Some(song) = v.get("song") {
        if let Some(t) = song["title"].as_str()    { state.name   = t.to_string(); }
        if let Some(a) = song["artist"].as_str()   { state.author = a.to_string(); }
        if let Some(b) = song["tempo_bpm"].as_f64() {
            state.tempo = format!("{}", b.round() as u32);
        }
        if let Some(k) = song["key"].as_str() {
            if HARP_KEYS.contains(&k) { state.key = k.to_string(); }
        }
    }
    if let Some(meta) = v.get("metadata") {
        if let Some(audio) = meta["audio_file"].as_str() {
            if !audio.is_empty() { state.music = audio.to_string(); }
        }
    }

    let bpm: f32 = state.tempo.parse().unwrap_or(120.0);
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;

    let mut notes: Vec<GridNote> = Vec::new();
    let mut next_id = 0u32;
    let empty = vec![];

    if let Some(track) = v["track"].as_array() {
        for phrase in track {
            let start_tick = if let Some(t) = phrase["tick"].as_u64() {
                t as usize
            } else if let Some(t) = phrase["time"].as_f64() {
                (t as f32 / secs_per_tick).round() as usize
            } else {
                continue;
            };

            let duration_secs = phrase["duration"].as_f64().unwrap_or(
                secs_per_tick as f64 * TICKS_PER_BEAT as f64,
            ) as f32;
            let len = ((duration_secs / secs_per_tick).round() as usize).max(1);

            let events = phrase["events"].as_array().unwrap_or(&empty);
            for event in events {
                let hole = event["hole"].as_u64().unwrap_or(1) as u8;
                if !(1..=10).contains(&hole) { continue; }
                let dir = if event["action"].as_str() == Some("draw") {
                    Dir::Draw
                } else {
                    Dir::Blow
                };
                let mods_empty = vec![];
                let mods = event["modifiers"].as_array().unwrap_or(&mods_empty);
                let (pitch, expr) = parse_pitch_expr(mods);
                notes.push(GridNote { id: next_id, hole, tick: start_tick, len, dir, pitch, expr });
                next_id += 1;
            }
        }
    }

    state.notes      = notes;
    state.next_id    = next_id;
    state.selected   = None;
    state.dragging   = None;
    state.focus      = None;
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
        if ev.purpose != LOAD_PURPOSE { continue; }
        let text = match std::fs::read_to_string(&ev.path) {
            Ok(t) => t,
            Err(e) => { println!("Load failed (read): {e}"); continue; }
        };
        let v: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => { println!("Load failed (parse): {e}"); continue; }
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
        if ev.purpose != MUSIC_PURPOSE { continue; }
        state.music = ev.path.to_string_lossy().into_owned();
    }
}

pub(super) fn handle_save_chosen(
    mut chosen: MessageReader<FileChosen>,
    state: Res<EditorState>,
) {
    for ev in chosen.read() {
        if ev.purpose != SAVE_PURPOSE { continue; }
        let json = serialize_harpchart(&state);
        if let Some(parent) = ev.path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!("Save failed (mkdir): {e}");
                continue;
            }
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
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
