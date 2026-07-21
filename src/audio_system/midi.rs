// SPDX-License-Identifier: MIT

/// The 12 chromatic pitch classes, always sharp-spelled — the one source of
/// truth every note-name lookup/cycle in the crate should share instead of
/// re-declaring its own copy (`note_to_midi`/`midi_to_note` below,
/// `song::harmonica::semitone`'s transposition table, `audio_system::
/// pitch_detect`'s detected-pitch display, and the key pickers in
/// `gameplay::bending_trainer`/`menu::jam_generate`, all used to keep
/// independent copies of this exact array).
pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

// Convert a note string like "G4", "C#5", "Bb3" to a MIDI number.
pub fn note_to_midi(note: &str) -> Option<i32> {
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
    let semitone = NOTE_NAMES.iter().position(|&n| n == pitch)? as i32;
    let octave: i32 = oct_str.parse().ok()?;
    Some((octave + 1) * 12 + semitone)
}

pub fn midi_to_note(midi: i32) -> String {
    let semitone = midi.rem_euclid(12);
    let octave = midi / 12 - 1;
    format!("{}{}", NOTE_NAMES[semitone as usize], octave)
}

/// The key after `k` in the chromatic cycle (wrapping from B back to C).
/// Shared by any "cycle through all 12 keys" picker — `gameplay::
/// bending_trainer`'s and `menu::jam_generate`'s key controls both call
/// this instead of keeping their own copy. Unrecognised input falls back to
/// `NOTE_NAMES[0]` ("C"), same permissive fallback `prev_key` uses.
pub fn next_key(k: &str) -> String {
    let i = NOTE_NAMES.iter().position(|&x| x == k).unwrap_or(0);
    NOTE_NAMES[(i + 1) % 12].to_string()
}

/// The key before `k` in the chromatic cycle (wrapping from C back to B).
pub fn prev_key(k: &str) -> String {
    let i = NOTE_NAMES.iter().position(|&x| x == k).unwrap_or(0);
    NOTE_NAMES[(i + 11) % 12].to_string()
}

/// Concert-pitch frequency (Hz) for a MIDI note number. Fractional input is
/// allowed so callers can price in bends/cents (e.g. a half-step-flat draw
/// note is `midi - 0.5`) without rounding to the nearest semitone first.
pub fn midi_to_freq_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// Concert-pitch frequency (Hz) for a note name like `"C#4"`.
pub fn note_to_freq_hz(note: &str) -> Option<f32> {
    Some(midi_to_freq_hz(note_to_midi(note)? as f32))
}

/// Nearest MIDI note number for a raw frequency (rounds to the nearest
/// semitone) — the inverse of [`midi_to_freq_hz`]. `None` outside the valid
/// MIDI range (0-127), which also catches non-positive/nonsensical input
/// rather than producing a bogus octave.
pub fn freq_to_midi(freq: f32) -> Option<i32> {
    if freq <= 0.0 {
        return None;
    }
    let midi = (69.0 + 12.0 * (freq / 440.0).log2()).round() as i32;
    (0..=127).contains(&midi).then_some(midi)
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

    // ── next_key / prev_key ──────────────────────────────────────────────────

    #[test]
    fn next_key_cycles_forward_and_wraps() {
        assert_eq!(next_key("C"), "C#");
        assert_eq!(next_key("B"), "C");
    }

    #[test]
    fn prev_key_cycles_backward_and_wraps() {
        assert_eq!(prev_key("C#"), "C");
        assert_eq!(prev_key("C"), "B");
    }

    #[test]
    fn every_key_round_trips_through_next_then_prev() {
        for &k in &NOTE_NAMES {
            assert_eq!(prev_key(&next_key(k)), k);
        }
    }

    #[test]
    fn unrecognised_key_falls_back_to_c() {
        assert_eq!(next_key("nonsense"), "C#");
        assert_eq!(prev_key("nonsense"), "B");
    }

    // ── freq_to_midi ──────────────────────────────────────────────────────────

    #[test]
    fn freq_to_midi_identifies_concert_pitch() {
        assert_eq!(freq_to_midi(440.0), Some(69));
    }

    #[test]
    fn freq_to_midi_rounds_to_the_nearest_semitone() {
        assert_eq!(freq_to_midi(261.63), Some(60)); // middle C
    }

    #[test]
    fn freq_to_midi_is_none_for_nonpositive_input() {
        assert_eq!(freq_to_midi(0.0), None);
        assert_eq!(freq_to_midi(-1.0), None);
    }

    #[test]
    fn freq_to_midi_is_none_outside_the_midi_range() {
        assert_eq!(freq_to_midi(50_000.0), None);
    }

    #[test]
    fn freq_to_midi_round_trips_through_midi_to_freq_hz() {
        for midi in 21i32..=108 {
            let freq = midi_to_freq_hz(midi as f32);
            assert_eq!(
                freq_to_midi(freq),
                Some(midi),
                "round trip failed for {midi}"
            );
        }
    }
}
