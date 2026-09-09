// SPDX-License-Identifier: MIT

//! Undo/redo for the note grid — `Ctrl+Z`/`Ctrl+Y` (see
//! `interaction::handle_undo_redo`) step through a history of the editor's
//! *content*: [`EditorState::notes`]/[`EditorState::tempo_changes`], the
//! two places a mistake is destructive rather than trivially undone by
//! clicking again (unlike a click-to-cycle meta field or a plain toggle).
//! Every other `EditorState` field is deliberately excluded — undoing a
//! note edit shouldn't rewind an unrelated scroll position or field.
//!
//! Snapshot-based, not command-based: [`track_changes`] runs every frame
//! `EditorState` changes and diffs against the last-seen snapshot, pushing
//! the *previous* one onto the undo stack only when content actually
//! differs — no instrumentation needed at each note-mutating call site.
//! The one exception is live recording (`record::RecordState::active`):
//! notes grow every frame during a take, so [`track_changes`] skips while
//! one is active, collapsing the whole take into one undo step.

use bevy::prelude::*;

use super::record::RecordState;
use super::state::{EditorState, GridNote};

/// History is limited by both edit count and retained allocation size.
pub(super) const HISTORY_LIMIT: usize = 100;
const HISTORY_BYTES: usize = 32 * 1024 * 1024;

/// The editable content one undo/redo step restores — deliberately
/// narrower than `EditorState` itself, see the module doc comment.
#[derive(Clone, PartialEq)]
struct Snapshot {
    notes: Vec<GridNote>,
    tempo_changes: Vec<(usize, f32)>,
}

impl Snapshot {
    fn bytes(&self) -> usize {
        self.notes.capacity() * std::mem::size_of::<GridNote>()
            + self.tempo_changes.capacity() * std::mem::size_of::<(usize, f32)>()
    }

    fn matches(&self, state: &EditorState) -> bool {
        self.notes == state.notes && self.tempo_changes == state.tempo_changes
    }

    fn capture(state: &EditorState) -> Self {
        Self {
            notes: state.notes.clone(),
            tempo_changes: state.tempo_changes.clone(),
        }
    }

    fn restore(self, state: &mut EditorState) {
        state.notes = self.notes;
        state.tempo_changes = self.tempo_changes;
        state.prune_selection();
    }
}

/// Undo/redo history for the current editing session — reset every time
/// the Song Editor is (re)entered (`ui::init_state`), even though
/// `EditorState` itself can persist across a leave-and-return within one
/// app session: a history that quietly outlives the notes it describes is
/// more likely to confuse than help.
#[derive(Resource, Default)]
pub(super) struct UndoHistory {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    /// The content as of the last [`Self::record_if_changed`] call —
    /// `None` only before the very first one, which seeds it without
    /// pushing anything (there's nothing to undo *to* yet).
    last: Option<Snapshot>,
}

impl UndoHistory {
    fn trim(&mut self) {
        self.trim_to(HISTORY_BYTES);
    }

    fn trim_to(&mut self, budget: usize) {
        // The current snapshot is required for comparisons, even if a single
        // document exceeds the budget. Drop old undo/redo entries first.
        let mut bytes: usize = self
            .past
            .iter()
            .chain(&self.future)
            .map(Snapshot::bytes)
            .sum();
        bytes += self.last.as_ref().map_or(0, Snapshot::bytes);
        while self.past.len() + self.future.len() > HISTORY_LIMIT || bytes > budget {
            let removed = if !self.past.is_empty() {
                self.past.remove(0)
            } else if !self.future.is_empty() {
                self.future.remove(0)
            } else {
                break;
            };
            bytes -= removed.bytes();
        }
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Steps `state` back to the previous snapshot, if any, pushing the
    /// current one onto the redo stack. A no-op if there's nothing to
    /// undo.
    pub(super) fn undo(&mut self, state: &mut EditorState) {
        let Some(prev) = self.past.pop() else {
            return;
        };
        if let Some(current) = self.last.replace(prev.clone()) {
            self.future.push(current);
        }
        prev.restore(state);
        self.trim();
    }

    /// Steps `state` forward to the next snapshot, pushing the current one
    /// back onto the undo stack. No-op if there's nothing to redo, or after
    /// any fresh edit — a new edit invalidates redo, same as any editor.
    pub(super) fn redo(&mut self, state: &mut EditorState) {
        let Some(next) = self.future.pop() else {
            return;
        };
        if let Some(current) = self.last.replace(next.clone()) {
            self.past.push(current);
        }
        next.restore(state);
        self.trim();
    }

    /// Compares `state`'s current content against the last-seen snapshot;
    /// if it changed, pushes the *previous* content onto the undo stack
    /// (so undoing returns to it) and clears the redo stack. A no-op if
    /// nothing content-shaped actually changed (e.g. `EditorState` changed
    /// for an unrelated reason — selection, scroll, a meta field edit).
    pub(super) fn record_if_changed(&mut self, state: &EditorState) {
        if self.last.as_ref().is_some_and(|last| last.matches(state)) {
            return;
        }
        if let Some(prev) = self.last.replace(Snapshot::capture(state)) {
            self.past.push(prev);
        }
        self.future.clear();
        self.trim();
    }
}

/// Drives [`UndoHistory::record_if_changed`] every frame `EditorState`
/// changes — except while a recording take is actively running
/// (`RecordState::active`), when notes grow every frame and diffing
/// continuously would flood the history with one entry per frame instead
/// of one entry for the whole take (see the module doc comment).
pub(super) fn track_changes(
    state: Res<EditorState>,
    record: Res<RecordState>,
    mut history: ResMut<UndoHistory>,
) {
    if !state.is_changed() || record.active {
        return;
    }
    history.record_if_changed(&state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_changes_preserve_snapshot_allocation_and_redo() {
        let mut state = EditorState::default();
        state.tempo_changes.push((0, 120.0));
        let mut history = UndoHistory::default();
        history.record_if_changed(&state);
        state.tempo_changes.push((12, 90.0));
        history.record_if_changed(&state);
        history.undo(&mut state);
        let ptr = history.last.as_ref().unwrap().tempo_changes.as_ptr();
        state.scroll_beat += 1;
        history.record_if_changed(&state);
        assert_eq!(ptr, history.last.as_ref().unwrap().tempo_changes.as_ptr());
        assert!(history.can_redo());
        history.redo(&mut state);
        assert_eq!(state.tempo_changes, vec![(0, 120.0), (12, 90.0)]);
    }

    #[test]
    fn memory_budget_evicts_old_history_without_losing_current_content() {
        let mut state = EditorState::default();
        let mut history = UndoHistory::default();
        for i in 0..5 {
            state.tempo_changes.push((i, 120.0));
            history.record_if_changed(&state);
        }
        let current_bytes = history.last.as_ref().unwrap().bytes();
        history.trim_to(current_bytes);
        assert!(!history.can_undo());
        assert!(history.last.as_ref().unwrap().matches(&state));
        state.tempo_changes.push((10, 90.0));
        history.record_if_changed(&state);
        assert!(history.can_undo());
    }
}
