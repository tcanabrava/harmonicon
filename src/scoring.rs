// SPDX-License-Identifier: MIT

//! Pure scoring math shared by every mode that judges a player's timing and
//! pitch against a schedule of notes: the main scored gameplay screens
//! (`crate::gameplay`) and the Song Editor's Practice mode
//! (`crate::song_editor`). Everything here is deliberately free of ECS
//! types/resources so both call sites can use it without depending on each
//! other's UI/HUD code.

use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HitQuality {
    Perfect,
    Good,
}

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

// ── Clean attack ─────────────────────────────────────────────────────────────

/// True when `expected` is the *only* harp-producible pitch sounding right
/// now — a note landed on time and on pitch still fails this if a breathy
/// multi-hole leak also sounded a second, unintended pitch alongside it.
/// `harp_pitches` must already be filtered to pitches the harp can actually
/// produce (e.g. `gameplay::score_notes`'s own `harp_pitches`, itself
/// `ActivePitches` intersected with `ValidHarpNotes`) — unfiltered ambient
/// noise picked up by the mic isn't something the player could have avoided
/// and shouldn't count against them.
pub fn is_clean_attack(harp_pitches: &HashSet<u8>, expected: u8) -> bool {
    harp_pitches.len() == 1 && harp_pitches.contains(&expected)
}

// ── Chord target ─────────────────────────────────────────────────────────────

