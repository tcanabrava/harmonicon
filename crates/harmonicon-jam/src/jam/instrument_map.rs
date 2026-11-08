// SPDX-License-Identifier: MIT

//! The instrument a jam is played on, drawn with the notes that fit the
//! scale marked — the harmonica's hole map (`hole_map`), for every other
//! instrument. A fretted instrument is its strings and first twelve frets,
//! so a scale's familiar boxes appear without anyone authoring them; a
//! piano is its keyboard; anything else is its range as note cells, an
//! octave to a row.
//!
//! Colour follows the one rule the hole map and the improv tally use
//! ([`classify_note_fit`]): a note's label is bright when it is in the
//! scale, and a note being played is lit by its fit to the bar sounding —
//! chord tone, in scale, outside.

use std::collections::HashSet;

use bevy::prelude::*;
use harmonicon_core::instrument::guitar::{FretAssignment, StringTechnique};
use harmonicon_core::instrument::{Instrument, InstrumentModel};
use harmonicon_core::midi::midi_to_note;
use harmonicon_gameplay::gameplay::{ActivePitches, CurrentBar};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::piano_keys::{
    KeyboardAxis, KeyboardSpec, PianoKey, key_colour, spawn_piano_keys,
};

use super::hole_map::{
    HOLE_DEFAULT, JamGuide, LABEL_IN_SCALE, LABEL_OUT_SCALE, PLAY_CHORD_TONE, PLAY_IN_SCALE,
    PLAY_OUT_SCALE, note_class,
};
use super::improv::{NoteFit, classify_note_fit};

/// Frets drawn above the open string: an octave, where every scale shape
/// repeats.
const MAP_FRETS: u8 = 12;

/// The keys a jam's keyboard spans, C3 to C6: three octaves around middle C,
/// where improvising sits, rather than 88 keys too narrow to read.
const KEYBOARD_SPAN: (u8, u8) = (48, 84);

/// How strongly an in-scale key is tinted, when nothing is playing it.
const KEY_IN_SCALE_MIX: f32 = 0.35;

/// One drawn note: where it sits in the grid and what it sounds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MapCell {
    pub row: u8,
    pub column: u8,
    pub midi: u8,
    pub label: String,
}

/// What a jam draws for an instrument.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InstrumentMap {
    /// A keyboard from `low` to `high`.
    Keyboard { low: u8, high: u8 },
    /// A `rows` × `columns` grid of note cells.
    Cells { rows: u8, columns: u8, cells: Vec<MapCell> },
}

/// The map for `instrument`, or `None` for a harmonica, which has its own
/// (`hole_map`), or an instrument with no range.
pub(crate) fn instrument_map(instrument: &Instrument) -> Option<InstrumentMap> {
    match instrument {
        Instrument::Harmonica(_) => None,
        Instrument::Piano(piano) => {
            let low = KEYBOARD_SPAN.0.max(piano.lowest());
            let high = KEYBOARD_SPAN.1.min(piano.highest());
            (low <= high).then_some(InstrumentMap::Keyboard { low, high })
        }
        Instrument::Fretted(guitar) => {
            let frets = MAP_FRETS.min(guitar.top_fret());
            let cells = (1..=guitar.string_count())
                .flat_map(|string| (0..=frets).map(move |fret| (string, fret)))
                .filter_map(|(string, fret)| {
                    let at = FretAssignment { string, fret, technique: StringTechnique::Fretted };
                    let midi = guitar.pitch_of(&at)?;
                    Some(MapCell {
                        row: string - 1,
                        column: fret,
                        midi,
                        label: note_class(&midi_to_note(i32::from(midi))).to_string(),
                    })
                })
                .collect();
            Some(InstrumentMap::Cells { rows: guitar.string_count(), columns: frets + 1, cells })
        }
        other => {
            let (low, high) = other.range()?;
            let first_octave = low / 12;
            let cells = (low..=high)
                .map(|midi| MapCell {
                    // Highest octave on top, as on a staff.
                    row: high / 12 - midi / 12,
                    column: midi % 12,
                    midi,
                    label: midi_to_note(i32::from(midi)),
                })
                .collect();
            Some(InstrumentMap::Cells { rows: high / 12 - first_octave + 1, columns: 12, cells })
        }
    }
}

