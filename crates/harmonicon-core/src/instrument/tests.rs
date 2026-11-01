// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use super::*;
use crate::harmonica::{
    chromatic_16_harp, chromatic_harp, country_tuned_harp, natural_minor_harp, paddy_richter_harp,
    richter_harp,
};
use crate::midi::note_to_midi;

fn every_harp() -> Vec<(&'static str, Harmonica)> {
    vec![
        ("richter C", richter_harp("C")),
        ("richter G", richter_harp("G")),
        ("country C", country_tuned_harp("C")),
        ("paddy C", paddy_richter_harp("C")),
        ("natural minor A", natural_minor_harp("A")),
        ("chromatic C", chromatic_harp("C")),
        ("chromatic 16 C", chromatic_16_harp("C")),
    ]
}

fn hole(hole: u8, action: Action, technique: Technique) -> PlayPosition {
    PlayPosition::Hole(HoleAssignment { hole, action, technique })
}

fn midi(note: &str) -> u8 {
    u8::try_from(note_to_midi(note).unwrap()).unwrap()
}

#[test]
fn every_position_sounds_the_pitch_it_was_found_for() {
    for (name, harp) in every_harp() {
        for target in 0..=127 {
            for position in harp.positions_for(target) {
                assert_eq!(harp.pitch_of(&position), Some(target), "{name}: {position:?}");
            }
        }
    }
}

/// The judge discards any detected pitch outside `build_valid_notes`, so
/// the model reaching a pitch that set lacks (or missing one it has) would
/// be a note the editor offers and the game can never score.
#[test]
fn reachable_pitches_are_exactly_the_judges_valid_notes() {
    for (name, harp) in every_harp() {
        let reachable: HashSet<u8> =
            (0..=127).filter(|&m| !harp.positions_for(m).is_empty()).collect();
        assert_eq!(reachable, harp.build_valid_notes(), "{name}");
    }
}

#[test]
fn range_spans_the_lowest_and_highest_valid_note() {
    for (name, harp) in every_harp() {
        let notes = harp.build_valid_notes();
        let expected = (*notes.iter().min().unwrap(), *notes.iter().max().unwrap());
        assert_eq!(harp.range(), Some(expected), "{name}");
    }
}

#[test]
fn pitch_of_follows_each_technique_on_a_c_harp() {
    let harp = richter_harp("C");
    assert_eq!(harp.pitch_of(&hole(4, Action::Blow, Technique::Natural)), Some(midi("C5")));
    assert_eq!(harp.pitch_of(&hole(3, Action::Draw, Technique::Bend(2.0))), Some(midi("A4")));
    assert_eq!(harp.pitch_of(&hole(10, Action::Blow, Technique::Bend(1.0))), Some(midi("B6")));
    assert_eq!(harp.pitch_of(&hole(6, Action::Blow, Technique::Overblow)), Some(midi("A#5")));
    assert_eq!(harp.pitch_of(&hole(7, Action::Draw, Technique::Overdraw)), Some(midi("C#6")));
}

#[test]
fn pitch_of_refuses_what_the_harp_cannot_play() {
    let harp = richter_harp("C");
    // Hole 1 draw bends one semitone, not two.
    assert_eq!(harp.pitch_of(&hole(1, Action::Draw, Technique::Bend(2.0))), None);
    // Low holes bend on the draw, not the blow.
    assert_eq!(harp.pitch_of(&hole(3, Action::Blow, Technique::Bend(1.0))), None);
    // An overblow is blown; drawing hole 6 that way is not a technique.
    assert_eq!(harp.pitch_of(&hole(6, Action::Draw, Technique::Overblow)), None);
    // Hole 2 is not an overblow hole.
    assert_eq!(harp.pitch_of(&hole(2, Action::Blow, Technique::Overblow)), None);
    assert_eq!(harp.pitch_of(&hole(11, Action::Blow, Technique::Natural)), None);
    // A diatonic has no slide.
    assert_eq!(harp.pitch_of(&hole(4, Action::Blow, Technique::Slide)), None);
}

