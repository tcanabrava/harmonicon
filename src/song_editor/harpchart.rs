// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::playback::build_harp;
use super::state::{
    Dir, EditorState, Expr, GridNote, HARP_KEYS, HarmonicaKind, POSITIONS, Pitch, Scroll,
};
use super::{LOAD_PURPOSE, MUSIC_PURPOSE, SAVE_PURPOSE, TICKS_PER_BEAT};
use crate::audio_system::midi::{midi_to_note, note_to_midi};
use crate::dialogs::file_dialog::FileChosen;
use crate::song::chart::{Action, TempoPoint, seconds_to_tick, tick_to_seconds};
use crate::song::harmonica::{Harmonica, hole_notes};

// ── Serialisation ────────────────────────────────────────────────────────────

/// Resolves `n`'s note name for the chart's `events[].note` field — the
/// actual sounded pitch (bend/overblow/overdraw/slide already applied), not
/// just the natural blow/draw note. Shares its derivation with the editor's
/// own preview/practice synthesis (`playback::note_freq`) via
/// `crate::song::harmonica`, so an exported chart's note always matches what
/// the editor actually plays for it — in particular, overblow/overdraw are
/// resolved via [`hole_notes`] rather than a flat "+1 semitone from whichever
/// direction the note is tagged with", which got the wrong reed for holes
/// 1/4/5/6 (overblow sits above the *draw* reed, not the blow reed).
fn note_name_for(n: &GridNote, harp: &Harmonica) -> String {
    let action = match n.dir {
        Dir::Blow => Action::Blow,
        Dir::Draw => Action::Draw,
    };
    let label = match n.pitch {
        Pitch::Normal => harp.wind_direction_label(n.hole, &action),
        Pitch::Slide => harp.slide_label(n.hole, &action),
        Pitch::Overblow | Pitch::Overdraw => hole_notes(harp, n.hole)
            .over
            .unwrap_or_else(|| "C4".to_string()),
        Pitch::Bend(a) => {
            let base = harp.wind_direction_label(n.hole, &action);
            let midi = note_to_midi(&base).unwrap_or(60);
            return midi_to_note((midi as f32 - a).round() as i32);
        }
    };
    if label == "\u{2014}" {
        "C4".to_string()
    } else {
        label
    }
}

