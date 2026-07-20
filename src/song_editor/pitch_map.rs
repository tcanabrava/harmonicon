// SPDX-License-Identifier: MIT

//! Resolving MIDI pitches onto a harmonica: which hole/direction/technique
//! (bend, slide) produces a given note, and which harp key fits a set of
//! notes best. Shared by MIDI import (`midi_import`, which wants every note
//! to land *somewhere* — [`map_pitch`]) and live recording (`record`, which
//! wants to *discard* detections the harp can't have made —
//! [`map_pitch_playable`]).

use super::playback::build_harp;
use super::state::{Dir, HARP_KEYS, HarmonicaKind, Pitch, max_bend, pitch_compatible};
use crate::song::chart::Action;
use crate::song::harmonica::Harmonica;

/// Resolves `target` (a MIDI note number) onto `harp` only if the harp can
/// genuinely produce it: an exact blow/draw match if one exists; otherwise,
/// for a diatonic harp, a bend reachable within [`max_bend`]'s per-hole cap
/// (draw bend on holes 1..=6, blow bend on 7..=hole_count — mirroring real
/// harmonica bend physics), or, for a chromatic harp, a slide (which raises
/// a hole's natural note by a semitone, so a target one semitone above some
/// hole's natural note is reachable that way). `None` for a pitch outside
/// all of those — what lets live recording *discard* a detection the harp
/// can't have made instead of disguising it as the nearest playable note
/// (see [`map_pitch`] for the always-resolves variant MIDI import wants).
pub(super) fn map_pitch_playable(
    target: u8,
    harp: &Harmonica,
    kind: HarmonicaKind,
) -> Option<(u8, Dir, Pitch)> {
    let hole_count = harp.hole_count();

    for hole in 1..=hole_count {
        if harp.wind_direction_midi(hole, &Action::Blow) == Some(target) {
            return Some((hole, Dir::Blow, Pitch::Normal));
        }
        if harp.wind_direction_midi(hole, &Action::Draw) == Some(target) {
            return Some((hole, Dir::Draw, Pitch::Normal));
        }
    }

    match kind {
        HarmonicaKind::Diatonic => {
            for hole in 1..=hole_count.min(6) {
                if let Some(reed) = harp.wind_direction_midi(hole, &Action::Draw)
                    && reed > target
                {
                    let depth = (reed - target) as f32;
                    if depth <= max_bend(hole) + f32::EPSILON && pitch_compatible(Pitch::Bend(depth), hole)
                    {
                        return Some((hole, Dir::Draw, Pitch::Bend(depth)));
                    }
                }
            }
            for hole in 7..=hole_count {
                if let Some(reed) = harp.wind_direction_midi(hole, &Action::Blow)
                    && reed > target
                {
                    let depth = (reed - target) as f32;
                    if depth <= max_bend(hole) + f32::EPSILON && pitch_compatible(Pitch::Bend(depth), hole)
                    {
                        return Some((hole, Dir::Blow, Pitch::Bend(depth)));
                    }
                }
            }
        }
        HarmonicaKind::Chromatic => {
            if let Some(natural) = target.checked_sub(1) {
                for hole in 1..=hole_count {
                    if harp.wind_direction_midi(hole, &Action::Blow) == Some(natural) {
                        return Some((hole, Dir::Blow, Pitch::Slide));
                    }
                    if harp.wind_direction_midi(hole, &Action::Draw) == Some(natural) {
                        return Some((hole, Dir::Draw, Pitch::Slide));
                    }
                }
            }
        }
    }

    None
}

