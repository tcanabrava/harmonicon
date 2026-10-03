// SPDX-License-Identifier: MIT

use super::*;

/// The shipped trainer configuration — what every player who never opens
/// the Advanced drawer gets. Most tests here want exactly that, and naming
/// it keeps the ones that *do* vary a knob obvious at a glance.
fn default_settings() -> BendingTrainerSettings {
    BendingTrainerSettings::default()
}

// ── row_to_technique (diagram click → drill target) ──────────────────────

#[test]
fn plain_blow_and_draw_rows_map_directly() {
    assert_eq!(row_to_technique(Row::Blow), Some(Technique::Blow));
    assert_eq!(row_to_technique(Row::Draw), Some(Technique::Draw));
}

#[test]
fn both_bend_wings_collapse_to_the_same_depth() {
    // The diagram distinguishes which wing a bend is on (to read the
    // right reed); the drill target doesn't need that, just the depth.
    for (blow, draw, expected) in [
        (Row::BlowBend(0), Row::DrawBend(0), Technique::Bend1),
        (Row::BlowBend(1), Row::DrawBend(1), Technique::Bend2),
        (Row::BlowBend(2), Row::DrawBend(2), Technique::Bend3),
    ] {
        assert_eq!(row_to_technique(blow), Some(expected));
        assert_eq!(row_to_technique(draw), Some(expected));
    }
}

// ── Drill progress grid (drill_accuracy, progress_tint) ──────────────────

#[test]
fn accuracy_is_none_for_a_never_attempted_target() {
    assert_eq!(drill_accuracy(None), None);
    assert_eq!(drill_accuracy(Some(&DrillStat { attempts: 0, hits: 0, ..default() })), None);
}

#[test]
fn accuracy_is_hit_rate_for_an_attempted_target() {
    assert_eq!(drill_accuracy(Some(&DrillStat { attempts: 4, hits: 3, ..default() })), Some(0.75));
    assert_eq!(drill_accuracy(Some(&DrillStat { attempts: 5, hits: 0, ..default() })), Some(0.0));
}

#[test]
fn untried_tint_matches_the_diagram_idle_color() {
    assert_eq!(progress_tint(None), CELL_DEFAULT);
}

#[test]
fn tint_reddens_toward_zero_and_greens_toward_one() {
    let weak = progress_tint(Some(0.0)).to_srgba();
    let strong = progress_tint(Some(1.0)).to_srgba();
    assert!(weak.red > weak.green, "0% accuracy should read as red");
    assert!(strong.green > strong.red, "100% accuracy should read as green");
    // Never attempted stays visually distinct from a 0%-accuracy target.
    assert_ne!(progress_tint(Some(0.0)), progress_tint(None));
}

#[test]
fn overblow_and_overdraw_rows_both_map_to_over() {
    assert_eq!(row_to_technique(Row::Overblow), Some(Technique::Over));
    assert_eq!(row_to_technique(Row::Overdraw), Some(Technique::Over));
}

// ── Drill persistence (Technique storage key, stats <-> profile) ─────────

#[test]
fn every_technique_storage_key_round_trips() {
    for &t in &ALL_TECHNIQUES {
        assert_eq!(Technique::from_storage_key(t.storage_key()), Some(t));
    }
}

#[test]
fn unknown_storage_key_is_none() {
    assert_eq!(Technique::from_storage_key("nonsense"), None);
}

#[test]
fn drill_key_combines_hole_and_technique() {
    assert_eq!(drill_key(2, Technique::Bend1), "2:bend1");
    assert_eq!(drill_key(10, Technique::Over), "10:over");
}

#[test]
fn stats_round_trip_through_the_profile_shape() {
    let mut stats = std::collections::HashMap::new();
    stats.insert(
        (2u8, Technique::Bend1),
        DrillStat {
            attempts: 5,
            hits: 3,
            skips: 2,
            recent_control: 0.7,
            recent_samples: 4,
            recent_stability_cents: 3.5,
            practiced_at: 6,
        },
    );
    stats.insert((9u8, Technique::Over), DrillStat { attempts: 1, hits: 0, ..default() });

    let profile = stats_to_profile(&stats);
    assert_eq!(profile.len(), 2);
    assert_eq!(
        profile["2:bend1"],
        DrillRecord {
            attempts: 5,
            hits: 3,
            skips: 2,
            recent_control: 0.7,
            recent_samples: 4,
            recent_stability_cents: 3.5,
            practiced_at: 6,
        }
    );

    let restored = stats_from_profile(&profile);
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[&(2, Technique::Bend1)].attempts, 5);
    assert_eq!(restored[&(2, Technique::Bend1)].hits, 3);
    assert_eq!(restored[&(9, Technique::Over)].attempts, 1);
}

