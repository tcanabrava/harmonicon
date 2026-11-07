// SPDX-License-Identifier: MIT

//! `port-part`: writes a guitar, flute or piano part from a harmonica
//! chart, for a song folder that has only the harmonica's.
//!
//! ```text
//! cargo run --bin port-part -- guitar path/to/song/chart.harpchart
//! cargo run --bin port-part -- flute  path/to/song/chart.harpchart
//! cargo run --bin port-part -- piano  path/to/song/chart.harpchart
//! ```
//!
//! It writes `guitar.harpchart`, `flute.harpchart` or `piano.harpchart`
//! beside the source, so
//! the game lists it as another part of the same song (sharing the folder's
//! backing audio). The timing, lyrics and phrases are the source's, and the
//! pitches are the ones the harmonica sounds, bends included, moved by
//! whole octaves to fit the instrument:
//!
//! - **The octave** is the one that puts the most notes in reach, and among
//!   those the one whose median lands nearest the instrument's comfortable
//!   middle: C4 on a guitar (the top strings, where a melody sits), D5 on a
//!   flute.
//! - **A guitar** is standard-tuned and acoustic. Each onset is fingered by
//!   core's own shortest-path solver (`guitar::fingering`), chords
//!   included; an onset it can't voice is reduced to its top note.
//! - **A flute** is a C foot, playing one note at a time: a chord keeps
//!   its top note, the melody. Fingerings are the standard ones.
//! - **A piano** is the full 88 keys and plays every note, chords
//!   included, the melody centred around C5, where a right hand plays it.
//! - **What can't be played is dropped**, and harmonica techniques (bends,
//!   overblows) become the plain note they sounded. Vibrato survives on a
//!   flute.
//!
//! The result is generated, so its description says a live playing pass is
//! still owed. Check it with `validate-pack` afterwards.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use harmonicon_core::chart::{ChartInstrument, HarpChart};
use harmonicon_core::instrument::flute::{Flute, FootJoint};
use harmonicon_core::instrument::guitar::fingering::{Onset, fingering};
use harmonicon_core::instrument::guitar::{Guitar, GuitarBody};
use harmonicon_core::instrument::{Instrument, InstrumentModel};
use harmonicon_core::midi::midi_to_note;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Guitar,
    Flute,
    Clarinet,
    Piano,
}

impl Target {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "guitar" => Some(Self::Guitar),
            "flute" => Some(Self::Flute),
            "clarinet" => Some(Self::Clarinet),
            "piano" => Some(Self::Piano),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Guitar => "guitar",
            Self::Flute => "flute",
            Self::Clarinet => "clarinet",
            Self::Piano => "piano",
        }
    }

    fn instrument(self) -> Instrument {
        match self {
            Self::Guitar => Guitar::standard(GuitarBody::Acoustic).into(),
            Self::Flute => Flute::new(FootJoint::C).into(),
            Self::Clarinet => harmonicon_core::instrument::clarinet::Clarinet::default().into(),
            Self::Piano => harmonicon_core::instrument::piano::Piano::grand().into(),
        }
    }

    /// Where the median of a melody sits comfortably.
    fn middle(self) -> i32 {
        match self {
            Self::Guitar => 60,
            Self::Flute => 74,
            Self::Clarinet => 65,
            Self::Piano => 72,
        }
    }
}

/// The octave shift, in semitones, that puts the most of `pitches` in
/// reach of `instrument`, ties going to the one whose median lands nearest
/// `middle`.
fn best_shift(pitches: &[u8], instrument: &Instrument, middle: i32) -> i32 {
    let mut sorted: Vec<i32> = pitches.iter().map(|&p| i32::from(p)).collect();
    sorted.sort_unstable();
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(middle);
    (-4..=2)
        .map(|octaves| octaves * 12)
        .max_by_key(|shift| {
            let reached = pitches
                .iter()
                .filter_map(|&p| u8::try_from(i32::from(p) + shift).ok())
                .filter(|&p| !instrument.positions_for(p).is_empty())
                .count();
            (reached, -(median + shift - middle).abs())
        })
        .unwrap_or(0)
}

