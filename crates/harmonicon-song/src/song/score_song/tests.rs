// SPDX-License-Identifier: MIT

use super::*;
use harmonicon_core::chart::HarpChart;
use harmonicon_core::midi::note_to_midi;
use harmonicon_core::midi_file::{meta, note_off, note_on, smf_bytes};
use midly::MetaMessage;

fn midi(note: &str) -> u8 {
    note_to_midi(note).unwrap() as u8
}

/// Tracks as `(name, keys)`, each key a quarter-note.
fn midi_file(tracks: &[(&str, &[u8])]) -> Vec<u8> {
    let built: Vec<Vec<midly::TrackEvent<'static>>> = tracks
        .iter()
        .map(|(name, keys)| {
            let mut events = vec![meta(
                0,
                MetaMessage::TrackName(name.as_bytes().to_vec().leak()),
            )];
            for &key in *keys {
                events.push(note_on(0, key, 100));
                events.push(note_off(480, key));
            }
            events.push(meta(0, MetaMessage::EndOfTrack));
            events
        })
        .collect();
    smf_bytes(built)
}

/// A C-major run every C harp plays on plain blow reeds.
fn easy_notes() -> Vec<u8> {
    ["C4", "E4", "G4", "C5", "E5", "G5"].map(midi).to_vec()
}

/// The chart a file opens on — what the player would actually see.
fn chart_of(bytes: Vec<u8>, artist: &str, title: Option<String>) -> HarpChart {
    let converted = convert_score("mid", bytes, artist, title).unwrap();
    converted.tracks[converted.selected].chart.clone()
}

#[test]
fn a_simple_midi_becomes_a_playable_chart() {
    let chart = chart_of(
        midi_file(&[("Harmonica", &easy_notes())]),
        "Some Artist",
        None,
    );
    assert_eq!(chart.track.len(), easy_notes().len());
    assert_eq!(chart.song.artist, "Some Artist");
    assert!(
        chart.track.iter().all(|i| i.events[0].note.is_some()),
        "every converted note must state its sounding pitch"
    );
}

#[test]
fn the_track_named_harmonica_is_the_one_played() {
    // A guitar part converted to harmonica is mostly unreachable notes, so
    // picking by name is what makes a multi-track file usable at all.
    let low: Vec<u8> = (40u8..46).collect();
    let chart = chart_of(
        midi_file(&[("Guitar", &low), ("Harmonica", &easy_notes())]),
        "A",
        None,
    );
    assert_eq!(chart.track.len(), easy_notes().len());
}

#[test]
fn a_single_unnamed_track_is_played_without_asking() {
    assert!(convert_score("mid", midi_file(&[("", &easy_notes())]), "A", None).is_ok());
}

#[test]
fn several_unnamed_tracks_are_all_offered_with_one_chosen() {
    // This used to refuse outright, because an asset loader has nowhere to
    // ask which part is the harmonica. Every part is now converted and the
    // harp-check screen offers the list, so a default is safe to pick.
    let low: Vec<u8> = (40u8..46).collect();
    let converted = convert_score(
        "mid",
        midi_file(&[("", &low), ("", &easy_notes())]),
        "A",
        None,
    )
    .unwrap();
    assert_eq!(
        converted.tracks.len(),
        2,
        "both parts must remain offerable"
    );
    assert_eq!(
        converted.tracks[converted.selected].track.index, 1,
        "the part that actually fits a harmonica should be the default"
    );
}

#[test]
fn a_part_no_harmonica_can_play_is_refused_with_a_reason() {
    // Two octaves below any harp — a bass line. The error names how close
    // the best part got rather than saying "invalid", because the file
    // isn't invalid; it just has no harmonica in it.
    let bass: Vec<u8> = (28u8..40).collect();
    let err = convert_score("mid", midi_file(&[("Harmonica", &bass)]), "A", None)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("playable on a harmonica"),
        "unhelpful error: {err}"
    );
}

#[test]
fn every_playable_track_is_converted_not_just_the_chosen_one() {
    // What makes the picker possible without a reload.
    let converted = convert_score(
        "mid",
        midi_file(&[
            ("Tempo", &[]),
            ("Guitar", &easy_notes()),
            ("Harmonica", &easy_notes()),
        ]),
        "A",
        None,
    )
    .unwrap();
    assert_eq!(
        converted.tracks.len(),
        2,
        "the note-free tempo track is not offerable, the other two are"
    );
    assert!(
        converted.tracks.iter().all(|t| !t.chart.track.is_empty()),
        "every offered part must carry a real chart"
    );
}

#[test]
fn an_unsupported_extension_is_refused_by_name() {
    // The dispatch lives in `harmonicon_score`; this is the loader's own
    // half of it, so a file the crate can't read fails as a load error
    // rather than being parsed as something it isn't.
    let err = convert_score("gp5", midi_file(&[("Harmonica", &easy_notes())]), "A", None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("gp5"), "error should name the format: {err}");
}

#[test]
fn the_artist_comes_from_the_folder_that_holds_the_song() {
    // A MIDI file carries no artist, and the layout already encodes one.
    let path = std::path::Path::new("songs/Sonny Boy/Help Me/song/tune.mid");
    assert_eq!(artist_from_path(path), "Sonny Boy");
}

#[test]
fn an_unexpected_layout_falls_back_rather_than_panicking() {
    assert_eq!(
        artist_from_path(std::path::Path::new("tune.mid")),
        "Imported"
    );
}

#[test]
fn the_song_title_comes_from_its_folder_not_the_track_name() {
    // Seen on screen before this was fixed: a file whose only track is
    // named "Harmonica" produced a song called "Harmonica", because MIDI's
    // title convention is the first track's name.
    let chart = chart_of(
        midi_file(&[("Harmonica", &easy_notes())]),
        "A",
        Some("Scale Practice".into()),
    );
    assert_eq!(chart.song.title, "Scale Practice");
}

#[test]
fn a_track_name_is_never_used_as_a_song_title() {
    // With no folder to fall back on, the result is the neutral default —
    // *not* "Harmonica". `MidiScore::title` reports None precisely so this
    // correction doesn't have to be repeated by every caller.
    let chart = chart_of(midi_file(&[("Harmonica", &easy_notes())]), "A", None);
    assert_ne!(chart.song.title, "Harmonica");
}

#[test]
fn the_title_is_read_from_the_song_folder() {
    let path = std::path::Path::new("songs/Sonny Boy/Help Me/song/tune.mid");
    assert_eq!(title_from_path(path).as_deref(), Some("Help Me"));
}
