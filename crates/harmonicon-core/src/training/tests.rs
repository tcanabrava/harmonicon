// SPDX-License-Identifier: MIT

use super::*;
use crate::harmonica::richter_harp;
use crate::midi::note_to_midi;
use crate::pitch_map::{map_pitch_playable, max_bend};

fn spec(tier: Tier, holes: &[u8]) -> DrillSpec {
    DrillSpec {
        technique: DrillTechnique::Bend,
        holes: holes.to_vec(),
        tier,
        seed: 4242,
    }
}

fn drill(tier: Tier, holes: &[u8]) -> HarpChart {
    drill_chart(&spec(tier, holes), &richter_harp("C"), "Drill", "Trainer")
        .expect("a C harp can bend holes 1-6")
}

// ── every note must be real ──────────────────────────────────────────────

#[test]
fn every_generated_note_is_playable_on_the_harp_it_was_built_for() {
    // The property the whole generator rests on. A drill asking for a note
    // the instrument cannot make is worse than no drill: the player fails
    // it and cannot tell whether it was them.
    let harp = richter_harp("C");
    for tier in Tier::ALL {
        let chart = drill(tier, &[2, 3, 4]);
        for item in &chart.track {
            let event = &item.events[0];
            let sounded = sounded_pitch(event, &harp)
                .unwrap_or_else(|| panic!("{tier:?}: {event:?} resolves to no pitch"));
            assert!(
                map_pitch_playable(sounded, &harp).is_some(),
                "{tier:?}: {event:?} sounds MIDI {sounded}, which this harp cannot play"
            );
        }
    }
}

#[test]
fn a_bent_note_states_the_reed_so_the_bend_is_applied_once() {
    // The convention every hand-authored chart follows, and the one the
    // score importer got wrong: both readers *add* the modifier to `note`,
    // so stating the bent pitch bends it twice.
    let harp = richter_harp("C");
    let chart = drill(Tier::Isolate, &[3]);
    let bent = chart
        .track
        .iter()
        .find(|i| i.events[0].modifiers.is_some())
        .expect("an isolate drill must contain a bend");
    let event = &bent.events[0];
    assert_eq!(
        event.note.as_deref(),
        Some("B4"),
        "hole 3 draw's reed is B4; the bend modifier moves it down from there"
    );
    assert_eq!(
        sounded_pitch(event, &harp),
        note_to_midi("A#4").map(|m| m as u8)
    );
}

#[test]
fn a_plain_note_carries_no_modifier() {
    let chart = drill(Tier::Isolate, &[3]);
    let plain = chart
        .track
        .iter()
        .find(|i| i.events[0].modifiers.is_none())
        .expect("a bend drill must contain its reference note");
    assert_eq!(plain.events[0].note.as_deref(), Some("B4"));
}

#[test]
fn no_drill_asks_for_a_bend_deeper_than_the_hole_allows() {
    let harp = richter_harp("C");
    for tier in Tier::ALL {
        for item in &drill(tier, &[1, 2, 3, 4, 5, 6]).track {
            let event = &item.events[0];
            let depth = event
                .modifiers
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .find_map(|m| match m {
                    Modifier::Bend { semitones, .. } => Some(-semitones),
                    _ => None,
                })
                .unwrap_or(0.0);
            assert!(
                depth <= max_bend(event.hole),
                "{tier:?}: hole {} asked for {depth} semitones, max is {}",
                event.hole,
                max_bend(event.hole)
            );
            let _ = &harp;
        }
    }
}

#[test]
fn a_hole_that_cannot_bend_yields_no_drill() {
    // Hole 5's reeds are a semitone apart, so there is no bend note to
    // reach. Returning `None` beats generating a drill of plain notes that
    // silently trains nothing.
    assert!(
        drill_chart(
            &spec(Tier::Isolate, &[5]),
            &richter_harp("C"),
            "Drill",
            "Trainer"
        )
        .is_none()
    );
    assert!(
        drill_chart(&spec(Tier::Isolate, &[]), &richter_harp("C"), "D", "T").is_none(),
        "no holes at all is not a drill either"
    );
}

// ── the ladder ───────────────────────────────────────────────────────────

#[test]
fn each_tier_asks_for_notes_faster_than_the_one_before() {
    // Note *rate*, not BPM: tempo and subdivision both feed it, and an
    // earlier ladder raised both at once — jumping the rate 2.3x in a single
    // step, which is not a step a player working on bends can take.
    let rates: Vec<f32> = Tier::ALL.iter().map(|t| t.notes_per_second()).collect();
    assert!(
        rates.windows(2).all(|w| w[1] > w[0]),
        "note rate must rise across the ladder, got {rates:?}"
    );
    let steps: Vec<f32> = rates.windows(2).map(|w| w[1] / w[0]).collect();
    assert!(
        steps.iter().all(|s| *s < 1.5),
        "no tier may be half again as fast as the one before it, got {steps:?}"
    );
}

#[test]
fn a_phrase_never_repeats_one_note_three_times() {
    // The degenerate figure the in-context tier used to emit when its
    // "neighbouring hole" resolved to the target's own.
    for tier in Tier::ALL {
        let chart = drill(tier, &[2, 3, 4]);
        for w in chart.track.windows(3) {
            let holes: Vec<u8> = w.iter().map(|i| i.events[0].hole).collect();
            let plain = w.iter().all(|i| i.events[0].modifiers.is_none());
            assert!(
                !(plain && holes[0] == holes[1] && holes[1] == holes[2]),
                "{tier:?}: three identical plain notes in a row on hole {}",
                holes[0]
            );
        }
    }
}

