// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use harmonicon_core::harmonica::{ChordQuality, chord_intervals, richter_harp, semitone};

use super::*;

fn chord(root: &str, quality: ChordQuality) -> HashSet<String> {
    chord_intervals(quality).iter().map(|&n| semitone(root, n)).collect()
}

/// The C blues hexatonic, in note-class spelling.
fn blues_scale() -> HashSet<String> {
    ["C", "D#", "F", "F#", "G", "A#"].into_iter().map(String::from).collect()
}

fn c_harp() -> Vec<PlayableNote> {
    playable_notes(&richter_harp("C"))
}

struct Fixture {
    playable: Vec<PlayableNote>,
    opening: HashSet<String>,
    ending: HashSet<String>,
    scale: HashSet<String>,
}

impl Fixture {
    fn new(opening: HashSet<String>, ending: HashSet<String>) -> Self {
        Self { playable: c_harp(), opening, ending, scale: blues_scale() }
    }

    fn ctx(&self, density: CallDensity, swung: bool, seed: u64) -> CallContext<'_> {
        CallContext {
            playable: &self.playable,
            opening_chord: &self.opening,
            ending_chord: &self.ending,
            scale: &self.scale,
            swung,
            density,
            seed,
        }
    }
}

/// The chord pairs a call meets across the standard and quick-change
/// forms: I→I, I→IV, V→IV, and a minor pair.
fn chord_pairs() -> Vec<(HashSet<String>, HashSet<String>)> {
    use ChordQuality::{Dominant7, Minor7};
    vec![
        (chord("C", Dominant7), chord("C", Dominant7)),
        (chord("C", Dominant7), chord("F", Dominant7)),
        (chord("G", Dominant7), chord("F", Dominant7)),
        (chord("C", Minor7), chord("F", Minor7)),
    ]
}

const DENSITIES: [CallDensity; 3] =
    [CallDensity::Sparse, CallDensity::Conversational, CallDensity::Busy];

fn note_fit(note: PlayableNote, chord: &HashSet<String>, scale: &HashSet<String>) -> Fit {
    fit(note.midi, chord, scale)
}

// ── playable_notes ───────────────────────────────────────────────────────

#[test]
fn playable_notes_lists_every_blow_and_draw_of_the_harp() {
    let notes = c_harp();
    assert_eq!(notes.len(), 20);
    assert!(notes.contains(&PlayableNote { midi: 60, hole: 1, blow: true }));
    assert!(notes.contains(&PlayableNote { midi: 67, hole: 2, blow: false }));
    assert!(notes.iter().all(|n| (1..=10).contains(&n.hole)));
}

// ── rhythm grid ──────────────────────────────────────────────────────────

#[test]
fn straight_offbeats_split_the_beat_evenly_and_swung_ones_two_to_one() {
    assert_eq!(slot_tick(0, false), 0);
    assert_eq!(slot_tick(1, false), TICKS_PER_BEAT / 2);
    assert_eq!(slot_tick(1, true), TICKS_PER_BEAT * 2 / 3);
    assert_eq!(slot_tick(2, true), TICKS_PER_BEAT);
    assert_eq!(slot_tick(8, true), 4 * TICKS_PER_BEAT);
}

#[test]
fn every_cell_fits_its_bar_in_order_without_overlap() {
    for density in DENSITIES {
        for cell in openings(density).iter().chain(endings(density)) {
            let mut last_end = 0;
            for &(slot, len) in cell.iter() {
                assert!(len >= 1, "{density:?} {cell:?}");
                assert!(slot >= last_end, "{density:?} {cell:?} overlaps");
                last_end = slot + len;
            }
            assert!(last_end <= SLOTS_PER_BAR, "{density:?} {cell:?}");
        }
    }
}

#[test]
fn every_ending_cell_leaves_two_beats_of_space() {
    for density in DENSITIES {
        for cell in endings(density) {
            let end = cell.iter().map(|&(s, l)| s + l).max().unwrap_or(0);
            assert!(end <= ENDING_LAST_SLOT, "{density:?} {cell:?}");
        }
    }
}

#[test]
fn every_density_has_a_rest_and_a_pickup_in_its_openings() {
    for density in DENSITIES {
        let cells = openings(density);
        assert!(
            cells.iter().any(|c| c.iter().map(|&(_, l)| l).sum::<u8>() < SLOTS_PER_BAR),
            "{density:?} never rests"
        );
        // A pickup: a one-eighth note on an off-beat leading into a longer
        // note on the next beat.
        assert!(
            cells.iter().any(|c| c
                .windows(2)
                .any(|w| w[0].1 == 1 && w[0].0 % 2 == 1 && w[1].0 == w[0].0 + 1 && w[1].1 > 1)),
            "{density:?} has no pickup"
        );
    }
}