/// The Song Editor steps bends by quarter tones; each sounds the semitone it
/// rounds to, which is the pitch gameplay expects for it.
#[test]
fn a_fractional_bend_sounds_the_semitone_it_rounds_to() {
    let harp = richter_harp("C");
    let draw3 = |depth| harp.pitch_of(&hole(3, Action::Draw, Technique::Bend(depth)));
    assert_eq!(draw3(0.5), Some(midi("A#4")));
    assert_eq!(draw3(1.5), Some(midi("A4")));
    assert_eq!(draw3(0.25), Some(midi("B4")), "rounds to no bend at all");
    // Hole 1 bends one semitone; 1.5 rounds past it.
    assert_eq!(harp.pitch_of(&hole(1, Action::Draw, Technique::Bend(1.5))), None);
}

#[test]
fn chromatic_slide_raises_the_reed_a_semitone() {
    let harp = chromatic_harp("C");
    assert_eq!(harp.pitch_of(&hole(1, Action::Blow, Technique::Slide)), Some(midi("C#4")));
    assert_eq!(harp.pitch_of(&hole(1, Action::Draw, Technique::Bend(1.0))), None);
}

#[test]
fn holes_sound_together_only_on_one_breath() {
    let harp = richter_harp("C");
    let blow4 = hole(4, Action::Blow, Technique::Natural);
    assert!(harp.can_sound_together(&blow4, &hole(5, Action::Blow, Technique::Natural)));
    assert!(!harp.can_sound_together(&blow4, &hole(5, Action::Draw, Technique::Natural)));
    assert!(!harp.can_sound_together(&blow4, &hole(4, Action::Blow, Technique::Natural)));
}

#[test]
fn reconcile_gives_every_overlapping_note_the_anchors_breath() {
    let harp = richter_harp("C");
    let anchor = hole(4, Action::Draw, Technique::Natural);
    let mut others =
        [hole(5, Action::Blow, Technique::Natural), hole(6, Action::Draw, Technique::Natural)];
    harp.reconcile(&anchor, &mut others);
    assert_eq!(
        others,
        [hole(5, Action::Draw, Technique::Natural), hole(6, Action::Draw, Technique::Natural)]
    );
}

#[test]
fn reconcile_drops_a_technique_the_new_breath_cannot_play() {
    let harp = richter_harp("C");
    let anchor = hole(4, Action::Blow, Technique::Natural);
    // Bent on the draw; blown, hole 3 doesn't bend at all.
    let mut others = [hole(3, Action::Draw, Technique::Bend(1.0))];
    harp.reconcile(&anchor, &mut others);
    assert_eq!(others, [hole(3, Action::Blow, Technique::Natural)]);
}

#[test]
fn reconciled_notes_can_all_sound_with_the_anchor() {
    let harp = richter_harp("C");
    let anchor = hole(2, Action::Draw, Technique::Natural);
    let mut others: Vec<PlayPosition> = (3..=10)
        .map(|h| hole(h, if h % 2 == 0 { Action::Draw } else { Action::Blow }, Technique::Natural))
        .collect();
    harp.reconcile(&anchor, &mut others);
    for other in &others {
        assert!(harp.can_sound_together(&anchor, other), "{other:?}");
        assert!(harp.pitch_of(other).is_some(), "{other:?}");
    }
}

#[test]
fn lanes_are_one_per_hole_numbered_from_one() {
    let layout = chromatic_harp("C").lane_layout();
    assert_eq!(layout.axis, ScrollAxis::Vertical);
    assert_eq!(layout.lanes.len(), 12);
    assert_eq!(layout.lanes[0].label, "1");
    assert_eq!(layout.lanes[11].label, "12");
    assert_eq!(layout.lane_of(&hole(1, Action::Blow, Technique::Natural)), Some(0));
    assert_eq!(layout.lane_of(&hole(12, Action::Draw, Technique::Slide)), Some(11));
    assert_eq!(layout.lane_of(&hole(13, Action::Blow, Technique::Natural)), None);
    assert_eq!(layout.lane_of(&hole(0, Action::Blow, Technique::Natural)), None);
}

#[test]
fn a_position_without_a_lane_is_drawn_in_the_nearest_one() {
    let layout = richter_harp("C").lane_layout();
    assert_eq!(layout.nearest_lane(&hole(4, Action::Blow, Technique::Natural)), 3);
    assert_eq!(layout.nearest_lane(&hole(0, Action::Blow, Technique::Natural)), 0);
    assert_eq!(layout.nearest_lane(&hole(99, Action::Draw, Technique::Natural)), 9);
}

