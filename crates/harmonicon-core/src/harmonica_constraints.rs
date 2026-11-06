// SPDX-License-Identifier: MIT

//! Post-detection harmonica-constraint filtering — the "Harmonica
//! Constraint Solver" stage from `Harmonicon Note Detection Roadmap.md`
//! (repo root): a harmonica's reed plate only responds to
//! one wind direction at a time, so a raw detector's simultaneous
//! candidate pitch set (e.g. `PitchAlgorithm::Nmf`'s activations) can
//! still contain a physically impossible mix — a pitch only reachable by
//! blowing alongside one only reachable by drawing, which no single
//! breath can produce. This module is pure post-processing over a
//! `Harmonica` plus a candidate MIDI pitch set; it doesn't know or care
//! which detector produced the candidates, so it composes with any of
//! them (most usefully the polyphonic ones).
//!
//! This stays in the Bevy-free core beside [`Harmonica`]. The DSP and audio
//! crates remain instrument-agnostic and publish raw candidates;
//! [`crate::note_tracker`] applies [`BreathDirectionTracker`] to them as the
//! harmonica's voice filter.

use crate::harmonica::{Harmonica, hole_notes};
use crate::midi::note_to_midi;

fn to_midi_u8(note: &str) -> Option<u8> {
    u8::try_from(note_to_midi(note)?).ok()
}

/// Whether `harp` can produce `midi` via a blow-family technique (the
/// natural blow note, or an overblow on holes 1/4/5/6) and/or a
/// draw-family technique (the natural draw note, a bend, or an overdraw on
/// holes 7-10). A pitch reachable both ways (e.g. a bend landing on the
/// same semitone an adjacent hole's natural note already covers — ordinary
/// on a Richter-tuned diatonic) sets both flags.
///
/// Which family a bend/overblow/overdraw falls into follows
/// [`hole_notes`]'s own derivation: on holes 1-6 a bend pulls the *draw*
/// reed down (draw-family) and an overblow is produced by blowing
/// (blow-family, even though its pitch sits a semitone above the draw
/// reed); on holes 7-10 a bend pushes the *blow* reed down (blow-family)
/// and an overdraw is produced by drawing (draw-family).
pub fn reachable_directions(harp: &Harmonica, midi: u8) -> (bool, bool) {
    let mut directions = (false, false);
    visit_directions(harp, |note, blow| {
        if note == midi {
            directions.0 |= blow;
            directions.1 |= !blow;
        }
    });
    directions
}

/// Visit each pitch with its breath family; repeated pitches may have both.
fn visit_directions(harp: &Harmonica, mut visit: impl FnMut(u8, bool)) {
    let mut mark = |note: Option<&str>, blow| {
        if let Some(midi) = note.and_then(to_midi_u8) {
            visit(midi, blow);
        }
    };
    for hole in 1..=harp.hole_count() {
        let notes = hole_notes(harp, hole);
        mark(notes.blow.as_deref(), true);
        mark(notes.draw.as_deref(), false);
        for bend in &notes.bends {
            mark(Some(bend), hole > 6);
        }
        mark(notes.over.as_deref(), matches!(hole, 1 | 4 | 5 | 6));
    }
}

/// Derive all MIDI breath families in a single pass over the harp's holes.
pub(crate) fn direction_table<const N: usize>(harp: &Harmonica) -> [(bool, bool); N] {
    let mut table = [(false, false); N];
    visit_directions(harp, |midi, blow| {
        if let Some(entry) = table.get_mut(usize::from(midi)) {
            entry.0 |= blow;
            entry.1 |= !blow;
        }
    });
    table
}

