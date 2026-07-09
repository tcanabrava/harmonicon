// SPDX-License-Identifier: MIT

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy_fluent::prelude::Localization;

use crate::audio_system::midi::{freq_to_midi, midi_to_note};
use crate::audio_system::pitch_detect::PitchEvent;
use crate::localization::{LocalizationExt, LocalizedStr};
use crate::scoring::{
    AttackGate, HitQuality, NoteOutcome, classify_note, compute_points, sustain_points,
};
use crate::settings::AudioSettings;

use super::TICKS_PER_BEAT;
use super::playback::{EditorAudio, Playhead, build_harp, note_freq};
use super::state::EditorState;

// ── Timing windows ────────────────────────────────────────────────────────────

/// Onset must land within ±60 ms of the note start for a Perfect.
const PERFECT_WINDOW: f64 = 0.060;
/// Onset within ±130 ms scores a Good.
const GOOD_WINDOW: f64 = 0.130;
/// After 200 ms past the onset the note is marked Missed.
const MISS_WINDOW: f64 = 0.200;

/// How long a hit/miss result stays on screen before a "waiting for the next
/// note" prompt is allowed to replace it. Without this, the tick right after
/// a hit immediately re-evaluates the next (already-`Waiting`) note and
/// overwrites the result before it's readable — see `practice_tick`.
const MSG_HOLD_SECS: f32 = 0.6;

/// 2^(0.5/12) — frequency ratio spanning ±50 cents.
/// Detected pitches within this band of the expected frequency count as a match.
const PITCH_TOLERANCE: f32 = 1.029_302_2;

// ── Types ─────────────────────────────────────────────────────────────────────

/// One note from the editor grid, compiled into a practice-scoring record.
struct PracticeNote {
    start_secs: f64,
    end_secs: f64,
    /// Expected pitch frequency in Hz (key-transposed, bend-adjusted).
    expected_freq: f32,
    /// Human-readable name of the expected pitch, e.g. "G4".
    expected_name: String,
    hit: bool,
    missed: bool,
    /// Seconds the player held the correct pitch after scoring the onset.
    held: f64,
    /// True once the sustain bonus for this note has been paid out.
    sustain_done: bool,
}

#[derive(Resource, Default)]
pub(super) struct PracticeState {
    pub active: bool,
    notes: Vec<PracticeNote>,
    /// Notes (keyed by schedule index) consumed by the current sustained
    /// breath. An index is released once that note's expected frequency
    /// stops being detected, re-arming it for the next articulation. Shared
    /// re-attack logic with `crate::gameplay::PitchGate` — see
    /// `crate::scoring::AttackGate`.
    consumed: AttackGate<usize>,
    pub score: u32,
    pub hits: u32,
    pub misses: u32,
    pub total: u32,
    /// Status line shown in the editor's status bar while practice is running.
    pub msg: LocalizedStr,
    /// Seconds left before [`MSG_HOLD_SECS`] releases its hold on `msg` —
    /// see that constant's doc comment.
    msg_hold: f32,
}

impl PracticeState {
    pub(super) fn reset(&mut self) {
        *self = PracticeState::default();
    }
}

// ── Schedule builder ──────────────────────────────────────────────────────────

