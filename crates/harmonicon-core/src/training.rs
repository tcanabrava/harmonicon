// SPDX-License-Identifier: MIT

//! Generated practice drills — the *training* half of a lesson.
//!
//! A lesson teaches; a training presents the same material again, harder,
//! with no explanation. Five tiers per lesson, and 41 lessons, is over two
//! hundred charts — far too many to author by hand — so a training is a
//! [`DrillSpec`] rendered on demand rather than a file. See
//! `docs/training_tree_plan.md`.
//!
//! Pure and Bevy-free. `jam::backing::generated_chart` is the precedent for
//! building a playable chart from parameters, but it lives above this crate
//! and serves backing tracks; a drill has to be reachable from
//! `harmonicon-song`, which sits below it.
//!
//! **A bend drill alternates the plain reed with the bent one**, rather than
//! playing bends back to back. That is how the technique is actually taught:
//! the unbent note is the reference the ear needs to hear the bend against,
//! and releasing back to it is half the skill.
//!
//! **Deterministic on the seed.** A player re-attempting tier 3 must get the
//! same exercise, not a fresh one — otherwise a failed attempt and its retry
//! are not comparable, and the tier's own pass threshold means nothing.

use crate::chart::{
    Action, Difficulty, HarpChart, Metadata, Modifier, NoteEvent, Scoring, Song, TempoPoint,
    Timing, TrackItem,
};
use crate::harmonica::{Harmonica, hole_notes};
use crate::pitch_map::{Technique, technique_fits_hole};

/// What a drill trains. Only techniques that change which *pitch* comes out
/// of a hole: an expression technique (wah, vibrato) shapes a note rather
/// than choosing it and wants a different generator entirely.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrillTechnique {
    Bend,
}

/// The five tiers, in order. One ladder for every technique, so the shape is
/// learnable once rather than per lesson.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    /// The technique alone, slowly, on one hole.
    Isolate,
    /// The same material, faster.
    Consolidate,
    /// Every hole the lesson covers, and every depth they reach.
    Vary,
    /// The technique inside a phrase rather than on its own.
    InContext,
    /// Unpredictable order, so the player cannot run it from muscle memory.
    Interleave,
}

impl Tier {
    pub const ALL: [Tier; 5] = [
        Tier::Isolate,
        Tier::Consolidate,
        Tier::Vary,
        Tier::InContext,
        Tier::Interleave,
    ];

    /// 1-based, as a player sees it.
    pub fn number(self) -> u8 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as u8 + 1
    }

    pub fn from_number(n: u8) -> Option<Tier> {
        Self::ALL.get(n.checked_sub(1)? as usize).copied()
    }

    /// Tempo in BPM.
    ///
    /// **This does not rise monotonically, and [`notes_per_second`]
    /// (Self::notes_per_second) is the ladder's real measure.** Tempo and
    /// subdivision both feed it, so raising both at the same step doubles
    /// the note rate in one go — an earlier draft jumped from 1.27 to 2.9
    /// notes per second between tiers 2 and 3, which is not a step a player
    /// working on bends can take. Eighths arrive at a *lower* tempo than
    /// the quarters before them, which keeps the click somewhere the player
    /// can still feel it while the notes get closer together.
    pub fn bpm(self) -> f32 {
        match self {
            Tier::Isolate => 60.0,
            Tier::Consolidate => 76.0,
            Tier::Vary => 96.0,
            Tier::InContext => 60.0,
            Tier::Interleave => 76.0,
        }
    }

    /// Notes per beat. Quarters while the pitch itself is the problem;
    /// eighths once it isn't.
    pub fn notes_per_beat(self) -> usize {
        match self {
            Tier::Isolate | Tier::Consolidate | Tier::Vary => 1,
            _ => 2,
        }
    }

    /// How fast notes actually arrive — what the ladder escalates. Rises
    /// about 25-27% a tier, roughly 1.0 to 2.5 across the five.
    pub fn notes_per_second(self) -> f32 {
        self.bpm() / 60.0 * self.notes_per_beat() as f32
    }

    pub fn bars(self) -> usize {
        match self {
            Tier::Isolate | Tier::Consolidate | Tier::Vary => 4,
            _ => 8,
        }
    }

    /// Whether the tier uses every hole the lesson covers, or only the first.
    fn uses_every_hole(self) -> bool {
        !matches!(self, Tier::Isolate | Tier::Consolidate)
    }

    /// Difficulty stamped on the generated chart, so the ordinary song UI
    /// describes a training the same way it describes anything else.
    fn difficulty(self) -> Difficulty {
        match self {
            Tier::Isolate | Tier::Consolidate => Difficulty::Easy,
            Tier::Vary | Tier::InContext => Difficulty::Intermediate,
            Tier::Interleave => Difficulty::Advanced,
        }
    }
}

