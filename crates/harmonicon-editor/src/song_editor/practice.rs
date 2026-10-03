// SPDX-License-Identifier: MIT

use bevy::audio::AudioSource;
use bevy::prelude::*;

use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::{PitchEvent, PitchInfo};
use harmonicon_core::midi::{freq_to_midi, midi_to_note};
use harmonicon_core::scoring::{
    AttackGate, HitQuality, NoteOutcome, classify_note, compute_points, sustain_points,
};
use harmonicon_platform::localization::{Localization, LocalizationExt, LocalizedStr};

#[cfg(test)]
use super::TICKS_PER_BEAT;
use super::playback::{
    EditorAudio, Playhead, note_freq, playhead_for, secs_per_tick, spawn_background_music,
};
use super::state::EditorState;

// ── Timing windows ────────────────────────────────────────────────────────────

/// Onset must land within ±60 ms of the note start for a Perfect.
const PERFECT_WINDOW: f64 = 0.060;
/// Onset within ±130 ms scores a Good.
const GOOD_WINDOW: f64 = 0.130;
/// After 200 ms past the onset the note is marked Missed.
const MISS_WINDOW: f64 = 0.200;

/// How long a hit/miss result stays on screen before anything else may
/// replace it — long enough to actually read "Perfect G4 +100". Without
/// this, the next tick immediately re-evaluates the following note and
/// overwrites the result before it's readable — see `practice_tick`.
const MSG_HOLD_SECS: f32 = 1.0;

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
    /// re-attack logic with `harmonicon_gameplay::gameplay::PitchGate` — see
    /// `harmonicon_core::scoring::AttackGate`.
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
    /// A result message that arrived while `msg`'s hold was still active —
    /// queued rather than dropped, so a note scored quickly after another
    /// (common at any reasonable tempo) still gets its own moment on
    /// screen instead of silently losing its feedback. Only the latest
    /// queued result is kept; a still-newer one replaces it rather than
    /// building up a backlog that would lag further and further behind
    /// actual play.
    pending_msg: Option<LocalizedStr>,
}

impl PracticeState {
    pub(super) fn reset(&mut self) {
        *self = PracticeState::default();
    }
}

// ── Schedule builder ──────────────────────────────────────────────────────────

