// SPDX-License-Identifier: MIT

//! Import a MIDI file's track into the note grid, generalized to whatever
//! harmonica key the editor session is currently set to (not a fixed C
//! diatonic), producing [`GridNote`]s in memory rather than writing a chart
//! file, since here the destination is [`EditorState`], not disk. The *key*
//! isn't actually fixed to whatever was already selected, though: picking
//! a track ([`on_midi_track_selected`]) auto-picks whichever [`HARP_KEYS`]
//! entry needs the fewest bend/slide/nearest-note fallbacks to play the
//! track ([`suggest_key`]), the same "always resolve to something
//! reasonable, don't make the user discover a bad fit the hard way"
//! spirit as [`map_pitch`]'s own fallback chain — the harmonica *kind*
//! (diatonic vs. chromatic) is left alone, since switching that is a much
//! bigger, more disruptive change than a key (one more click to undo).
//!
//! The MIDI-file *parsing* itself (tempo map, note extraction, track
//! names) is shared, low-level code living in `crate::song::midi`; this
//! module builds on it with the editor-specific pieces: pitch-to-harp
//! resolution ([`map_pitch`]), key suggestion ([`suggest_key`]), and
//! converting a MIDI tempo map into the editor's own tick/BPM units
//! ([`editor_tempo_map`]).

use bevy::prelude::*;
use midly::Smf;

use super::playback::build_harp;
use super::state::{
    Dir, EditorState, Expr, GridNote, HARP_KEYS, HarmonicaKind, Pitch, max_bend, pitch_compatible,
};
use super::{MIDI_PURPOSE, TICKS_PER_BEAT};
use crate::audio_system::midi::midi_to_freq_hz;
use crate::audio_system::synth::{PhraseNote, render_pcm};
use crate::dialogs::combobox::{ComboboxSelect, spawn_combobox};
use crate::dialogs::file_dialog::FileChosen;
use crate::localization::LocalizationExt;
use crate::song::chart::{Action, TempoPoint, seconds_to_tick};
use crate::song::harmonica::Harmonica;
use crate::song::midi::{
    collect_tempo_map, extract_notes, note_on_count, tick_to_seconds, ticks_per_quarter,
    track_name_of,
};
use bevy_fluent::prelude::Localization;

// ── Resource ──────────────────────────────────────────────────────────────────

/// One track's identity, as shown in the track-picker combobox.
#[derive(Clone)]
pub(super) struct MidiTrackInfo {
    pub(super) index: usize,
    pub(super) name: String,
    pub(super) note_count: usize,
}

impl MidiTrackInfo {
    /// The combobox option label for this track — also how a clicked
    /// [`ComboboxSelect::value`] is matched back to a track index, so this
    /// is the *only* place that formatting is decided.
    pub(super) fn option_label(&self) -> String {
        format!("[{}] {} ({} notes)", self.index, self.name, self.note_count)
    }
}

/// The currently loaded MIDI file, kept as raw bytes (not a parsed [`Smf`],
/// which borrows the byte buffer) so it can be re-parsed cheaply whenever
/// the user switches which track to import — see the module docs on why
/// this module never stores a parsed `Smf` across a frame boundary.
#[derive(Resource)]
pub(super) struct MidiImport {
    pub(super) path: std::path::PathBuf,
    pub(super) bytes: Vec<u8>,
    pub(super) tracks: Vec<MidiTrackInfo>,
    pub(super) selected: Option<usize>,
}

/// Fired once when a new MIDI file finishes loading (not on every track
/// selection) — the signal [`rebuild_midi_track_combobox`] rebuilds on,
/// kept distinct from `MidiImport` mutation in general so picking a track
/// doesn't also tear down and respawn the combobox out from under the click
/// that just landed on it.
#[derive(Message)]
pub(super) struct MidiFileLoaded;

// ── Pitch mapping ─────────────────────────────────────────────────────────────

