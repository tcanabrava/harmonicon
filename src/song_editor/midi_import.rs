// SPDX-License-Identifier: MIT

//! Import a MIDI file's track into the note grid — the Song Editor's
//! analogue of `bin/midi_to_chart`, but generalized to whatever harmonica
//! key/kind the editor session is currently set to (not a fixed C
//! diatonic), and producing [`GridNote`]s in memory instead of writing a
//! chart file, since here the destination is [`EditorState`], not disk.
//!
//! Kept independent from `bin/midi_to_chart`'s own tempo/note-extraction
//! code (a small amount of duplication) rather than sharing it, since the
//! bin's pitch mapping is intentionally simpler (fixed C diatonic, no
//! chromatic/slide support) and already shipped/tested — reusing it here
//! would mean either generalizing it (churn on a working tool for a
//! feature that doesn't need it changed) or accepting its
//! C-diatonic-only limitation inside the editor, which does need to
//! support both harmonica kinds and any key.

use bevy::prelude::*;
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use std::collections::HashMap;

use super::playback::{PhraseNote, build_harp, render_pcm};
use super::state::{
    Dir, EditorState, Expr, GridNote, HarmonicaKind, Pitch, max_bend, pitch_compatible,
};
use super::{MIDI_PURPOSE, TICKS_PER_BEAT};
use crate::audio_system::midi::midi_to_freq_hz;
use crate::dialogs::combobox::{ComboboxSelect, spawn_combobox};
use crate::dialogs::file_dialog::FileChosen;
use crate::localization::LocalizationExt;
use crate::song::chart::Action;
use crate::song::harmonica::Harmonica;
use bevy_fluent::prelude::Localization;

const DEFAULT_TEMPO_US: u32 = 500_000; // 120 BPM if the file specifies none

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

// ── Pure MIDI parsing ─────────────────────────────────────────────────────────

fn ticks_per_quarter(smf: &Smf) -> Result<u32, String> {
    match smf.header.timing {
        Timing::Metrical(tpq) => Ok(tpq.as_int() as u32),
        Timing::Timecode(..) => {
            Err("timecode-based MIDI timing is not supported (need metrical)".to_string())
        }
    }
}