/// One training, as a lesson declares it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrillSpec {
    pub technique: DrillTechnique,
    /// Holes the lesson covers, in teaching order — the first is what the
    /// isolating tiers drill on its own.
    pub holes: Vec<u8>,
    pub tier: Tier,
    pub seed: u64,
}

/// One note to place: a hole, and how deep to bend it (0 = play it plain).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Step {
    hole: u8,
    depth: u8,
}

/// Bends live on the draw reed of the low holes and the blow reed of the
/// high ones, because a bend pulls toward the *other* reed in the hole and
/// that is which way round it sits. Same rule `pitch_map::map_pitch_playable`
/// resolves by; `bend_action_matches_the_resolver` pins the two together.
pub fn bend_action(hole: u8) -> Action {
    if hole <= 6 {
        Action::Draw
    } else {
        Action::Blow
    }
}

/// Every bend `hole` can actually produce, shallowest first, as semitone
/// depths.
///
/// Read off [`hole_notes`] — the layout, not a table — so a harp with an
/// unusual tuning drills what it can really do.
fn depths_available(harp: &Harmonica, hole: u8) -> Vec<u8> {
    hole_notes(harp, hole)
        .bends
        .iter()
        .enumerate()
        .map(|(i, _)| i as u8 + 1)
        .filter(|d| technique_fits_hole(Technique::Bend(f32::from(*d)), hole))
        .collect()
}

/// Builds a playable drill, or `None` if nothing in `spec.holes` can do the
/// technique on this harp at all.
pub fn drill_chart(
    spec: &DrillSpec,
    harp: &Harmonica,
    title: &str,
    artist: &str,
) -> Option<HarpChart> {
    let scope: Vec<u8> = if spec.tier.uses_every_hole() {
        spec.holes.clone()
    } else {
        spec.holes.first().copied().into_iter().collect()
    };

    let steps = match spec.technique {
        DrillTechnique::Bend => bend_steps(&scope, harp, spec.tier, spec.seed),
    };
    if steps.is_empty() {
        return None;
    }

    let bpm = spec.tier.bpm();
    let secs_per_note = 60.0 / f64::from(bpm) / spec.tier.notes_per_beat() as f64;
    let total = spec.tier.bars() * 4 * spec.tier.notes_per_beat();

    let mut track = Vec::with_capacity(total);
    for i in 0..total {
        let step = steps[i % steps.len()];
        let action = bend_action(step.hole);
        // The reed, always — not the pitch that sounds. Both readers of this
        // field add the bend modifier to it (`gameplay::notes::target_pitch`,
        // `harp_remap::source_pitch`), so stating the bent pitch would apply
        // the bend twice.
        let reed = harp.wind_direction_label(step.hole, &action);
        let modifiers = (step.depth > 0).then(|| {
            vec![Modifier::Bend {
                semitones: -f32::from(step.depth),
                intensity: None,
            }]
        });
        track.push(TrackItem {
            id: None,
            time: Some(i as f64 * secs_per_note),
            tick: None,
            duration: secs_per_note,
            phrase: None,
            groove: None,
            play_mode: None,
            call: false,
            events: vec![NoteEvent {
                hole: step.hole,
                action,
                note: Some(reed),
                modifiers,
            }],
        });
    }

    Some(HarpChart {
        metadata: Some(Metadata {
            format_version: Some(crate::chart::CURRENT_FORMAT_VERSION.to_string()),
            author: None,
            source: Some(format!("generated drill, tier {}", spec.tier.number())),
            license: None,
            description: None,
        }),
        song: Song {
            title: title.to_string(),
            artist: artist.to_string(),
            tempo_bpm: bpm,
            key: crate::harmonica::detected_harp_key(harp).unwrap_or_else(|| "C".to_string()),
            difficulty: spec.tier.difficulty(),
            time_signature: Some("4/4".to_string()),
            feel: None,
        },
        timing: Timing {
            resolution: crate::synth::TICKS_PER_BEAT as u32,
            tempo_map: vec![TempoPoint { tick: 0, bpm }],
            time_signature_map: None,
        },
        harmonica: harp.clone(),
        track,
        loop_section: None,
        scoring: Scoring {
            perfect_window_ms: 50,
            good_window_ms: 100,
            miss_window_ms: 130,
            combo: None,
            style_bonus: None,
        },
    })
}