#[test]
fn lay_out_marks_held_notes_and_shaves_the_articulation_gap() {
    let n = PlayableNote { midi: 64, hole: 2, blow: true };
    let notes = lay_out(&[(0, 1), (4, 3)], &[n, n], 1, false);
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].tick, TICKS_PER_BAR);
    assert_eq!(notes[0].len, TICKS_PER_BEAT / 2 - NOTE_GAP_TICKS);
    assert!(!notes[0].held);
    assert_eq!(notes[1].tick, TICKS_PER_BAR + 2 * TICKS_PER_BEAT);
    assert_eq!(notes[1].len, 3 * TICKS_PER_BEAT / 2 - NOTE_GAP_TICKS);
    assert!(notes[1].held);
}

// ── transitions ──────────────────────────────────────────────────────────

#[test]
fn a_breath_change_is_playable_on_the_same_or_next_hole_only() {
    let blow = |hole| PlayableNote { midi: 60, hole, blow: true };
    let draw = |hole| PlayableNote { midi: 60, hole, blow: false };
    assert!(transition_ok(blow(4), draw(4)));
    assert!(transition_ok(blow(4), draw(5)));
    assert!(transition_ok(blow(4), blow(6)));
    assert!(!transition_ok(blow(4), draw(6)));
    assert!(!transition_ok(blow(4), blow(7)));
}

// ── generate_call ────────────────────────────────────────────────────────

#[test]
fn the_same_seed_always_yields_the_same_call() {
    let (opening, ending) = chord_pairs().remove(1);
    let fx = Fixture::new(opening, ending);
    let a = generate_call(&fx.ctx(CallDensity::Conversational, true, 42));
    let b = generate_call(&fx.ctx(CallDensity::Conversational, true, 42));
    assert!(!a.is_empty());
    assert_eq!(a, b);
}

#[test]
fn different_seeds_yield_different_calls() {
    let (opening, ending) = chord_pairs().remove(0);
    let fx = Fixture::new(opening, ending);
    let calls: HashSet<Vec<(usize, u8)>> = (0..16u64)
        .map(|seed| {
            generate_call(&fx.ctx(CallDensity::Conversational, true, seed))
                .iter()
                .map(|n| (n.tick, n.note.midi))
                .collect()
        })
        .collect();
    assert!(calls.len() > 4, "only {} distinct calls in 16 seeds", calls.len());
}

