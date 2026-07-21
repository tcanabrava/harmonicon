// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::song::chart::{Action, BendingProfile, Scale};

use crate::song::chart::{ChromaticLayout, DiatonicLayout};

use crate::audio_system::midi::{NOTE_NAMES, midi_to_freq_hz, midi_to_note, note_to_midi};

use std::collections::HashSet;

// ── Reference layouts ────────────────────────────────────────────────────────
//
// Standard-tuned C reference layouts, transposed by [`key_offset`] to build a
// synthetic [`Harmonica`] for any key — shared by the Bending Trainer (no
// chart loaded, just a picked key) and the Song Editor's Practice/preview
// synthesis (its own `GridNote`s, not a chart's authored layout).

/// Standard Richter-tuned C-harp blow notes, holes 1–10.
pub const C_BLOW: [&str; 10] = ["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"];
/// Standard Richter-tuned C-harp draw notes, holes 1–10.
pub const C_DRAW: [&str; 10] = ["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"];

/// Standard 12-hole C chromatic blow notes: a straight C-major scale (unlike
/// the diatonic layout above, blow and draw are each already a full scale —
/// the slide button fills in the remaining chromatic steps, see
/// [`C_BLOW_SLIDE_CHROMATIC`]/[`C_DRAW_SLIDE_CHROMATIC`]).
pub const C_BLOW_CHROMATIC: [&str; 12] = [
    "C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5", "D5", "E5", "F5", "G5",
];
/// Standard 12-hole C chromatic draw notes (the scale a whole step up).
pub const C_DRAW_CHROMATIC: [&str; 12] = [
    "D4", "E4", "F#4", "G4", "A4", "B4", "C#5", "D5", "E5", "F#5", "G5", "A5",
];
/// Blow notes with the slide button pressed: each a half-step above the
/// unslid blow note.
pub const C_BLOW_SLIDE_CHROMATIC: [&str; 12] = [
    "C#4", "D#4", "F4", "F#4", "G#4", "A#4", "C5", "C#5", "D#5", "F5", "F#5", "G#5",
];
/// Draw notes with the slide button pressed: each a half-step above the
/// unslid draw note.
pub const C_DRAW_SLIDE_CHROMATIC: [&str; 12] = [
    "D#4", "F4", "G4", "G#4", "A#4", "C5", "D5", "D#5", "F5", "G5", "G#5", "A#5",
];

/// Semitone shift from a C harp to `key`, choosing the octave the real harp
/// sits in: keys up to F# pitch above C, G–B pitch below (the "low" harps) —
/// e.g. a G harp's hole-1 blow is G3, not G4. Accepts either sharp or flat
/// spellings (`"C#"`/`"Db"`), since callers use both.
pub fn key_offset(key: &str) -> i32 {
    let semis = note_to_midi(&format!("{}4", key.trim())).map_or(0, |m| m - 60);
    if semis <= 6 { semis } else { semis - 12 }
}

/// Transposes each entry of a reference table by `offset` semitones.
fn transpose_table(notes: &[&str], offset: i32) -> Vec<String> {
    notes
        .iter()
        .filter_map(|n| note_to_midi(n).map(|m| midi_to_note(m + offset)))
        .collect()
}

/// A Richter diatonic harp for `key`, transposed from the [`C_BLOW`]/[`C_DRAW`]
/// reference layout.
pub fn richter_harp(key: &str) -> Harmonica {
    let off = key_offset(key);
    Harmonica::Diatonic {
        holes: 10,
        bending_profile: BendingProfile::RichterStandard,
        position: None,
        scale: None,
        layout: Some(DiatonicLayout {
            blow: Some(transpose_table(&C_BLOW, off)),
            draw: Some(transpose_table(&C_DRAW, off)),
        }),
    }
}

