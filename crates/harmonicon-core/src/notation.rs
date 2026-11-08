// SPDX-License-Identifier: MIT
//! Written notation is a projection of sounding pitch, never the identity
//! used by the detector, judge or synthesizer. Chromatic and diatonic offsets
//! are both explicit so transposition preserves note spelling.

const NATURALS: [i16; 7] = [0, 2, 4, 5, 7, 9, 11];
const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpelledPitch {
    /// C=0, D=1, …, B=6.
    pub letter: u8,
    pub accidental: i8,
    pub octave: i8,
}
impl SpelledPitch {
    pub fn new(letter: u8, accidental: i8, octave: i8) -> Option<Self> {
        let pitch = Self { letter, accidental, octave };
        (letter < 7 && (-2..=2).contains(&accidental)).then_some(pitch)
    }
    pub fn parse(text: &str) -> Option<Self> {
        let mut chars = text.trim().chars().peekable();
        let letter =
            LETTERS.iter().position(|&letter| Some(letter) == chars.peek().copied())? as u8;
        chars.next();
        let mut accidental = 0;
        while let Some(&next) = chars.peek() {
            match next {
                '#' | '♯' if accidental >= 0 => accidental += 1,
                'b' | '♭' if accidental <= 0 => accidental -= 1,
                _ => break,
            }
            chars.next();
            if !(-2..=2).contains(&accidental) {
                return None;
            }
        }
        let octave = chars.collect::<String>().parse::<i8>().ok()?;
        Self::new(letter, accidental, octave)
    }
    /// Absolute semitone coordinate, allowing written notation outside the
    /// detector's MIDI range (e.g. a transposing instrument's high notes).
    pub fn semitone(self) -> Option<i16> {
        if !(-2..=2).contains(&self.accidental) {
            return None;
        }
        Some(
            (i16::from(self.octave) + 1) * 12
                + *NATURALS.get(usize::from(self.letter))?
                + i16::from(self.accidental),
        )
    }
    pub fn midi(self) -> Option<u8> {
        u8::try_from(self.semitone()?).ok().filter(|&midi| midi <= 127)
    }
    pub fn diatonic_step(self) -> i16 {
        i16::from(self.octave) * 7 + i16::from(self.letter)
    }
    pub fn name(self) -> String {
        let letter = LETTERS.get(usize::from(self.letter)).copied().unwrap_or('?');
        let mark = if self.accidental < 0 { "b" } else { "#" };
        format!(
            "{letter}{}{}",
            mark.repeat(usize::from(self.accidental.unsigned_abs())),
            self.octave
        )
    }
}

