// SPDX-License-Identifier: MIT

//! Playing a chart on a harmonica other than the one it was written for.
//!
//! A chart declares its harp, and every expected pitch derives from that
//! harp's layout. A player who owns a different key has two coherent
//! choices, and they are opposites:
//!
//! - [`HarpMapping::SameHoles`] — keep the tab, let the music transpose.
//!   Hole 4 draw stays hole 4 draw; on a G harp it simply sounds a fifth
//!   away from what a C harp would. This is how a harmonica player already
//!   thinks: the tab *is* the piece, and the key is whichever harp is in
//!   your pocket.
//! - [`HarpMapping::Transpose`] — keep the music, recompute the tab. The
//!   piece sounds as written; the holes change, and some notes may need a
//!   bend or an overblow they didn't before, or become unreachable.
//!
//! **The load-bearing property is that `midi` is what the microphone will
//! hear.** Scoring compares detected pitch against it, so if it ever
//! disagrees with what the player's harp physically produces, the game
//! listens for a note nobody can play and scores nothing. Every function
//! here exists to keep that one field honest.
//!
//! One trap worth stating outright: a chart event may carry an explicit
//! `note`, and in practice every bundled event does. That name describes
//! the *chart's own* harp. Under `SameHoles` it must be ignored and
//! re-derived, or the expected pitch silently stays on the original harp
//! while the player blows a different one — the exact failure this module
//! is meant to prevent, and one that would affect every shipped chart.

use crate::chart::{Action, Modifier};
use crate::harmonica::{Harmonica, hole_notes};
use crate::midi::note_to_midi;
use crate::pitch_map::{Technique, map_pitch_playable, technique_fits_hole};

/// What swapping the harmonica should preserve.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HarpMapping {
    /// Keep the tab; the music transposes with the harp.
    #[default]
    SameHoles,
    /// Keep the music; the tab is recomputed for the new harp.
    Transpose,
}

impl HarpMapping {
    pub fn all() -> &'static [HarpMapping] {
        &[HarpMapping::SameHoles, HarpMapping::Transpose]
    }
}

/// One chart event, resolved onto the harp actually being played.
#[derive(Clone, PartialEq, Debug)]
pub struct RemappedEvent {
    pub hole: u8,
    pub action: Action,
    /// Techniques the *target* harp needs for this note. Under
    /// `SameHoles` these are the chart's own; under `Transpose` they are
    /// whatever the new harp requires, which may be nothing where the
    /// original needed a bend, or an overblow where it needed nothing.
    pub modifiers: Vec<Modifier>,
    /// The pitch that will actually sound — what scoring must expect and
    /// the microphone must hear. `None` only if the harp has no note there
    /// at all.
    pub midi: Option<u8>,
    /// False when the target harp cannot produce this note by any means.
    /// The caller keeps such notes visible but must not score them: a
    /// player can't miss what they were never able to play.
    pub playable: bool,
}