/// Resolves `target` (a MIDI note number) onto `harp`: an exact blow/draw
/// match if one exists; otherwise, for a diatonic harp, a bend reachable
/// within [`max_bend`]'s per-hole cap (draw bend on holes 1..=6, blow bend
/// on 7..=hole_count — mirroring real harmonica bend physics), or, for a
/// chromatic harp, a slide (which raises a hole's natural note by a
/// semitone, so a target one semitone above some hole's natural note is
/// reachable that way); otherwise the nearest playable natural note, so
/// this always resolves to *something* rather than silently dropping the
/// MIDI note.
pub(super) fn map_pitch(target: u8, harp: &Harmonica, kind: HarmonicaKind) -> (u8, Dir, Pitch) {
    let hole_count = harp.hole_count();

    for hole in 1..=hole_count {
        if harp.wind_direction_midi(hole, &Action::Blow) == Some(target) {
            return (hole, Dir::Blow, Pitch::Normal);
        }
        if harp.wind_direction_midi(hole, &Action::Draw) == Some(target) {
            return (hole, Dir::Draw, Pitch::Normal);
        }
    }

    match kind {
        HarmonicaKind::Diatonic => {
            for hole in 1..=hole_count.min(6) {
                if let Some(reed) = harp.wind_direction_midi(hole, &Action::Draw)
                    && reed > target
                {
                    let depth = (reed - target) as f32;
                    if depth <= max_bend(hole) + f32::EPSILON && pitch_compatible(Pitch::Bend(depth), hole)
                    {
                        return (hole, Dir::Draw, Pitch::Bend(depth));
                    }
                }
            }
            for hole in 7..=hole_count {
                if let Some(reed) = harp.wind_direction_midi(hole, &Action::Blow)
                    && reed > target
                {
                    let depth = (reed - target) as f32;
                    if depth <= max_bend(hole) + f32::EPSILON && pitch_compatible(Pitch::Bend(depth), hole)
                    {
                        return (hole, Dir::Blow, Pitch::Bend(depth));
                    }
                }
            }
        }
        HarmonicaKind::Chromatic => {
            if let Some(natural) = target.checked_sub(1) {
                for hole in 1..=hole_count {
                    if harp.wind_direction_midi(hole, &Action::Blow) == Some(natural) {
                        return (hole, Dir::Blow, Pitch::Slide);
                    }
                    if harp.wind_direction_midi(hole, &Action::Draw) == Some(natural) {
                        return (hole, Dir::Draw, Pitch::Slide);
                    }
                }
            }
        }
    }

    // Fallback: snap to the nearest playable natural note.
    let mut best: Option<(u8, Dir, u8)> = None;
    for hole in 1..=hole_count {
        for (dir, action) in [(Dir::Blow, Action::Blow), (Dir::Draw, Action::Draw)] {
            if let Some(m) = harp.wind_direction_midi(hole, &action) {
                let dist = m.abs_diff(target);
                if best.is_none_or(|(_, _, best_dist)| dist < best_dist) {
                    best = Some((hole, dir, dist));
                }
            }
        }
    }
    best.map(|(hole, dir, _)| (hole, dir, Pitch::Normal))
        // Only unreachable if `harp` has no playable holes at all, which
        // never happens for a real Diatonic/Chromatic harp — 1 is as safe
        // a hole/action default as any.
        .unwrap_or((1, Dir::Blow, Pitch::Normal))
}

/// Fraction of `midi_keys` that land on `key`/`kind`'s harp via an *exact*
/// natural blow/draw match (no bend/slide/nearest-note fallback needed) —
/// the fitness measure [`suggest_key`] maximizes. `midi_keys` empty scores
/// `0.0` rather than dividing by zero (never actually reached in practice:
/// `import_track_notes` already rejects an empty track before this would
/// run on one).
fn key_fit_score(midi_keys: &[u8], key: &str, kind: HarmonicaKind) -> f32 {
    if midi_keys.is_empty() {
        return 0.0;
    }
    let harp = build_harp(key, kind);
    let exact = midi_keys
        .iter()
        .filter(|&&target| {
            (1..=harp.hole_count()).any(|hole| {
                harp.wind_direction_midi(hole, &Action::Blow) == Some(target)
                    || harp.wind_direction_midi(hole, &Action::Draw) == Some(target)
            })
        })
        .count();
    exact as f32 / midi_keys.len() as f32
}

