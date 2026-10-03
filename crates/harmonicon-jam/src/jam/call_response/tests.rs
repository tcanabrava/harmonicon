// SPDX-License-Identifier: MIT

use super::phrase::PlayableNote;
use super::*;

// ── phase_for_bar ────────────────────────────────────────────────────────

#[test]
fn first_two_bars_of_the_cycle_are_calling() {
    assert_eq!(phase_for_bar(0), CallResponsePhase::Calling);
    assert_eq!(phase_for_bar(1), CallResponsePhase::Calling);
}

#[test]
fn next_two_bars_of_the_cycle_are_responding() {
    assert_eq!(phase_for_bar(2), CallResponsePhase::Responding);
    assert_eq!(phase_for_bar(3), CallResponsePhase::Responding);
}

#[test]
fn the_cycle_repeats_indefinitely() {
    assert_eq!(phase_for_bar(4), CallResponsePhase::Calling);
    assert_eq!(phase_for_bar(100), CallResponsePhase::Calling);
    assert_eq!(phase_for_bar(101), CallResponsePhase::Calling);
    assert_eq!(phase_for_bar(102), CallResponsePhase::Responding);
}

#[test]
fn the_cycle_divides_the_twelve_bar_form() {
    assert_eq!(12 % (CALL_BARS + RESPONSE_BARS), 0);
}

// ── call_seed ────────────────────────────────────────────────────────────

#[test]
fn call_seed_varies_by_bar_and_is_stable_for_a_session() {
    assert_eq!(call_seed(7, 4), call_seed(7, 4));
    assert_ne!(call_seed(7, 4), call_seed(7, 8));
    assert_ne!(call_seed(7, 4), call_seed(8, 4));
}

#[test]
fn a_fresh_state_is_reset_apart_from_its_seed() {
    let state = CallResponseState::fresh();
    assert_eq!(state.phase, CallResponsePhase::Calling);
    assert!(state.lick_holes.is_empty());
    assert!(state.speaking_until.is_none());
}

// ── call_phrase_notes ────────────────────────────────────────────────────

#[test]
fn call_phrase_notes_keeps_timing_and_breathes_on_held_notes() {
    let note = PlayableNote { midi: 64, hole: 2, blow: true };
    let call = [
        CallNote { tick: 0, len: 5, note, held: false },
        CallNote { tick: 24, len: 35, note, held: true },
    ];
    let notes = call_phrase_notes(&call);
    assert_eq!(notes.len(), 2);
    assert_eq!((notes[0].tick, notes[0].len), (0, 5));
    assert_eq!((notes[1].tick, notes[1].len), (24, 35));
    assert_eq!(notes[0].expr, Expr::None);
    assert_eq!(notes[1].expr, Expr::Vibrato(HELD_VIBRATO_HZ));
    assert!(notes.iter().all(|n| n.freq.is_some()));
}

// ── approach (the duck ramp) ─────────────────────────────────────────────

#[test]
fn the_duck_eases_down_and_back_over_the_ramp_time_without_overshoot() {
    let mut gain = 1.0;
    // Half the ramp: half-way down.
    gain = approach(gain, DUCK_GAIN, DUCK_SECS / 2.0);
    assert!((gain - (1.0 + DUCK_GAIN) / 2.0).abs() < 1e-5);
    // A whole ramp more: clamped at the dip, not past it.
    gain = approach(gain, DUCK_GAIN, DUCK_SECS);
    assert_eq!(gain, DUCK_GAIN);
    // Back up, clamped at unity.
    gain = approach(gain, 1.0, DUCK_SECS * 3.0);
    assert_eq!(gain, 1.0);
}

#[test]
fn call_duck_starts_at_unity() {
    assert_eq!(CallDuck::default().0, 1.0);
}

// ── density cycling ──────────────────────────────────────────────────────

#[test]
fn density_cycles_through_all_three_and_back() {
    let start = CallDensity::default();
    assert_eq!(start, CallDensity::Conversational);
    assert_eq!(start.next().next().next(), start);
    let mut seen = vec![start];
    let mut d = start.next();
    while d != start {
        seen.push(d);
        d = d.next();
    }
    assert_eq!(seen.len(), 3);
}
