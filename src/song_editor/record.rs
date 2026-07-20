// SPDX-License-Identifier: MIT

//! Records notes played live on a real harmonica straight onto the note
//! grid — the Song Editor's live counterpart to `midi_import`, sharing that
//! module's pitch resolution instead of reading MIDI file bytes. The
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
//!
//! Unlike gameplay scoring — which tolerates raw detector noise because it
//! only compares detections against the chart's expected notes — recording
//! has no chart to lean on, so it defends itself three ways, all set up
//! once at [`start_record`] rather than per event:
//! - [`PitchRange`] is narrowed to the selected harp's own range (the same
//!   thing gameplay does from a loaded chart), so the detector never even
//!   searches where this harp can't sound;
//! - a precomputed 128-entry MIDI→(hole, dir, pitch) table
//!   ([`build_pitch_table`], backed by `map_pitch_playable`) resolves each
//!   detection, *discarding* pitches the harp can't produce instead of
//!   letting MIDI import's nearest-note fallback disguise noise as a
//!   plausible hole;
//! - onsets are provisional until seen in [`CONFIRM_EVENTS`] consecutive
//!   pitch events (a one-chunk blip is deleted again, so the live-growing
//!   UX costs nothing), and releases get [`RELEASE_GRACE_EVENTS`] events of
//!   grace (a one-chunk dropout doesn't split a held note in two).

use std::collections::HashMap;

use bevy::audio::AudioSource;
use bevy::prelude::*;

use crate::audio_system::audio_input::{AudioCapture, CHUNK_SIZE};
use crate::audio_system::pitch_detect::{PITCH_RANGE_MARGIN_SEMITONES, PitchEvent, PitchRange};
use crate::settings::AudioSettings;
use crate::song::harmonica::Harmonica;

#[cfg(test)]
use super::TICKS_PER_BEAT;
use super::pitch_map::map_pitch_playable;
use super::playback::{EditorAudio, Playhead, build_harp, secs_per_tick, spawn_background_music};
use super::state::{Dir, EditorState, Expr, GridNote, HarmonicaKind, Pitch};

// ── Tuning ────────────────────────────────────────────────────────────────────

/// How many consecutive pitch events (one per ~46 ms audio chunk) a pitch
/// must appear in before its note is considered real. The note is pushed at
/// the *first* event (live feedback shouldn't wait), but if it dies before
/// reaching this count it's deleted again — a single noisy chunk shouldn't
/// leave a permanent blip on the grid.
const CONFIRM_EVENTS: u32 = 2;

/// How many consecutive pitch events a currently-open pitch may go missing
/// for before its note is released. One dropout chunk mid-sustain would
/// otherwise split a held note into two (release + immediate re-onset).
const RELEASE_GRACE_EVENTS: u8 = 2;

// ── State ─────────────────────────────────────────────────────────────────────

/// One note currently sounding: the id of the (already-pushed, still
/// growing) [`GridNote`] it became at onset, the elapsed time its onset was
/// detected — needed every frame to recompute the note's length as it
/// grows — and the confirm/release-debounce counters (see [`CONFIRM_EVENTS`]
/// / [`RELEASE_GRACE_EVENTS`]).
struct OpenNote {
    id: u32,
    start_secs: f32,
    /// Pitch events this pitch has been present in so far.
    events_seen: u32,
    /// Consecutive pitch events this pitch has been *missing* from — reset
    /// to zero whenever it reappears within the grace window.
    missed_events: u8,
}

#[derive(Resource, Default)]
pub(super) struct RecordState {
    pub(super) active: bool,
    /// Notes started so far this take — shown in the status bar so
    /// there's some live feedback that something is actually being
    /// captured. Counted at onset, not release, so it climbs the instant a
    /// note starts rather than lagging a beat behind what's visibly on the
    /// grid (and drops back down if an unconfirmed blip gets deleted).
    pub(super) note_count: u32,
    /// MIDI pitches currently sounding, keyed by pitch.
    open: HashMap<u8, OpenNote>,
    /// MIDI → (hole, dir, pitch) for every pitch the selected harp can
    /// produce, `None` for everything it can't — precomputed once at
    /// [`start_record`] so `record_tick` is a table lookup instead of a
    /// per-event harp rebuild, and so non-producible detections are
    /// discarded as noise.
    table: Vec<Option<(u8, Dir, Pitch)>>,
    /// Seconds the detection pipeline lags behind the sound itself — half
    /// the analysis window plus the player's calibrated input latency —
    /// subtracted from the clock when placing onsets so recorded notes
    /// don't land systematically late. Cached here (refreshed each
    /// `record_tick`) so `stop_record`'s final grow uses the same offset.
    detect_delay: f32,
}

