// SPDX-License-Identifier: MIT

//! The instrument a chart is played on, behind one interface.
//!
//! Scoring already works in pitches: the judge compares a MIDI number
//! against what the detector heard. What is instrument-specific is
//! everything around that — *where* a pitch is played ([`PlayPosition`]),
//! *what can sound together* ([`InstrumentModel::can_sound_together`]), and
//! *how it is drawn* ([`LaneLayout`]). [`InstrumentModel`] answers those
//! three questions, so gameplay, the Song Editor and the detection filter
//! can ask them without naming a harmonica.
//!
//! The harmonica is the only instrument today; its implementation is a thin
//! adapter over [`crate::pitch_map`] and [`crate::harmonica`], which remain
//! the one place harmonica physics is stated. `docs/multi_instrument_plan.md`
//! has the design and the instruments still to come.
//!
//! **A position is guidance, not something scored.** A microphone hears the
//! pitch, not which hole or string made it, so the judge only ever compares
//! [`InstrumentModel::pitch_of`]'s answer.

use crate::chart::Action;
use crate::harmonica::Harmonica;
use crate::pitch_map::{HoleAssignment, Technique, playable_assignments};

/// The instrument a chart part is played on. A closed enum rather than a
/// trait object, so a chart can name its instrument in JSON and every UI
/// `match` breaks at compile time when a new one is added.
#[derive(Clone, Debug, PartialEq)]
pub enum Instrument {
    Harmonica(Harmonica),
}

impl From<Harmonica> for Instrument {
    fn from(harp: Harmonica) -> Self {
        Self::Harmonica(harp)
    }
}

/// Where on the instrument a note is played.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayPosition {
    /// A harmonica hole, breath direction and technique.
    Hole(HoleAssignment),
}

impl From<HoleAssignment> for PlayPosition {
    fn from(assignment: HoleAssignment) -> Self {
        Self::Hole(assignment)
    }
}

/// How a technique changes the note, which decides how the Note Trainer
/// teaches it. A bend and an overblown flute note both reach a pitch the
/// plain position doesn't, but one is a continuous glide and the other a
/// jump between discrete harmonics, so they are practised differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechniqueKind {
    /// Hit and hold a note: every instrument has this.
    Target,
    /// A continuous slide in pitch from a position to a target (bends).
    Glide,
    /// A jump between discrete pitches from one position (overblows).
    Register,
}

/// Whether expression (vibrato, wah) shapes the whole sound or one voice.
/// Breath vibrato colours every sounding hole at once; a string's vibrato
/// leaves the other strings alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpressionScope {
    WholeInstrument,
    PerVoice,
}

/// How a held note behaves, which decides whether holding it is a skill to
/// reward: a breath holds its pitch for as long as the player blows, while a
/// plucked string decays however well it was played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Envelope {
    Sustained,
    Decaying,
}

/// The direction notes travel on the play highway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    /// Notes fall down columns, one column per lane.
    Vertical,
}

/// One lane of the play highway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lane {
    /// What the instrument strip prints under the lane: a hole number.
    pub label: String,
}

/// The lanes notes travel in, and which lane each position belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneLayout {
    pub axis: ScrollAxis,
    pub lanes: Vec<Lane>,
}

impl LaneLayout {
    /// The lane `position` is drawn in, or `None` for a position this
    /// layout has no lane for (a hole beyond the harp's last).
    pub fn lane_of(&self, position: &PlayPosition) -> Option<usize> {
        let lane = match position {
            PlayPosition::Hole(a) => usize::from(a.hole.checked_sub(1)?),
        };
        (lane < self.lanes.len()).then_some(lane)
    }
}

/// What an instrument can play, what it can play at once, and how it is
/// drawn. Every answer comes from the instrument's own configuration (a
/// harp's key and tuning), never from a fixed table.
pub trait InstrumentModel {
    /// Lowest and highest pitch the instrument produces, by any technique,
    /// as MIDI numbers. `None` for an instrument with no notes at all.
    fn range(&self) -> Option<(u8, u8)>;

    /// Every position that sounds `midi`, easiest first.
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition>;

    /// The pitch `position` sounds, or `None` if the instrument can't
    /// actually play it.
    fn pitch_of(&self, position: &PlayPosition) -> Option<u8>;

    /// Whether `a` and `b` can sound at the same instant.
    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool;

