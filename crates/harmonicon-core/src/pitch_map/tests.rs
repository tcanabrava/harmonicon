// SPDX-License-Identifier: MIT

use super::*;
use crate::harmonica::{chromatic_harp, richter_harp};
use crate::midi::note_to_midi;

fn midi(note: &str) -> u8 {
    note_to_midi(note).unwrap() as u8
}

fn natural(hole: u8, action: Action) -> Option<HoleAssignment> {
    Some(HoleAssignment {
        hole,
        action,
        technique: Technique::Natural,
    })
}

// ── Exact reeds ─────────────────────────────────────────────────────────────

#[test]
fn a_directly_playable_blow_note_maps_to_its_own_reed() {
    // C4 is hole 1 blow on a C Richter harp.
    let harp = richter_harp("C");
    assert_eq!(
        map_pitch_playable(midi("C4"), &harp),
        natural(1, Action::Blow)
    );
}

// ── Bends ───────────────────────────────────────────────────────────────────

#[test]
fn a_semitone_under_a_draw_reed_is_reached_by_bending_it() {
    // Hole 1 draw is D4; C#4 isn't on any reed but is within hole 1's cap.
    let harp = richter_harp("C");
    assert_eq!(
        map_pitch_playable(midi("D4") - 1, &harp),
        Some(HoleAssignment {
            hole: 1,
            action: Action::Draw,
            technique: Technique::Bend(1.0),
        })
    );
}

#[test]
fn a_bend_is_preferred_over_an_overblow_for_the_same_pitch() {
    // The ordering that keeps a beginner melody a beginner melody: any
    // pitch a bend can reach must never resolve to an overblow.
    let harp = richter_harp("C");
    for target in 40u8..=100 {
        if let Some(a) = map_pitch_playable(target, &harp)
            && matches!(a.technique, Technique::Overblow | Technique::Overdraw)
        {
            // Nothing bendable may also produce this pitch — using the real
            // physical rule, not "any hole, any direction": a diatonic harp
            // draw-bends the low holes and blow-bends the high ones, so
            // hole 1 *blow* is not a bend candidate however the arithmetic
            // works out.
            let bendable = [
                (1..=harp.hole_count().min(6), Action::Draw),
                (7..=harp.hole_count(), Action::Blow),
            ]
            .into_iter()
            .any(|(range, action)| {
                range.clone().any(|hole| {
                    harp.wind_direction_midi(hole, &action).is_some_and(|reed| {
                        reed > target
                            && technique_fits_hole(Technique::Bend((reed - target) as f32), hole)
                    })
                })
            });
            assert!(
                !bendable,
                "MIDI {target} resolved to {:?} though a bend reaches it",
                a.technique
            );
        }
    }
}

// ── Overblow / overdraw ─────────────────────────────────────────────────────

#[test]
fn an_overblow_reaches_a_note_a_diatonic_harp_otherwise_lacks() {
    // Eb4 on a C harp: hole 1 draw is D4, hole 1 blow C4 — a semitone above
    // the draw reed, which is exactly what a hole-1 overblow produces, and
    // outside hole 1's bend range (bends go *down*).
    let harp = richter_harp("C");
    let assignment = map_pitch_playable(midi("Eb4"), &harp).expect("Eb4 via overblow");
    assert_eq!(assignment.technique, Technique::Overblow);
    assert_eq!(assignment.action, Action::Blow, "an overblow is blown");
    assert!(overblow_ok(assignment.hole));
}

#[test]
fn an_overdraw_is_drawn_even_though_it_sounds_above_the_blow_reed() {
    // The breath-direction rule that's easy to get backwards.
    let harp = richter_harp("C");
    let mut seen = false;
    for target in 40u8..=110 {
        if let Some(a) = map_pitch_playable(target, &harp)
            && a.technique == Technique::Overdraw
        {
            seen = true;
            assert_eq!(a.action, Action::Draw);
            assert!(overdraw_ok(a.hole), "overdraw on hole {}", a.hole);
        }
    }
    assert!(seen, "no overdraw resolved anywhere in the harp's range");
}

#[test]
fn every_over_technique_lands_on_a_hole_that_supports_it() {
    for key in HARP_KEYS {
        let harp = richter_harp(key);
        for target in 0u8..=127 {
            let Some(a) = map_pitch_playable(target, &harp) else {
                continue;
            };
            assert!(
                technique_fits_hole(a.technique, a.hole),
                "{key} harp: {:?} on hole {} is not physically available",
                a.technique,
                a.hole
            );
        }
    }
}

