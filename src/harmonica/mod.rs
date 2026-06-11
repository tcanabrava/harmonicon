use crate::song::HarpChart;
use crate::song::chart::{
    Harmonica,
    BendingProfile,
};

use crate::audio_system::midi_functions::{
    midi_to_note,
    note_to_midi
};

use std::collections::HashSet;

// Creates the twelve-bar key signature for the given key.
pub fn twelve_bar(key: &str) -> [String; 12] {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    [
        key.into(), key.into(), key.into(), key.into(),
        iv.clone(), iv.clone(), key.into(), key.into(),
        v.clone(),  iv.clone(), key.into(), v.clone(),
    ]
}

// Returns the semitone label for the given root and offset.
pub fn semitone(root: &str, n: i32) -> String {
    const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let i = NOTES.iter().position(|&x| x == root).unwrap_or(0);
    NOTES[((i as i32 + n).rem_euclid(12)) as usize].to_string()
}

// Returns the blow label for the given hole, or a dash if not available.
pub fn blow_label(hole: u8, chart: &HarpChart) -> String {
    let Some(idx) = hole.checked_sub(1) else { return "\u{2014}".into() };
    if let Harmonica::Diatonic { layout: Some(ref l), .. } = chart.harmonica {
        if let Some(ref notes) = l.blow {
            if let Some(n) = notes.get(idx as usize) {
                return n.clone();
            }
        }
    }
    "\u{2014}".into()
}

// Returns the draw label for the given hole, or a dash if not available.
pub fn draw_label(hole: u8, chart: &HarpChart) -> String {
    let Some(idx) = hole.checked_sub(1) else { return "\u{2014}".into() };
    if let Harmonica::Diatonic { layout: Some(ref l), .. } = chart.harmonica {
        if let Some(ref notes) = l.draw {
            if let Some(n) = notes.get(idx as usize) {
                return n.clone();
            }
        }
    }
    "\u{2014}".into()
}

// Returns a human-readable string describing the harmonica type and settings.
pub fn harp_display(chart: &HarpChart) -> String {
    match &chart.harmonica {
        Harmonica::Diatonic { holes, bending_profile, position, .. } => {
            let pos = position.as_deref().unwrap_or("?");
            let profile = match bending_profile {
                BendingProfile::RichterStandard => "Richter",
                BendingProfile::CountryTuned => "Country",
            };
            format!("Diatonic \u{00B7} {} holes \u{00B7} {} position \u{00B7} {}", holes, pos, profile)
        }
        Harmonica::Chromatic { holes, position, .. } => {
            let pos = position.as_deref().unwrap_or("?");
            format!("Chromatic \u{00B7} {} holes \u{00B7} {} position", holes, pos)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::chart::{Action, HarpChart, Harmonica};

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
        assert_eq!(semitone("C", 7), "G");  // perfect fifth
        assert_eq!(semitone("C", 5), "F");  // perfect fourth
        assert_eq!(semitone("C", 4), "E");  // major third
        assert_eq!(semitone("G", 5), "C");  // fourth up from G
        assert_eq!(semitone("A", 3), "C");  // minor third up
    }

    #[test]
    fn semitone_wraps_at_octave() {
        assert_eq!(semitone("B",  1), "C");
        assert_eq!(semitone("C", 12), "C");
        assert_eq!(semitone("A", 14), "B");
    }

    #[test]
    fn twelve_bar_c_major() {
        let bar = twelve_bar("C");
        // Pattern: I I I I  IV IV I I  V IV I V
        let expected = ["C","C","C","C","F","F","C","C","G","F","C","G"];
        assert_eq!(bar, expected.map(str::to_string));
    }

    #[test]
    fn twelve_bar_g_major() {
        let bar = twelve_bar("G");
        // IV of G = C,  V of G = D
        let expected = ["G","G","G","G","C","C","G","G","D","C","G","D"];
        assert_eq!(bar, expected.map(str::to_string));
    }

    #[test]
    fn blow_label_returns_correct_note() {
        let chart = test_chart();
        assert_eq!(blow_label(1, &chart), "C4");
        assert_eq!(blow_label(4, &chart), "C5");
        assert_eq!(blow_label(10, &chart), "C7");
    }

    #[test]
    fn blow_label_out_of_range_returns_dash() {
        let chart = test_chart();
        assert_eq!(blow_label(0, &chart), "\u{2014}");   // hole=0 guard
        assert_eq!(blow_label(11, &chart), "\u{2014}");  // beyond layout
    }

    #[test]
    fn draw_label_returns_correct_note() {
        let chart = test_chart();
        assert_eq!(draw_label(1, &chart), "D4");
        assert_eq!(draw_label(3, &chart), "B4");
        assert_eq!(draw_label(0, &chart), "\u{2014}");
    }

    #[test]
    fn build_valid_notes_contains_blow_and_draw() {
        let chart = test_chart();
        let notes = build_valid_notes(&chart);
        for n in &["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"] {
            assert!(notes.contains(*n), "missing blow note {n}");
        }
        for n in &["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"] {
            assert!(notes.contains(*n), "missing draw note {n}");
        }
    }

    #[test]
    fn build_valid_notes_includes_bend_pitches() {
        let chart = test_chart();
        let notes = build_valid_notes(&chart);
        // Hole 1: draw=D4(62) bends down to blow=C4(60) → C#4(61) reachable
        assert!(notes.contains("C#4"), "missing bend note C#4");
        // Hole 2: draw=G4(67) bends down to blow=E4(64) → F4(65), F#4(66) reachable
        assert!(notes.contains("F4"),  "missing bend note F4");
        assert!(notes.contains("F#4"), "missing bend note F#4");
    }

    #[test]
    fn build_valid_notes_excludes_unrelated_notes() {
        let chart = test_chart();
        let notes = build_valid_notes(&chart);
        assert!(!notes.contains("D#0"));
        assert!(!notes.contains("C8"));
    }
}

// Build the complete set of notes this harmonica can physically produce,
// including all bendable pitches between blow and draw notes.
pub fn build_valid_notes(chart: &HarpChart) -> HashSet<String> {
    let mut set = HashSet::new();
    match &chart.harmonica {
        Harmonica::Diatonic { layout: Some(l), .. } => {
            let blow = l.blow.as_deref().unwrap_or(&[]);
            let draw = l.draw.as_deref().unwrap_or(&[]);
            for (i, (b, d)) in blow.iter().zip(draw.iter()).enumerate() {
                set.insert(b.clone());
                set.insert(d.clone());
                // Holes 1-6: draw bends downward toward the blow note.
                // Holes 7-10: blow bends downward toward the draw note.
                let (bend_from, bend_to) = if i < 6 { (d, b) } else { (b, d) };
                if let (Some(from_m), Some(to_m)) = (note_to_midi(bend_from), note_to_midi(bend_to)) {
                    let lo = from_m.min(to_m);
                    let hi = from_m.max(to_m);
                    for m in (lo + 1)..hi {
                        set.insert(midi_to_note(m));
                    }
                }
            }
        }
        Harmonica::Chromatic { layout: Some(l), .. } => {
            for opt in [&l.blow, &l.draw, &l.blow_slide, &l.draw_slide] {
                if let Some(notes) = opt {
                    for n in notes { set.insert(n.clone()); }
                }
            }
        }
        _ => {}
    }
    set
}
