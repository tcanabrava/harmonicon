// SPDX-License-Identifier: MIT

//! What a judged note does *on the highway*, as pure functions shared by the
//! 2D and 3D renderers: the hold-progress uniform a sustained note's tail
//! draws from, and the pop / shrink a head performs the instant it is judged.
//! Both renderers compute the same numbers and only differ in what they
//! apply them to (a `UiTransform` vs a `Transform`, a UI material vs a mesh
//! material), so the two modes cannot drift apart on what a hit looks like.

use bevy::prelude::*;

use super::notes::ScheduledNote;

/// The live state of a hit note's sustain, as the tail shader wants it —
/// see `hold` in `assets/shaders/note_ribbon_2d.wesl`.
///
/// The tail scrolls through the hit line time-accurately (its tip meets the
/// line at the note's end), so *progress* is already the line sweeping up
/// it; what the part still above the line can add is state. `x` = whether
/// the expected pitch is sounding this frame (1 / 0); `y` = how well the
/// hold has gone so far, `held` over the time elapsed since the note began
/// on the judge's timeline (`judge::judged_instant`, which `held` accrues
/// on), 0..1; `z` = live technique status: `1` confirmed, `-1` not heard,
/// `0` nothing to confirm or not measurable yet; `w` = 1 once hit. All zero
/// for a note that hasn't been hit, so the shader draws it plain.
pub fn hold_uniform(
    note: &ScheduledNote,
    judged: f64,
    holding_now: bool,
    technique: Option<bool>,
) -> Vec4 {
    if !note.hit || note.duration <= 0.0 {
        return Vec4::ZERO;
    }
    let elapsed = (judged - note.time).clamp(0.0, note.duration);
    let integrity = if elapsed > 0.0 { (note.held / elapsed).clamp(0.0, 1.0) as f32 } else { 1.0 };
    let technique = match technique {
        Some(true) => 1.0,
        Some(false) => -1.0,
        None => 0.0,
    };
    Vec4::new(f32::from(holding_now), integrity, technique, 1.0)
}

/// How long the hit pop lasts, in gameplay-clock seconds.
pub const POP_SECS: f32 = 0.22;
/// Peak scale of the hit pop.
pub const POP_PEAK: f32 = 1.35;
/// A missed head settles at this scale and stays there — a shape cue that
/// survives without colour, unlike the red tint.
pub const MISS_SCALE: f32 = 0.72;
/// How long the miss shrink takes.
pub const SHRINK_SECS: f32 = 0.16;

/// Scale multiplier for a judged note's head, `age` seconds after the
/// judgment. A hit pops to [`POP_PEAK`] and eases back to 1 over
/// [`POP_SECS`]; a miss eases down to [`MISS_SCALE`] over [`SHRINK_SECS`]
/// and holds. Ages before zero (a clock rewound under a still-spawned
/// visual) read as unjudged. With `reduced_motion` the pop is dropped
/// entirely and the miss shrink is immediate — the shrunk head is a state
/// cue that has to stay; only the movement into it goes.
pub fn judged_scale(hit: bool, age: f32, reduced_motion: bool) -> f32 {
    if age < 0.0 {
        return 1.0;
    }
    if reduced_motion {
        return if hit { 1.0 } else { MISS_SCALE };
    }
    if hit {
        let t = (age / POP_SECS).min(1.0);
        // Straight to the peak, then a quadratic ease back down.
        let settled = 1.0 - (1.0 - t).powi(2);
        1.0 + (POP_PEAK - 1.0) * (1.0 - settled)
    } else {
        let t = (age / SHRINK_SECS).min(1.0);
        1.0 + (MISS_SCALE - 1.0) * t
    }
}

/// The stamp a judged note's head label shows instead of its tab: a check
/// on a hit, a cross on a miss. Both are in the bundled fallback font (see
/// `dialogs::font_fallback`).
pub fn judged_stamp(hit: bool) -> &'static str {
    if hit { "\u{2713}" } else { "\u{2717}" }
}

/// Colour of a 2D note head's label: dark on the bright pending/hit head,
/// light on the dim red of a miss, where the dark ✗ would vanish.
pub fn head_label_color(judged: Option<bool>) -> Color {
    match judged {
        Some(false) => Color::srgba(0.98, 0.88, 0.88, 0.95),
        _ => Color::srgba(0.05, 0.05, 0.08, 0.95),
    }
}

/// Last judged state a note visual has been drawn in — `None` while the note
/// is pending — so a renderer notices the transition (and only the
/// transition) without `ScheduledNote` being a component it could put a
/// `Changed` filter on. Also what makes an A–B loop reset clean: the loop
/// clears `hit`/`missed`, this reads the change back to `None`, and the
/// head is restored rather than left mid-pop.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgedState(pub Option<bool>);

