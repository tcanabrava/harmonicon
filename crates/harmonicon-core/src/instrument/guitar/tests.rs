// SPDX-License-Identifier: MIT

use super::*;
use crate::instrument::{
    Envelope, ExpressionScope, Instrument, InstrumentModel, PlayPosition, ScrollAxis,
};
use crate::midi::note_to_midi;

fn midi(note: &str) -> u8 {
    u8::try_from(note_to_midi(note).unwrap()).unwrap()
}

fn fret(string: u8, fret: u8) -> FretAssignment {
    FretAssignment { string, fret, technique: StringTechnique::Fretted }
}

fn standard() -> Guitar {
    Guitar::standard(GuitarBody::Electric)
}

#[test]
fn standard_tuning_spans_low_e_to_the_top_fret_of_high_e() {
    let guitar = standard();
    assert_eq!(guitar.range(), Some((midi("E2"), midi("E4") + 22)));
    assert_eq!(Guitar::standard(GuitarBody::Acoustic).top_fret(), 20);
}

#[test]
fn string_one_is_the_highest() {
    let guitar = standard();
    assert_eq!(guitar.open_pitch(1), Some(midi("E4")));
    assert_eq!(guitar.open_pitch(6), Some(midi("E2")));
    assert_eq!(guitar.open_pitch(7), None);
    assert_eq!(guitar.open_pitch(0), None);
}

/// Tab for a capoed guitar counts from the capo: fret 0 is the capo itself,
/// and the frets above it are fewer.
#[test]
fn frets_count_from_the_capo() {
    let guitar = Guitar { capo: 2, ..standard() };
    assert_eq!(guitar.open_pitch(6), Some(midi("F#2")));
    assert_eq!(guitar.pitch_of(&fret(6, 0)), Some(midi("F#2")));
    assert_eq!(guitar.top_fret(), 20);
    assert_eq!(guitar.pitch_of(&fret(1, 21)), None);
}

#[test]
fn a_pitch_is_found_on_every_string_that_reaches_it_lowest_fret_first() {
    let guitar = standard();
    assert_eq!(guitar.fretted_positions(midi("A2")), vec![fret(5, 0), fret(6, 5)]);
    // Below the low E there is nowhere to play it.
    assert!(guitar.fretted_positions(midi("D2")).is_empty());
}

#[test]
fn every_position_sounds_the_pitch_it_was_found_for() {
    let guitar = standard();
    for target in 0..=127 {
        for position in guitar.positions_for(target) {
            assert_eq!(InstrumentModel::pitch_of(&guitar, &position), Some(target), "{position:?}");
        }
    }
}

#[test]
fn every_pitch_in_range_is_reachable_and_nothing_outside_it() {
    let guitar = standard();
    let (low, high) = guitar.range().unwrap();
    for target in 0..=127u8 {
        let reachable = !guitar.positions_for(target).is_empty();
        assert_eq!(reachable, (low..=high).contains(&target), "MIDI {target}");
    }
}

#[test]
fn a_bend_pushes_the_fretted_pitch_up() {
    let guitar = standard();
    let bend = |depth| {
        guitar.pitch_of(&FretAssignment { technique: StringTechnique::Bend(depth), ..fret(2, 8) })
    };
    assert_eq!(bend(2.0), Some(midi("A4")));
    assert_eq!(bend(0.5), Some(midi("G#4")), "a quarter-tone bend sounds its rounded semitone");
    assert_eq!(bend(0.0), None, "no bend is not a bend");
    assert_eq!(bend(4.0), None, "past the deepest bend offered");
}

#[test]
fn natural_harmonics_sound_above_the_open_string() {
    let guitar = standard();
    let harmonic = |fret| {
        guitar.pitch_of(&FretAssignment { string: 6, fret, technique: StringTechnique::Harmonic })
    };
    assert_eq!(harmonic(12), Some(midi("E3")));
    assert_eq!(harmonic(7), Some(midi("B3")));
    assert_eq!(harmonic(5), Some(midi("E4")));
    assert_eq!(harmonic(3), None, "no natural harmonic is offered over fret 3");
}

