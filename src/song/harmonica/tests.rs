// SPDX-License-Identifier: MIT

use super::*;
use crate::song::chart::HarpChart;

fn test_chart() -> HarpChart {
    serde_json::from_str(r#"{
        "song": { "title": "T", "artist": "A", "tempo_bpm": 120.0, "key": "C", "difficulty": "easy" },
        "timing": { "resolution": 480, "tempo_map": [{"tick": 0, "bpm": 120.0}] },
        "harmonica": {
            "type": "diatonic",
            "holes": 10,
            "bending_profile": "richter_standard",
            "layout": {
                "blow": ["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                "draw": ["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]
            }
        },
        "track": [],
        "scoring": { "perfect_window_ms": 50, "good_window_ms": 100, "miss_window_ms": 130 }
    }"#).unwrap()
}

#[test]
fn semitone_identity() {
    assert_eq!(semitone("C", 0), "C");
    assert_eq!(semitone("G", 0), "G");
}

#[test]
fn semitone_intervals() {
    assert_eq!(semitone("C", 7), "G"); // perfect fifth
    assert_eq!(semitone("C", 5), "F"); // perfect fourth
    assert_eq!(semitone("C", 4), "E"); // major third
    assert_eq!(semitone("G", 5), "C"); // fourth up from G
    assert_eq!(semitone("A", 3), "C"); // minor third up
}

#[test]
fn semitone_wraps_at_octave() {
    assert_eq!(semitone("B", 1), "C");
    assert_eq!(semitone("C", 12), "C");
    assert_eq!(semitone("A", 14), "B");
}

#[test]
fn blues_scale_is_the_six_classes() {
    // C blues: C, Eb(=D#), F, Gb(=F#), G, Bb(=A#).
    let s = blues_scale_classes("C");
    for c in ["C", "D#", "F", "F#", "G", "A#"] {
        assert!(s.contains(c), "missing {c}");
    }
    assert_eq!(s.len(), 6);
    assert!(!s.contains("D"), "major 2nd is not in the blues scale");
    assert!(!s.contains("E"), "major 3rd is not in the blues scale");
}

#[test]
fn twelve_bar_c_major() {
    let bar = twelve_bar("C");
    // Pattern: I I I I  IV IV I I  V IV I V
    let expected = ["C", "C", "C", "C", "F", "F", "C", "C", "G", "F", "C", "G"];
    assert_eq!(bar, expected.map(str::to_string));
}

#[test]
fn twelve_bar_g_major() {
    let bar = twelve_bar("G");
    // IV of G = C,  V of G = D
    let expected = ["G", "G", "G", "G", "C", "C", "G", "G", "D", "C", "G", "D"];
    assert_eq!(bar, expected.map(str::to_string));
}

// ── progression_bars ─────────────────────────────────────────────────────

fn roots(bars: &[(String, ChordQuality); 12]) -> Vec<&str> {
    bars.iter().map(|(root, _)| root.as_str()).collect()
}

#[test]
fn standard_progression_matches_twelve_bar() {
    let bars = progression_bars("C", Progression::Standard);
    assert_eq!(
        roots(&bars),
        vec!["C", "C", "C", "C", "F", "F", "C", "C", "G", "F", "C", "G"]
    );
    assert!(bars.iter().all(|(_, q)| *q == ChordQuality::Dominant7));
}

#[test]
fn quick_change_moves_bar_two_to_the_iv() {
    let bars = progression_bars("C", Progression::QuickChange);
    assert_eq!(
        roots(&bars),
        vec!["C", "F", "C", "C", "F", "F", "C", "C", "G", "F", "C", "G"]
    );
    assert!(bars.iter().all(|(_, q)| *q == ChordQuality::Dominant7));
}

#[test]
fn minor_blues_keeps_the_standard_roots_but_i_and_iv_go_minor() {
    let bars = progression_bars("C", Progression::Minor);
    // Same root sequence as Standard...
    assert_eq!(
        roots(&bars),
        vec!["C", "C", "C", "C", "F", "F", "C", "C", "G", "F", "C", "G"]
    );
    // ...but i/iv bars are minor 7th and the V bars stay dominant.
    for (bar, (_, q)) in bars.iter().enumerate() {
        let expected = if bar == 8 || bar == 11 {
            ChordQuality::Dominant7
        } else {
            ChordQuality::Minor7
        };
        assert_eq!(*q, expected, "bar {bar}");
    }
}