#[test]
fn the_isolating_tiers_use_one_hole_and_the_rest_use_them_all() {
    let isolate = drill(Tier::Isolate, &[2, 3, 4]);
    let holes: std::collections::HashSet<u8> =
        isolate.track.iter().map(|i| i.events[0].hole).collect();
    assert_eq!(holes.len(), 1, "isolate must not wander across holes");

    let vary = drill(Tier::Vary, &[2, 3, 4]);
    let holes: std::collections::HashSet<u8> =
        vary.track.iter().map(|i| i.events[0].hole).collect();
    assert!(holes.len() > 1, "vary must cover more than one hole");
}

#[test]
fn a_bend_is_always_preceded_somewhere_by_its_own_plain_reed() {
    // The reference note is what makes a bend drill teachable rather than a
    // guess — the ear needs to hear the pitch it is bending away from.
    for tier in Tier::ALL {
        let chart = drill(tier, &[2, 3]);
        for item in &chart.track {
            let e = &item.events[0];
            if e.modifiers.is_some() {
                assert!(
                    chart.track.iter().any(|other| {
                        let o = &other.events[0];
                        o.hole == e.hole && o.modifiers.is_none()
                    }),
                    "{tier:?}: hole {} is bent but never played plain",
                    e.hole
                );
            }
        }
    }
}

#[test]
fn a_tier_fills_the_bars_it_claims() {
    for tier in Tier::ALL {
        let chart = drill(tier, &[2, 3]);
        assert_eq!(
            chart.track.len(),
            tier.bars() * 4 * tier.notes_per_beat(),
            "{tier:?} produced the wrong number of notes"
        );
    }
}

#[test]
fn notes_are_evenly_spaced_at_the_tiers_tempo() {
    let tier = Tier::Vary;
    let chart = drill(tier, &[2, 3]);
    let expected = 60.0 / f64::from(tier.bpm()) / tier.notes_per_beat() as f64;
    assert_eq!(chart.song.tempo_bpm, tier.bpm());
    assert!((chart.track[1].time.unwrap() - chart.track[0].time.unwrap() - expected).abs() < 1e-9);
}

#[test]
fn a_tier_number_round_trips() {
    for tier in Tier::ALL {
        assert_eq!(Tier::from_number(tier.number()), Some(tier));
    }
    assert_eq!(Tier::from_number(0), None);
    assert_eq!(Tier::from_number(6), None);
}

// ── determinism ──────────────────────────────────────────────────────────

#[test]
fn the_same_seed_gives_the_same_exercise() {
    // A retry has to be the same drill, or a tier's pass threshold compares
    // two different exercises and means nothing.
    let harp = richter_harp("C");
    let a = drill_chart(&spec(Tier::Interleave, &[2, 3, 4]), &harp, "D", "T").unwrap();
    let b = drill_chart(&spec(Tier::Interleave, &[2, 3, 4]), &harp, "D", "T").unwrap();
    let holes = |c: &HarpChart| -> Vec<(u8, bool)> {
        c.track
            .iter()
            .map(|i| (i.events[0].hole, i.events[0].modifiers.is_some()))
            .collect()
    };
    assert_eq!(holes(&a), holes(&b));
}

#[test]
fn a_different_seed_gives_a_different_order() {
    let harp = richter_harp("C");
    let mut one = spec(Tier::Interleave, &[2, 3, 4]);
    let mut two = one.clone();
    one.seed = 1;
    two.seed = 99;
    let holes = |s: &DrillSpec| -> Vec<(u8, bool)> {
        drill_chart(s, &harp, "D", "T")
            .unwrap()
            .track
            .iter()
            .map(|i| (i.events[0].hole, i.events[0].modifiers.is_some()))
            .collect()
    };
    assert_ne!(holes(&one), holes(&two));
}

// ── agreement with the resolver ──────────────────────────────────────────

#[test]
fn bend_action_matches_the_resolver() {
    // Two statements of one physical fact — a bend pulls toward the other
    // reed in the hole — so they must not drift. `map_pitch_playable` is the
    // authority; this pins `bend_action` against it.
    let harp = richter_harp("C");
    for hole in 1..=10u8 {
        for depth in depths_available(&harp, hole) {
            let reed = harp
                .wind_direction_midi(hole, &bend_action(hole))
                .expect("a reed");
            let target = reed - depth;
            let resolved = map_pitch_playable(target, &harp).expect("a reachable bend");
            if resolved.hole == hole {
                assert_eq!(
                    resolved.action,
                    bend_action(hole),
                    "hole {hole} bends on the other breath than the resolver thinks"
                );
            }
        }
    }
}

#[test]
fn depths_come_from_the_layout_not_a_table() {
    // Hole 3 bends three semitones and hole 1 only one, which is a fact
    // about where the reeds sit rather than a rule written down twice.
    let harp = richter_harp("C");
    assert_eq!(depths_available(&harp, 3), vec![1, 2, 3]);
    assert_eq!(depths_available(&harp, 1), vec![1]);
    assert!(depths_available(&harp, 5).is_empty());
}
