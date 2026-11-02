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

pub mod flute;
pub mod guitar;
pub mod substitute;

use crate::chart::Action;
use crate::harmonica::Harmonica;
use crate::pitch_map::{HoleAssignment, Technique, playable_assignments};
use flute::{FingeringAssignment, Flute};
use guitar::{FretAssignment, Guitar, StringTechnique};

/// The instrument a chart part is played on. A closed enum rather than a
/// trait object, so a chart can name its instrument in JSON and every UI
/// `match` breaks at compile time when a new one is added.
#[derive(Clone, Debug, PartialEq)]
pub enum Instrument {
    Harmonica(Harmonica),
    Guitar(Guitar),
    Flute(Flute),
}

impl From<Harmonica> for Instrument {
    fn from(harp: Harmonica) -> Self {
        Self::Harmonica(harp)
    }
}

impl Instrument {
    /// How the synth sounds this instrument, for demos and auditions.
    pub fn synth_voice(&self) -> crate::synth::Voice {
        match self {
            Self::Harmonica(_) => crate::synth::Voice::Harmonica,
            Self::Guitar(_) => crate::synth::Voice::PluckedString,
            Self::Flute(_) => crate::synth::Voice::Flute,
        }
    }
}

impl From<Flute> for Instrument {
    fn from(flute: Flute) -> Self {
        Self::Flute(flute)
    }
}

impl From<Guitar> for Instrument {
    fn from(guitar: Guitar) -> Self {
        Self::Guitar(guitar)
    }
}

/// Where on the instrument a note is played.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayPosition {
    /// A harmonica hole, breath direction and technique.
    Hole(HoleAssignment),
    /// A guitar string, fret and technique.
    Fret(FretAssignment),
    /// A flute note, and which of its fingerings.
    Fingering(FingeringAssignment),
}

impl PlayPosition {
    /// A harmonica hole played plainly: no bend, overblow or slide.
    pub fn natural_hole(hole: u8, action: Action) -> Self {
        Self::Hole(HoleAssignment { hole, action, technique: Technique::Natural })
    }
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
    /// A jump between discrete pitches from one position (overblows,
    /// a guitar's natural harmonics).
    Register,
    /// The same pitch found in another place (another string).
    Position,
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
    /// Notes travel right to left along rows, one row per lane, the way
    /// tablature reads.
    Horizontal,
}

