// SPDX-License-Identifier: MIT

//! Transposing placed notes by semitones on the harp the chart is written
//! for. The music moves; the tab is recomputed — the same idea as
//! `harmonicon_core::harp_remap::HarpMapping::Transpose`, applied to a
//! pitch shift on one harp instead of the same pitch on another harp.
//!
//! Each note's sounding pitch (`playback::note_midi`) is shifted and
//! resolved back onto a hole, breath and technique through the editor's
//! `pitch_map`, the same resolver MIDI import and recording use. A note the
//! harp cannot sound at the new pitch, or whose new place is already taken
//! by a note that isn't moving, **stays exactly as it was** and is counted,
//! never silently dropped or nudged to a wrong pitch — the status bar then
//! says how many were left, alongside the mixed-breath and duplicate-hole
//! stacks the shift may have created (`midi_import::phrase_diagnostics`,
//! the same read import gives). Expression and per-note metadata ride
//! along on the note's id.

use bevy::prelude::*;

use harmonicon_core::harmonica::Harmonica;
use harmonicon_platform::localization::{Localization, LocalizationExt, LocalizedStr};

use super::midi_import::{ImportDiagnostics, phrase_diagnostics};
use super::pitch_map::playable_assignments;
use super::playback::note_midi;
use super::save_feedback::SaveFeedback;
use super::state::{EditorState, GridNote, HarmonicaKind, overlaps};

/// What one transposition did, for the status bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TransposeOutcome {
    /// By how many semitones.
    pub(super) semitones: i32,
    /// Notes that landed on a new hole/breath/technique.
    pub(super) moved: usize,
    /// Notes left where they were: the harp can't sound the new pitch, or
    /// the spot was taken.
    pub(super) kept: usize,
    /// Same-onset stacks that now need both breaths / share a hole.
    pub(super) diagnostics: ImportDiagnostics,
}

/// Transposes the notes whose ids are in `scope` (every note when `scope`
/// is empty) by `semitones` on `harp`, in place. Notes outside the scope
/// never move, and a moving note may not land on one. Pure over the note
/// list, so it is directly testable.
pub(super) fn transpose_notes(
    notes: &mut [GridNote],
    scope: &[u32],
    semitones: i32,
    harp: &Harmonica,
    _kind: HarmonicaKind,
) -> TransposeOutcome {
    let in_scope = |n: &GridNote| scope.is_empty() || scope.contains(&n.id);

    // Where each in-scope note wants to go, if the harp can take it there.
    let candidates: Vec<Vec<GridNote>> = notes
        .iter()
        .map(|n| {
            if !in_scope(n) {
                return Vec::new();
            }
            let Some(midi) = note_midi(n, harp) else {
                return Vec::new();
            };
            let Ok(target) = u8::try_from(i32::from(midi) + semitones) else {
                return Vec::new();
            };
            playable_assignments(target, harp)
                .into_iter()
                .map(|(hole, dir, pitch)| GridNote { hole, dir, pitch, ..*n })
                .collect()
        })
        .collect();
    let mut choices: Vec<Option<usize>> =
        candidates.iter().map(|options| (!options.is_empty()).then_some(0)).collect();

    // Settle collisions: a candidate that would overlap another note's
    // final place gives up and stays put. One revert per pass, then look
    // again — reverting a note can expose a collision for another, and
    // two candidates fighting over one hole should lose one, not both.
    // Each pass reverts exactly one candidate or is the last.
    loop {
        // When two moving notes collide, advance the one with more remaining
        // fingerings. This preserves a note with only one playable home and
        // lets an ambiguous pitch (such as G4 on a C harp) try its next hole.
        let colliding = (0..notes.len())
            .filter(|&i| {
                choices[i].is_some_and(|choice| {
                    let candidate = candidates[i][choice];
                    notes.iter().enumerate().any(|(j, original)| {
                        let other = choices[j].map_or(*original, |choice| candidates[j][choice]);
                        j != i && other.hole == candidate.hole && overlaps(&candidate, &other)
                    })
                })
            })
            .max_by_key(|&i| candidates[i].len() - choices[i].unwrap() - 1);
        match colliding {
            Some(i) => {
                let next = choices[i].unwrap() + 1;
                choices[i] = (next < candidates[i].len()).then_some(next);
            }
            None => break,
        }
    }

    let mut outcome = TransposeOutcome { semitones, ..Default::default() };
    for (i, n) in notes.iter_mut().enumerate() {
        if !in_scope(n) {
            continue;
        }
        match choices[i] {
            Some(choice) => {
                *n = candidates[i][choice];
                outcome.moved += 1;
            }
            None => outcome.kept += 1,
        }
    }
    outcome.diagnostics = phrase_diagnostics(notes, 0);
    outcome
}

/// Transposes the selection — or the whole chart when nothing is selected
/// — and leaves the outcome for the status bar to report.
pub(super) fn transpose_selection(state: &mut EditorState, semitones: i32) {
    let harp = state.effective_harp();
    let kind = state.harmonica_kind;
    let outcome = transpose_notes(&mut state.notes, &state.selected, semitones, &harp, kind);
    state.transpose_notice = Some(outcome);
}

/// The status-bar line for `outcome`: a plain "transposed N notes ±S"
/// when everything moved cleanly, otherwise the count left behind and any
/// stacks that now need both breaths or share a hole — the same
/// diagnostics MIDI import reports, because they're the same problem.
pub(super) fn outcome_message(outcome: &TransposeOutcome, loc: &Localization) -> LocalizedStr {
    let semitones = format!("{:+}", outcome.semitones);
    let clean = outcome.kept == 0
        && outcome.diagnostics.mixed_breath_groups == 0
        && outcome.diagnostics.duplicate_hole_groups == 0;
    if clean {
        loc.msg_args(
            "editor-transposed",
            &[("count", outcome.moved.to_string()), ("semitones", semitones)],
        )
    } else {
        loc.msg_args(
            "editor-transposed-warning",
            &[
                ("count", outcome.moved.to_string()),
                ("semitones", semitones),
                ("kept", outcome.kept.to_string()),
                ("mixed", outcome.diagnostics.mixed_breath_groups.to_string()),
                ("duplicate", outcome.diagnostics.duplicate_hole_groups.to_string()),
            ],
        )
    }
}

/// Puts a pending transposition outcome in the status bar.
pub(super) fn report_transpose(
    mut state: ResMut<EditorState>,
    loc: Res<Localization>,
    mut feedback: ResMut<SaveFeedback>,
) {
    if state.transpose_notice.is_none() {
        return;
    }
    let Some(outcome) = state.bypass_change_detection().transpose_notice.take() else {
        return;
    };
    feedback.set(outcome_message(&outcome, &loc));
}

#[cfg(test)]
mod tests;