/// A 12-hole chromatic harp for `key`, transposed from the reference layout.
pub fn chromatic_harp(key: &str) -> Harmonica {
    let off = key_offset(key);
    Harmonica::Chromatic {
        holes: 12,
        position: None,
        scale: None,
        layout: Some(ChromaticLayout {
            blow: Some(transpose_table(&C_BLOW_CHROMATIC, off)),
            draw: Some(transpose_table(&C_DRAW_CHROMATIC, off)),
            blow_slide: Some(transpose_table(&C_BLOW_SLIDE_CHROMATIC, off)),
            draw_slide: Some(transpose_table(&C_DRAW_SLIDE_CHROMATIC, off)),
        }),
    }
}

// ── Per-hole note set ─────────────────────────────────────────────────────────

/// Every note one hole can produce, by technique. `pub` so any trainer/editor
/// (the Bending Trainer's target picker, the Song Editor's note-frequency
/// resolution) can reuse the same bend/overblow math instead of re-deriving
/// it — see [`hole_notes`].
pub struct HoleNotes {
    pub over: Option<String>,
    pub blow: Option<String>,
    pub draw: Option<String>,
    /// Bends, smallest first (½ step, whole, 1½). Draw bends on holes 1–6,
    /// blow bends on holes 7–10.
    pub bends: Vec<String>,
}

/// Transpose a note label by `semis` semitones, e.g. `transpose("B4", 1) → "C5"`.
fn transpose(s: &str, semis: i32) -> Option<String> {
    note_to_midi(s).map(|m| midi_to_note(m + semis))
}

/// Keep a harp label only if it's a real note (drops the `—` "not available".)
/// `pub(crate)` so callers building their own per-technique note lookups
/// (the harmonica overlay's chromatic diagram) can filter the same way
/// [`hole_notes`] does, without re-deriving it.
pub(crate) fn valid_note(s: String) -> Option<String> {
    note_to_midi(&s).map(|_| s)
}

/// Every note `hole` can produce on `harp`, across every technique that
/// applies to it — the shared derivation behind the harmonica overlay
/// diagram, the Bending Trainer's target picker, and the Song Editor's note
/// frequency resolution, so all three agree on e.g. which reed an overblow
/// actually sounds above.
pub fn hole_notes(harp: &Harmonica, hole: u8) -> HoleNotes {
    let blow = valid_note(harp.wind_direction_label(hole, &Action::Blow));
    let draw = valid_note(harp.wind_direction_label(hole, &Action::Draw));

    // Overblow (holes 1,4,5,6) sits a semitone above the draw reed; overdraw
    // (holes 7–10) a semitone above the blow reed.
    let over = match hole {
        1 | 4 | 5 | 6 => draw.as_deref().and_then(|d| transpose(d, 1)),
        7..=10 => blow.as_deref().and_then(|b| transpose(b, 1)),
        _ => None,
    };

    // Bends fill the chromatic steps between blow and draw: drawn down from the
    // draw reed on holes 1–6, down from the blow reed on holes 7–10.
    let mut bends = Vec::new();
    if let (Some(b), Some(d)) = (&blow, &draw)
        && let (Some(bm), Some(dm)) = (note_to_midi(b), note_to_midi(d))
    {
        if hole <= 6 && dm > bm + 1 {
            bends = (1..dm - bm).map(|k| midi_to_note(dm - k)).collect();
        } else if hole >= 7 && bm > dm + 1 {
            bends = (1..bm - dm).map(|k| midi_to_note(bm - k)).collect();
        }
    }

    HoleNotes {
        over,
        blow,
        draw,
        bends,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Harmonica {
    Diatonic {
        holes: u8,
        bending_profile: BendingProfile,
        position: Option<String>,
        scale: Option<Scale>,
        layout: Option<DiatonicLayout>,
    },
    Chromatic {
        holes: u8,
        position: Option<String>,
        scale: Option<Scale>,
        layout: Option<ChromaticLayout>,
    },
}

// Creates the twelve-bar key signature for the given key. Always the
// standard form (`Progression::Standard`) — the scored-gameplay/song-editor
// grid overlays that call this are an educational reference independent of
// what a loaded chart's actual chords do, not tied to Jam Session's
// selectable progression (see [`progression_bars`]).
pub fn twelve_bar(key: &str) -> [String; 12] {
    progression_bars(key, Progression::Standard).map(|(root, _)| root)
}

/// A 12-bar blues variant — which chord roots land on which bars, and (for
/// [`Minor`](Progression::Minor)) which bars change quality. Selectable in
/// Jam Session (`jam::session`, `jam::backing`) via the "Generate
/// Jam" config page; every other 12-bar display (`twelve_bar`, the song
/// editor's grid, scored gameplay's grid overlay) always uses `Standard`.
/// `QuickChange` and `Minor` are ordinary blues-theory forms; a "jazz blues"
/// variant is deliberately not included here — that belongs to the 0.6 Jazz
/// milestone's chord-tone work (`ROADMAP.md`), not this one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Progression {
    #[default]
    Standard,
    /// Bar 2 moves to IV instead of staying on I — the "quick change" most
    /// blues players default to today.
    QuickChange,
    /// i/iv chords are minor 7th instead of dominant 7th; V stays dominant
    /// for the pull back to i, the standard minor-blues convention (e.g.
    /// "Mr. P.C.", "The Thrill Is Gone").
    Minor,
}

impl Progression {
    /// Display label for the picker on the "Generate Jam" config page.
    pub fn label(self) -> &'static str {
        match self {
            Progression::Standard => "Standard",
            Progression::QuickChange => "Quick Change",
            Progression::Minor => "Minor Blues",
        }
    }

    /// Cycles to the next variant, wrapping — same "◂ ▸ over a small enum"
    /// pattern as `audio_system::midi::next_key`, just with 3 states instead
    /// of 12.
    pub fn next(self) -> Self {
        match self {
            Progression::Standard => Progression::QuickChange,
            Progression::QuickChange => Progression::Minor,
            Progression::Minor => Progression::Standard,
        }
    }

    /// Cycles to the previous variant, wrapping.
    pub fn prev(self) -> Self {
        match self {
            Progression::Standard => Progression::Minor,
            Progression::QuickChange => Progression::Standard,
            Progression::Minor => Progression::QuickChange,
        }
    }
}