/// One lane of the play highway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lane {
    /// What the instrument strip prints for the lane: a hole number, a
    /// string's name, a flute key's, a note's.
    pub label: String,
    /// The pitch a pitch lane carries (a flute's note view); `None` for a
    /// lane that is a place on the instrument (a hole, a string, a key).
    pub pitch: Option<u8>,
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
            PlayPosition::Fret(a) => usize::from(a.string.checked_sub(1)?),
            // On pitch lanes, its note's lane; on key rows a fingering is
            // many rows (`Flute::pressed_rows`), so no one lane.
            PlayPosition::Fingering(a) => {
                self.lanes.iter().position(|l| l.pitch == Some(a.note))?
            }
        };
        (lane < self.lanes.len()).then_some(lane)
    }

    /// [`lane_of`](Self::lane_of), but a position with no lane of its own is
    /// drawn in the nearest one, so a note never silently goes missing from
    /// the highway while still being judged.
    pub fn nearest_lane(&self, position: &PlayPosition) -> usize {
        let last = self.lanes.len().saturating_sub(1);
        match position {
            PlayPosition::Hole(a) => usize::from(a.hole.saturating_sub(1)).min(last),
            PlayPosition::Fret(a) => usize::from(a.string.saturating_sub(1)).min(last),
            PlayPosition::Fingering(a) => self
                .lanes
                .iter()
                .enumerate()
                .filter_map(|(i, l)| Some((i, l.pitch?)))
                .min_by_key(|&(_, pitch)| pitch.abs_diff(a.note))
                .map_or(0, |(i, _)| i),
        }
    }

    /// Whether these lanes are pitches (a flute's note view) rather than
    /// places on the instrument.
    pub fn is_pitch_lanes(&self) -> bool {
        self.lanes.first().is_some_and(|lane| lane.pitch.is_some())
    }

    /// A flute's note view: one row per pitch in `used`, highest at the
    /// top, travelled right to left. Three octaves would be 37 rows too
    /// thin to read, and a tune uses a handful.
    pub fn pitch_rows(used: &std::collections::BTreeSet<u8>) -> Self {
        Self {
            axis: ScrollAxis::Horizontal,
            lanes: used
                .iter()
                .rev()
                .map(|&pitch| Lane {
                    label: crate::midi::midi_to_note(i32::from(pitch)),
                    pitch: Some(pitch),
                })
                .collect(),
        }
    }

    /// Each lane's share of the highway's width, in percent.
    pub fn lane_pct(&self) -> f32 {
        100.0 / self.lanes.len().max(1) as f32
    }

    /// The left edge, in percent of the highway, of something `width_pct`
    /// wide centred in `lane`.
    pub fn centred_left_pct(&self, lane: usize, width_pct: f32) -> f32 {
        let lane_pct = self.lane_pct();
        lane as f32 * lane_pct + (lane_pct - width_pct) * 0.5
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

    /// Every pitch a player of this instrument can sound, by any technique:
    /// the set a detected pitch must belong to before it may score.
    fn valid_pitches(&self) -> std::collections::HashSet<u8>;

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

/// `technique` with a bend's depth rounded to the semitone it sounds, and a
/// bend that rounds to nothing read as the plain reed.
fn sounding_technique(technique: Technique) -> Technique {
    match technique {
        Technique::Bend(depth) if depth.round() == 0.0 => Technique::Natural,
        Technique::Bend(depth) => Technique::Bend(depth.round()),
        other => other,
    }
}

impl InstrumentModel for Harmonica {
    fn range(&self) -> Option<(u8, u8)> {
        let notes = self.build_valid_notes();
        Some((*notes.iter().min()?, *notes.iter().max()?))
    }

    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        playable_assignments(midi, self).into_iter().map(PlayPosition::Hole).collect()
    }

    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        self.build_valid_notes()
    }

    /// Checked against [`playable_assignments`] rather than computed from
    /// the technique alone, so a bend on a hole that doesn't bend, or an
    /// overblow on the wrong breath, is unplayable here exactly as it is
    /// everywhere else.
    ///
    /// A bend sounds the semitone its depth rounds to, as gameplay judges it
    /// (`gameplay::notes::target_pitch`): a quarter-tone bend is aimed at,
    /// but the pitch identity is the nearest semitone.
    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        let PlayPosition::Hole(a) = position else {
            return None;
        };
        let a = &HoleAssignment { technique: sounding_technique(a.technique), ..*a };
        let reed = self.wind_direction_midi(a.hole, &a.action)?;
        let candidate = match a.technique {
            Technique::Natural => Some(reed),
            Technique::Bend(depth) => reed.checked_sub(depth as u8),
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
        let (PlayPosition::Hole(a), PlayPosition::Hole(b)) = (a, b) else {
            return false;
        };
        a.action == b.action && a.hole != b.hole
    }

    /// Gives every overlapping note the anchor's breath direction — what the
    /// Song Editor has always done — dropping a technique the new direction
    /// can't play back to the plain reed.
    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]) {
        let PlayPosition::Hole(anchor) = anchor else {
            return;
        };
        for other in others {
            let PlayPosition::Hole(a) = other else {
                continue;
            };
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
            lanes: (1..=self.hole_count())
                .map(|hole| Lane { label: hole.to_string(), pitch: None })
                .collect(),
        }
    }
}

impl InstrumentModel for Guitar {
    fn range(&self) -> Option<(u8, u8)> {
        let pitches = self.valid_pitches();
        Some((*pitches.iter().min()?, *pitches.iter().max()?))
    }

