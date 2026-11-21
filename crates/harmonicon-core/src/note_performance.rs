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
    Fretted(Fretting),
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
            _ => None,
        }
    }
    pub fn from_bow(bow: Option<BowGesture>) -> Self {
        bow.map(Self::Bowed).unwrap_or_default()
    }
    pub fn fretting(self) -> Option<Fretting> {
        match self {
            Self::Fretted(fretting) => Some(fretting),
            _ => None,
        }
    }
    pub fn from_fretting(fretting: Option<Fretting>) -> Self {
        fretting
            .filter(|value| *value != Fretting::default())
            .map(Self::Fretted)
            .unwrap_or_default()
    }
    pub fn from_event(event: &crate::chart::NoteEvent) -> Self {
        if let Some(fretting) = event.fretting {
            Self::from_fretting(Some(fretting))
        } else {
            Self::from_bow(event.bow)
        }
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

/// Fretting-hand guidance. Fingers 1–4 are index, middle, ring and little.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fretting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finger: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barre: Option<Barre>,
}

/// Index-finger barre, with a contiguous span in tab string order. The
/// fret is relative to any physical capo, just like the note's fret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Barre {
    pub fret: u8,
    pub first_string: u8,
    pub last_string: u8,
}
impl Barre {
    pub fn label(self) -> String {
        let mut remaining = self.fret;
        let mut roman = String::new();
        for (value, symbol) in [
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ] {
            while remaining >= value {
                roman.push_str(symbol);
                remaining -= value;
            }
        }
        format!("C{roman} ({first}–{last})", first = self.first_string, last = self.last_string)
    }
}
impl Fretting {
    /// A barre is anchored to a note actually stopped by the index finger.
    /// Other notes at that onset retain their own sounded frets and fingers.
    pub fn valid_for(self, at: crate::chart::FretPosition, string_count: usize) -> bool {
        at.fret > 0
            && self.finger.is_none_or(|finger| (1..=4).contains(&finger))
            && self.barre.is_none_or(|barre| {
                self.finger.is_none_or(|finger| finger == 1)
                    && barre.fret == at.fret
                    && barre.first_string > 0
                    && barre.first_string < barre.last_string
                    && usize::from(barre.last_string) <= string_count
                    && (barre.first_string..=barre.last_string).contains(&at.string)
            })
    }

    pub fn annotate(self, tab: String) -> String {
        if let Some(barre) = self.barre {
            format!("{tab} · {}", barre.label())
        } else if let Some(finger) = self.finger {
            format!("{tab} · ({finger})")
        } else {
            tab
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
