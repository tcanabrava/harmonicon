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
pub const fn classify_note(
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
pub const fn compute_multiplier(combo: u32, base_mult: f32, step_mult: f32, max_mult: f32) -> f32 {
    (base_mult + combo as f32 * step_mult).min(max_mult)
}

/// Points awarded for a single hit.
pub const fn compute_points(quality: HitQuality, multiplier: f32) -> u32 {
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
pub const fn sustain_points(held: f64, duration: f64) -> u32 {
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

/// Oscillation rate (Hz) of `samples`' values divided by `divisor`, or
/// `None` if there are too few samples, the peak-to-trough swing never
/// reaches `min_swing`, or it reverses direction fewer than twice. Deltas
/// below 15% of `min_swing` are treated as frame-to-frame jitter and
/// ignored so they don't count as spurious direction changes.
///
/// One pass and no allocation: it runs every frame for every held vibrato or
/// wah note (the highway's live technique status), over a history that grows
/// by a sample a frame. Consecutive reversals are half a cycle apart, and the
/// mean of the gaps between them telescopes to `(last − first) / (count − 1)`,
/// so only the first and last reversal times and the count are kept.
fn oscillation_hz(samples: &[(f64, f32)], divisor: f32, min_swing: f32) -> Option<f32> {
    if samples.len() < 6 {
        return None;
    }
    let (min, max) = samples
        .iter()
        .map(|&(_, v)| v / divisor)
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    if max - min < min_swing {
        return None;
    }
    let noise_floor = min_swing * 0.15;
    let mut direction = 0i32;
    let (mut flips, mut first, mut last) = (0usize, 0.0f64, 0.0f64);
    for pair in samples.windows(2) {
        let d = pair[1].1 / divisor - pair[0].1 / divisor;
        if d.abs() < noise_floor {
            continue;
        }
        let sign = if d > 0.0 { 1 } else { -1 };
        if direction != 0 && sign != direction {
            if flips == 0 {
                first = pair[1].0;
            }
            last = pair[1].0;
            flips += 1;
        }
        direction = sign;
    }
    if flips < 2 {
        return None;
    }
    let mean_half_period = (last - first) / (flips - 1) as f64;
    if mean_half_period <= 0.0 {
        return None;
    }
    Some((1.0 / (2.0 * mean_half_period)) as f32)
}

/// Estimated oscillation rate (Hz) from the real elapsed time between
/// direction reversals in timestamped `samples` — frame-rate independent,
/// unlike counting flips over a fixed number of samples. `None` if `samples`
/// never wobbles enough to qualify as oscillation at all (fewer than two
/// reversals, or peak-to-trough swing under `min_swing`).
pub fn measured_oscillation_hz(samples: &[(f64, f32)], min_swing: f32) -> Option<f32> {
    oscillation_hz(samples, 1.0, min_swing)
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
    oscillation_hz(samples, mean, min_frac)
}

/// True when `measured_hz` is within `tolerance_frac` of `target_hz` (e.g.
/// `0.4` = ±40%) — generous, since hand vibrato/wah speed varies naturally
/// between players and even between notes.
pub const fn oscillation_matches_rate(
    measured_hz: f32,
    target_hz: f32,
    tolerance_frac: f32,
) -> bool {
    if target_hz <= 0.0 {
        return false;
    }
    let lower = target_hz * (1.0 - tolerance_frac);
    let upper = target_hz * (1.0 + tolerance_frac);
    measured_hz >= lower && measured_hz <= upper
}

/// True when the combo should reset due to inactivity.
pub const fn should_decay_combo(
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
mod tests;
