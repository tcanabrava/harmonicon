// SPDX-License-Identifier: MIT
//! Pitch-authored notes on physical string rows. Bow guidance is independent
//! of left-hand placement and survives drag/copy/paste with the note itself.
use super::state::{EditorState, GridNote, Mode};
use harmonicon_core::instrument::bowed_strings::{BowGesture, FingerboardPosition};
use harmonicon_core::instrument::{Instrument, InstrumentModel, PlayPosition};
use harmonicon_core::note_target::NoteTarget;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct NewNoteSettings {
    pub pitch: Option<u8>,
    pub bow: Option<BowGesture>,
}

pub(super) fn place(state: &mut EditorState, string: u8, tick: usize, len: usize) {
    let Instrument::Bowed(strings) = state.effective_instrument() else { return };
    let Some(pitch) = state.bowed.pitch.or_else(|| strings.open_pitch(string)) else { return };
    let Some(at) = strings.positions_for(pitch).into_iter().find(|at| at.string == string) else {
        state.unplayable_notice = true;
        return;
    };
    let note = GridNote::new(
        state.next_id,
        tick,
        len,
        state.sticky_expr,
        NoteTarget::Positioned(PlayPosition::Fingerboard(at)),
    )
    .with_performance(harmonicon_core::note_performance::PerformanceInstructions::from_bow(
        state.bowed.bow,
    ));
    if !super::note_edit::fits(&Instrument::Bowed(strings), &state.notes, &note, &[]) {
        state.unplayable_notice = true;
        return;
    }
    state.notes.push(note);
    state.next_id += 1;
    state.select_only(note.id);
}

/// Keep a note on its chosen string. Failure leaves the complete selection
/// intact, rather than quietly moving it to a different physical location.
pub(super) fn set_pitch(state: &mut EditorState, pitch: u8) -> bool {
    if state.mode != Mode::Edit {
        return false;
    }
    let Instrument::Bowed(strings) = state.effective_instrument() else { return false };
    let mut notes = state.notes.clone();
    for note in notes.iter_mut().filter(|note| state.is_selected(note.id)) {
        let Some(PlayPosition::Fingerboard(old)) = note.target.position() else { return false };
        let Some(at) = strings.positions_for(pitch).into_iter().find(|at| at.string == old.string)
        else {
            return false;
        };
        note.target = NoteTarget::Positioned(PlayPosition::Fingerboard(at));
    }
    let instrument = Instrument::Bowed(strings);
    if notes.iter().any(|note| !super::note_edit::fits(&instrument, &notes, note, &[])) {
        return false;
    }
    state.notes = notes;
    state.bowed.pitch = Some(pitch);
    true
}

pub(super) fn transpose(
    state: &mut EditorState,
    semitones: i32,
) -> super::transpose::TransposeOutcome {
    let mut outcome = super::transpose::TransposeOutcome { semitones, ..Default::default() };
    let Instrument::Bowed(strings) = state.effective_instrument() else { return outcome };
    let everything = state.selected.is_empty();
    for note in
        state.notes.iter_mut().filter(|note| everything || state.selected.contains(&note.id))
    {
        let Some(PlayPosition::Fingerboard(old)) = note.target.position() else { continue };
        let target = u8::try_from(i32::from(old.note) + semitones).ok().and_then(|pitch| {
            strings.positions_for(pitch).into_iter().find(|at| at.string == old.string)
        });
        if let Some(at) = target {
            note.target = NoteTarget::Positioned(PlayPosition::Fingerboard(at));
            outcome.moved += 1;
        } else {
            outcome.kept += 1;
        }
    }
    outcome
}

pub(super) fn set_hints(
    state: &mut EditorState,
    finger: Option<Option<u8>>,
    hand: Option<Option<u8>>,
) -> bool {
    if state.mode != Mode::Edit {
        return false;
    }
    let instrument = state.effective_instrument();
    let mut notes = state.notes.clone();
    for note in notes.iter_mut().filter(|note| state.is_selected(note.id)) {
        let Some(PlayPosition::Fingerboard(mut at)) = note.target.position() else { return false };
        if let Some(finger) = finger {
            at.finger = finger;
        }
        if let Some(hand) = hand {
            at.hand_position = hand;
        }
        if instrument.pitch_of(&PlayPosition::Fingerboard(at)).is_none() {
            return false;
        }
        note.target = NoteTarget::Positioned(PlayPosition::Fingerboard(at));
    }
    state.notes = notes;
    true
}

pub(super) fn at(note: &GridNote) -> Option<FingerboardPosition> {
    match note.target.position() {
        Some(PlayPosition::Fingerboard(at)) => Some(at),
        _ => None,
    }
}

