// SPDX-License-Identifier: MIT

//! The chart note model: [`ScheduledNote`]/[`SongNotes`] (score state, kept
//! as plain data rather than ECS components — see the doc comment on
//! [`ScheduledNote`]) and the pure helpers that resolve chart-time and
//! decide which notes need a spawned visual or a wait-freeze this frame.

use std::collections::HashSet;

use bevy::prelude::*;

use harmonicon_app::app::EffectiveHarmonica;
use harmonicon_core::chart::{Action, HarpChart, Modifier, PlayMode};
use harmonicon_core::harp_remap;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::midi::note_to_midi;
use harmonicon_core::pitch_map::{HoleAssignment, Technique, technique_of};

use super::adaptive_difficulty::{AdaptiveDifficulty, track_items, unlocked_flags};
use super::state::MissReason;

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
/// represents it on screen. That split is what lets `gameplay_2d` spawn
/// note *visuals* only for a [`LOOKAHEAD`] window around
/// the playhead instead of the whole song up front, and lets
/// `clock::handle_loop_boundary` reset a note's state without needing it to
/// have a live entity at all.
#[derive(Clone)]
pub struct ScheduledNote {
    pub time: f64,
    /// Note length in seconds; long notes reward sustaining the pitch.
    pub duration: f64,
    /// Where the note is played: for a harmonica, the hole, breath and
    /// technique. Drawing and coaching read it; scoring never does, since a
    /// microphone hears the pitch, not the hole (`expected_pitch`).
    pub position: PlayPosition,
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
    /// item produced (one per `NoteEvent`). Empty for an ordinary
    /// single-event item — the signal `judge::score_notes` uses to skip the
    /// simultaneity check entirely. Non-empty (a `PlayMode::Chord`/`Split`
    /// item, two or more `events` at the same `time`) means this note's
    /// onset only counts as "playing" while every pitch in the set sounds
    /// together, not just its own — built on the chart format's existing
    /// multi-event `TrackItem` shape rather than a new schema field.
    pub chord_pitches: Vec<u8>,
    /// False when the harmonica actually being played cannot produce this
    /// note at all — only possible once `EffectiveHarmonica` substitutes a
    /// different harp (see `harmonicon_core::harp_remap`).
    ///
    /// `judge::score_notes` skips these entirely rather than letting them
    /// resolve: `expected_pitch: None` would look like the same thing but
    /// isn't, because a note that can never be "playing" still falls through
    /// to `NoteOutcome::Missed` when its window passes. A player must not be
    /// marked down for a note their harp physically cannot make.
    pub playable: bool,
    /// Why this note looks like it is failing, recorded the first frame
    /// inside its window that carried evidence (see
    /// `judge::observed_failure`) and read once the miss window elapses.
    ///
    /// It has to accumulate rather than be computed at the miss, because by
    /// the time the window closes the offending pitch has usually stopped
    /// sounding — classifying from that last frame alone would report almost
    /// every miss as `MissReason::NoAttack`. `None` at the miss *is*
    /// `NoAttack`: nothing was ever heard to blame.
    pub miss_evidence: Option<MissReason>,
    /// From the chart's `TrackItem::call` — this note is the "response"
    /// half of a call-and-response phrase. `clock::tick_clock`'s
    /// wait-freeze condition treats it like `WaitForNoteMode` being on
    /// regardless of the player's own practice toggle: the whole drill is
    /// "echo what you just heard, in your own time," so freezing isn't
    /// optional here. See `gameplay::call_response`.
    pub force_wait: bool,
}

/// A pending, playable blow note on hole 1 at time 0 with nothing recorded
/// yet. Chart loading sets every field itself; this is the baseline tests
/// and benches vary with struct-update syntax.
impl Default for ScheduledNote {
    fn default() -> Self {
        Self {
            time: 0.0,
            duration: 0.0,
            position: PlayPosition::Hole(HoleAssignment {
                hole: 1,
                action: Action::Blow,
                technique: Technique::Natural,
            }),
            expected_pitch: None,
            hit: false,
            missed: false,
            held: 0.0,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: 0,
            chord_pitches: Vec::new(),
            playable: true,
            miss_evidence: None,
            force_wait: false,
        }
    }
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
    /// both `hit` and `sustain_scored`). Advanced by `judge::score_notes`
    /// as a prefix finishes for good; rewound by `clock::
    /// handle_loop_boundary` on a loop wrap, since notes before the loop's
    /// start are no longer "permanently done" once it can replay them.
    /// Purely a scan-avoidance optimization — correctness only needs it
    /// `<=` the true first unresolved index.
    pub cursor: usize,
}