/// The [`HARP_KEYS`] entry that best fits `midi_keys` for `kind` — the one
/// needing the fewest bends/slides/nearest-note fallbacks to play (highest
/// [`key_fit_score`]). Ties keep whichever key sorts earlier in
/// `HARP_KEYS`, so the result is deterministic regardless of float
/// rounding. Lets MIDI import pick a sensible key on its own — the same
/// "always resolves to something reasonable" spirit as [`map_pitch`]'s own
/// bend/slide/nearest-note fallback chain, rather than requiring the user
/// to already have the right key selected (or to discover a bad fit only
/// after seeing how many notes needed a fallback).
pub(super) fn suggest_key(midi_keys: &[u8], kind: HarmonicaKind) -> &'static str {
    let mut best_key = HARP_KEYS[0];
    let mut best_score = -1.0;
    for &key in &HARP_KEYS {
        let score = key_fit_score(midi_keys, key, kind);
        if score > best_score {
            best_score = score;
            best_key = key;
        }
    }
    best_key
}

/// The raw MIDI key numbers of every note in `track_index`, in the order
/// [`extract_notes`] produces them — the input [`suggest_key`] scores
/// candidate harp keys against.
pub(super) fn track_midi_keys(bytes: &[u8], track_index: usize) -> Result<Vec<u8>, String> {
    let smf = Smf::parse(bytes).map_err(|e| e.to_string())?;
    let track = smf
        .tracks
        .get(track_index)
        .ok_or_else(|| "track index out of range".to_string())?;
    Ok(extract_notes(track).into_iter().map(|n| n.key).collect())
}

// ── Track listing / import ───────────────────────────────────────────────────

pub(super) fn list_midi_tracks(bytes: &[u8]) -> Result<Vec<MidiTrackInfo>, String> {
    let smf = Smf::parse(bytes).map_err(|e| e.to_string())?;
    Ok(smf
        .tracks
        .iter()
        .enumerate()
        .map(|(index, t)| MidiTrackInfo {
            index,
            name: track_name_of(t).unwrap_or_else(|| format!("Track {index}")),
            note_count: note_on_count(t),
        })
        .collect())
}

/// Converts a MIDI tempo map (`(tick, microseconds_per_quarter)`, file `tpq`
/// units) into the editor's own tempo map (`TICKS_PER_BEAT` ticks, `bpm`).
/// Each point is placed by its *real time* position (`tick_to_seconds`/
/// `seconds_to_tick` against the already-converted prefix), not its raw
/// tick, since a MIDI file's `tpq` has no fixed ratio to the editor's
/// resolution the way two `resolution: TICKS_PER_BEAT` charts do (see
/// `harpchart::load_harpchart`'s simpler constant-ratio rescaling there).
fn editor_tempo_map(midi_tempo: &[(u64, u32)], tpq: u32) -> Vec<TempoPoint> {
    let mut editor_map: Vec<TempoPoint> = Vec::with_capacity(midi_tempo.len());
    for &(tick, us) in midi_tempo {
        let bpm = (60_000_000.0 / us as f64).clamp(20.0, 300.0) as f32;
        let editor_tick = if editor_map.is_empty() {
            0
        } else {
            let secs = tick_to_seconds(tick, tpq, midi_tempo);
            seconds_to_tick(secs, TICKS_PER_BEAT as u32, &editor_map)
        };
        editor_map.push(TempoPoint {
            tick: editor_tick,
            bpm,
        });
    }
    editor_map
}

pub(super) struct ImportedTrack {
    pub(super) initial_bpm: f32,
    /// Every tempo change after the opening one, already in the editor's
    /// own tick unit — see [`editor_tempo_map`]. Empty for the common case
    /// of a MIDI file with no mid-song tempo automation.
    pub(super) tempo_changes: Vec<(usize, f32)>,
    pub(super) notes: Vec<GridNote>,
}