/// When a note was judged, on the gameplay clock, and how. Inserted on the
/// transition [`JudgedState`] observes; the head's scale is a function of the
/// clock minus `at`, so pausing freezes the pop like everything else.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Judged {
    pub hit: bool,
    pub at: f64,
}

/// The note's current judgment as the visual should show it.
pub fn judged_now(note: &ScheduledNote) -> Option<bool> {
    if note.hit {
        Some(true)
    } else if note.missed {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(hit: bool, time: f64, duration: f64, held: f64) -> ScheduledNote {
        ScheduledNote {
            time,
            duration,
            hole: 4,
            is_blow: true,
            expected_pitch: Some(60),
            hit,
            missed: false,
            held,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: 0,
            chord_pitches: Vec::new(),
            playable: true,
            miss_evidence: None,
            force_wait: false,
        }
    }

    #[test]
    fn an_unhit_note_has_no_hold_state() {
        assert_eq!(hold_uniform(&note(false, 1.0, 2.0, 1.0), 2.0, true, None), Vec4::ZERO);
    }

    #[test]
    fn hold_state_reports_sounding_now_and_integrity_so_far() {
        // 1.0 s in, 0.5 s of it credited: half-held so far, sounding now.
        let u = hold_uniform(&note(true, 1.0, 2.0, 0.5), 2.0, true, None);
        assert_eq!(u.x, 1.0);
        assert!((u.y - 0.5).abs() < 1e-6);
        let u = hold_uniform(&note(true, 1.0, 2.0, 1.0), 2.0, false, None);
        assert_eq!(u.x, 0.0, "pitch dropped out this frame");
        assert!((u.y - 1.0).abs() < 1e-6, "…but everything so far was credited");
    }

    #[test]
    fn hold_integrity_is_whole_on_the_hit_frame_and_capped_after_the_end() {
        // Nothing has elapsed yet: nothing lost yet either.
        assert_eq!(hold_uniform(&note(true, 1.0, 2.0, 0.0), 1.0, true, None).y, 1.0);
        // Past the end, `held` can exceed the clamped elapsed time.
        assert_eq!(hold_uniform(&note(true, 1.0, 2.0, 5.0), 9.0, true, None).y, 1.0);
    }

    #[test]
    fn technique_status_is_encoded_as_a_sign() {
        let n = note(true, 0.0, 1.0, 0.0);
        assert_eq!(hold_uniform(&n, 0.5, true, Some(true)).z, 1.0);
        assert_eq!(hold_uniform(&n, 0.5, true, Some(false)).z, -1.0);
        assert_eq!(hold_uniform(&n, 0.5, true, None).z, 0.0);
    }

    #[test]
    fn a_hit_pops_then_settles_back_to_one() {
        assert!((judged_scale(true, 0.0, false) - POP_PEAK).abs() < 1e-6);
        assert!(judged_scale(true, POP_SECS * 0.5, false) > 1.0);
        assert!((judged_scale(true, POP_SECS, false) - 1.0).abs() < 1e-6);
        assert!((judged_scale(true, 10.0, false) - 1.0).abs() < 1e-6, "and stays there");
    }

    #[test]
    fn a_miss_shrinks_and_stays_shrunk() {
        assert!((judged_scale(false, 0.0, false) - 1.0).abs() < 1e-6);
        let mid = judged_scale(false, SHRINK_SECS * 0.5, false);
        assert!(mid < 1.0 && mid > MISS_SCALE);
        assert!((judged_scale(false, SHRINK_SECS, false) - MISS_SCALE).abs() < 1e-6);
        assert!((judged_scale(false, 10.0, false) - MISS_SCALE).abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_keeps_the_miss_state_but_drops_the_movement() {
        assert_eq!(judged_scale(true, 0.0, true), 1.0, "no pop");
        assert_eq!(judged_scale(false, 0.0, true), MISS_SCALE, "shrunk at once");
        assert_eq!(judged_scale(false, 10.0, true), MISS_SCALE);
    }

    #[test]
    fn a_negative_age_reads_as_unjudged() {
        assert_eq!(judged_scale(true, -0.1, false), 1.0);
        assert_eq!(judged_scale(false, -0.1, false), 1.0);
    }

    #[test]
    fn the_label_goes_light_only_on_a_miss() {
        assert_eq!(head_label_color(None), head_label_color(Some(true)));
        assert_ne!(head_label_color(None), head_label_color(Some(false)));
    }

    #[test]
    fn judged_now_reads_hit_before_missed() {
        let mut n = note(true, 0.0, 1.0, 0.0);
        assert_eq!(judged_now(&n), Some(true));
        n.hit = false;
        assert_eq!(judged_now(&n), None);
        n.missed = true;
        assert_eq!(judged_now(&n), Some(false));
    }
}