pub(super) fn serialize_harpchart(state: &EditorState) -> String {
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    let bpm: f32 = state.tempo.parse().unwrap_or(120.0);
    let tempo_map = state.tempo_map();
    let harp = build_harp(&state.key, state.harmonica_kind);

    let mut by_tick: BTreeMap<usize, Vec<&GridNote>> = BTreeMap::new();
    for n in &state.notes {
        by_tick.entry(n.tick).or_default().push(n);
    }

    let track: Vec<Value> = by_tick
        .iter()
        .enumerate()
        .map(|(idx, (&tick, notes))| {
            let max_len = notes.iter().map(|n| n.len).max().unwrap_or(1);
            // Via the real tempo map, not a flat bpm — correct even for the
            // rare phrase whose sustain crosses a tempo-change boundary.
            let start_secs = tick_to_seconds(tick as u64, TICKS_PER_BEAT as u32, &tempo_map);
            let end_secs =
                tick_to_seconds((tick + max_len) as u64, TICKS_PER_BEAT as u32, &tempo_map);
            let duration_secs = end_secs - start_secs;
            let play_mode = if notes.len() == 1 { "single" } else { "chord" };

            let events: Vec<Value> = notes
                .iter()
                .map(|n| {
                    let action = match n.dir {
                        Dir::Blow => "blow",
                        Dir::Draw => "draw",
                    };
                    let note_name = note_name_for(n, &harp);
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

    // The harp's own layout, transposed to `state.key` like every note in
    // `track` above — a 2nd-position G-key song still calls out a C harp
    // here (the physical instrument to grab), but a straight/1st-position
    // song's layout now actually matches its key instead of always reading
    // as a plain, untransposed C harp.
    let harmonica = match state.harmonica_kind {
        HarmonicaKind::Diatonic => {
            let (blow, draw) = match &harp {
                Harmonica::Diatonic {
                    layout: Some(l), ..
                } => (
                    l.blow.clone().unwrap_or_default(),
                    l.draw.clone().unwrap_or_default(),
                ),
                _ => (Vec::new(), Vec::new()),
            };
            json!({
                "type": "diatonic",
                "holes": 10,
                "position": state.position,
                "bending_profile": "richter_standard",
                "layout": { "blow": blow, "draw": draw }
            })
        }
        HarmonicaKind::Chromatic => {
            let (blow, draw, blow_slide, draw_slide) = match &harp {
                Harmonica::Chromatic {
                    layout: Some(l), ..
                } => (
                    l.blow.clone().unwrap_or_default(),
                    l.draw.clone().unwrap_or_default(),
                    l.blow_slide.clone().unwrap_or_default(),
                    l.draw_slide.clone().unwrap_or_default(),
                ),
                _ => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            };
            json!({
                "type": "chromatic",
                "holes": 12,
                "position": state.position,
                "layout": {
                    "blow": blow,
                    "draw": draw,
                    "blow_slide": blow_slide,
                    "draw_slide": draw_slide
                }
            })
        }
    };

    // `audio_file` is optional in the schema and purely a Song Editor
    // round-trip convenience (gameplay always loads `song/*.ogg` by
    // convention, never this field) — omit it entirely rather than writing
    // an empty string when no audio file has been picked yet.
    let mut metadata = json!({
        "format_version": "1.0.0",
        "author": artist,
        "description": "Created with Harmonicon Song Editor 2"
    });
    let audio_file = state.music.trim();
    if !audio_file.is_empty() {
        metadata["audio_file"] = json!(audio_file);
    }

    let chart = json!({
        "metadata": metadata,
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
            "tempo_map": tempo_map
                .iter()
                .map(|p| json!({ "tick": p.tick, "bpm": p.bpm }))
                .collect::<Vec<_>>()
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

    // The file's own resolution/tempo map — independent of `state.tempo`/
    // `tempo_changes` below, which get *populated from* this data, not read
    // by it. A chart missing (or declaring an empty) `timing.tempo_map`
    // falls back to a single tick-0 point at `song.tempo_bpm` (already in
    // `state.tempo` from above) — the same "always resolves to something
    // reasonable" fallback the rest of the editor's load path already uses.
    let file_resolution = v["timing"]["resolution"]
        .as_u64()
        .map(|r| r as u32)
        .filter(|&r| r > 0)
        .unwrap_or(TICKS_PER_BEAT as u32);
    let file_tempo_map: Vec<TempoPoint> = v["timing"]["tempo_map"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some(TempoPoint {
                        tick: p["tick"].as_u64()?,
                        bpm: p["bpm"].as_f64()? as f32,
                    })
                })
                .collect::<Vec<_>>()
        })
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| {
            vec![TempoPoint {
                tick: 0,
                bpm: state.tempo.parse::<f32>().unwrap_or(120.0).max(1.0),
            }]
        });

    // Editor ticks and file ticks are both "N per quarter note" grids
    // sharing the same real-time axis — rescaling between them is a
    // constant ratio, independent of tempo entirely (tempo affects
    // tick-to-*seconds*, not tick-to-tick). `tempo_map`'s own tempo values
    // carry over unchanged; only their tick anchors get rescaled.
    let scale = TICKS_PER_BEAT as f64 / file_resolution as f64;
    state.tempo = format!("{}", file_tempo_map[0].bpm.round() as u32);
    state.tempo_changes = file_tempo_map[1..]
        .iter()
        .map(|p| ((p.tick as f64 * scale).round() as usize, p.bpm))
        .collect();
    let editor_tempo_map = state.tempo_map();

    let mut notes: Vec<GridNote> = Vec::new();
    let mut next_id = 0u32;
    let empty = vec![];
    let hole_count = state.hole_count();

    if let Some(track) = v["track"].as_array() {
        for phrase in track {
            let start_tick = if let Some(t) = phrase["tick"].as_u64() {
                (t as f64 * scale).round() as usize
            } else if let Some(t) = phrase["time"].as_f64() {
                seconds_to_tick(t, TICKS_PER_BEAT as u32, &editor_tempo_map) as usize
            } else {
                continue;
            };

            let start_secs =
                tick_to_seconds(start_tick as u64, TICKS_PER_BEAT as u32, &editor_tempo_map);
            let default_beat_secs = tick_to_seconds(
                (start_tick + TICKS_PER_BEAT) as u64,
                TICKS_PER_BEAT as u32,
                &editor_tempo_map,
            ) - start_secs;
            let duration_secs = phrase["duration"].as_f64().unwrap_or(default_beat_secs);
            let end_tick =
                seconds_to_tick(start_secs + duration_secs, TICKS_PER_BEAT as u32, &editor_tempo_map);
            let len = (end_tick as usize).saturating_sub(start_tick).max(1);

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

pub(super) fn handle_save_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut state: ResMut<EditorState>,
    midi: Option<Res<super::midi_import::MidiImport>>,
) {
    for ev in chosen.read() {
        if ev.purpose != SAVE_PURPOSE {
            continue;
        }
        if let Some(parent) = ev.path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            println!("Save failed (mkdir): {e}");
            continue;
        }

        // If a MIDI track is currently imported, write its backing audio
        // and processed copy *before* serializing the chart, so this same
        // save records the freshly-written backing track in
        // `metadata.audio_file` rather than whatever `state.music` held
        // before (see `save_midi_backing`).
        if let (Some(midi), Some(parent)) = (midi.as_deref(), ev.path.parent())
            && let Some(track_index) = midi.selected
        {
            save_midi_backing(parent, midi, track_index, &mut state);
        }

        let json = serialize_harpchart(&state);
        match std::fs::write(&ev.path, json.as_bytes()) {
            Ok(()) => println!("Saved: {}", ev.path.display()),
            Err(e) => println!("Save failed (write): {e}"),
        }
    }
}

/// Writes the two extra files a MIDI-backed save produces alongside the
/// chart itself: a copy of the original MIDI with the imported track
/// removed (the same "processed" copy `bin/midi_to_chart` writes — the
/// original the user picked is never modified), and a synthesized WAV
/// mixdown of every *other* track as the song's backing audio. The engine
/// can't play a raw `.mid` file, so this is what "use the MIDI file as the
/// background song" resolves to — see `song::loader`'s `song/music.wav`
/// fallback. Sets `EditorState::music` to the new WAV's path on success, so
/// the chart being saved right after this records it, and the editor's own
/// Play preview picks it up too.
fn save_midi_backing(
    dir: &std::path::Path,
    midi: &super::midi_import::MidiImport,
    track_index: usize,
    state: &mut EditorState,
) {
    match super::midi_import::remove_track_bytes(&midi.bytes, track_index) {
        Ok(bytes) => {
            let stem = midi
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("song");
            let out = dir.join(format!("{stem}_processed.mid"));
            match std::fs::write(&out, &bytes) {
                Ok(()) => println!(
                    "Wrote {} \u{2014} a copy of {} with the imported track removed; \
                     the original is untouched.",
                    out.display(),
                    midi.path.display()
                ),
                Err(e) => println!("Save failed (processed MIDI): {e}"),
            }
        }
        Err(e) => println!("Save failed (processed MIDI): {e}"),
    }

    match super::midi_import::render_backing_pcm(&midi.bytes, track_index) {
        Ok((_bpm, pcm)) => {
            let wav = crate::audio_system::wav::encode_wav(
                &pcm,
                crate::audio_system::synth::SAMPLE_RATE,
            );
            let out = dir.join("music.wav");
            match std::fs::write(&out, &wav) {
                Ok(()) => {
                    println!(
                        "Wrote {} \u{2014} a synthesized backing track from the MIDI file's \
                         other tracks.",
                        out.display()
                    );
                    state.music = out.to_string_lossy().into_owned();
                }
                Err(e) => println!("Save failed (backing track): {e}"),
            }
        }
        Err(e) => println!("No backing track written: {e}"),
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
