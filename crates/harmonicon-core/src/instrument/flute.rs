// SPDX-License-Identifier: MIT

//! The transverse (Boehm) concert flute: one voice, a fingering per note,
//! and a foot joint that decides the lowest one.
//!
//! **The fingering table is data, from a source.** Every entry is the
//! Woodwind Fingering Guide's flute chart (wfg.woodwind.org, the basic
//! first-, second- and third-octave pages for the standard fingerings and
//! the third-octave alternates page for the rest), not reconstructed from
//! acoustics. A change here should cite a chart too.
//!
//! A position is a sounding note and which of its fingerings is meant
//! ([`FingeringAssignment`]: 0 the standard, then the alternates in the
//! table's order). The keys come from the table. Two notes on the *same*
//! keys are one tube overblown to another partial (E4 and E5, say), which
//! is what the Note Trainer's register drill is built from
//! ([`Flute::overblown_pairs`]); nothing here has to know a partial number,
//! which the chart doesn't give for the third octave anyway.

use crate::midi::note_to_midi;

/// The lowest joint: a C foot reaches C4, a B foot B3 (and carries the
/// low-B roller the B3 fingering needs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FootJoint {
    #[default]
    C,
    B,
}

/// The keys a fingering presses, as bits. Plates are closed by a finger;
/// the right pinky's E♭ key is *opened* by pressing it, which is why it is
/// pressed for nearly every note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keys(pub u16);

impl Keys {
    /// Left thumb, B key.
    pub const TH: u16 = 1 << 0;
    pub const L1: u16 = 1 << 1;
    pub const L2: u16 = 1 << 2;
    pub const L3: u16 = 1 << 3;
    /// Left pinky, G♯ key.
    pub const GS: u16 = 1 << 4;
    pub const R1: u16 = 1 << 5;
    pub const R2: u16 = 1 << 6;
    pub const R3: u16 = 1 << 7;
    /// Right pinky, D♯/E♭ key.
    pub const EB: u16 = 1 << 8;
    /// Right pinky, low C♯ roller.
    pub const CS: u16 = 1 << 9;
    /// Right pinky, low C roller.
    pub const C: u16 = 1 << 10;
    /// Right pinky, low B roller (B foot only).
    pub const B: u16 = 1 << 11;
    /// The D trill key, between R1 and R2.
    pub const TRILL_D: u16 = 1 << 12;
    /// The D♯ trill key, between R2 and R3.
    pub const TRILL_DS: u16 = 1 << 13;

    /// Every key, in the order a diagram lays them out: left hand, then
    /// right, then the right pinky's foot keys, then the trill keys.
    pub const ALL: [u16; 14] = [
        Self::TH,
        Self::L1,
        Self::L2,
        Self::L3,
        Self::GS,
        Self::R1,
        Self::R2,
        Self::R3,
        Self::EB,
        Self::CS,
        Self::C,
        Self::B,
        Self::TRILL_D,
        Self::TRILL_DS,
    ];

    pub fn pressed(self, key: u16) -> bool {
        self.0 & key != 0
    }
}

/// The keys from the mouthpiece end down the tube, as a vertical drawing of
/// the flute shows them and as its play highway's rows run: the left
/// thumb, the left hand's plates and its G♯ key, the right hand's plates
/// with the two trill keys between them, then the right pinky's E♭ key and
/// the foot's rollers (a B foot's low B last).
pub fn key_order(foot: FootJoint) -> Vec<u16> {
    let mut keys = vec![
        Keys::TH,
        Keys::L1,
        Keys::L2,
        Keys::L3,
        Keys::GS,
        Keys::R1,
        Keys::TRILL_D,
        Keys::R2,
        Keys::TRILL_DS,
        Keys::R3,
        Keys::EB,
        Keys::CS,
        Keys::C,
    ];
    if foot == FootJoint::B {
        keys.push(Keys::B);
    }
    keys
}