/// A cell's background and label colours: lit by its fit while `sounding`,
/// otherwise dark, with the label bright when the note is in the scale.
pub(crate) fn cell_colours(
    midi: u8,
    sounding: bool,
    chord_tones: &HashSet<String>,
    scale: &HashSet<String>,
) -> (Color, Color) {
    let note = midi_to_note(i32::from(midi));
    let class = note_class(&note);
    let label = if scale.contains(class) { LABEL_IN_SCALE } else { LABEL_OUT_SCALE };
    if !sounding {
        return (HOLE_DEFAULT, label);
    }
    let lit = match classify_note_fit(class, chord_tones, scale) {
        NoteFit::ChordTone => PLAY_CHORD_TONE,
        NoteFit::InScale => PLAY_IN_SCALE,
        NoteFit::OutOfScale => PLAY_OUT_SCALE,
    };
    (lit, Color::BLACK)
}

/// A keyboard key's colour: lit by its fit while `sounding`, tinted green
/// when in the scale, otherwise its own.
pub(crate) fn key_fill(
    midi: u8,
    sounding: bool,
    chord_tones: &HashSet<String>,
    scale: &HashSet<String>,
) -> Color {
    if sounding {
        return cell_colours(midi, true, chord_tones, scale).0;
    }
    let class = note_class(&midi_to_note(i32::from(midi))).to_string();
    let own = key_colour(midi);
    if scale.contains(&class) { own.mix(&PLAY_IN_SCALE, KEY_IN_SCALE_MIX) } else { own }
}

/// One drawn note cell.
#[derive(Component)]
pub struct JamMapCell {
    midi: u8,
}

/// The keyboard a jam draws, so its keys can be told from any other.
#[derive(Component)]
pub struct JamMapKeyboard;

