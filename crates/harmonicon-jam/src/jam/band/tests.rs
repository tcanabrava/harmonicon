// SPDX-License-Identifier: MIT

use super::*;

/// A quarter-note line: four attacks a bar, sparse.
const PLAYING: BeatActivity = BeatActivity { attacks: 1, sounding: true };
/// Steady eighths: eight attacks a bar, dense.
const BUSY: BeatActivity = BeatActivity { attacks: 2, sounding: true };
const QUIET: BeatActivity = BeatActivity { attacks: 0, sounding: false };
/// A note held over from an earlier beat: nothing new attacked, but still
/// sounding.
const HELD: BeatActivity = BeatActivity { attacks: 0, sounding: true };

/// Feeds `beats` to a listener starting at beat 0 and collects every
/// answer as `(beat index the answer begins on, kind)`.
fn run(listener: &mut BandListener, beats: &[BeatActivity]) -> Vec<(usize, BandAnswer)> {
    let mut out = Vec::new();
    for (i, &b) in beats.iter().enumerate() {
        if let Some(answer) = listener.observe(i, b) {
            out.push((i + 1, answer));
        }
    }
    out
}

/// Three bars of playing and a bar of rest — the classic blues phrase.
fn phrase_then_rest() -> Vec<BeatActivity> {
    let mut v = vec![PLAYING; 12];
    v.extend([QUIET; 4]);
    v
}

// ── phrase_density / comping_target_after ────────────────────────────────

#[test]
fn density_is_descriptive_by_attacks_per_bar() {
    assert_eq!(phrase_density(&[QUIET; 16]), PhraseDensity::Silent);
    assert_eq!(phrase_density(&[PLAYING; 16]), PhraseDensity::Sparse);
    assert_eq!(phrase_density(&[BUSY; 16]), PhraseDensity::Dense);
    assert_eq!(phrase_density(&[]), PhraseDensity::Silent);
}

#[test]
fn comping_thins_after_a_dense_phrase_and_holds_the_pocket_otherwise() {
    assert_eq!(comping_target_after(PhraseDensity::Dense), THINNED_COMPING);
    assert_eq!(comping_target_after(PhraseDensity::Sparse), 1.0);
    assert_eq!(comping_target_after(PhraseDensity::Silent), 1.0);
}

// ── BandListener ─────────────────────────────────────────────────────────

#[test]
fn a_finished_phrase_is_answered_on_beat_three_of_its_last_bar() {
    let mut listener = BandListener::default();
    let answers = run(&mut listener, &phrase_then_rest());
    // Beats are zero-based: bar 4 starts at beat 12, its beat 3 is 14.
    assert_eq!(answers, vec![(14, BandAnswer::Drums)]);
}

#[test]
fn no_answer_while_the_player_is_still_playing_into_the_last_bar() {
    let mut listener = BandListener::default();
    let answers = run(&mut listener, &[PLAYING; 16]);
    assert!(answers.is_empty());
}

#[test]
fn a_note_still_ringing_is_not_a_release() {
    let mut listener = BandListener::default();
    let mut beats = vec![PLAYING; 12];
    beats.extend([HELD, HELD, QUIET, QUIET]);
    assert!(run(&mut listener, &beats).is_empty());
}

#[test]
fn silence_is_left_alone_not_filled() {
    let mut listener = BandListener::default();
    let answers = run(&mut listener, &[QUIET; 48]);
    assert!(answers.is_empty());
    assert_eq!(listener.comping_target(), 1.0);
}

#[test]
fn a_stray_note_or_two_is_not_a_phrase() {
    let mut listener = BandListener::default();
    let mut beats = vec![QUIET; 16];
    beats[5] = BeatActivity { attacks: 1, sounding: true };
    beats[9] = BeatActivity { attacks: 2, sounding: true };
    assert!(run(&mut listener, &beats).is_empty());
}

#[test]
fn answers_are_rate_capped_and_alternate_drums_and_chord() {
    let mut listener = BandListener::default();
    let mut beats = Vec::new();
    for _ in 0..4 {
        beats.extend(phrase_then_rest());
    }
    let answers = run(&mut listener, &beats);
    // Four phrase ends, but at least eight bars between answers: two answers.
    assert_eq!(answers, vec![(14, BandAnswer::Drums), (46, BandAnswer::Chord)]);
}