/// The property that makes this mapper usable for scoring at all: whatever
/// it returns must actually sound like the pitch that was asked for.
#[test]
fn a_resolved_assignment_sounds_the_pitch_it_was_asked_for() {
    for key in HARP_KEYS {
        for harp in [richter_harp(key), chromatic_harp(key)] {
            for target in 0u8..=127 {
                let Some(a) = map_pitch_playable(target, &harp) else {
                    continue;
                };
                let sounded = match a.technique {
                    Technique::Natural => harp.wind_direction_midi(a.hole, &a.action),
                    Technique::Bend(depth) => harp
                        .wind_direction_midi(a.hole, &a.action)
                        .map(|reed| reed - depth as u8),
                    Technique::Slide => harp
                        .wind_direction_midi(a.hole, &a.action)
                        .map(|reed| reed + 1),
                    Technique::Overblow | Technique::Overdraw => hole_notes(&harp, a.hole)
                        .over
                        .as_deref()
                        .and_then(note_to_midi)
                        .map(|m| m as u8),
                };
                assert_eq!(
                    sounded,
                    Some(target),
                    "{key}: {a:?} does not sound MIDI {target}"
                );
            }
        }
    }
}

// ── Chromatic slide ─────────────────────────────────────────────────────────

#[test]
fn a_chromatic_harp_reaches_the_gap_with_its_slide() {
    let harp = chromatic_harp("C");
    let assignment = map_pitch_playable(midi("C4") + 1, &harp).expect("C#4 via slide");
    assert_eq!(assignment.technique, Technique::Slide);
    assert_eq!(assignment.hole, 1);
}

#[test]
fn a_chromatic_harp_never_resolves_a_diatonic_over_technique() {
    // The harp family is read from the Harmonica itself now, so a chromatic
    // can't accidentally be handed diatonic techniques.
    let harp = chromatic_harp("C");
    for target in 0u8..=127 {
        if let Some(a) = map_pitch_playable(target, &harp) {
            assert!(!matches!(
                a.technique,
                Technique::Overblow | Technique::Overdraw
            ));
        }
    }
}

// ── The strict/lenient split ────────────────────────────────────────────────

#[test]
fn the_strict_variant_rejects_a_pitch_the_harp_cannot_produce() {
    let harp = richter_harp("C");
    assert_eq!(map_pitch_playable(0, &harp), None);
}

#[test]
fn the_lenient_variant_falls_back_to_the_nearest_natural_note() {
    let harp = richter_harp("C");
    let assignment = map_pitch(0, &harp);
    assert_eq!(assignment.technique, Technique::Natural);
    assert!((1..=10).contains(&assignment.hole));
}

#[test]
fn the_two_variants_agree_wherever_the_strict_one_resolves() {
    let harp = richter_harp("C");
    for target in 0u8..=127 {
        if let Some(strict) = map_pitch_playable(target, &harp) {
            assert_eq!(map_pitch(target, &harp), strict);
        }
    }
}

// ── Key fitting ─────────────────────────────────────────────────────────────

#[test]
fn key_fit_is_perfect_when_every_note_is_natural_on_that_harp() {
    let keys = [midi("C4"), midi("E4"), midi("G4")];
    assert_eq!(key_fit_score(&keys, "C", HarpKind::Diatonic), 1.0);
}

#[test]
fn key_fit_is_lower_on_a_harp_the_notes_do_not_belong_to() {
    let keys = [midi("C4"), midi("E4"), midi("G4")];
    assert!(
        key_fit_score(&keys, "F#", HarpKind::Diatonic)
            < key_fit_score(&keys, "C", HarpKind::Diatonic)
    );
}

#[test]
fn key_fit_is_zero_for_no_notes() {
    assert_eq!(key_fit_score(&[], "C", HarpKind::Diatonic), 0.0);
}

#[test]
fn suggest_key_picks_the_harp_the_notes_are_natural_on() {
    let keys = [midi("D4"), midi("F#4"), midi("A4")];
    assert_eq!(suggest_key(&keys, HarpKind::Diatonic), "D");
}

#[test]
fn suggest_key_breaks_ties_by_harp_keys_own_order() {
    assert_eq!(suggest_key(&[], HarpKind::Diatonic), HARP_KEYS[0]);
}

// ── max_bend ─────────────────────────────────────────────────────────────────