#[test]
fn unparsable_profile_entries_are_dropped_not_fatal() {
    let mut profile = std::collections::HashMap::new();
    profile.insert("not-a-key".to_string(), DrillRecord { attempts: 1, hits: 1, ..default() });
    profile
        .insert("2:not-a-technique".to_string(), DrillRecord { attempts: 1, hits: 1, ..default() });
    profile.insert("2:bend1".to_string(), DrillRecord { attempts: 4, hits: 2, ..default() });
    let stats = stats_from_profile(&profile);
    assert_eq!(stats.len(), 1, "only the one well-formed entry should survive");
    assert!(stats.contains_key(&(2, Technique::Bend1)));
}

// `key_offset` is now `harmonicon_core::harmonica::key_offset` — its
// octave-folding behaviour is tested there, not duplicated here.
// `richter_harp`'s own reference layout (C/D/G hole-1 pitches) is tested
// once, centrally, in `song::harmonica::tests` — not duplicated here; this
// module's own tests below only care that bending-trainer logic built atop
// `richter_harp` (target resolution, valid targets, hints) behaves
// correctly, not that `richter_harp` itself is correct.

#[test]
fn target_note_reads_the_right_technique_off_the_harp() {
    let harp = richter_harp("C");
    // Hole 1: blow C4, draw D4, single ½-step bend C#4, overblow D#4.
    assert_eq!(
        target_note(&harp, TrainerTarget { hole: 1, technique: Technique::Blow }).as_deref(),
        Some("C4")
    );
    assert_eq!(
        target_note(&harp, TrainerTarget { hole: 1, technique: Technique::Bend1 }).as_deref(),
        Some("C#4")
    );
    // Hole 5 has no bend (blow E5, draw F5 are a semitone apart).
    assert_eq!(target_note(&harp, TrainerTarget { hole: 5, technique: Technique::Bend1 }), None);
}

#[test]
fn natural_anchor_uses_the_reed_direction_that_creates_the_technique() {
    let harp = richter_harp("C");
    assert_eq!(
        natural_note_for_target(&harp, TrainerTarget { hole: 2, technique: Technique::Bend1 },)
            .as_deref(),
        Some("G4")
    );
    assert_eq!(
        natural_note_for_target(&harp, TrainerTarget { hole: 8, technique: Technique::Bend1 },)
            .as_deref(),
        Some("E6")
    );
    assert_eq!(
        natural_note_for_target(&harp, TrainerTarget { hole: 4, technique: Technique::Over },)
            .as_deref(),
        Some("C5")
    );
    assert_eq!(
        natural_note_for_target(&harp, TrainerTarget { hole: 8, technique: Technique::Over },)
            .as_deref(),
        Some("D6")
    );
}

#[test]
fn note_freq_hz_matches_concert_pitch() {
    assert!((note_freq_hz("A4").unwrap() - 440.0).abs() < 0.01);
    // One semitone below A4.
    assert!((note_freq_hz("G#4").unwrap() - 415.30).abs() < 0.1);
}

fn detected(note: &str) -> harmonicon_audio::pitch_detect::PitchInfo {
    let midi = note_to_midi(note).unwrap() as u8;
    let octave = midi as i32 / 12 - 1;
    let name = note.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    harmonicon_audio::pitch_detect::PitchInfo {
        midi,
        note: name,
        octave,
        frequency: harmonicon_core::midi::midi_to_freq_hz(midi as f32),
    }
}

#[test]
fn tuner_observation_distinguishes_silence_wrong_pitch_and_target_family() {
    let harp = richter_harp("C");
    let target = TrainerTarget { hole: 2, technique: Technique::Bend1 };
    assert_eq!(
        tuner_observation(&harp, target, &ActivePitches::default(), 0.0),
        Some(TunerObservation::Silent)
    );
    assert_eq!(
        tuner_observation(&harp, target, &ActivePitches(vec![detected("C6")]), 0.0),
        Some(TunerObservation::WrongPitch("C6".to_string()))
    );
    assert!(matches!(
        tuner_observation(&harp, target, &ActivePitches(vec![detected("F#4")]), 0.0),
        Some(TunerObservation::TargetFamily(cents)) if cents.abs() < 0.1
    ));
}

#[test]
fn tuner_observation_accepts_the_natural_anchor_on_the_selected_hole() {
    let harp = richter_harp("C");
    let target = TrainerTarget { hole: 2, technique: Technique::Bend1 };
    assert!(matches!(
        tuner_observation(&harp, target, &ActivePitches(vec![detected("G4")]), 0.0),
        Some(TunerObservation::TargetFamily(cents)) if cents > 90.0
    ));
}

