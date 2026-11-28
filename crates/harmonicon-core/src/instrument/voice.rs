// SPDX-License-Identifier: MIT
//! A singing voice. Nothing about a sung note needs placing beyond its pitch,
//! so a note's position *is* its pitch ([`PlayPosition::Sung`]), and one voice
//! sings one note at a time. What differs between singers is how far the voice
//! reaches comfortably: the [`VoiceType`]s are the choral classifications.
//!
//! Range reference: the choral ranges in Wikipedia's "Voice type" article
//! (soprano C4–A5, mezzo-soprano A3–F5, alto F3–D5, tenor C3–A4, baritone
//! A2–F4, bass E2–E4) — a comfortable span to train in, not a singer's
//! absolute limits. Notes are written at sounding pitch; the score's octave
//! convention for a tenor line is the notation layer's business.
//!
//! [`PlayPosition::Sung`]: super::PlayPosition::Sung

/// A choral voice classification, by its comfortable range.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum VoiceType {
    Soprano,
    Mezzo,
    Alto,
    /// The default: a tenor's range sits in the middle of the six, where
    /// most untrained adult voices overlap.
    #[default]
    Tenor,
    Baritone,
    Bass,
}

impl VoiceType {
    pub const ALL: [Self; 6] =
        [Self::Soprano, Self::Mezzo, Self::Alto, Self::Tenor, Self::Baritone, Self::Bass];

    /// Stable name, used in charts, settings and localization keys.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Soprano => "soprano",
            Self::Mezzo => "mezzo",
            Self::Alto => "alto",
            Self::Tenor => "tenor",
            Self::Baritone => "baritone",
            Self::Bass => "bass",
        }
    }

    pub fn from_slug(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == value)
    }

    /// Lowest and highest comfortable note, as MIDI.
    pub const fn range(self) -> (u8, u8) {
        match self {
            Self::Soprano => (60, 81),  // C4–A5
            Self::Mezzo => (57, 77),    // A3–F5
            Self::Alto => (53, 74),     // F3–D5
            Self::Tenor => (48, 69),    // C3–A4
            Self::Baritone => (45, 65), // A2–F4
            Self::Bass => (40, 64),     // E2–E4
        }
    }
}

/// A singer of one [`VoiceType`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SingingVoice {
    pub voice_type: VoiceType,
}

impl SingingVoice {
    pub fn new(voice_type: VoiceType) -> Self {
        Self { voice_type }
    }

    pub fn lowest(&self) -> u8 {
        self.voice_type.range().0
    }

    pub fn highest(&self) -> u8 {
        self.voice_type.range().1
    }

    /// Whether `midi` is inside the voice's comfortable range.
    pub fn has(&self, midi: u8) -> bool {
        (self.lowest()..=self.highest()).contains(&midi)
    }

    /// The slides the Note Trainer drills: from each note, up a major third
    /// and up a perfect fifth, where the voice reaches both ends. Sliding
    /// through the pitches in between, rather than jumping, is the first
    /// pitch-control exercise a singer learns ("sirens").
    pub fn slides(&self) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        for start in self.lowest()..=self.highest() {
            for interval in [4, 7] {
                let target = start + interval;
                if self.has(target) {
                    out.push((start, target));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_voice_type_spans_at_least_an_octave_and_a_half() {
        for kind in VoiceType::ALL {
            let (low, high) = kind.range();
            assert!(high - low >= 18, "{kind:?}");
            assert_eq!(VoiceType::from_slug(kind.slug()), Some(kind));
        }
        assert_eq!(VoiceType::Tenor.range(), (48, 69), "C3 to A4");
    }

    #[test]
    fn slides_stay_inside_the_range() {
        let voice = SingingVoice::new(VoiceType::Bass);
        let slides = voice.slides();
        assert!(slides.contains(&(40, 44)) && slides.contains(&(40, 47)));
        assert!(slides.iter().all(|&(a, b)| voice.has(a) && voice.has(b) && b > a));
        assert!(!slides.iter().any(|&(a, _)| a == voice.highest()));
    }
}
