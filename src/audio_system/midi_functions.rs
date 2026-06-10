
// Convert a note string like "G4", "C#5", "Bb3" to a MIDI number.
pub fn note_to_midi(note: &str) -> Option<i32> {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let (pitch, oct_str) = if note.len() >= 3
        && (note.as_bytes().get(1) == Some(&b'#') || note.as_bytes().get(1) == Some(&b'b'))
    {
        (&note[..2], &note[2..])
    } else {
        (&note[..1], &note[1..])
    };
    // Normalise flats to enharmonic sharps.
    let pitch = match pitch {
        "Db" => "C#", "Eb" => "D#", "Fb" => "E", "Gb" => "F#",
        "Ab" => "G#", "Bb" => "A#", "Cb" => "B", p => p,
    };
    let semitone = NAMES.iter().position(|&n| n == pitch)? as i32;
    let octave: i32 = oct_str.parse().ok()?;
    Some((octave + 1) * 12 + semitone)
}

pub fn midi_to_note(midi: i32) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let semitone = ((midi % 12) + 12) % 12;
    let octave = midi / 12 - 1;
    format!("{}{}", NAMES[semitone as usize], octave)
}
