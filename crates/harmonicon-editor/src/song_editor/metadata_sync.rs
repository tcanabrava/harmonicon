// SPDX-License-Identifier: MIT

//! Keeps the two metadata stores that live *beside* the notes in step with
//! bulk edits to the notes themselves.
//!
//! `EditorState::phrase_annotations` (section/chord/groove/call/split) is
//! keyed by onset tick — it describes a *phrase*, the set of notes that
//! start together, not any one note. `EditorState::expression_intensities`
//! is keyed by note id. Neither is stored on `GridNote`, so a move, paste,
//! delete or range edit that only touches `notes` leaves them pointing at
//! ticks nothing starts on any more, or ids nothing has.
//!
//! Every bulk edit therefore goes through one of the methods here rather
//! than mutating `notes` directly. The rules they implement:
//!
//! - **An annotation moves with its phrase only when the whole phrase
//!   moves** — every note at that onset — and lands only where no phrase
//!   already is. Part of a phrase moving away leaves the label with the
//!   part that stayed; a phrase joining an existing one keeps the existing
//!   label.
//! - **Paste copies** an intensity to each pasted note's new id and an
//!   annotation to each pasted onset, never overwriting one already there.
//! - **Removing notes drops an annotation only once nothing starts at its
//!   tick**, and an intensity as soon as its note is gone. This is
//!   [`EditorState::drop_orphaned_metadata`], which
//!   `EditorState::prune_selection` runs, so every existing removal path
//!   (delete, a harmonica-kind switch, a recording take punching out what
//!   it overlaps, undo) gets it without a call of its own.
//! - **Closing a gap shifts** every annotation and timing-map point after it
//!   back by the gap's length, and drops those inside it. The tempo and meter
//!   active at the cut's end are restored at its start.

use std::collections::{BTreeMap, BTreeSet};

use super::clipboard::{NoteClipboard, paste_targets_with_sources};
use super::state::{EditorState, PhraseAnnotation};

fn close_timing_gap<T: Clone + PartialEq>(
    opening: &mut T,
    changes: &mut Vec<(usize, T)>,
    start: usize,
    end: usize,
) {
    if start >= end {
        return;
    }
    let active_after_cut = changes
        .iter()
        .filter(|(tick, _)| *tick <= end)
        .max_by_key(|(tick, _)| *tick)
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| opening.clone());
    let span = end - start;
    changes.retain(|(tick, _)| !(start..end).contains(tick));
    for (tick, _) in changes.iter_mut().filter(|(tick, _)| *tick >= end) {
        *tick -= span;
    }
    changes.sort_by_key(|(tick, _)| *tick);
    changes.dedup_by(|later, earlier| {
        if later.0 == earlier.0 {
            earlier.1 = later.1.clone();
            true
        } else {
            false
        }
    });

    if start == 0 {
        *opening = active_after_cut;
        changes.retain(|(tick, _)| *tick > 0);
        return;
    }
    let active_before_cut = changes
        .iter()
        .filter(|(tick, _)| *tick < start)
        .max_by_key(|(tick, _)| *tick)
        .map(|(_, value)| value)
        .unwrap_or(opening);
    if *active_before_cut != active_after_cut {
        changes.retain(|(tick, _)| *tick != start);
        changes.push((start, active_after_cut));
        changes.sort_by_key(|(tick, _)| *tick);
    } else {
        changes.retain(|(tick, _)| *tick != start);
    }
}

impl EditorState {
    /// Opens the phrase editor on the phrase at `tick` and selects every
    /// note that starts there — so the popover, a drag and Delete all act
    /// on the same thing. A tick nothing starts on is ignored.
    pub(super) fn open_phrase_editor(&mut self, tick: usize) {
        if !self.notes.iter().any(|n| n.tick == tick) {
            return;
        }
        self.selected.clear();
        self.selected.extend(self.notes.iter().filter(|n| n.tick == tick).map(|n| n.id));
        self.phrase_editor = Some(tick);
    }

    /// Drops annotations at ticks no note starts on and intensities for
    /// ids no note has. Idempotent; cheap enough to run after any removal.
    pub(super) fn drop_orphaned_metadata(&mut self) {
        let onsets: BTreeSet<usize> = self.notes.iter().map(|n| n.tick).collect();
        self.phrase_annotations.retain(|tick, _| onsets.contains(tick));
        let ids: BTreeSet<u32> = self.notes.iter().map(|n| n.id).collect();
        self.expression_intensities.retain(|id, _| ids.contains(id));
    }

