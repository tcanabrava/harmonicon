// SPDX-License-Identifier: MIT

//! Adapter for piano notes on the shared grid, the counterpart of
//! `flute_note.rs`. A row is a key, C8 at the top
//! ([`super::note_model::PIANO_TOP_NOTE`]), and a note is its key and
//! nothing more. Any keys can sound together, so a note is refused only
//! on top of the same key still sounding: `guitar_note::fits`, which asks
//! the instrument, checks that for placing, dragging and pasting alike.
//!
//! A keyboard has far more rows than a screen can show at a readable
//! height, so the grid shows a window of [`WINDOW_ROWS`] keys onto them
//! (`EditorState::row_offset`) and scrolls it ([`scroll_keys`]); everything
//! that draws asks `display_rows`/`display_row`, so the window is a view,
//! never a change to any note.
use super::note_model::PIANO_TOP_NOTE;
use super::state::{DEFAULT_INTENSITY, EditorState, GridNote};
use harmonicon_core::instrument::{Instrument, InstrumentModel, PlayPosition};
use harmonicon_core::note_target::NoteTarget;

/// How many keys the grid shows at once.
pub(super) const WINDOW_ROWS: u8 = 24;

pub(super) fn is_piano(state: &EditorState) -> bool {
    matches!(state.effective_instrument(), Instrument::Piano(_))
}

/// The key on grid row `row` (from 1, C8 at the top).
pub(super) fn row_note(row: u8) -> u8 {
    PIANO_TOP_NOTE + 1 - row
}

fn key_of(note: &GridNote) -> Option<u8> {
    match note.target.position() {
        Some(PlayPosition::Key(key)) => Some(key),
        _ => None,
    }
}

/// Places a note on `row`'s key at `tick` with the armed expression, and
/// selects it. Refused, and reported, on a key the keyboard lacks or one
/// already sounding then.
pub(super) fn place(state: &mut EditorState, row: u8, tick: usize, len: usize) {
    let instrument = state.effective_instrument();
    let note = GridNote {
        id: state.next_id,
        tick,
        len,
        expr: super::state::Expr::None,
        target: NoteTarget::Positioned(PlayPosition::Key(row_note(row))),
    };
    if !super::guitar_note::fits(&instrument, &state.notes, &note, &[]) {
        state.unplayable_notice = true;
        return;
    }
    state.notes.push(note);
    state.next_id += 1;
    state.select_only(note.id);
    if note.expr != super::state::Expr::None && state.sticky_intensity != DEFAULT_INTENSITY {
        let depth = state.sticky_intensity.clone();
        state.expression_intensities.insert(note.id, depth);
    }
}

/// Moves the selection — everything, with nothing selected — `semitones`
/// up or down. A note whose new key is off the keyboard stays put and is
/// counted.
pub(super) fn transpose(
    state: &mut EditorState,
    semitones: i32,
) -> super::transpose::TransposeOutcome {
    let instrument = state.effective_instrument();
    let everything = state.selected.is_empty();
    let ids = state.selected.clone();
    let (mut moved, mut kept) = (0, 0);
    for n in state.notes.iter_mut().filter(|n| everything || ids.contains(&n.id)) {
        let Some(key) = key_of(n) else { continue };
        let target = u8::try_from(i32::from(key) + semitones)
            .ok()
            .filter(|&k| instrument.pitch_of(&PlayPosition::Key(k)).is_some());
        match target {
            Some(key) => {
                n.target = NoteTarget::Positioned(PlayPosition::Key(key));
                moved += 1;
            }
            None => kept += 1,
        }
    }
    super::transpose::TransposeOutcome { semitones, moved, kept, ..Default::default() }
}

/// The furthest the window can scroll down: the last [`WINDOW_ROWS`] rows.
fn max_offset(state: &EditorState) -> u8 {
    state.row_count().saturating_sub(WINDOW_ROWS)
}

/// Scrolls the window `delta` keys down (up for negative), within the
/// keyboard.
pub(super) fn scroll_keys(state: &mut EditorState, delta: i32) {
    if !is_piano(state) {
        return;
    }
    let offset = (i32::from(state.row_offset) + delta).clamp(0, i32::from(max_offset(state)));
    let offset = offset as u8;
    if state.row_offset != offset {
        state.row_offset = offset;
    }
}

