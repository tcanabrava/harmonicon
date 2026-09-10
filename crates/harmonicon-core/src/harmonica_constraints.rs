// SPDX-License-Identifier: MIT

//! Post-detection harmonica-constraint filtering — the "Harmonica
//! Constraint Solver" stage from `Harmonica Note Detection Roadmap.md`
//! (repo root, not checked in): a harmonica's reed plate only responds to
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
//! crates remain instrument-agnostic and publish raw candidates; gameplay,
//! Song Editor recording, and the offline benchmark supply the selected harp
//! and share the state tracker below.

use std::collections::{HashMap, HashSet};

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
    let mut blow = false;
    let mut draw = false;
    for hole in 1..=harp.hole_count() {
        let notes = hole_notes(harp, hole);
        if notes.blow.as_deref().and_then(to_midi_u8) == Some(midi) {
            blow = true;
        }
        if notes.draw.as_deref().and_then(to_midi_u8) == Some(midi) {
            draw = true;
        }
        if notes.bends.iter().any(|b| to_midi_u8(b) == Some(midi)) {
            if hole <= 6 {
                draw = true;
            } else {
                blow = true;
            }
        }
        if notes.over.as_deref().and_then(to_midi_u8) == Some(midi) {
            if matches!(hole, 1 | 4 | 5 | 6) {
                blow = true;
            } else {
                draw = true;
            }
        }
    }
    (blow, draw)
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
        Self {
            current: None,
            pending: None,
            pending_frames: 0,
            silent_frames: 0,
            change_frames: 2,
        }
    }
}

