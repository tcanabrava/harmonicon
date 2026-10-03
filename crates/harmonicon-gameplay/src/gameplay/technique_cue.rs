// SPDX-License-Identifier: MIT

//! What a technique note asks of the player, in terms they can act on while
//! playing: how far to bend and which note to land on, how fast to wobble a
//! vibrato or a wah — and, while the note is live, how the player's own
//! pitch or loudness compares with that.
//!
//! Pure functions only. The 2D highway draws the cue beside each note head
//! ([`note_cue`]) and drives the hit-line gauge (`technique_coach`) from
//! the readings here, so the words and the gauge can't disagree about what
//! the note wants.

use harmonicon_core::chart::Modifier;
use harmonicon_core::midi::{midi_to_freq_hz, midi_to_note};
use harmonicon_core::scoring::{
    VIBRATO_MIN_SWING_CENTS, WAH_MIN_SWING_FRAC, measured_oscillation_hz,
    measured_relative_oscillation_hz, oscillation_matches_rate,
};
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::judge::OSCILLATION_RATE_TOLERANCE_FRAC;
use super::notes::ScheduledNote;

/// Pitch-class name of a MIDI note with no octave (`A`, `C#`): a player
/// aims a bend at "A", and the hole already fixes which A.
pub(super) fn pitch_class_name(midi: u8) -> String {
    midi_to_note(i32::from(midi))
        .trim_end_matches(|c: char| c.is_ascii_digit() || c == '-')
        .to_string()
}

/// A bend's depth in whole semitones (at least one), rounded the same way
/// `notes::target_pitch` rounds it into the expected pitch.
pub(super) fn bend_semitones(modifiers: &[Modifier]) -> Option<u8> {
    modifiers.iter().find_map(|m| match m {
        Modifier::Bend { semitones, .. } => Some((semitones.abs().round() as u8).max(1)),
        _ => None,
    })
}

/// The unbent pitch a bent note starts from: the expected (bent) pitch with
/// the bend taken back off.
pub(super) fn natural_pitch(expected: u8, modifiers: &[Modifier]) -> Option<u8> {
    let shift = modifiers.iter().find_map(|m| match m {
        Modifier::Bend { semitones, .. } => Some(semitones.round() as i32),
        _ => None,
    })?;
    u8::try_from(i32::from(expected) - shift).ok()
}

/// An oscillation rate as a player reads it: `5`, not `5.0`; `4.5` stays.
pub(super) fn format_rate(hz: f32) -> String {
    if (hz - hz.round()).abs() < 0.05 { format!("{:.0}", hz.round()) } else { format!("{hz:.1}") }
}

/// One clause of a note's cue: a Fluent key and the arguments it takes.
#[derive(Debug, PartialEq)]
pub(super) struct CuePart {
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
}

/// The cue clauses for a note, in modifier order — as short as they can be
/// and still say what to do, since they are read while scrolling. The bend
/// depth is already on the head (`-3''`), so a pitch technique's clause is
/// only the note to land on; a wobble's is its rate. A note the harp can't
/// produce (`expected` is `None`) gets no pitch clause: nothing to aim at.
pub(super) fn cue_parts(modifiers: &[Modifier], expected: Option<u8>) -> Vec<CuePart> {
    let mut parts: Vec<CuePart> = Vec::new();
    if let Some(midi) = expected
        && modifiers.iter().any(|m| {
            matches!(
                m,
                Modifier::Bend { .. } | Modifier::Overblow | Modifier::Overdraw | Modifier::Slide
            )
        })
    {
        parts.push(CuePart { key: "cue-target", args: vec![("note", pitch_class_name(midi))] });
    }
    parts.extend(modifiers.iter().filter_map(|m| match m {
        Modifier::Vibrato { oscillation_hz, .. } => {
            Some(CuePart { key: "cue-vibrato", args: vec![("rate", format_rate(*oscillation_hz))] })
        }
        Modifier::WahWah { oscillation_hz, .. } => {
            Some(CuePart { key: "cue-wah", args: vec![("rate", format_rate(*oscillation_hz))] })
        }
        _ => None,
    }));
    parts
}

