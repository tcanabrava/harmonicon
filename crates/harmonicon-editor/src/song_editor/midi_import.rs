// SPDX-License-Identifier: MIT

//! Imports a MIDI file's track into the note grid as [`GridNote`]s in
//! memory (destination is [`EditorState`], not disk), generalized to
//! whatever harmonica key the session is set to. Picking a track
//! ([`on_midi_track_selected`]) auto-picks whichever [`HARP_KEYS`] entry
//! needs the fewest bend/slide/nearest-note fallbacks ([`suggest_key`]);
//! harmonica *kind* is left alone, since switching that is far more
//! disruptive than a key.
//!
//! MIDI *parsing* (tempo map, note extraction, track names) is shared,
//! low-level code in `harmonicon_core::midi_file`; this module adds the
//! editor-specific pieces: pitch-to-harp resolution ([`map_pitch`]), key
//! suggestion ([`suggest_key`]), and tempo-map conversion
//! ([`editor_tempo_map`]).
//!
//! *Which* track is a harmonica part is not an editor question, so the
//! answer comes from `harmonicon_score`: [`default_track`] defers to
//! `pick_harmonica_track`, the same rule that applies when a `.mid` is
//! played directly as a song (`harmonicon_song::song::score_song`), and a
//! named match is imported on load rather than merely pre-selected. The
//! editor keeps the *fallback* the loader can't have — an unnamed file
//! shows a picker, where a loader has nowhere to ask.

use bevy::prelude::*;
use midly::Smf;

use super::pitch_map::{map_pitch, suggest_key};
use super::playback::build_harp;
use super::state::{EditorState, Expr, GridNote, HarmonicaKind};
use super::{MIDI_PURPOSE, TICKS_PER_BEAT};
use bevy_fluent::prelude::Localization;
use harmonicon_core::chart::{TempoPoint, seconds_to_tick};
use harmonicon_core::midi_file::{
    collect_tempo_map, extract_notes, notes_to_phrase, tick_to_seconds, ticks_per_quarter,
};
use harmonicon_core::synth::render_pcm;
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_score::midi::MidiScore;
use harmonicon_score::{ScoreFile, ScoreTrack, pick_harmonica_track};
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};
use harmonicon_ui::dialogs::file_dialog::FileChosen;
use harmonicon_ui::dialogs::tooltip::Tooltip;

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

// ── Key suggestion input ─────────────────────────────────────────────────────

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
    let score = MidiScore::parse(bytes.to_vec()).map_err(|e| e.to_string())?;
    Ok(score
        .tracks()
        .iter()
        .map(|t| MidiTrackInfo {
            index: t.index,
            // A track with no name of its own still needs a label to click,
            // which is why this fallback lives here and not in `ScoreTrack`
            // — a reader reports what the file says, and "unnamed" is a
            // fact the picker's own selection rules depend on.
            name: t
                .name
                .clone()
                .unwrap_or_else(|| format!("Track {}", t.index)),
            note_count: t.note_count,
        })
        .collect())
}

/// Which track the picker should start on.
///
/// **Not the first one**, which is what a combobox defaults to and is wrong
/// nearly every time: a MIDI file's track 0 conventionally carries only
/// tempo and metadata, so importing it yields an empty grid and the user
/// has to discover that by trying. Prefers a track *named* for a harmonica
/// (`harmonicon_score::pick_harmonica_track` — the same rule that picks a
/// track when a `.mid` is played directly as a song), then simply the first
/// track with any notes in it.
pub(super) fn default_track(tracks: &[MidiTrackInfo]) -> Option<usize> {
    pick_harmonica_track(&score_tracks(tracks))
        .or_else(|| tracks.iter().find(|t| t.note_count > 0).map(|t| t.index))
        .or_else(|| tracks.first().map(|t| t.index))
}

/// The picker's own track list back in `harmonicon_score`'s vocabulary, so
/// track selection follows one rule whether a MIDI arrives here or is played
/// directly as a song.
fn score_tracks(tracks: &[MidiTrackInfo]) -> Vec<ScoreTrack> {
    tracks
        .iter()
        .map(|t| ScoreTrack {
            index: t.index,
            name: Some(t.name.clone()),
            note_count: t.note_count,
        })
        .collect()
}