#[test]
fn stability_fit_accepts_smooth_pitch_motion() {
    let samples = (0..=6)
        .map(|index| TraceSample {
            time: index as f32 * 0.025,
            target_cents: 100.0 - index as f32 * 12.0,
        })
        .collect();
    assert!(residual_rms(&samples).unwrap() < 0.01);
}

#[test]
fn stability_fit_rejects_jitter_around_a_pitch() {
    let cents = [0.0, 18.0, -17.0, 16.0, -19.0, 14.0, -15.0];
    let samples = cents
        .iter()
        .enumerate()
        .map(|(index, cents)| TraceSample { time: index as f32 * 0.025, target_cents: *cents })
        .collect();
    assert!(residual_rms(&samples).unwrap() > 10.0);
}

#[test]
fn bend_rail_maps_natural_target_and_overshoot() {
    let natural_cents = 100.0;
    assert!((rail_percent(natural_cents, natural_cents) - 8.0).abs() < 0.01);
    assert!((rail_percent(0.0, natural_cents) - 92.0).abs() < 0.01);
    assert!(rail_percent(-25.0, natural_cents) > 92.0);
}

#[test]
fn deep_bend_rail_names_shallower_slots() {
    let harp = richter_harp("C");
    let whole = TrainerTarget { hole: 3, technique: Technique::Bend2 };
    let deep = TrainerTarget { hole: 3, technique: Technique::Bend3 };
    assert_eq!(intermediate_bend_notes(&harp, whole), ["A#4"]);
    assert_eq!(intermediate_bend_notes(&harp, deep), ["A#4", "A4"]);
}

#[test]
fn find_and_hold_completes_after_a_centered_hold() {
    let mut practice = GesturePractice::for_shape(PracticeShape::FindHold);
    practice.advance(GestureFrame::Target, 0.2, None);
    assert_eq!(practice.phase, GesturePhase::Holding);
    practice.advance(GestureFrame::Target, 0.2, None);
    assert_eq!(practice.phase, GesturePhase::Complete);
}

#[test]
fn bend_and_release_requires_natural_target_natural() {
    let mut practice = GesturePractice::for_shape(PracticeShape::BendRelease);
    practice.advance(GestureFrame::Natural, 0.2, None);
    assert_eq!(practice.phase, GesturePhase::Travel);
    practice.advance(GestureFrame::Target, 0.4, None);
    assert_eq!(practice.phase, GesturePhase::Returning);
    practice.advance(GestureFrame::Natural, 0.2, None);
    assert_eq!(practice.phase, GesturePhase::Complete);
}

#[test]
fn repeated_bends_count_once_per_metronome_beat() {
    let mut practice = GesturePractice::for_shape(PracticeShape::Repeated);
    for beat in 0..4 {
        practice.advance(GestureFrame::Target, 0.1, Some(beat));
        practice.advance(GestureFrame::Target, 0.1, Some(beat));
    }
    assert_eq!(practice.phase, GesturePhase::Complete);
}

#[test]
fn bend_ladder_requires_slots_before_returning() {
    let mut practice = GesturePractice::for_shape(PracticeShape::Ladder);
    practice.advance(GestureFrame::Natural, 0.1, None);
    practice.advance(GestureFrame::Slot(0), 0.1, None);
    practice.advance(GestureFrame::Target, 0.1, None);
    assert_eq!(practice.phase, GesturePhase::Returning);
    practice.advance(GestureFrame::Natural, 0.1, None);
    assert_eq!(practice.phase, GesturePhase::Complete);
}

#[test]
fn overbend_response_ignores_wrong_pitch_and_finishes_on_release() {
    let mut practice = GesturePractice::for_shape(PracticeShape::OverbendResponse);
    practice.advance(GestureFrame::Natural, 0.2, None);
    practice.advance(GestureFrame::WrongPitch, 0.1, None);
    assert_eq!(practice.phase, GesturePhase::Travel);
    practice.advance(GestureFrame::Target, 0.4, None);
    practice.advance(GestureFrame::Silence, 0.1, None);
    assert_eq!(practice.phase, GesturePhase::Complete);
}

// ── technique_hint ────────────────────────────────────────────────────────

#[test]
fn blow_and_draw_hints_dont_depend_on_hole() {
    assert_eq!(technique_hint_key(Technique::Blow, 1), technique_hint_key(Technique::Blow, 9));
    assert_eq!(technique_hint_key(Technique::Draw, 1), technique_hint_key(Technique::Draw, 9));
}

#[test]
fn bend_hint_direction_matches_the_hole_side() {
    // Holes 1-6 bend by drawing; holes 7-10 bend by blowing.
    assert_eq!(technique_hint_key(Technique::Bend1, 3), "bending-technique-hint-draw-bend-half");
    assert_eq!(technique_hint_key(Technique::Bend1, 8), "bending-technique-hint-blow-bend-half");
}

