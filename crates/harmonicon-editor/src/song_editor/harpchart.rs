// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::save_feedback::SaveFeedback;
use super::state::{
    Dir, EditorState, Expr, GridNote, HARP_KEYS, HarmonicaKind, LoadedHarmonica, POSITIONS, Pitch,
    Scroll,
};
use super::{LOAD_PURPOSE, MUSIC_PURPOSE, SAVE_PURPOSE, TICKS_PER_BEAT};
use harmonicon_core::chart::{
    Action, CURRENT_FORMAT_VERSION, Scale, TempoPoint, seconds_to_tick, tick_to_seconds,
};
use harmonicon_core::harmonica::{Harmonica, hole_notes};
use harmonicon_core::midi::{midi_to_note, note_to_midi};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::file_dialog::FileChosen;

// ── Serialisation ────────────────────────────────────────────────────────────

/// Resolves `n`'s note name for the chart's `events[].note` field — the
/// actual sounded pitch (bend/overblow/overdraw/slide applied), not just
/// the natural blow/draw note. Shares its derivation with the editor's own
/// preview/practice synthesis (`playback::note_freq`) via
/// `harmonicon_core::harmonica`, so an exported chart always matches what the
/// editor actually plays; overblow/overdraw resolve via [`hole_notes`],
/// which knows overblow sits above the *draw* reed on holes 1/4/5/6.
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
    serialize_harpchart_notes(state, &state.notes)
}

