// SPDX-License-Identifier: MIT

//! Conversion between chart positions and layout-independent note intent.
//! Expression modifiers are retained on the chart event, outside the target.
use crate::chart::{FretPosition, HarpChart, Modifier, NoteEvent};
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
            NoteTarget::Sounding { midi: f32::from(midi), placement: None }
                .with_placement(&instrument, position)
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
}