pub(super) fn set_open(state: &mut EditorState) -> bool {
    let Instrument::Bowed(strings) = state.effective_instrument() else { return false };
    if state.mode != Mode::Edit {
        return false;
    }
    let ids = state.selected.clone();
    for note in state.notes.iter_mut().filter(|note| ids.contains(&note.id)) {
        let Some(PlayPosition::Fingerboard(at)) = note.target.position() else { return false };
        let Some(pitch) = strings.open_pitch(at.string) else { return false };
        let Some(at) = strings.positions_for(pitch).into_iter().find(|new| new.string == at.string)
        else {
            return false;
        };
        note.target = NoteTarget::Positioned(PlayPosition::Fingerboard(at));
    }
    state.bowed.pitch = None;
    true
}

pub(super) fn apply_modifier(state: &mut EditorState, kind: super::ui::ModButton) {
    use super::ui::ModButton;
    let Instrument::Bowed(strings) = state.effective_instrument() else { return };
    let ids = state.selected.clone();
    if kind == ModButton::Vibrato {
        let expr = super::interaction::next_expr(
            kind,
            state.selected_note().map_or(state.sticky_expr, |note| note.expr),
        );
        for note in state.notes.iter_mut().filter(|note| ids.contains(&note.id)) {
            note.expr = expr;
        }
        state.sticky_expr = expr;
    } else if matches!(kind, ModButton::StringUp | ModButton::StringDown) {
        let mut notes = state.notes.clone();
        for note in notes.iter_mut().filter(|note| ids.contains(&note.id)) {
            let Some(old) = at(note) else { continue };
            let string = if kind == ModButton::StringUp {
                old.string.checked_sub(1)
            } else {
                old.string.checked_add(1)
            };
            let Some(new) = string.and_then(|string| {
                strings.positions_for(old.note).into_iter().find(|at| at.string == string)
            }) else {
                state.unplayable_notice = true;
                return;
            };
            note.target = NoteTarget::Positioned(PlayPosition::Fingerboard(new));
        }
        let instrument = Instrument::Bowed(strings);
        if notes.iter().any(|note| !super::note_edit::fits(&instrument, &notes, note, &[])) {
            state.unplayable_notice = true;
            return;
        }
        state.notes = notes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::bowed_strings::{BowArticulation, BowDirection, BowedKind};
    fn state() -> EditorState {
        super::super::document_picker::fresh(super::super::state::InstrumentSelection::Configured {
            instrument: BowedKind::Violin.standard().into(),
            harmonica_origin: None,
        })
    }
    #[test]
    fn placement_and_string_moves_preserve_pitch_and_bow_and_refuse_impossible_edits() {
        let mut state = state();
        state.bowed.bow = Some(BowGesture {
            direction: Some(BowDirection::Down),
            articulation: BowArticulation::Legato,
        });
        place(&mut state, 2, 0, 12);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(at(&state.notes[0]).unwrap().note, 69);
        let bow = state.notes[0].performance.bowed();
        apply_modifier(&mut state, super::super::ui::ModButton::StringDown);
        assert_eq!(at(&state.notes[0]).unwrap().string, 3);
        assert_eq!(at(&state.notes[0]).unwrap().note, 69);
        assert_eq!(state.notes[0].performance.bowed(), bow);
        let before = state.notes.clone();
        assert!(!set_pitch(&mut state, 54));
        assert_eq!(state.notes, before);
        assert!(set_pitch(&mut state, 64));
        assert_eq!(at(&state.notes[0]).unwrap().note, 64);
    }
    #[test]
    fn chart_round_trip_preserves_fretless_placement_and_independent_bow_guidance() {
        let original: serde_json::Value = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/violin.harpchart"
        ))
        .unwrap();
        let mut state = EditorState::default();
        super::super::harpchart::load_harpchart(
            &original,
            &mut state,
            &mut super::super::state::Scroll::default(),
        );
        assert_eq!(state.notes.len(), 15);
        assert!(state.notes[0].performance.bowed().is_some());
        assert!(at(&state.notes[0]).is_some());
        let mut saved =
            serde_json::from_str(&super::super::harpchart::serialize_harpchart(&state)).unwrap();
        harmonicon_song::song::validate_and_migrate_chart(&mut saved).unwrap();
        let after: harmonicon_core::chart::HarpChart = serde_json::from_value(saved).unwrap();
        let before: harmonicon_core::chart::HarpChart = serde_json::from_value(original).unwrap();
        assert_eq!(before.instrument(), after.instrument());
        let events = |chart: &harmonicon_core::chart::HarpChart| {
            chart
                .track
                .iter()
                .flat_map(|item| &item.events)
                .map(|event| (chart.event_pitch(event), event.fingerboard, event.bow))
                .collect::<Vec<_>>()
        };
        assert_eq!(events(&before), events(&after));
    }
}