#[test]
fn bend_hint_wording_deepens_with_technique() {
    assert_ne!(technique_hint_key(Technique::Bend1, 3), technique_hint_key(Technique::Bend2, 3));
    assert_ne!(technique_hint_key(Technique::Bend2, 3), technique_hint_key(Technique::Bend3, 3));
}

#[test]
fn over_hint_names_overblow_or_overdraw_by_hole() {
    // Overblow holes.
    for hole in [1, 4, 5, 6] {
        assert_eq!(technique_hint_key(Technique::Over, hole), "bending-technique-hint-overblow");
    }
    // Overdraw holes.
    for hole in 7..=10 {
        assert_eq!(technique_hint_key(Technique::Over, hole), "bending-technique-hint-overdraw");
    }
    // Holes 2 and 3 support neither.
    for hole in [2, 3] {
        assert_eq!(
            technique_hint_key(Technique::Over, hole),
            "bending-technique-hint-over-unsupported"
        );
    }
}

#[test]
fn drill_stat_weight_favors_never_seen_and_weak_targets() {
    let never = DrillStat::default();
    let mostly_missed = DrillStat { attempts: 10, hits: 1, ..default() };
    let mostly_hit = DrillStat { attempts: 10, hits: 9, ..default() };
    let perfect = DrillStat { attempts: 10, hits: 10, ..default() };
    // Same sequence for all four, so staleness cancels out and the
    // comparison is about control alone.
    assert!(mostly_missed.weight(1) > never.weight(1));
    assert!(never.weight(1) > mostly_hit.weight(1));
    assert!(mostly_hit.weight(1) > perfect.weight(1));
}

#[test]
fn a_target_held_unsteadily_outweighs_one_held_clean() {
    let steady = DrillStat {
        attempts: 10,
        hits: 10,
        recent_samples: 4,
        recent_control: 1.0,
        recent_stability_cents: 1.0,
        practiced_at: 1,
        ..default()
    };
    let wobbly = DrillStat { recent_stability_cents: 9.0, ..steady };
    assert!(wobbly.weight(1) > steady.weight(1));
}

#[test]
fn a_stale_target_comes_back_without_any_miss_recorded() {
    let mastered = DrillStat {
        attempts: 10,
        hits: 10,
        recent_samples: 4,
        recent_control: 1.0,
        practiced_at: 1,
        ..default()
    };
    let fresh = mastered.weight(2);
    let stale = mastered.weight(2 + STALE_SPAN as u32);
    assert!(stale > fresh);
    // Bounded: staleness nudges a mastered target back into rotation, it
    // doesn't let it outrank a never-attempted one.
    assert!(stale < DrillStat::default().weight(999));
}

#[test]
fn next_sequence_advances_past_every_stamp() {
    let mut stats = std::collections::HashMap::new();
    assert_eq!(next_sequence(&stats), 1);
    stats.insert((2, Technique::Bend1), DrillStat { practiced_at: 4, ..default() });
    stats.insert((3, Technique::Bend2), DrillStat { practiced_at: 9, ..default() });
    assert_eq!(next_sequence(&stats), 10);
}

#[test]
fn a_skip_records_no_control_evidence() {
    let mut stat = DrillStat::default();
    stat.record_skip(1);
    assert_eq!(stat.skips, 1);
    assert_eq!(stat.attempts, 0);
    assert_eq!(stat.hits, 0);
    assert_eq!(stat.recent_samples, 0);
    // Nothing was heard, so nothing is known — the target still reads as
    // never-attempted rather than as a failure.
    assert_eq!(stat.weight(1), DrillStat::default().weight(1));
    assert_eq!(drill_accuracy(Some(&stat)), None);
}

#[test]
fn valid_targets_excludes_bends_the_harp_cant_produce() {
    let harp = richter_harp("C");
    let targets = valid_targets(&harp);
    assert!(targets.iter().any(|t| t.hole == 1 && t.technique == Technique::Blow));
    // Hole 5 has no bend on a Richter-tuned harp.
    assert!(!targets.iter().any(|t| t.hole == 5 && t.technique == Technique::Bend1));
}

#[test]
fn pick_next_target_avoids_immediate_repeat_when_alternatives_exist() {
    let harp = richter_harp("C");
    let stats = std::collections::HashMap::new();
    let avoid = TrainerTarget { hole: 2, technique: Technique::Bend1 };
    let empty = HashSet::new();
    for _ in 0..20 {
        let picked =
            pick_next_target(&harp, &stats, Some(avoid), DrillScope::AllBends, &empty, avoid)
                .expect("a target exists");
        assert_ne!((picked.hole, picked.technique), (avoid.hole, avoid.technique));
    }
}

