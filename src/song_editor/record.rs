// SPDX-License-Identifier: MIT

//! Records notes played live on a real harmonica straight onto the note
//! grid — the Song Editor's live counterpart to `midi_import`, sharing that
//! module's `map_pitch` resolution instead of reading MIDI file bytes. The
//! microphone/pitch-detection pipeline (`main.rs`'s `process_audio`) already
//! runs continuously in the background regardless of `AppState` — the same
//! [`PitchEvent`] stream the Song Editor's own Practice mode already
//! consumes (`practice::practice_tick`) — so recording needs no capture
//! lifecycle of its own, just a per-pitch onset/release diff against
//! successive events.
//!
//! A note is pushed onto the grid the instant its onset is detected (at
//! minimum length) rather than only once it's released, and its length is
//! grown every frame while it's held — so the player watches the note
//! appear and extend in real time, the same way any other DAW's live
//! recording works, instead of a note only appearing once they stop
//! playing it.

use std::collections::HashMap;

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::audio_system::pitch_detect::PitchEvent;
use crate::settings::AudioSettings;
use crate::song::harmonica::Harmonica;

use super::TICKS_PER_BEAT;
use super::midi_import::map_pitch;
use super::playback::{EditorAudio, Playhead, build_harp};
use super::state::{EditorState, Expr, GridNote, HarmonicaKind};

// ── State ─────────────────────────────────────────────────────────────────────

/// One note currently sounding: the id of the (already-pushed, still
/// growing) [`GridNote`] it became at onset, and the elapsed time its
/// onset was detected — needed every frame to recompute the note's length
/// as it grows.
struct OpenNote {
    id: u32,
    start_secs: f32,
}

#[derive(Resource, Default)]
pub(super) struct RecordState {
    pub(super) active: bool,
    /// Notes started so far this take — shown in the status bar so
    /// there's some live feedback that something is actually being
    /// captured. Counted at onset, not release, so it climbs the instant a
    /// note starts rather than lagging a beat behind what's visibly on the
    /// grid.
    pub(super) note_count: u32,
    /// MIDI pitches currently sounding, keyed by pitch.
    open: HashMap<u8, OpenNote>,
}

impl RecordState {
    fn reset(&mut self) {
        *self = RecordState::default();
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Starts recording: resets any prior take's state, and (re)starts the
/// shared [`Playhead`] clock with an effectively unbounded `total` — unlike
/// Play/Practice, which stop once the chart's own notes run out, a
/// recording take has no natural end until the player stops it. Reusing
/// `Playhead` this way also means `PlayheadLine`'s existing moving cursor
/// gives live visual feedback of where new notes are landing, with no new
/// plumbing. Also plays the chart's background music, if any, exactly as
/// Play and Practice do, so there's something to record along to.
pub(super) fn start_record(
    state: &EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing: &Query<Entity, With<EditorAudio>>,
    record: &mut RecordState,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    record.reset();
    record.active = true;

    let bpm = state.tempo.trim().parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    *playhead = Playhead {
        playing: true,
        paused: false,
        elapsed: 0.0,
        total: f32::MAX,
        secs_per_tick,
    };

    let music = state.music.trim();
    if !music.is_empty() {
        match std::fs::read(music) {
            Ok(bytes) => {
                let handle = sources.add(AudioSource {
                    bytes: bytes.into(),
                });
                commands.spawn((
                    EditorAudio,
                    AudioPlayer::<AudioSource>(handle),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.music_volume)),
                ));
            }
            Err(e) => warn!("Song editor: couldn't read background music {music:?}: {e}"),
        }
    }
}

/// Stops recording: grows any still-sounding notes one last time to reflect
/// the exact moment Stop was clicked — a held note shouldn't freeze one
/// frame short of wherever the player actually released it — then halts
/// the shared clock. A no-op (beyond the harmless despawn/halt) when
/// nothing was actually recording, so callers (the Stop button, switching
/// out of Perform mode) can call it unconditionally alongside
/// `stop_practice`.
pub(super) fn stop_record(
    state: &mut EditorState,
    playing: &Query<Entity, With<EditorAudio>>,
    record: &mut RecordState,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    if record.active {
        grow_open_notes(&mut state.notes, &record.open, playhead.elapsed, playhead.secs_per_tick);
        record.open.clear();
    }
    for e in playing {
        commands.entity(e).despawn();
    }
    playhead.playing = false;
    playhead.paused = false;
    record.active = false;
}

// ── System ────────────────────────────────────────────────────────────────────

/// Diffs each newly-arrived [`PitchEvent`] against the currently-open
/// notes to find onsets/releases (a MIDI pitch not already open starts a
/// new note, pushed at minimum length; an open pitch no longer present has
/// just ended), then — every frame, regardless of whether a new pitch
/// chunk arrived — grows every still-open note to the current elapsed
/// time, so the player watches each note extend in real time while held
/// rather than only seeing it appear once they release it.
pub(super) fn record_tick(
    playhead: Res<Playhead>,
    mut pitch_events: MessageReader<PitchEvent>,
    mut record: ResMut<RecordState>,
    mut state: ResMut<EditorState>,
) {
    if !record.active {
        // Drain unread pitch events so they don't pile up while idle.
        for _ in pitch_events.read() {}
        return;
    }

    let elapsed = playhead.elapsed;
    let secs_per_tick = playhead.secs_per_tick;

    // Freshest event wins — pitch events arrive at the audio pipeline's
    // chunk rate (~10 Hz), not the frame rate.
    let mut detected: Option<Vec<u8>> = None;
    for ev in pitch_events.read() {
        detected = Some(ev.0.iter().map(|p| p.midi).collect());
    }

    if let Some(detected) = detected {
        record.open.retain(|midi, _| detected.contains(midi));

        let harp = build_harp(&state.key, state.harmonica_kind);
        for midi in detected {
            if record.open.contains_key(&midi) {
                continue;
            }
            let id = state.next_id;
            state.next_id += 1;
            let note = spawn_open_note(id, midi, elapsed, secs_per_tick, &harp, state.harmonica_kind);
            state.notes.push(note);
            record.open.insert(midi, OpenNote { id, start_secs: elapsed });
            record.note_count += 1;
        }
    }

    grow_open_notes(&mut state.notes, &record.open, elapsed, secs_per_tick);
}