impl RecordState {
    fn reset(&mut self) {
        *self = RecordState::default();
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Starts recording: resets any prior take's state, precomputes the
/// harp-derived pitch table and narrows [`PitchRange`] to the harp (see the
/// module docs), and (re)starts the shared [`Playhead`] clock with an
/// effectively unbounded `total` — unlike Play/Practice, which stop once
/// the chart's own notes run out, a recording take has no natural end until
/// the player stops it. Reusing `Playhead` this way also means
/// `PlayheadLine`'s existing moving cursor gives live visual feedback of
/// where new notes are landing, with no new plumbing. Also plays the
/// chart's background music, if any, exactly as Play and Practice do, so
/// there's something to record along to.
pub(super) fn start_record(
    state: &EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing: &Query<Entity, With<EditorAudio>>,
    record: &mut RecordState,
    playhead: &mut Playhead,
    pitch_range: &mut PitchRange,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    record.reset();
    record.active = true;

    let harp = build_harp(&state.key, state.harmonica_kind);
    record.table = build_pitch_table(&harp, state.harmonica_kind);
    // Same harp-sized narrowing gameplay applies from a loaded chart
    // (`gameplay::lifecycle::setup_scoring_config`) — fewer candidates for
    // every detection algorithm. Restored to the default by `stop_record`.
    *pitch_range = harp
        .frequency_range()
        .map(|(lo, hi)| PitchRange::from_freqs([lo, hi], PITCH_RANGE_MARGIN_SEMITONES))
        .unwrap_or_default();

    *playhead = Playhead {
        playing: true,
        paused: false,
        elapsed: 0.0,
        total: f32::MAX,
        secs_per_tick: secs_per_tick(state),
    };

    spawn_background_music(state, sources, settings, commands);
}

/// Stops recording: closes out every still-sounding note at the exact
/// moment Stop was clicked — growing confirmed ones one last time (a held
/// note shouldn't freeze one frame short of wherever the player actually
/// released it) and deleting unconfirmed blips, same as a mid-take release
/// would — then halts the shared clock and restores the default
/// [`PitchRange`]. A no-op (beyond the harmless despawn/halt) when nothing
/// was actually recording, so callers (the Stop button, switching out of
/// Perform mode) can call it unconditionally alongside `stop_practice`.
pub(super) fn stop_record(
    state: &mut EditorState,
    playing: &Query<Entity, With<EditorAudio>>,
    record: &mut RecordState,
    playhead: &mut Playhead,
    pitch_range: &mut PitchRange,
    commands: &mut Commands,
) {
    if record.active {
        let t = (playhead.elapsed - record.detect_delay).max(0.0);
        finish_open_notes(record, &mut state.notes, t, playhead.secs_per_tick);
        *pitch_range = PitchRange::default();
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
/// notes to find onsets/releases (with the confirm/grace debouncing the
/// module docs describe), then — every frame, regardless of whether a new
/// pitch chunk arrived — grows every still-sounding note to the current
/// elapsed time, so the player watches each note extend in real time while
/// held rather than only seeing it appear once they release it.
pub(super) fn record_tick(
    playhead: Res<Playhead>,
    capture: Option<Res<AudioCapture>>,
    settings: Res<AudioSettings>,
    mut pitch_events: MessageReader<PitchEvent>,
    mut record: ResMut<RecordState>,
    mut state: ResMut<EditorState>,
) {
    if !record.active {
        // Drain unread pitch events so they don't pile up while idle.
        for _ in pitch_events.read() {}
        return;
    }

    // A detection describes audio from a ~93 ms window that *ended* some
    // queueing delay ago — on average the sound happened about half a
    // window before now — plus whatever input latency the player has
    // calibrated (the same `input_latency_ms` gameplay's judge subtracts
    // from its clock). Without this, every recorded note lands
    // systematically late on the grid.
    let half_window = capture
        .map(|c| CHUNK_SIZE as f32 * 0.5 / c.sample_rate.max(1) as f32)
        .unwrap_or(0.0);
    record.detect_delay = half_window + settings.input_latency_ms as f32 / 1000.0;
    let t = (playhead.elapsed - record.detect_delay).max(0.0);
    let secs_per_tick = playhead.secs_per_tick;

    // Pitch events arrive at the audio pipeline's chunk rate (~21 Hz), not
    // the frame rate — each one is a real detector verdict, so each drives
    // the onset/release debounce counters.
    for ev in pitch_events.read() {
        let detected: Vec<u8> = ev.0.iter().map(|p| p.midi).collect();
        apply_detected_pitches(&mut record, &mut state, &detected, t, secs_per_tick);
    }

    grow_open_notes(&mut state.notes, &record.open, t, secs_per_tick);
}

// ── Pure-ish helpers ─────────────────────────────────────────────────────────

/// MIDI → (hole, dir, pitch) for all 128 MIDI notes on `harp`, `None` where
/// the harp can't produce that pitch (exactly `map_pitch_playable`'s
/// verdict, precomputed) — see [`RecordState::table`].
fn build_pitch_table(harp: &Harmonica, kind: HarmonicaKind) -> Vec<Option<(u8, Dir, Pitch)>> {
    (0..=127u8)
        .map(|midi| map_pitch_playable(midi, harp, kind))
        .collect()
}

/// One pitch event's worth of onset/release bookkeeping — the debounced
/// core of [`record_tick`], kept free of ECS types so the confirm/grace
/// rules are directly testable. `t` is the (latency-compensated) elapsed
/// time this event is credited to.
fn apply_detected_pitches(
    record: &mut RecordState,
    state: &mut EditorState,
    detected: &[u8],
    t: f32,
    secs_per_tick: f32,
) {
    // Releases first: a missing pitch burns one grace event; only past the
    // grace window does its note actually close — and an unconfirmed one
    // (never seen `CONFIRM_EVENTS` times) is deleted as a blip.
    let mut closed: Vec<u8> = Vec::new();
    for (&midi, open) in record.open.iter_mut() {
        if detected.contains(&midi) {
            open.events_seen += 1;
            open.missed_events = 0;
        } else {
            open.missed_events += 1;
            if open.missed_events >= RELEASE_GRACE_EVENTS {
                closed.push(midi);
            }
        }
    }
    for midi in closed {
        let Some(open) = record.open.remove(&midi) else {
            continue;
        };
        if open.events_seen < CONFIRM_EVENTS {
            state.notes.retain(|n| n.id != open.id);
            record.note_count = record.note_count.saturating_sub(1);
        }
    }

    // Onsets: a detected pitch not already open starts a new (provisional)
    // note — if the precomputed table says this harp can produce it at all.
    for &midi in detected {
        if record.open.contains_key(&midi) {
            continue;
        }
        let Some(mapped) = record.table.get(midi as usize).copied().flatten() else {
            continue;
        };
        let id = state.next_id;
        state.next_id += 1;
        state.notes.push(spawn_open_note(id, mapped, t, secs_per_tick));
        record.open.insert(
            midi,
            OpenNote {
                id,
                start_secs: t,
                events_seen: 1,
                missed_events: 0,
            },
        );
        record.note_count += 1;
    }
}

/// Closes out every open note at once — [`stop_record`]'s counterpart to a
/// mid-take release: confirmed notes get their final length, unconfirmed
/// blips are deleted.
fn finish_open_notes(
    record: &mut RecordState,
    notes: &mut Vec<GridNote>,
    t: f32,
    secs_per_tick: f32,
) {
    for (_, open) in record.open.drain() {
        if open.events_seen < CONFIRM_EVENTS {
            notes.retain(|n| n.id != open.id);
            record.note_count = record.note_count.saturating_sub(1);
        } else if let Some(n) = notes.iter_mut().find(|n| n.id == open.id) {
            n.len = note_len(open.start_secs, t, secs_per_tick);
        }
    }
}

/// Places a fresh (already table-resolved) onset on the tick grid at
/// minimum length — the live-recording counterpart of
/// `midi_import::import_track_notes`'s per-note step. Length is filled in
/// afterward, every frame, by [`grow_open_notes`].
fn spawn_open_note(
    id: u32,
    (hole, dir, pitch): (u8, Dir, Pitch),
    start_secs: f32,
    secs_per_tick: f32,
) -> GridNote {
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

/// Extends every currently-sounding note's length to reflect `t` — called
/// every frame while notes are held (so they visibly grow in real time).
/// A note inside its release-grace window (missing from the latest
/// detection but not yet closed) is left frozen instead: if the pitch
/// comes back the next growth absorbs the gap seamlessly, and if it
/// doesn't, the note keeps the length it had when it actually stopped
/// sounding rather than the grace window's extra chunks.
fn grow_open_notes(
    notes: &mut [GridNote],
    open: &HashMap<u8, OpenNote>,
    t: f32,
    secs_per_tick: f32,
) {
    for o in open.values() {
        if o.missed_events > 0 {
            continue;
        }
        if let Some(n) = notes.iter_mut().find(|n| n.id == o.id) {
            n.len = note_len(o.start_secs, t, secs_per_tick);
        }
    }
}

fn note_len(start_secs: f32, end_secs: f32, secs_per_tick: f32) -> usize {
    (((end_secs - start_secs) / secs_per_tick).round() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::harmonica::richter_harp;

    fn open(id: u32, start_secs: f32) -> OpenNote {
        OpenNote {
            id,
            start_secs,
            events_seen: CONFIRM_EVENTS,
            missed_events: 0,
        }
    }

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
        let open: HashMap<u8, OpenNote> = [(60, open(0, 0.25))].into();
        // 120 BPM -> secs_per_tick = 0.125s; held from 0.25s to 0.75s = 4 ticks.
        grow_open_notes(&mut notes, &open, 0.75, 0.125);
        assert_eq!(notes[0].len, 4);
    }

    #[test]
    fn grow_open_notes_never_shrinks_below_one_tick() {
        let mut notes = vec![note(0, 2, 1)];
        let open: HashMap<u8, OpenNote> = [(60, open(0, 0.25))].into();
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

    #[test]
    fn grow_open_notes_freezes_a_note_inside_its_release_grace_window() {
        let mut notes = vec![note(0, 2, 3)];
        let mut o = open(0, 0.25);
        o.missed_events = 1;
        let open: HashMap<u8, OpenNote> = [(60, o)].into();
        grow_open_notes(&mut notes, &open, 5.0, 0.125);
        assert_eq!(notes[0].len, 3);
    }

    // ── build_pitch_table / spawn_open_note ──────────────────────────────────

    #[test]
    fn build_pitch_table_resolves_playable_pitches_and_rejects_the_rest() {
        let harp = richter_harp("C");
        let table = build_pitch_table(&harp, HarmonicaKind::Diatonic);
        assert_eq!(table.len(), 128);
        // C4 is hole 1 blow on a C richter harp.
        let c4 = crate::audio_system::midi::note_to_midi("C4").unwrap() as usize;
        assert_eq!(table[c4], Some((1, Dir::Blow, Pitch::Normal)));
        // MIDI 0 is far below anything a C harp can sound.
        assert_eq!(table[0], None);
    }

    #[test]
    fn build_pitch_table_resolves_a_bend_to_the_bend_not_the_nearest_natural() {
        let harp = richter_harp("C");
        // A rounded-to-semitone bend target (draw-2's reed minus a half
        // step) must resolve to a Bend, exactly like MIDI import already
        // does — recording a bent note shouldn't just snap to the nearest
        // natural note.
        let draw2 = harp.wind_direction_midi(2, &crate::song::chart::Action::Draw).unwrap();
        let table = build_pitch_table(&harp, HarmonicaKind::Diatonic);
        match table[(draw2 - 1) as usize] {
            Some((2, Dir::Draw, Pitch::Bend(_))) => {}
            other => panic!("expected a hole-2 draw bend, got {other:?}"),
        }
    }

    #[test]
    fn spawn_open_note_places_the_tick_and_starts_at_minimum_length() {
        let secs_per_tick = 60.0 / 120.0 / TICKS_PER_BEAT as f32;
        let n = spawn_open_note(7, (1, Dir::Blow, Pitch::Normal), 0.25, secs_per_tick);
        assert_eq!(n.id, 7);
        assert_eq!(n.tick, 2); // 0.25 / 0.125
        assert_eq!(n.len, 1);
    }

    // ── apply_detected_pitches (confirm / grace debouncing) ──────────────────

    /// A ready-to-record state for a C diatonic harp, plus the MIDI number
    /// of its hole-1 blow (C4) — a pitch the table certainly resolves.
    fn recording_setup() -> (RecordState, EditorState, u8) {
        let record = RecordState {
            active: true,
            table: build_pitch_table(&richter_harp("C"), HarmonicaKind::Diatonic),
            ..RecordState::default()
        };
        let c4 = crate::audio_system::midi::note_to_midi("C4").unwrap() as u8;
        (record, EditorState::default(), c4)
    }

    #[test]
    fn an_onset_pushes_a_note_immediately() {
        let (mut record, mut state, c4) = recording_setup();
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.0, 0.125);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(record.note_count, 1);
    }

    #[test]
    fn a_detection_the_harp_cant_produce_is_discarded() {
        let (mut record, mut state, _) = recording_setup();
        apply_detected_pitches(&mut record, &mut state, &[0], 0.0, 0.125);
        assert!(state.notes.is_empty());
        assert_eq!(record.note_count, 0);
    }

    #[test]
    fn a_one_chunk_blip_is_deleted_after_the_grace_window() {
        let (mut record, mut state, c4) = recording_setup();
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.0, 0.125);
        for _ in 0..RELEASE_GRACE_EVENTS {
            apply_detected_pitches(&mut record, &mut state, &[], 0.1, 0.125);
        }
        assert!(state.notes.is_empty(), "unconfirmed blip should be deleted");
        assert_eq!(record.note_count, 0);
    }

    #[test]
    fn a_note_seen_twice_survives_its_release() {
        let (mut record, mut state, c4) = recording_setup();
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.0, 0.125);
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.05, 0.125);
        for _ in 0..RELEASE_GRACE_EVENTS {
            apply_detected_pitches(&mut record, &mut state, &[], 0.5, 0.125);
        }
        assert_eq!(state.notes.len(), 1, "confirmed note should be kept");
        assert_eq!(record.note_count, 1);
    }

    #[test]
    fn release_grace_keeps_a_held_note_through_one_dropout_chunk() {
        let (mut record, mut state, c4) = recording_setup();
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.0, 0.125);
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.05, 0.125);
        // One dropout chunk, then the pitch comes back.
        apply_detected_pitches(&mut record, &mut state, &[], 0.1, 0.125);
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.15, 0.125);
        assert_eq!(state.notes.len(), 1, "dropout must not split the note");
        assert_eq!(record.note_count, 1);
    }

    #[test]
    fn finish_open_notes_deletes_unconfirmed_and_finalizes_confirmed_lengths() {
        let (mut record, mut state, c4) = recording_setup();
        // A confirmed note (two events) and an unconfirmed blip (one event,
        // hole-2 draw = D4 on a C harp).
        let d4 = crate::audio_system::midi::note_to_midi("D4").unwrap() as u8;
        apply_detected_pitches(&mut record, &mut state, &[c4], 0.0, 0.125);
        apply_detected_pitches(&mut record, &mut state, &[c4, d4], 0.05, 0.125);
        assert_eq!(state.notes.len(), 2);

        finish_open_notes(&mut record, &mut state.notes, 0.5, 0.125);
        assert_eq!(state.notes.len(), 1, "the blip should be deleted at stop");
        assert_eq!(record.note_count, 1);
        assert_eq!(state.notes[0].len, 4); // held 0.0..0.5 at 0.125 s/tick
        assert!(record.open.is_empty());
    }
}