/// Extracts `track_index`'s notes, quantized onto the editor's own tick
/// grid — a MIDI file's own tempo *map* (not just its first tempo) carries
/// over via [`editor_tempo_map`], so a note lands at the right editor tick
/// even after a mid-song tempo change.
pub(super) fn import_track_notes(
    bytes: &[u8],
    track_index: usize,
    key: &str,
    kind: HarmonicaKind,
) -> Result<ImportedTrack, String> {
    let smf = Smf::parse(bytes).map_err(|e| e.to_string())?;
    let tpq = ticks_per_quarter(&smf)?;
    let track = smf
        .tracks
        .get(track_index)
        .ok_or_else(|| "track index out of range".to_string())?;
    let raw_notes = extract_notes(track);
    if raw_notes.is_empty() {
        return Err("selected track has no notes".to_string());
    }

    let midi_tempo = collect_tempo_map(&smf);
    let editor_map = editor_tempo_map(&midi_tempo, tpq);
    let initial_bpm = editor_map[0].bpm;

    let harp = build_harp(key, kind);
    let mut notes = Vec::with_capacity(raw_notes.len());
    for (id, n) in raw_notes.into_iter().enumerate() {
        let (hole, dir, pitch) = map_pitch(n.key, &harp, kind);
        let start_secs = tick_to_seconds(n.start_tick, tpq, &midi_tempo);
        let end_secs = tick_to_seconds(n.start_tick + n.dur_ticks, tpq, &midi_tempo);
        let tick = seconds_to_tick(start_secs, TICKS_PER_BEAT as u32, &editor_map) as usize;
        let end_tick = seconds_to_tick(end_secs, TICKS_PER_BEAT as u32, &editor_map) as usize;
        let len = end_tick.saturating_sub(tick).max(1);
        notes.push(GridNote {
            id: id as u32,
            hole,
            tick,
            len,
            dir,
            pitch,
            expr: Expr::None,
        });
    }
    let tempo_changes = editor_map[1..]
        .iter()
        .map(|p| (p.tick as usize, p.bpm))
        .collect();
    Ok(ImportedTrack {
        initial_bpm,
        tempo_changes,
        notes,
    })
}