/// A key's short name, as a row label: `Th`, `L1`, `G#`, `Dtr`, `Eb`…
/// ASCII, so every bundled font has it.
pub fn key_label(key: u16) -> &'static str {
    match key {
        Keys::TH => "Th",
        Keys::L1 => "L1",
        Keys::L2 => "L2",
        Keys::L3 => "L3",
        Keys::GS => "G#",
        Keys::R1 => "R1",
        Keys::TRILL_D => "Dtr",
        Keys::R2 => "R2",
        Keys::TRILL_DS => "D#tr",
        Keys::R3 => "R3",
        Keys::EB => "Eb",
        Keys::CS => "C#",
        Keys::C => "C",
        Keys::B => "B",
        _ => "",
    }
}

/// One table row: a sounding note and the keys of one way to finger it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingering {
    /// Sounding pitch, as MIDI.
    pub note: u8,
    pub keys: Keys,
}

/// Where a flute note is played: the note, and which of its fingerings
/// (0 the standard one, then the table's alternates for it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FingeringAssignment {
    pub note: u8,
    pub alternate: u8,
}

use Keys as K;

/// `(note, keys)`, standard fingering first for each note, alternates
/// after it in the chart's order. Notes ascend.
const TABLE: &[(&str, u16)] = &[
    // ── First octave (fl_bas_1) ──────────────────────────────────────────
    ("B3", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::B),
    ("C4", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::C),
    ("C#4", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::CS),
    ("D4", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3),
    ("D#4", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::EB),
    ("E4", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::EB),
    ("F4", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::EB),
    ("F#4", K::TH | K::L1 | K::L2 | K::L3 | K::R3 | K::EB),
    ("G4", K::TH | K::L1 | K::L2 | K::L3 | K::EB),
    ("G#4", K::TH | K::L1 | K::L2 | K::L3 | K::GS | K::EB),
    ("A4", K::TH | K::L1 | K::L2 | K::EB),
    ("A#4", K::TH | K::L1 | K::R1 | K::EB),
    ("B4", K::TH | K::L1 | K::EB),
    ("C5", K::L1 | K::EB),
    ("C#5", K::EB),
    // ── Second octave (fl_bas_2) ─────────────────────────────────────────
    ("D5", K::TH | K::L2 | K::L3 | K::R1 | K::R2 | K::R3),
    ("D#5", K::TH | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::EB),
    ("E5", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::EB),
    ("F5", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::EB),
    ("F#5", K::TH | K::L1 | K::L2 | K::L3 | K::R3 | K::EB),
    ("G5", K::TH | K::L1 | K::L2 | K::L3 | K::EB),
    ("G#5", K::TH | K::L1 | K::L2 | K::L3 | K::GS | K::EB),
    ("A5", K::TH | K::L1 | K::L2 | K::EB),
    ("A#5", K::TH | K::L1 | K::R1 | K::EB),
    ("B5", K::TH | K::L1 | K::EB),
    ("C6", K::L1 | K::EB),
    ("C#6", K::EB),
    // ── Third octave (fl_bas_3; alternates from fl_alt_3) ────────────────
    ("D6", K::TH | K::L2 | K::L3 | K::EB),
    ("D6", K::TH | K::L2 | K::L3 | K::GS | K::R1 | K::EB),
    ("D6", K::TH | K::L2 | K::L3 | K::R1 | K::R2 | K::R3),
    ("D#6", K::TH | K::L1 | K::L2 | K::L3 | K::GS | K::R1 | K::R2 | K::R3 | K::EB),
    ("D#6", K::TH | K::L2 | K::L3 | K::GS | K::EB),
    ("D#6", K::TH | K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::EB),
    ("E6", K::TH | K::L1 | K::L2 | K::R1 | K::R2 | K::EB),
    ("E6", K::TH | K::L1 | K::L2 | K::R1 | K::R2),
    ("F6", K::TH | K::L1 | K::L3 | K::R1 | K::EB),
    ("F6", K::TH | K::L1 | K::L3 | K::R1 | K::R2 | K::R3 | K::EB),
    ("F#6", K::TH | K::L1 | K::L3 | K::R3 | K::EB),
    ("F#6", K::TH | K::L1 | K::L2 | K::L3 | K::R2 | K::EB),
    ("G6", K::L1 | K::L2 | K::L3 | K::EB),
    ("G6", K::L1 | K::L2 | K::L3 | K::CS),
    ("G6", K::L1 | K::L2 | K::L3 | K::R1 | K::R2 | K::R3 | K::C),
    ("G#6", K::L2 | K::L3 | K::GS | K::EB),
    ("G#6", K::TH | K::L2 | K::L3 | K::GS | K::R1 | K::R2 | K::EB),
    ("G#6", K::L2 | K::L3 | K::GS | K::R2 | K::R3 | K::EB),
    ("A6", K::TH | K::L2 | K::R1 | K::EB),
    ("A6", K::TH | K::L2 | K::R1 | K::R3 | K::CS),
    ("A#6", K::TH | K::R1 | K::TRILL_D),
    ("B6", K::TH | K::L1 | K::L3 | K::TRILL_DS),
    ("B6", K::TH | K::L1 | K::L3 | K::R2 | K::R3),
    ("B6", K::TH | K::L1 | K::L3 | K::R2 | K::R3 | K::EB),
    ("C7", K::L1 | K::L2 | K::L3 | K::GS | K::R1),
    ("C7", K::L1 | K::L2 | K::L3 | K::GS | K::R1 | K::R3),
    ("C7", K::TH | K::L1 | K::L2 | K::L3 | K::GS | K::R1),
];

