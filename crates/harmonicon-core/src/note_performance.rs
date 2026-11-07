// SPDX-License-Identifier: MIT
//! Optional performance instructions carried by any note. Instrument families
//! own their typed variants; adding one does not add fields to editor or judge
//! note records. None is the stable default for existing constructors.
use crate::instrument::bowed_strings::{BowArticulation, BowGesture};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PerformanceInstructions {
    #[default]
    None,
    Bowed(BowGesture),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SustainPolicy {
    TrackHold,
    OnsetOnly,
}
impl PerformanceInstructions {
    pub fn bowed(self) -> Option<BowGesture> {
        match self {
            Self::Bowed(bow) => Some(bow),
            Self::None => None,
        }
    }
    pub fn from_bow(bow: Option<BowGesture>) -> Self {
        bow.map(Self::Bowed).unwrap_or_default()
    }
    /// A separate scoring policy prevents the judge from knowing bow types.
    pub fn sustain_policy(self) -> SustainPolicy {
        match self {
            Self::Bowed(BowGesture { articulation: BowArticulation::Staccato, .. }) => {
                SustainPolicy::OnsetOnly
            }
            _ => SustainPolicy::TrackHold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_notes_keep_hold_scoring_and_optional_guidance_does_not_assess_direction() {
        assert_eq!(PerformanceInstructions::default().sustain_policy(), SustainPolicy::TrackHold);
        for direction in [
            None,
            Some(crate::instrument::bowed_strings::BowDirection::Up),
            Some(crate::instrument::bowed_strings::BowDirection::Down),
        ] {
            let bow = BowGesture { direction, articulation: BowArticulation::Staccato };
            let instructions = PerformanceInstructions::from_bow(Some(bow));
            assert_eq!(instructions.bowed(), Some(bow));
            assert_eq!(instructions.sustain_policy(), SustainPolicy::OnsetOnly);
        }
    }
}
