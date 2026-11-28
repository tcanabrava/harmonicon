// SPDX-License-Identifier: MIT

//! Adapter for sung notes on the shared grid, the counterpart of
//! `piano_note.rs`. A row is a pitch counted down from A5
//! ([`super::note_model::VOICE_TOP_NOTE`], a soprano's top), and the grid
//! shows the voice's own range by scrolling past the rows above it
//! (`reset_window`), as a clarinet's does. A voice sings one note at a
//! time, which `note_edit::fits` enforces by asking the instrument; its
//! one expression is vibrato.
use super::note_model::VOICE_TOP_NOTE;
use super::state::{DEFAULT_INTENSITY, EditorState, GridNote};
use super::ui::ModButton;
use harmonicon_core::instrument::{Instrument, InstrumentModel, PlayPosition};
use harmonicon_core::note_target::NoteTarget;

/// The pitch on grid row `row` (from 1, A5 at the top).
pub(super) fn row_note(row: u8) -> u8 {
    VOICE_TOP_NOTE + 1 - row
}

fn pitch_of(note: &GridNote) -> Option<u8> {
    match note.target.position() {
        Some(PlayPosition::Sung(pitch)) => Some(pitch),
        _ => None,
    }
}

/// Places a sung note on `row`'s pitch at `tick` with the armed vibrato,
/// and selects it. Refused, and reported, outside the voice's range or over
/// a note still sounding.
pub(super) fn place(state: &mut EditorState, row: u8, tick: usize, len: usize) {
    let instrument = state.effective_instrument();
    let note = GridNote::new(
        state.next_id,
        tick,
        len,
        state.sticky_expr,
        NoteTarget::Positioned(PlayPosition::Sung(row_note(row))),
    );
    if instrument.pitch_of(&PlayPosition::Sung(row_note(row))).is_none()
        || !super::note_edit::fits(&instrument, &state.notes, &note, &[])
    {
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

/// Vibrato on the selection, or armed for the next note with nothing
/// selected; nothing else applies to a voice.
pub(super) fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    if kind != ModButton::Vibrato {
        return;
    }
    let Some(anchor) = state.selected_note().copied() else {
        state.sticky_expr = super::interaction::next_expr(kind, state.sticky_expr);
        return;
    };
    let expr = super::interaction::next_expr(kind, anchor.expr);
    let ids = state.selected.clone();
    for n in state.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
        n.expr = expr;
    }
    state.sticky_expr = expr;
}

/// Moves the selection — everything, with nothing selected — `semitones`
/// up or down. A note that would leave the voice's range stays put and is
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
        let Some(pitch) = pitch_of(n) else { continue };
        let target = u8::try_from(i32::from(pitch) + semitones)
            .ok()
            .filter(|&p| instrument.pitch_of(&PlayPosition::Sung(p)).is_some());
        match target {
            Some(pitch) => {
                n.target = NoteTarget::Positioned(PlayPosition::Sung(pitch));
                moved += 1;
            }
            None => kept += 1,
        }
    }
    super::transpose::TransposeOutcome { semitones, moved, kept, ..Default::default() }
}

/// The rows above the voice's highest note, which the grid scrolls past.
pub(super) fn window_offset(instrument: &Instrument) -> Option<u8> {
    match instrument {
        Instrument::Voice(voice) => Some(VOICE_TOP_NOTE - voice.highest()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::voice::{SingingVoice, VoiceType};

    fn voice_state(kind: VoiceType) -> EditorState {
        let mut state = EditorState {
            instrument: super::super::state::InstrumentSelection::Configured {
                instrument: SingingVoice::new(kind).into(),
                harmonica_origin: None,
            },
            ..Default::default()
        };
        super::super::piano_note::reset_window(&mut state);
        state
    }

    /// The grid shows exactly the voice's range, highest pitch first.
    #[test]
    fn the_grid_shows_the_voice_range() {
        let state = voice_state(VoiceType::Tenor);
        assert_eq!(state.display_rows(), 69 - 48 + 1);
        assert_eq!(row_note(state.row_offset + 1), 69, "A4 at the top");
        assert_eq!(row_note(state.row_offset + state.display_rows()), 48, "C3 at the bottom");
    }

    /// One note at a time, and only inside the range.
    #[test]
    fn a_voice_sings_one_note_at_a_time_inside_its_range() {
        let mut state = voice_state(VoiceType::Soprano);
        let c4 = VOICE_TOP_NOTE + 1 - 60;
        place(&mut state, c4, 0, 12);
        assert_eq!(state.notes.iter().filter_map(pitch_of).collect::<Vec<_>>(), [60]);
        place(&mut state, c4 - 4, 6, 12);
        assert_eq!(state.notes.len(), 1, "C4 is still sounding");
        place(&mut state, VOICE_TOP_NOTE + 1 - 50, 24, 12);
        assert_eq!(state.notes.len(), 1, "D3 is below a soprano");
        assert!(state.unplayable_notice);
        let (_, label) = super::super::grid::note_face(&state.notes[0]);
        assert_eq!(label, "C4");
    }

    #[test]
    fn transposing_keeps_notes_that_would_leave_the_range() {
        let mut state = voice_state(VoiceType::Bass);
        place(&mut state, VOICE_TOP_NOTE + 1 - 64, 0, 12);
        place(&mut state, VOICE_TOP_NOTE + 1 - 50, 24, 12);
        state.selected.clear();
        let outcome = transpose(&mut state, 1);
        assert_eq!((outcome.moved, outcome.kept), (1, 1), "nothing above E4");
        assert_eq!(pitch_of(&state.notes[1]), Some(51));
    }

    #[test]
    fn vibrato_is_the_one_expression() {
        let mut state = voice_state(VoiceType::Alto);
        place(&mut state, VOICE_TOP_NOTE + 1 - 60, 0, 12);
        apply_modifier(&mut state, ModButton::Vibrato);
        assert_ne!(state.notes[0].expr, super::super::state::Expr::None);
        let before = state.notes[0];
        apply_modifier(&mut state, ModButton::Bend);
        assert_eq!(state.notes[0], before);
    }
}
