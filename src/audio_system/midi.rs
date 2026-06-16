// SPDX-License-Identifier: MIT

// Convert a note string like "G4", "C#5", "Bb3" to a MIDI number.
pub fn note_to_midi(note: &str) -> Option<i32> {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    // Note names are ASCII; bail before the byte-slicing below so non-ASCII input
    // (e.g. the "—" placeholder for a missing hole) returns None instead of panicking.
    if !note.is_ascii() || note.is_empty() {
        return None;
    }
    let (pitch, oct_str) = if note.len() >= 3
        && (note.as_bytes().get(1) == Some(&b'#') || note.as_bytes().get(1) == Some(&b'b'))
    {
        (&note[..2], &note[2..])
    } else {
        (&note[..1], &note[1..])
    };
    // Normalise flats to enharmonic sharps.
    let pitch = match pitch {
        "Db" => "C#",
        "Eb" => "D#",
        "Fb" => "E",
        "Gb" => "F#",
        "Ab" => "G#",
        "Bb" => "A#",
        "Cb" => "B",
        p => p,
    };
    let semitone = NAMES.iter().position(|&n| n == pitch)? as i32;
    let octave: i32 = oct_str.parse().ok()?;
    Some((octave + 1) * 12 + semitone)
}

pub fn midi_to_note(midi: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let semitone = midi.rem_euclid(12);
    let octave = midi / 12 - 1;
    format!("{}{}", NAMES[semitone as usize], octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naturals() {
        assert_eq!(note_to_midi("C4"), Some(60));
        assert_eq!(note_to_midi("A4"), Some(69));
        assert_eq!(note_to_midi("B4"), Some(71));
        assert_eq!(note_to_midi("C0"), Some(12));
    }

    #[test]
    fn sharps() {
        assert_eq!(note_to_midi("C#4"), Some(61));
        assert_eq!(note_to_midi("F#4"), Some(66));
        assert_eq!(note_to_midi("A#4"), Some(70));
    }

    #[test]
    fn flats_normalised_to_enharmonic_sharps() {
        assert_eq!(note_to_midi("Bb3"), Some(58)); // A#3
        assert_eq!(note_to_midi("Db5"), Some(73)); // C#5
        assert_eq!(note_to_midi("Eb4"), Some(63)); // D#4
        assert_eq!(note_to_midi("Gb4"), Some(66)); // F#4
    }

    #[test]
    fn invalid_note_names_return_none() {
        assert_eq!(note_to_midi("X4"), None);
        assert_eq!(note_to_midi("C"), None); // no octave
    }

    #[test]
    fn known_midi_values() {
        assert_eq!(midi_to_note(60), "C4");
        assert_eq!(midi_to_note(69), "A4");
        assert_eq!(midi_to_note(61), "C#4");
        assert_eq!(midi_to_note(21), "A0");
    }

    #[test]
    fn roundtrip_all_midi_values() {
        // midi_to_note only produces sharps, so every value round-trips cleanly.
        for midi in 0i32..=127 {
            let name = midi_to_note(midi);
            assert_eq!(
                note_to_midi(&name),
                Some(midi),
                "roundtrip failed for midi={midi}"
            );
        }
    }
}
