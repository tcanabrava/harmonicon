// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use crate::song::harmonica::{Harmonica, Progression};

use super::super::session::build_hole_guide;
use super::*;

/// Standard Richter C diatonic, matching `harmonica.rs`'s test layout.
fn c_harp() -> Harmonica {
    serde_json::from_str(
        r#"{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
            "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                      "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}"#,
    )
    .unwrap()
}

// ── ImprovStats ───────────────────────────────────────────────────────────

#[test]
fn improv_stats_adherence_is_none_with_nothing_played() {
    assert_eq!(ImprovStats::default().adherence(), None);
}

#[test]
fn improv_stats_adherence_counts_chord_tone_and_in_scale_as_good() {
    let stats = ImprovStats {
        chord_tone: 3,
        in_scale: 5,
        out_of_scale: 2,
        rest_violations: 0,
    };
    assert_eq!(stats.total(), 10);
    assert!((stats.adherence().unwrap() - 0.8).abs() < 1e-6);
}

#[test]
fn improv_stats_chord_tone_adherence_only_counts_chord_tones() {
    let stats = ImprovStats {
        chord_tone: 3,
        in_scale: 5,
        out_of_scale: 2,
        rest_violations: 0,
    };
    assert!((stats.chord_tone_adherence().unwrap() - 0.3).abs() < 1e-6);
}

#[test]
fn improv_stats_phrase_discipline_is_one_minus_the_violation_fraction() {
    let stats = ImprovStats {
        chord_tone: 4,
        in_scale: 4,
        out_of_scale: 2,
        rest_violations: 3,
    };
    assert!((stats.phrase_discipline().unwrap() - 0.7).abs() < 1e-6);
}

#[test]
fn improv_stats_phrase_discipline_is_none_with_nothing_played() {
    assert_eq!(ImprovStats::default().phrase_discipline(), None);
}

// ── in_rest_window ────────────────────────────────────────────────────────

#[test]
fn in_rest_window_alternates_by_the_given_pattern() {
    // 2 on / 2 off: bars 0-1 play, 2-3 rest, repeating.
    assert!(!in_rest_window(0, 2, 2));
    assert!(!in_rest_window(1, 2, 2));
    assert!(in_rest_window(2, 2, 2));
    assert!(in_rest_window(3, 2, 2));
    assert!(!in_rest_window(4, 2, 2));
    assert!(in_rest_window(7, 2, 2));
}

#[test]
fn in_rest_window_is_never_resting_with_a_zero_length_cycle() {
    assert!(!in_rest_window(0, 0, 0));
    assert!(!in_rest_window(100, 0, 0));
}

// ── accumulate_improv_stats ───────────────────────────────────────────────

fn improv_pitch_info(midi: u8, note: &str) -> crate::audio_system::pitch_detect::PitchInfo {
    crate::audio_system::pitch_detect::PitchInfo {
        midi,
        note: note.to_string(),
        octave: 4,
        frequency: crate::audio_system::midi::midi_to_freq_hz(midi as f32),
    }
}

fn improv_test_world() -> World {
    let mut world = World::new();
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Standard);
    world.insert_resource(guide);
    world.insert_resource(CurrentBar(0)); // bar 0 is the I chord (C7)
    world.insert_resource(AbsoluteBar(0)); // a "play" bar
    world.insert_resource(ImprovGate::default());
    world.insert_resource(ImprovStats::default());
    world
}

#[test]
fn accumulate_improv_stats_tallies_a_fresh_chord_tone_attack() {
    let mut world = improv_test_world();
    world.insert_resource(ActivePitches(vec![improv_pitch_info(60, "C")])); // C4: blow hole 1
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);
    schedule.run(&mut world);

    let stats = world.resource::<ImprovStats>();
    assert_eq!(stats.chord_tone, 1);
    assert_eq!(stats.total(), 1);
}

#[test]
fn accumulate_improv_stats_only_counts_a_held_note_once() {
    let mut world = improv_test_world();
    world.insert_resource(ActivePitches(vec![improv_pitch_info(60, "C")]));
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);
    schedule.run(&mut world); // fresh attack
    schedule.run(&mut world); // still held, same pitch

    assert_eq!(
        world.resource::<ImprovStats>().total(),
        1,
        "a held note shouldn't tally again every frame"
    );
}

#[test]
fn accumulate_improv_stats_rearms_after_the_note_stops_and_restarts() {
    let mut world = improv_test_world();
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);

    world.insert_resource(ActivePitches(vec![improv_pitch_info(60, "C")]));
    schedule.run(&mut world);
    world.insert_resource(ActivePitches(vec![])); // released
    schedule.run(&mut world);
    world.insert_resource(ActivePitches(vec![improv_pitch_info(60, "C")])); // re-attacked
    schedule.run(&mut world);

    assert_eq!(world.resource::<ImprovStats>().total(), 2);
}

#[test]
fn accumulate_improv_stats_ignores_pitches_the_harp_cant_produce() {
    let mut world = improv_test_world();
    // MIDI 61 (C#4) isn't anywhere in this harp's blow/draw layout.
    world.insert_resource(ActivePitches(vec![improv_pitch_info(61, "C#")]));
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);
    schedule.run(&mut world);

    assert_eq!(world.resource::<ImprovStats>().total(), 0);
}

#[test]
fn accumulate_improv_stats_classifies_by_the_current_bars_chord() {
    let mut world = improv_test_world();
    *world.resource_mut::<CurrentBar>() = CurrentBar(4); // bar 4 is IV (F7)
    // F4 (MIDI 65) isn't produced by this harp at all — use a note this
    // harp *can* play that's in the scale but not a tone of F7: G4 (the
    // blues-scale 5th over C, not in F/A/C/D#).
    world.insert_resource(ActivePitches(vec![improv_pitch_info(67, "G")]));
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);
    schedule.run(&mut world);

    let stats = world.resource::<ImprovStats>();
    assert_eq!(stats.in_scale, 1);
    assert_eq!(stats.chord_tone, 0);
}

#[test]
fn accumulate_improv_stats_tallies_a_rest_violation_during_a_rest_bar() {
    let mut world = improv_test_world();
    *world.resource_mut::<AbsoluteBar>() = AbsoluteBar(2); // a rest bar (2 on / 2 off)
    world.insert_resource(ActivePitches(vec![improv_pitch_info(60, "C")]));
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);
    schedule.run(&mut world);

    let stats = world.resource::<ImprovStats>();
    assert_eq!(stats.rest_violations, 1);
    // Still classified normally — phrase discipline judges *when*, not
    // *what*, so the ordinary chord-tone tally isn't suppressed.
    assert_eq!(stats.chord_tone, 1);
}

#[test]
fn accumulate_improv_stats_does_not_tally_a_violation_during_a_play_bar() {
    let mut world = improv_test_world(); // AbsoluteBar(0) — a play bar
    world.insert_resource(ActivePitches(vec![improv_pitch_info(60, "C")]));
    let mut schedule = Schedule::default();
    schedule.add_systems(accumulate_improv_stats);
    schedule.run(&mut world);

    assert_eq!(world.resource::<ImprovStats>().rest_violations, 0);
}
