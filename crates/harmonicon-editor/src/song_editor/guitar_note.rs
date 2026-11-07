// SPDX-License-Identifier: MIT

//! Adapter for guitar notes on the shared grid, the counterpart of
//! `harmonica_note.rs`. A grid row is a string. Dragging a note to another
//! row keeps its fret, as a harmonica note keeps its breath and technique
//! on another hole; the String buttons instead keep its pitch. Every edit
//! is checked against the guitar's voice rules first and refused whole,
//! never repaired by moving some other note.
use super::interaction::next_expr;
use super::note_edit::fits;
use super::state::{DEFAULT_INTENSITY, EditorState, Expr, GridNote};
use super::ui::ModButton;
use harmonicon_core::instrument::guitar::{FretAssignment, Guitar, StringTechnique};
use harmonicon_core::instrument::{Instrument, PlayPosition};
use harmonicon_core::note_target::NoteTarget;

pub(super) fn is_guitar(state: &EditorState) -> bool {
    matches!(state.effective_instrument(), Instrument::Fretted(_))
}

fn fret_of(note: &GridNote) -> Option<FretAssignment> {
    match note.target.position() {
        Some(PlayPosition::Fret(at)) => Some(at),
        _ => None,
    }
}

fn with_fret(note: GridNote, at: FretAssignment) -> GridNote {
    GridNote { target: NoteTarget::Positioned(PlayPosition::Fret(at)), ..note }
}

/// Places a note on `string` at `tick` and the armed fret, with the armed
/// expression, and selects it. Refused, and reported, when the guitar can't
/// play it alongside what is already sounding.
pub(super) fn place(state: &mut EditorState, string: u8, tick: usize, len: usize) {
    let instrument = state.effective_instrument();
    let note = GridNote::new(
        state.next_id,
        tick,
        len,
        state.sticky_expr,
        NoteTarget::Positioned(PlayPosition::Fret(FretAssignment {
            string,
            fret: state.sticky_fret,
            technique: StringTechnique::Fretted,
        })),
    );
    if !fits(&instrument, &state.notes, &note, &[]) {
        state.unplayable_notice = true;
        return;
    }
    state.notes.push(note);
    state.next_id += 1;
    state.select_only(note.id);
    if note.expr != Expr::None && state.sticky_intensity != DEFAULT_INTENSITY {
        let depth = state.sticky_intensity.clone();
        state.expression_intensities.insert(note.id, depth);
    }
}

/// The next bend depth a press of Bend gives a note at `technique`: off, a
/// half step, a whole step (the guitar's most common bend), then off again.
/// A harmonic can't be bent, so a press on one starts from a half step.
pub(super) fn next_bend(technique: StringTechnique) -> StringTechnique {
    match technique {
        StringTechnique::Bend(depth) if depth < 1.5 => StringTechnique::Bend(2.0),
        StringTechnique::Bend(_) => StringTechnique::Fretted,
        StringTechnique::Fretted | StringTechnique::Harmonic => StringTechnique::Bend(1.0),
    }
}

/// `at` moved onto `string` at whichever fret sounds the same pitch, or
/// `None` when that string can't reach it.
fn same_pitch_on(
    instrument: &Instrument,
    guitar: &Guitar,
    note: &GridNote,
    at: FretAssignment,
    string: u8,
) -> Option<FretAssignment> {
    let pitch = note.target.sounding_pitch(instrument)?;
    (0..=guitar.top_fret()).map(|fret| FretAssignment { string, fret, ..at }).find(|candidate| {
        NoteTarget::Positioned(PlayPosition::Fret(*candidate))
            .sounding_pitch(instrument)
            .is_some_and(|p| (p - pitch).abs() < 0.001)
    })
}

