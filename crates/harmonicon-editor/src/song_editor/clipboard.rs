// SPDX-License-Identifier: MIT

//! Ctrl+C/Ctrl+V for the note grid: `NoteClipboard` holds the last copied
//! notes verbatim (hole, pitch, direction, expression — everything except
//! their id, which a fresh paste always reassigns) together with the
//! metadata that belongs to them; [`paste_targets_with_sources`] derives where each one
//! lands. `EditorState::copy_selection`/`paste` (`metadata_sync`) are what
//! Ctrl+C/Ctrl+V and the mod panel's buttons actually call.

use std::collections::BTreeMap;

use bevy::prelude::Resource;

use super::state::{GridNote, PhraseAnnotation};

/// The last Ctrl+C'd notes, exactly as copied, plus their expression
/// intensities (by the *source* note's id) and their onsets' phrase
/// annotations (by the *source* tick) — a paste re-keys both onto whatever
/// lands. Empty until the first copy; a copy with nothing selected leaves
/// it untouched rather than clearing it (an accidental Ctrl+C shouldn't
/// wipe out a clipboard the player meant to keep pasting from).
#[derive(Resource, Default)]
pub(super) struct NoteClipboard {
    pub(super) notes: Vec<GridNote>,
    pub(super) intensities: BTreeMap<u32, String>,
    pub(super) annotations: BTreeMap<usize, PhraseAnnotation>,
}

impl NoteClipboard {
    pub(super) fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }
}

/// Where a Ctrl+V of `clipboard` lands, each landed note paired with the
/// clipboard note it was made from — what `EditorState::paste` needs to
/// re-key the copied intensities (source id → new id) and annotations
/// (source tick → new tick) onto exactly the notes that landed, skipping
/// those that didn't. Placement: the clipboard's own *earliest* note
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
pub(super) fn paste_targets_with_sources(
    clipboard: &[GridNote],
    target_tick: usize,
    hole_count: u8,
    existing: &[GridNote],
    next_id: u32,
) -> (Vec<(GridNote, GridNote)>, u32) {
    let Some(min_tick) = clipboard.iter().map(|n| n.tick).min() else {
        return (Vec::new(), next_id);
    };
    let mut id = next_id;
    let mut out = Vec::with_capacity(clipboard.len());
    for n in clipboard {
        if n.row() > hole_count {
            continue;
        }
        let tick = target_tick + (n.tick - min_tick);
        let collides = existing
            .iter()
            .any(|e| e.row() == n.row() && e.tick < tick + n.len && tick < e.tick + e.len);
        if collides {
            continue;
        }
        out.push((*n, GridNote { id, tick, ..*n }));
        id += 1;
    }
    (out, id)
}
