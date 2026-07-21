// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

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

// ── pick_from_pool ───────────────────────────────────────────────────────

#[test]
fn pick_from_pool_wraps_an_out_of_range_roll() {
    let pool = [60u8, 64, 67];
    assert_eq!(pick_from_pool(&pool, 0), 60);
    assert_eq!(pick_from_pool(&pool, 2), 67);
    assert_eq!(pick_from_pool(&pool, 3), 60);
    assert_eq!(pick_from_pool(&pool, 100), pool[100 % 3]);
}

// ── chord_tone_pitches ───────────────────────────────────────────────────

#[test]
fn chord_tone_pitches_keeps_only_matching_note_classes_sorted() {
    let mut note_to_holes = HashMap::new();
    note_to_holes.insert(64u8, vec![2]); // E4 — chord tone
    note_to_holes.insert(62u8, vec![1]); // D4 — not a chord tone
    note_to_holes.insert(60u8, vec![1]); // C4 — chord tone
    let chord_tones: HashSet<String> = ["C".to_string(), "E".to_string()].into_iter().collect();
    assert_eq!(
        chord_tone_pitches(&note_to_holes, &chord_tones),
        vec![60, 64]
    );
}

#[test]
fn chord_tone_pitches_is_empty_when_nothing_matches() {
    let mut note_to_holes = HashMap::new();
    note_to_holes.insert(62u8, vec![1]); // D4
    let chord_tones: HashSet<String> = ["C".to_string()].into_iter().collect();
    assert!(chord_tone_pitches(&note_to_holes, &chord_tones).is_empty());
}

// ── generate_lick ────────────────────────────────────────────────────────

#[test]
fn generate_lick_is_empty_for_an_empty_pool() {
    assert!(generate_lick(&[]).is_empty());
}

#[test]
fn generate_lick_has_the_expected_length_and_only_uses_pool_pitches() {
    let pool = [60u8, 64, 67, 70];
    let lick = generate_lick(&pool);
    assert_eq!(lick.len(), LICK_LEN);
    for &m in &lick {
        assert!(pool.contains(&m));
    }
}

// ── lick_phrase_notes ────────────────────────────────────────────────────

#[test]
fn lick_phrase_notes_places_one_note_per_beat_in_order() {
    let notes = lick_phrase_notes(&[60, 64, 67]);
    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0].tick, 0);
    assert_eq!(notes[1].tick, TICKS_PER_BEAT);
    assert_eq!(notes[2].tick, TICKS_PER_BEAT * 2);
    for n in &notes {
        assert_eq!(n.len, TICKS_PER_BEAT);
        assert!(n.freq.is_some());
    }
}
