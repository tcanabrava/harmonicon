// SPDX-License-Identifier: MIT
//! The staff's meter, and the one place a time-signature string is read.

use bevy::prelude::*;

/// The staff's meter: what the time signature at the head reads. Bar lines
/// come from [`MusicScoreBarMap`](super::MusicScoreBarMap). Written by whichever bridge is driving the
/// staff; [`Default`] is 4/4, matching what every caller assumed back when
/// the module had no meter at all.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct MusicScoreMeter {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for MusicScoreMeter {
    fn default() -> Self {
        Self { numerator: 4, denominator: 4 }
    }
}

impl MusicScoreMeter {
    /// Quarter-note beats per bar — the unit [`NotationNote::start_beat`](super::NotationNote::start_beat)
    /// counts in, so a 6/8 bar is 3 quarter-note beats, not 6.
    pub fn beats_per_bar(self) -> f64 {
        self.numerator as f64 * 4.0 / self.denominator.max(1) as f64
    }

    /// Ticks spanned by one beat *of this meter* — the unit its upper
    /// number counts, so an eighth in 7/8 and a quarter in 4/4 — given
    /// `quarter_ticks` ticks to a quarter note.
    ///
    /// Exact or nothing, unlike [`beats_per_bar`](Self::beats_per_bar),
    /// whose `f64` the Song Editor had to round to a whole number of
    /// quarters: a 7/8 bar is 3.5 of them, so rounding put its bar line
    /// half a beat out. `None` when the meter's beat isn't a whole number
    /// of ticks (a 32nd-note beat at 12 ticks to the quarter would be 1.5),
    /// which is why [`TIME_SIGNATURES`] stops at /16.
    pub fn ticks_per_beat(self, quarter_ticks: u32) -> Option<u32> {
        let whole_note = quarter_ticks.checked_mul(4)?;
        let denominator = u32::from(self.denominator);
        (denominator > 0 && whole_note.is_multiple_of(denominator))
            .then(|| whole_note / denominator)
    }

    /// Ticks spanned by one bar. `None` under the same conditions as
    /// [`ticks_per_beat`](Self::ticks_per_beat).
    pub fn ticks_per_bar(self, quarter_ticks: u32) -> Option<u32> {
        self.ticks_per_beat(quarter_ticks)?.checked_mul(u32::from(self.numerator))
    }

    /// Seconds spanned by one beat *of this meter* at `bpm` quarter notes a
    /// minute — an eighth's worth in 7/8, a quarter's in 4/4, a half's in
    /// 2/2.
    ///
    /// `bpm` counts quarter notes because that's what a chart's `tempo_bpm`
    /// means everywhere else (`harmonicon_core::chart::tick_to_seconds`
    /// converts `ticks / resolution` quarters at `60 / bpm` seconds each).
    /// This is the one place a metronome should get its beat length from:
    /// a click driven by `60 / bpm` directly clicks quarters whatever the
    /// meter, and a 3/8 bar is one and a half of those — its downbeat
    /// never lands on a click at all.
    pub fn beat_secs(self, bpm: f64) -> f64 {
        60.0 / bpm.max(f64::EPSILON) * 4.0 / f64::from(self.denominator.max(1))
    }

    /// Seconds spanned by one bar at `bpm` quarter notes a minute.
    pub fn bar_secs(self, bpm: f64) -> f64 {
        self.beat_secs(bpm) * f64::from(self.numerator)
    }
}

/// The meters the Song Editor offers, commonest first.
///
/// A fixed list rather than a free-text field because a time signature is
/// not two arbitrary numbers: the lower one names a *note value*, and note
/// values come from halving a whole note (1, 2, 4, 8, 16, …), so only a
/// power of two can appear there. Typing `4/3` asks for four thirds of a
/// whole note, which standard notation has no symbol for — and since
/// [`parse_time_signature`] only rejects what won't parse at all, such a
/// string would be silently rounded into a bar length that matches nothing.
/// Offering the real ones makes that unrepresentable.
///
/// Not exhaustive, and not meant to be — an exhaustive list is every
/// numerator crossed with five denominators, far too many to pick from.
/// `meters_are_all_well_formed` is what keeps this honest.
pub const TIME_SIGNATURES: [&str; 10] =
    ["4/4", "3/4", "2/4", "2/2", "6/8", "9/8", "12/8", "5/4", "7/8", "5/8"];

/// Parses `"6/8"` into its two halves — **the only place in the tree that
/// reads a time-signature string.** Everything that needs a bar length,
/// beat count or beat duration asks the returned [`MusicScoreMeter`]
/// rather than parsing the string itself: `gameplay::bars::chart_meter`
/// for a chart, `EditorState::meter` for the editor. Taking the numerator
/// alone as a beat count would give six for a 6/8 bar that is three
/// quarters long.
///
/// Anything unparseable falls back to 4/4 rather than failing — a chart
/// already on disk with a malformed signature should still open, costing
/// a wrong staff head rather than a crash. What a *user* can enter is
/// constrained upstream by [`TIME_SIGNATURES`].
pub fn parse_time_signature(s: &str) -> MusicScoreMeter {
    let mut parts = s.split('/');
    let numerator = parts.next().and_then(|n| n.trim().parse().ok());
    let denominator = parts.next().and_then(|d| d.trim().parse().ok());
    match (numerator, denominator) {
        (Some(n), Some(d)) if n > 0 && d > 0 => MusicScoreMeter { numerator: n, denominator: d },
        _ => MusicScoreMeter::default(),
    }
}