/// Whether the technique coach has anything to show for this modifier.
pub(super) fn is_coachable(modifier: &Modifier) -> bool {
    matches!(modifier, Modifier::Bend { .. } | Modifier::Vibrato { .. } | Modifier::WahWah { .. })
}

/// The localized cue drawn beside a note head, or `None` for a plain note.
pub(super) fn note_cue(loc: &Localization, note: &ScheduledNote) -> Option<String> {
    let parts = cue_parts(&note.modifiers, note.expected_pitch);
    if parts.is_empty() {
        return None;
    }
    let text: Vec<String> = parts
        .iter()
        .map(|p| {
            let args: Vec<(&str, String)> = p.args.iter().map(|(k, v)| (*k, v.clone())).collect();
            String::from(loc.msg_args(p.key, &args))
        })
        .collect();
    Some(text.join("  "))
}

// ── Live coaching ────────────────────────────────────────────────────────────

/// How long before a technique note reaches the hit line its gauge appears —
/// enough to read the target before the attack, short enough that the gauge
/// belongs to the note arriving rather than one further up.
pub(super) const COACH_LEAD_SECS: f64 = 0.75;

/// What the hit-line gauge coaches for one note. A bend wins over a vibrato
/// or wah on the same note: landing the pitch comes first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CoachMode {
    Bend { natural: u8, target: u8 },
    Vibrato { hz: f32 },
    Wah { hz: f32 },
}

pub(super) fn coach_mode(note: &ScheduledNote) -> Option<CoachMode> {
    if let Some(target) = note.expected_pitch
        && bend_semitones(&note.modifiers).is_some()
        && let Some(natural) = natural_pitch(target, &note.modifiers)
        && natural != target
    {
        return Some(CoachMode::Bend { natural, target });
    }
    note.modifiers.iter().find_map(|m| match m {
        Modifier::Vibrato { oscillation_hz, .. } => {
            Some(CoachMode::Vibrato { hz: *oscillation_hz })
        }
        Modifier::WahWah { oscillation_hz, .. } => Some(CoachMode::Wah { hz: *oscillation_hz }),
        _ => None,
    })
}

/// The note the gauge follows at `judged`: the earliest still-live note with
/// something to coach, from [`COACH_LEAD_SECS`] before it arrives until it
/// is missed or its hold ends. `notes` is sorted by time, so the scan stops
/// at the first note too far ahead.
pub(super) fn coach_note(notes: &[ScheduledNote], cursor: usize, judged: f64) -> Option<usize> {
    for (i, note) in notes.iter().enumerate().skip(cursor) {
        if note.time - COACH_LEAD_SECS > judged {
            break;
        }
        if note.missed || !note.playable || judged > note.time + note.duration {
            continue;
        }
        if coach_mode(note).is_some() {
            return Some(i);
        }
    }
    None
}

/// What to tell a player bending toward a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BendAdvice {
    /// Nothing near the bend's range is sounding.
    Silent,
    BendMore,
    OnTarget,
    TooFar,
}

/// Where the player's pitch sits on a bend: `position` is 0 at the unbent
/// note and 1 at the target (beyond 1 is an overshoot).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BendReading {
    pub position: Option<f32>,
    pub advice: BendAdvice,
}

/// Half a semitone either side of the target is the same note to the judge
/// (pitches compare as rounded MIDI numbers), so it is "on target" here too.
const BEND_ON_TARGET_CENTS: f32 = 50.0;

/// How far outside the natural-to-target span a heard pitch may lie and
/// still be read as this bend rather than some other note or a harmonic.
const BEND_CAPTURE_CENTS: f32 = 100.0;

fn cents_above(hz: f32, midi: u8) -> f32 {
    1200.0 * (hz / midi_to_freq_hz(f32::from(midi))).log2()
}

