// SPDX-License-Identifier: MIT

//! Adapter for flute notes on the shared grid, the counterpart of
//! `harmonica_note.rs` and `guitar_note.rs`. A row is a pitch, C7 at the
//! top ([`super::note_model::FLUTE_TOP_NOTE`]), and a note plays its
//! standard fingering unless the Fingering button picks one of its
//! alternates. The flute has one voice: a note placed while another sounds
//! ends that one there, and one placed on another's onset is refused,
//! since the two could only be a chord. Drags, resizes and pastes are
//! checked by the same rule through `guitar_note::fits`, which asks the
//! instrument what can sound together.
use super::interaction::next_expr;
use super::note_model::FLUTE_TOP_NOTE;
use super::state::{DEFAULT_INTENSITY, EditorState, GridNote};
use super::ui::ModButton;
use harmonicon_core::instrument::PlayPosition;
#[cfg(test)]
use harmonicon_core::instrument::flute::Flute;
use harmonicon_core::instrument::wind::FingeringAssignment;
use harmonicon_core::note_target::NoteTarget;

pub(super) fn is_wind(state: &EditorState) -> bool {
    state.effective_instrument().fingering_chart().is_some()
}

fn fingering_of(note: &GridNote) -> Option<FingeringAssignment> {
    match note.target.position() {
        Some(PlayPosition::Fingering(at)) => Some(at),
        _ => None,
    }
}

fn fingered(note: GridNote, at: FingeringAssignment) -> GridNote {
    GridNote { target: NoteTarget::Positioned(PlayPosition::Fingering(at)), ..note }
}

/// How many fingerings `note` has, standard included.
pub(super) fn fingering_count(
    flute: &dyn harmonicon_core::instrument::wind::FingeringChart,
    note: &GridNote,
) -> usize {
    fingering_of(note)
        .map_or(0, |at| harmonicon_core::instrument::wind::positions_for(flute, at.note).len())
}

/// Places a note on `row` at `tick` on its standard fingering, with the
/// armed expression, and selects it. It is cut short at the next onset,
/// and a note still sounding at `tick` ends there: one voice. Refused, and
/// reported, on another note's onset.
pub(super) fn place(state: &mut EditorState, row: u8, tick: usize, len: usize) {
    let instrument = state.effective_instrument();
    let Some(flute) = instrument.fingering_chart() else { return };
    let Some(pitch) = (FLUTE_TOP_NOTE + 1).checked_sub(row).filter(|&pitch| {
        !harmonicon_core::instrument::wind::positions_for(flute, pitch).is_empty()
    }) else {
        state.unplayable_notice = true;
        return;
    };
    if state.notes.iter().any(|n| n.tick == tick) {
        state.unplayable_notice = true;
        return;
    }
    let next = state.notes.iter().filter(|n| n.tick > tick).map(|n| n.tick).min();
    let len = next.map_or(len, |start| len.min(start - tick)).max(1);
    for n in state.notes.iter_mut().filter(|n| n.tick < tick && tick < n.tick + n.len) {
        n.len = tick - n.tick;
    }
    let note = GridNote {
        id: state.next_id,
        tick,
        len,
        expr: state.sticky_expr,
        target: NoteTarget::Positioned(PlayPosition::Fingering(FingeringAssignment {
            note: pitch,
            alternate: 0,
        })),
    };
    state.notes.push(note);
    state.next_id += 1;
    state.select_only(note.id);
    if note.expr != super::state::Expr::None && state.sticky_intensity != DEFAULT_INTENSITY {
        let depth = state.sticky_intensity.clone();
        state.expression_intensities.insert(note.id, depth);
    }
}

/// The note column on a flute: Fingering steps each selected note to its
/// next fingering (wrapping to the standard one), Vibrato cycles breath
/// vibrato like the harmonica's. With nothing selected Vibrato arms the
/// next note's.
pub(super) fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    let instrument = state.effective_instrument();
    let Some(flute) = instrument.fingering_chart() else { return };
    let Some(anchor) = state.selected_note().copied() else {
        if kind == ModButton::Vibrato {
            state.sticky_expr = next_expr(kind, state.sticky_expr);
        }
        return;
    };
    let ids = state.selected.clone();
    for n in state.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
        match kind {
            ModButton::Fingering => {
                if let Some(at) = fingering_of(n) {
                    let count = harmonicon_core::instrument::wind::positions_for(flute, at.note)
                        .len()
                        .max(1) as u8;
                    *n = fingered(
                        *n,
                        FingeringAssignment { alternate: (at.alternate + 1) % count, ..at },
                    );
                }
            }
            ModButton::Vibrato => n.expr = next_expr(kind, anchor.expr),
            _ => {}
        }
    }
    if kind == ModButton::Vibrato {
        state.sticky_expr = next_expr(kind, anchor.expr);
    }
}