/// Spawns the map for `instrument` under `parent`, headed by its name. Does
/// nothing for a harmonica (see [`instrument_map`]).
pub(crate) fn spawn_instrument_map(
    parent: &mut ChildSpawnerCommands,
    instrument: &Instrument,
    loc: &Localization,
) {
    let Some(map) = instrument_map(instrument) else {
        return;
    };
    parent.spawn((
        Text::new(loc.msg(&format!("song-instrument-{}", instrument.kind().slug())).to_string()),
        TextFont { font_size: FontSize::Px(20.0), ..default() },
        TextColor(Color::WHITE),
        Node { margin: UiRect::vertical(Val::Px(12.0)), ..default() },
    ));
    match map {
        InstrumentMap::Keyboard { low, high } => {
            parent
                .spawn((
                    Node {
                        width: Val::Percent(94.0),
                        height: Val::Px(160.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    JamMapKeyboard,
                ))
                .with_children(|board| {
                    spawn_piano_keys(
                        board,
                        KeyboardSpec { low, high, axis: KeyboardAxis::Horizontal, label_cs: true },
                    );
                });
        }
        InstrumentMap::Cells { rows, columns, cells } => {
            parent
                .spawn(Node {
                    width: Val::Percent(94.0),
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::flex(u16::from(columns), 1.0),
                    grid_template_rows: RepeatedGridTrack::auto(u16::from(rows)),
                    row_gap: Val::Px(2.0),
                    column_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    for cell in cells {
                        grid.spawn((
                            Node {
                                grid_row: GridPlacement::start(i16::from(cell.row) + 1),
                                grid_column: GridPlacement::start(i16::from(cell.column) + 1),
                                justify_content: JustifyContent::Center,
                                padding: UiRect::vertical(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(HOLE_DEFAULT),
                            JamMapCell { midi: cell.midi },
                        ))
                        .with_children(|label| {
                            label.spawn((
                                Text::new(cell.label),
                                TextFont { font_size: FontSize::Px(13.0), ..default() },
                                TextColor(LABEL_OUT_SCALE),
                            ));
                        });
                    }
                });
        }
    }
}

/// Paints the map each frame from what the mic hears, writing only what
/// changed.
pub fn update_instrument_map(
    active: Res<ActivePitches>,
    guide: Option<Res<JamGuide>>,
    current: Res<CurrentBar>,
    mut cells: Query<(&JamMapCell, &mut BackgroundColor, &Children), Without<PianoKey>>,
    mut labels: Query<&mut TextColor>,
    keyboards: Query<&Children, With<JamMapKeyboard>>,
    mut keys: Query<(&PianoKey, &mut BackgroundColor), Without<JamMapCell>>,
) {
    let Some(guide) = guide else {
        return;
    };
    let sounding: HashSet<u8> = active.0.iter().map(|p| p.midi).collect();
    let chord_tones = &guide.chord_tones_by_bar[current.0];
    let scale = &guide.scale_classes;
    for (cell, mut fill, children) in &mut cells {
        let (background, label) =
            cell_colours(cell.midi, sounding.contains(&cell.midi), chord_tones, scale);
        fill.set_if_neq(BackgroundColor(background));
        for &child in children {
            if let Ok(mut text) = labels.get_mut(child) {
                text.set_if_neq(TextColor(label));
            }
        }
    }
    let ours: HashSet<Entity> = keyboards.iter().flatten().copied().collect();
    for (key, mut fill) in keys.iter_mut().filter(|(key, _)| ours.contains(&key.keyboard)) {
        fill.set_if_neq(BackgroundColor(key_fill(
            key.midi,
            sounding.contains(&key.midi),
            chord_tones,
            scale,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::flute::Flute;
    use harmonicon_core::instrument::guitar::{Guitar, GuitarBody};
    use harmonicon_core::instrument::piano::Piano;

    fn classes(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn a_harmonica_keeps_its_own_hole_map() {
        let harp = harmonicon_core::harmonica::richter_harp("C");
        assert_eq!(instrument_map(&harp.into()), None);
    }

    #[test]
    fn a_guitar_is_every_string_to_the_twelfth_fret() {
        let Some(InstrumentMap::Cells { rows, columns, cells }) =
            instrument_map(&Guitar::standard(GuitarBody::default()).into())
        else {
            panic!("a guitar draws cells");
        };
        assert_eq!((rows, columns), (6, 13));
        assert_eq!(cells.len(), 6 * 13);
        // The twelfth fret is the open string an octave up, which is what
        // makes a scale's shapes repeat there.
        for string in 0..rows {
            let at = |fret| cells.iter().find(|c| c.row == string && c.column == fret).unwrap();
            assert_eq!(at(12).midi, at(0).midi + 12);
            assert_eq!(at(12).label, at(0).label);
        }
    }

    #[test]
    fn a_piano_is_three_octaves_around_middle_c() {
        assert_eq!(
            instrument_map(&Piano::default().into()),
            Some(InstrumentMap::Keyboard { low: 48, high: 84 })
        );
    }

    #[test]
    fn a_flute_is_its_range_an_octave_to_a_row_highest_on_top() {
        let flute = Flute::default();
        let (low, high) = flute.range().unwrap();
        let Some(InstrumentMap::Cells { rows, columns, cells }) =
            instrument_map(&flute.into())
        else {
            panic!("a flute draws cells");
        };
        assert_eq!(columns, 12);
        assert_eq!(cells.len(), usize::from(high - low + 1));
        assert!(cells.iter().all(|c| c.row < rows && c.column == c.midi % 12));
        let top = cells.iter().find(|c| c.midi == high).unwrap();
        assert_eq!(top.row, 0);
    }

    #[test]
    fn an_idle_note_is_dark_with_its_label_showing_whether_it_fits() {
        let scale = classes(&["C", "Eb", "F", "F#", "G", "Bb"]);
        let chord = classes(&["C", "E", "G", "Bb"]);
        assert_eq!(cell_colours(60, false, &chord, &scale), (HOLE_DEFAULT, LABEL_IN_SCALE));
        assert_eq!(cell_colours(62, false, &chord, &scale), (HOLE_DEFAULT, LABEL_OUT_SCALE));
    }

    #[test]
    fn a_played_note_is_lit_by_its_fit_to_the_bar() {
        let scale = classes(&["C", "Eb", "F", "F#", "G", "Bb"]);
        let chord = classes(&["C", "E", "G", "Bb"]);
        assert_eq!(cell_colours(67, true, &chord, &scale).0, PLAY_CHORD_TONE); // G
        assert_eq!(cell_colours(65, true, &chord, &scale).0, PLAY_IN_SCALE); // F
        assert_eq!(cell_colours(62, true, &chord, &scale).0, PLAY_OUT_SCALE); // D
    }

    #[test]
    fn an_in_scale_key_is_tinted_and_an_outside_one_keeps_its_colour() {
        let scale = classes(&["C", "Eb", "F", "F#", "G", "Bb"]);
        let chord = classes(&["C", "E", "G", "Bb"]);
        assert_ne!(key_fill(60, false, &chord, &scale), key_colour(60));
        assert_eq!(key_fill(62, false, &chord, &scale), key_colour(62));
        assert_eq!(key_fill(67, true, &chord, &scale), PLAY_CHORD_TONE);
    }
}