/// Reads the heard frequencies against a bend from `natural` to `target`,
/// picking the one closest to the target among those inside the bend's
/// range.
pub(super) fn bend_reading(
    heard_hz: impl IntoIterator<Item = f32>,
    natural: u8,
    target: u8,
) -> BendReading {
    let span = (f32::from(target) - f32::from(natural)) * 100.0;
    let (lo, hi) = (span.min(0.0), span.max(0.0));
    let best = heard_hz
        .into_iter()
        .filter(|hz| *hz > 0.0)
        .map(|hz| cents_above(hz, natural))
        .filter(|c| *c >= lo - BEND_CAPTURE_CENTS && *c <= hi + BEND_CAPTURE_CENTS)
        .min_by(|a, b| (a - span).abs().total_cmp(&(b - span).abs()));
    let Some(cents) = best else {
        return BendReading { position: None, advice: BendAdvice::Silent };
    };
    let miss = cents - span;
    let advice = if miss.abs() <= BEND_ON_TARGET_CENTS {
        BendAdvice::OnTarget
    } else if miss.signum() == span.signum() {
        BendAdvice::TooFar
    } else {
        BendAdvice::BendMore
    };
    BendReading { position: Some(cents / span), advice }
}

/// The on-target band of a bend, in the same 0-to-1 position units.
pub(super) fn bend_target_band(natural: u8, target: u8) -> (f32, f32) {
    let span = (f32::from(target) - f32::from(natural)).abs() * 100.0;
    let half = BEND_ON_TARGET_CENTS / span;
    (1.0 - half, 1.0 + half)
}

/// What to tell a player wobbling a vibrato or wah.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RateAdvice {
    /// Not enough of the hold heard yet to judge — follow the pulse.
    FollowPulse,
    /// Heard, but the swing is too small to count as a wobble.
    SwingMore,
    Faster,
    Slower,
    OnRate,
}

/// Cycles of the charted rate that must be heard before a rate verdict is
/// worth showing. The measurement needs two direction reversals, which a
/// correct wobble only completes after a full cycle; any sooner and every
/// slow wah would open on "Wider" while it is being played right.
const CYCLES_BEFORE_ADVICE: f32 = 1.5;

/// The rate verdict, on the same tolerance the judge pays out on, once
/// `heard_secs` of the hold cover [`CYCLES_BEFORE_ADVICE`] at `target_hz`.
pub(super) fn rate_advice(measured_hz: Option<f32>, target_hz: f32, heard_secs: f64) -> RateAdvice {
    if heard_secs * f64::from(target_hz) < f64::from(CYCLES_BEFORE_ADVICE) {
        return RateAdvice::FollowPulse;
    }
    match measured_hz {
        None => RateAdvice::SwingMore,
        Some(hz) if oscillation_matches_rate(hz, target_hz, OSCILLATION_RATE_TOLERANCE_FRAC) => {
            RateAdvice::OnRate
        }
        Some(hz) if hz < target_hz => RateAdvice::Faster,
        Some(_) => RateAdvice::Slower,
    }
}

/// Cents from centre that fill the vibrato gauge: twice the judge's
/// minimum peak-to-trough swing, so a just-qualifying wobble reaches a
/// quarter of the way out and a clear one most of it.
const VIBRATO_DISPLAY_CENTS: f32 = VIBRATO_MIN_SWING_CENTS * 2.0;
/// Relative loudness that fills the wah gauge, by the same rule.
const WAH_DISPLAY_FRAC: f32 = WAH_MIN_SWING_FRAC * 2.0;

/// Where the reference pulse sits, in -1..1, `since` seconds after the
/// note's onset: a sine at the chart's rate, the wobble to copy.
pub(super) fn reference_swing(hz: f32, since: f64) -> f32 {
    ((std::f64::consts::TAU * f64::from(hz) * since).sin() * 0.8) as f32
}

/// The player's latest vibrato deflection in -1..1, centred on their own
/// average pitch so a slightly sharp or flat hold still reads as centred.
pub(super) fn vibrato_swing(pitch_samples: &[(f64, f32)]) -> Option<f32> {
    let &(_, last) = pitch_samples.last()?;
    let mean = pitch_samples.iter().map(|&(_, c)| c).sum::<f32>() / pitch_samples.len() as f32;
    Some(((last - mean) / VIBRATO_DISPLAY_CENTS).clamp(-1.0, 1.0))
}