    /// Fretted positions only, lowest fret first. A bend or harmonic is a
    /// technique an author chooses, never a way to reach a pitch the neck
    /// lacks, so the fretted pitch is always the answer.
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        self.fretted_positions(midi).into_iter().map(PlayPosition::Fret).collect()
    }

    /// Enumerate sounding pitches, including harmonics above a short neck
    /// and bends. Custom tunings can have gaps, so a range is not enough.
    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        let mut pitches = std::collections::HashSet::new();
        for string in 1..=self.string_count() {
            for fret in 0..=self.top_fret() {
                for technique in [
                    StringTechnique::Fretted,
                    StringTechnique::Bend(1.0),
                    StringTechnique::Bend(2.0),
                    StringTechnique::Bend(guitar::MAX_BEND),
                    StringTechnique::Harmonic,
                ] {
                    if let Some(pitch) = self.pitch_of(&FretAssignment { string, fret, technique })
                        && pitch <= 127
                    {
                        pitches.insert(pitch);
                    }
                }
            }
        }
        pitches
    }

    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        let PlayPosition::Fret(a) = position else {
            return None;
        };
        Guitar::pitch_of(self, a)
    }

    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool {
        let (PlayPosition::Fret(a), PlayPosition::Fret(b)) = (a, b) else {
            return false;
        };
        self.can_hold_together(a, b)
    }

    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]) {
        let PlayPosition::Fret(anchor) = anchor else {
            return;
        };
        let mut frets: Vec<FretAssignment> = others
            .iter()
            .filter_map(|p| match p {
                PlayPosition::Fret(a) => Some(*a),
                PlayPosition::Hole(_) | PlayPosition::Fingering(_) => None,
            })
            .collect();
        Guitar::reconcile(self, anchor, &mut frets);
        let mut frets = frets.into_iter();
        for other in others {
            if let PlayPosition::Fret(a) = other
                && let Some(reconciled) = frets.next()
            {
                *a = reconciled;
            }
        }
    }

    fn techniques(&self) -> &'static [TechniqueKind] {
        &[
            TechniqueKind::Target,
            TechniqueKind::Glide,
            TechniqueKind::Register,
            TechniqueKind::Position,
        ]
    }

    fn expression_scope(&self) -> ExpressionScope {
        ExpressionScope::PerVoice
    }

    fn envelope(&self) -> Envelope {
        Envelope::Decaying
    }

    fn lane_layout(&self) -> LaneLayout {
        LaneLayout {
            axis: ScrollAxis::Horizontal,
            lanes: self
                .string_labels()
                .into_iter()
                .map(|label| Lane { label, pitch: None })
                .collect(),
        }
    }
}

/// One voice; every position is a fingering from the table in [`flute`],
/// and the play highway's rows are its keys.
impl InstrumentModel for Flute {
    fn range(&self) -> Option<(u8, u8)> {
        let notes = self.fingerings();
        Some((notes.first()?.note, notes.last()?.note))
    }

