// SPDX-License-Identifier: MIT

//! The rules a chart must follow that the schema can't state: its tempo and
//! meter maps run in tick order, its scoring windows nest, and its events
//! fit the instrument it names
//! (a harmonica chart's events name holes and a guitar chart's name strings
//! and frets, and each instrument's techniques only make sense on it).
//!
//! Messages are for a chart author reading `validate-pack` output, not for
//! a player, so they are plain English rather than localized.

use crate::chart::{FretPosition, HarpChart, Modifier, NoteEvent, Scoring, Timing};
use crate::instrument::flute::{FingeringAssignment, Flute};
use crate::instrument::guitar::{FretAssignment, Guitar, StringTechnique};
use crate::instrument::{Instrument, InstrumentModel, PlayPosition};
use crate::midi::note_to_midi;

/// Every rule `chart` breaks that its schema can't catch:
/// [`timing_problems`], [`scoring_problems`] and [`instrument_problems`].
/// Empty for a sound chart. What a loader or validator should run.
pub fn chart_problems(chart: &HarpChart) -> Vec<String> {
    let mut problems = timing_problems(&chart.timing);
    problems.extend(scoring_problems(&chart.scoring));
    problems.extend(instrument_problems(chart));
    problems
}

/// Whether the judging windows nest: perfect within good within miss, the
/// order `scoring::classify_note` tests them in. Out of order, a band of
/// timings becomes unhittable (a miss window inside the good one marks a
/// note missed while it still reads as on time) or a grade unreachable (a
/// perfect window wider than the good one awards nothing but Perfect).
pub fn scoring_problems(scoring: &Scoring) -> Vec<String> {
    let (perfect, good, miss) =
        (scoring.perfect_window_ms, scoring.good_window_ms, scoring.miss_window_ms);
    if perfect <= good && good <= miss {
        return Vec::new();
    }
    vec![format!(
        "scoring windows must nest perfect <= good <= miss, but are {perfect}, {good} and \
         {miss} ms"
    )]
}

/// Where `timing`'s maps break the order every tick↔seconds conversion
/// assumes: the tempo map must start at tick 0 (before its first point
/// there is no tempo to count with), and both maps must be in tick order.
/// The schema can bound each point but not compare neighbours.
pub fn timing_problems(timing: &Timing) -> Vec<String> {
    let mut problems = Vec::new();
    if let Some(first) = timing.tempo_map.first()
        && first.tick != 0
    {
        problems.push(format!("timing.tempo_map starts at tick {}, not 0", first.tick));
    }
    let tempo_ticks = timing.tempo_map.iter().map(|p| p.tick);
    problems.extend(out_of_order("timing.tempo_map", tempo_ticks));
    let meter_ticks = timing.time_signature_map.iter().flatten().map(|p| p.tick);
    problems.extend(out_of_order("timing.time_signature_map", meter_ticks));
    problems
}

/// The first point in `ticks` earlier than the one before it, if any.
fn out_of_order(field: &str, ticks: impl Iterator<Item = u64>) -> Option<String> {
    let ticks: Vec<u64> = ticks.collect();
    let index = ticks.windows(2).position(|pair| pair[1] < pair[0])? + 1;
    Some(format!(
        "{field}[{index}] is at tick {}, before the point ahead of it at {}",
        ticks[index],
        ticks[index - 1]
    ))
}

/// Every way `chart`'s events disagree with its instrument, each naming the
/// track item and event it is about. Empty for a sound chart.
pub fn instrument_problems(chart: &HarpChart) -> Vec<String> {
    let Some(instrument) = chart.instrument() else {
        return vec!["the instrument's strings name a note that does not exist".to_string()];
    };
    let mut problems = Vec::new();
    for (item_index, item) in chart.track.iter().enumerate() {
        for (event_index, event) in item.events.iter().enumerate() {
            let at = format!("track[{item_index}].events[{event_index}]");
            let problem = match &instrument {
                Instrument::Harmonica(_) => harmonica_event_problem(event),
                Instrument::Fretted(guitar) => guitar_event_problem(guitar, event),
                Instrument::Flute(flute) => flute_event_problem(flute, event),
                Instrument::Piano(piano) => piano_event_problem(piano, event),
            };
            problems.extend(problem.map(|p| format!("{at}: {p}")));
        }
    }
    problems
}

fn modifiers(event: &NoteEvent) -> &[Modifier] {
    event.modifiers.as_deref().unwrap_or(&[])
}

fn harmonica_event_problem(event: &NoteEvent) -> Option<String> {
    if event.hole_action().is_none() {
        return Some("a harmonica event needs a hole and an action".to_string());
    }
    if event.position.is_some() {
        return Some("a harmonica event has no string position".to_string());
    }
    for modifier in modifiers(event) {
        match modifier {
            Modifier::Bend { semitones, .. } if *semitones > 0.0 => {
                return Some(format!(
                    "a harmonica bend lowers the reed, so its semitones are negative, not \
                     {semitones}"
                ));
            }
            Modifier::Harmonic => {
                return Some("a harmonic is a string technique, not a harmonica one".to_string());
            }
            _ => {}
        }
    }
    None
}

