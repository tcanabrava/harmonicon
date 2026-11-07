// SPDX-License-Identifier: MIT

//! Musical intent, independent of an editor row or screen coordinate.
use crate::instrument::{InstrumentModel, PlayPosition};
use crate::pitch_map::Technique;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteTarget {
    /// A pitch-authored note. An authored placement is a constraint, never
    /// permission to silently choose another fingering when it is invalid.
    Sounding { midi: f32, placement: Option<PlayPosition> },
    /// Tab-authored intent: changing tuning keeps the physical instruction.
    Positioned(PlayPosition),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    Unplayable,
    PitchMismatch,
}

impl NoteTarget {
    pub fn position(self) -> Option<PlayPosition> {
        match self {
            Self::Positioned(position) => Some(position),
            Self::Sounding { placement, .. } => placement,
        }
    }

    pub fn sounding_pitch(self, instrument: &(impl InstrumentModel + ?Sized)) -> Option<f32> {
        match self {
            Self::Sounding { midi, .. } => {
                (midi.is_finite() && (0.0..=127.0).contains(&midi)).then_some(midi)
            }
            Self::Positioned(position) => position_pitch(instrument, position),
        }
    }

    /// Positions a view or solver may propose without mutating the note.
    /// Authored positions are fixed constraints; only unplaced pitches get
    /// alternatives. A sequence solver can then account for held voices.
    pub fn candidates(
        self,
        instrument: &(impl InstrumentModel + ?Sized),
    ) -> Result<Vec<PlayPosition>, PlacementError> {
        if let Some(position) = self.position() {
            self.with_placement(instrument, position)?;
            return Ok(vec![position]);
        }
        let midi = self.sounding_pitch(instrument).ok_or(PlacementError::Unplayable)?;
        let candidates: Vec<_> = instrument
            .positions_for(midi.round() as u8)
            .into_iter()
            .filter(|position| {
                position_pitch(instrument, *position)
                    .is_some_and(|pitch| (pitch - midi).abs() < 0.001)
            })
            .collect();
        if candidates.is_empty() { Err(PlacementError::Unplayable) } else { Ok(candidates) }
    }

    /// Used on explicit fingering edits, not while painting a view.
    pub fn with_placement(
        self,
        instrument: &(impl InstrumentModel + ?Sized),
        position: PlayPosition,
    ) -> Result<Self, PlacementError> {
        if matches!(self, Self::Sounding { .. }) && self.sounding_pitch(instrument).is_none() {
            return Err(PlacementError::Unplayable);
        }
        let pitch = position_pitch(instrument, position).ok_or(PlacementError::Unplayable)?;
        match self {
            Self::Sounding { midi, .. } if (midi - pitch).abs() > 0.001 => {
                Err(PlacementError::PitchMismatch)
            }
            Self::Sounding { midi, .. } => Ok(Self::Sounding { midi, placement: Some(position) }),
            Self::Positioned(_) => Ok(Self::Positioned(position)),
        }
    }
}