    /// The note's fingerings, standard first.
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        (0..self.fingerings_for(midi).len())
            .map(|alternate| {
                PlayPosition::Fingering(FingeringAssignment {
                    note: midi,
                    alternate: alternate as u8,
                })
            })
            .collect()
    }

    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        self.fingerings().into_iter().map(|f| f.note).collect()
    }

    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        let PlayPosition::Fingering(a) = position else {
            return None;
        };
        self.keys(*a).map(|_| a.note)
    }

    /// Never: a flute sounds one note at a time.
    fn can_sound_together(&self, _: &PlayPosition, _: &PlayPosition) -> bool {
        false
    }

    /// Nothing to change in the others: an overlapping note on a flute
    /// isn't a fingering problem, it is a new note ending the last.
    fn reconcile(&self, _: &PlayPosition, _: &mut [PlayPosition]) {}

    /// Its lip bend is intonation, not a way to a new note, so no `Glide`.
    fn techniques(&self) -> &'static [TechniqueKind] {
        &[TechniqueKind::Target, TechniqueKind::Register, TechniqueKind::Position]
    }

    fn expression_scope(&self) -> ExpressionScope {
        ExpressionScope::WholeInstrument
    }

    fn envelope(&self) -> Envelope {
        Envelope::Sustained
    }

    /// One row per key, down the tube ([`flute::key_order`]), travelled
    /// right to left like tab: a note arrives as the shape of its
    /// fingering, a block on every key it presses.
    fn lane_layout(&self) -> LaneLayout {
        LaneLayout {
            axis: ScrollAxis::Horizontal,
            lanes: flute::key_order(self.foot)
                .into_iter()
                .map(|key| Lane { label: flute::key_label(key).to_string(), pitch: None })
                .collect(),
        }
    }
}

impl InstrumentModel for Instrument {
    fn range(&self) -> Option<(u8, u8)> {
        match self {
            Self::Harmonica(harp) => harp.range(),
            Self::Guitar(guitar) => InstrumentModel::range(guitar),
            Self::Flute(flute) => InstrumentModel::range(flute),
        }
    }

    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        match self {
            Self::Harmonica(harp) => harp.positions_for(midi),
            Self::Guitar(guitar) => InstrumentModel::positions_for(guitar, midi),
            Self::Flute(flute) => InstrumentModel::positions_for(flute, midi),
        }
    }

    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        match self {
            Self::Harmonica(harp) => harp.valid_pitches(),
            Self::Guitar(guitar) => InstrumentModel::valid_pitches(guitar),
            Self::Flute(flute) => InstrumentModel::valid_pitches(flute),
        }
    }

    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        match self {
            Self::Harmonica(harp) => harp.pitch_of(position),
            Self::Guitar(guitar) => InstrumentModel::pitch_of(guitar, position),
            Self::Flute(flute) => InstrumentModel::pitch_of(flute, position),
        }
    }

    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool {
        match self {
            Self::Harmonica(harp) => harp.can_sound_together(a, b),
            Self::Guitar(guitar) => InstrumentModel::can_sound_together(guitar, a, b),
            Self::Flute(flute) => InstrumentModel::can_sound_together(flute, a, b),
        }
    }

    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]) {
        match self {
            Self::Harmonica(harp) => harp.reconcile(anchor, others),
            Self::Guitar(guitar) => InstrumentModel::reconcile(guitar, anchor, others),
            Self::Flute(flute) => InstrumentModel::reconcile(flute, anchor, others),
        }
    }

    fn techniques(&self) -> &'static [TechniqueKind] {
        match self {
            Self::Harmonica(harp) => harp.techniques(),
            Self::Guitar(guitar) => InstrumentModel::techniques(guitar),
            Self::Flute(flute) => InstrumentModel::techniques(flute),
        }
    }

    fn expression_scope(&self) -> ExpressionScope {
        match self {
            Self::Harmonica(harp) => harp.expression_scope(),
            Self::Guitar(guitar) => InstrumentModel::expression_scope(guitar),
            Self::Flute(flute) => InstrumentModel::expression_scope(flute),
        }
    }

    fn envelope(&self) -> Envelope {
        match self {
            Self::Harmonica(harp) => harp.envelope(),
            Self::Guitar(guitar) => InstrumentModel::envelope(guitar),
            Self::Flute(flute) => InstrumentModel::envelope(flute),
        }
    }

    fn lane_layout(&self) -> LaneLayout {
        match self {
            Self::Harmonica(harp) => harp.lane_layout(),
            Self::Guitar(guitar) => InstrumentModel::lane_layout(guitar),
            Self::Flute(flute) => InstrumentModel::lane_layout(flute),
        }
    }
}

#[cfg(test)]
mod tests;
