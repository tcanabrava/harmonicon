// SPDX-License-Identifier: MIT

use super::*;

// ── should_restart_jam_music ─────────────────────────────────────────────

#[test]
fn restarts_once_finished_when_loop_is_on() {
    assert!(should_restart_jam_music(true, true, false));
}

#[test]
fn does_not_restart_while_still_playing() {
    assert!(!should_restart_jam_music(true, true, true));
}

#[test]
fn does_not_restart_when_loop_is_off() {
    assert!(!should_restart_jam_music(false, true, false));
}

#[test]
fn does_not_restart_before_the_jam_has_started() {
    assert!(!should_restart_jam_music(true, false, false));
}

/// Standard Richter C diatonic, matching `harmonica.rs`'s test layout.
fn c_harp() -> Harmonica {
    serde_json::from_str(
        r#"{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
            "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                      "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}"#,
    )
    .unwrap()
}

#[test]
fn note_class_drops_octave() {
    assert_eq!(note_class("C4"), "C");
    assert_eq!(note_class("D#5"), "D#");
    assert_eq!(note_class("A6"), "A");
}

#[test]
fn guide_maps_a_shared_note_to_every_hole_that_sounds_it() {
    // On a C harp, G4 is both draw-2 and blow-3 — both holes should light.
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Standard);
    let mut holes = guide.note_to_holes.get(&67u8).cloned().unwrap_or_default(); // G4
    holes.sort_unstable();
    assert_eq!(holes, vec![2, 3]);
}

#[test]
fn guide_marks_scale_membership_per_direction() {
    let (holes, _) = build_hole_guide(&c_harp(), "C", Progression::Standard);
    let hole1 = holes.iter().find(|h| h.hole == 1).unwrap();
    assert!(hole1.blow_in_scale, "blow C4 is the root → in scale");
    assert!(!hole1.draw_in_scale, "draw D4 (major 2nd) → outside");
    let hole2 = holes.iter().find(|h| h.hole == 2).unwrap();
    assert!(hole2.draw_in_scale, "draw G4 (the 5th) → in scale");
}

#[test]
fn guide_covers_all_ten_holes() {
    let (holes, _) = build_hole_guide(&c_harp(), "C", Progression::Standard);
    assert_eq!(holes.len(), 10);
}

/// 12-hole chromatic, matching the fixture in `harmonica.rs`'s tests.
fn c_chromatic_harp() -> Harmonica {
    serde_json::from_str(
        r#"{"type":"chromatic","holes":12,
            "layout":{"blow":["C4","D4","E4","F4","G4","A4","B4","C5","D5","E5","F5","G5"],
                      "draw":["D4","E4","F#4","G4","A4","B4","C#5","D5","E5","F#5","G5","A5"],
                      "blow_slide":["C#4","D#4","F4","F#4","G#4","A#4","C5","C#5","D#5","F5","F#5","G#5"],
                      "draw_slide":["D#4","F4","G4","G#4","A#4","C5","D5","D#5","F5","G5","G#5","A#5"]}}"#,
    )
    .unwrap()
}

#[test]
fn guide_covers_all_twelve_holes_for_a_chromatic_harp() {
    let (holes, _) = build_hole_guide(&c_chromatic_harp(), "C", Progression::Standard);
    assert_eq!(holes.len(), 12);
}

#[test]
fn chord_tone_classes_are_the_dominant_seventh() {
    // C7: C, E, G, Bb(=A#).
    let s = chord_tone_classes("C", ChordQuality::Dominant7);
    assert_eq!(s.len(), 4);
    for c in ["C", "E", "G", "A#"] {
        assert!(s.contains(c), "missing {c}");
    }
    assert!(!s.contains("D"), "major 2nd is not a chord tone");
}