/// Converts a MIDI tempo map (`(tick, microseconds_per_quarter)`, file `tpq`
/// units) into the editor's own (`TICKS_PER_BEAT` ticks, `bpm`) map. Each
/// point is placed by real-time position (`tick_to_seconds`/
/// `seconds_to_tick`), since a MIDI file's `tpq` has no fixed ratio to the
/// editor's resolution the way two `resolution: TICKS_PER_BEAT` charts do
/// (contrast `harpchart::load_harpchart`'s simpler constant-ratio rescale).
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
pub(super) fn render_backing_pcm(
    bytes: &[u8],
    skip_track: usize,
) -> Result<(f32, Vec<f32>), String> {
    let smf = Smf::parse(bytes).map_err(|e| e.to_string())?;
    let tpq = ticks_per_quarter(&smf)?;
    let tempo = collect_tempo_map(&smf);
    let initial_bpm = (60_000_000.0 / tempo[0].1 as f64).clamp(20.0, 300.0) as f32;
    let secs_per_tick = 60.0 / initial_bpm.max(1.0) as f64 / TICKS_PER_BEAT as f64;

    let mut notes = Vec::new();
    for (i, track) in smf.tracks.iter().enumerate() {
        if i == skip_track {
            continue;
        }
        notes.extend(extract_notes(track));
    }
    if notes.is_empty() {
        return Err("no notes left outside the selected track".to_string());
    }
    let phrase = notes_to_phrase(&notes, tpq, &tempo, secs_per_tick);
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
    midi: Option<ResMut<MidiImport>>,
    mut state: ResMut<EditorState>,
    slot: Query<(Entity, Option<&Children>), With<super::ui::MidiTrackComboboxSlot>>,
    editor_root: Query<Entity, With<super::ui::EditorRoot>>,
    loc: Res<Localization>,
) {
    if loaded.read().next().is_none() {
        return;
    }
    let (Ok((slot_entity, children)), Ok(backdrop_parent), Some(mut midi)) =
        (slot.single(), editor_root.single(), midi)
    else {
        return;
    };
    if let Some(children) = children {
        for &c in children {
            commands.entity(c).despawn();
        }
    }
    let options: Vec<String> = midi
        .tracks
        .iter()
        .map(MidiTrackInfo::option_label)
        .collect();
    // `default_track`, not `options.first()`: track 0 is conventionally the
    // tempo/metadata track, so defaulting to it shows an empty grid.
    let Some(selected) = default_track(&midi.tracks)
        .and_then(|index| midi.tracks.iter().find(|t| t.index == index))
        .map(MidiTrackInfo::option_label)
    else {
        return;
    };
    let combo = spawn_combobox(
        &mut commands,
        slot_entity,
        backdrop_parent,
        &loc.msg("editor-field-midi-track"),
        &options,
        &selected,
        on_midi_track_selected,
    );
    commands.entity(combo).insert(Tooltip(String::from(
        loc.msg("editor-field-midi-track-tooltip"),
    )));

    // A track the file itself names as the harmonica needs no decision from
    // the user, so import it now rather than leaving the grid empty behind a
    // combobox already displaying the right answer — picking the value
    // that's already shown wouldn't fire a `ComboboxSelect` anyway. An
    // unnamed file is left alone deliberately: the default above is a guess
    // worth showing, not one worth acting on.
    if let Some(index) = pick_harmonica_track(&score_tracks(&midi.tracks)) {
        import_selected_track(&mut midi, &mut state, index);
    }
}

fn on_midi_track_selected(
    ev: On<ComboboxSelect>,
    mut midi: ResMut<MidiImport>,
    mut state: ResMut<EditorState>,
) {
    let Some(index) = midi
        .tracks
        .iter()
        .find(|t| t.option_label() == ev.value)
        .map(|t| t.index)
    else {
        return;
    };
    import_selected_track(&mut midi, &mut state, index);
}