    /// Makes `others`, which overlap `anchor` in time, playable alongside
    /// it, changing them as little as the instrument allows.
    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]);

    /// The technique kinds this instrument has, [`TechniqueKind::Target`]
    /// first.
    fn techniques(&self) -> &'static [TechniqueKind];

    fn expression_scope(&self) -> ExpressionScope;

    fn envelope(&self) -> Envelope;

    fn lane_layout(&self) -> LaneLayout;
}

impl InstrumentModel for Harmonica {
    fn range(&self) -> Option<(u8, u8)> {
        let notes = self.build_valid_notes();
        Some((*notes.iter().min()?, *notes.iter().max()?))
    }

    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        playable_assignments(midi, self).into_iter().map(PlayPosition::Hole).collect()
    }

    /// Checked against [`playable_assignments`] rather than computed from
    /// the technique alone, so a bend on a hole that doesn't bend, or an
    /// overblow on the wrong breath, is unplayable here exactly as it is
    /// everywhere else.
    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        let PlayPosition::Hole(a) = position;
        let reed = self.wind_direction_midi(a.hole, &a.action)?;
        let candidate = match a.technique {
            Technique::Natural => Some(reed),
            Technique::Bend(depth) => reed.checked_sub(depth.round() as u8),
            Technique::Overblow | Technique::Overdraw => {
                let other = match a.action {
                    Action::Blow => Action::Draw,
                    Action::Draw => Action::Blow,
                };
                self.wind_direction_midi(a.hole, &other)?.checked_add(1)
            }
            Technique::Slide => reed.checked_add(1),
        }?;
        playable_assignments(candidate, self).contains(a).then_some(candidate)
    }

    /// One breath at a time: every hole sounding at once shares the
    /// direction. Two notes in the same hole can't sound together either,
    /// since a hole holds one reed per direction.
    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool {
        let (PlayPosition::Hole(a), PlayPosition::Hole(b)) = (a, b);
        a.action == b.action && a.hole != b.hole
    }

    /// Gives every overlapping note the anchor's breath direction — what the
    /// Song Editor has always done — dropping a technique the new direction
    /// can't play back to the plain reed.
    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]) {
        let PlayPosition::Hole(anchor) = anchor;
        for other in others {
            let PlayPosition::Hole(a) = other;
            if a.action == anchor.action {
                continue;
            }
            a.action = anchor.action;
            if self.pitch_of(&PlayPosition::Hole(*a)).is_none() {
                a.technique = Technique::Natural;
            }
        }
    }

    fn techniques(&self) -> &'static [TechniqueKind] {
        match self {
            Harmonica::Diatonic { .. } => {
                &[TechniqueKind::Target, TechniqueKind::Glide, TechniqueKind::Register]
            }
            // The slide is a button, not a technique to drill.
            Harmonica::Chromatic { .. } => &[TechniqueKind::Target],
        }
    }

    fn expression_scope(&self) -> ExpressionScope {
        ExpressionScope::WholeInstrument
    }

    fn envelope(&self) -> Envelope {
        Envelope::Sustained
    }

    fn lane_layout(&self) -> LaneLayout {
        LaneLayout {
            axis: ScrollAxis::Vertical,
            lanes: (1..=self.hole_count()).map(|hole| Lane { label: hole.to_string() }).collect(),
        }
    }
}

impl InstrumentModel for Instrument {
    fn range(&self) -> Option<(u8, u8)> {
        match self {
            Self::Harmonica(harp) => harp.range(),
        }
    }

    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        match self {
            Self::Harmonica(harp) => harp.positions_for(midi),
        }
    }

    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        match self {
            Self::Harmonica(harp) => harp.pitch_of(position),
        }
    }

    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool {
        match self {
            Self::Harmonica(harp) => harp.can_sound_together(a, b),
        }
    }

    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]) {
        match self {
            Self::Harmonica(harp) => harp.reconcile(anchor, others),
        }
    }

    fn techniques(&self) -> &'static [TechniqueKind] {
        match self {
            Self::Harmonica(harp) => harp.techniques(),
        }
    }

    fn expression_scope(&self) -> ExpressionScope {
        match self {
            Self::Harmonica(harp) => harp.expression_scope(),
        }
    }

    fn envelope(&self) -> Envelope {
        match self {
            Self::Harmonica(harp) => harp.envelope(),
        }
    }

    fn lane_layout(&self) -> LaneLayout {
        match self {
            Self::Harmonica(harp) => harp.lane_layout(),
        }
    }
}

#[cfg(test)]
mod tests;
