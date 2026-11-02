// SPDX-License-Identifier: MIT

//! A fretted, six-string-style guitar: where each pitch lives on the neck,
//! and what a hand can hold at once.
//!
//! Conventions follow tablature and Guitar Pro, so a part read from either
//! keeps its own numbers:
//!
//! - **String 1 is the highest**, the top line of a tab staff.
//! - **Frets count from the capo.** On a guitar capoed at 2, fret 0 is the
//!   capo itself, as tab written for a capoed guitar has it.
//!
//! A microphone hears the pitch, not the string, so a position here is the
//! fingering a chart *suggests*; scoring compares pitches only.

use crate::midi::NOTE_NAMES;

pub mod fingering;

/// Electric or acoustic. They share the neck; the body changes what the
/// detector hears (an acoustic decays faster, an unplugged electric is
/// nearly silent), which is detection's business, not the model's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuitarBody {
    #[default]
    Electric,
    Acoustic,
}

/// One guitar as tuned and set up: its strings, neck length and capo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guitar {
    /// Each string's open pitch as a MIDI number, string 1 (highest) first.
    pub strings: Vec<u8>,
    /// Frets on the neck, counted from the nut.
    pub frets: u8,
    /// The fret the capo sits on, `0` for none.
    pub capo: u8,
    pub body: GuitarBody,
}

/// Standard tuning, string 1 first: E4 B3 G3 D3 A2 E2.
pub const STANDARD_TUNING: [u8; 6] = [64, 59, 55, 50, 45, 40];

/// A named six-string tuning a player can say their guitar is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    /// Stable name, also the localization key's suffix (`guitar-tuning-…`).
    pub id: &'static str,
    /// Open strings, string 1 (the highest) first, as MIDI.
    pub strings: [u8; 6],
}

/// The common six-string tunings, standard first.
pub const TUNINGS: &[Tuning] = &[
    Tuning { id: "standard", strings: STANDARD_TUNING },
    Tuning { id: "drop-d", strings: [64, 59, 55, 50, 45, 38] },
    Tuning { id: "half-step-down", strings: [63, 58, 54, 49, 44, 39] },
    Tuning { id: "dadgad", strings: [62, 57, 55, 50, 45, 38] },
    Tuning { id: "open-g", strings: [62, 59, 55, 50, 43, 38] },
    Tuning { id: "open-d", strings: [62, 57, 54, 50, 45, 38] },
];

/// The named tuning `strings` are in, if any.
pub fn tuning_of(strings: &[u8]) -> Option<&'static Tuning> {
    TUNINGS.iter().find(|t| t.strings[..] == *strings)
}

/// How far a fretting hand reaches across frets without moving: notes held
/// together may sit at most this many frets apart. Open strings are free.
pub const HAND_SPAN_FRETS: u8 = 4;

/// The deepest string bend offered, in semitones up.
pub const MAX_BEND: f32 = 3.0;

/// Natural harmonics: the fret a harmonic is touched over, and how far above
/// the open string it sounds. 12 is the octave, 7 the twelfth, 5 two octaves.
const HARMONICS: [(u8, u8); 3] = [(12, 12), (7, 19), (5, 24)];

/// How a fretted string is played to reach its pitch. Articulation (slides,
/// hammer-ons) and expression (vibrato) are chart modifiers, layered on top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StringTechnique {
    /// The fretted pitch itself.
    Fretted,
    /// Pushed *up* from the fretted pitch by this many semitones.
    Bend(f32),
    /// A natural harmonic touched over the fret (12, 7 or 5).
    Harmonic,
}

/// One way of playing a pitch on a guitar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FretAssignment {
    pub string: u8,
    pub fret: u8,
    pub technique: StringTechnique,
}

impl FretAssignment {
    /// The position as tab writes it: the fret (`7`), a bend with the fret
    /// whose pitch it reaches (`7b9`, a whole step up from the seventh), or
    /// a natural harmonic in angle brackets (`<12>`). The highway and the
    /// Song Editor both label notes with this, so they always agree.
    pub fn tab_label(&self) -> String {
        match self.technique {
            StringTechnique::Fretted => self.fret.to_string(),
            StringTechnique::Bend(depth) => {
                format!("{}b{}", self.fret, self.fret.saturating_add(depth.round() as u8))
            }
            StringTechnique::Harmonic => format!("<{}>", self.fret),
        }
    }
}

impl Guitar {
    /// A guitar in standard tuning with no capo.
    pub fn standard(body: GuitarBody) -> Self {
        // Electrics commonly have 22 frets; acoustics 20 reachable ones.
        let frets = match body {
            GuitarBody::Electric => 22,
            GuitarBody::Acoustic => 20,
        };
        Self { strings: STANDARD_TUNING.to_vec(), frets, capo: 0, body }
    }