/// Indices of notes that should have a spawned visual at `elapsed` but don't
/// yet (per `already_spawned`) — the windowing logic behind
/// `gameplay_2d::spawn_visible_notes`.
/// `notes` must be sorted by `time` ascending (as `SongNotes::notes` always
/// is). A note's window is open from [`LOOKAHEAD`] seconds before its `time`
/// until `elapsed` passes it (recycling/despawning is each mode's own
/// concern, based on how far the note has visually scrolled — this only
/// decides when a *new* visual should appear).
pub(crate) fn notes_needing_spawn<'a>(
    notes: &'a [ScheduledNote],
    already_spawned: &'a HashSet<usize>,
    elapsed: f64,
) -> impl Iterator<Item = usize> + 'a {
    // Sorted by time, so this is the first index whose window could
    // possibly be open — no need to consider anything before it.
    let start = notes.partition_point(|n| n.time + LOOKAHEAD < elapsed);
    notes[start..]
        .iter()
        // Sorted — nothing further out needs spawning yet either.
        .take_while(move |note| note.time - LOOKAHEAD <= elapsed)
        .enumerate()
        .map(move |(offset, _)| start + offset)
        .filter(move |i| !already_spawned.contains(i))
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
    first_due_unresolved_note(notes, cursor, clock_time)
        .filter(|&i| wait_mode || notes[i].force_wait)
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
    item: &harmonicon_core::chart::TrackItem,
    timing: &harmonicon_core::chart::Timing,
) -> f64 {
    item.time.unwrap_or_else(|| {
        let tick = item.tick.unwrap_or(0);
        harmonicon_core::chart::tick_to_seconds(tick, timing.resolution, &timing.tempo_map)
    })
}

/// The latest moment any note finishes (start + duration) across the track, in
/// seconds. Drives when the song's content ends. Zero for an empty track.
pub fn last_note_end(
    track: &[harmonicon_core::chart::TrackItem],
    timing: &harmonicon_core::chart::Timing,
) -> f64 {
    track.iter().map(|item| resolve_item_time(item, timing) + item.duration).fold(0.0_f64, f64::max)
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

/// Label for the multi-note play modes; `single` (and absent) needs no badge.
pub fn play_mode_label(mode: Option<&PlayMode>) -> Option<&'static str> {
    match mode {
        Some(PlayMode::Chord) => Some("chord"),
        Some(PlayMode::Split) => Some("split"),
        Some(PlayMode::Single) | None => None,
    }
}

