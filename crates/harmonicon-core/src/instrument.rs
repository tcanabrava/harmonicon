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
//! Fretted strings (guitar and bass), piano, flute and harmonica each supply
//! their physical guidance.
//! The harmonica adapter delegates to [`crate::pitch_map`] and
//! [`crate::harmonica`], which remain the source of its reed layout.
//!
//! **A position is guidance, not something scored.** A microphone hears the
//! pitch, not which hole or string made it, so the judge only ever compares
//! [`InstrumentModel::pitch_of`]'s answer.

pub mod bass;
pub mod bowed_strings;
pub mod cavaquinho;
pub mod clarinet;
pub mod flute;
pub mod fretted_strings;
pub mod guitar;
pub mod piano;
pub mod substitute;
pub mod wind;

use crate::chart::Action;
use crate::harmonica::Harmonica;
use crate::pitch_map::{HoleAssignment, Technique, playable_assignments};
use clarinet::Clarinet;
use flute::{FingeringAssignment, Flute};
use fretted_strings::FrettedInstrument;
use guitar::{FretAssignment, StringTechnique};
use piano::Piano;

/// Instrument identity used to select curricula independently of tuning or setup.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum InstrumentKind {
    #[default]
    Harmonica,
    Guitar,
    Bass,
    Cavaquinho,
    Piano,
    Flute,
    Clarinet,
    Violin,
    Viola,
}

impl InstrumentKind {
    pub const ALL: [Self; 9] = [
        Self::Harmonica,
        Self::Guitar,
        Self::Bass,
        Self::Cavaquinho,
        Self::Piano,
        Self::Flute,
        Self::Clarinet,
        Self::Violin,
        Self::Viola,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Harmonica => "harmonica",
            Self::Guitar => "guitar",
            Self::Bass => "bass",
            Self::Cavaquinho => "cavaquinho",
            Self::Piano => "piano",
            Self::Flute => "flute",
            Self::Clarinet => "clarinet",
            Self::Violin => "violin",
            Self::Viola => "viola",
        }
    }
    /// Default configuration registered with the identity, shared by document
    /// creation and trainer entry points.
    pub fn default_instrument(self) -> Instrument {
        match self {
            Self::Harmonica => crate::harmonica::richter_harp("C").into(),
            Self::Guitar => fretted_strings::FrettedKind::Guitar.default_setup().into(),
            Self::Bass => fretted_strings::FrettedKind::Bass.default_setup().into(),
            Self::Cavaquinho => fretted_strings::FrettedKind::Cavaquinho.default_setup().into(),
            Self::Piano => Piano::default().into(),
            Self::Flute => Flute::default().into(),
            Self::Clarinet => Clarinet::default().into(),
            Self::Violin => bowed_strings::BowedKind::Violin.standard().into(),
            Self::Viola => bowed_strings::BowedKind::Viola.standard().into(),
        }
    }
    pub fn from_slug(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.slug() == value)
    }
}

/// The instrument a chart part is played on. A closed enum rather than a
/// trait object for owned configuration and serialization. Generic behaviour
/// delegates through one family adapter registration rather than matching here
/// separately for each operation.
/// Instrument identity is separate: fretted guitar and bass share one model.
#[derive(Clone, Debug, PartialEq)]
pub enum Instrument {
    Harmonica(Harmonica),
    Fretted(FrettedInstrument),
    Flute(Flute),
    Clarinet(Clarinet),
    Bowed(bowed_strings::BowedStrings),
    Piano(Piano),
}

impl From<Harmonica> for Instrument {
    fn from(harp: Harmonica) -> Self {
        Self::Harmonica(harp)
    }
}

mod adapters;
pub use adapters::InstrumentAdapter;

impl Instrument {
    /// The single dispatch boundary for shared instrument behaviour.
    pub fn adapter(&self) -> &dyn InstrumentAdapter {
        match self {
            Self::Harmonica(model) => model,
            Self::Fretted(model) => model,
            Self::Flute(model) => model,
            Self::Clarinet(model) => model,
            Self::Bowed(model) => model,
            Self::Piano(model) => model,
        }
    }
    pub fn fingering_chart(&self) -> Option<&dyn wind::FingeringChart> {
        self.adapter().fingering_chart()
    }
    pub fn kind(&self) -> InstrumentKind {
        self.adapter().kind()
    }
    pub fn notation(&self) -> crate::notation::NotationContext {
        self.adapter().notation()
    }
    pub fn synth_voice(&self) -> crate::synth::Voice {
        self.adapter().synth_voice()
    }
}

impl From<bowed_strings::BowedStrings> for Instrument {
    fn from(strings: bowed_strings::BowedStrings) -> Self {
        Self::Bowed(strings)
    }
}