/// Cross-harp playing position: which harp key to grab relative to the jam's
/// own key. Selectable on the "Generate Jam" config page (`jam::backing`,
/// `menu::pages::jam_generate`) the same way [`Progression`] is; a hand-authored
/// chart instead states its own position directly via `Harmonica::position`
/// (see [`harp_banner`]) since its harmonica layout is already authored in
/// whatever key the chart needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Position {
    /// Straight harp: the harp's key is the jam's key.
    #[default]
    First,
    /// Cross harp: the harp is pitched a 4th below the jam's key (e.g. a C
    /// harp jamming in G) — the classic blues draw-note position.
    Second,
    /// The harp is pitched a whole step below the jam's key (e.g. a C harp
    /// jamming in D) — minor/dorian flavored.
    Third,
}

impl Position {
    /// Display label for the picker and [`harp_banner`].
    pub fn label(self) -> &'static str {
        match self {
            Position::First => "1st",
            Position::Second => "2nd",
            Position::Third => "3rd",
        }
    }

    /// Cycles to the next variant, wrapping — same pattern as
    /// [`Progression::next`].
    pub fn next(self) -> Self {
        match self {
            Position::First => Position::Second,
            Position::Second => Position::Third,
            Position::Third => Position::First,
        }
    }

    /// Cycles to the previous variant, wrapping.
    pub fn prev(self) -> Self {
        match self {
            Position::First => Position::Third,
            Position::Second => Position::First,
            Position::Third => Position::Second,
        }
    }

    /// Semitones the harp key sits below the jam key for this position.
    fn interval_below_jam_key(self) -> i32 {
        match self {
            Position::First => 0,
            Position::Second => 7,
            Position::Third => 2,
        }
    }

    /// The harp key to grab so playing this position lands in `jam_key`
    /// (e.g. `Second.harp_key("G") == "C"`).
    pub fn harp_key(self, jam_key: &str) -> String {
        semitone(jam_key, -self.interval_below_jam_key())
    }
}

