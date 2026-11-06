// SPDX-License-Identifier: MIT
//! Basic Boehm soprano clarinet finger contacts, written E3–C6.
//!
//! Fingerings verified against the Woodwind Fingering Guide's basic
//! chalumeau/clarion tables (https://www.wfg.woodwind.org/clarinet/cl_bas_1.html
//! and cl_bas_2.html). These are finger contacts, not every linked pad.
//! Altissimo and other key systems are deliberately outside this table.
use super::wind::*;
use crate::notation::{NotationContext, Transposition};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClarinetKey {
    #[default]
    Bb,
    A,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clarinet {
    pub key: ClarinetKey,
}

impl Clarinet {
    pub fn notation(self) -> NotationContext {
        NotationContext {
            transposition: match self.key {
                ClarinetKey::Bb => Transposition::B_FLAT,
                ClarinetKey::A => Transposition::A,
            },
        }
    }
    fn sounding_pitch(self, written: u8) -> u8 {
        (i16::from(written) + i16::from(self.notation().transposition.semitones)) as u8
    }
    pub fn pitch_of(self, position: FingeringAssignment) -> Option<u8> {
        self.pattern(position).map(|_| position.note)
    }
    pub fn range(self) -> std::ops::RangeInclusive<u8> {
        self.sounding_pitch(52)..=self.sounding_pitch(84)
    }
}

// Stable local IDs. The six main holes and thumb hole are circles; levers
// are separate oval controls in the teaching presentation.
pub const THUMB: u64 = 1 << 0;
pub const REGISTER: u64 = 1 << 1;
const L1: u64 = 1 << 2;
const L2: u64 = 1 << 3;
const L3: u64 = 1 << 4;
const R1: u64 = 1 << 5;
const R2: u64 = 1 << 6;
const R3: u64 = 1 << 7;
const LEFT_E: u64 = 1 << 8;
const LEFT_F: u64 = 1 << 9;
const LEFT_FS: u64 = 1 << 10;
const LEFT_CS: u64 = 1 << 11;
const RIGHT_E: u64 = 1 << 12;
const RIGHT_F: u64 = 1 << 13;
const RIGHT_FS: u64 = 1 << 14;
const RIGHT_GS: u64 = 1 << 15;
const THROAT_GS: u64 = 1 << 16;
const THROAT_A: u64 = 1 << 17;
const SIDE_EB: u64 = 1 << 18;
const LH: u64 = THUMB | L1 | L2 | L3;
const ALL_HOLES: u64 = LH | R1 | R2 | R3;

// Written MIDI and primary/alternate finger contacts. Only common, verified
// alternatives are included; alternate indices are stable chart identities.
const LOW: &[(u8, &[u64])] = &[
    (52, &[ALL_HOLES | LEFT_E, ALL_HOLES | RIGHT_E]),
    (53, &[ALL_HOLES | RIGHT_F, ALL_HOLES | LEFT_F]),
    (54, &[ALL_HOLES | LEFT_FS, ALL_HOLES | RIGHT_FS]),
    (55, &[ALL_HOLES]),
    (56, &[ALL_HOLES | RIGHT_GS]),
    (57, &[LH | R1 | R2]),
    (58, &[LH | R1]),
    (59, &[LH | R2]),
    (60, &[LH]),
    (61, &[LH | LEFT_CS]),
    (62, &[THUMB | L1 | L2]),
    (63, &[THUMB | L1 | L2 | SIDE_EB]),
    (64, &[THUMB | L1]),
    (65, &[THUMB]),
    (66, &[L1]),
    (67, &[0]),
    (68, &[THROAT_GS]),
    (69, &[THROAT_A]),
    (70, &[REGISTER | THROAT_A]),
];

impl FingeringChart for Clarinet {
    fn controls(&self) -> Vec<WindControl> {
        use ControlKind::*;
        use Finger::*;
        let controls = [
            ("Thumb", MainHole, LeftThumb),
            ("Register", RegisterKey, LeftThumb),
            ("L1", MainHole, LeftIndex),
            ("L2", MainHole, LeftMiddle),
            ("L3", MainHole, LeftRing),
            ("R1", MainHole, RightIndex),
            ("R2", MainHole, RightMiddle),
            ("R3", MainHole, RightRing),
            ("L E/B", AuxiliaryKey, LeftLittle),
            ("L F/C", AuxiliaryKey, LeftLittle),
            ("L F♯/C♯", AuxiliaryKey, LeftLittle),
            ("L C♯/G♯", AuxiliaryKey, LeftLittle),
            ("R E/B", AuxiliaryKey, RightLittle),
            ("R F/C", AuxiliaryKey, RightLittle),
            ("R F♯/C♯", AuxiliaryKey, RightLittle),
            ("R G♯/D♯", AuxiliaryKey, RightLittle),
            ("G♯", AuxiliaryKey, LeftIndex),
            ("A", AuxiliaryKey, LeftIndex),
            ("E♭/B♭ side", AuxiliaryKey, RightIndex),
        ];
        controls
            .into_iter()
            .enumerate()
            .map(|(index, (label, kind, finger))| WindControl {
                id: ControlId(index as u8),
                label,
                kind,
                finger: Some(finger),
            })
            .collect()
    }
    fn patterns(&self) -> Vec<FingeringPattern> {
        let mut patterns = Vec::new();
        for &(written, contacts) in LOW {
            for (alternate, &pressed) in contacts.iter().enumerate() {
                patterns.push(FingeringPattern {
                    position: FingeringAssignment {
                        note: self.sounding_pitch(written),
                        alternate: alternate as u8,
                    },
                    pressed: ControlMask(pressed),
                });
                // The clarion is a twelfth above the lower fingerings E3–F4.
                if written <= 65 {
                    patterns.push(FingeringPattern {
                        position: FingeringAssignment {
                            note: self.sounding_pitch(written + 19),
                            alternate: alternate as u8,
                        },
                        pressed: ControlMask(pressed | REGISTER),
                    });
                }
            }
        }
        patterns.sort_by_key(|pattern| (pattern.position.note, pattern.position.alternate));
        patterns
    }
    fn pattern(&self, at: FingeringAssignment) -> Option<ControlMask> {
        let written = self.notation().transposition.written_semitone(at.note);
        let (lower, register) =
            if (71..=84).contains(&written) { (written - 19, REGISTER) } else { (written, 0) };
        let (_, contacts) = LOW.iter().find(|(pitch, _)| i16::from(*pitch) == lower)?;
        contacts.get(usize::from(at.alternate)).map(|&pressed| ControlMask(pressed | register))
    }
    fn register_rule(&self) -> Option<RegisterRule> {
        Some(RegisterRule { semitones: 19, ignored_controls: ControlMask(REGISTER) })
    }
}

impl super::InstrumentModel for Clarinet {
    fn range(&self) -> Option<(u8, u8)> {
        let range = Clarinet::range(*self);
        Some((*range.start(), *range.end()))
    }
    fn positions_for(&self, midi: u8) -> Vec<super::PlayPosition> {
        positions_for(self, midi).into_iter().map(super::PlayPosition::Fingering).collect()
    }
    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        Clarinet::range(*self).collect()
    }
    fn pitch_of(&self, position: &super::PlayPosition) -> Option<u8> {
        let super::PlayPosition::Fingering(at) = position else { return None };
        Clarinet::pitch_of(*self, *at)
    }
    fn can_sound_together(&self, _: &super::PlayPosition, _: &super::PlayPosition) -> bool {
        false
    }
    fn reconcile(&self, _: &super::PlayPosition, _: &mut [super::PlayPosition]) {}
    fn techniques(&self) -> &'static [super::TechniqueKind] {
        use super::TechniqueKind::*;
        &[Target, Register, Position]
    }
    fn expression_scope(&self) -> super::ExpressionScope {
        super::ExpressionScope::WholeInstrument
    }
    fn envelope(&self) -> super::Envelope {
        super::Envelope::Sustained
    }
    fn lane_layout(&self) -> super::LaneLayout {
        // One arrival lane: the score and finger-contact diagram carry pitch
        // detail instead of squeezing 33 pitches into narrow columns.
        super::LaneLayout {
            axis: super::ScrollAxis::Vertical,
            lanes: vec![super::Lane::new(String::new(), None)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_written_range_is_complete_and_alternates_are_stable() {
        for key in [ClarinetKey::Bb, ClarinetKey::A] {
            let clarinet = Clarinet { key };
            let controls = clarinet.controls();
            for pitch in clarinet.range() {
                let positions = positions_for(&clarinet, pitch);
                assert!(!positions.is_empty(), "missing sounding pitch {pitch}");
                for (index, position) in positions.iter().enumerate() {
                    assert_eq!(position.alternate, index as u8);
                    let mask = clarinet.pattern(*position).unwrap();
                    assert_eq!(mask.0 >> controls.len(), 0);
                    assert_eq!(clarinet.pitch_of(*position), Some(pitch));
                }
            }
            assert!(positions_for(&clarinet, *clarinet.range().start() - 1).is_empty());
            assert!(positions_for(&clarinet, *clarinet.range().end() + 1).is_empty());
        }
    }
    #[test]
    fn register_exercises_are_twelfths_not_octaves_or_throat_semitones() {
        let clarinet = Clarinet::default();
        let pairs = register_pairs(&clarinet);
        assert_eq!(pairs.len(), 14);
        for (low, high) in pairs {
            assert_eq!(high.note - low.note, 19);
            assert!(!clarinet.pattern(low).unwrap().contains(ControlId(1)));
            assert!(clarinet.pattern(high).unwrap().contains(ControlId(1)));
        }
    }
    #[test]
    fn runtime_model_uses_sounding_pitch_and_one_voice() {
        use super::super::{Instrument, InstrumentModel};
        let instrument: Instrument = Clarinet::default().into();
        assert_eq!(instrument.kind(), super::super::InstrumentKind::Clarinet);
        assert_eq!(InstrumentModel::range(&instrument), Some((50, 82)));
        let low = instrument.positions_for(50)[0];
        let other = instrument.positions_for(51)[0];
        assert_eq!(instrument.pitch_of(&low), Some(50));
        assert!(!instrument.can_sound_together(&low, &other));
        assert_eq!(
            instrument
                .notation()
                .written_pitch(50, crate::notation::KeySignature::default())
                .unwrap()
                .name(),
            "E3"
        );
        assert_eq!(instrument.synth_voice(), crate::synth::Voice::Clarinet);
    }

    #[test]
    fn direct_lookup_matches_the_table_and_rejects_invalid_variants() {
        for key in [ClarinetKey::Bb, ClarinetKey::A] {
            let clarinet = Clarinet { key };
            for pattern in clarinet.patterns() {
                assert_eq!(clarinet.pattern(pattern.position), Some(pattern.pressed));
            }
            for note in [0, 127, 255] {
                assert!(clarinet.pattern(FingeringAssignment { note, alternate: 0 }).is_none());
            }
            assert!(
                clarinet
                    .pattern(FingeringAssignment {
                        note: *clarinet.range().start(),
                        alternate: 255,
                    })
                    .is_none()
            );
        }
    }
    #[test]
    fn b_natural_and_throat_b_flat_have_their_distinct_contacts() {
        let clarinet = Clarinet::default();
        let mask = |written| {
            clarinet
                .pattern(FingeringAssignment {
                    note: clarinet.sounding_pitch(written),
                    alternate: 0,
                })
                .unwrap()
                .0
        };
        assert_eq!(mask(59), LH | R2);
        assert_eq!(mask(70), REGISTER | THROAT_A);
        assert_eq!(mask(66), L1);
    }
}