#[test]
fn comping_thins_for_the_phrase_after_a_dense_one_then_recovers() {
    let mut listener = BandListener::default();
    run(&mut listener, &[BUSY; 16]);
    assert_eq!(listener.comping_target(), THINNED_COMPING);
    run_from(&mut listener, 16, &[PLAYING; 16]);
    assert_eq!(listener.comping_target(), 1.0);
}

fn run_from(listener: &mut BandListener, first_beat: usize, beats: &[BeatActivity]) {
    for (i, &b) in beats.iter().enumerate() {
        listener.observe(first_beat + i, b);
    }
}

#[test]
fn comping_target_only_changes_at_phrase_boundaries() {
    let mut listener = BandListener::default();
    for i in 0..15 {
        listener.observe(i, BUSY);
        assert_eq!(listener.comping_target(), 1.0, "changed mid-phrase at beat {i}");
    }
    listener.observe(15, BUSY);
    assert_eq!(listener.comping_target(), THINNED_COMPING);
}

#[test]
fn beats_skipped_by_a_long_frame_still_reach_the_phrase_boundary() {
    let mut listener = BandListener::default();
    run(&mut listener, &[BUSY; 13]);
    // One frame jumps from beat 13 to beat 17: beats 14–16 are fed silent,
    // so the boundary after beat 15 still thins the comping.
    assert_eq!(listener.observe_until(13, BUSY, 17), None);
    assert_eq!(listener.comping_target(), THINNED_COMPING);
}

#[test]
fn an_answer_due_on_a_skipped_beat_is_still_returned() {
    let mut listener = BandListener::default();
    run(&mut listener, &[PLAYING; 11]);
    // Beats 12 and 13 pass silently inside one frame; the answer due as
    // beat 14 begins is not lost.
    assert_eq!(listener.observe_until(11, PLAYING, 15), Some(BandAnswer::Drums));
}

#[test]
fn a_long_stall_replays_at_most_the_log() {
    let mut listener = BandListener::default();
    run(&mut listener, &[BUSY; 16]);
    assert_eq!(listener.observe_until(15, QUIET, 10_000), None);
    assert_eq!(listener.beats.len(), LOG_BEATS);
    assert_eq!(listener.comping_target(), 1.0);
}

#[test]
fn the_same_activity_stream_always_yields_the_same_reactions() {
    let mut stream = Vec::new();
    for round in 0..6u32 {
        for beat in 0..16u32 {
            stream.push(BeatActivity {
                attacks: (round * 7 + beat * 3) % 5,
                sounding: (round + beat) % 3 != 0,
            });
        }
    }
    let mut a = BandListener::default();
    let mut b = BandListener::default();
    assert_eq!(run(&mut a, &stream), run(&mut b, &stream));
    assert_eq!(a.comping_target(), b.comping_target());
}

// ── beat_index / ease_gain ───────────────────────────────────────────────

#[test]
fn beat_index_follows_the_clock_and_is_absent_during_the_countdown() {
    assert_eq!(beat_index(-1.0, 120.0), None);
    assert_eq!(beat_index(0.0, 120.0), Some(0));
    assert_eq!(beat_index(0.49, 120.0), Some(0));
    assert_eq!(beat_index(0.5, 120.0), Some(1));
    assert_eq!(beat_index(4.0, 60.0), Some(4));
}

#[test]
fn the_comping_gain_eases_over_the_ramp_and_never_overshoots() {
    let mut gain = 1.0;
    gain = ease_gain(gain, THINNED_COMPING, 1.0, 2.0);
    assert!((gain - (1.0 + THINNED_COMPING) / 2.0).abs() < 1e-5);
    gain = ease_gain(gain, THINNED_COMPING, 5.0, 2.0);
    assert_eq!(gain, THINNED_COMPING);
    gain = ease_gain(gain, 1.0, 5.0, 2.0);
    assert_eq!(gain, 1.0);
}

#[test]
fn adaptive_band_is_on_by_default_and_a_fresh_tracker_comps_at_full() {
    assert!(AdaptiveBand::default().0);
    assert_eq!(BandTracker::fresh().comping_gain, 1.0);
}
