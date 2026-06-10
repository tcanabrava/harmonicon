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
    if let Harmonica::Diatonic { layout: Some(ref l), .. } = chart.harmonica {
        if let Some(ref notes) = l.blow {
            if let Some(n) = notes.get(hole as usize - 1) {
                return n.clone();
            }
        }
    }
    "\u{2014}".into()
}

// Returns the draw label for the given hole, or a dash if not available.
pub fn draw_label(hole: u8, chart: &HarpChart) -> String {
    if let Harmonica::Diatonic { layout: Some(ref l), .. } = chart.harmonica {
        if let Some(ref notes) = l.draw {
            if let Some(n) = notes.get(hole as usize - 1) {
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
