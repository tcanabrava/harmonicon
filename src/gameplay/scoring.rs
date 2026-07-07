// SPDX-License-Identifier: MIT

use super::HitQuality;

pub const PERFECT_POINTS: u32 = 100;
pub const GOOD_POINTS: u32 = 50;

/// A note must be at least this long (seconds) to be a "sustain" note that
/// rewards holding the pitch; shorter notes are scored on their onset alone.
pub const SUSTAIN_MIN_DURATION: f64 = 0.5;
/// Bonus points per second of a long note actually held.
pub const SUSTAIN_POINTS_PER_SEC: f64 = 100.0;

/// The outcome of evaluating a scheduled note at a given instant.
#[derive(Debug, PartialEq)]
pub enum NoteOutcome {
    /// The note is still in the future — not yet inside the scoring window.
    TooEarly,
    /// The note has passed the miss deadline — the combo should reset.
    Missed,
    /// The note is between the good-window and the miss deadline.
    /// Not hittable any more, but not penalised yet.
    Gap,
    /// The note is inside the good window but the player is not playing the
    /// expected pitch right now.
    Waiting,
    /// The player hit the note with the given timing quality.
    Hit(HitQuality),
}

/// Classify a scheduled note.
///
/// `offset` = `clock - note.time` (positive when the clock has passed the note).
/// `playing_expected` is true when the player's current pitch matches the note.
pub fn classify_note(
    offset: f64,
    playing_expected: bool,
    perfect_window: f64,
    good_window: f64,
    miss_window: f64,
) -> NoteOutcome {
    if offset > miss_window {
        return NoteOutcome::Missed;
    }
    if offset < -good_window {
        return NoteOutcome::TooEarly;
    }
    if offset > good_window {
        return NoteOutcome::Gap;
    }
    if !playing_expected {
        return NoteOutcome::Waiting;
    }
    if offset.abs() <= perfect_window {
        NoteOutcome::Hit(HitQuality::Perfect)
    } else {
        NoteOutcome::Hit(HitQuality::Good)
    }
}

/// Score multiplier for the current combo level.
pub fn compute_multiplier(combo: u32, base_mult: f32, step_mult: f32, max_mult: f32) -> f32 {
    (base_mult + combo as f32 * step_mult).min(max_mult)
}

/// Points awarded for a single hit.
pub fn compute_points(quality: HitQuality, multiplier: f32) -> u32 {
    let base = match quality {
        HitQuality::Perfect => PERFECT_POINTS,
        HitQuality::Good => GOOD_POINTS,
    };
    (base as f32 * multiplier) as u32
}

/// Bonus points for sustaining a long note. `held` is the seconds the expected
/// pitch was held after the onset; `duration` is the note's length. Notes shorter
/// than [`SUSTAIN_MIN_DURATION`] aren't sustain notes and earn nothing; held time
/// is capped at the duration so over-holding can't over-score.
pub fn sustain_points(held: f64, duration: f64) -> u32 {
    if duration < SUSTAIN_MIN_DURATION {
        return 0;
    }
    (held.clamp(0.0, duration) * SUSTAIN_POINTS_PER_SEC).round() as u32
}

// ── Sustained-technique validation (vibrato, wah) ───────────────────────────────

/// Minimum pitch swing (cents, peak-to-trough) a held note must show to count
/// as genuine vibrato rather than natural breath-pitch noise.
pub const VIBRATO_MIN_SWING_CENTS: f32 = 15.0;
/// Minimum relative loudness swing a held note must show to count as a hand-wah
/// sweep rather than steady breath pressure. Expressed as a fraction of the
/// note's mean input level, since raw mic gain varies per player/setup.
pub const WAH_MIN_SWING_FRAC: f32 = 0.12;

