// SPDX-License-Identifier: MIT

use super::*;
use crate::instrument::guitar::GuitarBody;

const SONG: &str = r#""song": {"title":"T","artist":"A","genre":"Test","tempo_bpm":120.0,"key":"C","difficulty":"easy"},
    "timing": {"resolution":12,"tempo_map":[{"tick":0,"bpm":120.0}]},
    "scoring": {"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}"#;

const GUITAR: &str =
    r#""instrument": {"type":"guitar","strings":["E4","B3","G3","D3","A2","E2"],"frets":22}"#;

const HARMONICA: &str =
    r#""harmonica": {"type":"diatonic","holes":10,"bending_profile":"richter_standard"}"#;

fn chart(instrument: &str, events: &str) -> HarpChart {
    let json = format!(
        r#"{{ {SONG}, {instrument}, "track": [{{"time":0.0,"duration":0.5,"events":[{events}]}}] }}"#
    );
    serde_json::from_str(&json).unwrap()
}

#[test]
fn a_guitar_chart_reads_its_strings_as_the_guitar_it_names() {
    let chart = chart(GUITAR, r#"{"position":{"string":3,"fret":2},"note":"A3"}"#);
    let Some(Instrument::Guitar(guitar)) = chart.instrument() else {
        panic!("a guitar chart is played on a guitar");
    };
    assert_eq!(guitar.strings, crate::instrument::guitar::STANDARD_TUNING);
    assert_eq!((guitar.frets, guitar.capo, guitar.body), (22, 0, GuitarBody::Electric));
    assert!(chart.harmonica.is_none());
}

#[test]
fn a_sound_guitar_chart_has_no_problems() {
    let events = r#"{"position":{"string":3,"fret":2},"note":"A3"},
        {"position":{"string":2,"fret":8},"note":"A4","modifiers":[{"type":"bend","semitones":2}]},
        {"position":{"string":6,"fret":12},"note":"E3","modifiers":[{"type":"harmonic"}]}"#;
    assert_eq!(instrument_problems(&chart(GUITAR, events)), Vec::<String>::new());
}

#[test]
fn a_guitar_note_must_be_the_pitch_its_position_sounds() {
    let problems =
        instrument_problems(&chart(GUITAR, r#"{"position":{"string":3,"fret":2},"note":"B3"}"#));
    assert_eq!(problems, ["track[0].events[0]: string 3, fret 2 sounds A3, but the note says B3"]);
}

#[test]
fn a_guitar_event_names_a_real_string_and_a_string_technique() {
    let missing = r#"{"position":{"string":7,"fret":0},"note":"E4"}"#;
    assert!(instrument_problems(&chart(GUITAR, missing))[0].contains("not on a 6-string"));
    let down = r#"{"position":{"string":3,"fret":2},"note":"A3","modifiers":[{"type":"bend","semitones":-1}]}"#;
    assert!(instrument_problems(&chart(GUITAR, down))[0].contains("raises the pitch"));
    let overblow =
        r#"{"position":{"string":3,"fret":2},"note":"A3","modifiers":[{"type":"overblow"}]}"#;
    assert!(instrument_problems(&chart(GUITAR, overblow))[0].contains("harmonica techniques"));
    let holed = r#"{"hole":4,"action":"blow","note":"C5"}"#;
    assert!(instrument_problems(&chart(GUITAR, holed))[0].contains("not a hole"));
}

#[test]
fn a_harmonica_event_names_a_hole_and_bends_down() {
    assert!(instrument_problems(&chart(HARMONICA, r#"{"hole":4,"action":"blow"}"#)).is_empty());
    let fretted = r#"{"position":{"string":3,"fret":2},"note":"A3"}"#;
    assert!(instrument_problems(&chart(HARMONICA, fretted))[0].contains("needs a hole"));
    let up = r#"{"hole":3,"action":"draw","modifiers":[{"type":"bend","semitones":1}]}"#;
    assert!(instrument_problems(&chart(HARMONICA, up))[0].contains("lowers the reed"));
    let harmonic = r#"{"hole":4,"action":"blow","modifiers":[{"type":"harmonic"}]}"#;
    assert!(instrument_problems(&chart(HARMONICA, harmonic))[0].contains("string technique"));
}

#[test]
fn a_guitar_events_pitch_is_its_note() {
    let chart = chart(
        GUITAR,
        r#"{"position":{"string":2,"fret":8},"note":"A4","modifiers":[{"type":"bend","semitones":2}]}"#,
    );
    let event = &chart.track[0].events[0];
    assert_eq!(chart.event_pitch(event), Some(69), "the bent, sounding pitch");
}

#[test]
fn a_harmonica_bends_pitch_comes_from_the_reed_and_the_modifier() {
    let chart = chart(
        HARMONICA,
        r#"{"hole":3,"action":"draw","note":"B4","modifiers":[{"type":"bend","semitones":-2}]}"#,
    );
    assert_eq!(chart.event_pitch(&chart.track[0].events[0]), Some(69), "B4 bent to A4");
}

#[test]
fn a_harmonica_chart_round_trips_without_the_new_fields() {
    let chart = chart(HARMONICA, r#"{"hole":4,"action":"blow"}"#);
    let json = serde_json::to_value(&chart).unwrap();
    assert!(json.get("instrument").is_none());
    assert!(json["track"][0]["events"][0].get("position").is_none());
    assert_eq!(json["track"][0]["events"][0]["hole"], 4);
}

#[test]
fn a_guitar_chart_round_trips_its_instrument_and_positions() {
    let chart = chart(GUITAR, r#"{"position":{"string":3,"fret":2},"note":"A3"}"#);
    let json = serde_json::to_value(&chart).unwrap();
    assert!(json.get("harmonica").is_none());
    assert_eq!(json["instrument"]["type"], "guitar");
    assert!(json["instrument"].get("capo").is_none(), "no capo is not written");
    assert_eq!(json["track"][0]["events"][0]["position"]["fret"], 2);
    let back: HarpChart = serde_json::from_value(json).unwrap();
    assert_eq!(back.instrument_spec, chart.instrument_spec);
}
