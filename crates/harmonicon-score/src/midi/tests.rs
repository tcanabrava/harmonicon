// SPDX-License-Identifier: MIT

use super::*;
use harmonicon_core::midi_file::{meta, note_off, note_on, smf_bytes};
use midly::MetaMessage;

/// A file whose tracks are `(name, [(delta_to_on, key, len_ticks)])`.
fn midi_with(tracks: &[(&str, &[(u32, u8, u32)])]) -> Vec<u8> {
    let built: Vec<Vec<midly::TrackEvent<'static>>> = tracks
        .iter()
        .map(|(name, notes)| {
            let mut events = vec![meta(0, MetaMessage::TrackName(name.as_bytes().to_vec().leak()))];
            for &(delta, key, len) in *notes {
                events.push(note_on(delta, key, 100));
                events.push(note_off(len, key));
            }
            events.push(meta(0, MetaMessage::EndOfTrack));
            events
        })
        .collect();
    smf_bytes(built)
}

#[test]
fn tracks_are_listed_with_their_names_and_note_counts() {
    let bytes = midi_with(&[
        ("Conductor", &[]),
        ("Guitar", &[(0, 60, 480), (0, 62, 480)]),
        ("Harmonica", &[(0, 64, 480)]),
    ]);
    let score = MidiScore::parse(bytes).unwrap();
    let tracks = score.tracks();
    assert_eq!(tracks.len(), 3);
    assert_eq!(tracks[0].name.as_deref(), Some("Conductor"));
    assert_eq!(tracks[0].note_count, 0);
    assert_eq!(tracks[1].note_count, 2);
    assert_eq!(tracks[2].name.as_deref(), Some("Harmonica"));
}

#[test]
fn the_harmonica_track_is_found_by_name() {
    // The whole point of reading track names: a file with a dozen parts
    // should not make the player hunt for theirs.
    let bytes = midi_with(&[
        ("Conductor", &[]),
        ("Guitar", &[(0, 60, 480)]),
        ("Harmonica", &[(0, 64, 480)]),
    ]);
    let score = MidiScore::parse(bytes).unwrap();
    assert_eq!(crate::pick_harmonica_track(score.tracks()), Some(2));
}

#[test]
fn notes_come_back_in_seconds_not_ticks() {
    // 480 ticks per quarter at the default 120 BPM is half a second a beat.
    let bytes = midi_with(&[("Harmonica", &[(0, 60, 480), (0, 62, 480)])]);
    let score = MidiScore::parse(bytes).unwrap();
    let notes = score.notes(0).unwrap();
    assert_eq!(notes.len(), 2);
    assert!((notes[0].start_secs - 0.0).abs() < 1e-6);
    assert!(
        (notes[0].duration_secs - 0.5).abs() < 1e-3,
        "expected a half-second note, got {}",
        notes[0].duration_secs
    );
    assert!((notes[1].start_secs - 0.5).abs() < 1e-3);
}

#[test]
fn notes_are_sorted_by_start_time() {
    let bytes = midi_with(&[("Harmonica", &[(0, 60, 240), (0, 62, 240), (0, 64, 240)])]);
    let notes = MidiScore::parse(bytes).unwrap().notes(0).unwrap();
    assert!(
        notes.windows(2).all(|w| w[0].start_secs <= w[1].start_secs),
        "the trait promises sorted output"
    );
}

#[test]
fn a_file_with_no_notes_anywhere_is_rejected_up_front() {
    // Better than handing back a score whose every track plays silence.
    let bytes = midi_with(&[("Conductor", &[]), ("Markers", &[])]);
    assert!(matches!(MidiScore::parse(bytes), Err(crate::ScoreError::NoPlayableTracks)));
}

#[test]
fn rubbish_bytes_are_an_error_rather_than_a_panic() {
    assert!(MidiScore::parse(b"definitely not a midi file".to_vec()).is_err());
}

#[test]
fn asking_for_a_track_beyond_the_end_is_an_error() {
    let bytes = midi_with(&[("Harmonica", &[(0, 60, 480)])]);
    assert!(matches!(
        MidiScore::parse(bytes).unwrap().notes(9),
        Err(crate::ScoreError::NoSuchTrack(9))
    ));
}

#[test]
fn a_file_without_a_declared_meter_reads_as_common_time() {
    let bytes = midi_with(&[("Harmonica", &[(0, 60, 480)])]);
    assert_eq!(MidiScore::parse(bytes).unwrap().time_signature(), (4, 4));
}