#[test]
fn progression_cycles_forward_and_wraps() {
    assert_eq!(Progression::Standard.next(), Progression::QuickChange);
    assert_eq!(Progression::QuickChange.next(), Progression::Minor);
    assert_eq!(Progression::Minor.next(), Progression::Standard);
}

#[test]
fn progression_cycles_backward_and_wraps() {
    assert_eq!(Progression::Standard.prev(), Progression::Minor);
    assert_eq!(Progression::Minor.prev(), Progression::QuickChange);
    assert_eq!(Progression::QuickChange.prev(), Progression::Standard);
}

#[test]
fn position_cycles_forward_and_wraps() {
    assert_eq!(Position::First.next(), Position::Second);
    assert_eq!(Position::Second.next(), Position::Third);
    assert_eq!(Position::Third.next(), Position::First);
}

#[test]
fn position_cycles_backward_and_wraps() {
    assert_eq!(Position::First.prev(), Position::Third);
    assert_eq!(Position::Third.prev(), Position::Second);
    assert_eq!(Position::Second.prev(), Position::First);
}

#[test]
fn first_position_harp_key_matches_the_jam_key() {
    assert_eq!(Position::First.harp_key("G"), "G");
}

#[test]
fn second_position_harp_is_a_fourth_below_the_jam_key() {
    // Classic cross harp: a C harp jams in G.
    assert_eq!(Position::Second.harp_key("G"), "C");
}

#[test]
fn third_position_harp_is_a_whole_step_below_the_jam_key() {
    // A C harp, 3rd position, jams in D.
    assert_eq!(Position::Third.harp_key("D"), "C");
}

#[test]
fn chord_intervals_are_dominant_or_minor_seventh() {
    assert_eq!(chord_intervals(ChordQuality::Dominant7), [0, 4, 7, 10]);
    assert_eq!(chord_intervals(ChordQuality::Minor7), [0, 3, 7, 10]);
}

#[test]
fn blow_label_returns_correct_note() {
    let chart = test_chart();
    assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Blow), "C4");
    assert_eq!(chart.harmonica.wind_direction_label(4, &Action::Blow), "C5");
    assert_eq!(
        chart.harmonica.wind_direction_label(10, &Action::Blow),
        "C7"
    );
}

#[test]
fn blow_label_out_of_range_returns_dash() {
    let chart = test_chart();
    assert_eq!(
        chart.harmonica.wind_direction_label(0, &Action::Blow),
        "\u{2014}"
    ); // hole=0 guard
    assert_eq!(
        chart.harmonica.wind_direction_label(11, &Action::Blow),
        "\u{2014}"
    ); // beyond layout
}

#[test]
fn draw_label_returns_correct_note() {
    let chart = test_chart();
    assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Draw), "D4");
    assert_eq!(chart.harmonica.wind_direction_label(3, &Action::Draw), "B4");
    assert_eq!(
        chart.harmonica.wind_direction_label(0, &Action::Draw),
        "\u{2014}"
    );
}

#[test]
fn hole_count_reads_holes_from_either_variant() {
    assert_eq!(test_chart().harmonica.hole_count(), 10);
    assert_eq!(test_chromatic_chart().harmonica.hole_count(), 12);
}

fn test_chromatic_chart() -> HarpChart {
    serde_json::from_str(r#"{
        "song": { "title": "T", "artist": "A", "tempo_bpm": 120.0, "key": "C", "difficulty": "easy" },
        "timing": { "resolution": 480, "tempo_map": [{"tick": 0, "bpm": 120.0}] },
        "harmonica": {
            "type": "chromatic",
            "holes": 12,
            "layout": {
                "blow":       ["C4","D4","E4","F4","G4","A4","B4","C5","D5","E5","F5","G5"],
                "draw":       ["D4","E4","F#4","G4","A4","B4","C#5","D5","E5","F#5","G5","A5"],
                "blow_slide": ["C#4","D#4","F4","F#4","G#4","A#4","B4","C#5","D#5","F5","F#5","G#5"],
                "draw_slide": ["D#4","F4","G4","G#4","A#4","C5","D5","D#5","F5","G5","G#5","A#5"]
            }
        },
        "track": [],
        "scoring": { "perfect_window_ms": 50, "good_window_ms": 100, "miss_window_ms": 130 }
    }"#).unwrap()
}