impl From<Clarinet> for Instrument {
    fn from(clarinet: Clarinet) -> Self {
        Self::Clarinet(clarinet)
    }
}

impl From<Piano> for Instrument {
    fn from(piano: Piano) -> Self {
        Self::Piano(piano)
    }
}

impl From<Flute> for Instrument {
    fn from(flute: Flute) -> Self {
        Self::Flute(flute)
    }
}

impl From<FrettedInstrument> for Instrument {
    fn from(guitar: FrettedInstrument) -> Self {
        Self::Fretted(guitar)
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
    /// A piano key, by its pitch: a key is its note.
    Key(u8),
    /// A sounding target on a continuous, fretless string.
    Fingerboard(bowed_strings::FingerboardPosition),
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
    /// The lane's share of the highway relative to the others, in
    /// hundredths: 100 for a full lane. Equal for every instrument but the
    /// flute, whose side keys and the gap between its hands are thinner
    /// than its finger holes (`flute::key_rows`).
    pub weight: u16,
}

impl Lane {
    /// An ordinary, full-width lane.
    pub fn new(label: String, pitch: Option<u8>) -> Self {
        Self { label, pitch, weight: 100 }
    }
}

/// The lanes notes travel in, and which lane each position belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneLayout {
    pub axis: ScrollAxis,
    /// Whether the lanes are a piano keyboard's keys, so [`span`](Self::span)
    /// spaces them as white keys with the black ones straddling them. Set
    /// by [`keyboard_rows`](Self::keyboard_rows) and nowhere else: a layout
    /// of pitch lanes that is *not* a keyboard must not get it by looking
    /// like one.
    pub white_keys: bool,
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
            PlayPosition::Key(note) => self.lanes.iter().position(|l| l.pitch == Some(*note))?,
            PlayPosition::Fingerboard(at) => usize::from(at.string.checked_sub(1)?),
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
            PlayPosition::Fingerboard(a) => usize::from(a.string.saturating_sub(1)).min(last),
            PlayPosition::Fingering(FingeringAssignment { note, .. }) | PlayPosition::Key(note) => {
                self.lanes
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| Some((i, l.pitch?)))
                    .min_by_key(|&(_, pitch)| pitch.abs_diff(*note))
                    .map_or(0, |(i, _)| i)
            }
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
            white_keys: false,
            lanes: used
                .iter()
                .rev()
                .map(|&pitch| Lane::new(crate::midi::midi_to_note(i32::from(pitch)), Some(pitch)))
                .collect(),
        }
    }

    /// Falling-note columns, low to high from left to right, aligned with
    /// white keys and the accidentals between them.
    pub fn keyboard_rows(low: u8, high: u8) -> Self {
        Self {
            axis: ScrollAxis::Vertical,
            white_keys: true,
            lanes: (low.min(high)..=high.max(low))
                .map(|pitch| Lane::new(crate::midi::midi_to_note(i32::from(pitch)), Some(pitch)))
                .collect(),
        }
    }

    /// An even lane's share of the highway's width, in percent: exact when
    /// every lane has the same weight, as on every instrument but the
    /// flute's key rows, which [`span`](Self::span) lays out.
    pub fn lane_pct(&self) -> f32 {
        100.0 / self.lanes.len().max(1) as f32
    }

    /// Where `lane` sits across the highway, `(start, size)` in percent,
    /// by the lanes' weights.
    pub fn span(&self, lane: usize) -> (f32, f32) {
        // Piano columns follow white-key spacing; accidentals straddle
        // adjacent whites, matching the horizontal keyboard drawing.
        if self.white_keys && self.lanes.iter().all(|l| l.pitch.is_some()) {
            let white = |p: u8| !crate::instrument::piano::is_black(p);
            let total = self.lanes.iter().filter(|l| white(l.pitch.unwrap())).count().max(1) as f32;
            if let Some(own) = self.lanes.get(lane) {
                let before =
                    self.lanes.iter().take(lane).filter(|l| white(l.pitch.unwrap())).count() as f32;
                return if white(own.pitch.unwrap()) {
                    (before * 100.0 / total, 100.0 / total)
                } else {
                    ((before - 0.3) * 100.0 / total, 60.0 / total)
                };
            }
        }
        // Even lanes keep the exact arithmetic every instrument but the
        // flute's key rows has always used.
        if self.lanes.windows(2).all(|pair| pair[0].weight == pair[1].weight) {
            let pct = self.lane_pct();
            return (lane as f32 * pct, pct);
        }
        let total: f32 = self.lanes.iter().map(|l| f32::from(l.weight)).sum::<f32>().max(1.0);
        let before: f32 = self.lanes.iter().take(lane).map(|l| f32::from(l.weight)).sum();
        let own = self.lanes.get(lane).map_or(0.0, |l| f32::from(l.weight));
        (before / total * 100.0, own / total * 100.0)
    }

    /// The left edge, in percent of the highway, of something `width_pct`
    /// wide centred in `lane`.
    pub fn centred_left_pct(&self, lane: usize, width_pct: f32) -> f32 {
        let (start, size) = self.span(lane);
        start + (size - width_pct) * 0.5
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
            white_keys: false,
            lanes: (1..=self.hole_count()).map(|hole| Lane::new(hole.to_string(), None)).collect(),
        }
    }
}