/// Moves the selection — everything, with nothing selected — `semitones`
/// up or down, onto each new pitch's standard fingering. A note the flute
/// can't reach there stays put and is counted.
pub(super) fn transpose(
    state: &mut EditorState,
    semitones: i32,
) -> super::transpose::TransposeOutcome {
    let instrument = state.effective_instrument();
    let Some(flute) = instrument.fingering_chart() else {
        return super::transpose::TransposeOutcome { semitones, ..Default::default() };
    };
    let everything = state.selected.is_empty();
    let ids = state.selected.clone();
    let (mut moved, mut kept) = (0, 0);
    for n in state.notes.iter_mut().filter(|n| everything || ids.contains(&n.id)) {
        let Some(at) = fingering_of(n) else { continue };
        let target = u8::try_from(i32::from(at.note) + semitones).ok();
        match target.filter(|&note| {
            !harmonicon_core::instrument::wind::positions_for(flute, note).is_empty()
        }) {
            Some(note) => {
                *n = fingered(*n, FingeringAssignment { note, alternate: 0 });
                moved += 1;
            }
            None => kept += 1,
        }
    }
    super::transpose::TransposeOutcome { semitones, moved, kept, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::flute::FootJoint;

    fn flute_state() -> EditorState {
        EditorState {
            instrument: super::super::state::InstrumentSelection::Configured {
                instrument: Flute::new(FootJoint::C).into(),
                harmonica_origin: None,
            },
            ..Default::default()
        }
    }

    fn at(state: &EditorState, id: u32) -> FingeringAssignment {
        fingering_of(state.note_by_id(id).expect("note exists")).expect("a fingering")
    }

    #[test]
    fn out_of_range_placement_leaves_existing_notes_unchanged() {
        for foot in [FootJoint::C, FootJoint::B] {
            let mut state = flute_state();
            state.instrument = super::super::state::InstrumentSelection::Configured {
                instrument: Flute::new(foot).into(),
                harmonica_origin: None,
            };
            let lowest_row = state.row_count();
            place(&mut state, lowest_row, 0, 24);
            let before = state.notes.clone();
            for row in [0, state.row_count() + 1, u8::MAX] {
                place(&mut state, row, 12, 12);
                assert_eq!(state.notes, before);
                assert!(state.unplayable_notice);
            }
        }
    }

    /// Row 1 is C7; E4 is 32 semitones lower.
    #[test]
    fn a_click_places_the_rows_pitch_on_its_standard_fingering() {
        let mut state = flute_state();
        place(&mut state, 33, 0, 12);
        assert_eq!(at(&state, 0), FingeringAssignment { note: 64, alternate: 0 });
        assert_eq!(state.notes[0].row(), 33);
        assert_eq!(state.selected, [0]);
    }

    #[test]
    fn a_new_note_ends_the_one_sounding_and_stops_at_the_next() {
        let mut state = flute_state();
        place(&mut state, 33, 0, 24);
        place(&mut state, 30, 24, 12);
        place(&mut state, 31, 12, 24);
        assert_eq!(state.note_by_id(0).unwrap().len, 12, "cut where the new one starts");
        assert_eq!(state.note_by_id(2).unwrap().len, 12, "stops at the next onset");
        place(&mut state, 20, 24, 12);
        assert_eq!(state.notes.len(), 3, "two onsets together would be a chord");
        assert!(state.unplayable_notice);
    }

    /// D6 has the standard fingering and two alternates; E4 only one.
    #[test]
    fn the_fingering_button_cycles_a_notes_alternates() {
        let mut state = flute_state();
        place(&mut state, 11, 0, 12);
        let mut seen = Vec::new();
        for _ in 0..3 {
            apply_modifier(&mut state, ModButton::Fingering);
            seen.push(at(&state, 0).alternate);
        }
        assert_eq!(seen, [1, 2, 0]);
        place(&mut state, 33, 24, 12);
        apply_modifier(&mut state, ModButton::Fingering);
        assert_eq!(at(&state, 1).alternate, 0, "nothing to cycle to");
    }

    /// Fingering view draws fourteen rows on a C foot (its keys and the
    /// gap between the hands), every note in
    /// the middle one's place, and refuses to place a note on a key row;
    /// note view is untouched by it.
    #[test]
    fn fingering_view_draws_keys_and_places_nothing() {
        let mut state = flute_state();
        place(&mut state, 33, 0, 12);
        assert_eq!((state.display_rows(), state.row_count()), (37, 37));
        state.flute_view = super::super::state::FluteView::Fingering;
        assert_eq!(state.display_rows(), 14, "a C foot's rows, the hands' gap included");
        assert_eq!(state.row_count(), 37, "the notes' own rows are still pitches");
        assert_eq!(state.display_row(&state.notes[0]), 7);
        super::super::interaction::select_or_add(&mut state, 3, 24);
        assert_eq!(state.notes.len(), 1);
        state.flute_view = state.flute_view.toggled();
        super::super::interaction::select_or_add(&mut state, 3, 24);
        assert_eq!(state.notes.len(), 2);
    }

    #[test]
    fn switching_view_rebuilds_the_grid() {
        let mut state = flute_state();
        let mut cache = super::super::grid_cache::GridCache::default();
        assert!(cache.update(&state, 20, false));
        assert!(!cache.update(&state, 20, false));
        state.flute_view = state.flute_view.toggled();
        assert!(cache.update(&state, 20, false));
    }

    #[test]
    fn transposing_moves_onto_standard_fingerings_and_keeps_what_falls_off() {
        let mut state = flute_state();
        place(&mut state, 11, 0, 12);
        apply_modifier(&mut state, ModButton::Fingering);
        place(&mut state, 37, 24, 12);
        state.selected.clear();
        let outcome = transpose(&mut state, -1);
        assert_eq!((outcome.moved, outcome.kept), (1, 1), "C4 has nowhere lower on a C foot");
        assert_eq!(at(&state, 0), FingeringAssignment { note: 85, alternate: 0 });
        assert_eq!(at(&state, 1).note, 60);
    }
}
