// SPDX-License-Identifier: MIT

//! Turning a detector's per-frame pitch guesses into notes: which pitches
//! are sounding, and which just started.
//!
//! Two stages. First the instrument's [`VoiceFilter`] drops what it can't
//! sound alongside the rest (on a harmonica, a pitch the current breath
//! can't produce). Then onset and release counting confirms a pitch only
//! after it has been seen for a few frames and releases it once it goes
//! missing. The second stage knows nothing about instruments; the first is
//! chosen from the [`Instrument`] the tracker is built for.
//!
//! Shared by gameplay, Song Editor recording and the offline benchmark, so
//! all three hear the same notes from the same audio.

use std::collections::HashMap;

use crate::harmonica_constraints::{BreathDirectionTracker, direction_table, table_directions};
use crate::instrument::{Instrument, InstrumentModel};

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

/// What an instrument can sound at once, applied to one detector frame.
pub enum VoiceFilter {
    /// A harmonica: one breath direction at a time, held against a one-frame
    /// flicker by [`BreathDirectionTracker`].
    Breath {
        /// Which breath reaches each pitch, computed once: the harp never
        /// changes for a tracker's lifetime, and this is asked several
        /// times per detector hop. Boxed so the other filters don't carry
        /// its size.
        reach: Box<[(bool, bool); 128]>,
        direction: BreathDirectionTracker,
    },
    /// Strings: any pitch the neck reaches, bent or not, at most one per
    /// string. Candidates arrive strongest first, so the strongest survive.
    /// Whether a set can actually be fretted at once is left to a recorded
    /// corpus to justify rather than guessed here.
    Strings { pitches: std::collections::HashSet<u8>, strings: usize },
}

impl VoiceFilter {
    pub fn for_instrument(instrument: &Instrument, config: &NoteTrackerConfig) -> Self {
        match instrument {
            Instrument::Harmonica(harp) => Self::Breath {
                reach: Box::new(direction_table(harp)),
                direction: BreathDirectionTracker::with_change_frames(
                    config.direction_change_frames,
                ),
            },
            Instrument::Fretted(guitar) => {
                Self::Strings { pitches: guitar.valid_pitches(), strings: guitar.strings.len() }
            }
            // One voice: the same rule as a one-string guitar, the
            // strongest pitch the flute can play.
            Instrument::Flute(flute) => {
                Self::Strings { pitches: InstrumentModel::valid_pitches(flute), strings: 1 }
            }
            // Ten fingers: any keys at once, the ten strongest heard.
            Instrument::Piano(piano) => {
                Self::Strings { pitches: InstrumentModel::valid_pitches(piano), strings: 10 }
            }
        }
    }

    /// `candidates` cut down to what the instrument can sound together,
    /// sorted and deduplicated.
    pub fn allowed(&mut self, candidates: &[u8]) -> Vec<u8> {
        match self {
            Self::Breath { reach, direction } => {
                direction.filter_by(|midi| table_directions(reach, midi), candidates)
            }
            Self::Strings { pitches, strings } => {
                let mut kept: Vec<u8> = Vec::with_capacity(*strings);
                for &midi in candidates {
                    if kept.len() == *strings {
                        break;
                    }
                    if pitches.contains(&midi) && !kept.contains(&midi) {
                        kept.push(midi);
                    }
                }
                kept.sort_unstable();
                kept
            }
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Breath { direction, .. } => direction.reset(),
            // Stateless: each frame is judged on its own.
            Self::Strings { .. } => {}
        }
    }
}

/// Note tracking for one instrument, shared by gameplay, recording, and
/// benchmarks.
pub struct NoteTracker {
    voices: VoiceFilter,
    onset_frames: u8,
    release_frames: u8,
    pending: HashMap<u8, u8>,
    active: HashMap<u8, u8>,
}

impl NoteTracker {
    pub fn new(instrument: &Instrument, config: NoteTrackerConfig) -> Self {
        Self {
            // Every threshold is a count of frames to wait, so zero would
            // mean "decide before observing anything" — floor them all at
            // one. `direction_change_frames` is floored by the voice filter
            // that uses it rather than here, so each knob has exactly one
            // owner applying its floor and one place storing it.
            voices: VoiceFilter::for_instrument(instrument, &config),
            onset_frames: config.onset_frames.max(1),
            release_frames: config.release_frames.max(1),
            pending: HashMap::new(),
            active: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        self.voices.reset();
        self.pending.clear();
        self.active.clear();
    }

    /// Consecutive frames a pitch must be seen in before it is reported.
    /// Consumers need this to convert the tracker's fixed onset lag into
    /// real time — it depends on their own hop size, which is not this
    /// crate's business.
    pub fn onset_frames(&self) -> u8 {
        self.onset_frames
    }

    pub fn update(&mut self, candidates: &[u8]) -> TrackedNotes {
        // What the instrument can sound together first: a pitch it cannot
        // play alongside the others (on a harmonica, one the current breath
        // cannot produce) is not a candidate for onset counting at all.
        let allowed = self.voices.allowed(candidates);
        // `allowed` is sorted and deduplicated, so membership is a binary
        // search — no per-hop set.
        let is_allowed = |midi: &u8| allowed.binary_search(midi).is_ok();
        self.pending.retain(|midi, _| is_allowed(midi));

        let mut confirmed = Vec::new();
        for &midi in &allowed {
            if let Some(missed) = self.active.get_mut(&midi) {
                *missed = 0;
                continue;
            }
            let seen = self.pending.entry(midi).or_default();
            *seen += 1;
            if *seen >= self.onset_frames {
                confirmed.push(midi);
                self.active.insert(midi, 0);
            }
        }
        self.pending.retain(|midi, _| !self.active.contains_key(midi));
        self.active.retain(|midi, missed| {
            if is_allowed(midi) {
                true
            } else {
                *missed += 1;
                *missed < self.release_frames
            }
        });

        let mut active: Vec<u8> = self.active.keys().copied().collect();
        active.sort_unstable();
        confirmed.sort_unstable();
        TrackedNotes { active, confirmed }
    }
}

#[cfg(test)]
mod tests;
