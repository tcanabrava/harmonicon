// SPDX-License-Identifier: MIT

use super::*;
use crate::instrument::guitar::GuitarBody;
use crate::midi::note_to_midi;

fn midi(note: &str) -> u8 {
    u8::try_from(note_to_midi(note).unwrap()).unwrap()
}

fn fretted(string: u8, fret: u8) -> FretAssignment {
    FretAssignment { string, fret, technique: StringTechnique::Fretted }
}

fn note(pitch: &str) -> Onset {
    Onset { notes: vec![(midi(pitch), None)] }
}

fn placed(pitch: &str, string: u8, fret: u8) -> Onset {
    Onset { notes: vec![(midi(pitch), Some(fretted(string, fret)))] }
}

fn guitar() -> Guitar {
    Guitar::standard(GuitarBody::Electric)
}

/// The reason for a solver: after A3 on the seventh fret, C4 is played a
/// couple of frets away (third string, fifth fret), not down at the first
/// fret where the lowest-fret rule would send the hand.
#[test]
fn the_hand_stays_near_where_it_is() {
    let onsets = [placed("A3", 4, 7), note("B3"), note("C4")];
    let chosen = fingering(&guitar(), &onsets);
    assert_eq!(chosen[0], Some(vec![fretted(4, 7)]), "the tab's own place is kept");
    assert_eq!(chosen[2], Some(vec![fretted(3, 5)]));
    assert_eq!(guitar().fretted_positions(midi("C4"))[0], fretted(2, 1), "not the lowest fret");
}

#[test]
fn an_open_string_costs_the_hand_nothing() {
    // B3 is the open second string; taking it doesn't move the hand.
    let chosen = fingering(&guitar(), &[placed("A3", 4, 7), note("B3")]);
    assert_eq!(chosen[1], Some(vec![fretted(2, 0)]));
}

#[test]
fn a_part_with_no_tab_starts_low() {
    let chosen = fingering(&guitar(), &[note("E2"), note("A2")]);
    assert_eq!(chosen, [Some(vec![fretted(6, 0)]), Some(vec![fretted(5, 0)])]);
}

#[test]
fn every_chosen_place_sounds_its_pitch_and_every_chord_can_be_held() {
    let guitar = guitar();
    let chord =
        |pitches: &[&str]| Onset { notes: pitches.iter().map(|p| (midi(p), None)).collect() };
    let onsets = [
        chord(&["E2", "B2", "E3", "G#3", "B3", "E4"]),
        chord(&["A2", "E3", "A3", "C#4", "E4"]),
        note("F#4"),
        chord(&["D3", "A3", "D4", "F#4"]),
    ];
    let chosen = fingering(&guitar, &onsets);
    for (onset, voicing) in onsets.iter().zip(&chosen) {
        let voicing = voicing.as_ref().expect("every onset here is playable");
        for (&(pitch, _), place) in onset.notes.iter().zip(voicing) {
            assert_eq!(guitar.pitch_of(place), Some(pitch), "{place:?}");
        }
        for (i, a) in voicing.iter().enumerate() {
            for b in &voicing[i + 1..] {
                assert!(guitar.can_hold_together(a, b), "{a:?} with {b:?}");
            }
        }
    }
}

#[test]
fn an_unreachable_onset_is_skipped_without_losing_the_hand() {
    let onsets = [placed("A3", 4, 7), note("C2"), note("C4")];
    let chosen = fingering(&guitar(), &onsets);
    assert_eq!(chosen[1], None, "below the low E");
    assert_eq!(chosen[2], Some(vec![fretted(3, 5)]), "still near the seventh fret");
}

#[test]
fn a_chord_no_hand_can_hold_is_unplayable() {
    // Two notes only the sixth string reaches.
    let onsets = [Onset { notes: vec![(midi("F2"), None), (midi("G2"), None)] }];
    assert_eq!(fingering(&guitar(), &onsets), [None]);
}

#[test]
fn a_placement_that_sounds_another_pitch_is_replaced() {
    // The open first string is E4, not A3, so that tab is wrong here.
    let chosen = fingering(&guitar(), &[placed("A3", 1, 0)]);
    assert_eq!(chosen, [Some(vec![fretted(3, 2)])]);
}

#[test]
fn nothing_in_nothing_out() {
    assert!(fingering(&guitar(), &[]).is_empty());
}
