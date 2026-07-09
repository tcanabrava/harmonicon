// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::song::chart::{Action, BendingProfile};

use crate::song::chart::{ChromaticLayout, DiatonicLayout};

use crate::audio_system::midi::{midi_to_freq_hz, midi_to_note, note_to_midi};

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
        layout: Option<DiatonicLayout>,
    },
    Chromatic {
        holes: u8,
        position: Option<String>,
        layout: Option<ChromaticLayout>,
    },
}

// Creates the twelve-bar key signature for the given key.
pub fn twelve_bar(key: &str) -> [String; 12] {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    [
        key.into(),
        key.into(),
        key.into(),
        key.into(),
        iv.clone(),
        iv.clone(),
        key.into(),
        key.into(),
        v.clone(),
        iv.clone(),
        key.into(),
        v.clone(),
    ]
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
    const NOTES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let i = NOTES.iter().position(|&x| x == root).unwrap_or(0);
    NOTES[((i as i32 + n).rem_euclid(12)) as usize].to_string()
}

/// The six note classes of the blues scale rooted on `key` (1, b3, 4, b5, 5, b7).
/// Shared by Jam Session's live hole-map feedback and the song editor's
/// scale-aware note coloring, so both reflect the same blues-scale definition.
pub fn blues_scale_classes(key: &str) -> HashSet<String> {
    [0, 3, 5, 6, 7, 10]
        .iter()
        .map(|&n| semitone(key, n))
        .collect()
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
mod tests {
    use super::*;
    use crate::song::chart::HarpChart;

    fn test_chart() -> HarpChart {
        serde_json::from_str(r#"{
            "song": { "title": "T", "artist": "A", "tempo_bpm": 120.0, "key": "C", "difficulty": "easy" },
            "timing": { "resolution": 480, "tempo_map": [{"tick": 0, "bpm": 120.0}] },
            "harmonica": {
                "type": "diatonic",
                "holes": 10,
                "bending_profile": "richter_standard",
                "layout": {
                    "blow": ["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                    "draw": ["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]
                }
            },
            "track": [],
            "scoring": { "perfect_window_ms": 50, "good_window_ms": 100, "miss_window_ms": 130 }
        }"#).unwrap()
    }

    #[test]
    fn semitone_identity() {
        assert_eq!(semitone("C", 0), "C");
        assert_eq!(semitone("G", 0), "G");
    }

    #[test]
    fn semitone_intervals() {
        assert_eq!(semitone("C", 7), "G"); // perfect fifth
        assert_eq!(semitone("C", 5), "F"); // perfect fourth
        assert_eq!(semitone("C", 4), "E"); // major third
        assert_eq!(semitone("G", 5), "C"); // fourth up from G
        assert_eq!(semitone("A", 3), "C"); // minor third up
    }

    #[test]
    fn semitone_wraps_at_octave() {
        assert_eq!(semitone("B", 1), "C");
        assert_eq!(semitone("C", 12), "C");
        assert_eq!(semitone("A", 14), "B");
    }

    #[test]
    fn blues_scale_is_the_six_classes() {
        // C blues: C, Eb(=D#), F, Gb(=F#), G, Bb(=A#).
        let s = blues_scale_classes("C");
        for c in ["C", "D#", "F", "F#", "G", "A#"] {
            assert!(s.contains(c), "missing {c}");
        }
        assert_eq!(s.len(), 6);
        assert!(!s.contains("D"), "major 2nd is not in the blues scale");
        assert!(!s.contains("E"), "major 3rd is not in the blues scale");
    }

    #[test]
    fn twelve_bar_c_major() {
        let bar = twelve_bar("C");
        // Pattern: I I I I  IV IV I I  V IV I V
        let expected = ["C", "C", "C", "C", "F", "F", "C", "C", "G", "F", "C", "G"];
        assert_eq!(bar, expected.map(str::to_string));
    }

    #[test]
    fn twelve_bar_g_major() {
        let bar = twelve_bar("G");
        // IV of G = C,  V of G = D
        let expected = ["G", "G", "G", "G", "C", "C", "G", "G", "D", "C", "G", "D"];
        assert_eq!(bar, expected.map(str::to_string));
    }

    #[test]
    fn blow_label_returns_correct_note() {
        let chart = test_chart();
        assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Blow), "C4");
        assert_eq!(chart.harmonica.wind_direction_label(4, &Action::Blow), "C5");
        assert_eq!(
            chart.harmonica.wind_direction_label(10, &Action::Blow),
            "C7"
        );
    }

    #[test]
    fn blow_label_out_of_range_returns_dash() {
        let chart = test_chart();
        assert_eq!(
            chart.harmonica.wind_direction_label(0, &Action::Blow),
            "\u{2014}"
        ); // hole=0 guard
        assert_eq!(
            chart.harmonica.wind_direction_label(11, &Action::Blow),
            "\u{2014}"
        ); // beyond layout
    }

    #[test]
    fn draw_label_returns_correct_note() {
        let chart = test_chart();
        assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Draw), "D4");
        assert_eq!(chart.harmonica.wind_direction_label(3, &Action::Draw), "B4");
        assert_eq!(
            chart.harmonica.wind_direction_label(0, &Action::Draw),
            "\u{2014}"
        );
    }

    #[test]
    fn hole_count_reads_holes_from_either_variant() {
        assert_eq!(test_chart().harmonica.hole_count(), 10);
        assert_eq!(test_chromatic_chart().harmonica.hole_count(), 12);
    }

    fn test_chromatic_chart() -> HarpChart {
        serde_json::from_str(r#"{
            "song": { "title": "T", "artist": "A", "tempo_bpm": 120.0, "key": "C", "difficulty": "easy" },
            "timing": { "resolution": 480, "tempo_map": [{"tick": 0, "bpm": 120.0}] },
            "harmonica": {
                "type": "chromatic",
                "holes": 12,
                "layout": {
                    "blow":       ["C4","D4","E4","F4","G4","A4","B4","C5","D5","E5","F5","G5"],
                    "draw":       ["D4","E4","F#4","G4","A4","B4","C#5","D5","E5","F#5","G5","A5"],
                    "blow_slide": ["C#4","D#4","F4","F#4","G#4","A#4","B4","C#5","D#5","F5","F#5","G#5"],
                    "draw_slide": ["D#4","F4","G4","G#4","A#4","C5","D5","D#5","F5","G5","G#5","A#5"]
                }
            },
            "track": [],
            "scoring": { "perfect_window_ms": 50, "good_window_ms": 100, "miss_window_ms": 130 }
        }"#).unwrap()
    }

    #[test]
    fn wind_direction_label_works_for_chromatic_too() {
        let chart = test_chromatic_chart();
        assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Blow), "C4");
        assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Draw), "D4");
        assert_eq!(
            chart.harmonica.wind_direction_label(12, &Action::Blow),
            "G5"
        );
    }

    #[test]
    fn slide_label_reads_the_slide_tables_for_chromatic_only() {
        let chromatic = test_chromatic_chart();
        assert_eq!(chromatic.harmonica.slide_label(1, &Action::Blow), "C#4");
        assert_eq!(chromatic.harmonica.slide_label(1, &Action::Draw), "D#4");

        let diatonic = test_chart();
        assert_eq!(
            diatonic.harmonica.slide_label(1, &Action::Blow),
            "\u{2014}",
            "diatonic harmonicas have no slide button"
        );
    }

    #[test]
    fn build_valid_notes_contains_blow_and_draw() {
        let chart = test_chart();
        let notes = chart.harmonica.build_valid_notes();
        // C4, E4, G4, C5, E5, G5, C6, E6, G6, C7
        for n in &[60u8, 64, 67, 72, 76, 79, 84, 88, 91, 96] {
            assert!(notes.contains(n), "missing blow note {n}");
        }
        // D4, G4, B4, D5, F5, A5, B5, D6, F6, A6
        for n in &[62u8, 67, 71, 74, 77, 81, 83, 86, 89, 93] {
            assert!(notes.contains(n), "missing draw note {n}");
        }
    }

    #[test]
    fn build_valid_notes_includes_bend_pitches() {
        let chart = test_chart();
        let notes = chart.harmonica.build_valid_notes();
        // Hole 1: draw=D4(62) bends down to blow=C4(60) → C#4(61) reachable
        assert!(notes.contains(&61u8), "missing bend note C#4");
        // Hole 2: draw=G4(67) bends down to blow=E4(64) → F4(65), F#4(66) reachable
        assert!(notes.contains(&65u8), "missing bend note F4");
        assert!(notes.contains(&66u8), "missing bend note F#4");
    }

    #[test]
    fn build_valid_notes_excludes_unrelated_notes() {
        let chart = test_chart();
        let notes = chart.harmonica.build_valid_notes();
        assert!(!notes.contains(&3u8)); // D#0
        assert!(!notes.contains(&108u8)); // C8
    }

    #[test]
    fn frequency_range_spans_lowest_to_highest_valid_note() {
        let chart = test_chart();
        let (lo, hi) = chart.harmonica.frequency_range().expect("has a layout");
        // Lowest note is hole-1 blow (C4 ≈ 261.6 Hz), highest is hole-10 blow (C7 ≈ 2093 Hz).
        assert!((lo - 261.63).abs() < 1.0, "expected ~C4, got {lo}");
        assert!((hi - 2093.0).abs() < 1.0, "expected ~C7, got {hi}");
    }

    #[test]
    fn frequency_range_of_a_low_g_harp_dips_below_the_old_fixed_floor() {
        // Hole 1 blow on a key-of-G diatonic is G3 ≈ 196 Hz — below the
        // default 200 Hz detector floor.
        let harp = Harmonica::Diatonic {
            holes: 10,
            bending_profile: BendingProfile::RichterStandard,
            position: None,
            layout: Some(DiatonicLayout {
                blow: Some(vec!["G3".into(), "B3".into()]),
                draw: Some(vec!["A3".into(), "D4".into()]),
            }),
        };
        let (lo, _hi) = harp.frequency_range().expect("has a layout");
        assert!(lo < 200.0, "expected below 200 Hz, got {lo}");
    }

    #[test]
    fn frequency_range_is_none_without_a_layout() {
        let harp = Harmonica::Diatonic {
            holes: 10,
            bending_profile: BendingProfile::RichterStandard,
            position: None,
            layout: None,
        };
        assert_eq!(harp.frequency_range(), None);
    }

    // ── key_offset ────────────────────────────────────────────────────────────

    #[test]
    fn key_offsets_pick_the_real_harp_octave() {
        // C harp unchanged; D up two; A and G are the low harps (pitched down).
        assert_eq!(key_offset("C"), 0);
        assert_eq!(key_offset("D"), 2);
        assert_eq!(key_offset("F#"), 6);
        assert_eq!(key_offset("G"), -5);
        assert_eq!(key_offset("A"), -3);
    }

    #[test]
    fn key_offset_accepts_flat_spellings_too() {
        // "Db" and "C#" are the same pitch class — callers use both spellings
        // (the Song Editor's key picker uses flats, the Bending Trainer's
        // uses sharps).
        assert_eq!(key_offset("Db"), key_offset("C#"));
        assert_eq!(key_offset("Ab"), key_offset("G#"));
        assert_eq!(key_offset("Bb"), key_offset("A#"));
    }

    // ── richter_harp / chromatic_harp ─────────────────────────────────────────

    #[test]
    fn c_harp_keeps_the_reference_layout() {
        let Harmonica::Diatonic {
            layout: Some(l), ..
        } = richter_harp("C")
        else {
            panic!("expected diatonic");
        };
        assert_eq!(l.blow.unwrap()[0], "C4");
        assert_eq!(l.draw.unwrap()[0], "D4");
    }

    #[test]
    fn d_harp_hole_1_blow_is_d4() {
        let Harmonica::Diatonic {
            layout: Some(l), ..
        } = richter_harp("D")
        else {
            panic!("expected diatonic");
        };
        assert_eq!(l.blow.unwrap()[0], "D4");
    }

    #[test]
    fn g_harp_hole_1_blow_is_g3() {
        // The G harp is a low harp — hole-1 blow sits below C4.
        let Harmonica::Diatonic {
            layout: Some(l), ..
        } = richter_harp("G")
        else {
            panic!("expected diatonic");
        };
        assert_eq!(l.blow.unwrap()[0], "G3");
    }

    #[test]
    fn chromatic_harp_keeps_the_reference_layout_in_c() {
        let Harmonica::Chromatic {
            layout: Some(l), ..
        } = chromatic_harp("C")
        else {
            panic!("expected chromatic");
        };
        assert_eq!(l.blow.unwrap()[0], "C4");
        assert_eq!(l.draw.unwrap()[0], "D4");
        assert_eq!(l.blow_slide.unwrap()[0], "C#4");
        assert_eq!(l.draw_slide.unwrap()[0], "D#4");
    }

    #[test]
    fn chromatic_harp_transposes_every_table() {
        let Harmonica::Chromatic {
            layout: Some(l), ..
        } = chromatic_harp("D")
        else {
            panic!("expected chromatic");
        };
        assert_eq!(l.blow.unwrap()[0], "D4");
        assert_eq!(l.draw.unwrap()[0], "E4");
    }

    // ── hole_notes ────────────────────────────────────────────────────────────

    #[test]
    fn transpose_crosses_octave() {
        assert_eq!(transpose("B4", 1).as_deref(), Some("C5"));
        assert_eq!(transpose("C5", -1).as_deref(), Some("B4"));
        assert_eq!(transpose("D4", -1).as_deref(), Some("C#4"));
    }

    #[test]
    fn hole_1_has_one_draw_bend() {
        // C harp hole 1: blow C4, draw D4 → single ½-step bend C#4.
        let h = hole_notes(&richter_harp("C"), 1);
        assert_eq!(h.blow.as_deref(), Some("C4"));
        assert_eq!(h.draw.as_deref(), Some("D4"));
        assert_eq!(h.bends, vec!["C#4"]);
    }

    #[test]
    fn hole_3_has_three_draw_bends() {
        // blow G4, draw B4 → A#4, A4, G#4 (½, whole, 1½).
        let h = hole_notes(&richter_harp("C"), 3);
        assert_eq!(h.bends, vec!["A#4", "A4", "G#4"]);
    }

    #[test]
    fn hole_10_has_two_blow_bends() {
        // blow C7, draw A6 → blow bends B6, A#6.
        let h = hole_notes(&richter_harp("C"), 10);
        assert_eq!(h.bends, vec!["B6", "A#6"]);
        assert_eq!(
            h.over.as_deref(),
            Some("C#7"),
            "overdraw a semitone above blow"
        );
    }

    #[test]
    fn hole_5_has_no_draw_bend() {
        // blow E5, draw F5 — only a semitone apart, nothing between.
        let h = hole_notes(&richter_harp("C"), 5);
        assert!(h.bends.is_empty());
    }

    #[test]
    fn hole_1_overblow_is_a_semitone_above_draw() {
        assert_eq!(
            hole_notes(&richter_harp("C"), 1).over.as_deref(),
            Some("D#4")
        );
    }
}