fn track_name_of(track: &[midly::TrackEvent]) -> Option<String> {
    track.iter().find_map(|ev| match ev.kind {
        TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
            let name = String::from_utf8_lossy(bytes).trim().to_string();
            (!name.is_empty()).then_some(name)
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

struct RawNote {
    start_tick: u64,
    dur_ticks: u64,
    key: u8,
}

/// Pairs NoteOn/NoteOff into [`RawNote`]s, ordered by start tick.
fn extract_notes(track: &[midly::TrackEvent]) -> Vec<RawNote> {
    let mut open: HashMap<u8, u64> = HashMap::new();
    let mut notes: Vec<RawNote> = Vec::new();
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
                        notes.push(RawNote {
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

pub(super) struct ImportedTrack {
    pub(super) initial_bpm: f32,
    pub(super) notes: Vec<GridNote>,
}

/// Extracts `track_index`'s notes, quantized onto the editor's own tick grid
/// at that track's initial tempo (the editor has no tempo-map support — see
/// `ROADMAP.md` 0.5 — so a MIDI file with real tempo changes plays back at
/// its first tempo throughout; notes still land at the right *real* time up
/// to that quantization, since positions are computed from absolute
/// seconds, not raw ticks).
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

    let tempo = collect_tempo_map(&smf);
    let initial_bpm = (60_000_000.0 / tempo[0].1 as f64).clamp(20.0, 300.0) as f32;
    let secs_per_tick = 60.0 / initial_bpm.max(1.0) as f64 / TICKS_PER_BEAT as f64;

    let harp = build_harp(key, kind);
    let mut notes = Vec::with_capacity(raw_notes.len());
    for (id, n) in raw_notes.into_iter().enumerate() {
        let (hole, dir, pitch) = map_pitch(n.key, &harp, kind);
        let start_secs = tick_to_seconds(n.start_tick, tpq, &tempo);
        let end_secs = tick_to_seconds(n.start_tick + n.dur_ticks, tpq, &tempo);
        let tick = (start_secs / secs_per_tick).round() as usize;
        let len = (((end_secs - start_secs) / secs_per_tick).round() as usize).max(1);
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
    Ok(ImportedTrack { initial_bpm, notes })
}

/// A copy of the MIDI file with `track_index` removed — the same
/// "processed" copy `bin/midi_to_chart` writes, so the original file the
/// user picked is never modified.
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
/// the editor's own synth (`playback::render_pcm`, which already sums
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
    Ok((initial_bpm, render_pcm(&phrase, initial_bpm)))
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
    match import_track_notes(&midi.bytes, info.index, &state.key, state.harmonica_kind) {
        Ok(imported) => {
            state.next_id = imported.notes.len() as u32;
            state.notes = imported.notes;
            state.selected = None;
            state.dragging = None;
            state.tempo = format!("{}", imported.initial_bpm.round() as u32);
            midi.selected = Some(info.index);
            println!(
                "Imported MIDI track {}: {} ({} notes)",
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
    use crate::song::harmonica::{chromatic_harp, richter_harp};
    use midly::num::{u4, u7, u15, u24, u28};
    use midly::{Format, Header};

    fn meta(delta: u32, kind: MetaMessage<'static>) -> midly::TrackEvent<'static> {
        midly::TrackEvent {
            delta: u28::from(delta),
            kind: TrackEventKind::Meta(kind),
        }
    }

    fn note_on(delta: u32, key: u8, vel: u8) -> midly::TrackEvent<'static> {
        midly::TrackEvent {
            delta: u28::from(delta),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn {
                    key: u7::from(key),
                    vel: u7::from(vel),
                },
            },
        }
    }

    fn note_off(delta: u32, key: u8) -> midly::TrackEvent<'static> {
        midly::TrackEvent {
            delta: u28::from(delta),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff {
                    key: u7::from(key),
                    vel: u7::from(0),
                },
            },
        }
    }

    fn smf_bytes(tracks: Vec<Vec<midly::TrackEvent<'static>>>) -> Vec<u8> {
        let smf = Smf {
            header: Header {
                format: if tracks.len() > 1 {
                    Format::Parallel
                } else {
                    Format::SingleTrack
                },
                timing: Timing::Metrical(u15::from(480)),
            },
            tracks,
        };
        let mut out = Vec::new();
        smf.write_std(&mut out).unwrap();
        out
    }

    // ── track_name_of / note_on_count ────────────────────────────────────────

    #[test]
    fn track_name_of_reads_the_first_track_name_event() {
        let track = vec![meta(0, MetaMessage::TrackName(b"Bass")), note_on(0, 60, 100)];
        assert_eq!(track_name_of(&track).as_deref(), Some("Bass"));
    }

    #[test]
    fn track_name_of_is_none_without_a_name_event() {
        assert_eq!(track_name_of(&[note_on(0, 60, 100)]), None);
    }

    #[test]
    fn track_name_of_is_none_for_a_blank_name() {
        let track = vec![meta(0, MetaMessage::TrackName(b"   "))];
        assert_eq!(track_name_of(&track), None);
    }

    #[test]
    fn note_on_count_ignores_zero_velocity_note_ons() {
        let track = vec![
            note_on(0, 60, 100),
            note_on(10, 62, 0),
            note_off(10, 60),
            note_on(0, 64, 80),
        ];
        assert_eq!(note_on_count(&track), 2);
    }

    // ── collect_tempo_map / tick_to_seconds ─────────────────────────────────────

    #[test]
    fn collect_tempo_map_defaults_to_120bpm_at_tick_zero_when_unset() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100)]]);
        let smf = Smf::parse(&bytes).unwrap();
        assert_eq!(collect_tempo_map(&smf), vec![(0, DEFAULT_TEMPO_US)]);
    }

    #[test]
    fn tick_to_seconds_integrates_across_a_tempo_change() {
        let tpq = 480;
        let tempo = vec![(0u64, 500_000u32), (480, 250_000)];
        assert!((tick_to_seconds(0, tpq, &tempo) - 0.0).abs() < 1e-9);
        assert!((tick_to_seconds(480, tpq, &tempo) - 0.5).abs() < 1e-9);
        assert!((tick_to_seconds(960, tpq, &tempo) - 0.75).abs() < 1e-9);
    }

    // ── extract_notes ─────────────────────────────────────────────────────────────

    #[test]
    fn extract_notes_pairs_note_on_with_note_off() {
        let track = vec![note_on(0, 60, 100), note_off(240, 60)];
        let notes = extract_notes(&track);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].start_tick, 0);
        assert_eq!(notes[0].dur_ticks, 240);
        assert_eq!(notes[0].key, 60);
    }

    #[test]
    fn extract_notes_treats_zero_velocity_note_on_as_note_off() {
        let track = vec![note_on(0, 60, 100), note_on(120, 60, 0)];
        let notes = extract_notes(&track);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].dur_ticks, 120);
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