/// A "processed" copy of the MIDI file with `track_index` removed, so the
/// original file the user picked is never modified.
pub(super) fn remove_track_bytes(bytes: &[u8], track_index: usize) -> Result<Vec<u8>, String> {
    let mut smf = Smf::parse(bytes).map_err(|e| e.to_string())?;
    if track_index >= smf.tracks.len() {
        return Err("track index out of range".to_string());
    }
    smf.tracks.remove(track_index);
    let mut out = Vec::new();
    smf.write_std(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Mixes every track *except* `skip_track` down to a single PCM buffer via
/// the shared `audio_system::synth::render_pcm` (which already sums
/// overlapping notes — the same machinery a chord preview uses) — a
/// synthesized stand-in backing track, not a sampled/GM-accurate mix, since
/// the editor has only ever had one instrument voice to render with.
pub(super) fn render_backing_pcm(bytes: &[u8], skip_track: usize) -> Result<(f32, Vec<f32>), String> {
    let smf = Smf::parse(bytes).map_err(|e| e.to_string())?;
    let tpq = ticks_per_quarter(&smf)?;
    let tempo = collect_tempo_map(&smf);
    let initial_bpm = (60_000_000.0 / tempo[0].1 as f64).clamp(20.0, 300.0) as f32;
    let secs_per_tick = 60.0 / initial_bpm.max(1.0) as f64 / TICKS_PER_BEAT as f64;

    let mut phrase = Vec::new();
    for (i, track) in smf.tracks.iter().enumerate() {
        if i == skip_track {
            continue;
        }
        for n in extract_notes(track) {
            let start_secs = tick_to_seconds(n.start_tick, tpq, &tempo);
            let end_secs = tick_to_seconds(n.start_tick + n.dur_ticks, tpq, &tempo);
            let tick = (start_secs / secs_per_tick).round() as usize;
            let len = (((end_secs - start_secs) / secs_per_tick).round() as usize).max(1);
            phrase.push(PhraseNote {
                tick,
                len,
                freq: Some(midi_to_freq_hz(n.key as f32)),
                expr: Expr::None,
            });
        }
    }
    if phrase.is_empty() {
        return Err("no notes left outside the selected track".to_string());
    }
    Ok((initial_bpm, render_pcm(&phrase, secs_per_tick as f32)))
}

// ── Systems ───────────────────────────────────────────────────────────────────

pub(super) fn handle_midi_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut commands: Commands,
    mut loaded: MessageWriter<MidiFileLoaded>,
) {
    for ev in chosen.read() {
        if ev.purpose != MIDI_PURPOSE {
            continue;
        }
        let bytes = match std::fs::read(&ev.path) {
            Ok(b) => b,
            Err(e) => {
                println!("MIDI import failed (read): {e}");
                continue;
            }
        };
        let tracks = match list_midi_tracks(&bytes) {
            Ok(t) => t,
            Err(e) => {
                println!("MIDI import failed (parse): {e}");
                continue;
            }
        };
        if tracks.is_empty() {
            println!("MIDI import failed: {} has no tracks", ev.path.display());
            continue;
        }
        commands.insert_resource(MidiImport {
            path: ev.path.clone(),
            bytes,
            tracks,
            selected: None,
        });
        loaded.write(MidiFileLoaded);
        println!("Loaded MIDI: {}", ev.path.display());
    }
}

/// Despawns and respawns the track-picker combobox whenever a new MIDI file
/// finishes loading (see [`MidiFileLoaded`]'s doc comment for why this
/// isn't just `resource_changed::<MidiImport>`).
pub(super) fn rebuild_midi_track_combobox(
    mut loaded: MessageReader<MidiFileLoaded>,
    mut commands: Commands,
    midi: Option<Res<MidiImport>>,
    slot: Query<(Entity, Option<&Children>), With<super::ui::MidiTrackComboboxSlot>>,
    editor_root: Query<Entity, With<super::ui::EditorRoot>>,
    loc: Res<Localization>,
) {
    if loaded.read().next().is_none() {
        return;
    }
    let (Ok((slot_entity, children)), Ok(backdrop_parent), Some(midi)) =
        (slot.single(), editor_root.single(), midi)
    else {
        return;
    };
    if let Some(children) = children {
        for &c in children {
            commands.entity(c).despawn();
        }
    }
    let options: Vec<String> = midi.tracks.iter().map(MidiTrackInfo::option_label).collect();
    let Some(first) = options.first().cloned() else {
        return;
    };
    spawn_combobox(
        &mut commands,
        slot_entity,
        backdrop_parent,
        &loc.msg("editor-field-midi-track"),
        &options,
        &first,
        on_midi_track_selected,
    );
}

fn on_midi_track_selected(
    ev: On<ComboboxSelect>,
    mut midi: ResMut<MidiImport>,
    mut state: ResMut<EditorState>,
) {
    let Some(info) = midi
        .tracks
        .iter()
        .find(|t| t.option_label() == ev.value)
        .cloned()
    else {
        return;
    };
    // Auto-pick the best-fitting key for this track rather than importing
    // onto whatever key the editor already happened to be set to — same
    // "don't make the user discover a bad fit the hard way" reasoning as
    // `map_pitch`'s own fallback chain. Kept within the harmonica *kind*
    // already selected (diatonic vs. chromatic is a much bigger, more
    // disruptive choice than a key, which is just one more click to
    // change if this guess isn't the one the user wanted).
    let key = match track_midi_keys(&midi.bytes, info.index) {
        Ok(keys) => suggest_key(&keys, state.harmonica_kind).to_string(),
        Err(_) => state.key.clone(),
    };
    match import_track_notes(&midi.bytes, info.index, &key, state.harmonica_kind) {
        Ok(imported) => {
            state.next_id = imported.notes.len() as u32;
            state.notes = imported.notes;
            state.selected = None;
            state.dragging = None;
            state.tempo = format!("{}", imported.initial_bpm.round() as u32);
            state.tempo_changes = imported.tempo_changes;
            state.key = key.clone();
            midi.selected = Some(info.index);
            println!(
                "Imported MIDI track {}: {} ({} notes), auto-picked key {key}",
                info.index, info.name, info.note_count
            );
        }
        Err(e) => println!("MIDI track import failed: {e}"),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::midi::{meta, note_on, note_off, smf_bytes};
    use crate::song::harmonica::{chromatic_harp, richter_harp};
    use midly::num::u24;
    use midly::MetaMessage;

    // ── editor_tempo_map ──────────────────────────────────────────────────────────

    #[test]
    fn a_single_tempo_point_lands_at_tick_zero() {
        let map = editor_tempo_map(&[(0, 500_000)], 480);
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].tick, 0);
        assert!((map[0].bpm - 120.0).abs() < 0.01);
    }

    #[test]
    fn a_later_tempo_change_is_placed_by_real_time_not_raw_tick() {
        // 480 tpq at 120 BPM: tick 480 is exactly one beat (0.5s) in, which
        // is exactly TICKS_PER_BEAT in the editor's own resolution.
        let map = editor_tempo_map(&[(0, 500_000), (480, 250_000)], 480);
        assert_eq!(map.len(), 2);
        assert_eq!(map[1].tick, TICKS_PER_BEAT as u64);
        assert!((map[1].bpm - 240.0).abs() < 0.01);
    }

    #[test]
    fn bpm_is_clamped_to_a_sane_range() {
        let map = editor_tempo_map(&[(0, 20_000_000)], 480); // 3 BPM, absurdly slow
        assert!((map[0].bpm - 20.0).abs() < 0.01);
    }

    // ── map_pitch ─────────────────────────────────────────────────────────────────

    #[test]
    fn map_pitch_maps_a_directly_playable_blow_note() {
        let harp = richter_harp("C");
        // C4 is hole 1 blow on a C richter harp.
        let midi = crate::audio_system::midi::note_to_midi("C4").unwrap() as u8;
        let (hole, dir, pitch) = map_pitch(midi, &harp, HarmonicaKind::Diatonic);
        assert_eq!(hole, 1);
        assert_eq!(dir, Dir::Blow);
        assert_eq!(pitch, Pitch::Normal);
    }

    #[test]
    fn map_pitch_reaches_a_low_hole_draw_bend_within_the_editors_own_cap() {
        let harp = richter_harp("C");
        // Hole 1 draw is D4; bending down one semitone reaches C#4, which
        // isn't directly playable and is within hole 1's max_bend (1.0).
        let target = crate::audio_system::midi::note_to_midi("D4").unwrap() as u8 - 1;
        let (hole, dir, pitch) = map_pitch(target, &harp, HarmonicaKind::Diatonic);
        assert_eq!(hole, 1);
        assert_eq!(dir, Dir::Draw);
        assert_eq!(pitch, Pitch::Bend(1.0));
    }

    #[test]
    fn map_pitch_uses_slide_on_a_chromatic_harp() {
        let harp = chromatic_harp("C");
        // Hole 1 blow is C4; a target one semitone above (C#4) is only
        // reachable via the slide button on a chromatic harp.
        let target = crate::audio_system::midi::note_to_midi("C4").unwrap() as u8 + 1;
        let (hole, dir, pitch) = map_pitch(target, &harp, HarmonicaKind::Chromatic);
        assert_eq!(hole, 1);
        assert_eq!(dir, Dir::Blow);
        assert_eq!(pitch, Pitch::Slide);
    }

    #[test]
    fn map_pitch_falls_back_to_the_nearest_playable_note() {
        let harp = richter_harp("C");
        let (hole, _dir, pitch) = map_pitch(0, &harp, HarmonicaKind::Diatonic);
        assert_eq!(pitch, Pitch::Normal);
        assert!((1..=10).contains(&hole));
    }

    // ── key_fit_score / suggest_key ──────────────────────────────────────────────

    fn midi(note: &str) -> u8 {
        crate::audio_system::midi::note_to_midi(note).unwrap() as u8
    }

    #[test]
    fn key_fit_score_is_perfect_for_notes_that_are_all_natural_on_that_key() {
        // C4/E4/G4 are hole 1/2/3 blow on a C richter harp — no bend needed.
        let keys = [midi("C4"), midi("E4"), midi("G4")];
        assert_eq!(key_fit_score(&keys, "C", HarmonicaKind::Diatonic), 1.0);
    }

    #[test]
    fn key_fit_score_is_lower_when_notes_need_a_fallback() {
        // The same notes, scored against a harp a tritone away, won't line
        // up on natural blow/draw reeds nearly as often.
        let keys = [midi("C4"), midi("E4"), midi("G4")];
        let c_score = key_fit_score(&keys, "C", HarmonicaKind::Diatonic);
        let off_score = key_fit_score(&keys, "F#", HarmonicaKind::Diatonic);
        assert!(off_score < c_score);
    }

    #[test]
    fn key_fit_score_is_zero_for_no_notes() {
        assert_eq!(key_fit_score(&[], "C", HarmonicaKind::Diatonic), 0.0);
    }

    #[test]
    fn suggest_key_picks_the_key_whose_harp_the_notes_are_natural_on() {
        // A transposed-up-a-tone version of the same C-harp-natural notes
        // should suggest D, not C.
        let keys = [midi("D4"), midi("F#4"), midi("A4")];
        assert_eq!(suggest_key(&keys, HarmonicaKind::Diatonic), "D");
    }

    #[test]
    fn suggest_key_breaks_ties_by_harp_keys_own_order() {
        // No notes at all fits every key equally (badly) — the first
        // HARP_KEYS entry wins, deterministically.
        assert_eq!(suggest_key(&[], HarmonicaKind::Diatonic), HARP_KEYS[0]);
    }

    // ── track_midi_keys ───────────────────────────────────────────────────────────

    #[test]
    fn track_midi_keys_extracts_every_notes_pitch_in_order() {
        let bytes = smf_bytes(vec![vec![
            note_on(0, 60, 100),
            note_off(10, 60),
            note_on(0, 64, 100),
            note_off(10, 64),
        ]]);
        assert_eq!(track_midi_keys(&bytes, 0).unwrap(), vec![60, 64]);
    }

    #[test]
    fn track_midi_keys_rejects_an_out_of_range_track() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100), note_off(10, 60)]]);
        assert!(track_midi_keys(&bytes, 5).is_err());
    }

    // ── list_midi_tracks / option_label ──────────────────────────────────────────

    #[test]
    fn list_midi_tracks_reports_name_and_note_count_per_track() {
        let bytes = smf_bytes(vec![
            vec![meta(0, MetaMessage::TrackName(b"Bass")), note_on(0, 40, 100), note_off(10, 40)],
            vec![meta(0, MetaMessage::TrackName(b"Lead"))],
        ]);
        let tracks = list_midi_tracks(&bytes).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].name, "Bass");
        assert_eq!(tracks[0].note_count, 1);
        assert_eq!(tracks[1].name, "Lead");
        assert_eq!(tracks[1].note_count, 0);
    }

    #[test]
    fn option_label_encodes_index_name_and_count_uniquely() {
        let info = MidiTrackInfo {
            index: 2,
            name: "Bass".to_string(),
            note_count: 5,
        };
        assert_eq!(info.option_label(), "[2] Bass (5 notes)");
    }

    // ── import_track_notes ───────────────────────────────────────────────────────

    #[test]
    fn import_track_notes_maps_pitches_and_quantizes_timing() {
        let bytes = smf_bytes(vec![vec![
            meta(0, MetaMessage::Tempo(u24::from(500_000))), // 120 BPM
            note_on(0, 60, 100), // C4: hole 1 blow
            note_off(480, 60),   // one beat (480 ticks at 480 tpq)
        ]]);
        let imported = import_track_notes(&bytes, 0, "C", HarmonicaKind::Diatonic).unwrap();
        assert_eq!(imported.initial_bpm.round(), 120.0);
        assert_eq!(imported.notes.len(), 1);
        let n = &imported.notes[0];
        assert_eq!(n.hole, 1);
        assert_eq!(n.dir, Dir::Blow);
        assert_eq!(n.pitch, Pitch::Normal);
        // One beat at TICKS_PER_BEAT (4) resolution is 4 internal ticks.
        assert_eq!(n.tick, 0);
        assert_eq!(n.len, TICKS_PER_BEAT);
    }

    #[test]
    fn import_track_notes_carries_a_mid_song_tempo_change_into_tempo_changes() {
        let bytes = smf_bytes(vec![vec![
            meta(0, MetaMessage::Tempo(u24::from(500_000))), // 120 BPM
            note_on(0, 60, 100),
            note_off(480, 60), // one beat @ 120bpm
            meta(0, MetaMessage::Tempo(u24::from(250_000))), // doubles to 240 BPM
            note_on(0, 62, 100),
            note_off(480, 62), // one more beat, now @ 240bpm
        ]]);
        let imported = import_track_notes(&bytes, 0, "C", HarmonicaKind::Diatonic).unwrap();
        assert_eq!(imported.initial_bpm.round(), 120.0);
        assert_eq!(imported.tempo_changes.len(), 1);
        // The tempo doubles exactly one beat in -> editor tick TICKS_PER_BEAT.
        assert_eq!(imported.tempo_changes[0].0, TICKS_PER_BEAT);
        assert_eq!(imported.tempo_changes[0].1.round(), 240.0);
        // The second note starts right where the first one ends.
        assert_eq!(imported.notes[1].tick, TICKS_PER_BEAT);
    }

    #[test]
    fn import_track_notes_rejects_an_out_of_range_track() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100), note_off(10, 60)]]);
        assert!(import_track_notes(&bytes, 5, "C", HarmonicaKind::Diatonic).is_err());
    }

    #[test]
    fn import_track_notes_rejects_an_empty_track() {
        let bytes = smf_bytes(vec![vec![meta(0, MetaMessage::TrackName(b"Empty"))]]);
        assert!(import_track_notes(&bytes, 0, "C", HarmonicaKind::Diatonic).is_err());
    }

    // ── remove_track_bytes ───────────────────────────────────────────────────────

    #[test]
    fn remove_track_bytes_drops_only_the_named_track() {
        let bytes = smf_bytes(vec![
            vec![note_on(0, 60, 100), note_off(10, 60)],
            vec![note_on(0, 64, 100), note_off(10, 64)],
        ]);
        let out = remove_track_bytes(&bytes, 0).unwrap();
        let smf = Smf::parse(&out).unwrap();
        assert_eq!(smf.tracks.len(), 1);
        // The surviving track is the one that used to be index 1.
        let notes = extract_notes(&smf.tracks[0]);
        assert_eq!(notes[0].key, 64);
    }

    #[test]
    fn remove_track_bytes_rejects_an_out_of_range_track() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100), note_off(10, 60)]]);
        assert!(remove_track_bytes(&bytes, 3).is_err());
    }

    // ── render_backing_pcm ───────────────────────────────────────────────────────

    #[test]
    fn render_backing_pcm_skips_only_the_named_track_and_is_audible() {
        let bytes = smf_bytes(vec![
            vec![note_on(0, 60, 100), note_off(480, 60)],
            vec![note_on(0, 64, 100), note_off(480, 64)],
        ]);
        let (bpm, pcm) = render_backing_pcm(&bytes, 0).unwrap();
        assert!(bpm > 0.0);
        assert!(!pcm.is_empty());
        assert!(pcm.iter().any(|&s| s.abs() > 0.01), "backing track should be audible");
    }

    #[test]
    fn render_backing_pcm_errors_when_nothing_is_left_to_render() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100), note_off(480, 60)]]);
        assert!(render_backing_pcm(&bytes, 0).is_err());
    }
}