/// Same as [`serialize_harpchart`], but serializes `notes` instead of
/// `state.notes` — everything else (key, tempo, harmonica, ...) still comes
/// from the shared `state`. Used by `debug_record`'s benchmark-authoring
/// workflow to write `expected.harpchart` from the second, hand-annotated
/// [`EditorState::expected_notes`] vector without duplicating this whole
/// function.
pub(super) fn serialize_harpchart_notes(state: &EditorState, notes: &[GridNote]) -> String {
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    let bpm: f32 = state.tempo.parse().unwrap_or(120.0);
    let tempo_map = state.tempo_map();
    let harp = state.effective_harp();

    // A chart phrase has one duration shared by all of its events. Grouping
    // solely by onset would therefore lengthen every shorter chord tone to
    // the longest member on save. Keep equal-duration simultaneous notes as
    // a chord, and emit a separate same-tick phrase for each other duration.
    let mut by_tick_and_len: BTreeMap<(usize, usize), Vec<&GridNote>> = BTreeMap::new();
    for n in notes {
        by_tick_and_len.entry((n.tick, n.len)).or_default().push(n);
    }

    // Notes of different lengths starting together become separate items;
    // a lyric is sung once, so only the first of them carries it.
    let mut sung_at = std::collections::HashSet::new();
    let track: Vec<Value> = by_tick_and_len
        .iter()
        .enumerate()
        .map(|(idx, (&(tick, len), notes))| {
            // Via the real tempo map, not a flat bpm — correct even for the
            // rare phrase whose sustain crosses a tempo-change boundary.
            let start_secs = tick_to_seconds(tick as u64, TICKS_PER_BEAT as u32, &tempo_map);
            let end_secs = tick_to_seconds((tick + len) as u64, TICKS_PER_BEAT as u32, &tempo_map);
            let duration_secs = end_secs - start_secs;
            let annotation = state.phrase_annotations.get(&tick);
            let play_mode = if annotation.is_some_and(|annotation| annotation.split) {
                "split"
            } else if notes.len() == 1 {
                "single"
            } else {
                "chord"
            };

            let events: Vec<Value> = notes
                .iter()
                .map(|n| {
                    let intensity = state
                        .expression_intensities
                        .get(&n.id)
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.5);
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
                            json!({ "type": "vibrato", "oscillation_hz": hz, "intensity": intensity }),
                        ),
                        Expr::Wah(hz) => modifiers.push(
                            json!({ "type": "wah-wah", "oscillation_hz": hz, "intensity": intensity }),
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

            let mut phrase = json!({
                "id": format!("phrase_{:02}", idx + 1),
                "tick": tick,
                "duration": (duration_secs * 1000.0).round() / 1000.0,
                "play_mode": play_mode,
                "events": events,
            });
            if let Some(annotation) = annotation {
                if let Some(section) = &annotation.section {
                    phrase["phrase"] = json!(section);
                }
                if let Some(chord) = &annotation.chord {
                    phrase["chord"] = json!(chord);
                }
                if let Some(groove) = &annotation.groove {
                    phrase["groove"] = json!(groove);
                }
                if let Some(lyric) = &annotation.lyric
                    && sung_at.insert(tick)
                {
                    phrase["lyric"] = json!(lyric);
                }
                if annotation.call {
                    phrase["call"] = json!(true);
                }
            }
            phrase
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
        HarmonicaKind::Diatonic
        | HarmonicaKind::CountryTuned
        | HarmonicaKind::PaddyRichter
        | HarmonicaKind::NaturalMinor => {
            let (blow, draw) = match &harp {
                Harmonica::Diatonic {
                    layout: Some(l), ..
                } => (
                    l.blow.as_deref().unwrap_or(&[]),
                    l.draw.as_deref().unwrap_or(&[]),
                ),
                _ => (&[][..], &[][..]),
            };
            let bending_profile = match state.harmonica_kind {
                HarmonicaKind::Diatonic => "richter_standard",
                HarmonicaKind::PaddyRichter => "paddy_richter",
                HarmonicaKind::CountryTuned => "country_tuned",
                HarmonicaKind::NaturalMinor => "natural_minor",
                _ => unreachable!("matched diatonic variants above"),
            };
            json!({
                "type": "diatonic",
                "holes": 10,
                "position": state.position,
                "scale": state.scale,
                "bending_profile": bending_profile,
                "layout": { "blow": blow, "draw": draw }
            })
        }
        HarmonicaKind::Chromatic | HarmonicaKind::Chromatic16 => {
            let (blow, draw, blow_slide, draw_slide) = match &harp {
                Harmonica::Chromatic {
                    layout: Some(l), ..
                } => (
                    l.blow.as_deref().unwrap_or(&[]),
                    l.draw.as_deref().unwrap_or(&[]),
                    l.blow_slide.as_deref().unwrap_or(&[]),
                    l.draw_slide.as_deref().unwrap_or(&[]),
                ),
                _ => (&[][..], &[][..], &[][..], &[][..]),
            };
            json!({
                "type": "chromatic",
                "holes": state.hole_count(),
                "position": state.position,
                "scale": state.scale,
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
    let chart_author = if state.chart_author.is_empty() {
        artist
    } else {
        state.chart_author.as_str()
    };
    let mut metadata = json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "author": chart_author,
        "description": state.description
    });
    if !state.source.trim().is_empty() {
        metadata["source"] = json!(state.source.trim());
    }
    if !state.license.trim().is_empty() {
        metadata["license"] = json!(state.license.trim());
    }
    let audio_file = state.music.trim();
    if !audio_file.is_empty() {
        metadata["audio_file"] = json!(audio_file);
    }

    let mut song = json!({
        "title": title,
        "artist": artist,
        "genre": state.genre.trim(),
        "tempo_bpm": bpm,
        "key": state.key,
        "time_signature": state.time_signature,
        "difficulty": state.difficulty
    });
    if state.song_feel != "default" {
        song["feel"] = json!(state.song_feel);
    }
    let loop_start = state
        .loop_settings
        .start
        .parse::<usize>()
        .unwrap_or(0)
        .min(last_phrase);
    let loop_end = state
        .loop_settings
        .end
        .parse::<usize>()
        .unwrap_or(last_phrase)
        .clamp(loop_start, last_phrase);
    let loop_settings = json!({
        "type": state.loop_settings.kind,
        "repeat": state.loop_settings.repeat == "yes",
        "start_index": loop_start,
        "end_index": loop_end
    });
    let mut scoring = state.preserved_scoring.clone().unwrap_or_else(|| {
        json!({
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
        })
    });
    let positive_ms = |value: &str, fallback: u64| {
        value
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(fallback)
    };
    scoring["perfect_window_ms"] = json!(positive_ms(&state.perfect_window_ms, 60));
    scoring["good_window_ms"] = json!(positive_ms(&state.good_window_ms, 120));
    scoring["miss_window_ms"] = json!(positive_ms(&state.miss_window_ms, 220));
    let bounded = |value: &str, fallback: f64, minimum: f64| {
        value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value > minimum)
            .unwrap_or(fallback)
    };
    scoring["combo"] = json!({
        "enabled": state.combo.enabled != "disabled",
        "base_multiplier": bounded(&state.combo.base, 1.0, 1.0 - f64::EPSILON),
        "step_multiplier": bounded(&state.combo.step, 0.1, 0.0),
        "max_multiplier": bounded(&state.combo.max, 4.0, 1.0),
        "decay_ms": positive_ms(&state.combo.decay_ms, 2000)
    });

    let mut timing = json!({
        "resolution": TICKS_PER_BEAT,
        "tempo_map": tempo_map
            .iter()
            .map(|p| json!({ "tick": p.tick, "bpm": p.bpm }))
            .collect::<Vec<_>>()
    });
    if !state.meter_changes.is_empty() {
        timing["time_signature_map"] = json!(state.time_signature_map());
    }
    let pickup = state.pickup_ticks();
    if pickup > 0 {
        timing["pickup_ticks"] = json!(pickup);
    }
    if !state.repeats.is_empty() {
        timing["repeats"] = json!(state.repeats);
    }
    let chart = json!({
        "metadata": metadata,
        "song": song,
        "timing": timing,
        "harmonica": harmonica,
        "track": track,
        "loop": loop_settings,
        "scoring": scoring
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

/// A pickup of `editor_ticks` as the Details field shows it: beats of
/// `meter`, whole when it is whole ("1") and otherwise to two places
/// ("0.5"). Empty for no pickup.
pub(super) fn pickup_beats_text(
    editor_ticks: f64,
    meter: harmonicon_ui::music_score::MusicScoreMeter,
) -> String {
    let beat_ticks = meter
        .ticks_per_beat(TICKS_PER_BEAT as u32)
        .unwrap_or(TICKS_PER_BEAT as u32);
    let beats = editor_ticks.round() / f64::from(beat_ticks);
    if beats <= 0.0 {
        String::new()
    } else if beats.fract() == 0.0 {
        format!("{beats:.0}")
    } else {
        format!("{beats:.2}").trim_end_matches('0').to_string()
    }
}

pub(super) fn load_harpchart(v: &serde_json::Value, state: &mut EditorState, scroll: &mut Scroll) {
    state.preserved_scoring = v.get("scoring").cloned();
    if let Some(loop_settings) = v.get("loop") {
        state.loop_settings.kind = loop_settings["type"].as_str().unwrap_or("full").to_string();
        state.loop_settings.repeat = if loop_settings["repeat"].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        }
        .into();
        state.loop_settings.start = loop_settings["start_index"]
            .as_u64()
            .unwrap_or(0)
            .to_string();
        state.loop_settings.end = loop_settings["end_index"]
            .as_u64()
            .map_or_else(|| "last".into(), |value| value.to_string());
    }
    if let Some(scoring) = v.get("scoring") {
        state.perfect_window_ms = scoring["perfect_window_ms"]
            .as_u64()
            .unwrap_or(60)
            .to_string();
        state.good_window_ms = scoring["good_window_ms"]
            .as_u64()
            .unwrap_or(120)
            .to_string();
        state.miss_window_ms = scoring["miss_window_ms"]
            .as_u64()
            .unwrap_or(220)
            .to_string();
        let combo = &scoring["combo"];
        state.combo.enabled = if combo["enabled"].as_bool().unwrap_or(true) {
            "enabled"
        } else {
            "disabled"
        }
        .into();
        state.combo.base = combo["base_multiplier"].as_f64().unwrap_or(1.0).to_string();
        state.combo.step = combo["step_multiplier"].as_f64().unwrap_or(0.1).to_string();
        state.combo.max = combo["max_multiplier"].as_f64().unwrap_or(4.0).to_string();
        state.combo.decay_ms = combo["decay_ms"].as_u64().unwrap_or(2000).to_string();
    }
    if let Some(metadata) = v.get("metadata") {
        state.chart_author = metadata["author"].as_str().unwrap_or("").to_string();
        state.source = metadata["source"].as_str().unwrap_or("").to_string();
        state.license = metadata["license"].as_str().unwrap_or("").to_string();
        state.description = metadata["description"]
            .as_str()
            .unwrap_or("Created with Harmonicon Song Editor 2")
            .to_string();
    }
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
        // Round-trips whatever the chart declares. Not validated here: an
        // unparseable value falls back to 4/4 wherever it's *read*
        // (`EditorState::beats_per_bar`), so loading a chart with an odd
        // signature shows the author what it actually says rather than
        // silently rewriting it to 4/4 on the next save.
        if let Some(ts) = song["time_signature"].as_str() {
            state.time_signature = ts.to_string();
        }
        if let Some(difficulty) = song["difficulty"].as_str() {
            state.difficulty = difficulty.to_string();
        }
        state.genre = song["genre"]
            .as_str()
            .unwrap_or("Uncategorized")
            .to_string();
        state.song_feel = song["feel"].as_str().unwrap_or("default").to_string();
    }
    if let Some(p) = v["harmonica"]["position"].as_str()
        && POSITIONS.contains(&p)
    {
        state.position = p.to_string();
    }
    if let Ok(scale) = serde_json::from_value::<Scale>(v["harmonica"]["scale"].clone()) {
        state.scale = scale;
    }
    state.harmonica_kind = match (
        v["harmonica"]["type"].as_str(),
        v["harmonica"]["holes"].as_u64(),
    ) {
        (Some("chromatic"), Some(16)) => HarmonicaKind::Chromatic16,
        (Some("chromatic"), _) => HarmonicaKind::Chromatic,
        _ => match v["harmonica"]["bending_profile"].as_str() {
            Some("paddy_richter") => HarmonicaKind::PaddyRichter,
            Some("country_tuned") => HarmonicaKind::CountryTuned,
            Some("natural_minor") => HarmonicaKind::NaturalMinor,
            _ => HarmonicaKind::Diatonic,
        },
    };
    state.loaded_harmonica = serde_json::from_value::<Harmonica>(v["harmonica"].clone())
        .ok()
        .map(|harp| LoadedHarmonica {
            key: state.key.clone(),
            kind: state.harmonica_kind,
            harp,
        });
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
    state.meter_changes.clear();
    if let Some(points) = v["timing"]["time_signature_map"].as_array() {
        for point in points {
            let Some(file_tick) = point["tick"].as_u64() else {
                continue;
            };
            let Some(signature) = point["time_signature"].as_str() else {
                continue;
            };
            let tick = (file_tick as f64 * scale).round() as usize;
            if tick == 0 {
                state.time_signature = signature.to_string();
            } else {
                state.meter_changes.push((tick, signature.to_string()));
            }
        }
        state.meter_changes.sort_by_key(|(tick, _)| *tick);
    }
    // After the opening meter, which the pickup is counted in.
    state.pickup_beats = v["timing"]["pickup_ticks"]
        .as_u64()
        .map(|file_ticks| pickup_beats_text(file_ticks as f64 * scale, state.meter()))
        .unwrap_or_default();
    // Validated with the rest of the chart, so a malformed entry is a load
    // error before here, not a silently dropped sign.
    let rescale = |file_tick: u64| (file_tick as f64 * scale).round() as u64;
    state.repeats = serde_json::from_value::<Vec<harmonicon_core::chart::Repeat>>(
        v["timing"]["repeats"].clone(),
    )
    .unwrap_or_default()
    .into_iter()
    .map(|mut repeat| {
        repeat.start_tick = rescale(repeat.start_tick);
        repeat.end_tick = rescale(repeat.end_tick);
        for ending in &mut repeat.endings {
            ending.start_tick = rescale(ending.start_tick);
            ending.end_tick = rescale(ending.end_tick);
        }
        repeat
    })
    .collect();
    let editor_tempo_map = state.tempo_map();

    let mut notes: Vec<GridNote> = Vec::new();
    state.phrase_annotations.clear();
    state.expression_intensities.clear();
    let mut next_id = 0u32;
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

            let section = phrase["phrase"].as_str().map(str::to_owned);
            let chord = phrase["chord"].as_str().map(str::to_owned);
            let groove = phrase["groove"].as_str().map(str::to_owned);
            let lyric = phrase["lyric"].as_str().map(str::to_owned);
            let call = phrase["call"].as_bool() == Some(true);
            let split = phrase["play_mode"].as_str() == Some("split");
            if section.is_some()
                || chord.is_some()
                || groove.is_some()
                || lyric.is_some()
                || call
                || split
            {
                let annotation = state.phrase_annotations.entry(start_tick).or_default();
                if section.is_some() {
                    annotation.section = section;
                }
                if chord.is_some() {
                    annotation.chord = chord;
                }
                if groove.is_some() {
                    annotation.groove = groove;
                }
                if lyric.is_some() {
                    annotation.lyric = lyric;
                }
                annotation.call |= call;
                annotation.split |= split;
            }

            let start_secs =
                tick_to_seconds(start_tick as u64, TICKS_PER_BEAT as u32, &editor_tempo_map);
            let default_beat_secs = tick_to_seconds(
                (start_tick + TICKS_PER_BEAT) as u64,
                TICKS_PER_BEAT as u32,
                &editor_tempo_map,
            ) - start_secs;
            let duration_secs = phrase["duration"].as_f64().unwrap_or(default_beat_secs);
            let end_tick = seconds_to_tick(
                start_secs + duration_secs,
                TICKS_PER_BEAT as u32,
                &editor_tempo_map,
            );
            let len = (end_tick as usize).saturating_sub(start_tick).max(1);

            let events = phrase["events"].as_array().map_or(&[][..], Vec::as_slice);
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
                let mods = event["modifiers"].as_array().map_or(&[][..], Vec::as_slice);
                let (pitch, expr) = parse_pitch_expr(mods);
                if let Some(intensity) = mods.iter().find_map(|modifier| {
                    matches!(modifier["type"].as_str(), Some("vibrato" | "wah-wah"))
                        .then(|| modifier["intensity"].as_f64())
                        .flatten()
                }) && intensity != 0.5
                {
                    state
                        .expression_intensities
                        .insert(next_id, intensity.to_string());
                }
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
    state.selected.clear();
    state.dragging = None;
    state.scroll_beat = 0;
    scroll.px = 0.0;
}

/// Parses, migrates, and validates a chart before editor state is touched.
/// A failed load therefore leaves the current document unchanged.
pub(super) fn validated_harpchart(text: &str) -> Result<serde_json::Value, String> {
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    harmonicon_song::song::validate_and_migrate_chart(&mut value).map_err(|e| e.to_string())?;
    let unsupported = unsupported_chart_features(&value);
    if !unsupported.is_empty() {
        return Err(format!(
            "this chart uses features the Song Editor cannot preserve:\n  - {}",
            unsupported.join("\n  - ")
        ));
    }
    Ok(value)
}

/// Features accepted by gameplay but not faithfully represented by the
/// editor's current grid model. Refusing these charts is safer than opening
/// them successfully and silently erasing their meaning on the next save.
pub(super) fn unsupported_chart_features(value: &serde_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(track) = value["track"].as_array() {
        for (index, phrase) in track.iter().enumerate() {
            let number = index + 1;

            if let Some(events) = phrase["events"].as_array() {
                for (event_index, event) in events.iter().enumerate() {
                    let Some(modifiers) = event["modifiers"].as_array() else {
                        continue;
                    };
                    let pitch_count = modifiers
                        .iter()
                        .filter(|modifier| {
                            matches!(
                                modifier["type"].as_str(),
                                Some("bend" | "overblow" | "overdraw" | "slide")
                            )
                        })
                        .count();
                    let expression_count = modifiers
                        .iter()
                        .filter(|modifier| {
                            matches!(modifier["type"].as_str(), Some("vibrato" | "wah-wah"))
                        })
                        .count();
                    if pitch_count > 1 {
                        found.push(format!(
                            "phrase {number}, event {} combines multiple mutually exclusive pitch techniques",
                            event_index + 1
                        ));
                    }
                    if expression_count > 1 {
                        found.push(format!(
                            "phrase {number}, event {} combines multiple mutually exclusive expressions",
                            event_index + 1
                        ));
                    }
                }
            }
        }
    }
    found
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// The `ContentKind::Song` half of loading — its `ContentKind::Lesson`
/// sibling is `lesson_form::handle_load_lesson_chosen`; each skips the
/// other's `ContentKind`, so exactly one acts on a given `FileChosen {
/// purpose: LOAD_PURPOSE }` message.
pub(super) fn handle_load_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut state: ResMut<EditorState>,
    mut scroll: ResMut<Scroll>,
    mut feedback: ResMut<SaveFeedback>,
    loc: Res<Localization>,
) {
    use super::state::ContentKind;
    for ev in chosen.read() {
        if ev.purpose != LOAD_PURPOSE || state.content_kind != ContentKind::Song {
            continue;
        }
        let text = match std::fs::read_to_string(&ev.path) {
            Ok(t) => t,
            Err(e) => {
                warn!("Song editor: load failed (read {}): {e}", ev.path.display());
                feedback.set(loc.msg_args("editor-load-failed", &[("detail", e.to_string())]));
                continue;
            }
        };
        let v = match validated_harpchart(&text) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Song editor: load failed (validation {}): {e}",
                    ev.path.display()
                );
                feedback.set(loc.msg_args("editor-load-failed", &[("detail", e.to_string())]));
                continue;
            }
        };
        load_harpchart(&v, &mut state, &mut scroll);
        info!("Song editor: loaded {}", ev.path.display());
        feedback.set(loc.msg_args(
            "editor-load-success",
            &[("path", ev.path.display().to_string())],
        ));
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

/// The `ContentKind::Song` half of saving — its `ContentKind::Lesson`
/// sibling is `lesson_form::handle_save_lesson_chosen`; each skips the
/// other's `ContentKind`. MIDI backing generation (`save_midi_backing`) is
/// a `ContentKind::Song`-only convenience — see `lesson_form`'s module doc
/// for why a lesson save skips it.
pub(super) fn handle_save_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut state: ResMut<EditorState>,
    midi: Option<Res<super::midi_import::MidiImport>>,
    mut feedback: ResMut<SaveFeedback>,
    loc: Res<Localization>,
) {
    use super::state::ContentKind;
    for ev in chosen.read() {
        if ev.purpose != SAVE_PURPOSE || state.content_kind != ContentKind::Song {
            continue;
        }
        if let Some(parent) = ev.path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            warn!("Song editor: save failed (mkdir {}): {e}", parent.display());
            feedback.set(loc.msg_args("editor-save-failed", &[("detail", e.to_string())]));
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
            Ok(()) => {
                info!("Song editor: saved {}", ev.path.display());
                feedback.set(loc.msg_args(
                    "editor-save-success",
                    &[("path", ev.path.display().to_string())],
                ));
            }
            Err(e) => {
                warn!(
                    "Song editor: save failed (write {}): {e}",
                    ev.path.display()
                );
                feedback.set(loc.msg_args("editor-save-failed", &[("detail", e.to_string())]));
            }
        }
    }
}

/// Writes the two extra files a MIDI-backed save produces alongside the
/// chart: a "processed" copy of the original MIDI with the imported track
/// removed, and a synthesized WAV mixdown of every *other* track as the
/// song's backing audio — the engine can't play a raw `.mid` file (see
/// `song::loader`'s `song/music.wav` fallback). Sets `EditorState::music`
/// to the new WAV's path on success, so the save right after this records
/// it and the editor's own Play preview picks it up.
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
                Ok(()) => info!(
                    "Song editor: wrote {} \u{2014} a copy of {} with the imported track \
                     removed; the original is untouched.",
                    out.display(),
                    midi.path.display()
                ),
                Err(e) => warn!("Song editor: save failed (processed MIDI): {e}"),
            }
        }
        Err(e) => warn!("Song editor: save failed (processed MIDI): {e}"),
    }

    match super::midi_import::render_backing_pcm(&midi.bytes, track_index) {
        Ok((_bpm, pcm)) => {
            let wav = harmonicon_core::wav::encode_wav(&pcm, harmonicon_core::synth::SAMPLE_RATE);
            let out = dir.join("music.wav");
            match std::fs::write(&out, &wav) {
                Ok(()) => {
                    info!(
                        "Song editor: wrote {} \u{2014} a synthesized backing track from the \
                         MIDI file's other tracks.",
                        out.display()
                    );
                    state.music = out.to_string_lossy().into_owned();
                }
                Err(e) => warn!("Song editor: save failed (backing track): {e}"),
            }
        }
        Err(e) => warn!("Song editor: no backing track written: {e}"),
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

pub(super) fn safe_path_segment(s: &str) -> String {
    let mut segment = String::new();
    let mut separator = false;
    for c in s.trim().chars() {
        if c.is_alphanumeric() || c == '-' {
            if separator && !segment.is_empty() {
                segment.push('_');
            }
            segment.push(c);
            separator = false;
        } else {
            separator = true;
        }
    }
    segment
}
