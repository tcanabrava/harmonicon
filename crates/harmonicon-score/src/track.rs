// SPDX-License-Identifier: MIT

//! Finding the harmonica part in a file that wasn't written for us.
//!
//! A Guitar Pro tab or a MIDI arrangement usually has a dozen tracks, of
//! which at most one is a harmonica. Guessing wrong is worse than asking:
//! a guitar track mapped onto a harp is mostly notes it cannot reach, and
//! playing that is a far worse experience than a track picker.

use crate::ScoreTrack;

/// Track names that mean "harmonica", matched case-insensitively as a
/// substring.
///
/// `gaita` is the Portuguese name and `mouth harp` the common English
/// alternative; both appear in real files.
///
/// **Bare "harp" is deliberately absent.** It would match an actual harp —
/// the orchestral instrument — and a harp part converted to harmonica is
/// exactly the unplayable mess this list exists to avoid. Missing a track
/// named only "Harp" costs the player one pick from a list; matching a real
/// harp costs them a chart they can't play and no explanation.
pub const HARMONICA_TRACK_NAMES: &[&str] = &["harmonica", "gaita", "mouth harp", "blues harp"];

/// The index of the first playable track whose name says harmonica, or
/// `None` if nothing matches and the player should choose.
///
/// Only ever returns a track with notes in it: a MIDI file's tempo track is
/// sometimes named after the song, and picking it would start a song in
/// which nothing ever happens.
pub fn pick_harmonica_track(tracks: &[ScoreTrack]) -> Option<usize> {
    tracks
        .iter()
        .find(|t| t.is_playable() && name_says_harmonica(t.name.as_deref()))
        .map(|t| t.index)
}

/// Whether a track name names a harmonica.
pub fn name_says_harmonica(name: Option<&str>) -> bool {
    let Some(name) = name else {
        return false;
    };
    let lower = name.to_lowercase();
    HARMONICA_TRACK_NAMES.iter().any(|needle| lower.contains(needle))
}

/// The tracks worth offering in a picker: those with notes.
pub fn playable_tracks(tracks: &[ScoreTrack]) -> Vec<&ScoreTrack> {
    tracks.iter().filter(|t| t.is_playable()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(index: usize, name: Option<&str>, note_count: usize) -> ScoreTrack {
        ScoreTrack { index, name: name.map(str::to_string), note_count }
    }

    #[test]
    fn every_spelling_of_harmonica_is_recognised() {
        for name in [
            "Harmonica",
            "harmonica",
            "HARMONICA",
            "Gaita",
            "gaita de boca",
            "Mouth Harp",
            "Blues Harp",
            "Lead Harmonica",
        ] {
            assert!(name_says_harmonica(Some(name)), "{name} not recognised");
        }
    }

    #[test]
    fn an_orchestral_harp_is_not_a_harmonica() {
        // The reason bare "harp" is not in the list. A harp part mapped onto
        // a harmonica is mostly unreachable notes, and picking it silently
        // is worse than showing a chooser.
        for name in ["Harp", "Concert Harp", "Harpsichord"] {
            assert!(!name_says_harmonica(Some(name)), "{name} was mistaken for a harmonica");
        }
    }

    #[test]
    fn an_unnamed_track_is_never_assumed_to_be_the_harmonica() {
        assert!(!name_says_harmonica(None));
    }

    #[test]
    fn the_first_matching_playable_track_wins() {
        let tracks = [
            track(0, Some("Tempo"), 0),
            track(1, Some("Guitar"), 40),
            track(2, Some("Harmonica"), 30),
            track(3, Some("Harmonica solo"), 12),
        ];
        assert_eq!(pick_harmonica_track(&tracks), Some(2));
    }

    #[test]
    fn a_named_but_empty_track_is_not_picked() {
        // A MIDI tempo track named after the song would otherwise start a
        // song in which nothing ever happens.
        let tracks = [track(0, Some("Harmonica"), 0), track(1, Some("Harmonica"), 25)];
        assert_eq!(pick_harmonica_track(&tracks), Some(1));
    }

    #[test]
    fn nothing_is_picked_when_no_track_says_harmonica() {
        // The player chooses. Guessing "the busiest track" would routinely
        // hand back a guitar part.
        let tracks = [track(0, Some("Guitar"), 90), track(1, Some("Bass"), 60)];
        assert_eq!(pick_harmonica_track(&tracks), None);
    }

    #[test]
    fn a_picker_is_offered_only_tracks_with_notes() {
        let tracks =
            [track(0, Some("Conductor"), 0), track(1, Some("Guitar"), 90), track(2, None, 12)];
        let offered: Vec<usize> = playable_tracks(&tracks).iter().map(|t| t.index).collect();
        assert_eq!(offered, vec![1, 2]);
    }
}
