// SPDX-License-Identifier: MIT

use super::*;
use crate::ScoreFile;
use harmonicon_core::midi::note_to_midi;

fn chart_json(track: &str, extra_song: &str) -> Vec<u8> {
    format!(
        r#"{{
            "song": {{"title":"T","artist":"A","genre":"Test","tempo_bpm":140.0,"key":"C",
                      "difficulty":"easy"{extra_song}}},
            "timing": {{"resolution":480,"tempo_map":[{{"tick":0,"bpm":140.0}}]}},
            "harmonica": {{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
                "layout": {{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                           "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}}},
            "track": {track},
            "scoring": {{"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}}
        }}"#
    )
    .into_bytes()
}

#[test]
fn a_chart_reports_the_pitches_it_sounds() {
    let bytes = chart_json(
        r#"[{"time":0.0,"duration":0.5,"events":[{"hole":1,"action":"blow","note":"C4"}]},
            {"time":0.5,"duration":0.5,"events":[{"hole":2,"action":"draw","note":"G4"}]}]"#,
        "",
    );
    let score = HarpChartScore::parse(&bytes).unwrap();
    let notes = score.notes(0).unwrap();
    assert_eq!(
        notes.iter().map(|n| n.midi as i32).collect::<Vec<_>>(),
        vec![note_to_midi("C4").unwrap(), note_to_midi("G4").unwrap()]
    );
    assert_eq!(notes[0].start_secs, 0.0);
    assert_eq!(notes[1].start_secs, 0.5);
}

#[test]
fn a_bent_note_reports_the_bent_pitch_not_the_reed() {
    // The trap this reader exists to avoid: `event.note` names the natural
    // reed, and a bend moves the pitch off it. Reporting the reed would
    // hand every downstream consumer a different tune.
    let bytes = chart_json(
        r#"[{"time":0.0,"duration":0.5,"events":[{"hole":3,"action":"draw","note":"B4",
             "modifiers":[{"type":"bend","semitones":-1.0}]}]}]"#,
        "",
    );
    let notes = HarpChartScore::parse(&bytes).unwrap().notes(0).unwrap();
    assert_eq!(notes[0].midi as i32, note_to_midi("B4").unwrap() - 1);
}

#[test]
fn an_item_placed_by_tick_still_resolves_to_seconds() {
    // Charts may state either; a reader that only understood `time` would
    // silently drop half of them.
    let bytes = chart_json(
        r#"[{"tick":480,"duration":0.5,"events":[{"hole":1,"action":"blow","note":"C4"}]}]"#,
        "",
    );
    let notes = HarpChartScore::parse(&bytes).unwrap().notes(0).unwrap();
    // 480 ticks at resolution 480 is one beat; one beat at 140 BPM.
    assert!((notes[0].start_secs - 60.0 / 140.0).abs() < 1e-6);
}

#[test]
fn a_chord_reports_every_one_of_its_pitches() {
    let bytes = chart_json(
        r#"[{"time":0.0,"duration":0.5,"play_mode":"chord","events":[
             {"hole":1,"action":"blow","note":"C4"},
             {"hole":2,"action":"blow","note":"E4"}]}]"#,
        "",
    );
    let notes = HarpChartScore::parse(&bytes).unwrap().notes(0).unwrap();
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().all(|n| n.start_secs == 0.0));
}

#[test]
fn the_time_signature_survives_both_halves() {
    // Every other caller in the tree keeps only the numerator; a score
    // file's meter is the first thing that needs the denominator too.
    let bytes = chart_json(r#"[]"#, r#","time_signature":"6/8""#);
    assert_eq!(
        HarpChartScore::parse(&bytes).unwrap().time_signature(),
        (6, 8)
    );
}

#[test]
fn a_missing_time_signature_falls_back_to_common_time() {
    let bytes = chart_json(r#"[]"#, "");
    assert_eq!(
        HarpChartScore::parse(&bytes).unwrap().time_signature(),
        (4, 4)
    );
}

#[test]
fn parsing_rubbish_is_an_error_rather_than_a_panic() {
    assert!(HarpChartScore::parse(b"not json at all").is_err());
}

#[test]
fn asking_for_a_track_a_chart_does_not_have_is_an_error() {
    let bytes = chart_json(r#"[]"#, "");
    assert!(HarpChartScore::parse(&bytes).unwrap().notes(1).is_err());
}

#[test]
fn time_signature_parsing_rejects_nonsense() {
    assert_eq!(parse_time_signature("6/8"), Some((6, 8)));
    assert_eq!(parse_time_signature(" 3 / 4 "), Some((3, 4)));
    assert_eq!(parse_time_signature("4"), None);
    assert_eq!(parse_time_signature("x/y"), None);
}
