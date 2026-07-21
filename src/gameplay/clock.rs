// SPDX-License-Identifier: MIT

//! The single gameplay clock, and the audio-sync correction that keeps it
//! aligned with the music sink.
//!
//! [`GameplayClock`]'s value is deliberately not a public field: every write
//! goes through a method that documents (and, for the anchored case,
//! enforces) the invariant that matters here — **whoever jumps the clock to
//! a new value must also keep the music sink in sync, or suspend anchoring**,
//! otherwise [`tick_clock`](super::tick_clock)'s anchoring sees a stale sink
//! position and drags the clock right back toward it. This module exists so
//! any rewind (A–B looping, practice speed changes) can't violate that
//! invariant by accident.

use bevy::prelude::*;

use crate::app::GameplayMode;

use super::notes::{SongNotes, loop_reset_range, wait_freeze_index};
use super::pause_menu;
use super::state::{LoopConfig, MusicPlayer, MusicStarted};
use super::wait_freeze_overlay;

/// Length of the pre-song countdown, in seconds. The clock starts at
/// `-COUNTDOWN` and counts up to 0, when the music starts.
pub const COUNTDOWN: f64 = 3.0;

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
const fn advance_clock(current: f64, dt: f64, audio_pos: Option<f64>) -> f64 {
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
    pub const fn new(t: f64) -> Self {
        Self(t)
    }

    /// The current clock value (negative during the countdown).
    pub const fn get(&self) -> f64 {
        self.0
    }

    /// Set the clock directly, free-running (no audio anchoring involved).
    /// Only valid where anchoring is guaranteed inactive: scene setup before
    /// the countdown, Jam Session, and the Bending Trainer (neither anchors
    /// to a music sink at all). Anything that might run while a song is
    /// actively anchored must use [`rewind_to`](Self::rewind_to) instead.
    pub const fn set_free(&mut self, t: f64) {
        self.0 = t;
    }

    /// Advance the clock by `dt`, re-anchoring toward `audio_pos` when it's
    /// known. This is [`tick_clock`](super::tick_clock)'s own per-frame
    /// update, not a jump — see [`rewind_to`](Self::rewind_to) for that.
    pub const fn advance(&mut self, dt: f64, audio_pos: Option<f64>) {
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

/// Ticks the single [`GameplayClock`] all gameplay systems read. Once the
/// countdown finishes and the song's music starts, the clock is kept
/// anchored to the `AudioSink` playback position instead of free-running on
/// `Time::delta` — otherwise decoder start-up delay and frame hitches drift
/// the notes out of sync with the audio over a long song. Jam Session has no
/// long track to drift against and stays frame-timer driven (metronome-led).
///
/// Two things can take the clock off that path, and both work the same way:
/// the music sink should or shouldn't be audible right now
/// (`should_play`), computed once and compared against the sink's own
/// `is_paused()` so `AudioSink::pause`/`play` only ever fires on the actual
/// edge — calling it ~60 times a second turned out to visibly upset the
/// audio backend (observed as odd behaviour in the *microphone* input, a
/// fully separate pipeline, which only makes sense if repeatedly toggling
/// the output stream was disturbing a shared audio graph/server).
///
/// - `WaitForNoteMode` on (or the due note's own `ScheduledNote::force_wait`
///   — a call-and-response phrase's response notes always freeze, whether
///   or not the player has that practice toggle on) and a playable note due
///   and still unhit (`first_due_unresolved_note`): the clock simply isn't
///   advanced this frame, holding it exactly at the hit line. `judge::
///   score_notes` keeps re-judging the same held instant every frame, so
///   the moment the player plays the note it scores (typically a Perfect,
///   since the offset never moved) and the very next frame the condition is
///   false again. Jam Session never populates `SongNotes`, so this is a
///   no-op there.
/// - `PracticeSpeed` below 100%: real time-stretched audio isn't
///   implemented, so the sink just pauses instead of playing pitch-shifted,
///   and the clock free-runs on `Time::delta` scaled by the speed instead of
///   anchoring (the sink's position wouldn't mean anything at the wrong
///   speed anyway). Coming back to 100% re-seeks the sink to the clock's
///   current position (`GameplayClock::rewind_to`) before resuming it, since
///   it sat still the whole time the clock kept moving.
pub(crate) fn tick_clock(
    mut clock: ResMut<GameplayClock>,
    time: Res<Time>,
    mode: Res<GameplayMode>,
    music_started: Res<MusicStarted>,
    wait_mode: Res<pause_menu::WaitForNoteMode>,
    mut wait_freeze: ResMut<wait_freeze_overlay::WaitFreezeState>,
    practice_speed: Res<pause_menu::PracticeSpeed>,
    song_notes: Res<SongNotes>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    let due = wait_freeze_index(
        &song_notes.notes,
        song_notes.cursor,
        clock.get(),
        wait_mode.0,
    );
    // Gated so `ResMut`'s change detection (which `wait_freeze_overlay`'s
    // prompt reacts to) only fires on an actual transition, not every frame.
    if due != wait_freeze.0 {
        wait_freeze.0 = due;
    }

    let full_speed = practice_speed.0 == 1.0;
    let should_play = due.is_none() && full_speed;
    if let Ok(sink) = sinks.single() {
        if should_play && sink.is_paused() {
            let t = clock.get();
            clock.rewind_to(t, Some(sink));
            sink.play();
        } else if !should_play && !sink.is_paused() {
            sink.pause();
        }
    }

    if due.is_some() {
        return;
    }
    if !full_speed {
        clock.advance(time.delta_secs_f64() * practice_speed.0 as f64, None);
        return;
    }

    let dt = time.delta_secs_f64();
    let audio_pos = sinks
        .single()
        .ok()
        .filter(|sink| should_anchor_to_sink(clock.get(), music_started.0, &mode, sink.empty()))
        .map(|sink| sink.position().as_secs_f64());
    clock.advance(dt, audio_pos);
}

/// Whether [`tick_clock`] should anchor the clock to the music sink's
/// reported position this frame, rather than free-running on frame delta:
/// past the countdown, once music has actually started, and never in Jam
/// Session (no long track to drift against there — see `tick_clock`'s doc
/// comment).
///
/// Also `false` once the sink's queue is empty. A finished sink's
/// `position()` freezes at its last value instead of continuing to advance,
/// so anchoring to it would make `advance_clock` repeatedly snap the clock
/// back to that frozen point once real time drifts past
/// `SNAP_THRESHOLD_SECS` — better to free-run past that point instead.
pub(crate) fn should_anchor_to_sink(
    clock: f64,
    music_started: bool,
    mode: &GameplayMode,
    sink_empty: bool,
) -> bool {
    clock >= 0.0 && music_started && *mode != GameplayMode::JamSession && !sink_empty
}

pub(crate) fn handle_loop_boundary(
    loop_cfg: Res<LoopConfig>,
    mut clock: ResMut<GameplayClock>,
    mut song_notes: ResMut<SongNotes>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    if !loop_cfg.active || clock.get() < loop_cfg.end_time {
        return;
    }
    // `rewind_to` also seeks the sink, so `tick_clock`'s anchoring doesn't
    // see it far ahead of the just-rewound clock next frame and drag the
    // clock forward again — see the doc comment on `GameplayClock`.
    clock.rewind_to(loop_cfg.start_time, sinks.single().ok());

    // `notes` is sorted by `time`, so the reset range is one contiguous
    // slice — binary search it instead of scanning the whole song.
    let (start_idx, end_idx) =
        loop_reset_range(&song_notes.notes, loop_cfg.start_time, loop_cfg.end_time);
    for note in &mut song_notes.notes[start_idx..end_idx] {
        note.hit = false;
        note.missed = false;
        note.held = 0.0;
        note.sustain_scored = false;
    }
    // These notes are playable again, so `judge::score_notes`'s cursor
    // (which only ever advances past *permanently* resolved notes) can't
    // stay ahead of them — `min` in case the loop wraps before ever
    // reaching this section.
    song_notes.cursor = song_notes.cursor.min(start_idx);
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
