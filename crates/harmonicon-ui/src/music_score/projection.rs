// SPDX-License-Identifier: MIT
//! Convert sounding pitches into notation at the boundary of the score view.
use super::*;
use harmonicon_core::notation::{KeySignature, NotationContext, SpelledPitch};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MusicScoreKeySignature(pub KeySignature);

pub fn project_notes(notes: &mut [NotationNote], context: NotationContext, sounding_key: KeySignature) {
    for note in notes {
        let written = note.spelling.and_then(|spelling| context.written_spelling(spelling, sounding_key))
            .or_else(|| context.written_pitch(note.midi, sounding_key));
        if let Some(written) = written
            && let Some(semitone) = written.semitone().and_then(|pitch| u8::try_from(pitch).ok()) {
            note.midi = semitone;
            note.spelling = Some(written);
        }
    }
}

pub fn note_spelling(note: &NotationNote) -> SpelledPitch {
    note.spelling.unwrap_or_else(|| SpelledPitch::parse(&harmonicon_core::midi::midi_to_note(i32::from(note.midi))).expect("a notation semitone has a valid spelling"))
}
pub fn note_staff_step(note: &NotationNote, clef: Clef) -> i32 {
    let pitch = note_spelling(note);
    let base = SpelledPitch::parse(&harmonicon_core::midi::midi_to_note(i32::from(clef.base_midi()))).unwrap();
    i32::from(pitch.diatonic_step() - base.diatonic_step())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn score_projection_changes_written_notes_without_changing_acoustic_targets() {
        let mut notes = [NotationNote { start_beat: 0.0, duration_beats: 1.0, midi: 58, spelling: SpelledPitch::parse("Bb3"), tied_from_previous: false, highlighted: false }];
        let context = NotationContext { transposition: harmonicon_core::notation::Transposition::B_FLAT };
        project_notes(&mut notes, context, KeySignature::parse("Bb").unwrap());
        assert_eq!(notes[0].midi, 60);
        assert_eq!(notes[0].spelling.unwrap().name(), "C4");
        assert_eq!(note_staff_step(&notes[0], Clef::Treble), -2);
    }
}
