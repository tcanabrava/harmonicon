// SPDX-License-Identifier: MIT

//! The rules a chart must follow that the schema can't state, because they
//! depend on which instrument the chart names: a harmonica chart's events
//! name holes and a guitar chart's name strings and frets, and each
//! instrument's techniques only make sense on it.
//!
//! Messages are for a chart author reading `validate-pack` output, not for
//! a player, so they are plain English rather than localized.

use crate::chart::{FretPosition, HarpChart, Modifier, NoteEvent};
use crate::instrument::guitar::{FretAssignment, Guitar, StringTechnique};
use crate::instrument::{Instrument, InstrumentModel, PlayPosition};
use crate::midi::note_to_midi;

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
                Instrument::Guitar(guitar) => guitar_event_problem(guitar, event),
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