/// Did `samples` genuinely oscillate — swinging at least `min_swing` with at
/// least one full up/down cycle — rather than holding steady, drifting
/// monotonically, or doing a single one-way bend? Small deltas below 15% of
/// `min_swing` are treated as frame-to-frame jitter and ignored so they don't
/// get counted as spurious direction changes.
pub fn detect_wobble(samples: &[f32], min_swing: f32) -> bool {
    if samples.len() < 6 {
        return false;
    }
    let max = samples.iter().cloned().fold(f32::MIN, f32::max);
    let min = samples.iter().cloned().fold(f32::MAX, f32::min);
    if max - min < min_swing {
        return false;
    }
    let noise_floor = min_swing * 0.15;
    let mut direction = 0i32;
    let mut flips = 0;
    for w in samples.windows(2) {
        let d = w[1] - w[0];
        if d.abs() < noise_floor {
            continue;
        }
        let sign = if d > 0.0 { 1 } else { -1 };
        if direction != 0 && sign != direction {
            flips += 1;
        }
        direction = sign;
    }
    flips >= 2
}

/// Like [`detect_wobble`], but for a signal whose absolute scale is meaningless
/// (input gain varies per mic/player) — normalises to the sample mean first, so
/// `min_frac` is a *relative* swing (e.g. `0.12` = 12% above/below the mean).
pub fn detect_relative_wobble(samples: &[f32], min_frac: f32) -> bool {
    if samples.is_empty() {
        return false;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    if mean <= 0.0001 {
        return false;
    }
    let normalized: Vec<f32> = samples.iter().map(|s| s / mean).collect();
    detect_wobble(&normalized, min_frac)
}

/// True when the combo should reset due to inactivity.
pub fn should_decay_combo(
    combo: u32,
    clock: f64,
    last_hit_time: f64,
    decay_secs: Option<f64>,
) -> bool {
    if combo == 0 {
        return false;
    }
    match decay_secs {
        Some(decay) => clock - last_hit_time > decay,
        None => false,
    }
}

/// HUD label for the current combo. Empty string when combo is ≤ 1.
///
/// `multiplier` must be the same value actually applied to points (i.e. from
/// [`compute_multiplier`] with the chart's configured base/step/max, or `1.0`
/// when combo scoring is disabled) — a separately hardcoded formula here
/// would disagree with the score whenever a chart sets a non-default
/// `step_multiplier`/`max_multiplier`.
pub fn combo_label(combo: u32, multiplier: f32) -> String {
    if combo <= 1 {
        return String::new();
    }
    if multiplier > 1.0 {
        format!("\u{00D7}{combo} [\u{00D7}{} pts]", format_multiplier(multiplier))
    } else {
        format!("\u{00D7}{combo}")
    }
}

/// Render a multiplier without a noisy fractional tail: `2.0` -> `"2"`,
/// `1.25` -> `"1.25"`.
fn format_multiplier(mult: f32) -> String {
    let s = format!("{mult:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sustain_points ────────────────────────────────────────────────────────

    #[test]
    fn short_notes_earn_no_sustain() {
        // Below the threshold: it's an onset-only note.
        assert_eq!(sustain_points(0.3, 0.3), 0);
    }

    #[test]
    fn fully_held_long_note_scores_full_duration() {
        // 1.6 s held fully → 160 points at 100 pts/s.
        assert_eq!(sustain_points(1.6, 1.6), 160);
    }

    #[test]
    fn partial_hold_scores_proportionally() {
        // Held half of a 2 s note → 100 points.
        assert_eq!(sustain_points(1.0, 2.0), 100);
    }

    #[test]
    fn over_holding_is_capped_at_duration() {
        // Holding past the note's end can't score beyond the full duration.
        assert_eq!(sustain_points(5.0, 1.0), 100);
    }

    // ── classify_note ─────────────────────────────────────────────────────────

    #[test]
    fn too_early_before_window() {
        assert_eq!(
            classify_note(-0.20, false, 0.06, 0.13, 0.13),
            NoteOutcome::TooEarly
        );
    }

    #[test]
    fn missed_past_miss_window() {
        assert_eq!(
            classify_note(0.20, false, 0.06, 0.13, 0.13),
            NoteOutcome::Missed
        );
    }

    #[test]
    fn gap_between_good_and_miss_window() {
        // good_window=0.13, miss_window=0.20 → offset 0.15 is in the gap
        assert_eq!(
            classify_note(0.15, false, 0.06, 0.13, 0.20),
            NoteOutcome::Gap
        );
    }

    #[test]
    fn waiting_in_window_but_not_playing() {
        assert_eq!(
            classify_note(0.05, false, 0.06, 0.13, 0.13),
            NoteOutcome::Waiting
        );
    }

    #[test]
    fn perfect_hit_within_perfect_window() {
        assert_eq!(
            classify_note(0.03, true, 0.06, 0.13, 0.13),
            NoteOutcome::Hit(HitQuality::Perfect)
        );
    }

    #[test]
    fn perfect_hit_early_side() {
        assert_eq!(
            classify_note(-0.04, true, 0.06, 0.13, 0.13),
            NoteOutcome::Hit(HitQuality::Perfect)
        );
    }

    #[test]
    fn good_hit_outside_perfect_window_late() {
        assert_eq!(
            classify_note(0.10, true, 0.06, 0.13, 0.13),
            NoteOutcome::Hit(HitQuality::Good)
        );
    }

    #[test]
    fn good_hit_outside_perfect_window_early() {
        assert_eq!(
            classify_note(-0.10, true, 0.06, 0.13, 0.13),
            NoteOutcome::Hit(HitQuality::Good)
        );
    }

    // ── input latency offset ──────────────────────────────────────────────────

    #[test]
    fn latency_offset_turns_late_good_into_perfect() {
        // Without compensation: a note at t=1.0 detected at clock=1.070 has
        // offset +70 ms — just outside the ±60 ms perfect window.
        let raw_offset = 1.070 - 1.0;
        assert_eq!(
            classify_note(raw_offset, true, 0.060, 0.130, 0.130),
            NoteOutcome::Hit(HitQuality::Good),
            "raw offset should be a late Good"
        );

        // With 70 ms compensation: judged = 1.070 - 0.070 = 1.000 → offset 0 ms.
        let judged_offset = (1.070 - 0.070) - 1.0;
        assert_eq!(
            classify_note(judged_offset, true, 0.060, 0.130, 0.130),
            NoteOutcome::Hit(HitQuality::Perfect),
            "compensated offset should be Perfect"
        );
    }

    #[test]
    fn zero_latency_offset_changes_nothing() {
        // offset = 0.0 → compensated offset = 0.0 - 0.0 = 0.0, still Perfect.
        let offset = 0.020 - 0.0; // 20 ms early relative to note
        assert_eq!(
            classify_note(offset - 0.0, true, 0.060, 0.130, 0.130),
            classify_note(offset, true, 0.060, 0.130, 0.130),
        );
    }

    // ── compute_multiplier ────────────────────────────────────────────────────

    #[test]
    fn multiplier_at_zero_combo() {
        assert!((compute_multiplier(0, 1.0, 0.1, 4.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn multiplier_grows_linearly() {
        assert!((compute_multiplier(10, 1.0, 0.1, 4.0) - 2.0).abs() < f32::EPSILON);
        assert!((compute_multiplier(20, 1.0, 0.1, 4.0) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn multiplier_capped_at_max() {
        assert!((compute_multiplier(1000, 1.0, 0.1, 4.0) - 4.0).abs() < f32::EPSILON);
    }

    // ── compute_points ────────────────────────────────────────────────────────

    #[test]
    fn perfect_points_no_multiplier() {
        assert_eq!(compute_points(HitQuality::Perfect, 1.0), 100);
    }

    #[test]
    fn good_points_no_multiplier() {
        assert_eq!(compute_points(HitQuality::Good, 1.0), 50);
    }

    #[test]
    fn points_scale_with_multiplier() {
        assert_eq!(compute_points(HitQuality::Perfect, 2.0), 200);
        assert_eq!(compute_points(HitQuality::Good, 3.0), 150);
    }

    // ── should_decay_combo ────────────────────────────────────────────────────

    #[test]
    fn no_decay_when_combo_is_zero() {
        assert!(!should_decay_combo(0, 10.0, 0.0, Some(2.0)));
    }

    #[test]
    fn no_decay_without_decay_config() {
        assert!(!should_decay_combo(5, 100.0, 0.0, None));
    }

    #[test]
    fn decays_when_gap_exceeds_threshold() {
        assert!(should_decay_combo(5, 5.0, 2.5, Some(2.0)));
    }

    #[test]
    fn no_decay_when_within_threshold() {
        assert!(!should_decay_combo(5, 4.0, 2.5, Some(2.0)));
    }

    // ── combo_label ───────────────────────────────────────────────────────────

    #[test]
    fn empty_for_zero_and_one() {
        assert_eq!(combo_label(0, 1.0), "");
        assert_eq!(combo_label(1, 1.0), "");
    }

    #[test]
    fn simple_label_when_multiplier_is_at_baseline() {
        assert_eq!(combo_label(5, 1.0), "\u{00D7}5");
    }

    #[test]
    fn shows_multiplier_when_above_baseline() {
        let s = combo_label(10, 2.0);
        assert!(s.contains("\u{00D7}10"), "label: {s}");
        assert!(s.contains("\u{00D7}2"), "label: {s}");
    }

    #[test]
    fn shows_fractional_multiplier_from_a_custom_step() {
        // A chart with step_multiplier: 0.25 should show the real value, not
        // the old hardcoded (1 + combo/10).min(4) formula's "2".
        let s = combo_label(10, 1.25);
        assert!(s.contains("\u{00D7}1.25"), "label: {s}");
    }

    #[test]
    fn multiplier_capped_at_four_in_label() {
        let s = combo_label(40, 4.0);
        assert!(s.contains("\u{00D7}4"), "label: {s}");
    }

    // ── format_multiplier ────────────────────────────────────────────────────

    #[test]
    fn format_multiplier_drops_trailing_zeros() {
        assert_eq!(format_multiplier(2.0), "2");
        assert_eq!(format_multiplier(2.5), "2.5");
        assert_eq!(format_multiplier(1.25), "1.25");
    }

    // ── detect_wobble / detect_relative_wobble ──────────────────────────────────

    #[test]
    fn steady_pitch_is_not_wobble() {
        let steady = vec![2.0; 20];
        assert!(!detect_wobble(&steady, VIBRATO_MIN_SWING_CENTS));
    }

    #[test]
    fn single_bend_is_not_wobble() {
        // One smooth ramp down and back up is a single direction change,
        // not a repeating oscillation.
        let mut samples = vec![0.0; 10];
        samples.extend((0..10).map(|i| -i as f32 * 4.0));
        assert!(!detect_wobble(&samples, VIBRATO_MIN_SWING_CENTS));
    }

    #[test]
    fn real_vibrato_is_wobble() {
        // A few full cycles of a sine-like swing, well above the swing floor.
        let samples: Vec<f32> = (0..40)
            .map(|i| 25.0 * (i as f32 * 0.6).sin())
            .collect();
        assert!(detect_wobble(&samples, VIBRATO_MIN_SWING_CENTS));
    }

    #[test]
    fn tiny_swing_is_not_wobble_even_with_direction_changes() {
        // Oscillates, but well under the swing threshold — natural jitter.
        let samples: Vec<f32> = (0..40).map(|i| 2.0 * (i as f32 * 0.6).sin()).collect();
        assert!(!detect_wobble(&samples, VIBRATO_MIN_SWING_CENTS));
    }

    #[test]
    fn too_few_samples_is_not_wobble() {
        assert!(!detect_wobble(&[0.0, 20.0, 0.0], VIBRATO_MIN_SWING_CENTS));
    }

    #[test]
    fn relative_wobble_scales_with_input_level() {
        // Same 20% swing shape at two very different gain levels — both should
        // register, since the check is relative to each signal's own mean.
        let quiet: Vec<f32> = (0..40).map(|i| 0.02 + 0.004 * (i as f32 * 0.6).sin()).collect();
        let loud: Vec<f32> = (0..40).map(|i| 0.5 + 0.10 * (i as f32 * 0.6).sin()).collect();
        assert!(detect_relative_wobble(&quiet, WAH_MIN_SWING_FRAC));
        assert!(detect_relative_wobble(&loud, WAH_MIN_SWING_FRAC));
    }

    #[test]
    fn steady_loudness_is_not_relative_wobble() {
        let steady = vec![0.2; 20];
        assert!(!detect_relative_wobble(&steady, WAH_MIN_SWING_FRAC));
    }
}