    /// Moves each `(id, hole, tick)` note, carrying along the annotation of
    /// every onset the move empties out — see the module docs for when an
    /// annotation travels and when it stays.
    pub(super) fn move_notes(&mut self, targets: &[(u32, u8, usize)]) {
        // Which onsets the moved notes came from, and which ticks already
        // host a phrase *before* anything moves — both have to be read up
        // front, since the notes themselves are about to change.
        let moving: BTreeSet<u32> = targets.iter().map(|&(id, _, _)| id).collect();
        let vacated: BTreeMap<usize, usize> = targets
            .iter()
            .filter_map(|&(id, _, new_tick)| {
                let old_tick = self.notes.iter().find(|n| n.id == id)?.tick;
                let stays_occupied =
                    self.notes.iter().any(|n| n.tick == old_tick && !moving.contains(&n.id));
                (!stays_occupied).then_some((old_tick, new_tick))
            })
            .collect();
        let occupied_before: BTreeSet<usize> =
            self.notes.iter().filter(|n| !moving.contains(&n.id)).map(|n| n.tick).collect();

        for &(id, hole, tick) in targets {
            if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
                n.hole = hole;
                n.tick = tick;
            }
        }

        for (old_tick, new_tick) in vacated {
            let Some(annotation) = self.phrase_annotations.remove(&old_tick) else {
                continue;
            };
            if !occupied_before.contains(&new_tick) {
                self.phrase_annotations.entry(new_tick).or_insert(annotation);
            }
        }
        self.drop_orphaned_metadata();
    }

    /// Everything Ctrl+C needs to reproduce the selection elsewhere: the
    /// notes verbatim, plus each one's intensity and each onset's
    /// annotation, both keyed by their *source* id/tick so a paste can
    /// re-key them onto what actually lands.
    pub(super) fn copy_selection(&self) -> NoteClipboard {
        let notes: Vec<_> =
            self.notes.iter().filter(|n| self.selected.contains(&n.id)).copied().collect();
        let intensities = notes
            .iter()
            .filter_map(|n| self.expression_intensities.get(&n.id).map(|v| (n.id, v.clone())))
            .collect();
        let annotations = notes
            .iter()
            .filter_map(|n| self.phrase_annotations.get(&n.tick).map(|a| (n.tick, a.clone())))
            .collect();
        NoteClipboard { notes, intensities, annotations }
    }

    /// Pastes `clip` with its earliest note at `target_tick`, re-keying the
    /// copied metadata onto the notes that actually land (a note whose
    /// hole doesn't exist or whose spot is taken is skipped, and so is its
    /// metadata). The pasted notes become the selection. Returns whether
    /// anything landed.
    pub(super) fn paste(&mut self, clip: &NoteClipboard, target_tick: usize) -> bool {
        let hole_count = self.hole_count();
        let (placed, next_id) = paste_targets_with_sources(
            &clip.notes,
            target_tick,
            hole_count,
            &self.notes,
            self.next_id,
        );
        if placed.is_empty() {
            return false;
        }
        self.next_id = next_id;
        self.selected.clear();
        self.selected.extend(placed.iter().map(|(_, p)| p.id));
        for (source, placed) in &placed {
            if let Some(v) = clip.intensities.get(&source.id) {
                self.expression_intensities.insert(placed.id, v.clone());
            }
            if let Some(a) = clip.annotations.get(&source.tick) {
                self.phrase_annotations.entry(placed.tick).or_insert_with(|| a.clone());
            }
        }
        self.notes.extend(placed.into_iter().map(|(_, p)| p));
        true
    }

    /// The Erase tool: deletes every note overlapping `[start, end)`,
    /// nothing else moves.
    pub(super) fn erase_notes_in(&mut self, start: usize, end: usize) {
        self.notes = super::ranges::erase_range(&self.notes, start, end);
        self.selected.clear();
        self.prune_selection();
    }

    /// The Remove tool: deletes every note overlapping `[start, end)` and
    /// shifts everything after `end` earlier by the span, closing the gap
    /// — annotations included.
    pub(super) fn remove_range_closing_gap(&mut self, start: usize, end: usize) {
        let span = end.saturating_sub(start);
        self.notes = super::ranges::remove_range(&self.notes, start, end);
        let mut opening_tempo = self.tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
        close_timing_gap(&mut opening_tempo, &mut self.tempo_changes, start, end);
        if start == 0 && start < end {
            self.tempo = format!("{opening_tempo}");
        }
        close_timing_gap(&mut self.time_signature, &mut self.meter_changes, start, end);
        super::repeat_marks::close_gap(&mut self.repeats, start as u64, end as u64);
        let shifted: BTreeMap<usize, PhraseAnnotation> =
            std::mem::take(&mut self.phrase_annotations)
                .into_iter()
                .filter(|(tick, _)| !(start..end).contains(tick))
                .map(|(tick, a)| (if tick >= end { tick - span } else { tick }, a))
                .collect();
        self.phrase_annotations = shifted;
        self.selected.clear();
        self.prune_selection();
    }
}