/// A flute, by its foot joint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flute {
    pub foot: FootJoint,
}

impl Flute {
    pub fn new(foot: FootJoint) -> Self {
        Self { foot }
    }

    /// The lowest note: B3 on a B foot, C4 on a C foot.
    pub fn lowest(&self) -> u8 {
        match self.foot {
            FootJoint::B => 59,
            FootJoint::C => 60,
        }
    }

    /// Every fingering this flute has, notes ascending. A C foot has no
    /// low-B roller, so the fingerings that press it are left out.
    pub fn fingerings(&self) -> Vec<Fingering> {
        TABLE
            .iter()
            .filter_map(|&(name, keys)| {
                let note = u8::try_from(note_to_midi(name)?).ok()?;
                Some(Fingering { note, keys: Keys(keys) })
            })
            .filter(|f| self.foot == FootJoint::B || !f.keys.pressed(Keys::B))
            .collect()
    }

    /// The rows of [`key_order`] whose key `at` presses: where a fingering
    /// is drawn on the play highway. Empty for a fingering this flute
    /// doesn't have.
    pub fn pressed_rows(&self, at: FingeringAssignment) -> Vec<usize> {
        let Some(keys) = self.keys(at) else { return Vec::new() };
        key_order(self.foot)
            .into_iter()
            .enumerate()
            .filter(|&(_, key)| keys.pressed(key))
            .map(|(row, _)| row)
            .collect()
    }

    /// `note`'s fingerings, standard first.
    pub fn fingerings_for(&self, note: u8) -> Vec<Fingering> {
        self.fingerings().into_iter().filter(|f| f.note == note).collect()
    }

    /// The keys of one assignment, or `None` for a note this flute doesn't
    /// have or an alternate it has fewer of.
    pub fn keys(&self, at: FingeringAssignment) -> Option<Keys> {
        self.fingerings_for(at.note).get(usize::from(at.alternate)).map(|f| f.keys)
    }