#[test]
fn chord_tone_classes_are_the_minor_seventh_for_minor_quality() {
    // Cm7: C, Eb(=D#), G, Bb(=A#).
    let s = chord_tone_classes("C", ChordQuality::Minor7);
    assert_eq!(s.len(), 4);
    for c in ["C", "D#", "G", "A#"] {
        assert!(s.contains(c), "missing {c}");
    }
    assert!(!s.contains("E"), "major 3rd is not a minor-7th chord tone");
}

#[test]
fn guide_indexes_chord_tones_per_bar_of_the_twelve_bar_cycle() {
    // C 12-bar: bars are [I,I,I,I,IV,IV,I,I,V,IV,I,V] (0-indexed) — see
    // `twelve_bar`. Bar 4 is IV (F7); bar 8 is V (G7).
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Standard);
    assert!(guide.chord_tones_by_bar[0].contains("C"), "bar 0 is I (C7)");
    assert!(
        guide.chord_tones_by_bar[4].contains("F"),
        "bar 4 is IV (F7)"
    );
    assert!(guide.chord_tones_by_bar[8].contains("G"), "bar 8 is V (G7)");
    assert!(
        !guide.chord_tones_by_bar[0].contains("F"),
        "F is not a tone of the I chord"
    );
}

#[test]
fn guide_follows_a_non_standard_progression() {
    // Quick change: bar 1 (0-indexed) moves from I (C7) to IV (F7).
    // C7 = C,E,G,A#; F7 = F,A,C,D# — "E" is the major 3rd of C7 and
    // not a tone of F7 at all, so it distinguishes the two even though
    // both chords happen to share the note C (F7's 5th).
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::QuickChange);
    assert!(
        guide.chord_tones_by_bar[1].contains("F"),
        "quick change moves bar 1 to IV (F7)"
    );
    assert!(!guide.chord_tones_by_bar[1].contains("E"));
}

#[test]
fn guide_uses_minor_seventh_chord_tones_for_a_minor_blues() {
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Minor);
    // Bar 0 is i (Cm7): the minor 3rd (Eb=D#) is a chord tone, the
    // major 3rd (E) is not.
    assert!(guide.chord_tones_by_bar[0].contains("D#"));
    assert!(!guide.chord_tones_by_bar[0].contains("E"));
    // Bar 8 is still V (G7, dominant) even in a minor blues.
    assert!(guide.chord_tones_by_bar[8].contains("B"));
}

#[test]
fn note_fit_orders_chord_tone_above_scale_above_out_of_scale() {
    assert!(NoteFit::ChordTone > NoteFit::InScale);
    assert!(NoteFit::InScale > NoteFit::OutOfScale);
}

#[test]
fn classify_note_fit_prefers_chord_tone_over_plain_scale_membership() {
    let chord_tones = HashSet::from(["C".to_string()]);
    let scale = HashSet::from(["C".to_string(), "E".to_string()]);
    assert_eq!(
        classify_note_fit("C", &chord_tones, &scale),
        NoteFit::ChordTone
    );
    assert_eq!(classify_note_fit("E", &chord_tones, &scale), NoteFit::InScale);
    assert_eq!(
        classify_note_fit("F", &chord_tones, &scale),
        NoteFit::OutOfScale
    );
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

#[test]
fn banner_derives_harp_key_from_hole_1_blow() {
    // c_harp() has no position field → the "no position" wording.
    assert_eq!(
        harp_banner(&c_harp(), "G"),
        "Use a C harmonica  \u{00B7}  key of G"
    );
}

#[test]
fn banner_includes_position_when_present() {
    let harp: Harmonica = serde_json::from_str(
        r#"{"type":"diatonic","holes":10,"bending_profile":"richter_standard","position":"2nd",
            "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                      "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}"#,
    )
    .unwrap();
    // C harp, 2nd position → you play in G: the canonical cross-harp setup.
    assert_eq!(
        harp_banner(&harp, "G"),
        "Use a C harmonica  \u{00B7}  2nd position  \u{00B7}  key of G"
    );
}
