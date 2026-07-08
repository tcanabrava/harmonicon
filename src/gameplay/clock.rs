// SPDX-License-Identifier: MIT

//! The single gameplay clock, and the audio-sync correction that keeps it
//! aligned with the music sink.
//!
//! [`GameplayClock`]'s value is deliberately not a public field: every write
//! goes through a method that documents (and, for the anchored case,
//! enforces) the invariant that matters here — **whoever jumps the clock to
//! a new value must also keep the music sink in sync, or suspend anchoring**,
//! otherwise [`tick_clock`](super::tick_clock)'s anchoring sees a stale sink
//! position and drags the clock right back toward it. That bug already
//! happened once with `handle_loop_boundary` (see `TODO.md`); this module
//! exists so the next rewind (A–B looping, practice speed changes) can't
//! reintroduce it by accident.

use bevy::prelude::*;

/// The single clock all gameplay systems read. Negative during the 3s
/// countdown; music starts at clock 0.
#[derive(Resource, Default)]
pub struct GameplayClock(f64);

/// Max fractional deviation from real-time speed while gently correcting
/// drift toward the audio sink — ±0.5%. Expressed as a *rate*, not a fixed
/// per-frame time step: a steady, tiny speed nudge is inaudible/invisible,
/// whereas a fixed absolute step is a constant timing bias for as long as
/// the correction is active (every judged offset is off by that step), and
/// over- or under-corrects depending on the actual frame rate.
const MAX_RATE_ADJUST: f64 = 0.005;

/// Drift beyond this is treated as a discontinuity (a decoder stall, a
/// backend seek) rather than ordinary jitter, and snaps the clock straight
/// to the sink instead of rate-slewing — gently correcting half a second or
/// more of drift would leave notes visibly desynced from the audio for many
/// seconds while it converged.
const SNAP_THRESHOLD_SECS: f64 = 0.5;

/// Advance the clock by `dt`, re-anchoring toward `audio_pos` (the music
/// sink's playback position) when it's known. `audio_pos` is `None` during
/// the countdown, in Jam Session, and before the sink reports a position —
/// callers fall back to plain frame-delta accumulation in those cases.
fn advance_clock(current: f64, dt: f64, audio_pos: Option<f64>) -> f64 {
    let projected = current + dt;
    let Some(audio_pos) = audio_pos else {
        return projected;
    };
    let drift = audio_pos - projected;
    if drift.abs() > SNAP_THRESHOLD_SECS {
        return audio_pos;
    }
    let cap = MAX_RATE_ADJUST * dt;
    projected + drift.clamp(-cap, cap)
}

impl GameplayClock {
    /// Construct a clock already at `t` — for a freshly-inserted
    /// `GameplayClock` resource (setup, tests) there's no prior game state
    /// to invalidate and no sink to desync from, so no seeking is needed.
    pub fn new(t: f64) -> Self {
        Self(t)
    }

    /// The current clock value (negative during the countdown).
    pub fn get(&self) -> f64 {
        self.0
    }

    /// Set the clock directly, free-running (no audio anchoring involved).
    /// Only valid where anchoring is guaranteed inactive: scene setup before
    /// the countdown, Jam Session, and the Bending Trainer (neither anchors
    /// to a music sink at all). Anything that might run while a song is
    /// actively anchored must use [`rewind_to`](Self::rewind_to) instead.
    pub fn set_free(&mut self, t: f64) {
        self.0 = t;
    }

    /// Advance the clock by `dt`, re-anchoring toward `audio_pos` when it's
    /// known. This is [`tick_clock`](super::tick_clock)'s own per-frame
    /// update, not a jump — see [`rewind_to`](Self::rewind_to) for that.
    pub fn advance(&mut self, dt: f64, audio_pos: Option<f64>) {
        self.0 = advance_clock(self.0, dt, audio_pos);
    }

    /// Jump the clock to `t`, keeping the music sink in sync so the next
    /// anchoring pass doesn't see a stale position and drag the clock right
    /// back toward it. Pass the current song's sink whenever one might be
    /// playing (loop boundaries, future A–B looping, practice-speed
    /// changes); `None` is correct only where no sink exists yet (or ever,
    /// e.g. Jam Session).
    pub fn rewind_to(&mut self, t: f64, sink: Option<&AudioSink>) {
        self.0 = t;
        if let Some(sink) = sink
            && let Err(e) = sink.try_seek(std::time::Duration::from_secs_f64(t.max(0.0)))
        {
            warn!("GameplayClock::rewind_to({t}): failed to seek music sink: {e:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_clock_with_no_audio_pos_is_plain_frame_delta() {
        assert!((advance_clock(1.0, 0.016, None) - 1.016).abs() < 1e-9);
    }

    #[test]
    fn advance_clock_snaps_fully_when_drift_is_within_the_rate_cap() {
        // At dt=0.016s the rate cap is 0.5% of that = 0.00008s; a smaller
        // drift than that corrects fully in a single frame.
        let target = 1.016 + 0.00005;
        let result = advance_clock(1.0, 0.016, Some(target));
        assert!((result - target).abs() < 1e-9);
    }

    #[test]
    fn advance_clock_slews_moderate_positive_drift_by_the_rate_cap() {
        let dt = 0.016;
        let projected = 1.0 + dt;
        let cap = MAX_RATE_ADJUST * dt;
        // 50ms ahead: real drift, but nowhere near the snap threshold.
        let result = advance_clock(1.0, dt, Some(projected + 0.05));
        assert!(
            (result - (projected + cap)).abs() < 1e-9,
            "should correct by at most the rate cap, not the full 50ms"
        );
    }

    #[test]
    fn advance_clock_slews_moderate_negative_drift_by_the_rate_cap() {
        let dt = 0.016;
        let projected = 1.0 + dt;
        let cap = MAX_RATE_ADJUST * dt;
        let result = advance_clock(1.0, dt, Some(projected - 0.05));
        assert!((result - (projected - cap)).abs() < 1e-9);
    }

    #[test]
    fn advance_clock_snaps_outright_beyond_the_snap_threshold() {
        // A stall/seek-sized discontinuity shouldn't take many seconds to
        // rate-slew away — snap straight to the sink instead.
        let audio_pos = 1.0 + 0.016 + SNAP_THRESHOLD_SECS + 0.001;
        let result = advance_clock(1.0, 0.016, Some(audio_pos));
        assert_eq!(result, audio_pos);
    }

    #[test]
    fn get_reflects_set_free() {
        let mut clock = GameplayClock::default();
        clock.set_free(-3.0);
        assert_eq!(clock.get(), -3.0);
    }

    #[test]
    fn advance_updates_via_the_pure_function() {
        let mut clock = GameplayClock::default();
        clock.set_free(1.0);
        clock.advance(0.016, None);
        assert!((clock.get() - 1.016).abs() < 1e-9);
    }

    #[test]
    fn rewind_to_sets_the_clock_without_a_sink() {
        let mut clock = GameplayClock::default();
        clock.set_free(10.0);
        clock.rewind_to(2.0, None);
        assert_eq!(clock.get(), 2.0);
    }
}