/// The player's latest wah deflection in -1..1, relative to the hold's
/// average loudness (mic gain varies too much for an absolute scale).
pub(super) fn wah_swing(amp_samples: &[(f64, f32)]) -> Option<f32> {
    let &(_, last) = amp_samples.last()?;
    let mean = amp_samples.iter().map(|&(_, a)| a).sum::<f32>() / amp_samples.len() as f32;
    if mean <= 0.0001 {
        return None;
    }
    Some(((last / mean - 1.0) / WAH_DISPLAY_FRAC).clamp(-1.0, 1.0))
}

/// The half-width of the swing a wobble must exceed to count, in the same
/// -1..1 gauge units — the judge's minimum peak-to-trough, halved.
pub(super) fn min_swing_band(mode: CoachMode) -> f32 {
    match mode {
        CoachMode::Vibrato { .. } => VIBRATO_MIN_SWING_CENTS * 0.5 / VIBRATO_DISPLAY_CENTS,
        CoachMode::Wah { .. } => WAH_MIN_SWING_FRAC * 0.5 / WAH_DISPLAY_FRAC,
        CoachMode::Bend { .. } => 0.0,
    }
}

/// The measured wobble rate for a held note and how many seconds of hold it
/// was measured over, from the samples the judge verifies the technique
/// with.
pub(super) fn measured_rate(mode: CoachMode, note: &ScheduledNote) -> (Option<f32>, f64) {
    let span = |samples: &[(f64, f32)]| match (samples.first(), samples.last()) {
        (Some(&(first, _)), Some(&(last, _))) => last - first,
        _ => 0.0,
    };
    match mode {
        CoachMode::Vibrato { .. } => (
            measured_oscillation_hz(&note.pitch_samples, VIBRATO_MIN_SWING_CENTS),
            span(&note.pitch_samples),
        ),
        CoachMode::Wah { .. } => (
            measured_relative_oscillation_hz(&note.amp_samples, WAH_MIN_SWING_FRAC),
            span(&note.amp_samples),
        ),
        CoachMode::Bend { .. } => (None, 0.0),
    }
}

// ── Gauge geometry ───────────────────────────────────────────────────────────

/// The span of bend positions the gauge track shows: a little above the
/// unbent note, and past the far edge of the on-target band so an overshoot
/// stays visible. A half-step bend's band reaches half a step past the
/// target, so the range grows with it rather than clipping it.
pub(super) fn bend_track_range(natural: u8, target: u8) -> (f32, f32) {
    let (_, band_hi) = bend_target_band(natural, target);
    (-0.25, (band_hi + 0.1).max(1.35))
}

/// Top offset (percent of the track) for a bend position within `range`:
/// the unbent note near the top, the target near the bottom — bending
/// lowers the pitch, so the marker moves down as the player bends.
pub(super) fn bend_track_pct(position: f32, (lo, hi): (f32, f32)) -> f32 {
    (position.clamp(lo, hi) - lo) / (hi - lo) * 100.0
}

/// Top offset (percent of the track) for a -1..1 swing, up being positive.
pub(super) fn swing_track_pct(swing: f32) -> f32 {
    50.0 - swing.clamp(-1.0, 1.0) * 40.0
}

