// SPDX-License-Identifier: MIT

use super::*;
use crate::harmonica::richter_harp;
use crate::harmonica_constraints::BreathDirectionTracker;

#[test]
fn note_tracker_preserves_a_pitch_reachable_in_both_directions() {
    let harp = richter_harp("C");
    let mut tracker = NoteTracker::new(
        &harp.into(),
        NoteTrackerConfig { onset_frames: 1, ..NoteTrackerConfig::default() },
    );
    // G4: hole 2 draw and hole 3 blow.
    assert_eq!(tracker.update(&[67]).active, vec![67]);
}

#[test]
fn a_re_articulation_after_one_silent_frame_is_a_second_attack() {
    // Repeated notes on one hole are the basic rhythmic figure of blues
    // harmonica, and the gap between two of them is short: at the
    // shipped hop size a detector frame is ~46 ms, so chugged eighths
    // leave one, sometimes two, silent frames. The tracker has to
    // report the pitch as gone in that gap — `AttackGate` re-arms on
    // absence, so a pitch that never goes absent can only ever satisfy
    // one note, and the second chug scores nothing.
    let harp = richter_harp("C");
    let mut tracker = NoteTracker::new(&harp.into(), NoteTrackerConfig::default());
    tracker.update(&[60]);
    assert_eq!(tracker.update(&[60]).confirmed, vec![60]);
    assert!(
        tracker.update(&[]).active.is_empty(),
        "one silent frame must read as a release, or two chugged notes \
         merge into a single sustain"
    );
    tracker.update(&[60]);
    assert_eq!(tracker.update(&[60]).confirmed, vec![60], "the re-attack must confirm again");
}

#[test]
fn a_zero_frame_threshold_is_floored_to_one() {
    // Zero would mean deciding before observing anything. Each threshold
    // is floored by whichever type owns it, so no caller has to know.
    let harp = richter_harp("C");
    let mut tracker = NoteTracker::new(
        &harp.clone().into(),
        NoteTrackerConfig { onset_frames: 0, release_frames: 0, direction_change_frames: 0 },
    );
    assert_eq!(tracker.onset_frames(), 1);
    assert_eq!(tracker.update(&[60]).confirmed, vec![60]);
    assert!(tracker.update(&[]).active.is_empty());

    let mut direction = BreathDirectionTracker::with_change_frames(0);
    direction.filter(&harp, &[60]);
    assert_eq!(direction.filter(&harp, &[62]), vec![62]);
}

#[test]
fn a_longer_release_grace_bridges_a_dropout_when_asked_for() {
    // The knob still works, and is what a detector measured to drop
    // frames mid-sustain would want — see `docs/pitch_detection_plan.md`.
    let harp = richter_harp("C");
    let mut tracker = NoteTracker::new(
        &harp.into(),
        NoteTrackerConfig { release_frames: 2, ..NoteTrackerConfig::default() },
    );
    tracker.update(&[60]);
    tracker.update(&[60]);
    assert_eq!(tracker.update(&[]).active, vec![60]);
    assert!(tracker.update(&[]).active.is_empty());
}

#[test]
fn a_confirmation_is_always_the_same_number_of_frames_late() {
    // The property the whole latency correction rests on: whatever the
    // onset threshold, a pitch is confirmed on exactly the frame its run
    // reaches it — never earlier, never later, however long it goes on
    // sounding afterwards. So the lag is one constant for the stream, and
    // a consumer can subtract it from its clock once rather than tracking
    // a per-pitch age.
    let harp = richter_harp("C");
    for onset_frames in 1..=4u8 {
        let mut tracker = NoteTracker::new(
            &harp.clone().into(),
            NoteTrackerConfig { onset_frames, ..NoteTrackerConfig::default() },
        );
        let confirmed_on: Vec<usize> =
            (0..8).filter(|_| !tracker.update(&[60]).confirmed.is_empty()).collect();
        assert_eq!(
            confirmed_on,
            vec![onset_frames as usize - 1],
            "onset_frames = {onset_frames} should confirm once, on frame \
             {}, and never again while the pitch keeps sounding",
            onset_frames - 1
        );
    }
}