fn guitar_event_problem(guitar: &Guitar, event: &NoteEvent) -> Option<String> {
    if event.hole.is_some() || event.action.is_some() {
        return Some("a guitar event names a string position, not a hole or action".to_string());
    }
    let Some(FretPosition { string, fret }) = event.position else {
        return Some("a guitar event needs a string position".to_string());
    };
    let Some(note) = event.note.as_deref() else {
        return Some("a guitar event needs its note".to_string());
    };
    if guitar.open_pitch(string).is_none() {
        return Some(format!("string {string} is not on a {}-string guitar", guitar.strings.len()));
    }
    let technique = match guitar_technique(modifiers(event)) {
        Ok(technique) => technique,
        Err(problem) => return Some(problem),
    };
    let position = PlayPosition::Fret(FretAssignment { string, fret, technique });
    let stated = note_to_midi(note).and_then(|m| u8::try_from(m).ok());
    match InstrumentModel::pitch_of(guitar, &position) {
        None => Some(format!("string {string}, fret {fret} cannot be played that way here")),
        Some(sounds) if Some(sounds) != stated => Some(format!(
            "string {string}, fret {fret} sounds {}, but the note says {note}",
            crate::midi::midi_to_note(i32::from(sounds))
        )),
        Some(_) => None,
    }
}

fn flute_event_problem(flute: &Flute, event: &NoteEvent) -> Option<String> {
    if event.hole.is_some() || event.action.is_some() || event.position.is_some() {
        return Some(
            "a flute event names a note and a fingering, not a hole or string".to_string(),
        );
    }
    let Some(note) = event.note.as_deref() else {
        return Some("a flute event needs its note".to_string());
    };
    if let Some(other) = modifiers(event).iter().find(|m| !matches!(m, Modifier::Vibrato { .. })) {
        return Some(format!("a flute has no {other:?}; only vibrato"));
    }
    let Some(midi) = note_to_midi(note).and_then(|m| u8::try_from(m).ok()) else {
        return Some(format!("{note} is not a note"));
    };
    let alternate = event.fingering.unwrap_or(0);
    let position = PlayPosition::Fingering(FingeringAssignment { note: midi, alternate });
    match InstrumentModel::pitch_of(flute, &position) {
        Some(_) => None,
        None if flute.fingerings_for(midi).is_empty() => {
            Some(format!("{note} is outside this flute's range"))
        }
        None => Some(format!("{note} has no fingering {alternate}")),
    }
}

fn piano_event_problem(
    piano: &crate::instrument::piano::Piano,
    event: &NoteEvent,
) -> Option<String> {
    if event.hole.is_some()
        || event.action.is_some()
        || event.position.is_some()
        || event.fingering.is_some()
    {
        return Some("a piano event names only its note".to_string());
    }
    let Some(note) = event.note.as_deref() else {
        return Some("a piano event needs its note".to_string());
    };
    if let Some(other) = modifiers(event).first() {
        return Some(format!("a piano has no {other:?}"));
    }
    let Some(midi) = note_to_midi(note).and_then(|m| u8::try_from(m).ok()) else {
        return Some(format!("{note} is not a note"));
    };
    (!piano.has(midi)).then(|| format!("{note} is outside a {}-key keyboard", piano.keys))
}

/// The string technique a guitar event's modifiers name: a bend up, a
/// harmonic, or the plain fretted note. Modifiers a guitar can't play (which
/// validation reports) are ignored here.
pub fn string_technique(modifiers: &[Modifier]) -> StringTechnique {
    guitar_technique(modifiers).unwrap_or(StringTechnique::Fretted)
}

/// The string technique a guitar event's modifiers name, or why they name
/// none a guitar has.
fn guitar_technique(modifiers: &[Modifier]) -> Result<StringTechnique, String> {
    let mut technique = StringTechnique::Fretted;
    for modifier in modifiers {
        match modifier {
            Modifier::Bend { semitones, .. } if *semitones < 0.0 => {
                return Err(format!(
                    "a string bend raises the pitch, so its semitones are positive, not \
                     {semitones}"
                ));
            }
            Modifier::Bend { semitones, .. } => technique = StringTechnique::Bend(*semitones),
            Modifier::Harmonic => technique = StringTechnique::Harmonic,
            Modifier::Overblow | Modifier::Overdraw | Modifier::Slide => {
                return Err("overblow, overdraw and slide are harmonica techniques".to_string());
            }
            Modifier::Vibrato { .. } | Modifier::WahWah { .. } => {}
        }
    }
    Ok(technique)
}

#[cfg(test)]
mod tests;
