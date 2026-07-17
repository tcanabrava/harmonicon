// SPDX-License-Identifier: MIT

//! The chart note model: [`ScheduledNote`]/[`SongNotes`] (score state, kept
//! as plain data rather than ECS components — see the doc comment on
//! [`ScheduledNote`]) and the pure helpers that resolve chart-time and
//! decide which notes need a spawned visual or a wait-freeze this frame.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::audio_system::midi::note_to_midi;
use crate::song::chart::Modifier;

pub const LOOKAHEAD: f64 = 3.0;

#[derive(Component)]
pub struct NoteVisual {
    /// Index into [`SongNotes::notes`] — this entity is purely a rendering
    /// of that note's score state, spawned only while it's within
    /// [`LOOKAHEAD`] of the playhead (see `gameplay_2d::spawn_visible_notes`)
    /// and despawned once scrolled past, independent of the note's actual
    /// score state (which lives on regardless of whether anything currently
    /// renders it).
    pub note_id: usize,
}

/// One chart note's score state. Plain data, not an ECS component — lives in
/// [`SongNotes`], independent of whatever render entity (if any) currently
/// represents it on screen. That split is what lets `gameplay_2d`/
/// `gameplay_3d` spawn note *visuals* only for a [`LOOKAHEAD`] window around
/// the playhead instead of the whole song up front, and lets
/// `clock::handle_loop_boundary` reset a note's state without needing it to
/// have a live entity at all.
#[derive(Clone)]
pub struct ScheduledNote {
    pub time: f64,
    /// Note length in seconds; long notes reward sustaining the pitch.
    pub duration: f64,
    pub hole: u8,
    pub is_blow: bool,
    /// The MIDI note number this note expects, pre-computed at spawn
    /// (`None` for a hole/action/bend combination the harp can't actually
    /// produce — see [`target_pitch`] — which can never be hit).
    pub expected_pitch: Option<u8>,
    pub hit: bool,
    pub missed: bool,
    /// Seconds the expected pitch has been held since the onset was hit.
    pub held: f64,
    /// Set once the sustain window has closed and its bonus was awarded.
    pub sustain_scored: bool,
    /// Technique modifiers from the chart (bend, vibrato, etc.).
    /// Used to trigger fx sounds when the note is hit.
    pub modifiers: Vec<Modifier>,
    /// `(clock time, cents-from-expected-pitch)`, sampled once per frame
    /// while held — used to verify a declared `vibrato` was actually played
    /// at roughly its declared `oscillation_hz`, not just declared. Storing
    /// the timestamp (rather than trusting sample order) keeps the measured
    /// rate frame-rate independent.
    pub pitch_samples: Vec<(f64, f32)>,
    /// `(clock time, input loudness RMS)`, sampled once per frame while
    /// held — used to verify a declared `wah-wah` was actually played at
    /// roughly its declared `oscillation_hz`, not just declared.
    pub amp_samples: Vec<(f64, f32)>,
    /// Index into `adaptive_difficulty::AdaptiveDifficulty::sections` — the
    /// musical phrase this note belongs to. A note only exists in
    /// `SongNotes` at all once adaptive difficulty has unlocked it (see
    /// `adaptive_difficulty::unlocked_flags`), so this is always a real
    /// section, not an `Option`; charts with no `phrase` tags get a single
    /// implicit section (index 0) covering the whole track.
    pub phrase_section: usize,
    /// The full set of expected MIDI pitches for this note's chart
    /// `TrackItem`, shared identically by every sibling `ScheduledNote` the
    /// item produced (one per `NoteEvent` — see `gameplay_2d::
    /// build_combined_notes`/`gameplay_3d::build_notes_3d`). Empty for an
    /// ordinary single-event item, which is the signal `judge::score_notes`
    /// uses to skip the simultaneity check entirely — nothing about
    /// single-note charts changes. Non-empty (a `PlayMode::Chord`/`Split`
    /// item — two or more `events` at the same `time`) means this note's
    /// own onset only counts as "playing" while *every* pitch in the set
    /// sounds together, not just its own — the chord-target primitive
    /// `docs/lessons_plan.md` calls for, built on the chart format's
    /// existing multi-event `TrackItem` shape rather than a new schema
    /// field.
    pub chord_pitches: Vec<u8>,
    /// From the chart's `TrackItem::call` — this note is the "response" half
    /// of a call-and-response phrase. `clock::tick_clock`'s wait-freeze
    /// condition treats it like `WaitForNoteMode` being on, regardless of
    /// whether the player actually has that practice toggle enabled:
    /// freezing here isn't optional the way it is for an ordinary note,
    /// since the whole drill is "echo what you just heard, in your own
    /// time." See `gameplay::call_response`.
    pub force_wait: bool,
}

/// Every note in the loaded chart, sorted by `time` ascending (matches chart
/// authoring order; nothing re-sorts `chart.track` elsewhere either). The
/// scoring systems (`judge::score_notes`, `clock::handle_loop_boundary`,
/// `judge::update_active_targets`) read and mutate this directly instead of
/// querying ECS components, so a note's score state exists independent of
/// whether it currently has a render entity.
#[derive(Resource, Default)]
pub struct SongNotes {
    pub notes: Vec<ScheduledNote>,
    /// Index of the first not-fully-resolved note (not `missed`, and not
    /// both `hit` and `sustain_scored`). Advanced forward by `judge::
    /// score_notes` as a prefix of notes finishes for good; rewound by
    /// `clock::handle_loop_boundary` on a loop wrap, since notes before the
    /// loop's start are no longer "permanently done" once it can replay
    /// them. Purely a per-frame scan-avoidance optimization — correctness
    /// never depends on its exact value, only that it's `<=` the true first
    /// unresolved index.
    pub cursor: usize,
}