/// Keep fractional bends for synthesis; core's integer mapping intentionally
/// rounds for detection and scoring.
fn position_pitch(
    instrument: &(impl InstrumentModel + ?Sized),
    position: PlayPosition,
) -> Option<f32> {
    use crate::instrument::guitar::StringTechnique;
    // Reject invalid authored positions instead of repairing them implicitly.
    instrument.pitch_of(&position)?;
    let (base, delta) = match position {
        PlayPosition::Hole(mut p) if matches!(p.technique, Technique::Bend(_)) => {
            let Technique::Bend(depth) = p.technique else { unreachable!() };
            p.technique = Technique::Natural;
            (PlayPosition::Hole(p), -depth)
        }
        PlayPosition::Fret(mut p) if matches!(p.technique, StringTechnique::Bend(_)) => {
            let StringTechnique::Bend(depth) = p.technique else { unreachable!() };
            p.technique = StringTechnique::Fretted;
            (PlayPosition::Fret(p), depth)
        }
        _ => (position, 0.0),
    };
    let midi = f32::from(instrument.pitch_of(&base)?) + delta;
    (midi.is_finite() && (0.0..=127.0).contains(&midi)).then_some(midi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::Action;
    use crate::harmonica::richter_harp;
    use crate::instrument::guitar::{FretAssignment, Guitar, GuitarBody, StringTechnique};

    fn fret(string: u8, fret: u8) -> PlayPosition {
        PlayPosition::Fret(FretAssignment { string, fret, technique: StringTechnique::Fretted })
    }

    #[test]
    fn two_guitar_fingerings_preserve_one_sounding_note() {
        let guitar = Guitar::standard(GuitarBody::Electric);
        let note = NoteTarget::Sounding { midi: 64.0, placement: None };
        let choices = note.candidates(&guitar).unwrap();
        assert!(choices.contains(&fret(1, 0)) && choices.contains(&fret(2, 5)));
        assert_eq!(note.position(), None, "view resolution cannot mutate the document");
        let high_e = note.with_placement(&guitar, fret(1, 0)).unwrap();
        let b_string = high_e.with_placement(&guitar, fret(2, 5)).unwrap();
        assert_eq!(high_e.sounding_pitch(&guitar), b_string.sounding_pitch(&guitar));
        assert_ne!(high_e.position(), b_string.position());
        assert_eq!(
            b_string.with_placement(&guitar, fret(2, 4)),
            Err(PlacementError::PitchMismatch)
        );
        assert_eq!(b_string.with_placement(&guitar, fret(8, 0)), Err(PlacementError::Unplayable));
        assert_eq!(b_string.position(), Some(fret(2, 5)));
    }

    #[test]
    fn tuning_changes_distinguish_pitch_intent_from_fingering_intent() {
        let c = richter_harp("C");
        let g = richter_harp("G");
        let position = PlayPosition::natural_hole(4, Action::Blow);
        let tab = NoteTarget::Positioned(position);
        let pitch = NoteTarget::Sounding { midi: 72.0, placement: Some(position) };
        assert_eq!(tab.sounding_pitch(&c), Some(72.0));
        assert_ne!(tab.sounding_pitch(&g), Some(72.0));
        assert_eq!(pitch.sounding_pitch(&g), Some(72.0));
        assert_eq!(pitch.with_placement(&g, position), Err(PlacementError::PitchMismatch));
        assert_eq!(pitch.candidates(&g), Err(PlacementError::PitchMismatch));
    }

    #[test]
    fn fractional_bends_are_not_rounded_for_audio() {
        let harp = richter_harp("C");
        let position = PlayPosition::Hole(crate::pitch_map::HoleAssignment {
            hole: 3,
            action: Action::Draw,
            technique: Technique::Bend(1.5),
        });
        assert_eq!(NoteTarget::Positioned(position).sounding_pitch(&harp), Some(69.5));
        let mut guitar = Guitar::standard(GuitarBody::Electric);
        guitar.capo = 2;
        let bend = PlayPosition::Fret(FretAssignment {
            string: 1,
            fret: 5,
            technique: StringTechnique::Bend(1.5),
        });
        assert_eq!(NoteTarget::Positioned(bend).sounding_pitch(&guitar), Some(72.5));
    }

    #[test]
    fn invalid_pitch_and_technique_are_never_silently_reassigned() {
        let guitar = Guitar::standard(GuitarBody::Electric);
        let pitch = NoteTarget::Sounding { midi: f32::NAN, placement: None };
        assert!(pitch.sounding_pitch(&guitar).is_none());
        assert_eq!(pitch.with_placement(&guitar, fret(1, 0)), Err(PlacementError::Unplayable));
        let impossible = PlayPosition::Fret(FretAssignment {
            string: 1,
            fret: 0,
            technique: StringTechnique::Bend(90.0),
        });
        assert!(NoteTarget::Positioned(impossible).sounding_pitch(&guitar).is_none());
        assert_eq!(
            NoteTarget::Positioned(impossible).with_placement(&guitar, fret(1, 0)),
            Ok(NoteTarget::Positioned(fret(1, 0)))
        );
    }
}
