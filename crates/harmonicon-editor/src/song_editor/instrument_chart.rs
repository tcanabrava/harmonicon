// SPDX-License-Identifier: MIT

//! Instrument-specific event encoding at the editor's document boundary.
use super::state::{EditorState, GridNote};
use harmonicon_core::chart::{Modifier, NoteEvent};
use harmonicon_core::instrument::Instrument;

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
    let target = harmonicon_core::note_event_codec::decode_for(instrument, &event).ok()?;
    let expr = event
        .modifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find_map(|m| match m {
            Modifier::Vibrato { oscillation_hz, .. } => {
                Some(super::state::Expr::Vibrato(oscillation_hz.max(0.5)))
            }
            Modifier::WahWah { oscillation_hz, .. } => {
                Some(super::state::Expr::Wah(oscillation_hz.max(0.5)))
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
    Some(GridNote::new(id, tick, len, expr, target).with_performance(
        harmonicon_core::note_performance::PerformanceInstructions::from_bow(event.bow),
    ))
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
        bow: note.performance.bowed(),
        hole: None,
        action: None,
        position: None,
        fingering: None,
        note: None,
        modifiers,
        ..Default::default()
    };
    let event = harmonicon_core::note_event_codec::encode(instrument, note.target, &original)
        .expect("only validated placements enter the editor document");
    // Preserve f32's shortest JSON representation. `to_value` widens it to
    // f64 first, exposing binary rounding in authored values such as 0.9.
    serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn harmonica_bend_is_encoded_once_and_legacy_expression_rates_survive() {
        use super::super::state::{Dir, Expr, HarpTechnique};
        let mut state = EditorState::default();
        let instrument = state.effective_instrument();
        let note = GridNote::new(
            0,
            0,
            12,
            Expr::Vibrato(6.0),
            GridNote::harmonica_target(3, Dir::Draw, HarpTechnique::Bend(2.0)),
        );
        let encoded = encode_note(&state, &note, &instrument);
        // B4 is the source reed; the modifier lowers it to A4 once.
        assert_eq!(encoded["note"], "B4");
        let decoded = decode_note(&encoded, &instrument, 0, 0, 12, &mut state).unwrap();
        assert_eq!(decoded.target, note.target);
        assert_eq!(decoded.expr, note.expr);
        let event: NoteEvent = serde_json::from_value(encoded).unwrap();
        assert_eq!(
            harmonicon_core::harp_remap::source_pitch(
                3,
                harmonicon_core::chart::Action::Draw,
                event.note.as_deref(),
                event.modifiers.as_deref().unwrap(),
                instrument.adapter().breath_model().unwrap()
            ),
            Some(69)
        );
        for (kind, expected) in [("vibrato", Expr::Vibrato(5.5)), ("wah-wah", Expr::Wah(4.0))] {
            let value =
                serde_json::json!({"hole": 1, "action": "blow", "modifiers": [{"type": kind}]});
            assert_eq!(
                decode_note(&value, &instrument, 1, 0, 12, &mut state).unwrap().expr,
                expected
            );
        }
        let zero_rate = serde_json::json!({"hole": 1, "action": "blow", "modifiers": [{"type": "vibrato", "oscillation_hz": 0.0}]});
        assert_eq!(
            decode_note(&zero_rate, &instrument, 1, 0, 12, &mut state).unwrap().expr,
            Expr::Vibrato(0.5)
        );
    }

    #[test]
    fn editor_uses_core_validation_for_foreign_metadata_and_mismatched_pitch() {
        let instrument = harmonicon_core::instrument::piano::Piano::default().into();
        let mut state = EditorState::default();
        for event in [
            serde_json::json!({"note": "C4", "hole": 1}),
            serde_json::json!({"note": "C4", "bow": {"direction": "up"}}),
            serde_json::json!({"note": "C0"}),
        ] {
            assert!(decode_note(&event, &instrument, 0, 0, 12, &mut state).is_none());
        }
    }

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

    /// Every key, chords included, survives a load and a save, and the
    /// window opens on the notes.
    #[test]
    fn a_piano_document_round_trips_its_keys_and_opens_on_them() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/piano-chords.harpchart"
        ))
        .unwrap();
        let mut state = EditorState::default();
        super::super::harpchart::load_harpchart(
            &source,
            &mut state,
            &mut super::super::state::Scroll::default(),
        );
        let events = source["track"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["events"].as_array().unwrap().len())
            .sum::<usize>();
        assert_eq!(state.notes.len(), events);
        assert_eq!(state.row_count(), 88);
        // This two-hand fixture spans more pitches than the scrollable
        // keyboard window can show at once. Keep every note in the document
        // and centre the view, rather than dropping offscreen notes.
        let low = state.notes.iter().map(GridNote::row).min().unwrap();
        let high = state.notes.iter().map(GridNote::row).max().unwrap();
        assert!(state.row_visible(((u16::from(low) + u16::from(high)) / 2) as u8));
        assert!(state.notes.iter().any(|n| state.row_visible(n.row())));
        let saved: serde_json::Value =
            serde_json::from_str(&super::super::harpchart::serialize_harpchart(&state)).unwrap();
        assert_eq!(saved["instrument"], serde_json::json!({ "type": "piano", "keys": 88 }));
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