/// A chord's quality — which intervals above the root are its chord tones
/// (see [`chord_intervals`]). Every chord in a standard/quick-change 12-bar
/// blues is dominant 7th; a minor blues' i/iv chords are minor 7th instead
/// (see [`Progression::Minor`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChordQuality {
    Dominant7,
    Minor7,
}

/// Semitone intervals above the root for a dominant-7th chord (root, 3rd,
/// 5th, ♭7th) — see [`chord_intervals`].
const DOMINANT7_INTERVALS: [i32; 4] = [0, 4, 7, 10];
/// Semitone intervals above the root for a minor-7th chord (root, ♭3rd,
/// 5th, ♭7th) — see [`chord_intervals`].
const MINOR7_INTERVALS: [i32; 4] = [0, 3, 7, 10];

/// Semitone intervals above the root for `quality`'s chord tones (root,
/// 3rd, 5th, 7th).
pub const fn chord_intervals(quality: ChordQuality) -> [i32; 4] {
    match quality {
        ChordQuality::Dominant7 => DOMINANT7_INTERVALS,
        ChordQuality::Minor7 => MINOR7_INTERVALS,
    }
}

/// The 12 bars of `progression` in `key`: each bar's chord root + quality.
/// Bars are 0-indexed, matching `CurrentBar`/`twelve_bar`'s existing
/// convention.
pub fn progression_bars(key: &str, progression: Progression) -> [(String, ChordQuality); 12] {
    use ChordQuality::{Dominant7, Minor7};
    let i = key.to_string();
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    let q = if progression == Progression::Minor {
        Minor7
    } else {
        Dominant7
    };
    match progression {
        Progression::Standard | Progression::Minor => [
            (i.clone(), q),
            (i.clone(), q),
            (i.clone(), q),
            (i.clone(), q),
            (iv.clone(), q),
            (iv.clone(), q),
            (i.clone(), q),
            (i.clone(), q),
            (v.clone(), Dominant7),
            (iv, q),
            (i, q),
            (v, Dominant7),
        ],
        Progression::QuickChange => [
            (i.clone(), Dominant7),
            (iv.clone(), Dominant7),
            (i.clone(), Dominant7),
            (i.clone(), Dominant7),
            (iv.clone(), Dominant7),
            (iv.clone(), Dominant7),
            (i.clone(), Dominant7),
            (i.clone(), Dominant7),
            (v.clone(), Dominant7),
            (iv, Dominant7),
            (i, Dominant7),
            (v, Dominant7),
        ],
    }
}

/// One-line "which harp to grab" hint. A Richter diatonic's key is its hole-1
/// blow note, so it's derived from the layout and paired with the song's
/// position and key — e.g. `"Use a C harmonica · 2nd position · key of G"`.
/// Falls back to just the key when the harp key can't be determined.
pub fn harp_banner(harp: &Harmonica, song_key: &str) -> String {
    let blow1 = harp.wind_direction_label(1, &Action::Blow);
    let harp_key = blow1.trim_end_matches(|c: char| c.is_ascii_digit());
    if harp_key.is_empty() || harp_key == "\u{2014}" {
        return format!("Playing in {song_key}");
    }
    match harp.position() {
        Some(pos) => {
            format!(
                "Use a {harp_key} harmonica  \u{00B7}  {pos} position  \u{00B7}  key of {song_key}"
            )
        }
        None => format!("Use a {harp_key} harmonica  \u{00B7}  key of {song_key}"),
    }
}

// Returns the semitone label for the given root and offset.
pub fn semitone(root: &str, n: i32) -> String {
    let i = NOTE_NAMES.iter().position(|&x| x == root).unwrap_or(0);
    NOTE_NAMES[((i as i32 + n).rem_euclid(12)) as usize].to_string()
}

