// SPDX-License-Identifier: MIT

//! Conversion between chart positions and layout-independent note intent.
//! Expression modifiers are retained on the chart event, outside the target.
use crate::chart::{FretPosition, HarpChart, Modifier, NoteEvent};
use crate::instrument::flute::FingeringAssignment;
use crate::instrument::guitar::FretAssignment;
use crate::instrument::{Instrument, InstrumentModel, PlayPosition};
use crate::note_target::{NoteTarget, PlacementError};
use crate::pitch_map::{HoleAssignment, Technique};

pub fn decode(chart: &HarpChart, event: &NoteEvent) -> Result<NoteTarget, PlacementError> {
    let instrument = chart.instrument().ok_or(PlacementError::Unplayable)?;
    let modifiers = event.modifiers.as_deref().unwrap_or(&[]);
    let pitch_modifiers = modifiers
        .iter()
        .filter(|m| !matches!(m, Modifier::Vibrato { .. } | Modifier::WahWah { .. }))
        .count();
    if pitch_modifiers > 1 {
        return Err(PlacementError::Unplayable);
    }
    match &instrument {
        Instrument::Harmonica(_) => {
            if event.position.is_some() || modifiers.iter().any(|m| matches!(m, Modifier::Harmonic))
            {
                return Err(PlacementError::Unplayable);
            }
            let (hole, action) = event.hole_action().ok_or(PlacementError::Unplayable)?;
            let position = PlayPosition::Hole(HoleAssignment {
                hole,
                action,
                technique: crate::pitch_map::technique_of(modifiers),
            });
            let target = NoteTarget::Positioned(position);
            target.candidates(&instrument)?;
            Ok(target)
        }
        Instrument::Guitar(_) => {
            if event.hole.is_some()
                || event.action.is_some()
                || modifiers
                    .iter()
                    .any(|m| matches!(m, Modifier::Overblow | Modifier::Overdraw | Modifier::Slide))
            {
                return Err(PlacementError::Unplayable);
            }
            let at = event.position.ok_or(PlacementError::Unplayable)?;
            let position = PlayPosition::Fret(FretAssignment {
                string: at.string,
                fret: at.fret,
                technique: crate::chart_check::string_technique(modifiers),
            });
            let midi = chart.event_pitch(event).ok_or(PlacementError::Unplayable)?;
            if instrument.pitch_of(&position) != Some(midi) {
                return Err(PlacementError::PitchMismatch);
            }
            let acoustic = NoteTarget::Positioned(position)
                .sounding_pitch(&instrument)
                .ok_or(PlacementError::Unplayable)?;
            NoteTarget::Sounding { midi: acoustic, placement: None }
                .with_placement(&instrument, position)
        }
        Instrument::Flute(_) => {
            if event.hole.is_some()
                || event.action.is_some()
                || event.position.is_some()
                || modifiers.iter().any(|m| !matches!(m, Modifier::Vibrato { .. }))
            {
                return Err(PlacementError::Unplayable);
            }
            let midi = chart.event_pitch(event).ok_or(PlacementError::Unplayable)?;
            let position = PlayPosition::Fingering(FingeringAssignment {
                note: midi,
                alternate: event.fingering.unwrap_or(0),
            });
            NoteTarget::Sounding { midi: f32::from(midi), placement: None }
                .with_placement(&instrument, position)
        }
        Instrument::Piano(_) => {
            if event.hole.is_some()
                || event.action.is_some()
                || event.position.is_some()
                || event.fingering.is_some()
                || !modifiers.is_empty()
            {
                return Err(PlacementError::Unplayable);
            }
            let midi = chart.event_pitch(event).ok_or(PlacementError::Unplayable)?;
            NoteTarget::Sounding { midi: f32::from(midi), placement: None }
                .with_placement(&instrument, PlayPosition::Key(midi))
        }
    }
}

