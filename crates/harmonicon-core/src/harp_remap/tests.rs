// SPDX-License-Identifier: MIT

use super::*;
use crate::harmonica::{chromatic_harp, richter_harp};
use crate::pitch_map::HARP_KEYS;

fn bend(semitones: f32) -> Modifier {
    Modifier::Bend { semitones, intensity: None }
}

/// Every (hole, action) a diatonic harp actually has a note for.
fn playable_positions(harp: &Harmonica) -> Vec<(u8, Action)> {
    let mut out = Vec::new();
    for hole in 1..=harp.hole_count() {
        for action in [Action::Blow, Action::Draw] {
            if harp.wind_direction_midi(hole, &action).is_some() {
                out.push((hole, action));
            }
        }
    }
    out
}

// ── The identity case ───────────────────────────────────────────────────────

#[test]
fn the_same_harp_changes_nothing_in_either_mode() {
    // The default has to be free: choosing your own chart's harp must be
    // indistinguishable from not choosing at all.
    let harp = richter_harp("C");
    for (hole, action) in playable_positions(&harp) {
        let natural = harp.wind_direction_label(hole, &action);
        for mapping in HarpMapping::all() {
            let out = remap_event(hole, action, Some(&natural), &[], &harp, &harp, *mapping);
            assert_eq!(out.hole, hole, "{mapping:?} moved a hole");
            assert_eq!(out.action, action, "{mapping:?} changed breath");
            assert!(out.playable);
            assert_eq!(
                out.midi,
                harp.wind_direction_midi(hole, &action),
                "{mapping:?} changed the sounding pitch"
            );
        }
    }
}

// ── Same holes: the tab is preserved, the music moves ───────────────────────

#[test]
fn same_holes_keeps_the_tab_and_lets_the_pitch_follow_the_new_harp() {
    let c = richter_harp("C");
    let g = richter_harp("G");
    // Hole 4 blow: C5 on a C harp, G4 on a G harp.
    let out = remap_event(4, Action::Blow, Some("C5"), &[], &c, &g, HarpMapping::SameHoles);
    assert_eq!((out.hole, out.action), (4, Action::Blow));
    assert_eq!(out.midi, g.wind_direction_midi(4, &Action::Blow));
    assert_ne!(out.midi, c.wind_direction_midi(4, &Action::Blow));
}

#[test]
fn same_holes_ignores_the_charts_own_note_name() {
    // The failure this whole module exists to prevent, and the one that
    // would hit every bundled chart: all 609 shipped events name their
    // note, and honouring that name would leave the expected pitch on the
    // original harp while the player blows a different one.
    let c = richter_harp("C");
    let a = richter_harp("A");
    let out = remap_event(1, Action::Blow, Some("C4"), &[], &c, &a, HarpMapping::SameHoles);
    assert_eq!(
        out.midi,
        a.wind_direction_midi(1, &Action::Blow),
        "the explicit note name leaked through and pinned the old harp's pitch"
    );
}

#[test]
fn same_holes_refuses_a_hole_the_shorter_harp_does_not_have() {
    let diatonic = richter_harp("C");
    let chromatic = chromatic_harp("C"); // 12 holes
    let out = remap_event(
        12,
        Action::Blow,
        None,
        &[],
        &chromatic,
        &diatonic, // only 10
        HarpMapping::SameHoles,
    );
    assert!(!out.playable, "hole 12 does not exist on a 10-hole harp");
}

#[test]
fn same_holes_keeps_a_bend_as_a_bend() {
    let c = richter_harp("C");
    let g = richter_harp("G");
    let mods = [bend(-1.0)];
    let out = remap_event(3, Action::Draw, Some("B4"), &mods, &c, &g, HarpMapping::SameHoles);
    assert_eq!(out.modifiers, mods.to_vec());
    // A semitone below hole 3 draw on the *G* harp, not the C one.
    let reed = g.wind_direction_midi(3, &Action::Draw).unwrap();
    assert_eq!(out.midi, Some(reed - 1));
}

// ── Transpose: the music is preserved, the tab moves ────────────────────────

#[test]
fn transpose_keeps_the_sounding_pitch_and_moves_the_hole() {
    let c = richter_harp("C");
    let g = richter_harp("G");
    // G4 is hole 3 blow on a C harp. A G harp is pitched *below* C — its
    // hole 1 blow is G3 — so the same pitch lands on hole 4 blow, not hole 1.
    let out = remap_event(3, Action::Blow, Some("G4"), &[], &c, &g, HarpMapping::Transpose);
    assert!(out.playable);
    assert_eq!(out.midi, note_to_midi("G4").map(|m| m as u8));
    assert_eq!(out.hole, 4);
    assert_eq!(out.action, Action::Blow);
}

