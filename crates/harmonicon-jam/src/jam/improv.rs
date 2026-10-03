// SPDX-License-Identifier: MIT

//! Improv-lesson scale-adherence accumulator: classifies each fresh note
//! attack during an open Jam Session against the bar it landed on
//! ([`NoteFit`]/[`classify_note_fit`], shared with `jam::session`'s live
//! hole-map tint so the two can never silently disagree) and the
//! phrase-discipline "did you leave space" pattern ([`in_rest_window`]).

use std::collections::HashSet;

use bevy::prelude::*;

use harmonicon_gameplay::gameplay::{AbsoluteBar, ActivePitches, CurrentBar};

use super::hole_map::JamHoleGuide;

/// How "targeted" a sounding note is, worst to best.
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub(crate) enum NoteFit {
    OutOfScale,
    InScale,
    ChordTone,
}

/// Classifies one played note class (e.g. `"G"`, no octave — see
/// `PitchInfo::note`) by how well it fits the harmonic context right now:
/// a tone of the bar's current chord is the most targeted choice, elsewhere
/// in the blues scale is still "safe," anything else is out. Shared by the
/// live hole-map tint (`hole_map::update_hole_map`) and the improv-lesson
/// accumulator ([`accumulate_improv_stats`]) so the two can never silently
/// disagree.
pub(crate) fn classify_note_fit(
    note: &str,
    chord_tones: &HashSet<String>,
    scale_classes: &HashSet<String>,
) -> NoteFit {
    if chord_tones.contains(note) {
        NoteFit::ChordTone
    } else if scale_classes.contains(note) {
        NoteFit::InScale
    } else {
        NoteFit::OutOfScale
    }
}

/// Enforces a fresh attack per pitch for [`accumulate_improv_stats`] — the
/// same fresh-attack idea `gameplay::PitchGate` uses for scored modes
/// (`harmonicon_core::scoring::AttackGate`), so holding one note doesn't tally it
/// again every frame it stays sounding.
#[derive(Resource, Default)]
pub struct ImprovGate(harmonicon_core::scoring::AttackGate<u8>);

/// Running tally of every fresh note attack played during an open Jam
/// Session, classified by [`NoteFit`] against the bar it landed on. Reset
/// at the start of every `Playing` session (`gameplay::lifecycle::
/// reset_score`) — not jam-only, so it's always in a known state, but only
/// [`accumulate_improv_stats`] (jam-only) ever writes to it. The improv
/// lesson's pass criterion (`lessons::PassCriteria::ScaleAdherence`) reads
/// [`adherence`](Self::adherence) when the player ends the session.
#[derive(Resource, Default, Clone, Copy)]
pub struct ImprovStats {
    pub chord_tone: u32,
    pub in_scale: u32,
    pub out_of_scale: u32,
    /// Fresh attacks that landed inside a "rest" window of the phrase-
    /// discipline pattern (see [`in_rest_window`]), tallied regardless of
    /// pitch/chord-tone classification, since phrase discipline judges
    /// *when* you played, not *what*.
    pub rest_violations: u32,
}

impl ImprovStats {
    pub fn total(&self) -> u32 {
        self.chord_tone + self.in_scale + self.out_of_scale
    }

    /// Fraction of attacks that were at least in-scale (a chord tone is the
    /// strictly better case within "in scale", so it counts too) — `None`
    /// with nothing played yet, same "nothing to report" convention as
    /// `gameplay::TechniqueStats::accuracy`.
    pub fn adherence(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some((self.chord_tone + self.in_scale) as f32 / total as f32)
        }
    }

    /// Fraction of attacks that were specifically chord tones — stricter
    /// than [`adherence`](Self::adherence), which also accepts merely-in-
    /// scale notes. The `chord-tone-improv` lesson's criterion.
    pub fn chord_tone_adherence(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 { None } else { Some(self.chord_tone as f32 / total as f32) }
    }

    /// Fraction of attacks that landed *outside* a rest window — "did you
    /// leave space", not what was played. The `question-answer` lesson's
    /// criterion.
    pub fn phrase_discipline(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 { None } else { Some(1.0 - (self.rest_violations as f32 / total as f32)) }
    }
}

/// Whether `bar_index` (an absolute, non-wrapped bar count — see
/// `gameplay::AbsoluteBar`) falls inside a "rest" window of a repeating
/// play/rest pattern: `play_bars` bars of playing, then `rest_bars` bars of
/// rest, repeating. The phrase-discipline lesson's "leave space" primitive —
/// pure so it's directly unit-testable. A zero-length cycle (both zero)
/// never counts as rest, since there's no pattern to violate.
pub(crate) fn in_rest_window(bar_index: usize, play_bars: usize, rest_bars: usize) -> bool {
    let cycle = play_bars + rest_bars;
    if cycle == 0 {
        return false;
    }
    bar_index % cycle >= play_bars
}

/// The phrase-discipline pattern every jam session measures against: 2 bars
/// of playing, then 2 bars of rest — the "question and answer" phrasing
/// discipline the lesson teaches (see `docs/lessons_plan.md`, engine item
/// 3). Always-on, like every other `ImprovStats` tally, not gated on a
/// lesson being in flight.
const PHRASE_PLAY_BARS: usize = 2;
const PHRASE_REST_BARS: usize = 2;

/// Clears the running tallies at the start of every `Playing` session.
///
/// Registered by `jam::JamPlugin` on `OnEnter(AppState::Playing)` — the
/// same point `gameplay::lifecycle::reset_score` clears the scored-mode
/// counters. Jam resets its own rather than being reset from there, so
/// that `gameplay` needn't depend on `jam` (`docs/physical_design_plan.md`
/// rule 2); like `reset_score` it runs for every mode, not just jam, so
/// the tallies are always in a known state.
pub fn reset_improv_stats(mut gate: ResMut<ImprovGate>, mut stats: ResMut<ImprovStats>) {
    *gate = ImprovGate::default();
    *stats = ImprovStats::default();
}

/// Tallies each fresh note attack into [`ImprovStats`], classified by
/// [`classify_note_fit`] against the bar it landed on — the live twin of
/// `hole_map::update_hole_map`'s per-frame tint, but counting discrete
/// attacks once each instead of repainting every frame a pitch stays held.
pub fn accumulate_improv_stats(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    current: Res<CurrentBar>,
    absolute: Res<AbsoluteBar>,
    mut gate: ResMut<ImprovGate>,
    mut stats: ResMut<ImprovStats>,
) {
    let Some(guide) = guide else {
        return;
    };
    let mut sounding = [false; 256];
    for pitch in &active.0 {
        if guide.note_to_holes.contains_key(&pitch.midi) {
            sounding[pitch.midi as usize] = true;
        }
    }
    gate.0.release_absent(|m| sounding[m as usize]);

    let chord_tones = &guide.chord_tones_by_bar[current.0];
    let resting = in_rest_window(absolute.0, PHRASE_PLAY_BARS, PHRASE_REST_BARS);
    for p in &active.0 {
        if !guide.note_to_holes.contains_key(&p.midi) || !gate.0.is_fresh(p.midi, true) {
            continue;
        }
        gate.0.consume(p.midi);
        match classify_note_fit(&p.note, chord_tones, &guide.scale_classes) {
            NoteFit::ChordTone => stats.chord_tone += 1,
            NoteFit::InScale => stats.in_scale += 1,
            NoteFit::OutOfScale => stats.out_of_scale += 1,
        }
        if resting {
            stats.rest_violations += 1;
        }
    }
}

#[cfg(test)]
mod tests;