/// The bend depth a modifier list asks for, in semitones *down*.
///
/// Charts store a bend as a negative `semitones` (a downward pitch
/// offset), while [`Technique::Bend`] carries a positive depth. Keeping the
/// conversion in one place is what stops the sign being flipped twice.
fn bend_depth(modifiers: &[Modifier]) -> f32 {
    modifiers
        .iter()
        .find_map(|m| match m {
            Modifier::Bend { semitones, .. } => Some(-semitones),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// Non-pitch modifiers (vibrato, wah) — expression that survives any harp
/// swap untouched, unlike bends and over-techniques which describe *how a
/// pitch is reached* and must be recomputed.
fn expression_only(modifiers: &[Modifier]) -> Vec<Modifier> {
    modifiers
        .iter()
        .filter(|m| {
            !matches!(
                m,
                Modifier::Bend { .. } | Modifier::Overblow | Modifier::Overdraw | Modifier::Slide
            )
        })
        .cloned()
        .collect()
}

/// A technique expressed as the chart modifier that records it.
fn technique_modifier(technique: Technique) -> Option<Modifier> {
    match technique {
        Technique::Natural => None,
        // Back to the chart's negative-is-down convention.
        Technique::Bend(depth) => Some(Modifier::Bend {
            semitones: -depth,
            intensity: None,
        }),
        Technique::Overblow => Some(Modifier::Overblow),
        Technique::Overdraw => Some(Modifier::Overdraw),
        Technique::Slide => Some(Modifier::Slide),
    }
}

/// The pitch an event sounds on the harp it was written for.
///
/// `explicit` is the event's own `note` field when it has one — the chart
/// stating the natural reed note directly rather than leaving it to be
/// looked up. Modifiers apply on top either way.
pub fn source_pitch(
    hole: u8,
    action: Action,
    explicit: Option<&str>,
    modifiers: &[Modifier],
    chart_harp: &Harmonica,
) -> Option<u8> {
    let natural = match explicit {
        Some(note) => note.to_string(),
        None => chart_harp.wind_direction_label(hole, &action),
    };
    let midi = note_to_midi(&natural)? - bend_depth(modifiers).round() as i32;
    u8::try_from(midi).ok()
}

/// Resolves one chart event onto `target_harp`.
///
/// With `target_harp` equal to the chart's own harp this is the identity in
/// both modes, so the default costs nothing and changes nothing.
pub fn remap_event(
    hole: u8,
    action: Action,
    explicit: Option<&str>,
    modifiers: &[Modifier],
    chart_harp: &Harmonica,
    target_harp: &Harmonica,
    mapping: HarpMapping,
) -> RemappedEvent {
    match mapping {
        HarpMapping::SameHoles => {
            // Deliberately *not* `explicit`: that note names the chart's
            // own harp. Re-deriving from the target harp is the whole
            // point — see this module's header.
            //
            // Which note to re-derive depends on the technique, because an
            // over-technique or a slide does not sound its hole's blow/draw
            // reed. Reading the reed for those would keep the tab but report
            // the wrong pitch — the same class of error as honouring
            // `explicit`, just arrived at differently.
            let depth = bend_depth(modifiers);
            let over = modifiers
                .iter()
                .any(|m| matches!(m, Modifier::Overblow | Modifier::Overdraw));
            let slide = modifiers.iter().any(|m| matches!(m, Modifier::Slide));

            let midi = if over {
                hole_notes(target_harp, hole)
                    .over
                    .as_deref()
                    .and_then(note_to_midi)
                    .and_then(|m| u8::try_from(m).ok())
            } else {
                let natural = target_harp.wind_direction_label(hole, &action);
                note_to_midi(&natural)
                    .map(|m| m - depth.round() as i32 + i32::from(slide))
                    .and_then(|m| u8::try_from(m).ok())
            };

            // A hole that exists on the source harp may not bend as far on
            // the target, may not overblow at all, and may not exist on a
            // shorter one.
            let technique_ok = if over {
                modifiers.iter().all(|m| match m {
                    Modifier::Overblow => technique_fits_hole(Technique::Overblow, hole),
                    Modifier::Overdraw => technique_fits_hole(Technique::Overdraw, hole),
                    _ => true,
                })
            } else {
                depth == 0.0 || technique_fits_hole(Technique::Bend(depth), hole)
            };
            let playable = midi.is_some() && hole <= target_harp.hole_count() && technique_ok;
            RemappedEvent {
                hole,
                action,
                modifiers: modifiers.to_vec(),
                midi,
                playable,
            }
        }
        HarpMapping::Transpose => {
            let Some(target) = source_pitch(hole, action, explicit, modifiers, chart_harp) else {
                return RemappedEvent {
                    hole,
                    action,
                    modifiers: modifiers.to_vec(),
                    midi: None,
                    playable: false,
                };
            };
            // Least disturbance first: if the note *as written* already
            // sounds the right pitch on the target harp — same hole, same
            // breath, same technique — keep all of it.
            //
            // Two reasons, and the second is the load-bearing one:
            //
            // Without it, resolving picks the lowest hole producing that
            // pitch and shuffles the tab even when the harps are identical:
            // hole 3 blow and hole 2 draw are both G4 on a C harp.
            //
            // It also guards re-resolution generally: a note the chart
            // already sounds correctly on this harp is kept as written,
            // rather than being put back through the resolver and risking a
            // different answer. That mattered acutely while `max_bend`
            // disagreed with `build_valid_notes` about hole 3 (capping it
            // at 1.5 semitones instead of three), which made a
            // transposition onto the chart's *own* harmonica report its own
            // notes unplayable; `max_bend` is now derived from the same
            // blow-to-draw gap and the two agree, but keeping what already
            // works is the cheaper and safer rule regardless.
            if source_pitch(hole, action, None, modifiers, target_harp) == Some(target) {
                return RemappedEvent {
                    hole,
                    action,
                    modifiers: modifiers.to_vec(),
                    midi: Some(target),
                    playable: true,
                };
            }
            match map_pitch_playable(target, target_harp) {
                Some(assignment) => {
                    let mut new_modifiers = expression_only(modifiers);
                    if let Some(m) = technique_modifier(assignment.technique) {
                        new_modifiers.push(m);
                    }
                    RemappedEvent {
                        hole: assignment.hole,
                        action: assignment.action,
                        modifiers: new_modifiers,
                        midi: Some(target),
                        playable: true,
                    }
                }
                // Unreachable on this harp. The pitch is still reported, so
                // the note can be shown and the player told what it would
                // have been — it simply must not be scored.
                None => RemappedEvent {
                    hole,
                    action,
                    modifiers: modifiers.to_vec(),
                    midi: Some(target),
                    playable: false,
                },
            }
        }
    }
}

/// What choosing `target_harp` will cost, for the pre-play screen.
///
/// Counting before committing is the point: a transposition that quietly
/// turns a beginner melody into an overblow study is worse than being told
/// the harp doesn't fit.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RemapCost {
    pub total: usize,
    pub bends: usize,
    pub overblows: usize,
    /// Notes the target harp cannot produce at all.
    pub unplayable: usize,
}

impl RemapCost {
    /// Folds one resolved event in.
    pub fn add(&mut self, event: &RemappedEvent) {
        self.total += 1;
        if !event.playable {
            self.unplayable += 1;
            return;
        }
        for m in &event.modifiers {
            match m {
                Modifier::Bend { .. } => self.bends += 1,
                Modifier::Overblow | Modifier::Overdraw => self.overblows += 1,
                _ => {}
            }
        }
    }

    /// Whether this harp can play the chart at all.
    pub fn is_complete(&self) -> bool {
        self.unplayable == 0
    }
}

#[cfg(test)]
mod tests;