/// Semitone offsets of the six blues-scale degrees (1, ♭3, 4, ♭5, 5, ♭7)
/// above the root — see [`blues_scale_classes`].
const BLUES_SCALE_INTERVALS: [i32; 6] = [0, 3, 5, 6, 7, 10];

/// The six note classes of the blues scale rooted on `key` (1, b3, 4, b5, 5, b7).
/// Shared by Jam Session's live hole-map feedback and (via
/// [`Scale::classes`]'s position variants) the song editor's scale-aware
/// note coloring, so both reflect the same blues-scale definition.
pub fn blues_scale_classes(key: &str) -> HashSet<String> {
    BLUES_SCALE_INTERVALS
        .iter()
        .map(|&n| semitone(key, n))
        .collect()
}

/// Semitone offsets of the major (Ionian) scale's seven degrees above the
/// root — see [`Scale::Major`].
const MAJOR_SCALE_INTERVALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Semitone offsets of the minor pentatonic scale's five degrees (1, ♭3, 4,
/// 5, ♭7) above the root — see [`Scale::MinorPentatonic`].
const MINOR_PENTATONIC_INTERVALS: [i32; 5] = [0, 3, 5, 7, 10];
/// Semitone offsets of the major pentatonic scale's five degrees (1, 2, 3,
/// 5, 6) above the root — commonly called "the Country scale" in
/// harmonica pedagogy (the notes 2nd-position cross-harp playing reaches
/// without bending) — see [`Scale::Country`].
const COUNTRY_SCALE_INTERVALS: [i32; 5] = [0, 2, 4, 7, 9];

impl Scale {
    /// Every selectable scale, in the order the Song Editor's picker offers
    /// them.
    pub fn all() -> &'static [Scale] {
        &[
            Scale::FirstPosition,
            Scale::SecondPosition,
            Scale::ThirdPosition,
            Scale::Major,
            Scale::MinorPentatonic,
            Scale::Country,
        ]
    }

    /// Display label for the Song Editor's scale combobox.
    pub fn label(self) -> &'static str {
        match self {
            Scale::FirstPosition => "1st Position",
            Scale::SecondPosition => "2nd Position",
            Scale::ThirdPosition => "3rd Position",
            Scale::Major => "Major Scale",
            Scale::MinorPentatonic => "Minor Pentatonic",
            Scale::Country => "Country Scale",
        }
    }

    /// Inverse of [`label`](Self::label) — for UI that deals in plain
    /// strings (the combobox's own selection event) rather than the enum
    /// itself. `None` for anything that isn't one of [`Self::all`]'s labels.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().iter().copied().find(|s| s.label() == label)
    }

    /// Semitones this scale's root sits *above* the harp's own key —
    /// `0` for a shape rooted directly on the harp key (`Major`/
    /// `MinorPentatonic`/`Country`, and `FirstPosition` itself), or the 2nd/
    /// 3rd-position offset otherwise. Numerically the same 0/7/2 that
    /// `Position::interval_below_jam_key` uses, just applied upward from
    /// the harp's own key instead of downward from a separate jam key —
    /// a chart has no jam key distinct from its harp, so "position" here
    /// means "which mode of this harp," not "which harp for this jam."
    fn root_offset_semitones(self) -> i32 {
        match self {
            Scale::FirstPosition | Scale::Major | Scale::MinorPentatonic | Scale::Country => 0,
            Scale::SecondPosition => 7,
            Scale::ThirdPosition => 2,
        }
    }

    /// This scale's degree intervals (semitones above its own root) — the
    /// blues hexatonic for the three position variants (matching every
    /// other blues-scale overlay in the game), or the named scale's own
    /// intervals otherwise.
    fn degree_intervals(self) -> &'static [i32] {
        match self {
            Scale::FirstPosition | Scale::SecondPosition | Scale::ThirdPosition => {
                &BLUES_SCALE_INTERVALS
            }
            Scale::Major => &MAJOR_SCALE_INTERVALS,
            Scale::MinorPentatonic => &MINOR_PENTATONIC_INTERVALS,
            Scale::Country => &COUNTRY_SCALE_INTERVALS,
        }
    }

    /// The note classes (no octave) of this scale, rooted relative to
    /// `harp_key` — what the Song Editor's grid colors notes against
    /// (`song_editor::grid::note_in_scale`), replacing what used to be an
    /// unconditional [`blues_scale_classes`] call with an explicit,
    /// chart-author-selectable scale.
    pub fn classes(self, harp_key: &str) -> HashSet<String> {
        let root = semitone(harp_key, self.root_offset_semitones());
        self.degree_intervals()
            .iter()
            .map(|&n| semitone(&root, n))
            .collect()
    }
}