#[test]
fn novice_scope_contains_only_common_draw_bends() {
    let harp = richter_harp("C");
    let selected = TrainerTarget { hole: 2, technique: Technique::Bend1 };
    let targets = targets_for_scope(&harp, DrillScope::FirstBends, &HashSet::new(), selected);
    assert_eq!(targets.len(), 3);
    assert!(targets.iter().all(|target| target.hole == 2 || target.hole == 3));
    assert!(targets.iter().all(|target| target.technique != Technique::Over));
}

#[test]
fn no_scope_but_overbends_serves_an_overbend() {
    // A novice must never be handed an overdraw by the default scope.
    let harp = richter_harp("C");
    let selected = TrainerTarget::default();
    for scope in [DrillScope::FirstBends, DrillScope::AllBends, DrillScope::BlowBends] {
        assert!(
            targets_for_scope(&harp, scope, &HashSet::new(), selected)
                .iter()
                .all(|target| target.technique != Technique::Over),
            "{scope:?} served an overbend"
        );
    }
    assert!(
        targets_for_scope(&harp, DrillScope::Overbends, &HashSet::new(), selected)
            .iter()
            .all(|target| target.technique == Technique::Over)
    );
}

#[test]
fn custom_scope_is_the_picked_cells_and_never_empty() {
    let harp = richter_harp("C");
    let selected = TrainerTarget { hole: 4, technique: Technique::Draw };
    // No cells picked yet: the scope still has the selected one to serve,
    // rather than stranding the drill with an empty pool.
    assert_eq!(
        targets_for_scope(&harp, DrillScope::Custom, &HashSet::new(), selected),
        vec![selected]
    );

    let picked: HashSet<_> = [(2, Technique::Bend1), (8, Technique::Bend1)].into();
    let targets = targets_for_scope(&harp, DrillScope::Custom, &picked, selected);
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|target| picked.contains(&(target.hole, target.technique))));
    // The selected cell is not in the set, so it isn't drilled.
    assert!(!targets.contains(&selected));
}

#[test]
fn recent_control_outgrows_old_beginner_misses() {
    let mut stat = DrillStat { attempts: 20, hits: 2, ..default() };
    let old_weight = stat.weight(1);
    for sequence in 1..=8 {
        stat.record_attempt(true, Some(2.0), sequence);
    }
    assert!(stat.weight(8) < old_weight);
}

#[test]
fn an_unmeasurable_hold_leaves_the_steadiness_estimate_alone() {
    let mut stat = DrillStat::default();
    stat.record_attempt(true, Some(6.0), 1);
    assert_eq!(stat.recent_stability_cents, 6.0);
    stat.record_attempt(false, None, 2);
    assert_eq!(stat.recent_stability_cents, 6.0);
}

// ── drill_outcome (when an attempt ends, and what it was worth) ───────────

#[test]
fn free_exploration_advances_on_a_bare_hold() {
    assert_eq!(
        drill_outcome(
            PracticeShape::Free,
            GesturePhase::Waiting,
            default_settings().hold_secs,
            1.0,
            true,
            &default_settings(),
        ),
        Some(DrillOutcome::Controlled)
    );
    assert_eq!(
        drill_outcome(
            PracticeShape::Free,
            GesturePhase::Waiting,
            0.1,
            1.0,
            true,
            &default_settings(),
        ),
        None
    );
}

#[test]
fn a_structured_shape_waits_for_the_whole_gesture() {
    // Holding the target is a *stage* of bend-and-release, not the finish
    // line — advancing here would swap the target out mid-gesture.
    for phase in [
        GesturePhase::Waiting,
        GesturePhase::Travel,
        GesturePhase::Holding,
        GesturePhase::Returning,
    ] {
        assert_eq!(
            drill_outcome(PracticeShape::BendRelease, phase, 10.0, 1.0, true, &default_settings(),),
            None,
            "{phase:?} ended the attempt early"
        );
    }
    assert_eq!(
        drill_outcome(
            PracticeShape::BendRelease,
            GesturePhase::Complete,
            0.0,
            1.0,
            true,
            &default_settings(),
        ),
        Some(DrillOutcome::Controlled)
    );
}

#[test]
fn a_timeout_with_nothing_heard_is_a_skip_not_a_miss() {
    assert_eq!(
        drill_outcome(
            PracticeShape::Free,
            GesturePhase::Waiting,
            0.0,
            default_settings().timeout_secs,
            false,
            &default_settings(),
        ),
        Some(DrillOutcome::Skipped)
    );
    assert_eq!(
        drill_outcome(
            PracticeShape::Free,
            GesturePhase::Waiting,
            0.0,
            default_settings().timeout_secs,
            true,
            &default_settings(),
        ),
        Some(DrillOutcome::Missed)
    );
}

// ── key_labels (Key combobox options) ─────────────────────────────────────