/// True when every pitch in `expected` (a chord or octave-split target — two
/// or more holes meant to sound at once) is simultaneously present in
/// `harp_pitches` right now. Unlike a single note's exact-one-pitch match,
/// playing the same pitches one at a time doesn't satisfy this — they must
/// all be sounding together. `expected` empty is never "sounding" (an empty
/// chord target is a caller bug, not a trivially-satisfied one).
pub fn chord_is_sounding(expected: &[u8], harp_pitches: &HashSet<u8>) -> bool {
    !expected.is_empty() && expected.iter().all(|m| harp_pitches.contains(m))
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

/// Timestamps of `min_swing`-qualifying direction reversals in `values`
/// (paired 1:1 with `times`), or `None` if there aren't enough samples or the
/// swing never reaches `min_swing` peak-to-trough. Shared by the rate-matching
/// checks below. Deltas below 15% of `min_swing` are treated as
/// frame-to-frame jitter and ignored so they don't get counted as spurious
/// direction changes.
fn wobble_flip_times(values: &[f32], times: &[f64], min_swing: f32) -> Option<Vec<f64>> {
    if values.len() < 6 {
        return None;
    }
    let max = values.iter().cloned().fold(f32::MIN, f32::max);
    let min = values.iter().cloned().fold(f32::MAX, f32::min);
    if max - min < min_swing {
        return None;
    }
    let noise_floor = min_swing * 0.15;
    let mut direction = 0i32;
    let mut flip_times = Vec::new();
    for i in 1..values.len() {
        let d = values[i] - values[i - 1];
        if d.abs() < noise_floor {
            continue;
        }
        let sign = if d > 0.0 { 1 } else { -1 };
        if direction != 0 && sign != direction {
            flip_times.push(times[i]);
        }
        direction = sign;
    }
    Some(flip_times)
}

/// Estimated oscillation rate (Hz) from the real elapsed time between
/// direction reversals in timestamped `samples` — frame-rate independent,
/// unlike counting flips over a fixed number of samples. `None` if `samples`
/// never wobbles enough to qualify as oscillation at all (fewer than two
/// reversals, or peak-to-trough swing under `min_swing`).
pub fn measured_oscillation_hz(samples: &[(f64, f32)], min_swing: f32) -> Option<f32> {
    let times: Vec<f64> = samples.iter().map(|&(t, _)| t).collect();
    let values: Vec<f32> = samples.iter().map(|&(_, v)| v).collect();
    let flips = wobble_flip_times(&values, &times, min_swing)?;
    if flips.len() < 2 {
        return None;
    }
    // Consecutive reversals are half a cycle apart; average them then double
    // the period to get a full-cycle rate.
    let half_periods: Vec<f64> = flips.windows(2).map(|w| w[1] - w[0]).collect();
    let mean_half_period = half_periods.iter().sum::<f64>() / half_periods.len() as f64;
    if mean_half_period <= 0.0 {
        return None;
    }
    Some((1.0 / (2.0 * mean_half_period)) as f32)
}

/// Like [`measured_oscillation_hz`], but for a signal whose absolute scale is
/// meaningless (input gain varies per mic/player) — normalises to the sample
/// mean first, so `min_frac` is a *relative* swing.
pub fn measured_relative_oscillation_hz(samples: &[(f64, f32)], min_frac: f32) -> Option<f32> {
    if samples.is_empty() {
        return None;
    }
    let mean = samples.iter().map(|&(_, v)| v).sum::<f32>() / samples.len() as f32;
    if mean <= 0.0001 {
        return None;
    }
    let normalized: Vec<(f64, f32)> = samples.iter().map(|&(t, v)| (t, v / mean)).collect();
    measured_oscillation_hz(&normalized, min_frac)
}

/// True when `measured_hz` is within `tolerance_frac` of `target_hz` (e.g.
/// `0.4` = ±40%) — generous, since hand vibrato/wah speed varies naturally
/// between players and even between notes.
pub fn oscillation_matches_rate(measured_hz: f32, target_hz: f32, tolerance_frac: f32) -> bool {
    if target_hz <= 0.0 {
        return false;
    }
    let lower = target_hz * (1.0 - tolerance_frac);
    let upper = target_hz * (1.0 + tolerance_frac);
    (lower..=upper).contains(&measured_hz)
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
        format!(
            "\u{00D7}{combo} [\u{00D7}{} pts]",
            format_multiplier(multiplier)
        )
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

// ── Attack gate ──────────────────────────────────────────────────────────────

/// Enforces a fresh attack per key. A sustained pitch (or, for Practice mode,
/// a sustained frequency match against a scheduled note) may satisfy only
/// **one** note: once it scores, the key is "consumed" and cannot score again
/// until the condition that made it fresh (the pitch/frequency sounding)
/// stops holding and starts again. Without this, a single held breath would
/// clear every note that later scrolls into its hit window at the same pitch.
///
/// Generic over `K` so both consumers can key it the way that's natural for
/// them: `crate::gameplay` keys on the detected MIDI pitch (`u8`) since it
/// compares against a live, continuously-updated "currently playing" set;
/// the Song Editor's Practice mode keys on the note's index in its own
/// schedule (`usize`), since "is it still playing" there means re-checking
/// that specific note's expected frequency against the detected pitches.
#[derive(Default)]
pub struct AttackGate<K> {
    consumed: std::collections::HashSet<K>,
}

impl<K: Eq + std::hash::Hash + Copy> AttackGate<K> {
    /// Drop consumption for any key whose condition no longer holds, so its
    /// next articulation counts as a fresh attack. `is_playing` decides, for
    /// each already-consumed key, whether it's still sounding right now.
    pub fn release_absent(&mut self, mut is_playing: impl FnMut(K) -> bool) {
        self.consumed.retain(|&k| is_playing(k));
    }

    /// True if `key` is currently playing (per the caller-supplied `playing`
    /// flag) and has not already scored during its current, continuous
    /// sustain.
    pub fn is_fresh(&self, key: K, playing: bool) -> bool {
        playing && !self.consumed.contains(&key)
    }

    /// Mark `key` as having scored; it won't score another note until it is
    /// released (see [`release_absent`](Self::release_absent)) and replayed.
    pub fn consume(&mut self, key: K) {
        self.consumed.insert(key);
    }
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

    // ── is_clean_attack ───────────────────────────────────────────────────────

    #[test]
    fn clean_when_only_the_expected_pitch_sounds() {
        let harp_pitches = HashSet::from([60u8]);
        assert!(is_clean_attack(&harp_pitches, 60));
    }

    #[test]
    fn dirty_when_another_harp_pitch_sounds_alongside_it() {
        // A breathy attack that leaks into an adjacent hole — the expected
        // pitch is present, but so is a second one.
        let harp_pitches = HashSet::from([60u8, 64u8]);
        assert!(!is_clean_attack(&harp_pitches, 60));
    }

    #[test]
    fn dirty_when_the_expected_pitch_is_not_even_the_one_present() {
        let harp_pitches = HashSet::from([64u8]);
        assert!(!is_clean_attack(&harp_pitches, 60));
    }

    #[test]
    fn dirty_when_nothing_is_sounding() {
        assert!(!is_clean_attack(&HashSet::new(), 60));
    }

    // ── chord_is_sounding ─────────────────────────────────────────────────────

    #[test]
    fn chord_sounds_when_every_pitch_is_present_at_once() {
        let harp_pitches = HashSet::from([60u8, 64u8]);
        assert!(chord_is_sounding(&[60, 64], &harp_pitches));
    }

    #[test]
    fn chord_extra_pitches_dont_disqualify_it() {
        // A third, unrelated pitch sounding alongside the target chord
        // doesn't unmake the chord — unlike `is_clean_attack`, this isn't a
        // single-note precision check.
        let harp_pitches = HashSet::from([60u8, 64u8, 67u8]);
        assert!(chord_is_sounding(&[60, 64], &harp_pitches));
    }

    #[test]
    fn chord_is_not_sounding_when_only_part_of_it_plays() {
        let harp_pitches = HashSet::from([60u8]);
        assert!(!chord_is_sounding(&[60, 64], &harp_pitches));
    }

    #[test]
    fn chord_is_not_sounding_when_nothing_plays() {
        assert!(!chord_is_sounding(&[60, 64], &HashSet::new()));
    }

    #[test]
    fn an_empty_chord_target_never_sounds() {
        let harp_pitches = HashSet::from([60u8, 64u8]);
        assert!(!chord_is_sounding(&[], &harp_pitches));
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
        // A chart with step_multiplier: 0.25 should show the real,
        // fractional multiplier value.
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

    // ── measured_oscillation_hz / measured_relative_oscillation_hz ──────────────

    // Timestamped sine samples at `freq_hz`, `n` samples spaced `dt` seconds apart.
    fn timestamped_sine(freq_hz: f32, amplitude: f32, n: usize, dt: f64) -> Vec<(f64, f32)> {
        (0..n)
            .map(|i| {
                let t = i as f64 * dt;
                let v = amplitude * (2.0 * std::f32::consts::PI * freq_hz * t as f32).sin();
                (t, v)
            })
            .collect()
    }

    #[test]
    fn steady_pitch_is_not_wobble() {
        let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64, 2.0)).collect();
        assert_eq!(
            measured_oscillation_hz(&steady, VIBRATO_MIN_SWING_CENTS),
            None
        );
    }

    #[test]
    fn single_bend_is_not_wobble() {
        // One smooth ramp down and back up is a single direction change,
        // not a repeating oscillation.
        let mut values = vec![0.0; 10];
        values.extend((0..10).map(|i| -i as f32 * 4.0));
        let samples: Vec<(f64, f32)> = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as f64, v))
            .collect();
        assert_eq!(
            measured_oscillation_hz(&samples, VIBRATO_MIN_SWING_CENTS),
            None
        );
    }

    #[test]
    fn tiny_swing_is_not_wobble_even_with_direction_changes() {
        // Oscillates, but well under the swing threshold — natural jitter.
        let samples = timestamped_sine(5.0, 2.0, 40, 1.0 / 60.0);
        assert_eq!(
            measured_oscillation_hz(&samples, VIBRATO_MIN_SWING_CENTS),
            None
        );
    }

    #[test]
    fn too_few_samples_is_not_wobble() {
        let samples = [(0.0, 0.0), (1.0, 20.0), (2.0, 0.0)];
        assert_eq!(
            measured_oscillation_hz(&samples, VIBRATO_MIN_SWING_CENTS),
            None
        );
    }

    #[test]
    fn relative_wobble_scales_with_input_level() {
        // Same swing shape at two very different gain levels — both should
        // register at the same rate, since the check is relative to each
        // signal's own mean.
        let quiet = timestamped_sine(5.0, 0.004, 40, 1.0 / 60.0)
            .into_iter()
            .map(|(t, v)| (t, v + 0.02))
            .collect::<Vec<_>>();
        let loud = timestamped_sine(5.0, 0.10, 40, 1.0 / 60.0)
            .into_iter()
            .map(|(t, v)| (t, v + 0.5))
            .collect::<Vec<_>>();
        assert!(measured_relative_oscillation_hz(&quiet, WAH_MIN_SWING_FRAC).is_some());
        assert!(measured_relative_oscillation_hz(&loud, WAH_MIN_SWING_FRAC).is_some());
    }

    #[test]
    fn steady_loudness_is_not_relative_wobble() {
        let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64, 0.2)).collect();
        assert_eq!(
            measured_relative_oscillation_hz(&steady, WAH_MIN_SWING_FRAC),
            None
        );
    }

    #[test]
    fn measured_oscillation_hz_matches_a_clean_5hz_vibrato() {
        // 40 samples at a 60 Hz frame rate span ~0.65s — over 3 full cycles at 5 Hz.
        let samples = timestamped_sine(5.0, 25.0, 40, 1.0 / 60.0);
        let hz = measured_oscillation_hz(&samples, VIBRATO_MIN_SWING_CENTS)
            .expect("should measure a rate");
        assert!((hz - 5.0).abs() < 0.5, "expected ~5 Hz, got {hz}");
    }

    #[test]
    fn measured_oscillation_hz_is_frame_rate_independent() {
        // Same 5 Hz vibrato sampled twice as densely — a raw flip *count*
        // would roughly double, but the timestamp-based rate should read
        // the same either way.
        let sparse = timestamped_sine(5.0, 25.0, 40, 1.0 / 60.0);
        let dense = timestamped_sine(5.0, 25.0, 80, 1.0 / 120.0);
        let hz_sparse = measured_oscillation_hz(&sparse, VIBRATO_MIN_SWING_CENTS).unwrap();
        let hz_dense = measured_oscillation_hz(&dense, VIBRATO_MIN_SWING_CENTS).unwrap();
        assert!(
            (hz_sparse - hz_dense).abs() < 0.3,
            "{hz_sparse} vs {hz_dense}"
        );
    }

    #[test]
    fn measured_oscillation_hz_is_none_below_the_swing_threshold() {
        let flat = timestamped_sine(5.0, 1.0, 40, 1.0 / 60.0); // swing well under 15 cents
        assert_eq!(
            measured_oscillation_hz(&flat, VIBRATO_MIN_SWING_CENTS),
            None
        );
    }

    #[test]
    fn measured_relative_oscillation_hz_normalizes_scale() {
        let quiet = timestamped_sine(3.0, 0.004, 40, 1.0 / 60.0)
            .into_iter()
            .map(|(t, v)| (t, v + 0.02))
            .collect::<Vec<_>>();
        let hz = measured_relative_oscillation_hz(&quiet, WAH_MIN_SWING_FRAC)
            .expect("should measure a rate");
        assert!((hz - 3.0).abs() < 0.5, "expected ~3 Hz, got {hz}");
    }

    #[test]
    fn oscillation_matches_rate_within_tolerance() {
        assert!(oscillation_matches_rate(5.0, 5.0, 0.4));
        assert!(oscillation_matches_rate(6.9, 5.0, 0.4)); // +38%, inside ±40%
        assert!(oscillation_matches_rate(3.1, 5.0, 0.4)); // -38%, inside ±40%
    }

    #[test]
    fn oscillation_matches_rate_rejects_outside_tolerance() {
        // A slow ~1.5 Hz wobble should not satisfy a declared 5 Hz vibrato.
        assert!(!oscillation_matches_rate(1.5, 5.0, 0.4));
        assert!(!oscillation_matches_rate(10.0, 5.0, 0.4));
    }

    #[test]
    fn oscillation_matches_rate_rejects_nonpositive_target() {
        assert!(!oscillation_matches_rate(5.0, 0.0, 0.4));
        assert!(!oscillation_matches_rate(5.0, -1.0, 0.4));
    }

    // ── AttackGate ────────────────────────────────────────────────────────────

    fn playing(keys: &[u8]) -> impl Fn(u8) -> bool + '_ {
        move |k| keys.contains(&k)
    }

    #[test]
    fn a_fresh_pitch_is_fresh_when_playing() {
        let gate = AttackGate::<u8>::default();
        assert!(gate.is_fresh(67, true));
    }

    #[test]
    fn a_pitch_that_is_not_playing_is_never_fresh() {
        let gate = AttackGate::<u8>::default();
        assert!(!gate.is_fresh(67, false));
    }

    #[test]
    fn a_consumed_pitch_is_not_fresh_while_still_held() {
        let mut gate = AttackGate::<u8>::default();
        assert!(gate.is_fresh(67, true));
        gate.consume(67);
        gate.release_absent(playing(&[67]));
        assert!(!gate.is_fresh(67, true));
    }

    #[test]
    fn releasing_an_absent_pitch_re_arms_it() {
        let mut gate = AttackGate::<u8>::default();
        gate.consume(67);
        gate.release_absent(playing(&[]));
        assert!(gate.is_fresh(67, true));
    }

    #[test]
    fn consuming_one_pitch_does_not_affect_another() {
        let mut gate = AttackGate::<u8>::default();
        gate.consume(67);
        gate.release_absent(playing(&[67, 71]));
        assert!(!gate.is_fresh(67, true));
        assert!(gate.is_fresh(71, true));
    }

    #[test]
    fn works_with_a_non_u8_key_type() {
        // Practice mode keys on the note's schedule index, not a MIDI pitch.
        let mut gate = AttackGate::<usize>::default();
        gate.consume(3);
        assert!(!gate.is_fresh(3, true));
        assert!(gate.is_fresh(4, true));
    }
}
