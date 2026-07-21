// SPDX-License-Identifier: MIT

//! Pure tick-range functions over a song's notes — the Select/Erase/Remove
//! timeline tools (`timeline.rs`) and the silence track (`grid.rs`) both
//! read spans of the note list rather than mutating single notes. Split out
//! of `state.rs` to stay under the file-size budget
//! (`docs/physical_design_plan.md`); these functions don't touch
//! `EditorState` itself, just `&[GridNote]`.

use super::state::{GridNote, Side};

/// One past the last tick any note currently occupies — the right-hand
/// bound for a "from the split point to the end of the song" range. `0` for
/// an empty song.
pub(super) fn song_end_tick(notes: &[GridNote]) -> usize {
    notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0)
}

/// Orders a possibly-backwards drag span into `(start, end)` with
/// `start <= end`.
pub(super) fn normalize_range(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

/// The tick ranges strictly *between* two sounding notes — merging every
/// note's `[tick, tick+len)` interval across all holes first, since silence
/// means nothing at all is sounding, not just one particular hole (two holes
/// overlapping as a chord, or one note's tail overlapping the next note's
/// onset, must not read as a gap). Leading silence (before the first note)
/// and trailing silence (after the last) are deliberately excluded — the
/// silence track shows the space *between* notes, not lead-in/lead-out.
pub(super) fn silence_gaps(notes: &[GridNote]) -> Vec<(usize, usize)> {
    let mut intervals: Vec<(usize, usize)> =
        notes.iter().map(|n| (n.tick, n.tick + n.len)).collect();
    intervals.sort_by_key(|&(start, _)| start);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in intervals {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged.windows(2).map(|w| (w[0].1, w[1].0)).collect()
}

/// The whole-side range a split point resolves to once the user clicks the
/// highlighted side: from the start of the song up to `split` (`Side::Left`,
/// the pointer was hovering left of the split), or from `split` to the end
/// of the song (`Side::Right`).
pub(super) fn split_side_range(split: usize, side: Side, notes: &[GridNote]) -> (usize, usize) {
    match side {
        Side::Left => (0, split),
        Side::Right => (split, song_end_tick(notes).max(split)),
    }
}

/// Whether a note spanning `[tick, tick+len)` overlaps `[start, end)`.
fn range_overlaps(tick: usize, len: usize, start: usize, end: usize) -> bool {
    tick < end && start < tick + len
}

/// Deletes every note overlapping `[start, end)`, leaving every other note
/// exactly where it is — the song's own length is unaffected, just a gap
/// where those notes used to be. The **Erase** tool.
pub(super) fn erase_range(notes: &[GridNote], start: usize, end: usize) -> Vec<GridNote> {
    notes
        .iter()
        .copied()
        .filter(|n| !range_overlaps(n.tick, n.len, start, end))
        .collect()
}

/// Deletes every note overlapping `[start, end)`, *and* shifts every note
/// that starts at or after `end` earlier by `end - start` ticks, closing the
/// gap — the song gets shorter. The **Remove** tool.
pub(super) fn remove_range(notes: &[GridNote], start: usize, end: usize) -> Vec<GridNote> {
    let span = end.saturating_sub(start);
    notes
        .iter()
        .copied()
        .filter(|n| !range_overlaps(n.tick, n.len, start, end))
        .map(|mut n| {
            if n.tick >= end {
                n.tick -= span;
            }
            n
        })
        .collect()
}