/// One of the note column's buttons on a guitar: Fret −/+, String ↑/↓,
/// Bend, Harmonic and Vibrato. The primary selected note decides where a
/// cycling button goes next, as on a harmonica; every selected note then
/// takes the edit only if it stays playable, and one that doesn't keeps
/// its own and is reported. With nothing selected, Fret −/+ arm the fret
/// the next placed note gets and Vibrato arms its expression.
pub(super) fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    let instrument = state.effective_instrument();
    let Instrument::Fretted(guitar) = &instrument else { return };
    let Some(anchor) = state.selected_note().copied() else {
        match kind {
            ModButton::FretUp => {
                state.sticky_fret = (state.sticky_fret + 1).min(guitar.top_fret());
            }
            ModButton::FretDown => state.sticky_fret = state.sticky_fret.saturating_sub(1),
            ModButton::Vibrato => state.sticky_expr = next_expr(kind, state.sticky_expr),
            _ => {}
        }
        return;
    };
    let anchor_technique = fret_of(&anchor).map_or(StringTechnique::Fretted, |at| at.technique);
    let ids = state.selected.clone();
    let mut refused = 0;
    for &id in &ids {
        let Some(note) = state.note_by_id(id).copied() else { continue };
        let refit = |at: Option<FretAssignment>| at.map(|at| with_fret(note, at));
        let candidate = match (kind, fret_of(&note)) {
            (ModButton::Vibrato, _) => {
                Some(GridNote { expr: next_expr(kind, anchor.expr), ..note })
            }
            (_, None) => continue,
            (ModButton::FretUp, Some(at)) => {
                refit(Some(FretAssignment { fret: at.fret + 1, ..at }))
            }
            (ModButton::FretDown, Some(at)) => {
                refit(at.fret.checked_sub(1).map(|fret| FretAssignment { fret, ..at }))
            }
            (ModButton::StringUp, Some(at)) => refit(
                at.string
                    .checked_sub(1)
                    .and_then(|string| same_pitch_on(&instrument, guitar, &note, at, string)),
            ),
            (ModButton::StringDown, Some(at)) => {
                refit(same_pitch_on(&instrument, guitar, &note, at, at.string + 1))
            }
            (ModButton::Bend, Some(at)) => {
                refit(Some(FretAssignment { technique: next_bend(anchor_technique), ..at }))
            }
            (ModButton::Harmonic, Some(at)) => refit(Some(FretAssignment {
                technique: if anchor_technique == StringTechnique::Harmonic {
                    StringTechnique::Fretted
                } else {
                    StringTechnique::Harmonic
                },
                ..at
            })),
            _ => return,
        };
        match candidate.filter(|c| fits(&instrument, &state.notes, c, &[])) {
            Some(edited) => {
                if matches!(kind, ModButton::StringUp | ModButton::StringDown) {
                    state.note_motion.push(super::note_motion::Request {
                        id,
                        rows: f32::from(note.row()) - f32::from(edited.row()),
                        rejected: false,
                    });
                }
                if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                    *n = edited;
                }
            }
            None => {
                refused += 1;
                if matches!(kind, ModButton::StringUp | ModButton::StringDown) {
                    state.note_motion.push(super::note_motion::Request {
                        id,
                        rows: if kind == ModButton::StringUp { -1.0 } else { 1.0 },
                        rejected: true,
                    });
                }
            }
        }
    }
    if kind == ModButton::Vibrato {
        state.sticky_expr = next_expr(kind, anchor.expr);
    }
    if let Some(at) = state.selected_note().and_then(fret_of) {
        state.sticky_fret = at.fret;
    }
    match (refused, ids.len()) {
        (0, _) => {}
        (_, 1) => state.unplayable_notice = true,
        (count, _) => state.technique_notice = Some(count),
    }
}

/// Whether a drag's targets — `(id, row, tick, len, _)`, as
/// `grid::group_move_targets` computes them — all land playable. The notes
/// being moved are checked against everything else, not against each
/// other: they move as one shape, so they keep their relative places.
pub(super) fn group_fits<T>(
    instrument: &Instrument,
    notes: &[GridNote],
    moving: &[u32],
    targets: &[(u32, u8, usize, usize, T)],
) -> bool {
    targets.iter().all(|&(id, row, tick, _, _)| {
        let Some(mut moved) = notes.iter().find(|n| n.id == id).copied() else { return false };
        moved.set_row(row);
        moved.tick = tick;
        fits(instrument, notes, &moved, moving)
    })
}