impl BreathDirectionTracker {
    pub fn with_change_frames(change_frames: u8) -> Self {
        Self {
            change_frames: change_frames.max(1),
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        let change_frames = self.change_frames;
        *self = Self::with_change_frames(change_frames);
    }

    pub fn filter(&mut self, harp: &Harmonica, candidates: &[u8]) -> Vec<u8> {
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

        let evidence = candidates
            .iter()
            .find_map(|&midi| match reachable_directions(harp, midi) {
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
                let (blow, draw) = reachable_directions(harp, midi);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteTrackerConfig {
    /// Consecutive frames a pitch must appear in before it counts. Costs
    /// exactly `onset_frames - 1` hops of latency on every note, which
    /// gameplay compensates for out of the judged clock.
    pub onset_frames: u8,
    /// Frames a confirmed pitch survives without being detected. `1` is no
    /// grace at all — the pitch is released the first frame it is missing.
    pub release_frames: u8,
    pub direction_change_frames: u8,
}

impl Default for NoteTrackerConfig {
    fn default() -> Self {
        Self {
            // Two frames is what rejects a one-frame phantom; see
            // `tracker_resists_a_one_frame_direction_flip`.
            onset_frames: 2,
            // **No release grace.** A grace frame bridges a detector that
            // drops a frame mid-sustain, which stops one held breath from
            // re-arming `AttackGate` and satisfying a second note. It cannot
            // be told apart from a real re-articulation, though — both look
            // like one silent frame — and at the shipped hop size a frame is
            // ~46 ms, so a grace of 2 swallows the gap between chugged
            // eighth notes on one hole. That figure is the backbone of blues
            // harmonica; a detector dropping frames mid-sustain is so far
            // only hypothetical. Prefer the measured cost over the assumed
            // one, and revisit against the recorded corpus
            // (`docs/pitch_detection_plan.md`) rather than by taste.
            release_frames: 1,
            direction_change_frames: 2,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackedNotes {
    pub active: Vec<u8>,
    /// Pitches confirmed on *this* frame — the tracker's onset events.
    ///
    /// Every one of them is exactly [`NoteTrackerConfig::onset_frames`]` - 1`
    /// hops later than the sound that produced it, because a pitch is
    /// confirmed the instant its run of consecutive frames reaches that
    /// threshold and never after. So the lag a consumer has to correct for is
    /// one constant for the whole stream, not a per-pitch quantity — see
    /// `a_confirmation_is_always_the_same_number_of_frames_late`.
    pub confirmed: Vec<u8>,
}

/// Pure harmonica-state tracker shared by gameplay, recording, and benchmarks.
pub struct HarmonicaNoteTracker {
    harp: Harmonica,
    config: NoteTrackerConfig,
    direction: BreathDirectionTracker,
    pending: HashMap<u8, u8>,
    active: HashMap<u8, u8>,
}

impl HarmonicaNoteTracker {
    pub fn new(harp: Harmonica, config: NoteTrackerConfig) -> Self {
        Self {
            harp,
            config: NoteTrackerConfig {
                onset_frames: config.onset_frames.max(1),
                release_frames: config.release_frames.max(1),
                direction_change_frames: config.direction_change_frames.max(1),
            },
            direction: BreathDirectionTracker::with_change_frames(config.direction_change_frames),
            pending: HashMap::new(),
            active: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        self.direction.reset();
        self.pending.clear();
        self.active.clear();
    }

    /// Consecutive frames a pitch must be seen in before it is reported.
    /// Consumers need this to convert the tracker's fixed onset lag into
    /// real time — it depends on their own hop size, which is not this
    /// crate's business.
    pub fn onset_frames(&self) -> u8 {
        self.config.onset_frames
    }

    pub fn update(&mut self, candidates: &[u8]) -> TrackedNotes {
        // Keep the direction tracker's public two-frame default compatible,
        // while permitting consumers to request a different transition count.
        let allowed = self.direction.filter(&self.harp, candidates);
        let allowed_set: HashSet<u8> = allowed.iter().copied().collect();
        self.pending.retain(|midi, _| allowed_set.contains(midi));

        let mut confirmed = Vec::new();
        for midi in allowed {
            if let Some(missed) = self.active.get_mut(&midi) {
                *missed = 0;
                continue;
            }
            let seen = self.pending.entry(midi).or_default();
            *seen += 1;
            if *seen >= self.config.onset_frames {
                confirmed.push(midi);
                self.active.insert(midi, 0);
            }
        }
        self.pending
            .retain(|midi, _| !self.active.contains_key(midi));
        self.active.retain(|midi, missed| {
            if allowed_set.contains(midi) {
                true
            } else {
                *missed += 1;
                *missed < self.config.release_frames
            }
        });

        let mut active: Vec<u8> = self.active.keys().copied().collect();
        active.sort_unstable();
        confirmed.sort_unstable();
        TrackedNotes { active, confirmed }
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

    // Richter C harp reference (see song::harmonica::{C_BLOW, C_DRAW}):
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

    #[test]
    fn note_tracker_preserves_a_pitch_reachable_in_both_directions() {
        let harp = richter_harp("C");
        let mut tracker = HarmonicaNoteTracker::new(
            harp,
            NoteTrackerConfig {
                onset_frames: 1,
                ..NoteTrackerConfig::default()
            },
        );
        // G4: hole 2 draw and hole 3 blow.
        assert_eq!(tracker.update(&[67]).active, vec![67]);
    }

    #[test]
    fn a_re_articulation_after_one_silent_frame_is_a_second_attack() {
        // Repeated notes on one hole are the basic rhythmic figure of blues
        // harmonica, and the gap between two of them is short: at the
        // shipped hop size a detector frame is ~46 ms, so chugged eighths
        // leave one, sometimes two, silent frames. The tracker has to
        // report the pitch as gone in that gap — `AttackGate` re-arms on
        // absence, so a pitch that never goes absent can only ever satisfy
        // one note, and the second chug scores nothing.
        let harp = richter_harp("C");
        let mut tracker = HarmonicaNoteTracker::new(harp, NoteTrackerConfig::default());
        tracker.update(&[60]);
        assert_eq!(tracker.update(&[60]).confirmed, vec![60]);
        assert!(
            tracker.update(&[]).active.is_empty(),
            "one silent frame must read as a release, or two chugged notes \
             merge into a single sustain"
        );
        tracker.update(&[60]);
        assert_eq!(
            tracker.update(&[60]).confirmed,
            vec![60],
            "the re-attack must confirm again"
        );
    }

    #[test]
    fn a_longer_release_grace_bridges_a_dropout_when_asked_for() {
        // The knob still works, and is what a detector measured to drop
        // frames mid-sustain would want — see `docs/pitch_detection_plan.md`.
        let harp = richter_harp("C");
        let mut tracker = HarmonicaNoteTracker::new(
            harp,
            NoteTrackerConfig {
                release_frames: 2,
                ..NoteTrackerConfig::default()
            },
        );
        tracker.update(&[60]);
        tracker.update(&[60]);
        assert_eq!(tracker.update(&[]).active, vec![60]);
        assert!(tracker.update(&[]).active.is_empty());
    }

    #[test]
    fn a_confirmation_is_always_the_same_number_of_frames_late() {
        // The property the whole latency correction rests on: whatever the
        // onset threshold, a pitch is confirmed on exactly the frame its run
        // reaches it — never earlier, never later, however long it goes on
        // sounding afterwards. So the lag is one constant for the stream, and
        // a consumer can subtract it from its clock once rather than tracking
        // a per-pitch age.
        let harp = richter_harp("C");
        for onset_frames in 1..=4u8 {
            let mut tracker = HarmonicaNoteTracker::new(
                harp.clone(),
                NoteTrackerConfig {
                    onset_frames,
                    ..NoteTrackerConfig::default()
                },
            );
            let confirmed_on: Vec<usize> = (0..8)
                .filter(|_| !tracker.update(&[60]).confirmed.is_empty())
                .collect();
            assert_eq!(
                confirmed_on,
                vec![onset_frames as usize - 1],
                "onset_frames = {onset_frames} should confirm once, on frame \
                 {}, and never again while the pitch keeps sounding",
                onset_frames - 1
            );
        }
    }
}
