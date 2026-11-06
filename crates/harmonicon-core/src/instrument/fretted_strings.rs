// SPDX-License-Identifier: MIT

//! Shared fretted-string geometry, independent of instrument identity,
//! technique policy, ergonomics, notation and presentation.
//!
//! A borrowed view lets existing guitar setups keep their serialized/public
//! fields while other instruments can compose the same mechanics. String IDs
//! are one-based in the setup's order; tuning need not be monotonic.

/// Identity is separate from the shared string/fret mechanics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrettedKind {
    Guitar,
    Bass,
}

impl FrettedKind {
    pub const fn instrument_kind(self) -> super::InstrumentKind {
        match self {
            Self::Guitar => super::InstrumentKind::Guitar,
            Self::Bass => super::InstrumentKind::Bass,
        }
    }
}

/// Owned setup shared by fretted instruments. Technique and teaching policies
/// use `kind`; the geometry view below is independent of those policies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrettedInstrument {
    pub kind: FrettedKind,
    pub strings: Vec<u8>,
    pub frets: u8,
    pub capo: u8,
    pub body: super::guitar::GuitarBody,
}

#[derive(Clone, Copy, Debug)]
pub struct FrettedStrings<'a> {
    pub strings: &'a [u8],
    /// Physical fret count from the nut.
    pub frets: u8,
    pub capo: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FretPosition {
    pub string: u8,
    /// Fret relative to the capo, with zero meaning open.
    pub fret: u8,
}

impl FrettedStrings<'_> {
    pub fn string_count(&self) -> u8 {
        u8::try_from(self.strings.len()).unwrap_or(u8::MAX)
    }

    pub fn top_fret(&self) -> u8 {
        self.frets.saturating_sub(self.capo)
    }

    pub fn open_pitch(&self, string: u8) -> Option<u8> {
        let open = *self.strings.get(usize::from(string.checked_sub(1)?))?;
        open.checked_add(self.capo)
    }

    /// Discrete stopped/open pitch. Bends and harmonics belong to the
    /// instrument's technique policy, not to neck geometry.
    pub fn pitch_at(&self, position: FretPosition) -> Option<u8> {
        if position.fret > self.top_fret() {
            return None;
        }
        self.open_pitch(position.string)?.checked_add(position.fret)
    }

    /// Lowest fret first, then setup string order; makes no claim about
    /// which fingering is easiest for a particular instrument or player.
    pub fn positions_for(&self, pitch: u8) -> Vec<FretPosition> {
        let mut positions: Vec<_> = (1..=self.string_count())
            .filter_map(|string| {
                let fret = pitch.checked_sub(self.open_pitch(string)?)?;
                (fret <= self.top_fret()).then_some(FretPosition { string, fret })
            })
            .collect();
        positions.sort_by_key(|position| (position.fret, position.string));
        positions
    }

    pub fn range(&self) -> Option<(u8, u8)> {
        let low = (1..=self.string_count()).filter_map(|string| self.open_pitch(string)).min()?;
        let high = (1..=self.string_count())
            .filter_map(|string| self.pitch_at(FretPosition { string, fret: self.top_fret() }))
            .max()?;
        Some((low, high))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_and_seven_string_necks_have_no_six_string_assumption() {
        for strings in [&[43, 38, 33, 28][..], &[64, 59, 55, 50, 45, 40, 35][..]] {
            let neck = FrettedStrings { strings, frets: 24, capo: 0 };
            assert_eq!(usize::from(neck.string_count()), strings.len());
            for string in 1..=neck.string_count() {
                for fret in 0..=24 {
                    let position = FretPosition { string, fret };
                    let pitch = neck.pitch_at(position).unwrap();
                    assert!(neck.positions_for(pitch).contains(&position));
                }
            }
        }
    }

    #[test]
    fn reentrant_tuning_keeps_physical_string_ids_and_finds_actual_range() {
        let neck = FrettedStrings { strings: &[67, 60, 64, 69], frets: 12, capo: 0 };
        assert_eq!(neck.range(), Some((60, 81)));
        assert_eq!(
            neck.positions_for(67),
            [
                FretPosition { string: 1, fret: 0 },
                FretPosition { string: 3, fret: 3 },
                FretPosition { string: 2, fret: 7 }
            ]
        );
    }

    #[test]
    fn capo_and_bounds_are_shared_without_technique_or_hand_span_rules() {
        let neck = FrettedStrings { strings: &[64, 59], frets: 20, capo: 2 };
        assert_eq!(neck.top_fret(), 18);
        assert_eq!(neck.open_pitch(1), Some(66));
        assert_eq!(neck.pitch_at(FretPosition { string: 1, fret: 18 }), Some(84));
        assert_eq!(neck.pitch_at(FretPosition { string: 1, fret: 19 }), None);
        assert_eq!(neck.open_pitch(0), None);
        assert_eq!(neck.open_pitch(3), None);
        assert!(neck.positions_for(58).is_empty());
        assert_eq!(FrettedStrings { strings: &[], frets: 20, capo: 0 }.range(), None);
        assert_eq!(FrettedStrings { strings: &[255], frets: 20, capo: 1 }.open_pitch(1), None);
    }
}
