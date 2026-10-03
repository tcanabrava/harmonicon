// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use super::super::improv::{NoteFit, classify_note_fit};
use super::*;
use harmonicon_core::harmonica::{Harmonica, harp_banner};

// ── should_restart_jam_music ─────────────────────────────────────────────

#[test]
fn restarts_once_finished_when_loop_is_on() {
    assert!(should_restart_jam_music(true, false, false, true, false));
}

#[test]
fn does_not_restart_while_still_playing() {
    assert!(!should_restart_jam_music(true, false, false, true, true));
}

#[test]
fn does_not_restart_when_loop_is_off() {
    assert!(!should_restart_jam_music(false, false, false, true, false));
}

#[test]
fn does_not_restart_before_the_jam_has_started() {
    assert!(!should_restart_jam_music(true, false, false, false, false));
}

#[test]
fn generated_jam_restarts_without_the_finite_song_loop_toggle() {
    assert!(should_restart_jam_music(false, true, false, true, false));
}

#[test]
fn a_scheduled_or_completed_ending_prevents_restart() {
    assert!(!should_restart_jam_music(false, true, true, true, false));
}

#[test]
fn ending_is_scheduled_for_the_next_chorus_boundary() {
    assert_eq!(next_chorus_boundary(0), 12);
    assert_eq!(next_chorus_boundary(11), 12);
    assert_eq!(next_chorus_boundary(12), 24);
    assert_eq!(next_chorus_boundary(95), 96);
}

/// Standard Richter C diatonic, matching `harmonica.rs`'s test layout.
fn c_harp() -> Harmonica {
    harmonicon_core::harmonica::richter_harp("C")
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
    assert_eq!(classify_note_fit("C", &chord_tones, &scale), NoteFit::ChordTone);
    assert_eq!(classify_note_fit("E", &chord_tones, &scale), NoteFit::InScale);
    assert_eq!(classify_note_fit("F", &chord_tones, &scale), NoteFit::OutOfScale);
}

#[test]
fn banner_derives_harp_key_from_hole_1_blow() {
    // c_harp() has no position field → the "no position" wording.
    assert_eq!(harp_banner(&c_harp(), "G"), "Use a C harmonica  \u{00B7}  key of G");
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
