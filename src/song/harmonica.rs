// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::song::chart::{Action, BendingProfile};

use crate::song::chart::{ChromaticLayout, DiatonicLayout};

use crate::audio_system::midi::{midi_to_note, note_to_freq_hz, note_to_midi};

use std::collections::HashSet;

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
    /// 12-hole chromatic), instead of the old hardcoded `HOLE_COUNT = 10`.
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

    // Build the complete set of notes this harmonica can physically produce,
    // including all bendable pitches between blow and draw notes.
    pub fn build_valid_notes(&self) -> HashSet<String> {
        let mut set = HashSet::new();
        match &self {
            Harmonica::Diatonic {
                layout: Some(l), ..
            } => {
                let blow = l.blow.as_deref().unwrap_or(&[]);
                let draw = l.draw.as_deref().unwrap_or(&[]);
                for (i, (b, d)) in blow.iter().zip(draw.iter()).enumerate() {
                    set.insert(b.clone());
                    set.insert(d.clone());
                    // Holes 1-6: draw bends downward toward the blow note.
                    // Holes 7-10: blow bends downward toward the draw note.
                    let (bend_from, bend_to) = if i < 6 { (d, b) } else { (b, d) };
                    if let (Some(from_m), Some(to_m)) =
                        (note_to_midi(bend_from), note_to_midi(bend_to))
                    {
                        let lo = from_m.min(to_m);
                        let hi = from_m.max(to_m);
                        for m in (lo + 1)..hi {
                            set.insert(midi_to_note(m));
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
                        set.insert(n.clone());
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
            .filter_map(|n| note_to_freq_hz(n))
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

    // Before this fix, `wind_direction_label` only matched `Harmonica::Diatonic`
    // and silently returned "—" for every chromatic hole regardless of layout.
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
        for n in &["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"] {
            assert!(notes.contains(*n), "missing blow note {n}");
        }
        for n in &["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"] {
            assert!(notes.contains(*n), "missing draw note {n}");
        }
    }

    #[test]
    fn build_valid_notes_includes_bend_pitches() {
        let chart = test_chart();
        let notes = chart.harmonica.build_valid_notes();
        // Hole 1: draw=D4(62) bends down to blow=C4(60) → C#4(61) reachable
        assert!(notes.contains("C#4"), "missing bend note C#4");
        // Hole 2: draw=G4(67) bends down to blow=E4(64) → F4(65), F#4(66) reachable
        assert!(notes.contains("F4"), "missing bend note F4");
        assert!(notes.contains("F#4"), "missing bend note F#4");
    }

    #[test]
    fn build_valid_notes_excludes_unrelated_notes() {
        let chart = test_chart();
        let notes = chart.harmonica.build_valid_notes();
        assert!(!notes.contains("D#0"));
        assert!(!notes.contains("C8"));
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
        // Hole 1 blow on a key-of-G diatonic is G3 ≈ 196 Hz — below the old
        // fixed 200 Hz detector floor this range replaces.
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
}