#[test]
fn transpose_reports_a_note_the_target_harp_cannot_reach_as_unplayable() {
    let c = richter_harp("C");
    let g = richter_harp("G");
    // Far below any G harp reed, and not bendable or overblowable into range.
    let out = remap_event(1, Action::Blow, Some("C2"), &[], &c, &g, HarpMapping::Transpose);
    assert!(!out.playable);
    assert_eq!(
        out.midi,
        note_to_midi("C2").map(|m| m as u8),
        "an unplayable note still reports what it would have sounded"
    );
}

#[test]
fn transpose_drops_a_bend_when_the_new_harp_has_the_note_outright() {
    let c = richter_harp("C");
    // A#4 is a bend on a C harp (hole 3 draw, down one) and a plain blow
    // reed on a Bb harp — hole 4 blow, since a Bb harp's hole 1 blow is Bb3.
    let bb = richter_harp("Bb");
    let out =
        remap_event(3, Action::Draw, Some("B4"), &[bend(-1.0)], &c, &bb, HarpMapping::Transpose);
    assert!(out.playable);
    assert!(
        !out.modifiers.iter().any(|m| matches!(m, Modifier::Bend { .. })),
        "a note the new harp plays naturally should not still be bent: {:?}",
        out.modifiers
    );
}

#[test]
fn transpose_preserves_expression_but_recomputes_pitch_techniques() {
    let c = richter_harp("C");
    let g = richter_harp("G");
    let mods = [bend(-1.0), Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }];
    let out = remap_event(3, Action::Draw, Some("B4"), &mods, &c, &g, HarpMapping::Transpose);
    assert!(
        out.modifiers.iter().any(|m| matches!(m, Modifier::Vibrato { .. })),
        "vibrato is expression and must survive the swap"
    );
}

// ── The microphone invariant ────────────────────────────────────────────────

/// The property everything else rests on: whatever pitch is reported as
/// expected must be one the *target* harp can actually produce. If this ever
/// fails, the game listens for a note the player physically cannot make.
#[test]
fn every_playable_remap_sounds_a_pitch_the_target_harp_can_produce() {
    for source_key in HARP_KEYS {
        let chart_harp = richter_harp(source_key);
        let positions = playable_positions(&chart_harp);
        for target_key in HARP_KEYS {
            for target_harp in [richter_harp(target_key), chromatic_harp(target_key)] {
                let valid = target_harp.build_valid_notes();
                for &(hole, action) in &positions {
                    let natural = chart_harp.wind_direction_label(hole, &action);
                    for mapping in HarpMapping::all() {
                        let out = remap_event(
                            hole,
                            action,
                            Some(&natural),
                            &[],
                            &chart_harp,
                            &target_harp,
                            *mapping,
                        );
                        if !out.playable {
                            continue;
                        }
                        let midi = out.midi.expect("a playable note sounds something");
                        assert!(
                            valid.contains(&midi),
                            "{source_key}->{target_key} {mapping:?}: hole {hole} {action:?} \
                             expects MIDI {midi}, which that harp cannot produce"
                        );
                    }
                }
            }
        }
    }
}

// ── Cost reporting ──────────────────────────────────────────────────────────

#[test]
fn cost_counts_what_a_choice_will_actually_demand() {
    let mut cost = RemapCost::default();
    cost.add(&RemappedEvent {
        hole: 1,
        action: Action::Blow,
        modifiers: vec![],
        midi: Some(60),
        playable: true,
    });
    cost.add(&RemappedEvent {
        hole: 3,
        action: Action::Draw,
        modifiers: vec![bend(-1.0)],
        midi: Some(70),
        playable: true,
    });
    cost.add(&RemappedEvent {
        hole: 4,
        action: Action::Blow,
        modifiers: vec![Modifier::Overblow],
        midi: Some(75),
        playable: true,
    });
    cost.add(&RemappedEvent {
        hole: 1,
        action: Action::Blow,
        modifiers: vec![],
        midi: Some(30),
        playable: false,
    });
    assert_eq!(cost.total, 4);
    assert_eq!(cost.bends, 1);
    assert_eq!(cost.overblows, 1);
    assert_eq!(cost.unplayable, 1);
    assert!(!cost.is_complete());
}