fn build_schedule(state: &EditorState) -> Vec<PracticeNote> {
    let secs_per_tick = secs_per_tick(state);
    let harp = state.effective_harp();

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
        a.start_secs.partial_cmp(&b.start_secs).unwrap_or(std::cmp::Ordering::Equal)
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

    let spt = secs_per_tick(state);
    let end_tick = state.notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    *playhead = playhead_for(end_tick, spt);

    if !spawn_background_music(state, sources, settings, commands) {
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
    let mut detected: &[PitchInfo] = &[];
    for ev in pitch_events.read() {
        detected = &ev.0;
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
            .is_some_and(|n| detected.iter().any(|p| freq_matches(p.frequency, n.expected_freq)))
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
                if detected.iter().any(|p| freq_matches(p.frequency, note.expected_freq)) {
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
        let is_playing = detected.iter().any(|p| freq_matches(p.frequency, note.expected_freq));
        let playing_expected = consumed.is_fresh(i, is_playing);

        match classify_note(offset, playing_expected, PERFECT_WINDOW, GOOD_WINDOW, MISS_WINDOW) {
            NoteOutcome::Missed => {
                note.missed = true;
                misses_delta += 1;
                if new_msg.is_none() {
                    new_msg = Some(
                        loc.msg_args("practice-missed", &[("note", note.expected_name.clone())]),
                    );
                    is_result_msg = true;
                }
            }
            NoteOutcome::Waiting => {
                new_msg.get_or_insert_with(|| {
                    let got =
                        detected.first().map(|p| freq_to_name(p.frequency)).unwrap_or_default();
                    let expected = note.expected_name.clone();
                    if got.is_empty() {
                        loc.msg_args("practice-prompt", &[("note", expected)])
                    } else {
                        loc.msg_args("practice-wrong-note", &[("got", got), ("expected", expected)])
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
                    HitQuality::Good => loc
                        .msg_args("practice-hit-good", &[("note", name), ("pts", pts.to_string())]),
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
    let hold_active = practice.msg_hold > 0.0;

    match decide_msg_action(is_result_msg, hold_active, practice.pending_msg.is_some()) {
        MsgAction::ShowNew => {
            if let Some(msg) = new_msg {
                practice.msg = msg;
                practice.pending_msg = None;
                if is_result_msg {
                    practice.msg_hold = MSG_HOLD_SECS;
                }
            }
        }
        MsgAction::Queue => practice.pending_msg = new_msg,
        MsgAction::PromotePending => {
            if let Some(pending) = practice.pending_msg.take() {
                practice.msg = pending;
                practice.msg_hold = MSG_HOLD_SECS;
            }
        }
        MsgAction::Keep => {}
    }
}

/// What `practice_tick` should do with `msg`/`msg_hold`/`pending_msg` this
/// tick.
#[derive(PartialEq, Eq, Debug)]
enum MsgAction {
    /// Show this tick's own message now (arming a fresh hold only applies
    /// to a result — `practice_tick` checks `is_result_msg` itself for
    /// that, since it's the one place that already has it in scope).
    ShowNew,
    /// A previous result is still being held — queue this tick's result
    /// instead of dropping it.
    Queue,
    /// The hold just released and a result was already queued from an
    /// earlier tick — show that instead of this tick's own message (a
    /// "waiting for the next note" prompt, since a result always takes
    /// the `Queue`/`ShowNew` path above instead of reaching this one).
    PromotePending,
    /// Nothing to do — keep whatever's currently shown.
    Keep,
}

/// Decides message precedence for one `practice_tick` pass: a result
/// always wins eventually (immediately, or queued if one's already being
/// held — never dropped); a "waiting" prompt only wins once both the hold
/// and any queued result are out of the way, so a fresh hit can't get
/// overwritten before it's readable.
fn decide_msg_action(is_result_msg: bool, hold_active: bool, has_pending: bool) -> MsgAction {
    if is_result_msg {
        if hold_active { MsgAction::Queue } else { MsgAction::ShowNew }
    } else if hold_active {
        MsgAction::Keep
    } else if has_pending {
        MsgAction::PromotePending
    } else {
        MsgAction::ShowNew
    }
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
    use super::super::playback::build_harp;
    use super::*;
    use crate::song_editor::interaction::select_or_add;
    use crate::song_editor::state::{Dir, Expr, GridNote, Pitch};

    fn state_with_notes(key: &str, placements: &[(u8, usize)]) -> EditorState {
        let mut state = EditorState { key: key.into(), ..Default::default() };
        for &(hole, tick) in placements {
            select_or_add(&mut state, hole, tick);
        }
        state
    }

    // ── decide_msg_action ────────────────────────────────────────────────────

    #[test]
    fn a_result_shows_immediately_when_nothing_is_held() {
        assert_eq!(decide_msg_action(true, false, false), MsgAction::ShowNew);
    }

    #[test]
    fn a_result_queues_instead_of_dropping_when_a_hold_is_active() {
        // The bug this whole thing exists to fix: a second note scored
        // quickly after another used to overwrite the first result
        // immediately (or worse, silently lose it) instead of respecting
        // the hold.
        assert_eq!(decide_msg_action(true, true, false), MsgAction::Queue);
        // Still queues (replacing whatever was already queued) even if
        // something was already pending.
        assert_eq!(decide_msg_action(true, true, true), MsgAction::Queue);
    }

    #[test]
    fn a_prompt_is_blocked_while_the_hold_is_active() {
        assert_eq!(decide_msg_action(false, true, false), MsgAction::Keep);
        assert_eq!(decide_msg_action(false, true, true), MsgAction::Keep);
    }

    #[test]
    fn a_pending_result_is_promoted_over_a_fresh_prompt_once_the_hold_expires() {
        assert_eq!(decide_msg_action(false, false, true), MsgAction::PromotePending);
    }

    #[test]
    fn a_prompt_wins_once_the_hold_expires_and_nothing_is_pending() {
        assert_eq!(decide_msg_action(false, false, false), MsgAction::ShowNew);
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
        // `build_schedule` computes `secs_per_tick` in `f32` (production
        // code shares it with real-time playback, where `f32` is the
        // established precision throughout); at `TICKS_PER_BEAT` values
        // that aren't an exact power-of-two fraction of a beat (12 isn't,
        // unlike the old 4), that `f32` rounding no longer cancels out
        // exactly against this test's own `f64` computation — hence the
        // epsilon rather than `assert_eq!`.
        assert!((schedule[0].end_secs - state.notes[0].len as f64 * secs_per_tick).abs() < 1e-6);
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