/// [`map_pitch_playable`], plus a nearest-playable-natural-note fallback so
/// this always resolves to *something* rather than silently dropping the
/// MIDI note — what import wants (an authored MIDI note must land
/// somewhere), unlike live recording (a detection the harp can't have made
/// is noise, not a note to relocate).
pub(super) fn map_pitch(target: u8, harp: &Harmonica, kind: HarmonicaKind) -> (u8, Dir, Pitch) {
    if let Some(mapped) = map_pitch_playable(target, harp, kind) {
        return mapped;
    }
    let hole_count = harp.hole_count();

    // Fallback: snap to the nearest playable natural note.
    let mut best: Option<(u8, Dir, u8)> = None;
    for hole in 1..=hole_count {
        for (dir, action) in [(Dir::Blow, Action::Blow), (Dir::Draw, Action::Draw)] {
            if let Some(m) = harp.wind_direction_midi(hole, &action) {
                let dist = m.abs_diff(target);
                if best.is_none_or(|(_, _, best_dist)| dist < best_dist) {
                    best = Some((hole, dir, dist));
                }
            }
        }
    }
    best.map(|(hole, dir, _)| (hole, dir, Pitch::Normal))
        // Only unreachable if `harp` has no playable holes at all, which
        // never happens for a real Diatonic/Chromatic harp — 1 is as safe
        // a hole/action default as any.
        .unwrap_or((1, Dir::Blow, Pitch::Normal))
}

/// Fraction of `midi_keys` that land on `key`/`kind`'s harp via an *exact*
/// natural blow/draw match (no bend/slide/nearest-note fallback needed) —
/// the fitness measure [`suggest_key`] maximizes. `midi_keys` empty scores
/// `0.0` rather than dividing by zero (never actually reached in practice:
/// `import_track_notes` already rejects an empty track before this would
/// run on one).
fn key_fit_score(midi_keys: &[u8], key: &str, kind: HarmonicaKind) -> f32 {
    if midi_keys.is_empty() {
        return 0.0;
    }
    let harp = build_harp(key, kind);
    let exact = midi_keys
        .iter()
        .filter(|&&target| {
            (1..=harp.hole_count()).any(|hole| {
                harp.wind_direction_midi(hole, &Action::Blow) == Some(target)
                    || harp.wind_direction_midi(hole, &Action::Draw) == Some(target)
            })
        })
        .count();
    exact as f32 / midi_keys.len() as f32
}