// Returns the blow label for the given hole, or a dash if not available.

impl Harmonica {
    /// How many holes this harmonica has — the loaded chart's authority for
    /// lane counts, hole-strip ranges, etc. (a 10-hole diatonic vs. e.g. a
    /// 12-hole chromatic), rather than a fixed constant.
    pub fn hole_count(&self) -> u8 {
        match self {
            Harmonica::Diatonic { holes, .. } | Harmonica::Chromatic { holes, .. } => *holes,
        }
    }

    // Returns the blow/draw label for the given hole, or a dash if not available.
    pub fn wind_direction_label(&self, hole: u8, action: &Action) -> String {
        let default_return = "\u{2014}".into();
        let Some(idx) = hole.checked_sub(1) else {
            return default_return;
        };

        let notes = match self {
            Harmonica::Diatonic {
                layout: Some(l), ..
            } => match action {
                Action::Blow => &l.blow,
                Action::Draw => &l.draw,
            },
            Harmonica::Chromatic {
                layout: Some(l), ..
            } => match action {
                Action::Blow => &l.blow,
                Action::Draw => &l.draw,
            },
            _ => return default_return,
        };

        let Some(notes) = notes else {
            return default_return;
        };
        let Some(n) = notes.get(idx as usize) else {
            return default_return;
        };

        n.clone()
    }

    /// The MIDI note number for `hole`'s `action` (blow/draw), or `None` for
    /// a hole/direction the harp can't produce. Identity/comparison uses
    /// (e.g. matching a detected pitch to a hole for hole-lighting) should
    /// use this instead of comparing [`wind_direction_label`]'s display
    /// string, which is spelling-sensitive (`"A#4"` vs `"Bb4"`) in a way a
    /// MIDI number isn't.
    ///
    /// [`wind_direction_label`]: Self::wind_direction_label
    pub fn wind_direction_midi(&self, hole: u8, action: &Action) -> Option<u8> {
        let m = note_to_midi(&self.wind_direction_label(hole, action))?;
        u8::try_from(m).ok()
    }

    /// The slide-pressed pitch for the given hole/direction on a chromatic
    /// harmonica (a half-step above the natural note) — the chromatic
    /// equivalent of a diatonic bend. `"—"` for a diatonic harmonica (which
    /// has no slide button) or an out-of-range hole.
    pub fn slide_label(&self, hole: u8, action: &Action) -> String {
        let default_return = "\u{2014}".into();
        let Some(idx) = hole.checked_sub(1) else {
            return default_return;
        };
        let Harmonica::Chromatic {
            layout: Some(l), ..
        } = self
        else {
            return default_return;
        };
        let notes = match action {
            Action::Blow => &l.blow_slide,
            Action::Draw => &l.draw_slide,
        };
        let Some(notes) = notes else {
            return default_return;
        };
        let Some(n) = notes.get(idx as usize) else {
            return default_return;
        };
        n.clone()
    }

    /// The configured playing position label (e.g. `"1st"`, `"2nd"`), if any.
    pub fn position(&self) -> Option<&str> {
        match self {
            Harmonica::Diatonic { position, .. } | Harmonica::Chromatic { position, .. } => {
                position.as_deref()
            }
        }
    }