/// The note sequence for a bend drill, one tier's worth.
fn bend_steps(holes: &[u8], harp: &Harmonica, tier: Tier, seed: u64) -> Vec<Step> {
    // (hole, depth) for everything drillable, in teaching order.
    let targets: Vec<Step> = holes
        .iter()
        .flat_map(|&hole| {
            depths_available(harp, hole)
                .into_iter()
                .map(move |depth| Step { hole, depth })
        })
        .collect();
    if targets.is_empty() {
        return Vec::new();
    }

    let mut steps = Vec::new();
    match tier {
        // Bend and release, on one hole, at the shallowest depth: the plain
        // reed as the ear's reference, then the bend, over and over.
        Tier::Isolate | Tier::Consolidate => {
            let first = targets[0];
            steps.push(Step {
                hole: first.hole,
                depth: 0,
            });
            steps.push(first);
        }
        // Same shape, but across every hole and every depth they reach.
        Tier::Vary => {
            for t in &targets {
                steps.push(Step {
                    hole: t.hole,
                    depth: 0,
                });
                steps.push(*t);
            }
        }
        // The bend arrives at the end of a short figure instead of straight
        // after its own reference note, so it has to be found from somewhere
        // else rather than merely held.
        Tier::InContext => {
            for t in &targets {
                // The next hole *after this target's own*, never the target
                // itself — picking by position in `targets` let the figure
                // collapse to three repeats of one note whenever the two
                // happened to coincide.
                let neighbour = holes
                    .iter()
                    .position(|h| *h == t.hole)
                    .map(|i| holes[(i + 1) % holes.len()])
                    .filter(|h| *h != t.hole)
                    .unwrap_or(t.hole);
                steps.push(Step {
                    hole: t.hole,
                    depth: 0,
                });
                steps.push(Step {
                    hole: neighbour,
                    depth: 0,
                });
                steps.push(Step {
                    hole: t.hole,
                    depth: 0,
                });
                steps.push(*t);
            }
        }
        // Everything shuffled, so the next bend cannot be anticipated. This
        // is the tier that trains recall rather than repetition.
        Tier::Interleave => {
            for t in &targets {
                steps.push(Step {
                    hole: t.hole,
                    depth: 0,
                });
                steps.push(*t);
            }
            shuffle(&mut steps, seed);
        }
    }
    steps
}

/// Fisher-Yates against a small xorshift, so a seed always produces the same
/// order without pulling in a random-number dependency.
fn shuffle(steps: &mut [Step], seed: u64) {
    let mut state = seed | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in (1..steps.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        steps.swap(i, j);
    }
}

/// What a drill event should sound, for tests and for callers that want to
/// check a generated chart against the harp.
pub fn sounded_pitch(event: &NoteEvent, harp: &Harmonica) -> Option<u8> {
    crate::harp_remap::source_pitch(
        event.hole,
        event.action,
        event.note.as_deref(),
        event.modifiers.as_deref().unwrap_or(&[]),
        harp,
    )
}

#[cfg(test)]
mod tests;