/// The [`HARP_KEYS`] entry that best fits `midi_keys` for `kind` — the one
/// needing the fewest bends/slides/nearest-note fallbacks to play (highest
/// [`key_fit_score`]). Ties keep whichever key sorts earlier in
/// `HARP_KEYS`, so the result is deterministic regardless of float
/// rounding. Lets MIDI import pick a sensible key on its own — the same
/// "always resolves to something reasonable" spirit as [`map_pitch`]'s own
/// bend/slide/nearest-note fallback chain, rather than requiring the user
/// to already have the right key selected (or to discover a bad fit only
/// after seeing how many notes needed a fallback).
pub(super) fn suggest_key(midi_keys: &[u8], kind: HarmonicaKind) -> &'static str {
    let mut best_key = HARP_KEYS[0];
    let mut best_score = -1.0;
    for &key in &HARP_KEYS {
        let score = key_fit_score(midi_keys, key, kind);
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
    use crate::song::harmonica::{chromatic_harp, richter_harp};

    // ── map_pitch ─────────────────────────────────────────────────────────────────

    #[test]
    fn map_pitch_maps_a_directly_playable_blow_note() {
        let harp = richter_harp("C");
        // C4 is hole 1 blow on a C richter harp.
        let midi = crate::audio_system::midi::note_to_midi("C4").unwrap() as u8;
        let (hole, dir, pitch) = map_pitch(midi, &harp, HarmonicaKind::Diatonic);
        assert_eq!(hole, 1);
        assert_eq!(dir, Dir::Blow);
        assert_eq!(pitch, Pitch::Normal);
    }

    #[test]
    fn map_pitch_reaches_a_low_hole_draw_bend_within_the_editors_own_cap() {
        let harp = richter_harp("C");
        // Hole 1 draw is D4; bending down one semitone reaches C#4, which
        // isn't directly playable and is within hole 1's max_bend (1.0).
        let target = crate::audio_system::midi::note_to_midi("D4").unwrap() as u8 - 1;
        let (hole, dir, pitch) = map_pitch(target, &harp, HarmonicaKind::Diatonic);
        assert_eq!(hole, 1);
        assert_eq!(dir, Dir::Draw);
        assert_eq!(pitch, Pitch::Bend(1.0));
    }

    #[test]
    fn map_pitch_uses_slide_on_a_chromatic_harp() {
        let harp = chromatic_harp("C");
        // Hole 1 blow is C4; a target one semitone above (C#4) is only
        // reachable via the slide button on a chromatic harp.
        let target = crate::audio_system::midi::note_to_midi("C4").unwrap() as u8 + 1;
        let (hole, dir, pitch) = map_pitch(target, &harp, HarmonicaKind::Chromatic);
        assert_eq!(hole, 1);
        assert_eq!(dir, Dir::Blow);
        assert_eq!(pitch, Pitch::Slide);
    }

    #[test]
    fn map_pitch_falls_back_to_the_nearest_playable_note() {
        let harp = richter_harp("C");
        let (hole, _dir, pitch) = map_pitch(0, &harp, HarmonicaKind::Diatonic);
        assert_eq!(pitch, Pitch::Normal);
        assert!((1..=10).contains(&hole));
    }

    #[test]
    fn map_pitch_playable_rejects_a_pitch_the_harp_cant_produce() {
        // Same unreachable target the fallback test above snaps to the
        // nearest note — the playable-only variant must reject it instead.
        let harp = richter_harp("C");
        assert_eq!(map_pitch_playable(0, &harp, HarmonicaKind::Diatonic), None);
    }

    #[test]
    fn map_pitch_playable_still_resolves_exact_and_bent_notes() {
        let harp = richter_harp("C");
        let c4 = crate::audio_system::midi::note_to_midi("C4").unwrap() as u8;
        assert_eq!(
            map_pitch_playable(c4, &harp, HarmonicaKind::Diatonic),
            Some((1, Dir::Blow, Pitch::Normal))
        );
        let bent = crate::audio_system::midi::note_to_midi("D4").unwrap() as u8 - 1;
        assert_eq!(
            map_pitch_playable(bent, &harp, HarmonicaKind::Diatonic),
            Some((1, Dir::Draw, Pitch::Bend(1.0)))
        );
    }

    // ── key_fit_score / suggest_key ──────────────────────────────────────────────

    fn midi(note: &str) -> u8 {
        crate::audio_system::midi::note_to_midi(note).unwrap() as u8
    }

    #[test]
    fn key_fit_score_is_perfect_for_notes_that_are_all_natural_on_that_key() {
        // C4/E4/G4 are hole 1/2/3 blow on a C richter harp — no bend needed.
        let keys = [midi("C4"), midi("E4"), midi("G4")];
        assert_eq!(key_fit_score(&keys, "C", HarmonicaKind::Diatonic), 1.0);
    }

    #[test]
    fn key_fit_score_is_lower_when_notes_need_a_fallback() {
        // The same notes, scored against a harp a tritone away, won't line
        // up on natural blow/draw reeds nearly as often.
        let keys = [midi("C4"), midi("E4"), midi("G4")];
        let c_score = key_fit_score(&keys, "C", HarmonicaKind::Diatonic);
        let off_score = key_fit_score(&keys, "F#", HarmonicaKind::Diatonic);
        assert!(off_score < c_score);
    }

    #[test]
    fn key_fit_score_is_zero_for_no_notes() {
        assert_eq!(key_fit_score(&[], "C", HarmonicaKind::Diatonic), 0.0);
    }

    #[test]
    fn suggest_key_picks_the_key_whose_harp_the_notes_are_natural_on() {
        // A transposed-up-a-tone version of the same C-harp-natural notes
        // should suggest D, not C.
        let keys = [midi("D4"), midi("F#4"), midi("A4")];
        assert_eq!(suggest_key(&keys, HarmonicaKind::Diatonic), "D");
    }

    #[test]
    fn suggest_key_breaks_ties_by_harp_keys_own_order() {
        // No notes at all fits every key equally (badly) — the first
        // HARP_KEYS entry wins, deterministically.
        assert_eq!(suggest_key(&[], HarmonicaKind::Diatonic), HARP_KEYS[0]);
    }
}