#[test]
fn key_labels_matches_the_12_chromatic_note_names() {
    let labels = key_labels();
    assert_eq!(labels.len(), 12);
    assert_eq!(labels, NOTE_NAMES.iter().map(|s| s.to_string()).collect::<Vec<_>>());
}

// ── Advanced settings: defaults, reference shift, analysis ───────────────

#[test]
fn the_default_reference_shifts_nothing() {
    assert_eq!(reference_shift_cents(&default_settings(), "C", 2), 0.0);
}

#[test]
fn raising_a4_moves_every_target_up_by_the_same_cents() {
    let mut settings = default_settings();
    settings.a4_hz = 466.16; // one semitone above 440
    let shift = reference_shift_cents(&settings, "C", 2);
    assert!((shift - 100.0).abs() < 0.5, "got {shift}");
    // The shift is a property of the reference, not of the hole.
    assert_eq!(reference_shift_cents(&settings, "C", 9), reference_shift_cents(&settings, "C", 2));
}

#[test]
fn a_measured_reed_centre_only_shifts_its_own_harp_and_hole() {
    let mut settings = default_settings();
    settings.natural_center_cents.insert(BendingTrainerSettings::center_key("C", 2), -7.0);
    assert_eq!(reference_shift_cents(&settings, "C", 2), -7.0);
    assert_eq!(reference_shift_cents(&settings, "C", 3), 0.0);
    assert_eq!(reference_shift_cents(&settings, "G", 2), 0.0);
}

#[test]
fn the_reference_shift_is_subtracted_from_the_reported_distance() {
    // A harp tuned 20 cents sharp, told so: a reading that is 20 cents
    // sharp of the table is dead on *that* harp.
    let harp = richter_harp("C");
    let target = TrainerTarget { hole: 2, technique: Technique::Bend1 };
    let plain = tuner_observation(&harp, target, &ActivePitches(vec![detected("F#4")]), 0.0);
    let shifted = tuner_observation(&harp, target, &ActivePitches(vec![detected("F#4")]), 20.0);
    let (TunerObservation::TargetFamily(a), TunerObservation::TargetFamily(b)) =
        (plain.unwrap(), shifted.unwrap())
    else {
        panic!("both readings should land in the target family");
    };
    assert!((a - b - 20.0).abs() < 1e-3);
}

#[test]
fn the_natural_check_averages_its_samples_into_one_centre() {
    assert_eq!(observed_center_cents(&[]), None);
    assert_eq!(observed_center_cents(&[-5.0, -5.0]), None, "too few samples");
    let centre = observed_center_cents(&[-6.0, -4.0, -5.0, -5.0, -5.0]).unwrap();
    assert!((centre - (-5.0)).abs() < 1e-5);
}

fn trace_of(points: &[(f32, f32)]) -> std::collections::VecDeque<TraceSample> {
    points.iter().map(|&(time, target_cents)| TraceSample { time, target_cents }).collect()
}

#[test]
fn attempt_stability_reports_lean_and_scatter_separately() {
    // Consistently 10 cents flat, but rock steady: a lean, not scatter.
    let steady =
        trace_of(&[(0.0, -10.0), (0.05, -10.0), (0.10, -10.0), (0.15, -10.0), (0.20, -10.0)]);
    let s = attempt_stability(&steady).unwrap();
    assert!((s.mean_cents - (-10.0)).abs() < 1e-5);
    assert!(s.spread_cents < 1e-5);

    // Centred on average, but all over the place: no lean, wide scatter.
    let scattered =
        trace_of(&[(0.0, -20.0), (0.05, 20.0), (0.10, -20.0), (0.15, 20.0), (0.20, 0.0)]);
    let s = attempt_stability(&scattered).unwrap();
    assert!(s.mean_cents.abs() < 1e-5);
    assert!((s.spread_cents - 40.0).abs() < 1e-5);
}

#[test]
fn attempt_stability_needs_samples_before_it_claims_anything() {
    assert_eq!(attempt_stability(&trace_of(&[(0.0, 0.0)])), None);
}

#[test]
fn vibrato_reads_rate_and_depth_off_a_clean_oscillation() {
    // 5 Hz, ±15 cents: two mean crossings per cycle over half a second.
    let samples: Vec<(f32, f32)> = (0..50)
        .map(|i| {
            let t = i as f32 * 0.01;
            (t, 15.0 * (std::f32::consts::TAU * 5.0 * t).sin())
        })
        .collect();
    let v = vibrato(&trace_of(&samples)).expect("a clean wobble is measurable");
    assert!((v.rate_hz - 5.0).abs() < 0.6, "rate was {}", v.rate_hz);
    assert!((v.depth_cents - 30.0).abs() < 2.0, "depth was {}", v.depth_cents);
}

