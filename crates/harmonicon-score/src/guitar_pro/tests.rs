// SPDX-License-Identifier: MIT

use super::*;
use guitarpro::model::legacy::beat::{Beat, Voice};
use guitarpro::model::legacy::headers::MeasureHeader;
use guitarpro::model::legacy::measure::Measure;
use guitarpro::model::legacy::note::Note;
use guitarpro::model::legacy::track::Track;

/// Standard six-string guitar tuning, as Guitar Pro stores it: string 1 is
/// the *highest*, and each entry is `(number, open-string MIDI pitch)`.
fn standard_tuning() -> Vec<(i8, i8)> {
    vec![(1, 64), (2, 59), (3, 55), (4, 50), (5, 45), (6, 40)]
}

/// One quarter note per beat, on the given `(string, fret)` pairs.
///
/// Positions start at `DURATION_QUARTER_TIME` because Guitar Pro numbers
/// from one quarter in, not from zero — the offset `origin` exists to undo.
fn track_of(name: &str, tuning: Vec<(i8, i8)>, played: &[(i8, i16)]) -> Track {
    let mut beats = Vec::new();
    for (index, &(string, fret)) in played.iter().enumerate() {
        beats.push(Beat {
            notes: vec![Note { value: fret, string, kind: NoteType::Normal, ..Default::default() }],
            start: Some(960 + 960 * index as i64),
            ..Default::default()
        });
    }
    Track {
        name: name.to_string(),
        strings: tuning,
        measures: vec![Measure {
            voices: vec![Voice { beats, ..Default::default() }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn song_of(tracks: Vec<Track>) -> GpSong {
    GpSong {
        name: "Test Piece".to_string(),
        tempo: 120,
        tracks,
        measure_headers: vec![MeasureHeader { start: 960, tempo: 120, ..Default::default() }],
        ..Default::default()
    }
}

fn score_of(tracks: Vec<Track>) -> GpScore {
    GpScore::from_song(song_of(tracks), ScoreFormat::GuitarPro)
}

// ── Pitch ────────────────────────────────────────────────────────────────────

#[test]
fn a_fret_becomes_a_pitch_by_way_of_its_strings_tuning() {
    // The whole reason a tab needs a reader rather than a field read: the
    // file says "third fret, second string", and only the track's tuning
    // turns that into a sounding note. Open first string is E4 (64).
    let score =
        score_of(vec![track_of("Guitar", standard_tuning(), &[(1, 0), (1, 3), (2, 1), (6, 5)])]);
    let pitches: Vec<u8> = score.notes(0).unwrap().iter().map(|n| n.midi).collect();
    assert_eq!(pitches, vec![64, 67, 60, 45]);
}

#[test]
fn a_retuned_string_moves_every_note_on_it() {
    // Drop D: the sixth string goes from E2 (40) to D2 (38), so the same
    // fret sounds two semitones lower. A reader that assumed standard
    // tuning would be wrong on a large share of real tabs.
    let mut drop_d = standard_tuning();
    drop_d[5] = (6, 38);
    let score = score_of(vec![track_of("Guitar", drop_d, &[(6, 0), (6, 5)])]);
    let pitches: Vec<u8> = score.notes(0).unwrap().iter().map(|n| n.midi).collect();
    assert_eq!(pitches, vec![38, 43]);
}

#[test]
fn tied_rest_and_dead_notes_produce_no_pitch() {
    // A tie continues the note before it — counting it again would double
    // every tied pair. A dead note is a muted click with no pitch at all.
    let mut track = track_of("Guitar", standard_tuning(), &[(1, 0), (1, 2), (1, 4)]);
    let beats = &mut track.measures[0].voices[0].beats;
    beats[1].notes[0].kind = NoteType::Tie;
    beats[2].notes[0].kind = NoteType::Dead;
    let score = score_of(vec![track]);
    let pitches: Vec<u8> = score.notes(0).unwrap().iter().map(|n| n.midi).collect();
    assert_eq!(pitches, vec![64]);
}

#[test]
fn an_out_of_range_fret_is_dropped_rather_than_wrapping() {
    // A corrupt or exotic file can put a fret past MIDI's range; wrapping a
    // u8 would land it somewhere plausible and wrong.
    let score = score_of(vec![track_of("Guitar", standard_tuning(), &[(1, 0), (1, 120)])]);
    assert_eq!(score.notes(0).unwrap().len(), 1);
}

// ── Timing ───────────────────────────────────────────────────────────────────

#[test]
fn the_first_note_starts_at_zero_not_one_beat_in() {
    // Guitar Pro numbers positions from one quarter note in. Without
    // subtracting that origin every song would begin a beat late — silent
    // and uniform, so it would read as "the game is off" rather than a bug.
    let score = score_of(vec![track_of("Guitar", standard_tuning(), &[(1, 0)])]);
    assert_eq!(score.notes(0).unwrap()[0].start_secs, 0.0);
}

#[test]
fn quarter_notes_at_120_bpm_land_half_a_second_apart() {
    let score = score_of(vec![track_of("Guitar", standard_tuning(), &[(1, 0), (1, 2), (1, 4)])]);
    let starts: Vec<f64> = score.notes(0).unwrap().iter().map(|n| n.start_secs).collect();
    assert_eq!(starts, vec![0.0, 0.5, 1.0]);
}

/// A measure of quarter notes with *no* stated beat positions — the shape
/// GP6/GP7, MuseScore and MusicXML all arrive in.
fn unplaced_measure(header_index: usize, played: &[(i8, i16)]) -> Measure {
    let beats = played
        .iter()
        .map(|&(string, fret)| Beat {
            notes: vec![Note { value: fret, string, kind: NoteType::Normal, ..Default::default() }],
            start: None,
            ..Default::default()
        })
        .collect();
    Measure {
        header_index,
        voices: vec![Voice { beats, ..Default::default() }],
        ..Default::default()
    }
}

#[test]
fn a_tempo_change_at_a_bar_line_shifts_only_what_follows_it() {
    // The reason a tempo *map* is walked rather than one BPM divided
    // through: a piece that doubles tempo would otherwise drift further out
    // of time with every bar after it. Tempo changes belong to a measure
    // header, so they land on bar lines, never mid-bar.
    let mut song = song_of(vec![Track {
        name: "Harmonica".to_string(),
        strings: standard_tuning(),
        measures: vec![
            unplaced_measure(0, &[(1, 0), (1, 2)]),
            unplaced_measure(1, &[(1, 4), (1, 5)]),
        ],
        ..Default::default()
    }]);
    song.measure_headers.push(MeasureHeader { tempo: 240, ..Default::default() });
    let score = GpScore::from_song(song, ScoreFormat::GuitarPro);
    let starts: Vec<f64> = score.notes(0).unwrap().iter().map(|n| n.start_secs).collect();
    // Bar one at 120 BPM: quarters half a second apart, the bar four beats
    // long. Bar two at 240 BPM: a quarter of a second apart.
    assert_eq!(starts, vec![0.0, 0.5, 2.0, 2.25]);
}

#[test]
fn a_file_that_states_no_beat_positions_still_yields_notes() {
    // GP6/GP7 (and so MuseScore and MusicXML, which convert through them)
    // build beats from durations and never fill `Beat::start`. Reading only
    // that field returned no notes at all for four of the seven formats
    // here — silently, as an empty song.
    let song = song_of(vec![Track {
        name: "Harmonica".to_string(),
        strings: standard_tuning(),
        measures: vec![unplaced_measure(0, &[(1, 0), (1, 3), (2, 1)])],
        ..Default::default()
    }]);
    let score = GpScore::from_song(song, ScoreFormat::GuitarPro);
    let notes = score.notes(0).unwrap();
    assert_eq!(notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![64, 67, 60]);
    assert_eq!(notes.iter().map(|n| n.start_secs).collect::<Vec<_>>(), vec![0.0, 0.5, 1.0]);
}

#[test]
fn a_note_never_overruns_the_one_after_it() {
    // Lengths are inferred, so they are clamped to the next onset. A
    // written whole note followed a quarter later by another note must
    // stop at that note, or the two overlap on an instrument that can only
    // sound one thing per hole.
    let mut track = track_of("Guitar", standard_tuning(), &[(1, 0), (1, 2)]);
    track.measures[0].voices[0].beats[0].duration.value = 1; // whole note
    let score = score_of(vec![track]);
    let notes = score.notes(0).unwrap();
    assert_eq!(notes[0].duration_secs, 0.5);
    assert!(notes[0].start_secs + notes[0].duration_secs <= notes[1].start_secs);
}

// ── Tracks ───────────────────────────────────────────────────────────────────

#[test]
fn a_percussion_track_offers_no_notes() {
    // Its "frets" are drum-kit indices. Read as pitches they make a
    // plausible-looking part that could win the track picker outright and
    // then play nothing resembling the song.
    let mut drums = track_of("Drums", standard_tuning(), &[(1, 0), (1, 2)]);
    drums.percussion_track = true;
    let score = score_of(vec![track_of("Harmonica", standard_tuning(), &[(1, 0)]), drums]);
    assert_eq!(score.tracks()[0].note_count, 1);
    assert_eq!(score.tracks()[1].note_count, 0);
    assert!(!score.tracks()[1].is_playable());
}

#[test]
fn a_tracks_name_carries_through_so_the_harmonica_can_be_found() {
    let score = score_of(vec![
        track_of("Guitar", standard_tuning(), &[(1, 0)]),
        track_of("Harmonica", standard_tuning(), &[(1, 0)]),
    ]);
    assert_eq!(crate::pick_harmonica_track(score.tracks()), Some(1));
}

#[test]
fn a_blank_track_name_is_reported_as_absent_not_as_empty_text() {
    // `pick_harmonica_track` and the picker both distinguish "unnamed"
    // from "named something"; an empty string would read as the latter.
    let score = score_of(vec![track_of("   ", standard_tuning(), &[(1, 0)])]);
    assert_eq!(score.tracks()[0].name, None);
}

#[test]
fn asking_for_a_track_that_is_not_there_is_an_error() {
    let score = score_of(vec![track_of("Guitar", standard_tuning(), &[(1, 0)])]);
    assert!(matches!(score.notes(7), Err(ScoreError::NoSuchTrack(7))));
}

#[test]
fn the_files_own_title_is_reported() {
    // Unlike MIDI, these formats have a real title field.
    let score = score_of(vec![track_of("Guitar", standard_tuning(), &[(1, 0)])]);
    assert_eq!(score.title(), Some("Test Piece"));
}

// ── Real files ───────────────────────────────────────────────────────────────

/// Writing a score out and reading it back through the public dispatch —
/// the only test here that exercises real container parsing rather than the
/// in-memory model. The crate ships no fixtures of its own (its tests read
/// `test/*.gp4` files that aren't in the published package), so a
/// round-trip is what's available without hand-authoring binary tabs.
fn round_trip(extension: &str, bytes: Vec<u8>) {
    let score = crate::parse_import(extension, bytes)
        .unwrap_or_else(|e| panic!(".{extension} failed to read back: {e}"));
    let harmonica = crate::pick_harmonica_track(score.tracks())
        .unwrap_or_else(|| panic!(".{extension} lost its track names"));
    let pitches: Vec<u8> = score.notes(harmonica).unwrap().iter().map(|n| n.midi).collect();
    assert_eq!(pitches, vec![64, 67, 60], "{extension} changed the notes");
}

#[test]
fn a_gp7_file_round_trips_through_the_public_dispatch() {
    let song = song_of(vec![
        track_of("Guitar", standard_tuning(), &[(6, 0)]),
        track_of("Harmonica", standard_tuning(), &[(1, 0), (1, 3), (2, 1)]),
    ]);
    let bytes = song.write_gp().expect("failed to write a .gp file");
    round_trip("gp", bytes);
}

#[test]
fn a_gp6_file_round_trips_through_the_public_dispatch() {
    let song = song_of(vec![
        track_of("Guitar", standard_tuning(), &[(6, 0)]),
        track_of("Harmonica", standard_tuning(), &[(1, 0), (1, 3), (2, 1)]),
    ]);
    let bytes = song.write_gpx().expect("failed to write a .gpx file");
    round_trip("gpx", bytes);
}

#[test]
fn garbage_bytes_are_refused_rather_than_panicking() {
    // A player can rename anything to .gp5. Every format must fail as an
    // error, since a panic inside an asset loader takes the game with it.
    for extension in ["gp3", "gp4", "gp5", "gpx", "gp", "mscz", "musicxml"] {
        let err = crate::parse_import(extension, b"definitely not a tab".to_vec());
        assert!(err.is_err(), ".{extension} accepted garbage");
    }
}

// ── .mxl and MusicXML sniffing ─────────────────────────────────────────────

/// A minimal MusicXML document — one part named `Harmonica`, one bar, one
/// quarter-note C4 — written by hand like an exporter would, since the
/// `guitarpro` crate's own serializer doesn't round-trip through its
/// deserializer.
fn musicxml_bytes() -> Vec<u8> {
    br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Harmonica</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration>
        <type>quarter</type>
      </note>
    </measure>
  </part>
</score-partwise>
"#
    .to_vec()
}

/// Zips `entries` as an `.mxl` would be laid out.
fn zipped(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    cursor.into_inner()
}

#[test]
fn an_mxl_is_read_through_its_container_manifest() {
    let xml = musicxml_bytes();
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container><rootfiles><rootfile full-path="score.xml" media-type="application/vnd.recordare.musicxml+xml"/></rootfiles></container>"#;
    let mxl = zipped(&[
        ("META-INF/container.xml", container.as_slice()),
        // A decoy first entry: the manifest, not entry order, decides.
        ("readme.xml", b"<notes>not a score</notes>"),
        ("score.xml", &xml),
    ]);
    let score = crate::parse_import("mxl", mxl).expect("read .mxl");
    assert_eq!(score.tracks().len(), 1);
    assert_eq!(score.tracks()[0].name.as_deref(), Some("Harmonica"));
}

#[test]
fn an_mxl_without_a_manifest_falls_back_to_its_first_xml() {
    let xml = musicxml_bytes();
    let mxl = zipped(&[("anything.musicxml", &xml)]);
    let plain = crate::parse_import("musicxml", xml.clone());
    assert!(plain.is_ok(), "the serialized MusicXML itself: {:?}", plain.err());
    let read = crate::parse_import("mxl", mxl);
    assert!(read.is_ok(), "{:?}", read.err());
    let empty = zipped(&[("META-INF/container.xml", b"<container/>")]);
    assert!(crate::parse_import("mxl", empty).is_err(), "nothing to read");
    assert!(crate::parse_import("mxl", b"not a zip".to_vec()).is_err());
}

#[test]
fn container_rootfile_reads_the_first_full_path() {
    assert_eq!(
        container_rootfile(r#"<rootfiles><rootfile full-path="a/b.xml"/></rootfiles>"#),
        Some("a/b.xml".to_string())
    );
    assert_eq!(
        container_rootfile(
            r#"<ocf:container xmlns:ocf="urn:oasis:names:tc:opendocument:xmlns:container"><ocf:rootfile media-type="application/vnd.recordare.musicxml+xml" full-path='escaped &amp; score.xml'/></ocf:container>"#
        ),
        Some("escaped & score.xml".to_string())
    );
    assert_eq!(container_rootfile("<container/>"), None);
}

#[test]
fn only_a_musicxml_root_element_reads_as_a_score() {
    use crate::looks_like_musicxml;
    assert!(looks_like_musicxml(
        b"<?xml version=\"1.0\"?>\n<!DOCTYPE score-partwise PUBLIC ...>\n<score-partwise version=\"4.0\">"
    ));
    assert!(looks_like_musicxml(b"<score-timewise>"));
    assert!(!looks_like_musicxml(
        b"<?xml version=\"1.0\"?><settings><volume>1</volume></settings>"
    ));
    assert!(!looks_like_musicxml(b"<!-- <score-partwise> is only mentioned here --><settings/>"));
    assert!(!looks_like_musicxml(b""));
}

// ── Repeats ──────────────────────────────────────────────────────────────────

/// One 4/4 bar's marks at bar `index` (3840 ticks a bar).
fn bar(index: u64, open: bool, close: i8, alternative: u8) -> MeasureMarks {
    MeasureMarks { start: index * 3840, end: (index + 1) * 3840, open, close, alternative }
}

#[test]
fn a_close_without_an_open_repeats_from_the_top() {
    let repeats =
        repeats_from_marks(&[bar(0, false, -1, 0), bar(1, false, 1, 0), bar(2, false, -1, 0)]);
    assert_eq!(repeats.len(), 1);
    assert_eq!((repeats[0].start_tick, repeats[0].end_tick), (0, 2 * 3840));
    assert_eq!(repeats[0].passes(), 2, "one repeat is two passes");
}

#[test]
fn first_and_second_endings_attach_to_their_repeat() {
    // |: A | B | 1. C :| 2. D | E
    let repeats = repeats_from_marks(&[
        bar(0, true, -1, 0),
        bar(1, false, -1, 0),
        bar(2, false, 1, 0b01),
        bar(3, false, -1, 0b10),
        bar(4, false, -1, 0),
    ]);
    assert_eq!(repeats.len(), 1);
    let endings: Vec<(u64, u64, Vec<u32>)> = repeats[0]
        .endings
        .iter()
        .map(|e| (e.start_tick / 3840, e.end_tick / 3840, e.passes.clone()))
        .collect();
    assert_eq!(endings, vec![(2, 3, vec![1]), (3, 4, vec![2])]);
}

#[test]
fn a_multi_bar_ending_is_one_ending() {
    let repeats =
        repeats_from_marks(&[bar(0, true, -1, 0), bar(1, false, -1, 0b01), bar(2, false, 1, 0b01)]);
    assert_eq!(repeats[0].endings.len(), 1);
    assert_eq!(repeats[0].endings[0].start_tick, 3840);
    assert_eq!(repeats[0].endings[0].end_tick, 3 * 3840);
}

#[test]
fn a_repeated_bar_is_played_again_with_its_endings() {
    // |: bar 1 | 1. bar 2 :| 2. bar 3 |, one quarter note per bar at 120.
    let mut song = song_of(vec![Track {
        name: "Harmonica".to_string(),
        strings: standard_tuning(),
        measures: vec![
            unplaced_measure(0, &[(1, 0)]),
            unplaced_measure(1, &[(1, 2)]),
            unplaced_measure(2, &[(1, 4)]),
        ],
        ..Default::default()
    }]);
    song.measure_headers[0].repeat_open = true;
    song.measure_headers.push(MeasureHeader {
        repeat_close: 1,
        repeat_alternative: 0b01,
        ..Default::default()
    });
    song.measure_headers.push(MeasureHeader { repeat_alternative: 0b10, ..Default::default() });
    let score = GpScore::from_song(song, ScoreFormat::GuitarPro);
    let notes = score.notes(0).unwrap();
    let played: Vec<(f64, u8)> = notes.iter().map(|n| (n.start_secs, n.midi)).collect();
    // A bar of 4/4 at 120 is two seconds: bar 1, 1st ending, bar 1 again,
    // then the 2nd ending.
    assert_eq!(played, vec![(0.0, 64), (2.0, 66), (4.0, 64), (6.0, 68)]);
}

#[test]
fn a_repeat_survives_a_gp7_round_trip() {
    // The crate's own GPIF reader, not the in-memory model: the marks have
    // to come back out of a real container for any of this to matter.
    let mut song = song_of(vec![Track {
        name: "Harmonica".to_string(),
        strings: standard_tuning(),
        measures: vec![unplaced_measure(0, &[(1, 0)]), unplaced_measure(1, &[(1, 2)])],
        ..Default::default()
    }]);
    song.measure_headers[0].repeat_open = true;
    song.measure_headers[0].repeat_close = 1;
    song.measure_headers.push(MeasureHeader::default());
    let bytes = song.write_gp().expect("failed to write a .gp file");
    let score = crate::parse_import("gp", bytes).unwrap();
    let pitches: Vec<u8> = score.notes(0).unwrap().iter().map(|n| n.midi).collect();
    assert_eq!(pitches, vec![64, 64, 66], "the first bar is played twice");
}

#[test]
fn a_gp7_file_keeps_each_bar_in_its_place() {
    // The `.gp`/`.gpx` reader leaves every measure's `header_index` at 0;
    // trusting it stacked the whole tune on the first bar. Pitches alone
    // (as `round_trip` checks) can't see that — only the times can.
    let mut song = song_of(vec![Track {
        name: "Harmonica".to_string(),
        strings: standard_tuning(),
        measures: vec![
            unplaced_measure(0, &[(1, 0)]),
            unplaced_measure(1, &[(1, 2)]),
            unplaced_measure(2, &[(1, 4)]),
        ],
        ..Default::default()
    }]);
    song.measure_headers.push(MeasureHeader::default());
    song.measure_headers.push(MeasureHeader::default());
    for (extension, bytes) in [("gp", song.write_gp().unwrap()), ("gpx", song.write_gpx().unwrap())]
    {
        let score = crate::parse_import(extension, bytes).unwrap();
        let starts: Vec<f64> = score.notes(0).unwrap().iter().map(|n| n.start_secs).collect();
        assert_eq!(starts, vec![0.0, 2.0, 4.0], ".{extension}");
    }
}