/// The 1-based lane beside `hole` that a note's cue and gauge use: the one
/// to its right, or to its left for the last hole so it stays on the
/// highway.
pub(super) fn beside_lane(hole: u8, hole_count: u8) -> u8 {
    if hole < hole_count { hole + 1 } else { hole.saturating_sub(1).max(1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bend(semitones: f32) -> Modifier {
        Modifier::Bend { semitones, intensity: None }
    }

    fn vibrato(hz: f32) -> Modifier {
        Modifier::Vibrato { oscillation_hz: hz, intensity: None }
    }

    fn note(
        time: f64,
        duration: f64,
        expected: Option<u8>,
        modifiers: Vec<Modifier>,
    ) -> ScheduledNote {
        ScheduledNote {
            time,
            duration,
            hole: 3,
            is_blow: false,
            expected_pitch: expected,
            modifiers,
            ..Default::default()
        }
    }

    #[test]
    fn pitch_class_drops_the_octave() {
        assert_eq!(pitch_class_name(69), "A");
        assert_eq!(pitch_class_name(70), "A#");
        assert_eq!(pitch_class_name(0), "C");
    }

    #[test]
    fn natural_pitch_undoes_the_bend() {
        // Draw 3 on a C harp is B4 (71); a whole-step bend lands on A4 (69).
        assert_eq!(natural_pitch(69, &[bend(-2.0)]), Some(71));
        assert_eq!(natural_pitch(69, &[vibrato(5.0)]), None);
        assert_eq!(bend_semitones(&[bend(-1.4)]), Some(1));
        assert_eq!(bend_semitones(&[bend(-0.2)]), Some(1));
    }

    #[test]
    fn rates_read_without_a_useless_decimal() {
        assert_eq!(format_rate(5.0), "5");
        assert_eq!(format_rate(4.5), "4.5");
        assert_eq!(format_rate(5.98), "6");
    }

    #[test]
    fn a_pitch_cue_names_only_the_note_to_land_on() {
        let parts = cue_parts(&[bend(-2.0)], Some(69));
        assert_eq!(
            parts,
            vec![CuePart { key: "cue-target", args: vec![("note", "A".to_string())] }]
        );
        // Target first, then the wobble, whatever the modifier order.
        let both = cue_parts(&[vibrato(5.0), bend(-1.0)], Some(70));
        let keys: Vec<_> = both.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["cue-target", "cue-vibrato"]);
    }

    #[test]
    fn a_wobble_cue_names_its_rate_and_needs_no_pitch() {
        let parts = cue_parts(&[vibrato(5.0)], None);
        assert_eq!(parts[0].key, "cue-vibrato");
        assert_eq!(parts[0].args, vec![("rate", "5".to_string())]);
    }

    #[test]
    fn an_unproducible_pitch_gets_no_target_clause() {
        assert!(cue_parts(&[bend(-1.0)], None).is_empty());
        assert!(cue_parts(&[Modifier::Overblow], None).is_empty());
        assert_eq!(cue_parts(&[Modifier::Overblow], Some(63))[0].key, "cue-target");
    }

    #[test]
    fn a_plain_note_has_no_cue() {
        assert!(cue_parts(&[], Some(60)).is_empty());
    }

    #[test]
    fn the_bend_outranks_a_wobble_for_the_gauge() {
        let both = note(0.0, 1.0, Some(69), vec![vibrato(5.0), bend(-2.0)]);
        assert_eq!(coach_mode(&both), Some(CoachMode::Bend { natural: 71, target: 69 }));
        let wobble = note(0.0, 1.0, Some(69), vec![vibrato(5.0)]);
        assert_eq!(coach_mode(&wobble), Some(CoachMode::Vibrato { hz: 5.0 }));
        assert_eq!(coach_mode(&note(0.0, 1.0, Some(69), vec![])), None);
    }

    #[test]
    fn the_gauge_follows_the_live_technique_note() {
        let notes = vec![
            note(0.0, 0.5, Some(60), vec![]),
            note(1.0, 1.0, Some(69), vec![bend(-2.0)]),
            note(3.0, 1.0, Some(69), vec![vibrato(5.0)]),
        ];
        // Too early for either technique note.
        assert_eq!(coach_note(&notes, 0, 0.0), None);
        // Within the lead of the bend.
        assert_eq!(coach_note(&notes, 0, 0.5), Some(1));
        // Through the bend's hold.
        assert_eq!(coach_note(&notes, 0, 1.9), Some(1));
        // Between them: the bend is over and the vibrato not yet close.
        assert_eq!(coach_note(&notes, 0, 2.1), None);
        assert_eq!(coach_note(&notes, 0, 2.5), Some(2));

        let mut missed = notes.clone();
        missed[1].missed = true;
        assert_eq!(coach_note(&missed, 0, 1.2), None);
    }

    fn hz(midi: f32) -> f32 {
        midi_to_freq_hz(midi)
    }

    #[test]
    fn a_bend_reading_places_the_pitch_between_natural_and_target() {
        // Whole-step bend B4 -> A4.
        let unbent = bend_reading([hz(71.0)], 71, 69);
        assert_eq!(unbent.advice, BendAdvice::BendMore);
        assert!(unbent.position.unwrap().abs() < 0.01);

        let halfway = bend_reading([hz(70.0)], 71, 69);
        assert_eq!(halfway.advice, BendAdvice::BendMore);
        assert!((halfway.position.unwrap() - 0.5).abs() < 0.01);

        let there = bend_reading([hz(69.1)], 71, 69);
        assert_eq!(there.advice, BendAdvice::OnTarget);

        let past = bend_reading([hz(68.3)], 71, 69);
        assert_eq!(past.advice, BendAdvice::TooFar);
        assert!(past.position.unwrap() > 1.0);
    }

    #[test]
    fn a_bend_reading_ignores_pitches_outside_the_bend() {
        // An octave harmonic and an unrelated note are not the bend.
        let r = bend_reading([hz(83.0), hz(60.0)], 71, 69);
        assert_eq!(r.advice, BendAdvice::Silent);
        assert_eq!(r.position, None);
        // Among several candidates, the one nearest the target wins.
        let r = bend_reading([hz(71.0), hz(69.0)], 71, 69);
        assert_eq!(r.advice, BendAdvice::OnTarget);
    }

    #[test]
    fn the_on_target_band_matches_the_judges_rounding() {
        let (lo, hi) = bend_target_band(71, 69);
        assert!((lo - 0.75).abs() < 1e-6 && (hi - 1.25).abs() < 1e-6);
    }

    #[test]
    fn rate_advice_waits_then_points_the_way() {
        // 1.5 cycles at 5/s is 0.3 s; at 3/s, 0.5 s.
        assert_eq!(rate_advice(Some(5.0), 5.0, 0.2), RateAdvice::FollowPulse);
        assert_eq!(rate_advice(None, 3.0, 0.4), RateAdvice::FollowPulse);
        assert_eq!(rate_advice(None, 5.0, 0.4), RateAdvice::SwingMore);
        assert_eq!(rate_advice(Some(2.0), 5.0, 1.0), RateAdvice::Faster);
        assert_eq!(rate_advice(Some(9.0), 5.0, 1.0), RateAdvice::Slower);
        assert_eq!(rate_advice(Some(5.5), 5.0, 1.0), RateAdvice::OnRate);
    }

    #[test]
    fn swings_are_centred_on_the_players_own_average() {
        let samples = [(0.0, 10.0), (0.1, 10.0), (0.2, 40.0)];
        // Mean 20, last 40 -> +20 cents of a 30-cent scale.
        let s = vibrato_swing(&samples).unwrap();
        assert!((s - 20.0 / VIBRATO_DISPLAY_CENTS).abs() < 1e-6);
        assert_eq!(vibrato_swing(&[]), None);
        assert_eq!(wah_swing(&[(0.0, 0.0)]), None);
        let w = wah_swing(&[(0.0, 0.1), (0.1, 0.3)]).unwrap();
        assert!(w > 0.0);
    }

    #[test]
    fn the_reference_pulse_runs_at_the_charted_rate() {
        // A quarter cycle in is the peak.
        assert!((reference_swing(5.0, 0.05) - 0.8).abs() < 1e-4);
        assert!(reference_swing(5.0, 0.0).abs() < 1e-6);
    }

    #[test]
    fn gauge_geometry_keeps_the_marker_on_the_track() {
        let whole = bend_track_range(71, 69);
        assert_eq!(whole, (-0.25, 1.35));
        assert!((bend_track_pct(-0.25, whole)).abs() < 1e-4);
        assert!((bend_track_pct(1.35, whole) - 100.0).abs() < 1e-4);
        assert!(bend_track_pct(0.0, whole) < bend_track_pct(1.0, whole));
        assert_eq!(bend_track_pct(9.0, whole), 100.0);
        // A half step's band runs to 1.5; the track still shows all of it.
        let half = bend_track_range(71, 70);
        assert!(bend_track_pct(1.5, half) < 100.0);
        assert_eq!(swing_track_pct(0.0), 50.0);
        assert_eq!(swing_track_pct(1.0), 10.0);
        assert_eq!(swing_track_pct(-5.0), 90.0);
    }

    #[test]
    fn the_beside_lane_stays_on_the_highway() {
        assert_eq!(beside_lane(1, 10), 2);
        assert_eq!(beside_lane(9, 10), 10);
        assert_eq!(beside_lane(10, 10), 9);
        assert_eq!(beside_lane(1, 1), 1);
    }
}
