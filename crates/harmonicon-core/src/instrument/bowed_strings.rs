// SPDX-License-Identifier: MIT
//! Bowed, fretless strings. Pitch identity is independent of suggested fingers
//! and bow gestures. A microphone cannot verify those physical instructions.
//!
//! Tuning reference: Yamaha's violin/viola instrument guide and owner's manual:
//! https://hub.yamaha.com/strings/s-acoustic/whats-the-difference-between-violin-and-viola/
//! First-position reference: Yamaha, Violin / Fingering:
//! https://www.yamaha.com/en/musical_instrument_guide/violin/play/play003.html

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BowedKind {
    Violin,
}
impl BowedKind {
    pub const fn instrument_kind(self) -> super::InstrumentKind {
        match self {
            Self::Violin => super::InstrumentKind::Violin,
        }
    }

    pub fn standard(self) -> BowedStrings {
        match self {
            Self::Violin => {
                BowedStrings { kind: self, strings: vec![76, 69, 62, 55], stopped_semitones: 24 }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BowedStrings {
    pub kind: BowedKind,
    /// String 1 first, by physical order; pitches need not be sorted.
    pub strings: Vec<u8>,
    /// Supported stopped interval per string, not an anatomical limit or fret
    /// count. The initial violin adapter supports two octaves above each open.
    pub stopped_semitones: u8,
}

/// The sounding target and authored physical guidance. Notes above the basic
/// first-position patterns remain available without inventing a finger number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FingerboardPosition {
    pub string: u8,
    pub note: u8,
    /// 0 means open; 1–4 index through little finger. None is unspecified.
    pub finger: Option<u8>,
    /// A conventional position label, 1–7, supplied by the author. This hint
    /// does not change the pitch, and is not evidence of performed technique.
    pub hand_position: Option<u8>,
}

/// Placement stored alongside an event's sounding note, without duplicating it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BowedPlacement {
    pub string: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finger: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_position: Option<u8>,
}
impl BowedPlacement {
    pub fn at(self, note: u8) -> FingerboardPosition {
        FingerboardPosition {
            string: self.string,
            note,
            finger: self.finger,
            hand_position: self.hand_position,
        }
    }
}
impl FingerboardPosition {
    pub fn placement(self) -> BowedPlacement {
        BowedPlacement {
            string: self.string,
            finger: self.finger,
            hand_position: self.hand_position,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BowDirection {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BowArticulation {
    #[default]
    Detache,
    Legato,
    Staccato,
}

/// Authored bow guidance, separate from the left-hand placement. Detection
/// must never treat a correct pitch as proof of this gesture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BowGesture {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<BowDirection>,
    #[serde(default)]
    pub articulation: BowArticulation,
}

impl BowedStrings {
    pub fn is_valid(&self) -> bool {
        (1..=8).contains(&self.strings.len())
            && (1..=36).contains(&self.stopped_semitones)
            && self
                .strings
                .iter()
                .all(|&pitch| u16::from(pitch) + u16::from(self.stopped_semitones) <= 127)
    }
    pub fn open_pitch(&self, string: u8) -> Option<u8> {
        self.strings.get(usize::from(string.checked_sub(1)?)).copied().filter(|&pitch| pitch <= 127)
    }
    pub fn range(&self) -> Option<(u8, u8)> {
        if !self.is_valid() {
            return None;
        }
        Some((
            *self.strings.iter().min()?,
            self.strings.iter().max()?.checked_add(self.stopped_semitones)?,
        ))
    }
    pub fn pitch_of(&self, at: FingerboardPosition) -> Option<u8> {
        if !self.is_valid()
            || at.finger.is_some_and(|finger| finger > 4)
            || at.hand_position.is_some_and(|position| !(1..=7).contains(&position))
        {
            return None;
        }
        let open = self.open_pitch(at.string)?;
        let stopped = at.note.checked_sub(open)?;
        if stopped > self.stopped_semitones {
            return None;
        }
        if at.finger.is_some_and(|finger| (finger == 0) != (stopped == 0)) {
            return None;
        }
        // A stated first-position instruction must stay in that basic region.
        // Higher positions are author guidance; no guessed ergonomic solver.
        if at.hand_position == Some(1) && stopped > 8 {
            return None;
        }
        Some(at.note)
    }
    pub fn positions_for(&self, note: u8) -> Vec<FingerboardPosition> {
        let mut positions: Vec<_> = (1..=self.strings.len())
            .filter_map(|string| {
                let string = u8::try_from(string).ok()?;
                let stopped = note.checked_sub(self.open_pitch(string)?)?;
                let finger = match stopped {
                    0 => Some(0),
                    1 | 2 => Some(1),
                    3 | 4 => Some(2),
                    5 | 6 => Some(3),
                    7 => Some(4),
                    _ => None,
                };
                let at = FingerboardPosition {
                    string,
                    note,
                    finger,
                    hand_position: finger.filter(|&finger| finger > 0).map(|_| 1),
                };
                self.pitch_of(at).map(|_| at)
            })
            .collect();
        positions
            .sort_by_key(|at| (at.note - self.open_pitch(at.string).unwrap_or(at.note), at.string));
        positions
    }
    /// Necessary bow-contact constraint: two distinct adjacent strings.
    /// This does not certify the ergonomics of arbitrary authored fingerings.
    pub fn can_sound_together(&self, a: FingerboardPosition, b: FingerboardPosition) -> bool {
        self.pitch_of(a).is_some() && self.pitch_of(b).is_some() && a.string.abs_diff(b.string) == 1
    }
    /// Approximate equal-tempered stopping point measured from the nut, as a
    /// fraction of the vibrating open string. This is continuous geometry,
    /// not a set of imaginary frets, and is not a substitute for listening.
    pub fn stopping_fraction(&self, string: u8, pitch: f32) -> Option<f32> {
        if !self.is_valid() || !pitch.is_finite() {
            return None;
        }
        let interval = pitch - f32::from(self.open_pitch(string)?);
        (0.0..=f32::from(self.stopped_semitones))
            .contains(&interval)
            .then(|| 1.0 - 2.0_f32.powf(-interval / 12.0))
    }
}

impl super::InstrumentModel for BowedStrings {
    fn range(&self) -> Option<(u8, u8)> {
        BowedStrings::range(self)
    }
    fn positions_for(&self, midi: u8) -> Vec<super::PlayPosition> {
        BowedStrings::positions_for(self, midi)
            .into_iter()
            .map(super::PlayPosition::Fingerboard)
            .collect()
    }
    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        (0..=127).filter(|&pitch| !BowedStrings::positions_for(self, pitch).is_empty()).collect()
    }
    fn pitch_of(&self, position: &super::PlayPosition) -> Option<u8> {
        let super::PlayPosition::Fingerboard(at) = position else { return None };
        BowedStrings::pitch_of(self, *at)
    }
    fn can_sound_together(&self, a: &super::PlayPosition, b: &super::PlayPosition) -> bool {
        let (super::PlayPosition::Fingerboard(a), super::PlayPosition::Fingerboard(b)) = (a, b)
        else {
            return false;
        };
        BowedStrings::can_sound_together(self, *a, *b)
    }
    fn reconcile(&self, _: &super::PlayPosition, _: &mut [super::PlayPosition]) {}
    fn techniques(&self) -> &'static [super::TechniqueKind] {
        &[super::TechniqueKind::Target, super::TechniqueKind::Position]
    }
    fn expression_scope(&self) -> super::ExpressionScope {
        super::ExpressionScope::PerVoice
    }
    fn envelope(&self) -> super::Envelope {
        super::Envelope::Sustained
    }
    fn lane_layout(&self) -> super::LaneLayout {
        super::LaneLayout {
            axis: super::ScrollAxis::Horizontal,
            lanes: self
                .strings
                .iter()
                .map(|&pitch| super::Lane::new(crate::midi::midi_to_note(i32::from(pitch)), None))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_violin_has_sounding_tuning_and_multiple_locations() {
        let violin = BowedKind::Violin.standard();
        assert_eq!(violin.strings, [76, 69, 62, 55]);
        assert_eq!(violin.range(), Some((55, 100)));
        let choices = violin.positions_for(69);
        assert_eq!(choices[0].string, 2);
        assert_eq!(choices[0].finger, Some(0));
        assert!(choices.iter().any(|at| at.string == 3));
        assert!(violin.positions_for(54).is_empty());
        assert!(violin.positions_for(101).is_empty());
    }
    #[test]
    fn basic_first_position_suggestions_do_not_guess_high_fingerings() {
        let violin = BowedKind::Violin.standard();
        let b = violin.positions_for(71).into_iter().find(|at| at.string == 2).unwrap();
        assert_eq!((b.finger, b.hand_position), (Some(1), Some(1)));
        let high = violin.positions_for(96).into_iter().find(|at| at.string == 1).unwrap();
        assert_eq!((high.finger, high.hand_position), (None, None));
        assert_eq!(violin.pitch_of(FingerboardPosition { hand_position: Some(1), ..high }), None);
    }
    #[test]
    fn continuous_geometry_is_non_linear_and_rejects_invalid_input() {
        let violin = BowedKind::Violin.standard();
        assert_eq!(violin.stopping_fraction(2, 69.0), Some(0.0));
        assert_eq!(violin.stopping_fraction(2, 81.0), Some(0.5));
        assert_eq!(violin.stopping_fraction(2, 93.0), Some(0.75));
        let lower = violin.stopping_fraction(2, 70.0).unwrap();
        let upper = violin.stopping_fraction(2, 70.5).unwrap();
        assert!(lower < upper && upper < 0.5);
        for pitch in [f32::NAN, f32::INFINITY, 68.9, 94.0] {
            assert_eq!(violin.stopping_fraction(2, pitch), None);
        }
        assert_eq!(violin.stopping_fraction(0, 69.0), None);
    }
    #[test]
    fn bow_contact_requires_adjacent_strings_without_fretted_chord_assumptions() {
        let violin = BowedKind::Violin.standard();
        let open = |string| FingerboardPosition {
            string,
            note: violin.open_pitch(string).unwrap(),
            finger: Some(0),
            hand_position: None,
        };
        assert!(violin.can_sound_together(open(1), open(2)));
        assert!(!violin.can_sound_together(open(1), open(3)));
        assert!(!violin.can_sound_together(open(1), open(1)));
        assert_eq!(violin.pitch_of(FingerboardPosition { finger: Some(1), ..open(1) }), None);
    }
    #[test]
    fn geometry_respects_physical_order_and_independent_bow_metadata() {
        let mut setup = BowedKind::Violin.standard();
        setup.strings = vec![55, 76, 62, 69];
        assert_eq!(setup.open_pitch(1), Some(55));
        assert_eq!(setup.positions_for(55)[0].string, 1);
        let at = setup.positions_for(69)[0];
        let bow =
            BowGesture { direction: Some(BowDirection::Up), articulation: BowArticulation::Legato };
        let encoded = serde_json::to_string(&bow).unwrap();
        assert_eq!(serde_json::from_str::<BowGesture>(&encoded).unwrap(), bow);
        assert_eq!(setup.pitch_of(at), Some(69));
        setup.strings = vec![120];
        assert!(!setup.is_valid());
        assert!(setup.positions_for(120).is_empty());
    }
}
