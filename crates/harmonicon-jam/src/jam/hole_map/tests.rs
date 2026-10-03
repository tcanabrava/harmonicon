// SPDX-License-Identifier: MIT

use super::*;

/// Standard Richter C diatonic, matching `harmonica.rs`'s test layout.
fn c_harp() -> Harmonica {
    harmonicon_core::harmonica::richter_harp("C")
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
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Standard, Scale::FirstPosition);
    let mut holes = guide.note_to_holes.get(&67u8).cloned().unwrap_or_default(); // G4
    holes.sort_unstable();
    assert_eq!(holes, vec![2, 3]);
}

#[test]
fn guide_marks_scale_membership_per_direction() {
    let (holes, _) = build_hole_guide(&c_harp(), "C", Progression::Standard, Scale::FirstPosition);
    let hole1 = holes.iter().find(|h| h.hole == 1).unwrap();
    assert!(hole1.blow_in_scale, "blow C4 is the root → in scale");
    assert!(!hole1.draw_in_scale, "draw D4 (major 2nd) → outside");
    let hole2 = holes.iter().find(|h| h.hole == 2).unwrap();
    assert!(hole2.draw_in_scale, "draw G4 (the 5th) → in scale");
}

#[test]
fn guide_uses_the_scale_it_is_given_instead_of_always_blues() {
    let (_, blues) = build_hole_guide(&c_harp(), "C", Progression::Standard, Scale::FirstPosition);
    let (_, major) = build_hole_guide(&c_harp(), "C", Progression::Standard, Scale::Major);
    // Blues hexatonic on C: C, D#, F, F#, G, A#. Major scale on C: C, D, E,
    // F, G, A, B. "D" (a major 2nd) is in the major scale but not blues;
    // "D#" (the blues b3) is the other way around.
    assert!(major.scale_classes.contains("D"));
    assert!(!blues.scale_classes.contains("D"));
    assert!(blues.scale_classes.contains("D#"));
    assert!(!major.scale_classes.contains("D#"));
}

#[test]
fn guide_covers_all_ten_holes() {
    let (holes, _) = build_hole_guide(&c_harp(), "C", Progression::Standard, Scale::FirstPosition);
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
    let (holes, _) =
        build_hole_guide(&c_chromatic_harp(), "C", Progression::Standard, Scale::FirstPosition);
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
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Standard, Scale::FirstPosition);
    assert!(guide.chord_tones_by_bar[0].contains("C"), "bar 0 is I (C7)");
    assert!(guide.chord_tones_by_bar[4].contains("F"), "bar 4 is IV (F7)");
    assert!(guide.chord_tones_by_bar[8].contains("G"), "bar 8 is V (G7)");
    assert!(!guide.chord_tones_by_bar[0].contains("F"), "F is not a tone of the I chord");
}

#[test]
fn guide_follows_a_non_standard_progression() {
    // Quick change: bar 1 (0-indexed) moves from I (C7) to IV (F7).
    // C7 = C,E,G,A#; F7 = F,A,C,D# — "E" is the major 3rd of C7 and
    // not a tone of F7 at all, so it distinguishes the two even though
    // both chords happen to share the note C (F7's 5th).
    let (_, guide) =
        build_hole_guide(&c_harp(), "C", Progression::QuickChange, Scale::FirstPosition);
    assert!(guide.chord_tones_by_bar[1].contains("F"), "quick change moves bar 1 to IV (F7)");
    assert!(!guide.chord_tones_by_bar[1].contains("E"));
}

#[test]
fn guide_uses_minor_seventh_chord_tones_for_a_minor_blues() {
    let (_, guide) = build_hole_guide(&c_harp(), "C", Progression::Minor, Scale::FirstPosition);
    // Bar 0 is i (Cm7): the minor 3rd (Eb=D#) is a chord tone, the
    // major 3rd (E) is not.
    assert!(guide.chord_tones_by_bar[0].contains("D#"));
    assert!(!guide.chord_tones_by_bar[0].contains("E"));
    // Bar 8 is still V (G7, dominant) even in a minor blues.
    assert!(guide.chord_tones_by_bar[8].contains("B"));
}

// ── lowest_sounding ──────────────────────────────────────────────────────

fn pitch(midi: u8) -> PitchInfo {
    PitchInfo {
        midi,
        note: harmonicon_core::midi::midi_to_note(i32::from(midi)),
        octave: 0,
        frequency: 0.0,
    }
}

#[test]
fn the_indicator_names_the_lowest_playable_pitch() {
    let playable = playable_notes(&c_harp());
    // E4 (blow 2) and G4 (draw 2 / blow 3) sounding together: the root, E4,
    // wins; G4's two homes don't matter.
    let active = [pitch(67), pitch(64)];
    let (p, note) = lowest_sounding(&active, &playable).unwrap();
    assert_eq!(p.midi, 64);
    assert_eq!((note.hole, note.blow), (2, true));
    // A pitch the harp can't sound is ignored entirely.
    assert!(lowest_sounding(&[pitch(61)], &playable).is_none());
    assert!(lowest_sounding(&[], &playable).is_none());
}