/// Scrolls the window so `row` is in it, a few keys in from the edge when
/// it has to move.
pub(super) fn reveal_row(state: &mut EditorState, row: u8) {
    const MARGIN: i32 = 3;
    let first = i32::from(state.row_offset) + 1;
    let last = first + i32::from(WINDOW_ROWS) - 1;
    let row = i32::from(row);
    if row < first {
        scroll_keys(state, row - first - MARGIN);
    } else if row > last {
        scroll_keys(state, row - last + MARGIN);
    }
}

/// Instrument changes discard the previous instrument's pitch-window offset.
pub(super) fn reset_window(state: &mut EditorState) {
    state.row_offset = 0;
    if is_piano(state) {
        centre_on_notes(state);
    }
}

/// Puts the notes in the middle of the window, or middle C when there are
/// none — where a newly loaded piano chart opens.
pub(super) fn centre_on_notes(state: &mut EditorState) {
    let rows: Vec<u8> = state.notes.iter().map(GridNote::row).collect();
    let middle = match (rows.iter().min(), rows.iter().max()) {
        (Some(&lo), Some(&hi)) => (u16::from(lo) + u16::from(hi)) / 2,
        _ => u16::from(PIANO_TOP_NOTE + 1 - 60),
    };
    state.row_offset = 0;
    scroll_keys(state, i32::from(middle) - i32::from(WINDOW_ROWS / 2));
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::piano::Piano;

    fn piano_state() -> EditorState {
        let mut state = EditorState {
            instrument: super::super::state::InstrumentSelection::Configured {
                instrument: Piano::grand().into(),
                harmonica_origin: None,
            },
            ..Default::default()
        };
        centre_on_notes(&mut state);
        state
    }

    #[test]
    fn every_keyboard_key_can_be_drawn_after_placement() {
        for row in 1..=88 {
            let mut state = piano_state();
            place(&mut state, row, 0, 12);
            let note = state.notes.first().expect("key placed");
            let (_, label) = super::super::grid::note_face(note);
            assert_eq!(label, harmonicon_core::midi::midi_to_note(i32::from(row_note(row))));
        }
    }

    /// Row 1 is C8, so middle C is row 49; two keys at once are a chord,
    /// the same key twice is refused.
    #[test]
    fn a_click_places_its_rows_key_and_chords_are_free() {
        let mut state = piano_state();
        assert_eq!(state.row_count(), 88);
        place(&mut state, 49, 0, 12);
        place(&mut state, 45, 0, 12);
        assert_eq!(state.notes.iter().filter_map(key_of).collect::<Vec<_>>(), [60, 64]);
        place(&mut state, 49, 6, 12);
        assert_eq!(state.notes.len(), 2, "C4 is already sounding");
        assert!(state.unplayable_notice);
    }

    /// The window shows 24 keys, opens around middle C, scrolls within the
    /// keyboard, and draws a note at its place in the window.
    #[test]
    fn the_grid_is_a_scrolling_window_onto_the_keyboard() {
        let mut state = piano_state();
        assert_eq!(state.display_rows(), WINDOW_ROWS);
        let window = state.row_offset + 1..=state.row_offset + WINDOW_ROWS;
        assert!(window.contains(&49), "middle C on screen: {window:?}");
        place(&mut state, 49, 0, 12);
        assert_eq!(state.display_row(&state.notes[0]), usize::from(49 - 1 - state.row_offset));
        scroll_keys(&mut state, -1000);
        assert_eq!(state.row_offset, 0);
        scroll_keys(&mut state, 1000);
        assert_eq!(state.row_offset, 88 - WINDOW_ROWS);
        reveal_row(&mut state, 10);
        assert!(state.row_offset < 10 && 10 <= state.row_offset + WINDOW_ROWS);
    }

    #[test]
    fn transposing_moves_keys_and_keeps_those_that_would_fall_off() {
        let mut state = piano_state();
        place(&mut state, 49, 0, 12);
        place(&mut state, 1, 24, 12);
        state.selected.clear();
        let outcome = transpose(&mut state, 1);
        assert_eq!((outcome.moved, outcome.kept), (1, 1), "nothing above C8");
        assert_eq!(key_of(&state.notes[0]), Some(61));
    }
}
