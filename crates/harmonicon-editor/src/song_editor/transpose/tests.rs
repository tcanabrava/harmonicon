// SPDX-License-Identifier: MIT

use harmonicon_core::harmonica::richter_harp;
use harmonicon_core::synth::Expr;

use super::super::state::{Dir, Pitch};
use super::*;

fn note(id: u32, hole: u8, dir: Dir, pitch: Pitch, tick: usize) -> GridNote {
    GridNote { id, hole, tick, len: 6, dir, pitch, expr: Expr::None }
}

fn c_harp() -> Harmonica {
    richter_harp("C")
}

fn midis(notes: &[GridNote], harp: &Harmonica) -> Vec<Option<u8>> {
    notes.iter().map(|n| note_midi(n, harp)).collect()
}

// ── transpose_notes ──────────────────────────────────────────────────────

#[test]
fn a_semitone_up_moves_every_note_to_the_hole_that_sounds_it() {
    let harp = c_harp();
    // C4 (blow 1), E4 (blow 2), G4 (blow 3) — a C major arpeggio.
    let mut notes = vec![
        note(1, 1, Dir::Blow, Pitch::Normal, 0),
        note(2, 2, Dir::Blow, Pitch::Normal, 12),
        note(3, 3, Dir::Blow, Pitch::Normal, 24),
    ];
    let before = midis(&notes, &harp);
    let outcome = transpose_notes(&mut notes, &[], 2, &harp, HarmonicaKind::Diatonic);
    assert_eq!((outcome.moved, outcome.kept), (3, 0));
    let after = midis(&notes, &harp);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(a.unwrap(), b.unwrap() + 2);
    }
    // D4 is draw 1, F#4 is draw 2 bent a semitone, A4 is draw 3 bent.
    assert_eq!((notes[0].hole, notes[0].dir, notes[0].pitch), (1, Dir::Draw, Pitch::Normal));
    assert_eq!(notes[1].hole, 2);
    assert!(matches!(notes[1].pitch, Pitch::Bend(_)));
    assert_eq!((notes[2].hole, notes[2].dir), (3, Dir::Draw));
    assert!(matches!(notes[2].pitch, Pitch::Bend(_)));
    // Ids, timing and expression ride along.
    assert_eq!(notes.iter().map(|n| n.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(notes.iter().map(|n| n.tick).collect::<Vec<_>>(), [0, 12, 24]);
}

#[test]
fn transposing_down_and_back_up_is_the_identity_for_natural_notes() {
    let harp = c_harp();
    let mut notes =
        vec![note(1, 4, Dir::Blow, Pitch::Normal, 0), note(2, 5, Dir::Draw, Pitch::Normal, 12)];
    let original = notes.clone();
    transpose_notes(&mut notes, &[], -5, &harp, HarmonicaKind::Diatonic);
    transpose_notes(&mut notes, &[], 5, &harp, HarmonicaKind::Diatonic);
    assert_eq!(midis(&notes, &harp), midis(&original, &harp));
}

#[test]
fn only_the_selection_moves_and_it_may_not_land_on_a_note_that_stays() {
    let harp = c_harp();
    // Two notes on hole 4, one blow (C5) and one draw (D5), at the same
    // tick — a chord that can't exist, but the point is the collision:
    // shifting the C5 up two semitones would land on D5's exact spot.
    let mut notes = vec![
        note(1, 4, Dir::Blow, Pitch::Normal, 0),
        note(2, 4, Dir::Draw, Pitch::Normal, 0),
        note(3, 1, Dir::Blow, Pitch::Normal, 48),
    ];
    let outcome = transpose_notes(&mut notes, &[1], 2, &harp, HarmonicaKind::Diatonic);
    assert_eq!((outcome.moved, outcome.kept), (0, 1));
    assert_eq!(notes[0], note(1, 4, Dir::Blow, Pitch::Normal, 0));
    // Out-of-scope notes are untouched even though they could have moved.
    assert_eq!(notes[2], note(3, 1, Dir::Blow, Pitch::Normal, 48));
}

#[test]
fn a_pitch_the_harp_cannot_sound_keeps_the_note_where_it_was() {
    let harp = c_harp();
    // C7 (blow 10) can overdraw to C#7, but not reach C#8.
    let mut notes = vec![note(1, 10, Dir::Blow, Pitch::Normal, 0)];
    let outcome = transpose_notes(&mut notes, &[], 13, &harp, HarmonicaKind::Diatonic);
    assert_eq!((outcome.moved, outcome.kept), (0, 1));
    assert_eq!(notes[0], note(1, 10, Dir::Blow, Pitch::Normal, 0));
}

#[test]
fn alternate_fingering_keeps_two_transposed_notes_from_fighting_over_one_hole() {
    let harp = c_harp();
    // F4 (draw 2 bent 2) and F#4 (draw 2 bent 1) can't both exist at one
    // tick — but shifted up a semitone they become F#4 and G4, and G4 has
    // two homes (draw 2, blow 3), so the resolver's first pick, draw 2,
    // collides with F#4's. One of them must still land.
    let mut notes = vec![
        note(1, 2, Dir::Draw, Pitch::Bend(2.0), 0),
        note(2, 2, Dir::Draw, Pitch::Bend(1.0), 0),
    ];
    let outcome = transpose_notes(&mut notes, &[], 1, &harp, HarmonicaKind::Diatonic);
    assert_eq!((outcome.moved, outcome.kept), (2, 0), "{outcome:?}");
    let holes_at_zero: Vec<(u8, Dir)> = notes.iter().map(|n| (n.hole, n.dir)).collect();
    assert_ne!(holes_at_zero[0], holes_at_zero[1], "two notes on one reed");
    assert_eq!(midis(&notes, &harp), [Some(66), Some(67)]);
}

#[test]
fn the_report_counts_mixed_breath_stacks_the_shift_creates() {
    let harp = c_harp();
    // C4 + F4 up an octave become C5 (blow 4) + F5 (draw 5): a
    // simultaneous mixed-breath stack on distinct holes.
    let mut notes =
        vec![note(1, 1, Dir::Blow, Pitch::Normal, 0), note(2, 2, Dir::Draw, Pitch::Bend(2.0), 0)];
    let outcome = transpose_notes(&mut notes, &[], 12, &harp, HarmonicaKind::Diatonic);
    assert_eq!(outcome.moved, 2);
    assert_eq!(outcome.diagnostics.mixed_breath_groups, 1);
}

// ── outcome_message ──────────────────────────────────────────────────────

#[test]
fn a_clean_transposition_signs_its_interval() {
    let outcome = TransposeOutcome { semitones: -3, moved: 4, ..Default::default() };
    let loc = Localization::default();
    let message = outcome_message(&outcome, &loc);
    // Without loaded locales `msg_args` echoes the key; the arguments are
    // still resolved into it by the formatter when the key resolves, so
    // only the key choice is checked here.
    assert!(message.contains("editor-transposed"));
    assert!(!message.contains("warning"));
    let outcome = TransposeOutcome { kept: 1, ..outcome };
    assert!(outcome_message(&outcome, &loc).contains("editor-transposed-warning"));
}