// ── Pure-ish helpers ─────────────────────────────────────────────────────────

/// Resolves a fresh onset onto `harp` and places it on the tick grid at
/// minimum length — the live-recording counterpart of
/// `midi_import::import_track_notes`'s per-note step, sharing the same
/// `map_pitch` resolution. Length is filled in afterward, every frame, by
/// [`grow_open_notes`].
fn spawn_open_note(
    id: u32,
    midi: u8,
    start_secs: f32,
    secs_per_tick: f32,
    harp: &Harmonica,
    kind: HarmonicaKind,
) -> GridNote {
    let (hole, dir, pitch) = map_pitch(midi, harp, kind);
    let tick = (start_secs / secs_per_tick).round() as usize;
    GridNote {
        id,
        hole,
        tick,
        len: 1,
        dir,
        pitch,
        expr: Expr::None,
    }
}

/// Extends every currently-open note's length to reflect `elapsed` —
/// called every frame while notes are held (so they visibly grow in real
/// time) and once more when recording stops (so a note doesn't freeze one
/// frame short of wherever the player actually released it).
fn grow_open_notes(notes: &mut [GridNote], open: &HashMap<u8, OpenNote>, elapsed: f32, secs_per_tick: f32) {
    for o in open.values() {
        if let Some(n) = notes.iter_mut().find(|n| n.id == o.id) {
            n.len = note_len(o.start_secs, elapsed, secs_per_tick);
        }
    }
}

fn note_len(start_secs: f32, end_secs: f32, secs_per_tick: f32) -> usize {
    (((end_secs - start_secs) / secs_per_tick).round() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::Pitch;
    use crate::song::harmonica::richter_harp;

    // ── grow_open_notes ──────────────────────────────────────────────────────

    fn note(id: u32, tick: usize, len: usize) -> GridNote {
        GridNote {
            id,
            hole: 1,
            tick,
            len,
            dir: super::super::state::Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        }
    }

    #[test]
    fn grow_open_notes_extends_the_matching_note_to_the_new_elapsed_time() {
        let mut notes = vec![note(0, 2, 1)];
        let open: HashMap<u8, OpenNote> = [(60, OpenNote { id: 0, start_secs: 0.25 })].into();
        // 120 BPM -> secs_per_tick = 0.125s; held from 0.25s to 0.75s = 4 ticks.
        grow_open_notes(&mut notes, &open, 0.75, 0.125);
        assert_eq!(notes[0].len, 4);
    }

    #[test]
    fn grow_open_notes_never_shrinks_below_one_tick() {
        let mut notes = vec![note(0, 2, 1)];
        let open: HashMap<u8, OpenNote> = [(60, OpenNote { id: 0, start_secs: 0.25 })].into();
        // elapsed hasn't advanced past the onset yet — still a fresh blip.
        grow_open_notes(&mut notes, &open, 0.25, 0.125);
        assert_eq!(notes[0].len, 1);
    }

    #[test]
    fn grow_open_notes_leaves_notes_not_in_open_untouched() {
        let mut notes = vec![note(0, 2, 3)];
        let open: HashMap<u8, OpenNote> = HashMap::new();
        grow_open_notes(&mut notes, &open, 5.0, 0.125);
        assert_eq!(notes[0].len, 3);
    }

    // ── spawn_open_note ──────────────────────────────────────────────────────

    #[test]
    fn spawn_open_note_places_the_tick_and_starts_at_minimum_length() {
        let harp = richter_harp("C");
        let secs_per_tick = 60.0 / 120.0 / TICKS_PER_BEAT as f32;
        let n = spawn_open_note(7, 60, 0.25, secs_per_tick, &harp, HarmonicaKind::Diatonic);
        assert_eq!(n.id, 7);
        assert_eq!(n.tick, 2); // 0.25 / 0.125
        assert_eq!(n.len, 1);
    }

    #[test]
    fn spawn_open_note_resolves_pitch_via_map_pitch_including_bends() {
        let harp = richter_harp("C");
        // A rounded-to-semitone bend target (draw-2's reed minus a half
        // step) must resolve to a Bend, exactly like `map_pitch` already
        // does for MIDI import — recording a bent note shouldn't just snap
        // to the nearest natural note.
        let draw2 = harp.wind_direction_midi(2, &crate::song::chart::Action::Draw).unwrap();
        let bent_target = draw2 - 1;
        let n = spawn_open_note(0, bent_target, 0.0, 0.125, &harp, HarmonicaKind::Diatonic);
        assert_eq!(n.hole, 2);
        assert!(matches!(n.pitch, Pitch::Bend(_)));
    }
}