#[test]
fn an_unplayable_note_contributes_no_technique_counts() {
    // It can't demand a bend the player will never get to attempt.
    let mut cost = RemapCost::default();
    cost.add(&RemappedEvent {
        hole: 3,
        action: Action::Draw,
        modifiers: vec![bend(-1.0), Modifier::Overblow],
        midi: Some(70),
        playable: false,
    });
    assert_eq!((cost.bends, cost.overblows), (0, 0));
    assert_eq!(cost.unplayable, 1);
}

#[test]
fn the_charts_own_harp_costs_nothing_extra() {
    let harp = richter_harp("C");
    let mut cost = RemapCost::default();
    for (hole, action) in playable_positions(&harp) {
        let natural = harp.wind_direction_label(hole, &action);
        cost.add(&remap_event(
            hole,
            action,
            Some(&natural),
            &[],
            &harp,
            &harp,
            HarpMapping::SameHoles,
        ));
    }
    assert!(cost.is_complete());
    assert_eq!((cost.bends, cost.overblows), (0, 0));
}

#[test]
fn same_holes_reads_an_overblow_from_the_over_note_not_the_reed() {
    // An overblow does not sound its hole's blow reed, so keeping the tab
    // must re-derive the *over* note on the new harp. Reading the reed
    // would preserve the tab and report a pitch the player won't produce —
    // the same failure as honouring the chart's own note name, reached by a
    // different route.
    let c = richter_harp("C");
    let g = richter_harp("G");
    let out = remap_event(
        4,
        Action::Blow,
        Some("D#5"),
        &[Modifier::Overblow],
        &c,
        &g,
        HarpMapping::SameHoles,
    );
    assert!(out.playable);
    let expected = hole_notes(&g, 4).over.as_deref().and_then(note_to_midi);
    assert_eq!(out.midi.map(|m| m as i32), expected);
    assert_ne!(
        out.midi,
        g.wind_direction_midi(4, &Action::Blow),
        "an overblow must not report its blow reed's pitch"
    );
}

#[test]
fn same_holes_refuses_an_overblow_on_a_hole_that_cannot_overblow() {
    // Holes 2/3 have no overblow. Keeping the tab onto such a hole has to
    // report unplayable rather than inventing a pitch.
    let c = richter_harp("C");
    let out =
        remap_event(2, Action::Blow, None, &[Modifier::Overblow], &c, &c, HarpMapping::SameHoles);
    assert!(!out.playable);
}

#[test]
fn transposing_onto_the_charts_own_harp_never_loses_a_bent_note() {
    // Found on screen: a real chart bends hole 3 draw by -1.5 and -0.5
    // semitones, and transposing it onto its *own* C harp reported two
    // notes as unplayable. Re-resolving the pitch went through max_bend,
    // which caps hole 3 at 1.5, while the chart (and build_valid_notes)
    // treat the whole blow-to-draw gap as bendable.
    let c = richter_harp("C");
    for semitones in [-0.5, -1.0, -1.5, -2.0] {
        let out = remap_event(
            3,
            Action::Draw,
            Some("B4"),
            &[bend(semitones)],
            &c,
            &c,
            HarpMapping::Transpose,
        );
        assert!(
            out.playable,
            "a {semitones} bend on hole 3 draw became unplayable on the chart's own harp"
        );
        assert_eq!((out.hole, out.action), (3, Action::Draw), "the tab moved");
        assert_eq!(
            out.midi,
            source_pitch(3, Action::Draw, Some("B4"), &[bend(semitones)], &c),
            "the sounding pitch changed"
        );
    }
}

#[test]
fn the_same_harp_is_the_identity_for_bent_notes_too() {
    // The broader property the test above is one case of: whatever the
    // chart asked for, asking for it again on the same harmonica must
    // return exactly the same note.
    let c = richter_harp("C");
    for (hole, action) in playable_positions(&c) {
        let natural = c.wind_direction_label(hole, &action);
        for depth in [0.0f32, -0.5, -1.0] {
            let mods: Vec<Modifier> = if depth == 0.0 { vec![] } else { vec![bend(depth)] };
            for mapping in HarpMapping::all() {
                let out = remap_event(hole, action, Some(&natural), &mods, &c, &c, *mapping);
                if !out.playable {
                    // Only legitimate where the chart itself asked for a
                    // bend the hole doesn't have.
                    continue;
                }
                assert_eq!(
                    (out.hole, out.action),
                    (hole, action),
                    "{mapping:?} moved hole {hole} {action:?} on its own harp"
                );
            }
        }
    }
}