/// Imports one track into the grid, replacing whatever is there.
///
/// Split out of [`on_midi_track_selected`] so a track the file *names* as
/// the harmonica can be imported the moment the file loads, without
/// synthesizing a combobox selection the user never made.
fn import_selected_track(midi: &mut MidiImport, state: &mut EditorState, index: usize) {
    let Some(info) = midi.tracks.iter().find(|t| t.index == index).cloned() else {
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
            state.selected.clear();
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
    use super::super::state::{Dir, Pitch};
    use super::*;
    use harmonicon_core::midi_file::{meta, note_off, note_on, smf_bytes};
    use midly::MetaMessage;
    use midly::num::u24;

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

    // ── default_track ────────────────────────────────────────────────────────────

    #[test]
    fn a_track_named_harmonica_is_the_default_even_when_it_is_not_first() {
        let bytes = smf_bytes(vec![
            vec![meta(0, MetaMessage::TrackName(b"Tempo"))],
            vec![
                meta(0, MetaMessage::TrackName(b"Guitar")),
                note_on(0, 40, 100),
                note_off(10, 40),
            ],
            vec![
                meta(0, MetaMessage::TrackName(b"Harmonica")),
                note_on(0, 60, 100),
                note_off(10, 60),
            ],
        ]);
        let tracks = list_midi_tracks(&bytes).unwrap();
        assert_eq!(default_track(&tracks), Some(2));
    }

    #[test]
    fn the_default_skips_a_metadata_only_first_track() {
        // Track 0 conventionally carries tempo and nothing else, so the
        // combobox's own "first option" default would show an empty grid.
        let bytes = smf_bytes(vec![
            vec![meta(0, MetaMessage::TrackName(b"Conductor"))],
            vec![
                meta(0, MetaMessage::TrackName(b"Lead")),
                note_on(0, 60, 100),
                note_off(10, 60),
            ],
        ]);
        let tracks = list_midi_tracks(&bytes).unwrap();
        assert_eq!(default_track(&tracks), Some(1));
    }

    #[test]
    fn an_orchestral_harp_track_is_not_treated_as_the_harmonica() {
        // Bare "harp" is deliberately not a harmonica name; the fallback
        // still offers it, since it is the only track with notes.
        let bytes = smf_bytes(vec![vec![
            meta(0, MetaMessage::TrackName(b"Concert Harp")),
            note_on(0, 60, 100),
            note_off(10, 60),
        ]]);
        let tracks = list_midi_tracks(&bytes).unwrap();
        assert!(!harmonicon_score::track::name_says_harmonica(Some(
            tracks[0].name.as_str()
        )));
        assert_eq!(default_track(&tracks), Some(0));
    }

    #[test]
    fn a_file_with_no_notes_at_all_is_refused_with_a_reason() {
        // Listing it and offering a picker of empty tracks would only let
        // the user discover by clicking that there is nothing to import.
        let bytes = smf_bytes(vec![vec![meta(0, MetaMessage::TrackName(b"Empty"))]]);
        let Err(err) = list_midi_tracks(&bytes) else {
            panic!("a note-free MIDI file was accepted");
        };
        assert!(err.contains("no tracks"), "unhelpful message: {err}");
    }

    #[test]
    fn list_midi_tracks_reports_name_and_note_count_per_track() {
        let bytes = smf_bytes(vec![
            vec![
                meta(0, MetaMessage::TrackName(b"Bass")),
                note_on(0, 40, 100),
                note_off(10, 40),
            ],
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
            note_on(0, 60, 100),                             // C4: hole 1 blow
            note_off(480, 60),                               // one beat (480 ticks at 480 tpq)
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
            note_off(480, 60),                               // one beat @ 120bpm
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
        assert!(
            pcm.iter().any(|&s| s.abs() > 0.01),
            "backing track should be audible"
        );
    }

    #[test]
    fn render_backing_pcm_errors_when_nothing_is_left_to_render() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100), note_off(480, 60)]]);
        assert!(render_backing_pcm(&bytes, 0).is_err());
    }
}
