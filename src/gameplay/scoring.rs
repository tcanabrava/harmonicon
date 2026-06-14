// SPDX-License-Identifier: MIT

use super::HitQuality;

pub const PERFECT_POINTS: u32 = 100;
pub const GOOD_POINTS: u32 = 50;

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
pub fn combo_label(combo: u32) -> String {
    if combo <= 1 {
        return String::new();
    }
    let mult = (1 + combo / 10).min(4);
    if mult > 1 {
        format!("\u{00D7}{} [\u{00D7}{} pts]", combo, mult)
    } else {
        format!("\u{00D7}{}", combo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(combo_label(0), "");
        assert_eq!(combo_label(1), "");
    }

    #[test]
    fn simple_label_below_ten() {
        // mult = (1 + 5/10).min(4) = 1 — no multiplier shown
        assert_eq!(combo_label(5), "\u{00D7}5");
    }

    #[test]
    fn shows_multiplier_at_ten() {
        // mult = (1 + 10/10).min(4) = 2
        let s = combo_label(10);
        assert!(s.contains("\u{00D7}10"), "label: {s}");
        assert!(s.contains("\u{00D7}2"), "label: {s}");
    }

    #[test]
    fn multiplier_capped_at_four_in_label() {
        // mult = (1 + 40/10).min(4) = 4
        let s = combo_label(40);
        assert!(s.contains("\u{00D7}4"), "label: {s}");
    }
}