#[test]
fn max_bend_matches_the_notes_a_hole_actually_has() {
    // Two descriptions of one physical fact: `max_bend`'s table and the
    // bend notes `hole_notes` derives from the layout. They disagreed —
    // the table capped every hole at 1.5 semitones, so a 2- or 3-semitone
    // bend was unreachable and a plain C-major scale came back "needs a
    // chromatic harmonica" because F and A were judged unplayable on a C
    // diatonic. Key-independent, since Richter tuning repeats the same
    // interval pattern, so every key must agree.
    for key in HARP_KEYS {
        let harp = richter_harp(key);
        for hole in 1..=10u8 {
            let derived = hole_notes(&harp, hole).bends.len() as f32;
            assert_eq!(
                max_bend(hole),
                derived,
                "hole {hole} on a {key} harp has {derived} bend note(s), \
                 max_bend says {}",
                max_bend(hole)
            );
        }
    }
}

#[test]
fn the_three_draw_bends_low_notes_are_reachable() {
    // The specific notes the old cap made unplayable: hole 2 draw down two
    // semitones, hole 3 draw down two and three.
    let harp = richter_harp("C");
    for (note, hole, depth) in [("F4", 2u8, 2.0), ("A4", 3, 2.0), ("Ab4", 3, 3.0)] {
        let target = note_to_midi(note).unwrap() as u8;
        let Some(assignment) = map_pitch_playable(target, &harp) else {
            panic!("{note} is unreachable on a C harp");
        };
        assert_eq!(assignment.hole, hole, "{note} landed on the wrong hole");
        assert_eq!(assignment.action, Action::Draw);
        assert_eq!(assignment.technique, Technique::Bend(depth));
    }
}

#[test]
fn a_c_major_scale_fits_a_c_diatonic() {
    // What the bug looked like from the outside: an imported MIDI of the
    // plainest possible tune was offered a 12-hole chromatic.
    let harp = richter_harp("C");
    for note in ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"] {
        let target = note_to_midi(note).unwrap() as u8;
        assert!(
            map_pitch_playable(target, &harp).is_some(),
            "{note} judged unplayable on a C diatonic"
        );
    }
}

// ── chromatic layout ─────────────────────────────────────────────────────────

#[test]
fn a_chromatic_reaches_every_semitone_across_its_three_octaves() {
    // What a chromatic harmonica is *for*. This was false: the layout was a
    // C major scale ascending one note per hole, which both mistuned the
    // instrument and stopped it at A5 — so 14 of the 37 semitones from C4
    // to C7 were unreachable, and a chromatic scored worse than a diatonic
    // in `convert::suggested_harp`, which is backwards.
    let harp = chromatic_harp("C");
    let unreachable: Vec<i32> = (60..=96)
        .filter(|&p| map_pitch_playable(p as u8, &harp).is_none())
        .collect();
    assert!(
        unreachable.is_empty(),
        "a C chromatic cannot play MIDI {unreachable:?}"
    );
}

#[test]
fn a_chromatic_is_solo_tuned_with_its_repeating_four_hole_group() {
    // The property that makes one fingering work in every octave, and the
    // reason hole 4 blow and hole 5 blow are both C.
    let harp = chromatic_harp("C");
    let blow: Vec<String> = (1..=12)
        .map(|h| harp.wind_direction_label(h, &Action::Blow))
        .collect();
    assert_eq!(
        blow,
        [
            "C4", "E4", "G4", "C5", "C5", "E5", "G5", "C6", "C6", "E6", "G6", "C7"
        ]
    );
}

#[test]
fn the_slide_raises_every_reed_by_exactly_one_semitone() {
    // `map_pitch_playable`'s chromatic arm assumes this — it looks for
    // `target - 1` among the plain reeds rather than consulting the slide
    // tables, so a table that disagreed would silently resolve to the
    // wrong hole.
    for key in HARP_KEYS {
        let harp = chromatic_harp(key);
        let (blow, draw, blow_slide, draw_slide) = match &harp {
            Harmonica::Chromatic {
                layout: Some(l), ..
            } => (
                l.blow.clone().unwrap(),
                l.draw.clone().unwrap(),
                l.blow_slide.clone().unwrap(),
                l.draw_slide.clone().unwrap(),
            ),
            _ => panic!("{key} chromatic has no layout"),
        };
        for (plain, slid) in blow
            .iter()
            .zip(&blow_slide)
            .chain(draw.iter().zip(&draw_slide))
        {
            assert_eq!(
                note_to_midi(slid).unwrap(),
                note_to_midi(plain).unwrap() + 1,
                "{key}: slide on {plain} gave {slid}"
            );
        }
    }
}