/// Run condition for the harmonica-only developer tools (`expected_notes`,
/// `debug_record`), which read every note as a hole.
#[cfg(any(feature = "dev", test))]
pub(super) fn is_harmonica(state: Option<bevy::prelude::Res<EditorState>>) -> bool {
    state.is_none_or(|s| matches!(s.effective_instrument(), Instrument::Harmonica(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::guitar::GuitarBody;

    fn guitar_state() -> EditorState {
        EditorState {
            instrument: super::super::state::InstrumentSelection::Configured {
                instrument: Guitar::standard(GuitarBody::Electric).into(),
                harmonica_origin: None,
            },
            ..Default::default()
        }
    }

    fn at(state: &EditorState, id: u32) -> FretAssignment {
        fret_of(state.note_by_id(id).expect("note exists")).expect("a string position")
    }

    fn fretted(string: u8, fret: u8) -> FretAssignment {
        FretAssignment { string, fret, technique: StringTechnique::Fretted }
    }

    /// A3, on the third string's second fret, placed by a click and selected.
    fn with_a3() -> EditorState {
        let mut state = guitar_state();
        state.sticky_fret = 2;
        place(&mut state, 3, 0, 12);
        state
    }

    #[test]
    fn string_buttons_queue_movement_or_rejection_feedback() {
        let mut state = with_a3();
        apply_modifier(&mut state, ModButton::StringUp);
        assert_eq!(at(&state, 0), fretted(3, 2));
        assert!(state.note_motion.last().unwrap().rejected);
        apply_modifier(&mut state, ModButton::StringDown);
        assert_eq!(at(&state, 0), fretted(4, 7));
        assert!(!state.note_motion.last().unwrap().rejected);
        assert_eq!(state.note_motion.last().unwrap().rows, -1.0);
    }

    #[test]
    fn a_click_places_a_note_on_its_string_at_the_armed_fret() {
        let state = with_a3();
        assert_eq!(at(&state, 0), fretted(3, 2));
        assert_eq!(state.selected, [0]);
        assert_eq!(state.next_id, 1);
    }

    #[test]
    fn a_note_onto_a_sounding_string_is_refused_and_reported() {
        let mut state = with_a3();
        state.sticky_fret = 4;
        place(&mut state, 3, 6, 12);
        assert_eq!(state.notes.len(), 1);
        assert!(state.unplayable_notice);
    }

    #[test]
    fn fret_buttons_move_the_selected_note_or_arm_the_next_one() {
        let mut state = with_a3();
        apply_modifier(&mut state, ModButton::FretUp);
        assert_eq!(at(&state, 0), fretted(3, 3));
        assert_eq!(state.sticky_fret, 3, "the next note follows the edited one");
        state.selected.clear();
        apply_modifier(&mut state, ModButton::FretDown);
        assert_eq!(state.sticky_fret, 2);
        assert_eq!(at(&state, 0).fret, 3, "with nothing selected no note moves");
    }

    /// Moving to another string keeps the note's sound: A3 on string 4 is
    /// the seventh fret, and the open B3 string can't play it at all.
    #[test]
    fn string_buttons_keep_the_pitch_or_refuse() {
        let mut state = with_a3();
        apply_modifier(&mut state, ModButton::StringDown);
        assert_eq!(at(&state, 0), fretted(4, 7));
        apply_modifier(&mut state, ModButton::StringUp);
        apply_modifier(&mut state, ModButton::StringUp);
        assert_eq!(at(&state, 0), fretted(3, 2), "nothing above A3 on string 2");
        assert!(state.unplayable_notice);
    }

    #[test]
    fn bend_cycles_through_a_half_step_and_a_whole_step_to_off() {
        let mut state = with_a3();
        let mut techniques = Vec::new();
        for _ in 0..3 {
            apply_modifier(&mut state, ModButton::Bend);
            techniques.push(at(&state, 0).technique);
        }
        assert_eq!(
            techniques,
            [StringTechnique::Bend(1.0), StringTechnique::Bend(2.0), StringTechnique::Fretted]
        );
        assert_eq!(next_bend(StringTechnique::Harmonic), StringTechnique::Bend(1.0));
    }

    #[test]
    fn harmonic_toggles_only_where_the_fret_has_one() {
        let mut state = guitar_state();
        state.sticky_fret = 12;
        place(&mut state, 6, 0, 12);
        apply_modifier(&mut state, ModButton::Harmonic);
        assert_eq!(at(&state, 0).technique, StringTechnique::Harmonic);
        apply_modifier(&mut state, ModButton::Harmonic);
        assert_eq!(at(&state, 0).technique, StringTechnique::Fretted);
        let mut state = with_a3();
        apply_modifier(&mut state, ModButton::Harmonic);
        assert_eq!(at(&state, 0).technique, StringTechnique::Fretted);
    }

    /// A3 held on the third string for two beats: a copy of a note fretted
    /// ten frets away can't land under it, though the same copy pasted
    /// after it has ended can.
    #[test]
    fn a_paste_skips_a_note_the_hand_cannot_reach() {
        let mut state = with_a3();
        state.notes[0].len = 24;
        state.selected.clear();
        state.sticky_fret = 12;
        place(&mut state, 1, 30, 6);
        let clip = state.copy_selection();
        assert!(!state.paste(&clip, 12), "fret 12 beside a held fret 2");
        assert_eq!(state.notes.len(), 2);
        assert!(state.paste(&clip, 24));
        assert_eq!(state.notes.len(), 3);
    }

    #[test]
    fn sustained_strings_and_hand_span_are_checked_before_anything_changes() {
        let mut state = guitar_state();
        let instrument = state.effective_instrument();
        let note = |id, string, fret, tick, len| {
            with_fret(
                GridNote { id, tick, len, ..GridNote::plain(id, 1, tick, len) },
                fretted(string, fret),
            )
        };
        state.notes.push(note(1, 1, 2, 0, 24));
        assert!(!fits(&instrument, &state.notes, &note(2, 1, 4, 12, 6), &[]));
        assert!(!fits(&instrument, &state.notes, &note(2, 2, 10, 12, 6), &[]));
        assert!(fits(&instrument, &state.notes, &note(2, 2, 4, 12, 6), &[]));
        assert!(fits(&instrument, &state.notes, &note(2, 1, 10, 24, 6), &[]));
        // A drag onto the held string is refused; one moving the held note
        // with it is not.
        state.notes.push(note(2, 2, 4, 12, 6));
        assert!(!group_fits(&instrument, &state.notes, &[2], &[(2, 1, 12, 6, ())]));
        assert!(group_fits(&instrument, &state.notes, &[1, 2], &[(2, 1, 12, 6, ())]));
    }
}