    /// The chart-declared [`Scale`] to color notes against, if any.
    pub fn scale(&self) -> Option<Scale> {
        match self {
            Harmonica::Diatonic { scale, .. } | Harmonica::Chromatic { scale, .. } => *scale,
        }
    }

    // Returns a human-readable string describing the harmonica type and settings.
    pub fn display(&self) -> String {
        match &self {
            Harmonica::Diatonic {
                holes,
                bending_profile,
                position,
                ..
            } => {
                let pos = position.as_deref().unwrap_or("?");
                let profile = match bending_profile {
                    BendingProfile::RichterStandard => "Richter",
                    BendingProfile::CountryTuned => "Country",
                };
                format!(
                    "Diatonic \u{00B7} {} holes \u{00B7} {} position \u{00B7} {}",
                    holes, pos, profile
                )
            }
            Harmonica::Chromatic {
                holes, position, ..
            } => {
                let pos = position.as_deref().unwrap_or("?");
                format!(
                    "Chromatic \u{00B7} {} holes \u{00B7} {} position",
                    holes, pos
                )
            }
        }
    }

    // Build the complete set of MIDI note numbers this harmonica can
    // physically produce, including all bendable pitches between blow and
    // draw notes. Keying on the MIDI number (rather than a formatted name
    // like `"G4"`) is what lets scoring compare detected pitches by integer
    // equality — no allocation, no risk of an enharmonic spelling mismatch.
    pub fn build_valid_notes(&self) -> HashSet<u8> {
        // Doesn't capture `set`, so it can be called freely alongside direct
        // `set.insert` calls below without fighting the borrow checker.
        fn to_midi_u8(name: &str) -> Option<u8> {
            u8::try_from(note_to_midi(name)?).ok()
        }

        let mut set = HashSet::new();
        match &self {
            Harmonica::Diatonic {
                layout: Some(l), ..
            } => {
                let blow = l.blow.as_deref().unwrap_or(&[]);
                let draw = l.draw.as_deref().unwrap_or(&[]);
                for (i, (b, d)) in blow.iter().zip(draw.iter()).enumerate() {
                    set.extend(to_midi_u8(b));
                    set.extend(to_midi_u8(d));
                    // Holes 1-6: draw bends downward toward the blow note.
                    // Holes 7-10: blow bends downward toward the draw note.
                    let (bend_from, bend_to) = if i < 6 { (d, b) } else { (b, d) };
                    if let (Some(from_m), Some(to_m)) =
                        (note_to_midi(bend_from), note_to_midi(bend_to))
                    {
                        let lo = from_m.min(to_m);
                        let hi = from_m.max(to_m);
                        for m in (lo + 1)..hi {
                            set.extend(u8::try_from(m).ok());
                        }
                    }
                }
            }
            Harmonica::Chromatic {
                layout: Some(l), ..
            } => {
                for notes in [&l.blow, &l.draw, &l.blow_slide, &l.draw_slide]
                    .into_iter()
                    .flatten()
                {
                    for n in notes {
                        set.extend(to_midi_u8(n));
                    }
                }
            }
            _ => {}
        }
        set
    }

    /// Frequency bounds (Hz) spanning every note in [`build_valid_notes`], or
    /// `None` if the harmonica has no layout to derive them from. Used to
    /// size the pitch detector's search range to the actual instrument
    /// instead of a fixed constant — a Low-F/Low-D diatonic's hole-1 notes
    /// sit well below a standard-key harp's range.
    ///
    /// [`build_valid_notes`]: Self::build_valid_notes
    pub fn frequency_range(&self) -> Option<(f32, f32)> {
        let freqs: Vec<f32> = self
            .build_valid_notes()
            .iter()
            .map(|&m| midi_to_freq_hz(m as f32))
            .collect();
        if freqs.is_empty() {
            return None;
        }
        let lo = freqs.iter().cloned().fold(f32::MAX, f32::min);
        let hi = freqs.iter().cloned().fold(f32::MIN, f32::max);
        Some((lo, hi))
    }
}

#[cfg(test)]
mod tests;