    /// `(lower, upper)` pairs of positions on the very same keys — one
    /// fingering overblown from the lower note to the upper, as the flute
    /// plays its second octave over its first. Each upper note is paired
    /// with the nearest note below it on its keys.
    pub fn overblown_pairs(&self) -> Vec<(FingeringAssignment, FingeringAssignment)> {
        let positions: Vec<(FingeringAssignment, Keys)> = self
            .fingerings()
            .iter()
            .scan((0u8, 0u8), |(last, alt), f| {
                *alt = if *last == f.note { *alt + 1 } else { 0 };
                *last = f.note;
                Some((FingeringAssignment { note: f.note, alternate: *alt }, f.keys))
            })
            .collect();
        positions
            .iter()
            .filter_map(|&(upper, keys)| {
                positions
                    .iter()
                    .filter(|(lower, k)| *k == keys && lower.note < upper.note)
                    .max_by_key(|(lower, _)| lower.note)
                    .map(|&(lower, _)| (lower, upper))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard(flute: &Flute, name: &str) -> Keys {
        let note = note_to_midi(name).unwrap() as u8;
        flute.keys(FingeringAssignment { note, alternate: 0 }).unwrap()
    }

    #[test]
    fn the_foot_joint_decides_the_lowest_note() {
        let c = Flute::new(FootJoint::C);
        let b = Flute::new(FootJoint::B);
        assert_eq!(c.fingerings().first().map(|f| f.note), Some(60));
        assert_eq!(b.fingerings().first().map(|f| f.note), Some(59));
        assert_eq!((c.lowest(), b.lowest()), (60, 59));
    }

    /// Every chromatic note from the lowest to C7 has a standard fingering.
    #[test]
    fn the_table_covers_the_range_chromatically() {
        for foot in [FootJoint::C, FootJoint::B] {
            let flute = Flute::new(foot);
            for note in flute.lowest()..=96 {
                assert!(!flute.fingerings_for(note).is_empty(), "{note} on a {foot:?} foot");
            }
        }
    }

    /// The fingerings players first learn wrong, as the chart gives them.
    #[test]
    fn the_known_exceptions_match_the_chart() {
        let f = Flute::default();
        assert!(!standard(&f, "D4").pressed(Keys::EB), "low D lifts the E♭ key");
        assert_eq!(standard(&f, "C#5"), Keys(Keys::EB), "middle C♯ is almost all open");
        assert!(!standard(&f, "D5").pressed(Keys::L1), "middle D lifts L1");
        assert!(!standard(&f, "C5").pressed(Keys::TH), "middle C has no thumb");
        assert!(standard(&f, "A#6").pressed(Keys::TRILL_D));
    }

    /// The second octave is the first overblown on the same keys, from E up.
    #[test]
    fn overblown_pairs_link_an_octave_on_one_fingering() {
        let pairs = Flute::default().overblown_pairs();
        let e4 = FingeringAssignment { note: 64, alternate: 0 };
        let e5 = FingeringAssignment { note: 76, alternate: 0 };
        assert!(pairs.contains(&(e4, e5)));
        // D5 lifts L1, so it is not low D's fingering overblown.
        let d5 = FingeringAssignment { note: 74, alternate: 0 };
        assert!(pairs.iter().all(|&(_, upper)| upper != d5));
        assert!(pairs.iter().all(|(lower, upper)| lower.note < upper.note));
    }

    #[test]
    fn a_fingering_is_drawn_on_the_rows_of_its_pressed_keys() {
        let f = Flute::default();
        let e4 = FingeringAssignment { note: 64, alternate: 0 };
        let labels: Vec<&str> =
            f.pressed_rows(e4).into_iter().map(|r| key_label(key_order(f.foot)[r])).collect();
        assert_eq!(labels, ["Th", "L1", "L2", "L3", "R1", "R2", "Eb"]);
        assert!(f.pressed_rows(FingeringAssignment { note: 100, alternate: 0 }).is_empty());
        assert_eq!(key_order(FootJoint::B).len(), key_order(FootJoint::C).len() + 1);
        assert!(key_order(FootJoint::B).iter().all(|&k| !key_label(k).is_empty()));
    }

    #[test]
    fn an_alternate_past_the_last_is_none() {
        let f = Flute::default();
        assert!(
            f.keys(FingeringAssignment { note: 79 + 12, alternate: 2 }).is_some(),
            "G6 has two alternates"
        );
        assert!(f.keys(FingeringAssignment { note: 79 + 12, alternate: 3 }).is_none());
        assert!(f.keys(FingeringAssignment { note: 100, alternate: 0 }).is_none());
    }
}