/// Indices of notes that should have a spawned visual at `elapsed` but don't
/// yet (per `already_spawned`) — the windowing logic shared by
/// `gameplay_2d::spawn_visible_notes` and `gameplay_3d::spawn_visible_notes_3d`.
/// `notes` must be sorted by `time` ascending (as `SongNotes::notes` always
/// is). A note's window is open from [`LOOKAHEAD`] seconds before its `time`
/// until `elapsed` passes it (recycling/despawning is each mode's own
/// concern, based on how far the note has visually scrolled — this only
/// decides when a *new* visual should appear).
pub(crate) fn notes_needing_spawn(
    notes: &[ScheduledNote],
    already_spawned: &HashSet<usize>,
    elapsed: f64,
) -> Vec<usize> {
    // Sorted by time, so this is the first index whose window could
    // possibly be open — no need to consider anything before it.
    let start = notes.partition_point(|n| n.time + LOOKAHEAD < elapsed);
    let mut result = Vec::new();
    for (i, note) in notes.iter().enumerate().skip(start) {
        if note.time - LOOKAHEAD > elapsed {
            break; // sorted — nothing further out needs spawning yet either.
        }
        if !already_spawned.contains(&i) {
            result.push(i);
        }
    }
    result
}

/// Index of the first not-yet-resolved, *playable* note in `notes[cursor..]`
/// that has already reached `clock_time` — the freeze condition for
/// `pause_menu::WaitForNoteMode`. `clock::tick_clock` uses the index both to
/// decide whether to freeze and to label the wait-freeze prompt with which
/// note it's waiting on. `notes` sorted by `time` (as `SongNotes::notes`
/// always is) lets this stop scanning as soon as it reaches a note that
/// isn't due yet, same as `judge::score_notes`.
///
/// Notes with no `expected_pitch` (a hole/action the harp can't produce —
/// see `target_pitch`) are excluded: they can never be hit, so freezing on
/// one would wait forever.
pub(crate) fn first_due_unresolved_note(
    notes: &[ScheduledNote],
    cursor: usize,
    clock_time: f64,
) -> Option<usize> {
    for (i, note) in notes.iter().enumerate().skip(cursor) {
        if note.time > clock_time {
            break;
        }
        if note.expected_pitch.is_some() && !note.hit && !note.missed {
            return Some(i);
        }
    }
    None
}

/// The wait-freeze index `clock::tick_clock` should hold the clock at this
/// frame, or `None` to run normally: the first due unresolved playable note
/// ([`first_due_unresolved_note`]), but only kept when the player's global
/// `wait_mode` toggle is on, or the note itself demands it regardless
/// (`ScheduledNote::force_wait` — the response half of a call-and-response
/// phrase always freezes, whether or not the player has that practice
/// toggle on).
pub(crate) fn wait_freeze_index(
    notes: &[ScheduledNote],
    cursor: usize,
    clock_time: f64,
    wait_mode: bool,
) -> Option<usize> {
    first_due_unresolved_note(notes, cursor, clock_time).filter(|&i| wait_mode || notes[i].force_wait)
}

/// Index range (into `notes`, sorted by `time`) that a loop wrap must reset
/// `hit`/`missed`/`held`/`sustain_scored` for: `start_time..end_time`,
/// extended by [`LOOKAHEAD`] past `end_time` since [`notes_needing_spawn`]
/// can preview a note that far ahead of the clock before the loop ever
/// actually reaches it.
pub(crate) fn loop_reset_range(
    notes: &[ScheduledNote],
    start_time: f64,
    end_time: f64,
) -> (usize, usize) {
    let start_idx = notes.partition_point(|n| n.time < start_time);
    let end_idx = notes.partition_point(|n| n.time <= end_time + LOOKAHEAD);
    (start_idx, end_idx)
}

/// Resolve a track item's start time in seconds, preferring an explicit `time`
/// and falling back to converting its `tick` through the tempo map.
pub fn resolve_item_time(
    item: &crate::song::chart::TrackItem,
    timing: &crate::song::chart::Timing,
) -> f64 {
    item.time.unwrap_or_else(|| {
        let tick = item.tick.unwrap_or(0);
        crate::song::chart::tick_to_seconds(tick, timing.resolution, &timing.tempo_map)
    })
}

/// The latest moment any note finishes (start + duration) across the track, in
/// seconds. Drives when the song's content ends. Zero for an empty track.
pub fn last_note_end(
    track: &[crate::song::chart::TrackItem],
    timing: &crate::song::chart::Timing,
) -> f64 {
    track
        .iter()
        .map(|item| resolve_item_time(item, timing) + item.duration)
        .fold(0.0_f64, f64::max)
}

/// The MIDI note the player must actually produce for a note. A `bend`
/// shifts the note's natural pitch by its semitones (negative = down, and
/// rounded to the nearest whole semitone — the actual bent pitch is
/// continuous, but the matched target is discrete), so the bend is
/// *validated* by scoring — playing the unbent note no longer counts.
/// `None` if `natural` isn't a parseable note name (e.g. the "—" placeholder
/// for a hole/direction the harp can't produce) or the shifted result falls
/// outside the valid MIDI range.
pub fn target_pitch(natural: &str, modifiers: &[Modifier]) -> Option<u8> {
    let bend: i32 = modifiers
        .iter()
        .find_map(|m| match m {
            Modifier::Bend { semitones, .. } => Some(semitones.round() as i32),
            _ => None,
        })
        .unwrap_or(0);
    let midi = note_to_midi(natural)? + bend;
    (0..=127).contains(&midi).then_some(midi as u8)
}
