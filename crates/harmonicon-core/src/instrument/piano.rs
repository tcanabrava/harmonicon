// SPDX-License-Identifier: MIT

//! A piano: a keyboard of so many keys, every key one pitch. Nothing about a
//! key needs choosing beyond the key, so a note's position *is* its pitch
//! ([`PlayPosition::Key`](super::PlayPosition::Key)), and any keys can sound
//! together. What differs between pianos is how far the keyboard reaches:
//! [`KEYBOARDS`] are the sizes sold.
//!
//! Which keys are black is the one fact about the keyboard's *drawing* that
//! belongs here ([`is_black`]), so every view of it agrees.

/// A keyboard's size and its lowest key, by MIDI number: the full 88
/// (A0–C8), and the 76, 61 and 49 of smaller instruments.
pub const KEYBOARDS: [(u8, u8); 4] = [(88, 21), (76, 28), (61, 36), (49, 36)];

/// A piano with `keys` keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piano {
    pub keys: u8,
}

impl Default for Piano {
    fn default() -> Self {
        Self::grand()
    }
}

impl Piano {
    /// The full keyboard, A0 to C8.
    pub fn grand() -> Self {
        Self { keys: 88 }
    }

    /// A keyboard of one of the [`KEYBOARDS`] sizes; `None` for any other.
    pub fn with_keys(keys: u8) -> Option<Self> {
        KEYBOARDS.iter().any(|&(k, _)| k == keys).then_some(Self { keys })
    }

    /// The lowest key, by MIDI number.
    pub fn lowest(&self) -> u8 {
        KEYBOARDS.iter().find(|&&(k, _)| k == self.keys).map_or(21, |&(_, low)| low)
    }

    /// The highest key, by MIDI number.
    pub fn highest(&self) -> u8 {
        self.lowest() + self.keys - 1
    }

    /// Whether the keyboard has `midi`.
    pub fn has(&self, midi: u8) -> bool {
        (self.lowest()..=self.highest()).contains(&midi)
    }
}

/// Whether `midi` is a black key: C♯, D♯, F♯, G♯ and A♯.
pub fn is_black(midi: u8) -> bool {
    matches!(midi % 12, 1 | 3 | 6 | 8 | 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyboards_span_their_sizes() {
        let grand = Piano::grand();
        assert_eq!((grand.lowest(), grand.highest()), (21, 108), "A0 to C8");
        let small = Piano::with_keys(61).unwrap();
        assert_eq!((small.lowest(), small.highest()), (36, 96), "C2 to C7");
        assert_eq!(Piano::with_keys(77), None);
        assert!(grand.has(60) && !small.has(30));
    }

    #[test]
    fn five_of_every_twelve_keys_are_black() {
        assert_eq!((60..72).filter(|&m| is_black(m)).count(), 5);
        assert!(!is_black(60) && is_black(61) && !is_black(64) && !is_black(65) && is_black(70));
    }
}