#[test]
fn a_steady_hold_reports_no_vibrato() {
    let steady = trace_of(&[(0.0, 1.0), (0.05, -1.0), (0.10, 1.0), (0.15, -1.0), (0.20, 1.0)]);
    // It crosses the mean constantly, but two cents of detector noise is
    // not a vibrato — depth is what separates the two.
    assert_eq!(vibrato(&steady), None);
}

#[test]
fn smoothing_is_off_by_default_and_lags_when_turned_up() {
    let samples = trace_of(&[(0.0, 0.0), (0.1, 100.0), (0.2, 100.0)]);
    assert_eq!(
        smoothed_cents(&samples, 0.0),
        vec![0.0, 100.0, 100.0],
        "the default must draw the raw path"
    );
    let smoothed = smoothed_cents(&samples, 0.75);
    assert_eq!(smoothed[0], 0.0, "the first sample has nothing to lag");
    assert!(smoothed[1] < 100.0 && smoothed[1] > 0.0);
    assert!(smoothed[2] > smoothed[1], "it keeps catching up");
}

#[test]
fn every_advanced_knob_stays_inside_its_own_bounds() {
    for knob in KNOBS {
        let mut low = default_settings();
        let mut high = default_settings();
        for _ in 0..500 {
            knob.nudge(&mut low, -1.0);
            knob.nudge(&mut high, 1.0);
        }
        assert_eq!(
            low.clone().clamped(),
            low,
            "stepping down left a knob outside what `clamped` allows"
        );
        assert_eq!(
            high.clone().clamped(),
            high,
            "stepping up left a knob outside what `clamped` allows"
        );
    }
}

#[test]
fn a_knob_actually_moves_and_comes_back() {
    let mut settings = default_settings();
    AdvancedKnob::Tolerance.nudge(&mut settings, 1.0);
    assert!(settings.tolerance_cents > default_settings().tolerance_cents);
    AdvancedKnob::Tolerance.nudge(&mut settings, -1.0);
    assert_eq!(settings.tolerance_cents, default_settings().tolerance_cents);
}

#[test]
fn every_advanced_label_key_exists_in_every_locale() {
    // The drawer's keys are built from two enums rather than written out at
    // each call site, so a missing one wouldn't show up as a raw literal
    // for `build.rs`'s localization scan to catch.
    let keys: Vec<String> = KNOBS
        .iter()
        .map(|knob| knob.label_key().to_string())
        .chain(
            [
                "bending-setup-button",
                "bending-setup-summary",
                "bending-skip-button",
                "bending-progress-none",
                "bending-progress",
                "bending-adv-toggle",
                "bending-adv-reset",
                "bending-adv-clear-center",
                "bending-adv-stability-heading",
                "bending-adv-mean",
                "bending-adv-spread",
                "bending-adv-best-hold",
                "bending-adv-vibrato",
                "bending-adv-center",
            ]
            .iter()
            .map(|k| (*k).to_string()),
        )
        .collect();
    for locale in ["en-US", "es-ES", "pt-BR"] {
        let path =
            format!("{}/../../assets/locales/{locale}/main/ui.ftl", env!("CARGO_MANIFEST_DIR"));
        let ftl = std::fs::read_to_string(&path).expect("locale file");
        for key in &keys {
            assert!(
                ftl.lines().any(|line| line.starts_with(&format!("{key} ="))),
                "{locale} is missing {key}"
            );
        }
    }
}

// ── Layout, diagram navigation, Skip ─────────────────────────────────────

#[test]
fn taller_than_wide_is_portrait_and_stacks_the_diagram_below() {
    assert_eq!(orientation_for(800.0, 1280.0), TrainerOrientation::Portrait);
    assert_eq!(orientation_for(1920.0, 1080.0), TrainerOrientation::Landscape);
    assert_eq!(orientation_for(1280.0, 800.0), TrainerOrientation::Landscape);
    // Square reads as landscape: the side-by-side layout is the baseline.
    assert_eq!(orientation_for(1000.0, 1000.0), TrainerOrientation::Landscape);
    assert_eq!(body_direction(TrainerOrientation::Portrait), FlexDirection::Column);
    assert_eq!(body_direction(TrainerOrientation::Landscape), FlexDirection::Row);
}

fn t(hole: u8, technique: Technique) -> TrainerTarget {
    TrainerTarget { hole, technique }
}

#[test]
fn every_cell_maps_to_the_row_it_is_drawn_in_and_back() {
    let harp = richter_harp("C");
    for target in valid_targets(&harp) {
        assert_eq!(
            row_to_technique(target_row(target)),
            Some(target.technique),
            "{target:?} doesn't round-trip through its diagram row"
        );
    }
}