#[test]
fn every_call_is_playable_reachable_resolved_and_leaves_space() {
    for (opening, ending) in chord_pairs() {
        let fx = Fixture::new(opening, ending);
        for density in DENSITIES {
            for swung in [false, true] {
                for seed in 0..40u64 {
                    let ctx = fx.ctx(density, swung, seed);
                    let call = generate_call(&ctx);
                    assert!(!call.is_empty(), "{density:?} seed {seed}: empty");

                    for w in call.windows(2) {
                        assert!(
                            w[0].tick + w[0].len <= w[1].tick,
                            "{density:?} seed {seed}: notes overlap"
                        );
                        // A rest of a beat or more is time enough to move
                        // anywhere on the harp; anything tighter must be
                        // one playable move.
                        let rest = w[1].tick - (w[0].tick + w[0].len);
                        assert!(
                            rest >= TICKS_PER_BEAT || transition_ok(w[0].note, w[1].note),
                            "{density:?} seed {seed}: unplayable move {:?} → {:?}",
                            w[0].note,
                            w[1].note
                        );
                    }
                    assert!(call_end_tick(&call) <= call_space_tick());

                    let chord_for = |n: &CallNote| {
                        if n.tick < TICKS_PER_BAR { &fx.opening } else { &fx.ending }
                    };
                    for n in &call {
                        assert!(fx.playable.contains(&n.note), "{density:?} seed {seed}");
                    }
                    for w in call.windows(2) {
                        if note_fit(w[0].note, chord_for(&w[0]), &fx.scale) != Fit::Chord {
                            assert_eq!(
                                note_fit(w[1].note, chord_for(&w[1]), &fx.scale),
                                Fit::Chord,
                                "{density:?} seed {seed}: {:?} doesn't resolve",
                                w[0].note
                            );
                        }
                    }
                    let last = call.last().unwrap();
                    assert_eq!(
                        note_fit(last.note, chord_for(last), &fx.scale),
                        Fit::Chord,
                        "{density:?} seed {seed}: ends off the chord"
                    );

                    let first = call[0].note.midi as i32;
                    for n in call.iter().filter(|n| n.tick < TICKS_PER_BAR) {
                        assert!(
                            (n.note.midi as i32 - first).abs() <= MOTIF_RANGE,
                            "{density:?} seed {seed}: motif wider than a fifth"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn density_changes_how_many_notes_a_call_has() {
    let (opening, ending) = chord_pairs().remove(0);
    let fx = Fixture::new(opening, ending);
    let mean = |density| {
        let total: usize =
            (0..40u64).map(|seed| generate_call(&fx.ctx(density, true, seed)).len()).sum();
        total as f32 / 40.0
    };
    let sparse = mean(CallDensity::Sparse);
    let conversational = mean(CallDensity::Conversational);
    let busy = mean(CallDensity::Busy);
    assert!(sparse < conversational, "{sparse} vs {conversational}");
    assert!(conversational < busy, "{conversational} vs {busy}");
}

#[test]
fn calls_use_rests_held_notes_and_repeated_notes() {
    let (opening, ending) = chord_pairs().remove(0);
    let fx = Fixture::new(opening, ending);
    let calls: Vec<Vec<CallNote>> = (0..40u64)
        .map(|seed| generate_call(&fx.ctx(CallDensity::Conversational, false, seed)))
        .collect();
    assert!(calls.iter().any(|c| c.iter().any(|n| n.held)), "never holds");
    assert!(
        calls.iter().any(|c| c.windows(2).any(|w| w[0].note.midi == w[1].note.midi)),
        "never repeats a note"
    );
    assert!(calls.iter().any(|c| c[0].tick > 0), "never opens with a rest");
}

#[test]
fn a_chord_the_harp_cannot_sound_yields_no_call() {
    let unplayable: HashSet<String> = ["C#".to_string()].into_iter().collect();
    let fx = Fixture::new(unplayable.clone(), unplayable);
    assert!(generate_call(&fx.ctx(CallDensity::Busy, true, 1)).is_empty());
}

// ── contour ──────────────────────────────────────────────────────────────

fn opening_e_g_c() -> Vec<PlayableNote> {
    vec![
        PlayableNote { midi: 64, hole: 2, blow: true },
        PlayableNote { midi: 67, hole: 3, blow: true },
        PlayableNote { midi: 72, hole: 4, blow: true },
    ]
}

#[test]
fn a_repeat_answer_over_the_same_chord_sings_the_motif_again() {
    let (opening, ending) = chord_pairs().remove(0);
    let fx = Fixture::new(opening, ending);
    let ctx = fx.ctx(CallDensity::Conversational, true, 7);
    let answer =
        answer_pitches(&ctx, &fx.ending, &opening_e_g_c(), 3, Contour::Repeat, &mut Rng(7));
    let midis: Vec<u8> = answer.iter().map(|n| n.midi).collect();
    assert_eq!(midis, vec![64, 67, 72]);
}

#[test]
fn up_and_down_answers_move_the_anchor_and_keep_the_contour() {
    let (opening, ending) = chord_pairs().remove(0);
    let fx = Fixture::new(opening, ending);
    let ctx = fx.ctx(CallDensity::Conversational, true, 7);
    // E4 → G4, a rising two-note motif ending on hole 3, from where both
    // hole 1 (down) and hole 2's draw (up) are one easy move away.
    let motif = &opening_e_g_c()[..2];
    let up = answer_pitches(&ctx, &fx.ending, motif, 2, Contour::Up, &mut Rng(7));
    let down = answer_pitches(&ctx, &fx.ending, motif, 2, Contour::Down, &mut Rng(7));
    assert!(up[0].midi > 64, "up answer starts at {}", up[0].midi);
    assert!(down[0].midi < 64, "down answer starts at {}", down[0].midi);
    for a in [&up, &down] {
        assert!(a.windows(2).all(|w| w[1].midi >= w[0].midi), "{a:?} lost the rising contour");
    }
}

#[test]
fn snap_prefers_the_nearest_then_the_more_consonant_note() {
    let (opening, ending) = chord_pairs().remove(0);
    let fx = Fixture::new(opening, ending);
    let ctx = fx.ctx(CallDensity::Conversational, true, 0);
    // D5 (draw 4) is in neither the C7 chord nor the blues scale; the
    // nearest chord tones are C5 (blow 4) and E5 (blow 5), a whole tone
    // either side — the tie goes to the lower.
    let n = snap(&ctx, &fx.opening, 74, None, Fit::Chord).unwrap();
    assert_eq!(n.midi, 72);
    // Asking for at least in-scale keeps D5 out too: it's out of the scale.
    let n = snap(&ctx, &fx.opening, 74, None, Fit::Scale).unwrap();
    assert_ne!(n.midi, 74);
}