/// A pitch's `(blow, draw)` reachability from a [`direction_table`]; a
/// pitch outside the MIDI range is reachable neither way.
pub(crate) fn table_directions(table: &[(bool, bool); 128], midi: u8) -> (bool, bool) {
    table.get(usize::from(midi)).copied().unwrap_or((false, false))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathDirection {
    Blow,
    Draw,
}

/// Stateful breath-direction inference for successive detector frames.
///
/// Candidate order is significant: the pitch detectors return strongest-first,
/// so it is a useful confidence proxy until `PitchInfo` carries an explicit
/// strength. By default, a direction change needs two consecutive frames of opposing
/// evidence; this prevents one noisy FFT frame from turning a held blow chord
/// into an impossible draw/blow flicker.
#[derive(Debug)]
pub struct BreathDirectionTracker {
    current: Option<BreathDirection>,
    pending: Option<BreathDirection>,
    pending_frames: u8,
    silent_frames: u8,
    change_frames: u8,
}

impl Default for BreathDirectionTracker {
    fn default() -> Self {
        Self { current: None, pending: None, pending_frames: 0, silent_frames: 0, change_frames: 2 }
    }
}

impl BreathDirectionTracker {
    pub fn with_change_frames(change_frames: u8) -> Self {
        Self { change_frames: change_frames.max(1), ..Self::default() }
    }

    pub fn reset(&mut self) {
        let change_frames = self.change_frames;
        *self = Self::with_change_frames(change_frames);
    }

    pub fn filter(&mut self, harp: &Harmonica, candidates: &[u8]) -> Vec<u8> {
        if candidates.is_empty() {
            return self.filter_by(|_| (false, false), candidates);
        }
        let table: [_; 256] = direction_table(harp);
        self.filter_by(|midi| table[usize::from(midi)], candidates)
    }

    /// [`filter`](Self::filter) with the reachability question supplied, so
    /// a caller holding a precomputed table needn't re-derive the harp.
    pub(crate) fn filter_by(
        &mut self,
        reach: impl Fn(u8) -> (bool, bool),
        candidates: &[u8],
    ) -> Vec<u8> {
        if candidates.is_empty() {
            self.silent_frames += 1;
            self.pending = None;
            self.pending_frames = 0;
            if self.silent_frames >= 2 {
                self.reset();
            }
            return Vec::new();
        }
        self.silent_frames = 0;

        let evidence = candidates.iter().find_map(|&midi| match reach(midi) {
            (true, false) => Some(BreathDirection::Blow),
            (false, true) => Some(BreathDirection::Draw),
            _ => None,
        });

        match (self.current, evidence) {
            (None, Some(direction)) => self.current = Some(direction),
            (Some(current), Some(direction)) if current != direction => {
                if self.pending == Some(direction) {
                    self.pending_frames += 1;
                } else {
                    self.pending = Some(direction);
                    self.pending_frames = 1;
                }
                if self.pending_frames >= self.change_frames {
                    self.current = Some(direction);
                    self.pending = None;
                    self.pending_frames = 0;
                }
            }
            (_, Some(_)) => {
                self.pending = None;
                self.pending_frames = 0;
            }
            _ => {}
        }

        let mut kept: Vec<u8> = candidates
            .iter()
            .copied()
            .filter(|&midi| {
                let (blow, draw) = reach(midi);
                match self.current {
                    Some(BreathDirection::Blow) => blow,
                    Some(BreathDirection::Draw) => draw,
                    None => blow || draw,
                }
            })
            .collect();
        kept.sort_unstable();
        kept.dedup();
        kept
    }
}

/// Filters `candidates` (a raw detector's simultaneous MIDI pitch guesses)
/// down to a physically plausible subset for `harp`:
///
/// 1. Drop any pitch `harp` can't produce at all, by any technique — noise
///    or a detector artifact, not a real harmonica note.
/// 2. Of what's left, keep only the pitches reachable under a single wind
///    direction — all-blow-family or all-draw-family, whichever explains
///    more of the remaining candidates — since a player can't blow and
///    draw at the same instant. Ties (equally many either way) keep blow,
///    an arbitrary but deterministic choice.
///
/// Returns a sorted, deduplicated `Vec<u8>` — empty if `candidates` is
/// empty or none of them are producible on `harp` at all. A chord/octave
/// (several notes sharing one wind direction) passes through untouched;
/// only a mix that needs both directions at once loses its minority side.
pub fn plausible_notes(harp: &Harmonica, candidates: &[u8]) -> Vec<u8> {
    let reach: Vec<(u8, bool, bool)> = candidates
        .iter()
        .map(|&midi| {
            let (blow, draw) = reachable_directions(harp, midi);
            (midi, blow, draw)
        })
        .filter(|&(_, blow, draw)| blow || draw)
        .collect();

    let blow_count = reach.iter().filter(|&&(_, blow, _)| blow).count();
    let draw_count = reach.iter().filter(|&&(_, _, draw)| draw).count();
    let keep_blow = draw_count <= blow_count;

    let mut kept: Vec<u8> = reach
        .into_iter()
        .filter(|&(_, blow, draw)| if keep_blow { blow } else { draw })
        .map(|(midi, _, _)| midi)
        .collect();
    kept.sort_unstable();
    kept.dedup();
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonica::richter_harp;

    #[test]
    fn the_direction_table_agrees_with_reachable_directions_everywhere() {
        use crate::harmonica::{
            chromatic_16_harp, chromatic_harp, country_tuned_harp, natural_minor_harp,
            paddy_richter_harp,
        };
        let harps = [
            richter_harp("C"),
            richter_harp("G"),
            richter_harp("F#"),
            country_tuned_harp("A"),
            paddy_richter_harp("D"),
            natural_minor_harp("E"),
            chromatic_harp("C"),
            chromatic_16_harp("C"),
        ];
        for harp in &harps {
            let table = direction_table(harp);
            for midi in 0..=u8::MAX {
                assert_eq!(
                    table_directions(&table, midi),
                    reachable_directions(harp, midi),
                    "MIDI {midi} on {:?}",
                    harp.summary().profile
                );
            }
        }
    }

    // Richter C harp reference (see harmonicon_core::harmonica::{C_BLOW, C_DRAW}):
    // hole 1: blow C4=60, draw D4=62, bend C#4=61 (draw-family), overblow D#4=63 (blow-family)
    // hole 2: blow E4=64, draw G4=67
    // hole 3: blow G4=67 (same pitch as hole 2's draw — an ordinary Richter overlap)

    #[test]
    fn drops_the_minority_wind_direction() {
        let harp = richter_harp("C");
        // Two blow notes (60, 64) outnumber one draw note (62).
        let kept = plausible_notes(&harp, &[60, 64, 62]);
        assert_eq!(kept, vec![60, 64]);
    }

    #[test]
    fn an_overblow_counts_as_blow_family() {
        let harp = richter_harp("C");
        // Overblow (63) + natural blow (60) outvote the natural draw (62),
        // even though a naive "action per hole" reading might expect 62 to
        // survive as "the" hole-1 note.
        let kept = plausible_notes(&harp, &[63, 60, 62]);
        assert_eq!(kept, vec![60, 63]);
    }

    #[test]
    fn a_bend_counts_as_draw_family_on_low_holes() {
        let harp = richter_harp("C");
        // Bend (61) + natural draw (62) outvote the natural blow (60).
        let kept = plausible_notes(&harp, &[61, 62, 60]);
        assert_eq!(kept, vec![61, 62]);
    }

    #[test]
    fn an_unproducible_pitch_is_dropped_regardless() {
        let harp = richter_harp("C");
        // 40 (E2) is far below anything this harp can produce.
        let kept = plausible_notes(&harp, &[60, 40]);
        assert_eq!(kept, vec![60]);
    }

    #[test]
    fn a_pure_blow_chord_survives_untouched() {
        let harp = richter_harp("C");
        // Holes 1-3 blow together (a classic "train" chord) — 67 is
        // ambiguous (also hole 2's draw note) but blow still wins 3-to-1.
        let mut kept = plausible_notes(&harp, &[60, 64, 67]);
        kept.sort_unstable();
        assert_eq!(kept, vec![60, 64, 67]);
    }

    #[test]
    fn a_tie_keeps_blow() {
        let harp = richter_harp("C");
        let kept = plausible_notes(&harp, &[60, 62]);
        assert_eq!(kept, vec![60]);
    }

    #[test]
    fn empty_candidates_yield_empty_output() {
        let harp = richter_harp("C");
        assert!(plausible_notes(&harp, &[]).is_empty());
    }

    #[test]
    fn tracker_resists_a_one_frame_direction_flip() {
        let harp = richter_harp("C");
        let mut tracker = BreathDirectionTracker::default();
        assert_eq!(tracker.filter(&harp, &[60, 64]), vec![60, 64]);
        assert!(tracker.filter(&harp, &[62]).is_empty());
        assert_eq!(tracker.filter(&harp, &[60, 64]), vec![60, 64]);
    }

    #[test]
    fn a_phantom_that_keeps_leading_holds_the_wrong_direction_indefinitely() {
        // Characterisation of a known weakness, not an endorsement of it.
        //
        // Direction is inferred from the first candidate the harp can only
        // sound one way, on the assumption that detectors return
        // strongest-first. So a blow-only phantom ranked above a real draw
        // chord suppresses the entire chord — and goes on suppressing it for
        // as long as it keeps leading, because it supplies the same evidence
        // every frame and the hysteresis below only resists evidence that
        // *changes*. The two-frame rule defends against a phantom that
        // flickers, not one that persists.
        //
        // Replacing this with summed detector strength needs `PitchInfo` to
        // carry a strength at all, and needs recordings to tune against —
        // see `docs/pitch_detection_plan.md`.
        let harp = richter_harp("C");
        let mut tracker = BreathDirectionTracker::default();
        // C4 is blow-only; D4 and B4 are draw-only. All three "sound" at once.
        for _ in 0..10 {
            assert_eq!(tracker.filter(&harp, &[60, 62, 71]), vec![60]);
        }
        // Recovery takes the ordinary two frames once the phantom stops
        // outranking the real notes.
        assert!(tracker.filter(&harp, &[62, 71]).is_empty());
        assert_eq!(tracker.filter(&harp, &[62, 71]), vec![62, 71]);
    }

    #[test]
    fn tracker_changes_direction_after_two_frames() {
        let harp = richter_harp("C");
        let mut tracker = BreathDirectionTracker::default();
        tracker.filter(&harp, &[60]);
        assert!(tracker.filter(&harp, &[62]).is_empty());
        assert_eq!(tracker.filter(&harp, &[62]), vec![62]);
    }
}