#[test]
fn arrows_move_along_the_drawn_row() {
    let harp = richter_harp("C");
    assert_eq!(step_target(&harp, t(2, Technique::Bend1), 1, 0), t(3, Technique::Bend1));
    assert_eq!(step_target(&harp, t(3, Technique::Bend1), -1, 0), t(2, Technique::Bend1));
    // Hole 5 has no bend: Right from hole 4's ½-step draw bend skips to 6.
    assert_eq!(step_target(&harp, t(4, Technique::Bend1), 1, 0), t(6, Technique::Bend1));
}

#[test]
fn right_from_the_last_draw_bend_stays_put_rather_than_crossing_wings() {
    // Holes 7–10 bend by *blowing*, drawn above the blow row — not to the
    // right of hole 6's draw bend on screen, so the arrow finds nothing.
    let harp = richter_harp("C");
    assert_eq!(step_target(&harp, t(6, Technique::Bend1), 1, 0), t(6, Technique::Bend1));
}

#[test]
fn arrows_move_between_drawn_rows_of_one_hole() {
    let harp = richter_harp("C");
    assert_eq!(step_target(&harp, t(2, Technique::Draw), 0, 1), t(2, Technique::Bend1));
    assert_eq!(step_target(&harp, t(2, Technique::Draw), 0, -1), t(2, Technique::Blow));
    // Hole 1's rows above blow are empty until overblow: Up skips the gap.
    assert_eq!(step_target(&harp, t(1, Technique::Blow), 0, -1), t(1, Technique::Over));
}

#[test]
fn arrows_stop_at_the_edges() {
    let harp = richter_harp("C");
    assert_eq!(step_target(&harp, t(1, Technique::Blow), -1, 0), t(1, Technique::Blow));
    assert_eq!(step_target(&harp, t(10, Technique::Blow), 1, 0), t(10, Technique::Blow));
    assert_eq!(step_target(&harp, t(1, Technique::Over), 0, -1), t(1, Technique::Over));
}

#[test]
fn choosing_a_cell_selects_it_outside_the_custom_scope() {
    let mut drill = DrillState::default();
    let mut target = t(2, Technique::Bend1);
    choose_cell(&mut drill, &mut target, t(4, Technique::Draw));
    assert_eq!(target, t(4, Technique::Draw));
    assert!(drill.custom.is_empty());
}

#[test]
fn choosing_a_cell_twice_under_custom_adds_then_removes_it() {
    let mut drill = DrillState { scope: DrillScope::Custom, ..default() };
    let mut target = t(2, Technique::Bend1);
    choose_cell(&mut drill, &mut target, t(3, Technique::Bend2));
    assert!(drill.custom.contains(&(3, Technique::Bend2)));
    assert_eq!(target, t(3, Technique::Bend2));
    choose_cell(&mut drill, &mut target, t(3, Technique::Bend2));
    assert!(!drill.custom.contains(&(3, Technique::Bend2)));
}

#[test]
fn a_running_drill_advances_its_timers_without_marking_its_state_changed() {
    // Every drill label, the Skip/intro drawers and the progress line gate
    // on `DrillState::is_changed()`; per-frame timer ticks must not trip it.
    let mut world = World::new();
    world.insert_resource(TrainerKey::default());
    world.insert_resource(TrainerTarget::default());
    world.insert_resource(ActivePitches::default());
    world.insert_resource(BendTrace::default());
    world.insert_resource(GesturePractice::default());
    world.insert_resource(BendingTrainerSettings::default());
    world.insert_resource(DrillState { enabled: true, ..default() });
    world.insert_resource(Time::<()>::default());
    let mut schedule = Schedule::default();
    schedule.add_systems(drill_update);

    world.clear_trackers();
    world.resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
    schedule.run(&mut world);

    assert!(
        world.resource::<DrillState>().elapsed_secs > 0.0,
        "the drill ran and timed the attempt"
    );
    assert!(!world.is_resource_changed::<DrillState>());
}

#[test]
fn a_skip_moves_on_without_recording_an_attempt() {
    let harp = richter_harp("C");
    let mut drill = DrillState {
        enabled: true,
        streak: 3,
        attempted: true,
        hold_secs: 0.2,
        elapsed_secs: 5.0,
        ..default()
    };
    let start = t(2, Technique::Bend1);
    let mut target = start;
    finish_attempt(&mut drill, &mut target, &harp, DrillOutcome::Skipped, None);
    let stat = drill.stats[&(2, Technique::Bend1)];
    assert_eq!(stat.skips, 1);
    assert_eq!(stat.attempts, 0, "a skip is not evidence of failing to bend");
    assert_eq!(drill.streak, 0);
    assert_ne!(target, start, "the drill moves on to another target");
    assert!(!drill.attempted);
    assert_eq!(drill.elapsed_secs, 0.0);
}
