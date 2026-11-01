// SPDX-License-Identifier: MIT

//! Instrument-specific event encoding at the editor's document boundary.
use super::state::{EditorState, GridNote};
use harmonicon_core::chart::{Modifier, NoteEvent};
use harmonicon_core::instrument::InstrumentModel;
use harmonicon_core::instrument::{
    Instrument, PlayPosition, flute::FingeringAssignment, guitar::FretAssignment,
};
use harmonicon_core::note_target::NoteTarget;

pub(super) fn configured_instrument(value: &serde_json::Value) -> Option<Instrument> {
    value.get("instrument")?;
    // The chart parser is the single authority for tuning/configuration.
    let chart: harmonicon_core::chart::HarpChart = serde_json::from_value(value.clone()).ok()?;
    chart.instrument()
}

pub(super) fn decode_note(
    value: &serde_json::Value,
    instrument: &Instrument,
    id: u32,
    tick: usize,
    len: usize,
    state: &mut EditorState,
) -> Option<GridNote> {
    let event: NoteEvent = serde_json::from_value(value.clone()).ok()?;
    let midi = harmonicon_core::midi::note_to_midi(event.note.as_deref()?)? as f32;
    let position = match instrument {
        Instrument::Flute(_) => PlayPosition::Fingering(FingeringAssignment {
            note: midi as u8,
            alternate: event.fingering.unwrap_or(0),
        }),
        _ => {
            let at = event.position?;
            PlayPosition::Fret(FretAssignment {
                string: at.string,
                fret: at.fret,
                technique: harmonicon_core::chart_check::string_technique(
                    event.modifiers.as_deref().unwrap_or(&[]),
                ),
            })
        }
    };
    if instrument.pitch_of(&position)? != midi as u8 {
        return None;
    }
    let acoustic = NoteTarget::Positioned(position).sounding_pitch(instrument)?;
    let target = NoteTarget::Sounding { midi: acoustic, placement: None }
        .with_placement(instrument, position)
        .ok()?;
    let expr = event
        .modifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find_map(|m| match m {
            Modifier::Vibrato { oscillation_hz, .. } => {
                Some(super::state::Expr::Vibrato(*oscillation_hz))
            }
            Modifier::WahWah { oscillation_hz, .. } => {
                Some(super::state::Expr::Wah(*oscillation_hz))
            }
            _ => None,
        })
        .unwrap_or(super::state::Expr::None);
    if let Some(intensity) =
        event.modifiers.as_deref().unwrap_or(&[]).iter().find_map(|m| match m {
            Modifier::Vibrato { intensity, .. } | Modifier::WahWah { intensity, .. } => *intensity,
            _ => None,
        })
        && intensity != 0.5
    {
        state.expression_intensities.insert(id, intensity.to_string());
    }
    Some(GridNote { id, tick, len, expr, target })
}

pub(super) fn encode_note(
    state: &EditorState,
    note: &GridNote,
    instrument: &Instrument,
) -> serde_json::Value {
    let intensity =
        state.expression_intensities.get(&note.id).and_then(|v| v.parse().ok()).unwrap_or(0.5);
    let modifiers = match note.expr {
        super::state::Expr::Vibrato(oscillation_hz) => {
            Some(vec![Modifier::Vibrato { oscillation_hz, intensity: Some(intensity) }])
        }
        super::state::Expr::Wah(oscillation_hz) => {
            Some(vec![Modifier::WahWah { oscillation_hz, intensity: Some(intensity) }])
        }
        super::state::Expr::None => None,
    };
    let original = NoteEvent {
        hole: None,
        action: None,
        position: None,
        fingering: None,
        note: None,
        modifiers,
    };
    serde_json::to_value(
        harmonicon_core::note_event_codec::encode(instrument, note.target, &original)
            .expect("only validated placements enter the editor document"),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guitar_editor_document_round_trips_without_harmonica_fields() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/guitar-highway.harpchart"
        ))
        .unwrap();
        let mut state = EditorState::default();
        super::super::harpchart::load_harpchart(
            &source,
            &mut state,
            &mut super::super::state::Scroll::default(),
        );
        assert_eq!(state.notes.len(), 12);
        let saved = super::super::harpchart::serialize_harpchart(&state);
        let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert!(saved.get("harmonica").is_none());
        let before: harmonicon_core::chart::HarpChart =
            serde_json::from_value(source.clone()).unwrap();
        let after: harmonicon_core::chart::HarpChart =
            serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(before.instrument(), after.instrument());
        let events = |value: &serde_json::Value| -> Vec<serde_json::Value> {
            value["track"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|item| item["events"].as_array().unwrap().iter())
                .map(|event| {
                    serde_json::to_value(
                        serde_json::from_value::<NoteEvent>(event.clone()).unwrap(),
                    )
                    .unwrap()
                })
                .collect()
        };
        assert_eq!(events(&source), events(&saved));
        harmonicon_song::song::validate_and_migrate_chart(&mut saved.clone()).unwrap();
    }

    /// The B foot, each note's pitch row, the alternate fingering and the
    /// vibrato all survive a load and a save.
    #[test]
    fn a_flute_document_round_trips_its_foot_fingerings_and_vibrato() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/flute-scale.harpchart"
        ))
        .unwrap();
        let mut state = EditorState::default();
        super::super::harpchart::load_harpchart(
            &source,
            &mut state,
            &mut super::super::state::Scroll::default(),
        );
        assert_eq!(state.notes.len(), 7);
        assert_eq!(state.row_count(), 38, "C7 down to a B foot's B3");
        assert_eq!(state.notes[0].row(), 38);
        let saved: serde_json::Value =
            serde_json::from_str(&super::super::harpchart::serialize_harpchart(&state)).unwrap();
        assert!(saved.get("harmonica").is_none());
        assert_eq!(saved["instrument"], source["instrument"]);
        let notes = |value: &serde_json::Value| -> Vec<(String, Option<u64>, bool)> {
            value["track"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|item| item["events"].as_array().unwrap().iter())
                .map(|e| {
                    (
                        e["note"].as_str().unwrap().to_string(),
                        e["fingering"].as_u64(),
                        e["modifiers"].is_array(),
                    )
                })
                .collect()
        };
        assert_eq!(notes(&source), notes(&saved));
        harmonicon_song::song::validate_and_migrate_chart(&mut saved.clone()).unwrap();
    }
}