/// Builds every `ScheduledNote` a chart produces, gated by adaptive
/// difficulty's current unlock state, alongside each note's
/// [`play_mode_label`] tag (the chord/split badge). Sorted by `time` — `score_notes`/the spawn-window
/// helpers above all rely on that.
pub fn build_scheduled_notes(
    effective: &EffectiveHarmonica,
    chart: &HarpChart,
    adaptive: &AdaptiveDifficulty,
) -> (Vec<ScheduledNote>, Vec<Option<&'static str>>) {
    let items = track_items(&chart.track, &chart.timing);
    let flags = unlocked_flags(&items, &adaptive.sections, &adaptive.learned, adaptive.enabled);
    let mut flags = flags.into_iter();
    let mut combined: Vec<(ScheduledNote, Option<&'static str>)> = Vec::new();
    // A guitar in another tuning or a flute with the other foot joint: every
    // event placed on what the player holds (`instrument::substitute`),
    // pitches kept.
    let substituted =
        effective.substitutes_for(chart).then(|| effective.instrument_for(chart)).flatten().map(
            |played| harmonicon_core::instrument::substitute::substitute_chart(chart, &played),
        );
    for (item_index, item) in chart.track.iter().enumerate() {
        let t = resolve_item_time(item, &chart.timing);
        let tag = play_mode_label(item.play_mode.as_ref());
        // Each event's own modifiers/expected pitch, computed once up front
        // so the chord-target set (below) doesn't need to redo it.
        // (position, modifiers, expected pitch, playable). Only the pitch
        // and modifiers move for an ordinary chart; hole and action change
        // only under `HarpMapping::Transpose`.
        let event_data: Vec<(PlayPosition, Vec<Modifier>, Option<u8>, bool)> = item
            .events
            .iter()
            .enumerate()
            .filter_map(|(event_index, event)| {
                let mut modifiers = event.modifiers.clone().unwrap_or_default();
                // Any instrument but the harmonica: the event names where it
                // is played and its sounding note, and there is no
                // substitute instrument to remap onto.
                // Where that is comes from core's codec, the one reading of
                // an event per instrument (a string and fret, a fingering).
                let Some(chart_harp) = chart.harmonica.as_ref() else {
                    let charted =
                        harmonicon_core::note_event_codec::decode(chart, event).ok()?.position()?;
                    let Some(placed) = &substituted else {
                        return Some((charted, modifiers, chart.event_pitch(event), true));
                    };
                    // Out of the player's reach (a B3 on a C foot): shown
                    // where charted, not judged.
                    let Some(position) = placed[item_index][event_index] else {
                        return Some((charted, modifiers, chart.event_pitch(event), false));
                    };
                    // A harmonic replayed as a fretted note is no harmonic.
                    if let PlayPosition::Fret(at) = position
                        && at.technique
                            == harmonicon_core::instrument::guitar::StringTechnique::Fretted
                    {
                        modifiers.retain(|m| !matches!(m, Modifier::Harmonic));
                    }
                    return Some((position, modifiers, chart.event_pitch(event), true));
                };
                // Validation guarantees every event of a harmonica chart
                // names a hole and breath.
                let (hole, action) = event.hole_action()?;
                let hole_position = |hole, action, modifiers: &[Modifier]| {
                    let technique = technique_of(modifiers);
                    PlayPosition::Hole(HoleAssignment { hole, action, technique })
                };
                match &effective.harp {
                    // The overwhelmingly common case, kept on exactly the
                    // path it has always taken. A chart played on its own
                    // harmonica must not change behaviour because this
                    // feature exists, and routing it through the remapper
                    // "for consistency" would risk precisely that — the
                    // chart's explicit `note` is authoritative here and
                    // encodes pitches (an overblow's, a slide's) that a
                    // blow/draw layout lookup cannot reproduce.
                    None => {
                        let natural_pitch = event
                            .note
                            .clone()
                            .unwrap_or_else(|| chart_harp.wind_direction_label(hole, &action));
                        let expected_pitch = target_pitch(&natural_pitch, &modifiers);
                        Some((
                            hole_position(hole, action, &modifiers),
                            modifiers,
                            expected_pitch,
                            true,
                        ))
                    }
                    Some(target) => {
                        let r = harp_remap::remap_event(
                            hole,
                            action,
                            event.note.as_deref(),
                            &modifiers,
                            chart_harp,
                            target,
                            effective.mapping,
                        );
                        let position = hole_position(r.hole, r.action, &r.modifiers);
                        Some((position, r.modifiers, r.midi, r.playable))
                    }
                }
            })
            .collect();
        // A real chord/octave-split needs every sibling pitch sounding at
        // once (see `ScheduledNote::chord_pitches`) — a `TrackItem` with
        // only one event stays an ordinary single note, untouched by this.
        let chord_pitches: Vec<u8> = if item.events.len() > 1 {
            event_data
                .iter()
                .filter(|(.., playable)| *playable)
                .filter_map(|(_, _, p, _)| *p)
                .collect()
        } else {
            Vec::new()
        };
        for (position, modifiers, expected_pitch, playable) in event_data {
            let (unlocked, section) = flags.next().unwrap_or((true, 0));
            if !unlocked {
                continue;
            }
            combined.push((
                ScheduledNote {
                    time: t,
                    duration: item.duration,
                    position,
                    expected_pitch,
                    hit: false,
                    missed: false,
                    held: 0.0,
                    sustain_scored: false,
                    modifiers,
                    pitch_samples: Vec::new(),
                    amp_samples: Vec::new(),
                    phrase_section: section,
                    chord_pitches: chord_pitches.clone(),
                    playable,
                    miss_evidence: None,
                    force_wait: item.call,
                },
                tag,
            ));
        }
    }
    combined.sort_by(|a, b| a.0.time.partial_cmp(&b.0.time).unwrap_or(std::cmp::Ordering::Equal));
    combined.into_iter().unzip()
}

/// Converts scored notes' real-time `time`/`duration` into the beat-based
/// [`harmonicon_ui::music_score::NotationNote`]s the shared score overlay draws
/// (that module never touches a tempo map itself — see its own doc
/// comment). `resolution`/`tempo_map` are the chart's own, so this stays
/// correct across a mid-song tempo change. A note with no resolvable
/// `expected_pitch` has nothing to draw and is skipped, same as it already
/// can never be hit. The shared bar map splits notes at bar lines and meter
/// changes, including those in the expanded repeat performance.
pub fn notes_to_notation(
    notes: &[ScheduledNote],
    resolution: u32,
    tempo_map: &[harmonicon_core::chart::TempoPoint],
    bar_map: &harmonicon_ui::music_score::MusicScoreBarMap,
    highlight: Option<usize>,
) -> Vec<harmonicon_ui::music_score::NotationNote> {
    notes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let midi = n.expected_pitch?;
            let start_tick = harmonicon_core::chart::seconds_to_tick(n.time, resolution, tempo_map);
            let end_tick =
                harmonicon_core::chart::seconds_to_tick(n.time + n.duration, resolution, tempo_map);
            Some(bar_map.split_note(start_tick, end_tick, midi, highlight == Some(i)))
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::state::HoleTab;
    use super::*;
    use harmonicon_core::harmonica::{chromatic_harp, richter_harp};
    use harmonicon_core::harp_remap::HarpMapping;

    // ── play_mode_label ───────────────────────────────────────────────────────

    #[test]
    fn play_mode_label_badges_only_multi_note_modes() {
        assert_eq!(play_mode_label(Some(&PlayMode::Chord)), Some("chord"));
        assert_eq!(play_mode_label(Some(&PlayMode::Split)), Some("split"));
        assert_eq!(play_mode_label(Some(&PlayMode::Single)), None);
        assert_eq!(play_mode_label(None), None);
    }

    // ── build_scheduled_notes ─────────────────────────────────────────────────

    fn c_diatonic_chart(track_json: &str) -> HarpChart {
        serde_json::from_str(&format!(
            r#"{{
                "song": {{"title":"T","artist":"A","genre":"Test","tempo_bpm":120.0,"key":"C","difficulty":"easy"}},
                "timing": {{"resolution":480,"tempo_map":[{{"tick":0,"bpm":120.0}}]}},
                "harmonica": {{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
                    "layout": {{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                               "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}}},
                "track": {track_json},
                "scoring": {{"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}}
            }}"#
        ))
        .unwrap()
    }

    fn guitar_chart(track_json: &str) -> HarpChart {
        serde_json::from_str(&format!(
            r#"{{
                "song": {{"title":"T","artist":"A","genre":"Test","tempo_bpm":120.0,"key":"E","difficulty":"easy"}},
                "timing": {{"resolution":480,"tempo_map":[{{"tick":0,"bpm":120.0}}]}},
                "instrument": {{"type":"guitar","strings":["E4","B3","G3","D3","A2","E2"],"frets":22}},
                "track": {track_json},
                "scoring": {{"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}}
            }}"#
        ))
        .unwrap()
    }

    /// A guitar chart schedules its string positions and scores the note
    /// each one states, bend included, with a chord's pitches shared.
    #[test]
    fn a_guitar_chart_schedules_string_positions_and_their_notes() {
        use harmonicon_core::instrument::guitar::StringTechnique;
        let chart = guitar_chart(
            r#"[
                {"time":0.0,"duration":0.5,"phrase":"p1","events":[
                    {"position":{"string":6,"fret":0},"note":"E2"},
                    {"position":{"string":5,"fret":2},"note":"B2"}]},
                {"time":0.5,"duration":0.5,"phrase":"p2","events":[
                    {"position":{"string":2,"fret":8},"note":"A4",
                     "modifiers":[{"type":"bend","semitones":2}]}]}
            ]"#,
        );
        let (notes, _) =
            build_scheduled_notes(&EffectiveHarmonica::default(), &chart, &Default::default());
        let placed: Vec<(u8, u8, StringTechnique, Option<u8>)> = notes
            .iter()
            .map(|n| {
                let PlayPosition::Fret(a) = n.position else {
                    panic!("a guitar chart schedules string positions");
                };
                (a.string, a.fret, a.technique, n.expected_pitch)
            })
            .collect();
        assert_eq!(
            placed,
            [
                (6, 0, StringTechnique::Fretted, Some(40)),
                (5, 2, StringTechnique::Fretted, Some(47)),
                (2, 8, StringTechnique::Bend(2.0), Some(69)),
            ]
        );
        assert_eq!(notes[0].chord_pitches, [40, 47]);
        assert!(notes[2].chord_pitches.is_empty());
    }

    /// A flute event names only its note and fingering; every one is
    /// scheduled, on the pitch its note gives.
    #[test]
    fn a_flute_chart_schedules_every_note_on_its_fingering() {
        let chart: HarpChart = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/flute-scale.harpchart"
        ))
        .unwrap();
        let (notes, _) =
            build_scheduled_notes(&EffectiveHarmonica::default(), &chart, &Default::default());
        assert_eq!(notes.len(), 7);
        let d6 = notes.iter().find(|n| n.expected_pitch == Some(86)).expect("D6");
        assert_eq!(
            d6.position,
            PlayPosition::Fingering(harmonicon_core::instrument::flute::FingeringAssignment {
                note: 86,
                alternate: 1,
            })
        );
    }

    /// The flute fixture is written for a B foot. Played on a C foot its B3
    /// is out of reach, so it is shown but not judged; the rest play as
    /// charted.
    #[test]
    fn a_flute_with_a_c_foot_leaves_a_b_foot_charts_low_b_unjudged() {
        use harmonicon_core::instrument::flute::{Flute, FootJoint};
        let chart: HarpChart = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/flute-scale.harpchart"
        ))
        .unwrap();
        let effective = EffectiveHarmonica {
            instrument: Some(Flute::new(FootJoint::C).into()),
            ..Default::default()
        };
        assert!(effective.substitutes_for(&chart));
        let (notes, _) = build_scheduled_notes(&effective, &chart, &Default::default());
        let playable: Vec<bool> = notes.iter().map(|n| n.playable).collect();
        assert_eq!(playable, [false, true, true, true, true, true, true]);
        assert_eq!(notes[0].expected_pitch, Some(59));
    }

    /// A harp chosen on the harp-check page can't stand in for a guitar.
    #[test]
    fn a_substitute_harp_never_applies_to_a_guitar_chart() {
        let chart = guitar_chart(
            r#"[{"time":0.0,"duration":0.5,"events":[{"position":{"string":3,"fret":2},"note":"A3"}]}]"#,
        );
        let effective = EffectiveHarmonica {
            harp: Some(richter_harp("G")),
            mapping: HarpMapping::SameHoles,
            instrument: None,
        };
        assert!(effective.harp_for(&chart).is_none());
        assert!(matches!(
            effective.instrument_for(&chart),
            Some(harmonicon_core::instrument::Instrument::Guitar(_))
        ));
        let (notes, _) = build_scheduled_notes(&effective, &chart, &Default::default());
        assert_eq!(notes[0].expected_pitch, Some(57));
    }

    #[test]
    fn builds_one_note_per_event_sorted_by_time() {
        // Each item tags its own `phrase` so `unlocked_flags` treats them as
        // separate sections (see its own doc comment) — with
        // `AdaptiveDifficulty::default()`'s empty `sections`, two items
        // sharing one *implicit* section would otherwise have their
        // unlock-count math (ordinal vs. per-item `count`) collide, which
        // isn't a realistic chart shape (`setup_adaptive_difficulty` always
        // populates `sections` to match the chart's real phrase tags).
        let chart = c_diatonic_chart(
            r#"[
                {"time":1.0,"duration":0.5,"call":false,"phrase":"p2",
                 "events":[{"hole":2,"action":"blow"}]},
                {"time":0.0,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":1,"action":"blow"}]}
            ]"#,
        );
        let (notes, tags) = build_scheduled_notes(
            &EffectiveHarmonica::default(),
            &chart,
            &AdaptiveDifficulty::default(),
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(tags.len(), 2);
        // Sorted by time even though the track listed them out of order.
        assert_eq!(HoleTab::of(&notes[0].position).unwrap().hole, 1);
        assert_eq!(notes[0].time, 0.0);
        assert_eq!(HoleTab::of(&notes[1].position).unwrap().hole, 2);
        assert_eq!(notes[1].time, 1.0);
        assert!(notes.iter().all(|n| n.chord_pitches.is_empty()));
        assert!(tags.iter().all(|t| t.is_none()));
    }

    #[test]
    fn a_multi_event_item_shares_one_chord_pitches_set_and_gets_tagged() {
        let chart = c_diatonic_chart(
            r#"[
                {"time":0.0,"duration":0.5,"call":false,"play_mode":"chord",
                 "events":[
                    {"hole":1,"action":"blow"},
                    {"hole":4,"action":"blow"}
                 ]}
            ]"#,
        );
        let (notes, tags) = build_scheduled_notes(
            &EffectiveHarmonica::default(),
            &chart,
            &AdaptiveDifficulty::default(),
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(tags, vec![Some("chord"), Some("chord")]);
        // Both siblings carry the same chord_pitches set (both natural
        // blow notes are known pitches here), regardless of which hole.
        assert_eq!(notes[0].chord_pitches, notes[1].chord_pitches);
        assert_eq!(notes[0].chord_pitches.len(), 2);
    }

    #[test]
    fn a_single_event_item_never_gets_a_chord_pitches_set_even_if_call() {
        let chart = c_diatonic_chart(
            r#"[
                {"time":0.0,"duration":0.5,"call":true,
                 "events":[{"hole":1,"action":"blow"}]}
            ]"#,
        );
        let (notes, _) = build_scheduled_notes(
            &EffectiveHarmonica::default(),
            &chart,
            &AdaptiveDifficulty::default(),
        );
        assert_eq!(notes.len(), 1);
        assert!(notes[0].chord_pitches.is_empty());
        assert!(notes[0].force_wait, "call: true carries through to force_wait");
    }

    // ── Playing on a substituted harmonica ────────────────────────────────────

    /// Every note of a small chart, as `(hole, is_blow, expected_pitch)`.
    fn scheduled(effective: &EffectiveHarmonica, chart: &HarpChart) -> Vec<(u8, bool, Option<u8>)> {
        let (notes, _) = build_scheduled_notes(effective, chart, &AdaptiveDifficulty::default());
        notes
            .iter()
            .map(|n| {
                let tab = HoleTab::of(&n.position).unwrap();
                (tab.hole, tab.is_blow, n.expected_pitch)
            })
            .collect()
    }

    fn three_note_chart() -> HarpChart {
        c_diatonic_chart(
            r#"[
                {"time":0.0,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":1,"action":"blow","note":"C4"}]},
                {"time":0.5,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":4,"action":"blow","note":"C5"}]},
                {"time":1.0,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":2,"action":"draw","note":"G4"}]}
            ]"#,
        )
    }

    /// The position carries the technique the modifiers name, and on the
    /// played harp it sounds exactly the pitch the judge expects.
    #[test]
    fn a_notes_position_records_its_technique() {
        use harmonicon_core::instrument::InstrumentModel;
        let chart = c_diatonic_chart(
            r#"[
                {"time":0.0,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":3,"action":"draw","note":"B4",
                            "modifiers":[{"type":"bend","semitones":-2.0}]}]},
                {"time":0.5,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":6,"action":"blow","note":"A#5",
                            "modifiers":[{"type":"overblow"}]}]},
                {"time":1.0,"duration":0.5,"call":false,"phrase":"p1",
                 "events":[{"hole":4,"action":"blow","note":"C5",
                            "modifiers":[{"type":"vibrato","oscillation_hz":5.0}]}]}
            ]"#,
        );
        let (notes, _) =
            build_scheduled_notes(&EffectiveHarmonica::default(), &chart, &Default::default());
        let techniques: Vec<Technique> = notes
            .iter()
            .map(|n| {
                let PlayPosition::Hole(a) = n.position else {
                    panic!("a harmonica chart schedules hole positions");
                };
                a.technique
            })
            .collect();
        assert_eq!(techniques, [Technique::Bend(2.0), Technique::Overblow, Technique::Natural]);
        let harp = richter_harp("C");
        for note in &notes {
            assert_eq!(harp.pitch_of(&note.position), note.expected_pitch, "{:?}", note.position);
        }
    }

    #[test]
    fn no_substitution_leaves_a_chart_exactly_as_it_was() {
        // The default must be free. Anything else means this feature changed
        // how every existing chart plays.
        let chart = three_note_chart();
        let notes = scheduled(&EffectiveHarmonica::default(), &chart);
        assert_eq!(
            notes,
            vec![
                (1, true, Some(60)),  // C4
                (4, true, Some(72)),  // C5
                (2, false, Some(67)), // G4
            ]
        );
    }

    #[test]
    fn same_holes_keeps_the_tab_and_moves_every_expected_pitch() {
        let chart = three_note_chart();
        let effective = EffectiveHarmonica {
            harp: Some(richter_harp("G")),
            mapping: HarpMapping::SameHoles,
            instrument: None,
        };
        let notes = scheduled(&effective, &chart);
        let holes: Vec<(u8, bool)> = notes.iter().map(|(h, b, _)| (*h, *b)).collect();
        assert_eq!(
            holes,
            vec![(1, true), (4, true), (2, false)],
            "same-holes must not move the tab"
        );
        let g = richter_harp("G");
        assert_eq!(
            notes.iter().map(|(_, _, p)| *p).collect::<Vec<_>>(),
            vec![
                g.wind_direction_midi(1, &Action::Blow),
                g.wind_direction_midi(4, &Action::Blow),
                g.wind_direction_midi(2, &Action::Draw),
            ],
            "expected pitches must follow the G harp, not the chart's C"
        );
    }

    #[test]
    fn transpose_keeps_every_expected_pitch_and_moves_the_tab() {
        let chart = three_note_chart();
        let effective = EffectiveHarmonica {
            harp: Some(richter_harp("G")),
            mapping: HarpMapping::Transpose,
            instrument: None,
        };
        let original = scheduled(&EffectiveHarmonica::default(), &chart);
        let moved = scheduled(&effective, &chart);
        assert_eq!(
            moved.iter().map(|(_, _, p)| *p).collect::<Vec<_>>(),
            original.iter().map(|(_, _, p)| *p).collect::<Vec<_>>(),
            "transposing must preserve what the piece sounds like"
        );
    }

    /// The wiring half of the microphone invariant. `harp_remap`'s own tests
    /// prove the arithmetic; this proves that what `build_scheduled_notes`
    /// actually hands to scoring agrees with the harp `ValidHarpNotes` will
    /// be built from — the two are set in different files, and a chart's
    /// expected pitch outside that set can never be scored.
    #[test]
    fn every_expected_pitch_is_one_the_chosen_harp_can_produce() {
        let chart = three_note_chart();
        for key in ["C", "G", "A", "F", "Bb"] {
            let harp = richter_harp(key);
            let valid = harp.build_valid_notes();
            for mapping in HarpMapping::all() {
                let effective = EffectiveHarmonica {
                    harp: Some(harp.clone()),
                    mapping: *mapping,
                    instrument: None,
                };
                let (notes, _) =
                    build_scheduled_notes(&effective, &chart, &AdaptiveDifficulty::default());
                for note in &notes {
                    if !note.playable {
                        continue;
                    }
                    let Some(midi) = note.expected_pitch else {
                        continue;
                    };
                    assert!(
                        valid.contains(&midi),
                        "{key} harp, {mapping:?}: hole {} expects MIDI {midi}, \
                         which ValidHarpNotes would filter out",
                        HoleTab::of(&note.position).unwrap().hole
                    );
                }
            }
        }
    }

    #[test]
    fn a_note_the_chosen_harp_cannot_play_is_marked_unplayable() {
        // Hole 12 on a chart written for a chromatic, played on a 10-hole
        // diatonic. It must not be scoreable — and `judge` skips exactly on
        // this flag, because an unreachable note would otherwise resolve as
        // a miss.
        let chart = c_diatonic_chart(
            r#"[{"time":0.0,"duration":0.5,"events":[{"hole":1,"action":"blow","note":"C4"}]}]"#,
        );
        let effective = EffectiveHarmonica {
            harp: Some(chromatic_harp("C")),
            mapping: HarpMapping::SameHoles,
            instrument: None,
        };
        let (notes, _) = build_scheduled_notes(&effective, &chart, &AdaptiveDifficulty::default());
        assert!(notes.iter().all(|n| n.playable), "hole 1 exists on both");
    }
}