    /// The pitch `string` sounds open, with the capo applied.
    pub fn open_pitch(&self, string: u8) -> Option<u8> {
        let open = *self.strings.get(usize::from(string.checked_sub(1)?))?;
        open.checked_add(self.capo)
    }

    /// The highest fret above the capo.
    pub fn top_fret(&self) -> u8 {
        self.frets.saturating_sub(self.capo)
    }

    /// The pitch `assignment` sounds, or `None` if this guitar can't play it.
    pub fn pitch_of(&self, a: &FretAssignment) -> Option<u8> {
        let open = self.open_pitch(a.string)?;
        if a.fret > self.top_fret() {
            return None;
        }
        match a.technique {
            StringTechnique::Fretted => open.checked_add(a.fret),
            StringTechnique::Bend(depth) => {
                let up = depth.round();
                if up <= 0.0 || up > MAX_BEND {
                    return None;
                }
                open.checked_add(a.fret)?.checked_add(up as u8)
            }
            StringTechnique::Harmonic => {
                let (_, above) = HARMONICS.iter().find(|(fret, _)| *fret == a.fret)?;
                open.checked_add(*above)
            }
        }
    }

    /// Every fretted position sounding `midi`, lowest fret first: open and
    /// low positions are what a beginner reaches for, and the order a
    /// fingering solver starts from.
    pub fn fretted_positions(&self, midi: u8) -> Vec<FretAssignment> {
        let mut out: Vec<FretAssignment> = (1..=self.string_count())
            .filter_map(|string| {
                let fret = midi.checked_sub(self.open_pitch(string)?)?;
                (fret <= self.top_fret()).then_some(FretAssignment {
                    string,
                    fret,
                    technique: StringTechnique::Fretted,
                })
            })
            .collect();
        out.sort_by_key(|a| (a.fret, a.string));
        out
    }

    pub fn string_count(&self) -> u8 {
        u8::try_from(self.strings.len()).unwrap_or(u8::MAX)
    }

    /// Lowest open string to the top fret of the highest one.
    pub fn range(&self) -> Option<(u8, u8)> {
        let low = (1..=self.string_count()).filter_map(|s| self.open_pitch(s)).min()?;
        let high = (1..=self.string_count())
            .filter_map(|s| self.open_pitch(s)?.checked_add(self.top_fret()))
            .max()?;
        Some((low, high))
    }

    /// Whether `a` and `b` can be held at once: different strings, and
    /// fretted notes within a hand's reach of each other. An open string or
    /// a harmonic needs no fretting finger held down for the span to count.
    pub fn can_hold_together(&self, a: &FretAssignment, b: &FretAssignment) -> bool {
        if a.string == b.string {
            return false;
        }
        let fretted = |x: &FretAssignment| x.fret > 0 && x.technique != StringTechnique::Harmonic;
        !(fretted(a) && fretted(b)) || a.fret.abs_diff(b.fret) <= HAND_SPAN_FRETS
    }

    /// Re-voices `others` so they can be held with `anchor` and with each
    /// other, keeping each note's pitch. A note already compatible keeps its
    /// fingering; one that isn't moves to the nearest fretted position (by
    /// fret, to the anchor's) that fits, and one with nowhere to go is left
    /// as it was for the author to resolve.
    pub fn reconcile(&self, anchor: &FretAssignment, others: &mut [FretAssignment]) {
        let mut held = vec![*anchor];
        for other in others.iter_mut() {
            if held.iter().all(|h| self.can_hold_together(h, other)) {
                held.push(*other);
                continue;
            }
            let Some(pitch) = self.pitch_of(other) else {
                continue;
            };
            let mut candidates = self.fretted_positions(pitch);
            candidates.sort_by_key(|c| (c.fret.abs_diff(anchor.fret), c.string));
            if let Some(fit) =
                candidates.into_iter().find(|c| held.iter().all(|h| self.can_hold_together(h, c)))
            {
                *other = fit;
                held.push(fit);
            }
        }
    }

    /// Each string's tab label, string 1 first: the open pitch class, with
    /// string 1 in lower case when it repeats the lowest string's name, as
    /// tab writes standard tuning's two Es (`e B G D A E`).
    pub fn string_labels(&self) -> Vec<String> {
        let name = |midi: u8| NOTE_NAMES[usize::from(midi % 12)].to_string();
        let mut labels: Vec<String> = self.strings.iter().map(|&m| name(m)).collect();
        if labels.len() > 1 && labels.first() == labels.last() {
            labels[0] = labels[0].to_lowercase();
        }
        labels
    }
}

#[cfg(test)]
mod tests;