fn build_schedule(state: &EditorState) -> Vec<PracticeNote> {
    let bpm: f32 = state.tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    let harp = build_harp(&state.key, state.harmonica_kind);

    let mut notes: Vec<PracticeNote> = state
        .notes
        .iter()
        .filter_map(|n| {
            let freq = note_freq(n, &harp)?;
            let name = freq_to_name(freq);
            Some(PracticeNote {
                start_secs: n.tick as f64 * secs_per_tick as f64,
                end_secs: (n.tick + n.len) as f64 * secs_per_tick as f64,
                expected_freq: freq,
                expected_name: name,
                hit: false,
                missed: false,
                held: 0.0,
                sustain_done: false,
            })
        })
        .collect();

    notes.sort_by(|a, b| {
        a.start_secs
            .partial_cmp(&b.start_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    notes
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Start practice mode: plays background music only (no synthesized notes),
/// then scores the player's microphone input against the editor's note grid.
pub(super) fn start_practice(
    state: &EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing: &Query<Entity, With<EditorAudio>>,
    practice: &mut PracticeState,
    playhead: &mut Playhead,
    commands: &mut Commands,
    loc: &Localization,
) {
    for e in playing {
        commands.entity(e).despawn();
    }

    practice.reset();
    practice.notes = build_schedule(state);
    practice.total = practice.notes.len() as u32;
    practice.active = true;

    let bpm: f32 = state.tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    let end_tick = state
        .notes
        .iter()
        .map(|n| n.tick + n.len)
        .max()
        .unwrap_or(0);

    *playhead = Playhead {
        playing: true,
        paused: false,
        elapsed: 0.0,
        total: end_tick as f32 * secs_per_tick,
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
            Err(e) => warn!("Practice: can't read background music {music:?}: {e}"),
        }
    } else {
        practice.msg = loc.msg("practice-no-music");
    }
}

/// Stop practice and reset all state. Safe to call when practice is not active.
pub(super) fn stop_practice(
    playing: &Query<Entity, With<EditorAudio>>,
    practice: &mut PracticeState,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    playhead.playing = false;
    playhead.paused = false;
    practice.reset();
}

// ── System ────────────────────────────────────────────────────────────────────

pub(super) fn practice_tick(
    time: Res<Time>,
    playhead: Res<Playhead>,
    settings: Res<AudioSettings>,
    loc: Res<Localization>,
    mut pitch_events: MessageReader<PitchEvent>,
    mut practice: ResMut<PracticeState>,
) {
    if !practice.active {
        // Drain unread pitch events so they don't pile up while idle.
        for _ in pitch_events.read() {}
        return;
    }

    // Playback ended — finalize any unscored notes and show the summary.
    if !playhead.playing {
        let mut extra_misses = 0u32;
        for note in practice.notes.iter_mut() {
            if !note.hit && !note.missed {
                note.missed = true;
                extra_misses += 1;
            }
        }
        practice.misses += extra_misses;
        let (hits, total, score) = (practice.hits, practice.total, practice.score);
        practice.msg = loc.msg_args(
            "practice-done",
            &[
                ("hits", hits.to_string()),
                ("total", total.to_string()),
                ("score", score.to_string()),
            ],
        );
        practice.active = false;
        return;
    }

    // Collect the freshest detected pitches; last event wins (pitch events arrive
    // at the audio pipeline's chunk rate, ~10 Hz, not at the frame rate).
    let mut detected: Vec<f32> = Vec::new();
    for ev in pitch_events.read() {
        detected = ev.0.iter().map(|p| p.frequency).collect();
    }

    // Latency compensation: a pitch detected now was actually played
    // `input_latency_ms` ago, so shift the judgment point back.
    let latency = settings.input_latency_ms as f64 / 1000.0;
    let judged = playhead.elapsed as f64 - latency;
    let dt = time.delta_secs_f64();

    // Take ownership of `consumed` so we can freely read `notes` while filtering.
    // Re-arm any entry whose frequency is no longer sounding — the player must
    // re-articulate to score the next occurrence of the same pitch.
    let mut consumed = std::mem::take(&mut practice.consumed);
    consumed.release_absent(|idx| {
        practice
            .notes
            .get(idx)
            .is_some_and(|n| detected.iter().any(|&f| freq_matches(f, n.expected_freq)))
    });

    // Score all notes, collecting mutations for application after the loop.
    let mut hits_delta: u32 = 0;
    let mut misses_delta: u32 = 0;
    let mut score_delta: u32 = 0;
    let mut new_msg: Option<LocalizedStr> = None;
    // Whether `new_msg` is a hit/miss result (arms `msg_hold`) rather than a
    // "waiting for the next note" prompt (which must respect an active hold).
    let mut is_result_msg = false;

    for (i, note) in practice.notes.iter_mut().enumerate() {
        if note.missed {
            continue;
        }

        // Sustain phase: onset was already scored — reward holding the pitch.
        if note.hit {
            if note.sustain_done {
                continue;
            }
            if judged < note.end_secs {
                if detected
                    .iter()
                    .any(|&f| freq_matches(f, note.expected_freq))
                {
                    note.held += dt;
                }
            } else {
                score_delta += sustain_points(note.held, note.end_secs - note.start_secs);
                note.sustain_done = true;
            }
            continue;
        }

        let offset = judged - note.start_secs;
        // A note scores only on a fresh attack: the pitch must be sounding AND
        // not already consumed by an earlier note in this continuous breath.
        let is_playing = detected
            .iter()
            .any(|&f| freq_matches(f, note.expected_freq));
        let playing_expected = consumed.is_fresh(i, is_playing);

        match classify_note(
            offset,
            playing_expected,
            PERFECT_WINDOW,
            GOOD_WINDOW,
            MISS_WINDOW,
        ) {
            NoteOutcome::Missed => {
                note.missed = true;
                misses_delta += 1;
                let name = note.expected_name.clone();
                if new_msg.is_none() {
                    new_msg = Some(loc.msg_args("practice-missed", &[("note", name)]));
                    is_result_msg = true;
                }
            }
            NoteOutcome::Waiting => {
                let got = detected
                    .first()
                    .copied()
                    .map(freq_to_name)
                    .unwrap_or_default();
                let expected = note.expected_name.clone();
                new_msg.get_or_insert_with(|| {
                    if got.is_empty() {
                        loc.msg_args("practice-prompt", &[("note", expected)])
                    } else {
                        loc.msg_args(
                            "practice-wrong-note",
                            &[("got", got), ("expected", expected)],
                        )
                    }
                });
            }
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                hits_delta += 1;
                consumed.consume(i);
                let pts = compute_points(quality, 1.0);
                score_delta += pts;
                let name = note.expected_name.clone();
                new_msg = Some(match quality {
                    HitQuality::Perfect => loc.msg_args(
                        "practice-hit-perfect",
                        &[("note", name), ("pts", pts.to_string())],
                    ),
                    HitQuality::Good => loc.msg_args(
                        "practice-hit-good",
                        &[("note", name), ("pts", pts.to_string())],
                    ),
                });
                is_result_msg = true;
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap => {}
        }
    }

    practice.consumed = consumed;
    practice.hits += hits_delta;
    practice.misses += misses_delta;
    practice.score += score_delta;
    practice.msg_hold = (practice.msg_hold - dt as f32).max(0.0);
    if let Some(msg) = new_msg
        && should_update_msg(is_result_msg, practice.msg_hold)
    {
        practice.msg = msg;
        if is_result_msg {
            practice.msg_hold = MSG_HOLD_SECS;
        }
    }
}

/// Whether a freshly-computed status message should replace the one
/// currently on screen: a hit/miss result always wins (and — separately —
/// arms a fresh hold, see `practice_tick`); a "waiting for the next note"
/// prompt only wins once the previous result's hold has counted down to
/// zero. Without this, the tick right after a hit immediately overwrites the
/// result with the next note's prompt before it's readable.
fn should_update_msg(is_result_msg: bool, msg_hold: f32) -> bool {
    is_result_msg || msg_hold <= 0.0
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// True when `detected` is within ±50 cents of `expected`.
pub(super) fn freq_matches(detected: f32, expected: f32) -> bool {
    if expected <= 0.0 {
        return false;
    }
    let ratio = detected / expected;
    (1.0 / PITCH_TOLERANCE..=PITCH_TOLERANCE).contains(&ratio)
}

/// Nearest MIDI note name for a raw frequency (used in "you played X" messages).
pub(super) fn freq_to_name(freq: f32) -> String {
    freq_to_midi(freq).map(midi_to_note).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_editor::interaction::select_or_add;
    use crate::song_editor::state::{Dir, Expr, GridNote, Pitch};

    fn state_with_notes(key: &str, placements: &[(u8, usize)]) -> EditorState {
        let mut state = EditorState::default();
        state.key = key.into();
        for &(hole, tick) in placements {
            select_or_add(&mut state, hole, tick);
        }
        state
    }

    // ── should_update_msg ────────────────────────────────────────────────────

    #[test]
    fn a_result_message_always_wins() {
        assert!(should_update_msg(true, 0.6));
        assert!(should_update_msg(true, 0.0));
    }

    #[test]
    fn a_prompt_is_blocked_while_the_hold_is_active() {
        assert!(!should_update_msg(false, 0.6));
        assert!(!should_update_msg(false, 0.001));
    }

    #[test]
    fn a_prompt_wins_once_the_hold_expires() {
        assert!(should_update_msg(false, 0.0));
    }

    // ── freq_matches ─────────────────────────────────────────────────────────

    #[test]
    fn freq_matches_exact_pitch() {
        assert!(freq_matches(440.0, 440.0));
    }

    #[test]
    fn freq_matches_within_fifty_cents_either_direction() {
        // 2^(0.5/12) ≈ the ±50-cent boundary ratio.
        assert!(freq_matches(440.0 * 1.029, 440.0));
        assert!(freq_matches(440.0 / 1.029, 440.0));
    }

    #[test]
    fn freq_matches_rejects_beyond_fifty_cents() {
        assert!(!freq_matches(440.0 * 1.06, 440.0));
        assert!(!freq_matches(440.0 / 1.06, 440.0));
    }

    #[test]
    fn freq_matches_rejects_nonpositive_expected() {
        assert!(!freq_matches(440.0, 0.0));
        assert!(!freq_matches(440.0, -10.0));
    }

    // ── freq_to_name ─────────────────────────────────────────────────────────

    #[test]
    fn freq_to_name_identifies_concert_pitch() {
        assert_eq!(freq_to_name(440.0), "A4");
    }

    #[test]
    fn freq_to_name_is_empty_for_silence_or_invalid_input() {
        assert_eq!(freq_to_name(0.0), "");
        assert_eq!(freq_to_name(-5.0), "");
    }

    // ── build_schedule ───────────────────────────────────────────────────────

    #[test]
    fn build_schedule_sorts_notes_by_start_time() {
        let state = state_with_notes("C", &[(3, 8), (2, 0), (5, 4)]);
        let schedule = build_schedule(&state);
        let starts: Vec<f64> = schedule.iter().map(|n| n.start_secs).collect();
        let mut sorted = starts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(starts, sorted);
    }

    #[test]
    fn build_schedule_matches_note_freq_and_applies_key_transposition() {
        let state = state_with_notes("D", &[(2, 0)]);
        let schedule = build_schedule(&state);
        assert_eq!(schedule.len(), 1);
        let harp = build_harp(&state.key, state.harmonica_kind);
        let expected_freq = note_freq(&state.notes[0], &harp).unwrap();
        assert_eq!(schedule[0].expected_freq, expected_freq);
        // A C-harp draw-2 in D (up a whole step) should not equal the C-key freq.
        let c_state = state_with_notes("C", &[(2, 0)]);
        let c_harp = build_harp(&c_state.key, c_state.harmonica_kind);
        let c_freq = note_freq(&c_state.notes[0], &c_harp).unwrap();
        assert_ne!(expected_freq, c_freq);
    }

    #[test]
    fn build_schedule_derives_timing_from_tempo_and_tick_length() {
        let mut state = state_with_notes("C", &[(2, 0)]);
        state.tempo = "120".into();
        let schedule = build_schedule(&state);
        let secs_per_tick = 60.0 / 120.0 / TICKS_PER_BEAT as f64;
        assert_eq!(schedule[0].start_secs, 0.0);
        assert_eq!(
            schedule[0].end_secs,
            state.notes[0].len as f64 * secs_per_tick
        );
    }

    #[test]
    fn build_schedule_skips_notes_with_no_resolvable_frequency() {
        // Hole 0 is out of the harp's 1..=10 range, so note_freq returns None
        // and the note must be dropped rather than panicking or defaulting.
        let mut state = EditorState::default();
        state.notes.push(GridNote {
            id: 0,
            hole: 0,
            tick: 0,
            len: 4,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        });
        assert!(build_schedule(&state).is_empty());
    }
}
