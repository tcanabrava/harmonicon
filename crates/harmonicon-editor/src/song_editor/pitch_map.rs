// SPDX-License-Identifier: MIT

//! The editor's view of [`harmonicon_core::pitch_map`].
//!
//! Core resolves pitches; this module maps assignments to the editor's
//! `Dir` and `Pitch` types. MIDI import permits nearest-note fallback
//! through [`map_pitch`], while recording uses [`map_pitch_playable`].

use super::playback::build_harp;
use super::state::{Dir, HARP_KEYS, HarmonicaKind, Pitch};
use harmonicon_core::chart::Action;
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::pitch_map::{self, HoleAssignment, Technique};

/// Core's resolution in the editor's own terms.
fn from_core(assignment: HoleAssignment) -> (u8, Dir, Pitch) {
    let dir = match assignment.action {
        Action::Blow => Dir::Blow,
        Action::Draw => Dir::Draw,
    };
    let pitch = match assignment.technique {
        Technique::Natural => Pitch::Normal,
        Technique::Bend(depth) => Pitch::Bend(depth),
        Technique::Overblow => Pitch::Overblow,
        Technique::Overdraw => Pitch::Overdraw,
        Technique::Slide => Pitch::Slide,
    };
    (assignment.hole, dir, pitch)
}

/// Resolves `target` onto `harp` only if it can genuinely produce it. Core
/// reads the harp family off the [`Harmonica`] itself, the one source that
/// cannot disagree with the layout being searched.
pub(super) fn map_pitch_playable(target: u8, harp: &Harmonica) -> Option<(u8, Dir, Pitch)> {
    pitch_map::map_pitch_playable(target, harp).map(from_core)
}

/// Every playable resolution of `target`, in core's easiest-first order.
/// Transposition uses the alternatives when two simultaneous pitches would
/// otherwise compete for the same hole.
pub(super) fn playable_assignments(target: u8, harp: &Harmonica) -> Vec<(u8, Dir, Pitch)> {
    pitch_map::playable_assignments(target, harp).into_iter().map(from_core).collect()
}

/// [`map_pitch_playable`] with core's nearest-natural-note fallback.
pub(super) fn map_pitch(target: u8, harp: &Harmonica) -> (u8, Dir, Pitch) {
    from_core(pitch_map::map_pitch(target, harp))
}

/// The harp key needing the fewest bends, overblows and fallbacks.
pub(super) fn suggest_key(midi_keys: &[u8], kind: HarmonicaKind) -> &'static str {
    let mut best_key = HARP_KEYS[0];
    let mut best_score = -1.0;
    for &key in &HARP_KEYS {
        let score = pitch_map::key_fit_score_for_harp(midi_keys, &build_harp(key, kind));
        if score > best_score {
            best_score = score;
            best_key = key;
        }
    }
    best_key
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::harmonica::{chromatic_harp, richter_harp};
    use harmonicon_core::midi::note_to_midi;

    /// The translation, not the resolution — core owns and tests the latter.
    #[test]
    fn core_techniques_arrive_as_the_editors_own_pitch_variants() {
        for (technique, expected) in [
            (Technique::Natural, Pitch::Normal),
            (Technique::Bend(1.5), Pitch::Bend(1.5)),
            (Technique::Overblow, Pitch::Overblow),
            (Technique::Overdraw, Pitch::Overdraw),
            (Technique::Slide, Pitch::Slide),
        ] {
            let (_, _, pitch) =
                from_core(HoleAssignment { hole: 4, action: Action::Blow, technique });
            assert_eq!(pitch, expected);
        }
    }

    #[test]
    fn key_suggestion_scores_the_selected_alternate_layout() {
        let midi = ["C4", "D#4", "G4", "A#4"]
            .map(|note| u8::try_from(note_to_midi(note).unwrap()).unwrap());
        assert_eq!(suggest_key(&midi, HarmonicaKind::NaturalMinor), "C");
        let harp = build_harp("C", HarmonicaKind::NaturalMinor);
        assert_eq!(pitch_map::key_fit_score_for_harp(&midi, &harp), 1.0);

        let country =
            ["D5", "F#5", "A5"].map(|note| u8::try_from(note_to_midi(note).unwrap()).unwrap());
        assert_eq!(suggest_key(&country, HarmonicaKind::CountryTuned), "C");
    }

    #[test]
    fn key_suggestion_scores_the_low_octave_of_a_sixteen_hole_harp() {
        let midi =
            ["C3", "E3", "G3"].map(|note| u8::try_from(note_to_midi(note).unwrap()).unwrap());
        assert_eq!(suggest_key(&midi, HarmonicaKind::Chromatic16), "C");
        let harp = build_harp("C", HarmonicaKind::Chromatic16);
        assert_eq!(pitch_map::key_fit_score_for_harp(&midi, &harp), 1.0);
    }

    #[test]
    fn both_breath_directions_survive_the_translation() {
        for (action, expected) in [(Action::Blow, Dir::Blow), (Action::Draw, Dir::Draw)] {
            let (_, dir, _) =
                from_core(HoleAssignment { hole: 1, action, technique: Technique::Natural });
            assert_eq!(dir, expected);
        }
    }

    /// The two behaviours the editor's callers actually depend on: import
    /// always lands somewhere, recording rejects what the harp can't make.
    #[test]
    fn import_always_resolves_where_recording_refuses() {
        let harp = richter_harp("C");
        assert_eq!(map_pitch_playable(0, &harp), None);
        let (hole, _, pitch) = map_pitch(0, &harp);
        assert_eq!(pitch, Pitch::Normal);
        assert!((1..=10).contains(&hole));
    }

    #[test]
    fn a_chromatic_harp_still_resolves_its_slide() {
        let harp = chromatic_harp("C");
        let c4 = harmonicon_core::midi::note_to_midi("C4").unwrap() as u8;
        assert_eq!(map_pitch_playable(c4 + 1, &harp), Some((1, Dir::Blow, Pitch::Slide)));
    }
}
