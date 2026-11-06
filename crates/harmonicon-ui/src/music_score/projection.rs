// SPDX-License-Identifier: MIT
//! Convert sounding pitches into notation at the boundary of the score view.
use super::*;
use harmonicon_core::notation::{KeySignature, NotationContext, SpelledPitch};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MusicScoreKeySignature(pub KeySignature);

pub fn project_notes(
    notes: &mut [NotationNote],
    context: NotationContext,
    sounding_key: KeySignature,
) {
    for note in notes {
        let written = note
            .spelling
            .and_then(|spelling| context.written_spelling(spelling, sounding_key))
            .or_else(|| context.written_pitch(note.midi, sounding_key));
        if let Some(written) = written
            && let Some(semitone) = written.semitone().and_then(|pitch| u8::try_from(pitch).ok())
        {
            note.midi = semitone;
            note.spelling = Some(written);
        }
    }
}

/// Project freshly built sounding notes and return the score's written key.
/// Never pass notes that have already been projected.
pub fn project_score(
    notes: &mut [NotationNote],
    context: NotationContext,
    sounding_key: &str,
) -> MusicScoreKeySignature {
    let key = KeySignature::parse(sounding_key).unwrap_or_default();
    project_notes(notes, context, key);
    MusicScoreKeySignature(context.written_key(key).unwrap_or_default())
}

pub fn note_spelling(note: &NotationNote) -> SpelledPitch {
    note.spelling.unwrap_or_else(|| {
        SpelledPitch::parse(&harmonicon_core::midi::midi_to_note(i32::from(note.midi)))
            .expect("a notation semitone has a valid spelling")
    })
}
pub fn note_staff_step(note: &NotationNote, clef: Clef) -> i32 {
    let pitch = note_spelling(note);
    let base =
        SpelledPitch::parse(&harmonicon_core::midi::midi_to_note(i32::from(clef.base_midi())))
            .unwrap();
    i32::from(pitch.diatonic_step() - base.diatonic_step())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn score_projection_changes_written_notes_without_changing_acoustic_targets() {
        let mut notes = [NotationNote {
            start_beat: 0.0,
            duration_beats: 1.0,
            midi: 58,
            spelling: SpelledPitch::parse("Bb3"),
            tied_from_previous: false,
            highlighted: false,
        }];
        let context =
            NotationContext { transposition: harmonicon_core::notation::Transposition::B_FLAT };
        project_notes(&mut notes, context, KeySignature::parse("Bb").unwrap());
        assert_eq!(notes[0].midi, 60);
        assert_eq!(notes[0].spelling.unwrap().name(), "C4");
        assert_eq!(note_staff_step(&notes[0], Clef::Treble), -2);
    }
}

#[cfg(test)]
mod spelling_tests {
    use super::super::notation::{Accidental, accidentals_with_key, resolve_clef};
    use super::*;

    fn note(name: &str, beat: f64) -> NotationNote {
        let spelling = SpelledPitch::parse(name).unwrap();
        NotationNote {
            start_beat: beat,
            duration_beats: 1.0,
            midi: spelling.semitone().unwrap() as u8,
            spelling: Some(spelling),
            tied_from_previous: false,
            highlighted: false,
        }
    }

    #[test]
    fn enharmonic_notes_keep_their_own_staff_positions() {
        let flat = note("Bb4", 0.0);
        let sharp = note("A#4", 1.0);
        assert_eq!(flat.midi, sharp.midi);
        assert_eq!(note_staff_step(&flat, Clef::Treble), note_staff_step(&sharp, Clef::Treble) + 1);
    }

    #[test]
    fn key_signature_cancellation_and_bar_reset_use_written_spelling() {
        let notes = [note("Bb4", 0.0), note("B4", 1.0), note("Bb4", 2.0), note("Bb4", 4.0)];
        assert_eq!(
            accidentals_with_key(&notes, Clef::Treble, 4.0, KeySignature::parse("F").unwrap()),
            vec![Accidental::None, Accidental::Natural, Accidental::Flat, Accidental::None]
        );
    }

    #[test]
    fn accidentals_reset_at_actual_meter_changes() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::new([(0, "4/4"), (24, "3/4")], 12), quarter_ticks: 12,
        };
        let notes = [note("F#4", 1.0), note("F#4", 2.0), note("F#4", 3.0), note("F#4", 5.0)];
        assert_eq!(super::super::notation::accidentals_in_bars(&notes, Clef::Treble, &bars, KeySignature::default()),
            vec![Accidental::Sharp, Accidental::Sharp, Accidental::None, Accidental::Sharp]);
    }

    #[test]
    fn octave_instruments_project_once_and_high_bass_keeps_its_clef_fallback() {
        let context = NotationContext {
            transposition: harmonicon_core::notation::Transposition::OCTAVE_DOWN,
        };
        let mut notes = [note("E1", 0.0), note("C4", 1.0)];
        project_score(&mut notes, context, "C");
        assert_eq!(notes[0].spelling.unwrap().name(), "E2");
        assert_eq!(resolve_clef(&notes[..1], Some(Clef::Bass)), Clef::Bass);
        assert_eq!(resolve_clef(&notes[1..], Some(Clef::Bass)), Clef::Treble);
    }

    #[test]
    fn switching_back_to_concert_pitch_resets_the_previous_key_signature() {
        let mut signature;
        let mut notes = [note("Bb3", 0.0)];
        let context =
            NotationContext { transposition: harmonicon_core::notation::Transposition::B_FLAT };
        signature = project_score(&mut notes, context, "Bb");
        assert_eq!(notes[0].spelling.unwrap().name(), "C4");
        assert_eq!(signature.0, KeySignature::default());
        signature = project_score(&mut [], NotationContext::default(), "F");
        assert_eq!(signature.0, KeySignature::parse("F").unwrap());
    }
}