/// Replace pitch/placement fields while preserving expression and intensity.
/// No automatic fingering is guessed: an unplaced target must be resolved
/// explicitly before it can be saved to a position-based chart.
pub fn encode(
    instrument: &Instrument,
    target: NoteTarget,
    original: &NoteEvent,
) -> Result<NoteEvent, PlacementError> {
    use crate::instrument::guitar::StringTechnique;
    let position = target.position().ok_or(PlacementError::Unplayable)?;
    target.with_placement(instrument, position)?;
    let mut result = original.clone();
    result.fingering = None;
    let mut modifiers: Vec<_> = original
        .modifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|m| matches!(m, Modifier::Vibrato { .. } | Modifier::WahWah { .. }))
        .cloned()
        .collect();
    // Preserve authored bend intensity even when changing its position.
    let intensity = original
        .modifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find_map(|m| if let Modifier::Bend { intensity, .. } = m { *intensity } else { None });
    match position {
        PlayPosition::Hole(at) => {
            result.hole = Some(at.hole);
            result.action = Some(at.action);
            result.position = None;
            let mut natural = at;
            natural.technique = Technique::Natural;
            let midi = match at.technique {
                Technique::Natural | Technique::Bend(_) => {
                    instrument.pitch_of(&PlayPosition::Hole(natural))
                }
                _ => instrument.pitch_of(&position),
            }
            .ok_or(PlacementError::Unplayable)?;
            result.note = Some(crate::midi::midi_to_note(i32::from(midi)));
            match at.technique {
                Technique::Natural => {}
                Technique::Bend(depth) => {
                    modifiers.push(Modifier::Bend { semitones: -depth, intensity })
                }
                Technique::Overblow => modifiers.push(Modifier::Overblow),
                Technique::Overdraw => modifiers.push(Modifier::Overdraw),
                Technique::Slide => modifiers.push(Modifier::Slide),
            }
        }
        PlayPosition::Fret(at) => {
            result.hole = None;
            result.action = None;
            result.position = Some(FretPosition { string: at.string, fret: at.fret });
            result.note = Some(crate::midi::midi_to_note(
                target.sounding_pitch(instrument).ok_or(PlacementError::Unplayable)?.round() as i32,
            ));
            match at.technique {
                StringTechnique::Fretted => {}
                StringTechnique::Bend(depth) => {
                    modifiers.push(Modifier::Bend { semitones: depth, intensity })
                }
                StringTechnique::Harmonic => modifiers.push(Modifier::Harmonic),
            }
        }
        PlayPosition::Fingering(at) => {
            result.hole = None;
            result.action = None;
            result.position = None;
            result.fingering = (at.alternate > 0).then_some(at.alternate);
            // Breath vibrato is all a flute keeps of the original's
            // expression.
            modifiers.retain(|m| matches!(m, Modifier::Vibrato { .. }));
            result.note = Some(crate::midi::midi_to_note(i32::from(at.note)));
        }
        PlayPosition::Key(note) => {
            result.hole = None;
            result.action = None;
            result.position = None;
            // A piano key has no expression to keep.
            modifiers.clear();
            result.note = Some(crate::midi::midi_to_note(i32::from(note)));
        }
    }
    result.modifiers = (!modifiers.is_empty()).then_some(modifiers);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guitar_fixture_round_trips_fingering_techniques_and_sounding_pitches() {
        let chart: HarpChart = serde_json::from_str(include_str!(
            "../../harmonicon-song/tests/fixtures/guitar-highway.harpchart"
        ))
        .unwrap();
        let instrument = chart.instrument().unwrap();
        for event in chart.track.iter().flat_map(|item| &item.events) {
            let target = decode(&chart, event).unwrap();
            let restored = encode(&instrument, target, event).unwrap();
            assert_eq!(
                serde_json::to_value(event).unwrap(),
                serde_json::to_value(restored).unwrap()
            );
        }
    }
    fn flute_fixture() -> HarpChart {
        serde_json::from_str(include_str!(
            "../../harmonicon-song/tests/fixtures/flute-scale.harpchart"
        ))
        .unwrap()
    }

    #[test]
    fn flute_fixture_round_trips_its_fingerings_and_is_sound() {
        let chart = flute_fixture();
        assert!(crate::chart_check::instrument_problems(&chart).is_empty());
        let instrument = chart.instrument().unwrap();
        for event in chart.track.iter().flat_map(|item| &item.events) {
            let target = decode(&chart, event).unwrap();
            let restored = encode(&instrument, target, event).unwrap();
            assert_eq!(
                serde_json::to_value(event).unwrap(),
                serde_json::to_value(restored).unwrap()
            );
        }
        let d6 = decode(&chart, &chart.track[4].events[0]).unwrap();
        assert_eq!(
            d6.position(),
            Some(PlayPosition::Fingering(FingeringAssignment { note: 86, alternate: 1 }))
        );
    }

    fn piano_fixture() -> HarpChart {
        serde_json::from_str(include_str!(
            "../../harmonicon-song/tests/fixtures/piano-chords.harpchart"
        ))
        .unwrap()
    }

    /// A piano event is its note and nothing else; it round-trips as a
    /// key, and a hole, a technique or a key off the keyboard is refused.
    #[test]
    fn a_piano_event_is_a_key_and_round_trips() {
        let chart = piano_fixture();
        assert!(crate::chart_check::instrument_problems(&chart).is_empty());
        let instrument = chart.instrument().unwrap();
        for event in chart.track.iter().flat_map(|item| &item.events) {
            let target = decode(&chart, event).unwrap();
            let restored = encode(&instrument, target, event).unwrap();
            assert_eq!(
                serde_json::to_value(event).unwrap(),
                serde_json::to_value(restored).unwrap()
            );
        }
        assert_eq!(
            decode(&chart, &chart.track[0].events[0]).unwrap().position(),
            Some(PlayPosition::Key(60))
        );

        let mut chart = piano_fixture();
        chart.track[0].events[0].hole = Some(4);
        assert!(decode(&chart, &chart.track[0].events[0]).is_err());
        assert!(!crate::chart_check::instrument_problems(&chart).is_empty());
        let mut chart = piano_fixture();
        chart.instrument_spec = Some(crate::chart::ChartInstrument::Piano { keys: 61 });
        chart.track[4].events[1].note = Some("G1".into());
        assert!(crate::chart_check::instrument_problems(&chart)[0].contains("61-key"));
    }

    #[test]
    fn a_flute_event_out_of_range_or_with_a_reed_technique_is_refused() {
        let mut chart = flute_fixture();
        chart.instrument_spec =
            Some(crate::chart::ChartInstrument::Flute { foot: Default::default() });
        let problems = crate::chart_check::instrument_problems(&chart);
        assert_eq!(problems.len(), 1, "B3 needs a B foot: {problems:?}");
        assert!(problems[0].starts_with("track[0]"));
        let mut event = chart.track[1].events[0].clone();
        event.fingering = Some(4);
        assert_eq!(decode(&chart, &event), Err(PlacementError::Unplayable));
        event.fingering = None;
        event.modifiers = Some(vec![Modifier::Overblow]);
        assert_eq!(decode(&chart, &event), Err(PlacementError::Unplayable));
    }

    #[test]
    fn mismatched_pinned_guitar_pitch_is_refused() {
        let mut chart: HarpChart = serde_json::from_str(include_str!(
            "../../harmonicon-song/tests/fixtures/guitar-highway.harpchart"
        ))
        .unwrap();
        chart.track[0].events[0].note = Some("F2".into());
        assert_eq!(decode(&chart, &chart.track[0].events[0]), Err(PlacementError::PitchMismatch));
    }
    #[test]
    fn expression_intensity_survives_a_fingering_edit() {
        let chart: HarpChart = serde_json::from_str(include_str!(
            "../../harmonicon-song/tests/fixtures/guitar-highway.harpchart"
        ))
        .unwrap();
        let instrument = chart.instrument().unwrap();
        let mut event = chart.track[0].events[0].clone();
        event.modifiers =
            Some(vec![Modifier::Vibrato { oscillation_hz: 4.5, intensity: Some(0.8) }]);
        let target = decode(&chart, &event).unwrap();
        let restored = encode(&instrument, target, &event).unwrap();
        assert_eq!(serde_json::to_value(&event).unwrap(), serde_json::to_value(restored).unwrap());
        event.modifiers = Some(vec![Modifier::Overblow]);
        assert_eq!(decode(&chart, &event), Err(PlacementError::Unplayable));
    }
    #[test]
    fn fractional_string_bends_preserve_audio_pitch_and_chart_pitch() {
        let mut chart: HarpChart = serde_json::from_str(include_str!(
            "../../harmonicon-song/tests/fixtures/guitar-highway.harpchart"
        ))
        .unwrap();
        let event = &mut chart.track[5].events[0];
        event.modifiers = Some(vec![Modifier::Bend { semitones: 1.5, intensity: None }]);
        // B3 + fret 8 + 1.5 = G#4 plus a quarter tone, scored as A4.
        let event = event.clone();
        let target = decode(&chart, &event).unwrap();
        let instrument = chart.instrument().unwrap();
        assert_eq!(target.sounding_pitch(&instrument), Some(68.5));
        let saved = encode(&instrument, target, &event).unwrap();
        assert_eq!(saved.note, event.note);
        assert_eq!(
            serde_json::to_value(saved.modifiers).unwrap(),
            serde_json::to_value(event.modifiers).unwrap()
        );
    }
}