fn shifted(pitch: u8, shift: i32) -> Option<u8> {
    u8::try_from(i32::from(pitch) + shift).ok()
}

/// One track item's sounding pitches, highest first.
fn item_pitches(chart: &HarpChart, item: usize) -> Vec<u8> {
    let mut pitches: Vec<u8> =
        chart.track[item].events.iter().filter_map(|e| chart.event_pitch(e)).collect();
    pitches.sort_unstable_by(|a, b| b.cmp(a));
    pitches.dedup();
    pitches
}

/// Whether a harmonica chart's modifier means anything on `target`.
fn keeps_modifier(target: Target, modifier: &Value) -> bool {
    matches!(target, Target::Flute | Target::Clarinet)
        && modifier.get("type").and_then(Value::as_str) == Some("vibrato")
}

fn port(source: &Path, target: Target) -> Result<(PathBuf, usize, usize, i32), String> {
    let text = std::fs::read_to_string(source).map_err(|e| format!("{}: {e}", source.display()))?;
    let chart: HarpChart =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", source.display()))?;
    if chart.harmonica.is_none() {
        return Err(format!("{} is not a harmonica chart", source.display()));
    }
    let mut out: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let instrument = target.instrument();
    let all: Vec<u8> = (0..chart.track.len()).flat_map(|i| item_pitches(&chart, i)).collect();
    let shift = best_shift(&all, &instrument, target.middle());

    // What each item will play, before it's written: the pitches, and on a
    // guitar the solver's voicing for each onset.
    let wanted: Vec<Vec<u8>> = (0..chart.track.len())
        .map(|i| {
            let pitches: Vec<u8> =
                item_pitches(&chart, i).into_iter().filter_map(|p| shifted(p, shift)).collect();
            match target {
                Target::Flute | Target::Clarinet => pitches.into_iter().take(1).collect(),
                Target::Guitar | Target::Piano => pitches,
            }
        })
        .collect();
    let voicings = match &instrument {
        Instrument::Fretted(guitar) => {
            // An onset the solver can't voice is retried as its top note.
            let onsets = |items: &[Vec<u8>]| -> Vec<Onset> {
                items
                    .iter()
                    .map(|ps| Onset { notes: ps.iter().map(|&p| (p, None)).collect() })
                    .collect()
            };
            let first = fingering(guitar, &onsets(&wanted));
            let reduced: Vec<Vec<u8>> =
                wanted
                    .iter()
                    .zip(&first)
                    .map(|(ps, v)| {
                        if v.is_some() { ps.clone() } else { ps.iter().take(1).copied().collect() }
                    })
                    .collect();
            Some((reduced.clone(), fingering(guitar, &onsets(&reduced))))
        }
        _ => None,
    };

    let items = out["track"].as_array_mut().ok_or("the chart has no track")?;
    let source_items = std::mem::take(items);
    let total = source_items.len();
    for (i, mut item) in source_items.into_iter().enumerate() {
        let modifiers: Vec<Value> = item["events"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|e| e["modifiers"].as_array().cloned().unwrap_or_default())
            .filter(|m| keeps_modifier(target, m))
            .collect();
        let events: Vec<Value> = match &voicings {
            Some((pitches, voicings)) => match &voicings[i] {
                Some(voicing) => pitches[i]
                    .iter()
                    .zip(voicing)
                    .map(|(&p, at)| {
                        json!({
                            "note": midi_to_note(i32::from(p)),
                            "position": { "string": at.string, "fret": at.fret },
                        })
                    })
                    .collect(),
                None => Vec::new(),
            },
            None => wanted[i]
                .iter()
                .filter(|&&p| !instrument.positions_for(p).is_empty())
                .map(|&p| {
                    let mut event = json!({ "note": midi_to_note(i32::from(p)) });
                    if !modifiers.is_empty() {
                        event["modifiers"] = Value::Array(modifiers.clone());
                    }
                    event
                })
                .collect(),
        };
        if events.is_empty() {
            continue;
        }
        item["play_mode"] = json!(if events.len() > 1 { "chord" } else { "single" });
        item["events"] = Value::Array(events);
        items.push(item);
    }
    let kept = items.len();

    let spec = ChartInstrument::from_instrument(&instrument).expect("non-harmonica target");
    let object = out.as_object_mut().ok_or("the chart is not an object")?;
    object.remove("harmonica");
    object.insert("instrument".into(), serde_json::to_value(spec).map_err(|e| e.to_string())?);
    let octaves = shift / 12;
    let moved = match octaves {
        0 => "at the harmonica's pitch".to_string(),
        n if n.abs() == 1 => format!("one octave {}", if n < 0 { "lower" } else { "higher" }),
        n => format!("{} octaves {}", n.abs(), if n < 0 { "lower" } else { "higher" }),
    };
    let what = match target {
        Target::Guitar => "a standard-tuned guitar",
        Target::Flute => "a C-foot flute",
        Target::Clarinet => "a B-flat Boehm soprano clarinet",
        Target::Piano => "an 88-key piano",
    };
    let review = match target {
        Target::Piano => "Generated part",
        Target::Guitar | Target::Flute | Target::Clarinet => "Generated fingering",
    };
    let metadata = &mut out["metadata"];
    metadata["format_version"] = json!(harmonicon_core::chart::CURRENT_FORMAT_VERSION);
    metadata["description"] = json!(format!(
        "The harmonica part's melody, {moved}, for {what}. Timing and phrasing match the \
         harmonica part. {review}; live play review pending."
    ));

    let dest = source.with_file_name(format!("{}.harpchart", target.name()));
    let mut written = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    written.push('\n');
    std::fs::write(&dest, written).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok((dest, kept, total, octaves))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(target), sources) =
        (args.first().and_then(|a| Target::parse(a)), &args[1.min(args.len())..])
    else {
        eprintln!("usage: port-part <guitar|flute> <chart.harpchart>...");
        return ExitCode::FAILURE;
    };
    if sources.is_empty() {
        eprintln!("usage: port-part <guitar|flute> <chart.harpchart>...");
        return ExitCode::FAILURE;
    }
    let mut failed = false;
    for source in sources {
        match port(Path::new(source), target) {
            Ok((dest, kept, total, octaves)) => {
                println!("{}: {kept} of {total} notes, {octaves:+} octave(s)", dest.display())
            }
            Err(error) => {
                eprintln!("{error}");
                failed = true;
            }
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A melody around C6, as harmonica charts sit, goes two octaves down
    /// on a guitar and one on a flute.
    #[test]
    fn the_octave_puts_the_melody_where_the_instrument_is_comfortable() {
        let melody = [84, 86, 88, 89, 91, 88, 84];
        assert_eq!(best_shift(&melody, &Target::Guitar.instrument(), Target::Guitar.middle()), -24);
        assert_eq!(best_shift(&melody, &Target::Flute.instrument(), Target::Flute.middle()), -12);
    }

    /// Reach outranks comfort: a shift that loses notes never wins.
    #[test]
    fn reach_comes_before_the_comfortable_middle() {
        // C4 is a flute's floor. A middle far below it would pull the
        // melody down an octave, out of reach, so it stays where it is.
        let low = [60, 62, 64];
        assert_eq!(best_shift(&low, &Target::Flute.instrument(), 40), 0);
    }

    #[test]
    fn only_vibrato_survives_and_only_on_a_flute() {
        let vibrato = json!({ "type": "vibrato" });
        let bend = json!({ "type": "bend" });
        assert!(keeps_modifier(Target::Flute, &vibrato));
        assert!(!keeps_modifier(Target::Flute, &bend));
        assert!(!keeps_modifier(Target::Guitar, &vibrato));
    }
}