/// Add these intervals to a written note to obtain its sounding pitch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transposition {
    pub semitones: i8,
    pub diatonic_steps: i8,
}
impl Transposition {
    pub const CONCERT: Self = Self { semitones: 0, diatonic_steps: 0 };
    pub const OCTAVE_DOWN: Self = Self { semitones: -12, diatonic_steps: -7 };
    pub const B_FLAT: Self = Self { semitones: -2, diatonic_steps: -1 };
    pub const A: Self = Self { semitones: -3, diatonic_steps: -2 };
    fn apply(self, pitch: SpelledPitch, direction: i16) -> Option<SpelledPitch> {
        let midi = pitch.semitone()? + direction * i16::from(self.semitones);
        let step = pitch.diatonic_step() + direction * i16::from(self.diatonic_steps);
        let letter = step.rem_euclid(7) as u8;
        let octave = i8::try_from(step.div_euclid(7)).ok()?;
        let natural = (i16::from(octave) + 1) * 12 + NATURALS[usize::from(letter)];
        SpelledPitch::new(letter, i8::try_from(midi - natural).ok()?, octave)
    }
    pub fn to_sounding(self, written: SpelledPitch) -> Option<SpelledPitch> {
        self.apply(written, 1)
    }
    pub fn to_written(self, sounding: SpelledPitch) -> Option<SpelledPitch> {
        self.apply(sounding, -1)
    }
    pub fn written_semitone(self, sounding: u8) -> i16 {
        i16::from(sounding) - i16::from(self.semitones)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeySignature {
    fifths: i8,
    minor: bool,
}
impl KeySignature {
    pub fn fifths(self) -> i8 {
        self.fifths
    }
    pub fn is_minor(self) -> bool {
        self.minor
    }
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (root, minor) = text.strip_suffix('m').map_or((text, false), |root| (root, true));
        let tonic = SpelledPitch::parse(&format!("{root}4"))?;
        Self::from_tonic(tonic, minor)
    }
    fn from_tonic(tonic: SpelledPitch, minor: bool) -> Option<Self> {
        let natural_fifths = *[0, 2, 4, -1, 1, 3, 5].get(usize::from(tonic.letter))?;
        let fifths = natural_fifths + 7 * i16::from(tonic.accidental) - if minor { 3 } else { 0 };
        if (-7..=7).contains(&fifths) {
            return Some(Self { fifths: fifths as i8, minor });
        }
        // Prefer an ordinary enharmonic key over an eight-sharp signature.
        let pc = tonic.midi()? % 12;
        (-7..=7)
            .map(|fifths| Self { fifths, minor })
            .filter(|key| key.tonic().midi().is_some_and(|midi| midi % 12 == pc))
            .min_by_key(|key| key.fifths.unsigned_abs())
    }
    pub fn tonic(self) -> SpelledPitch {
        let pitch_class =
            (7 * i16::from(self.fifths) + if self.minor { 9 } else { 0 }).rem_euclid(12) as u8;
        for letter in 0..7 {
            let accidental = self.accidental_for(letter);
            let pitch = SpelledPitch { letter, accidental, octave: 4 };
            if pitch.midi().is_some_and(|midi| midi % 12 == pitch_class) {
                return pitch;
            }
        }
        SpelledPitch { letter: 0, accidental: 0, octave: 4 }
    }
    pub fn accidental_for(self, letter: u8) -> i8 {
        let order = if self.fifths >= 0 { [3, 0, 4, 1, 5, 2, 6] } else { [6, 2, 5, 1, 4, 0, 3] };
        if order.iter().take(usize::from(self.fifths.unsigned_abs())).any(|&step| step == letter) {
            self.fifths.signum()
        } else {
            0
        }
    }
    pub fn transposed(self, transposition: Transposition) -> Option<Self> {
        Self::from_tonic(transposition.to_written(self.tonic())?, self.minor)
    }
    /// Spell an unspelled MIDI pitch in this key. Explicit authored spelling
    /// can instead be retained with `Transposition::to_written`.
    pub fn spell(self, midi: u8) -> Option<SpelledPitch> {
        if midi > 127 {
            return None;
        }
        self.spell_written(i16::from(midi))
    }
    pub fn spell_written(self, semitone: i16) -> Option<SpelledPitch> {
        let mut best = None;
        for letter in 0..7 {
            for accidental in -2i8..=2 {
                let natural_pc = NATURALS[usize::from(letter)] + i16::from(accidental);
                let octave = (semitone - natural_pc).div_euclid(12) - 1;
                let Some(pitch) = i8::try_from(octave)
                    .ok()
                    .and_then(|octave| SpelledPitch::new(letter, accidental, octave))
                else {
                    continue;
                };
                if pitch.semitone() != Some(semitone) {
                    continue;
                }
                let cost = (
                    (accidental - self.accidental_for(letter)).unsigned_abs(),
                    accidental.unsigned_abs(),
                    (accidental < 0) != (self.fifths < 0),
                    letter,
                );
                if best.as_ref().is_none_or(|(previous, _)| cost < *previous) {
                    best = Some((cost, pitch));
                }
            }
        }
        best.map(|(_, pitch)| pitch)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NotationContext {
    pub transposition: Transposition,
}
impl NotationContext {
    pub fn written_key(self, sounding_key: KeySignature) -> Option<KeySignature> {
        sounding_key.transposed(self.transposition)
    }
    pub fn written_spelling(
        self,
        sounding: SpelledPitch,
        sounding_key: KeySignature,
    ) -> Option<SpelledPitch> {
        let written = self.transposition.to_written(sounding)?;
        let tonic = self.transposition.to_written(sounding_key.tonic())?;
        let key = self.written_key(sounding_key)?;
        let key_tonic = key.tonic();
        if tonic.letter == key_tonic.letter && tonic.accidental == key_tonic.accidental {
            Some(written)
        } else {
            // An enharmonic key normalization must respell its notes too.
            key.spell_written(written.semitone()?)
        }
    }
    pub fn written_pitch(
        self,
        sounding_midi: u8,
        sounding_key: KeySignature,
    ) -> Option<SpelledPitch> {
        self.written_spelling(sounding_key.spell(sounding_midi)?, sounding_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pitch(value: &str) -> SpelledPitch {
        SpelledPitch::parse(value).unwrap()
    }
    #[test]
    fn spelling_preserves_enharmonic_positions_and_bounded_pitch() {
        assert_eq!(pitch("Bb3").midi(), pitch("A#3").midi());
        assert_ne!(pitch("Bb3").diatonic_step(), pitch("A#3").diatonic_step());
        assert_eq!(pitch("B#3").midi(), pitch("C4").midi());
        for invalid in ["", "H4", "C9999999999", "C###4", "C#b4"] {
            assert!(SpelledPitch::parse(invalid).is_none(), "{invalid}");
        }
        for key in ["C", "F", "D", "Bb", "F#m", "Ebm"] {
            let key = KeySignature::parse(key).unwrap();
            for midi in 0..=127 {
                assert_eq!(key.spell(midi).unwrap().midi(), Some(midi));
            }
        }
    }
    #[test]
    fn every_midi_name_parses_back_to_its_own_semitone() {
        // `harmonicon-ui`'s `projection::note_spelling` expects on this:
        // a score note's MIDI number, named, must always parse, including
        // the negative octave below C0 and anything up to u8::MAX.
        for midi in 0..=u8::MAX {
            let name = crate::midi::midi_to_note(i32::from(midi));
            let spelled = SpelledPitch::parse(&name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(spelled.semitone(), Some(i16::from(midi)), "{name}");
        }
    }

    #[test]
    fn written_and_sounding_pitch_are_inverses_with_correct_letters() {
        assert_eq!(Transposition::B_FLAT.to_sounding(pitch("C4")), Some(pitch("Bb3")));
        assert_eq!(Transposition::B_FLAT.to_written(pitch("Eb4")), Some(pitch("F4")));
        assert_eq!(Transposition::A.to_sounding(pitch("C4")), Some(pitch("A3")));
        assert_eq!(Transposition::OCTAVE_DOWN.to_written(pitch("E1")), Some(pitch("E2")));
        for transposition in [
            Transposition::CONCERT,
            Transposition::B_FLAT,
            Transposition::A,
            Transposition::OCTAVE_DOWN,
        ] {
            for name in ["C4", "F#4", "Bb3", "Eb5", "B4"] {
                let start = pitch(name);
                assert_eq!(
                    transposition.to_sounding(transposition.to_written(start).unwrap()),
                    Some(start)
                );
            }
        }
        assert_eq!(Transposition::OCTAVE_DOWN.written_semitone(120), 132);
        assert_eq!(
            NotationContext { transposition: Transposition::OCTAVE_DOWN }
                .written_pitch(127, KeySignature::default())
                .unwrap()
                .name(),
            "G10"
        );
        assert!(pitch("G#9").midi().is_none());
    }
    #[test]
    fn keys_transpose_and_keep_flat_or_sharp_spelling_in_the_signature() {
        assert_eq!(
            KeySignature::parse("C").unwrap().transposed(Transposition::B_FLAT),
            KeySignature::parse("D")
        );
        assert_eq!(
            KeySignature::parse("Bb").unwrap().transposed(Transposition::B_FLAT),
            KeySignature::parse("C")
        );
        assert_eq!(
            KeySignature::parse("F#").unwrap().transposed(Transposition::B_FLAT),
            KeySignature::parse("Ab")
        );
        assert_eq!(
            KeySignature::parse("Gm").unwrap().transposed(Transposition::B_FLAT),
            KeySignature::parse("Am")
        );
        assert!(KeySignature::default().spell(128).is_none());
        let context = NotationContext { transposition: Transposition::B_FLAT };
        assert_eq!(
            context.written_spelling(pitch("Db4"), KeySignature::default()),
            Some(pitch("Eb4"))
        );
        assert_eq!(
            context.written_pitch(66, KeySignature::parse("F#").unwrap()),
            Some(pitch("Ab4"))
        );
        let bb = KeySignature::parse("Bb").unwrap();
        assert_eq!(bb.spell(70).unwrap().name(), "Bb4");
        assert_eq!(bb.accidental_for(6), -1);
        assert_eq!(bb.accidental_for(2), -1);
    }
}
