// SPDX-License-Identifier: MIT
//! Shared note-edit constraints, independent of any teaching instrument.
use super::state::GridNote;
use harmonicon_core::instrument::{Instrument, InstrumentModel};

/// Whether `candidate` is playable, and playable alongside every note
/// sounding at the same time — overlapping sustains, not only shared
/// attacks, since a held string is still occupied. Notes in `moving` are
/// ignored: they are being moved with it, as one shape.
pub(super) fn fits(
    instrument: &Instrument,
    notes: &[GridNote],
    candidate: &GridNote,
    moving: &[u32],
) -> bool {
    let Some(position) = candidate.target.position() else { return false };
    if candidate.target.with_placement(instrument, position).is_err() {
        return false;
    }
    notes
        .iter()
        .filter(|n| {
            n.id != candidate.id
                && !moving.contains(&n.id)
                && n.tick < candidate.tick.saturating_add(candidate.len)
                && candidate.tick < n.tick.saturating_add(n.len)
        })
        .all(|n| n.target.position().is_some_and(|p| instrument.can_sound_together(&position, &p)))
}