#[test]
fn notes_held_together_need_different_strings_and_a_hands_reach() {
    let guitar = standard();
    assert!(!guitar.can_hold_together(&fret(3, 2), &fret(3, 4)), "one string, one note");
    assert!(guitar.can_hold_together(&fret(3, 2), &fret(2, 6)), "four frets apart");
    assert!(!guitar.can_hold_together(&fret(3, 2), &fret(2, 7)), "five is beyond the hand");
    assert!(guitar.can_hold_together(&fret(3, 9), &fret(1, 0)), "an open string is free");
    let harmonic = FretAssignment { string: 6, fret: 12, technique: StringTechnique::Harmonic };
    assert!(guitar.can_hold_together(&fret(3, 2), &harmonic), "a harmonic holds no fret down");
}

#[test]
fn reconcile_moves_a_clashing_note_to_the_nearest_string_that_fits() {
    let guitar = standard();
    let anchor = fret(3, 2); // A3
    let mut others = [fret(3, 4)]; // B3, on the anchor's own string
    guitar.reconcile(&anchor, &mut others);
    assert_eq!(others, [fret(2, 0)], "B3 moves to the open B string");
    assert_eq!(guitar.pitch_of(&others[0]), Some(midi("B3")));
}

#[test]
fn reconcile_keeps_a_note_that_already_fits_and_leaves_an_impossible_one() {
    let guitar = standard();
    let anchor = fret(6, 3);
    let mut others = [fret(5, 2), fret(6, 3)];
    guitar.reconcile(&anchor, &mut others);
    assert_eq!(others[0], fret(5, 2), "already fits, so untouched");
    // G2 lives only on string 6, which the anchor holds: nowhere to go.
    assert_eq!(others[1], fret(6, 3));
}

#[test]
fn reconciled_chord_can_all_be_held_together() {
    let guitar = standard();
    // An open E major chord, every note first offered on the lowest string
    // that reaches it, so they pile onto strings 6 and 5.
    let anchor = fret(6, 0);
    let pitches = ["B2", "E3", "G#3", "B3", "E4"];
    let lowest_string =
        |p: &&str| guitar.fretted_positions(midi(p)).into_iter().max_by_key(|a| a.string).unwrap();
    let mut others: Vec<FretAssignment> = pitches.iter().map(lowest_string).collect();
    guitar.reconcile(&anchor, &mut others);
    for (other, pitch) in others.iter().zip(pitches) {
        assert_eq!(guitar.pitch_of(other), Some(midi(pitch)), "{other:?} kept its pitch");
        assert!(guitar.can_hold_together(&anchor, other));
    }
    for (i, a) in others.iter().enumerate() {
        for b in &others[i + 1..] {
            assert!(guitar.can_hold_together(a, b), "{a:?} with {b:?}");
        }
    }
}

#[test]
fn strings_are_labelled_as_tab_writes_them() {
    assert_eq!(standard().string_labels(), ["e", "B", "G", "D", "A", "E"]);
    let drop_d = Guitar { strings: vec![64, 59, 55, 50, 45, 38], ..standard() };
    assert_eq!(drop_d.string_labels(), ["E", "B", "G", "D", "A", "D"]);
}

#[test]
fn the_highway_is_one_horizontal_row_per_string() {
    let guitar = standard();
    let layout = guitar.lane_layout();
    assert_eq!(layout.axis, ScrollAxis::Horizontal);
    assert_eq!(layout.lanes.len(), 6);
    assert_eq!(layout.lane_of(&PlayPosition::Fret(fret(1, 5))), Some(0));
    assert_eq!(layout.lane_of(&PlayPosition::Fret(fret(6, 5))), Some(5));
    assert_eq!(layout.lane_of(&PlayPosition::Fret(fret(7, 5))), None);
}

#[test]
fn a_guitar_decays_and_its_vibrato_is_per_string() {
    let guitar = standard();
    assert_eq!(guitar.envelope(), Envelope::Decaying);
    assert_eq!(guitar.expression_scope(), ExpressionScope::PerVoice);
}

#[test]
fn a_harmonica_position_means_nothing_to_a_guitar() {
    let guitar = standard();
    let hole = PlayPosition::natural_hole(4, crate::chart::Action::Blow);
    assert_eq!(InstrumentModel::pitch_of(&guitar, &hole), None);
    assert!(!guitar.can_sound_together(&hole, &PlayPosition::Fret(fret(1, 0))));
}

#[test]
fn the_instrument_enum_answers_as_its_guitar() {
    let guitar = standard();
    let instrument = Instrument::from(guitar.clone());
    assert_eq!(instrument.range(), Guitar::range(&guitar));
    assert_eq!(instrument.lane_layout(), guitar.lane_layout());
    assert_eq!(instrument.positions_for(midi("A2")), guitar.positions_for(midi("A2")));
}
