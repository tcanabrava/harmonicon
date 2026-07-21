// SPDX-License-Identifier: MIT

//! Ctrl+C/Ctrl+V for the note grid: `NoteClipboard` holds the last copied
//! notes verbatim (hole, pitch, direction, expression — everything except
//! their id, which a fresh paste always reassigns); [`paste_targets`]
//! derives where each one lands. See `interaction::handle_copy_paste` for
//! the keyboard wiring.

use bevy::prelude::Resource;

use super::state::GridNote;

/// The last Ctrl+C'd notes, exactly as copied. Empty until the first copy;
/// a copy with nothing selected leaves it untouched rather than clearing it
/// (an accidental Ctrl+C shouldn't wipe out a clipboard the player meant to
/// keep pasting from).
#[derive(Resource, Default)]
pub(super) struct NoteClipboard(pub(super) Vec<GridNote>);

/// Every currently-selected note, verbatim — what Ctrl+C stores into
/// [`NoteClipboard`].
pub(super) fn copy_selected(notes: &[GridNote], selected: &[u32]) -> Vec<GridNote> {
    notes
        .iter()
        .filter(|n| selected.contains(&n.id))
        .copied()
        .collect()
}

/// Where a Ctrl+V of `clipboard` lands: the clipboard's own *earliest* note
/// arrives at `target_tick` (the tick under the mouse), every other member
/// keeps its original offset from that earliest note — pasting preserves
/// the copied shape, just shifted in time. Holes are never changed (only
/// `target_tick` — where the mouse is — drives the paste, not vertical
/// position). Ids are freshly assigned starting at `next_id`, returned
/// alongside so the caller can advance `EditorState::next_id` by exactly
/// how many notes actually landed.
///
/// A note is silently skipped (not forced) if its hole doesn't exist on
/// the current harp, or if its computed target would overlap a note
/// already in `existing` — pasting where nothing fits is a no-op for that
/// one note, not an error, the same "silently skip what doesn't fit"
/// spirit `select_or_add`'s sticky-pitch fallback already follows.
pub(super) fn paste_targets(
    clipboard: &[GridNote],
    target_tick: usize,
    hole_count: u8,
    existing: &[GridNote],
    next_id: u32,
) -> (Vec<GridNote>, u32) {
    let Some(min_tick) = clipboard.iter().map(|n| n.tick).min() else {
        return (Vec::new(), next_id);
    };
    let mut id = next_id;
    let mut out = Vec::new();
    for n in clipboard {
        if n.hole > hole_count {
            continue;
        }
        let tick = target_tick + (n.tick - min_tick);
        let collides = existing
            .iter()
            .any(|e| e.hole == n.hole && e.tick < tick + n.len && tick < e.tick + e.len);
        if collides {
            continue;
        }
        out.push(GridNote { id, tick, ..*n });
        id += 1;
    }
    (out, id)
}