impl InstrumentModel for FrettedInstrument {
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
        FrettedInstrument::pitch_of(self, a)
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
                PlayPosition::Hole(_)
                | PlayPosition::Fingering(_)
                | PlayPosition::Key(_)
                | PlayPosition::Fingerboard(_) => None,
            })
            .collect();
        FrettedInstrument::reconcile(self, anchor, &mut frets);
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
        match self.kind {
            fretted_strings::FrettedKind::Guitar => &[
                TechniqueKind::Target,
                TechniqueKind::Glide,
                TechniqueKind::Register,
                TechniqueKind::Position,
            ],
            fretted_strings::FrettedKind::Cavaquinho => {
                &[TechniqueKind::Target, TechniqueKind::Position]
            }
            fretted_strings::FrettedKind::Bass => {
                &[TechniqueKind::Target, TechniqueKind::Register, TechniqueKind::Position]
            }
        }
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
            white_keys: false,
            lanes: self.string_labels().into_iter().map(|label| Lane::new(label, None)).collect(),
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

    /// The chart's stable fingering order, standard first.
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        wind::positions_for(self, midi).into_iter().map(PlayPosition::Fingering).collect()
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

    /// One row per key, down the tube ([`flute::key_rows`]), travelled
    /// right to left like tab: a note arrives as the shape of its
    /// fingering, a block on every key it presses.
    fn lane_layout(&self) -> LaneLayout {
        LaneLayout {
            axis: ScrollAxis::Horizontal,
            white_keys: false,
            lanes: flute::key_rows(self.foot)
                .into_iter()
                .map(|row| Lane {
                    label: row.key.map(flute::key_label).unwrap_or_default().to_string(),
                    pitch: None,
                    weight: (row.weight * 100.0).round() as u16,
                })
                .collect(),
        }
    }
}

/// Every key is its own pitch, and any keys sound together.
impl InstrumentModel for Piano {
    fn range(&self) -> Option<(u8, u8)> {
        Some((self.lowest(), self.highest()))
    }

    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        if self.has(midi) { vec![PlayPosition::Key(midi)] } else { Vec::new() }
    }

    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        (self.lowest()..=self.highest()).collect()
    }

    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        match position {
            PlayPosition::Key(note) if self.has(*note) => Some(*note),
            _ => None,
        }
    }

    /// Any two different keys: ten fingers and a sustain pedal.
    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool {
        matches!((a, b), (PlayPosition::Key(a), PlayPosition::Key(b)) if a != b)
    }

    fn reconcile(&self, _: &PlayPosition, _: &mut [PlayPosition]) {}

    fn techniques(&self) -> &'static [TechniqueKind] {
        &[TechniqueKind::Target]
    }

    fn expression_scope(&self) -> ExpressionScope {
        ExpressionScope::PerVoice
    }

    fn envelope(&self) -> Envelope {
        Envelope::Decaying
    }

    /// The whole keyboard as rows, highest first. Play 2D narrows it to the
    /// song's own range ([`LaneLayout::keyboard_rows`]).
    fn lane_layout(&self) -> LaneLayout {
        LaneLayout::keyboard_rows(self.lowest(), self.highest())
    }
}

impl InstrumentModel for Instrument {
    fn range(&self) -> Option<(u8, u8)> {
        self.adapter().range()
    }
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition> {
        self.adapter().positions_for(midi)
    }
    fn valid_pitches(&self) -> std::collections::HashSet<u8> {
        self.adapter().valid_pitches()
    }
    fn pitch_of(&self, position: &PlayPosition) -> Option<u8> {
        self.adapter().pitch_of(position)
    }
    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool {
        self.adapter().can_sound_together(a, b)
    }
    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]) {
        self.adapter().reconcile(anchor, others)
    }
    fn techniques(&self) -> &'static [TechniqueKind] {
        self.adapter().techniques()
    }
    fn expression_scope(&self) -> ExpressionScope {
        self.adapter().expression_scope()
    }
    fn envelope(&self) -> Envelope {
        self.adapter().envelope()
    }
    fn lane_layout(&self) -> LaneLayout {
        self.adapter().lane_layout()
    }
}

#[cfg(test)]
mod tests;