#[test]
fn wind_direction_label_works_for_chromatic_too() {
    let chart = test_chromatic_chart();
    assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Blow), "C4");
    assert_eq!(chart.harmonica.wind_direction_label(1, &Action::Draw), "D4");
    assert_eq!(
        chart.harmonica.wind_direction_label(12, &Action::Blow),
        "G5"
    );
}

#[test]
fn slide_label_reads_the_slide_tables_for_chromatic_only() {
    let chromatic = test_chromatic_chart();
    assert_eq!(chromatic.harmonica.slide_label(1, &Action::Blow), "C#4");
    assert_eq!(chromatic.harmonica.slide_label(1, &Action::Draw), "D#4");

    let diatonic = test_chart();
    assert_eq!(
        diatonic.harmonica.slide_label(1, &Action::Blow),
        "\u{2014}",
        "diatonic harmonicas have no slide button"
    );
}

#[test]
fn build_valid_notes_contains_blow_and_draw() {
    let chart = test_chart();
    let notes = chart.harmonica.build_valid_notes();
    // C4, E4, G4, C5, E5, G5, C6, E6, G6, C7
    for n in &[60u8, 64, 67, 72, 76, 79, 84, 88, 91, 96] {
        assert!(notes.contains(n), "missing blow note {n}");
    }
    // D4, G4, B4, D5, F5, A5, B5, D6, F6, A6
    for n in &[62u8, 67, 71, 74, 77, 81, 83, 86, 89, 93] {
        assert!(notes.contains(n), "missing draw note {n}");
    }
}

#[test]
fn build_valid_notes_includes_bend_pitches() {
    let chart = test_chart();
    let notes = chart.harmonica.build_valid_notes();
    // Hole 1: draw=D4(62) bends down to blow=C4(60) → C#4(61) reachable
    assert!(notes.contains(&61u8), "missing bend note C#4");
    // Hole 2: draw=G4(67) bends down to blow=E4(64) → F4(65), F#4(66) reachable
    assert!(notes.contains(&65u8), "missing bend note F4");
    assert!(notes.contains(&66u8), "missing bend note F#4");
}

#[test]
fn build_valid_notes_excludes_unrelated_notes() {
    let chart = test_chart();
    let notes = chart.harmonica.build_valid_notes();
    assert!(!notes.contains(&3u8)); // D#0
    assert!(!notes.contains(&108u8)); // C8
}

#[test]
fn frequency_range_spans_lowest_to_highest_valid_note() {
    let chart = test_chart();
    let (lo, hi) = chart.harmonica.frequency_range().expect("has a layout");
    // Lowest note is hole-1 blow (C4 ≈ 261.6 Hz), highest is hole-10 blow (C7 ≈ 2093 Hz).
    assert!((lo - 261.63).abs() < 1.0, "expected ~C4, got {lo}");
    assert!((hi - 2093.0).abs() < 1.0, "expected ~C7, got {hi}");
}

#[test]
fn frequency_range_of_a_low_g_harp_dips_below_the_old_fixed_floor() {
    // Hole 1 blow on a key-of-G diatonic is G3 ≈ 196 Hz — below the
    // default 200 Hz detector floor.
    let harp = Harmonica::Diatonic {
        holes: 10,
        bending_profile: BendingProfile::RichterStandard,
        position: None,
        layout: Some(DiatonicLayout {
            blow: Some(vec!["G3".into(), "B3".into()]),
            draw: Some(vec!["A3".into(), "D4".into()]),
        }),
    };
    let (lo, _hi) = harp.frequency_range().expect("has a layout");
    assert!(lo < 200.0, "expected below 200 Hz, got {lo}");
}

#[test]
fn frequency_range_is_none_without_a_layout() {
    let harp = Harmonica::Diatonic {
        holes: 10,
        bending_profile: BendingProfile::RichterStandard,
        position: None,
        layout: None,
    };
    assert_eq!(harp.frequency_range(), None);
}

// ── key_offset ────────────────────────────────────────────────────────────

#[test]
fn key_offsets_pick_the_real_harp_octave() {
    // C harp unchanged; D up two; A and G are the low harps (pitched down).
    assert_eq!(key_offset("C"), 0);
    assert_eq!(key_offset("D"), 2);
    assert_eq!(key_offset("F#"), 6);
    assert_eq!(key_offset("G"), -5);
    assert_eq!(key_offset("A"), -3);
}