/// The arithmetic the highway used inline before the layout owned it:
/// `(hole - 1) * lane + (lane - width) / 2` with `lane = 100 / holes`.
#[test]
fn a_centred_note_sits_in_the_middle_of_its_lane() {
    let layout = richter_harp("C").lane_layout();
    assert_eq!(layout.lane_pct(), 10.0);
    assert_eq!(layout.centred_left_pct(0, 10.0), 0.0);
    assert_eq!(layout.centred_left_pct(3, 6.0), 32.0);
    let chromatic = chromatic_harp("C").lane_layout();
    let lane = 100.0 / 12.0;
    assert_eq!(chromatic.centred_left_pct(11, lane * 0.5), 11.0 * lane + lane * 0.25);
}

#[test]
fn an_empty_layout_does_not_divide_by_zero() {
    let layout = LaneLayout { axis: ScrollAxis::Vertical, lanes: Vec::new() };
    assert_eq!(layout.lane_pct(), 100.0);
    assert_eq!(layout.nearest_lane(&hole(3, Action::Blow, Technique::Natural)), 0);
}

#[test]
fn only_the_diatonic_has_techniques_to_drill() {
    let diatonic = richter_harp("C").techniques();
    assert_eq!(diatonic[0], TechniqueKind::Target);
    assert!(diatonic.contains(&TechniqueKind::Glide));
    assert!(diatonic.contains(&TechniqueKind::Register));
    assert_eq!(chromatic_harp("C").techniques(), &[TechniqueKind::Target]);
}

#[test]
fn the_instrument_enum_answers_as_its_harmonica() {
    let harp = richter_harp("D");
    let instrument = Instrument::from(harp.clone());
    assert_eq!(instrument.range(), harp.range());
    assert_eq!(instrument.lane_layout(), harp.lane_layout());
    assert_eq!(instrument.positions_for(midi("A4")), harp.positions_for(midi("A4")));
    assert_eq!(instrument.envelope(), Envelope::Sustained);
    assert_eq!(instrument.expression_scope(), ExpressionScope::WholeInstrument);
}

#[test]
fn guitar_detection_includes_harmonics_and_respects_tuning_gaps() {
    let mut guitar = guitar::Guitar::standard(guitar::GuitarBody::Acoustic);
    let pitches = guitar.valid_pitches();
    assert!(pitches.contains(&88), "fifth-fret high-E harmonic sounds E6");
    assert_eq!(InstrumentModel::range(&guitar), Some((40, 88)));
    guitar.strings = vec![100, 40];
    guitar.frets = 5;
    let pitches = guitar.valid_pitches();
    assert!(!pitches.contains(&70), "a tuning gap is not a playable pitch");
    assert!(pitches.contains(&124));
    let mut filter =
        crate::note_tracker::VoiceFilter::for_instrument(&guitar.into(), &Default::default());
    assert_eq!(filter.allowed(&[124, 70, 40]), [40, 124]);
}

#[test]
fn a_flute_is_one_voice_on_pitch_lanes_with_its_fingerings_as_positions() {
    use flute::FootJoint;
    let flute = Flute::new(FootJoint::C);
    let positions = InstrumentModel::positions_for(&flute, 86);
    assert_eq!(positions.len(), 3, "D6: the standard fingering and two alternates");
    assert!(positions.iter().all(|p| InstrumentModel::pitch_of(&flute, p) == Some(86)));
    let missing = PlayPosition::Fingering(FingeringAssignment { note: 86, alternate: 3 });
    assert_eq!(InstrumentModel::pitch_of(&flute, &missing), None);
    assert!(!InstrumentModel::can_sound_together(&flute, &positions[0], &positions[1]));
    assert_eq!(InstrumentModel::range(&flute), Some((60, 96)));
    let lanes = InstrumentModel::lane_layout(&flute);
    assert_eq!(lanes.lanes.len(), 37);
    assert_eq!(lanes.lanes[0].label, "C4");
    assert_eq!(lanes.lane_of(&positions[0]), Some(26));
    let below = PlayPosition::Fingering(FingeringAssignment { note: 50, alternate: 0 });
    assert_eq!((lanes.lane_of(&below), lanes.nearest_lane(&below)), (None, 0));
}
