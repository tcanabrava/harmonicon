// SPDX-License-Identifier: MIT
//! The editor's single family registration point. Generic interaction code
//! consumes operations and layout instead of identifying instruments.
use super::{panel::NoteButtons, state::EditorState, transpose::TransposeOutcome, ui::ModButton};
use harmonicon_core::instrument::{Instrument, InstrumentModel};

type Place = fn(&mut EditorState, u8, usize, usize);
type Modify = fn(&mut EditorState, ModButton);
type Transpose = fn(&mut EditorState, i32) -> TransposeOutcome;

pub(super) enum RowStyle {
    Holes,
    Strings(fn(&Instrument) -> Vec<String>),
    Pitches { top: u8, dark_accidentals: bool },
}

pub(super) struct EditorAdapter {
    pub labels: RowStyle,
    pub place: Option<Place>,
    pub modify: Option<Modify>,
    pub transpose: Option<Transpose>,
    pub buttons: NoteButtons,
    pub rows: u8,
    pub window: Option<u8>,
    pub invalid_message: &'static str,
}

pub(super) fn for_instrument(instrument: &Instrument) -> EditorAdapter {
    match instrument {
        Instrument::Harmonica(harp) => EditorAdapter {
            labels: RowStyle::Holes,
            place: None,
            modify: None,
            transpose: None,
            buttons: if matches!(harp, harmonicon_core::harmonica::Harmonica::Diatonic { .. }) {
                NoteButtons::Diatonic
            } else {
                NoteButtons::Chromatic
            },
            rows: harp.hole_count(),
            window: None,
            invalid_message: "editor-guitar-invalid",
        },
        Instrument::Fretted(setup) => EditorAdapter {
            labels: RowStyle::Strings(|instrument| {
                let Instrument::Fretted(setup) = instrument else {
                    unreachable!("fretted editor adapter")
                };
                setup.string_labels()
            }),
            place: Some(super::guitar_note::place),
            modify: Some(super::guitar_note::apply_modifier),
            transpose: None,
            buttons: NoteButtons::Guitar,
            rows: setup.string_count(),
            window: None,
            invalid_message: "editor-guitar-invalid",
        },
        Instrument::Bowed(setup) => EditorAdapter {
            labels: RowStyle::Strings(|instrument| {
                let Instrument::Bowed(setup) = instrument else {
                    unreachable!("bowed editor adapter")
                };
                setup
                    .strings
                    .iter()
                    .map(|&midi| harmonicon_core::midi::midi_to_note(i32::from(midi)))
                    .collect()
            }),
            place: Some(super::bowed_note::place),
            modify: Some(super::bowed_note::apply_modifier),
            transpose: Some(super::bowed_note::transpose),
            buttons: NoteButtons::Bowed,
            rows: setup.strings.len() as u8,
            window: None,
            invalid_message: "bowed-invalid",
        },
        Instrument::Flute(_) | Instrument::Clarinet(_) => EditorAdapter {
            labels: RowStyle::Pitches {
                top: super::note_model::FLUTE_TOP_NOTE,
                dark_accidentals: true,
            },
            place: Some(super::flute_note::place),
            modify: Some(super::flute_note::apply_modifier),
            transpose: Some(super::flute_note::transpose),
            buttons: NoteButtons::Flute,
            rows: instrument
                .range()
                .map_or(0, |(low, _)| super::note_model::FLUTE_TOP_NOTE + 1 - low),
            window: None,
            invalid_message: "editor-wind-invalid",
        },
        Instrument::Piano(setup) => EditorAdapter {
            labels: RowStyle::Pitches {
                top: super::note_model::PIANO_TOP_NOTE,
                dark_accidentals: false,
            },
            place: Some(super::piano_note::place),
            modify: Some(|_, _| {}),
            transpose: Some(super::piano_note::transpose),
            buttons: NoteButtons::Piano,
            rows: super::note_model::PIANO_TOP_NOTE + 1 - setup.lowest(),
            window: Some(super::piano_note::WINDOW_ROWS),
            invalid_message: "editor-piano-invalid",
        },
    }
}

pub(super) fn row_labels(
    instrument: &Instrument,
    rows: u8,
    offset: u8,
) -> Vec<super::meta_form::RowLabel> {
    use super::meta_form::RowLabel;
    use bevy::prelude::Color;
    match for_instrument(instrument).labels {
        RowStyle::Holes => {
            (1..=rows).map(|row| RowLabel { text: format!("{row:02}"), swatch: None }).collect()
        }
        RowStyle::Strings(labels) => labels(instrument)
            .into_iter()
            .enumerate()
            .map(|(index, text)| RowLabel {
                text,
                swatch: Some(harmonicon_gameplay::gameplay::guitar_visuals::string_color(
                    index as u8 + 1,
                )),
            })
            .collect(),
        RowStyle::Pitches { top, dark_accidentals } => (1..=rows)
            .map(|row| {
                let note = top + 1 - row - offset;
                RowLabel {
                    text: harmonicon_core::midi::midi_to_note(i32::from(note)),
                    swatch: (dark_accidentals && matches!(note % 12, 1 | 3 | 6 | 8 | 10))
                        .then_some(Color::srgb(0.12, 0.12, 0.16)),
                }
            })
            .collect(),
    }
}