#[test]
fn key_offset_accepts_flat_spellings_too() {
    // "Db" and "C#" are the same pitch class — callers use both spellings
    // (the Song Editor's key picker uses flats, the Bending Trainer's
    // uses sharps).
    assert_eq!(key_offset("Db"), key_offset("C#"));
    assert_eq!(key_offset("Ab"), key_offset("G#"));
    assert_eq!(key_offset("Bb"), key_offset("A#"));
}

// ── richter_harp / chromatic_harp ─────────────────────────────────────────

#[test]
fn c_harp_keeps_the_reference_layout() {
    let Harmonica::Diatonic {
        layout: Some(l), ..
    } = richter_harp("C")
    else {
        panic!("expected diatonic");
    };
    assert_eq!(l.blow.unwrap()[0], "C4");
    assert_eq!(l.draw.unwrap()[0], "D4");
}

#[test]
fn d_harp_hole_1_blow_is_d4() {
    let Harmonica::Diatonic {
        layout: Some(l), ..
    } = richter_harp("D")
    else {
        panic!("expected diatonic");
    };
    assert_eq!(l.blow.unwrap()[0], "D4");
}

#[test]
fn g_harp_hole_1_blow_is_g3() {
    // The G harp is a low harp — hole-1 blow sits below C4.
    let Harmonica::Diatonic {
        layout: Some(l), ..
    } = richter_harp("G")
    else {
        panic!("expected diatonic");
    };
    assert_eq!(l.blow.unwrap()[0], "G3");
}

#[test]
fn chromatic_harp_keeps_the_reference_layout_in_c() {
    let Harmonica::Chromatic {
        layout: Some(l), ..
    } = chromatic_harp("C")
    else {
        panic!("expected chromatic");
    };
    assert_eq!(l.blow.unwrap()[0], "C4");
    assert_eq!(l.draw.unwrap()[0], "D4");
    assert_eq!(l.blow_slide.unwrap()[0], "C#4");
    assert_eq!(l.draw_slide.unwrap()[0], "D#4");
}

#[test]
fn chromatic_harp_transposes_every_table() {
    let Harmonica::Chromatic {
        layout: Some(l), ..
    } = chromatic_harp("D")
    else {
        panic!("expected chromatic");
    };
    assert_eq!(l.blow.unwrap()[0], "D4");
    assert_eq!(l.draw.unwrap()[0], "E4");
}

// ── hole_notes ────────────────────────────────────────────────────────────

#[test]
fn transpose_crosses_octave() {
    assert_eq!(transpose("B4", 1).as_deref(), Some("C5"));
    assert_eq!(transpose("C5", -1).as_deref(), Some("B4"));
    assert_eq!(transpose("D4", -1).as_deref(), Some("C#4"));
}

#[test]
fn hole_1_has_one_draw_bend() {
    // C harp hole 1: blow C4, draw D4 → single ½-step bend C#4.
    let h = hole_notes(&richter_harp("C"), 1);
    assert_eq!(h.blow.as_deref(), Some("C4"));
    assert_eq!(h.draw.as_deref(), Some("D4"));
    assert_eq!(h.bends, vec!["C#4"]);
}

#[test]
fn hole_3_has_three_draw_bends() {
    // blow G4, draw B4 → A#4, A4, G#4 (½, whole, 1½).
    let h = hole_notes(&richter_harp("C"), 3);
    assert_eq!(h.bends, vec!["A#4", "A4", "G#4"]);
}

#[test]
fn hole_10_has_two_blow_bends() {
    // blow C7, draw A6 → blow bends B6, A#6.
    let h = hole_notes(&richter_harp("C"), 10);
    assert_eq!(h.bends, vec!["B6", "A#6"]);
    assert_eq!(
        h.over.as_deref(),
        Some("C#7"),
        "overdraw a semitone above blow"
    );
}

#[test]
fn hole_5_has_no_draw_bend() {
    // blow E5, draw F5 — only a semitone apart, nothing between.
    let h = hole_notes(&richter_harp("C"), 5);
    assert!(h.bends.is_empty());
}

#[test]
fn hole_1_overblow_is_a_semitone_above_draw() {
    assert_eq!(
        hole_notes(&richter_harp("C"), 1).over.as_deref(),
        Some("D#4")
    );
}
