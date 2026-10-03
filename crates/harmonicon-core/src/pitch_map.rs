// SPDX-License-Identifier: MIT

//! Resolving a MIDI pitch onto a harmonica: which hole, breath direction and
//! technique produces it, and which harp key fits a set of notes best.
//!
//! The inverse of [`Harmonica::wind_direction_midi`], which answers "what
//! does this hole sound like". Everything that has to put arbitrary music
//! onto a harp needs this direction instead: MIDI import, live recording,
//! importing other score formats, and transposing a chart onto whichever
//! harmonica the player actually owns.
//!
//! It lived in `harmonicon-editor`'s `song_editor::pitch_map` until those
//! last two needed it — a crate above gameplay, so neither could reach it.
//! Here it is Bevy-free and reachable from anywhere.
//!
//! Stated in core's own vocabulary ([`Action`], [`Harmonica`]) rather than
//! the editor's parallel `Dir`/`Pitch`/`HarmonicaKind` enums, and the harp
//! family is read from the [`Harmonica`] itself instead of being passed
//! alongside it — a `Chromatic` harp with a `Diatonic` kind argument was
//! always a bug waiting to happen.

use crate::chart::Action;
use crate::harmonica::{Harmonica, chromatic_harp, hole_notes, richter_harp};
use crate::midi::note_to_midi;

/// How a hole is played to reach a pitch.
///
/// Narrower than [`crate::chart::Modifier`] on purpose: that also carries
/// expression (vibrato, wah), which is a performance choice layered on top
/// of a note rather than part of reaching its pitch.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Technique {
    /// The reed's own pitch, blown or drawn.
    Natural,
    /// Bent down by this many semitones from the reed's natural pitch.
    Bend(f32),
    /// Diatonic overblow — holes 1/4/5/6, a semitone above the *draw* reed.
    Overblow,
    /// Diatonic overdraw — holes 7–10, a semitone above the *blow* reed.
    Overdraw,
    /// Chromatic slide button, raising a hole's natural pitch a semitone.
    Slide,
}

/// One way of producing a pitch on a harmonica.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HoleAssignment {
    pub hole: u8,
    pub action: Action,
    pub technique: Technique,
}

/// The twelve keys a harmonica is sold in, in the order [`suggest_key`]
/// considers them — which is also its tie-break, so the result is stable.
pub const HARP_KEYS: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/// Which family of harp to build for a key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HarpKind {
    #[default]
    Diatonic,
    Chromatic,
}

/// A standard harp of `kind` in `key`.
pub fn harp_for_key(key: &str, kind: HarpKind) -> Harmonica {
    match kind {
        HarpKind::Diatonic => richter_harp(key),
        HarpKind::Chromatic => chromatic_harp(key),
    }
}

/// How far `hole` bends on `harp`, in semitones. Zero means it doesn't.
///
/// A bend pulls a reed down toward *the other reed in the same hole*, so
/// the depth available is however many semitones lie strictly between them
/// — which is exactly what [`hole_notes`] enumerates as `bends`. Read off
/// the reeds rather than stated as a table, because a table is only right
/// for the one tuning it was written from: Richter's constants said
/// country tuning's raised draw 5 couldn't bend (it can — a semitone) and
/// that Paddy Richter's hole 3 bent three semitones (it bends one), and
/// disagreed with natural minor on seven of ten holes. Two descriptions
/// of one physical fact drift; this keeps one.
pub fn max_bend(harp: &Harmonica, hole: u8) -> f32 {
    hole_notes(harp, hole).bends.len() as f32
}

/// Whether `hole` can be overblown — holes 1/4/5/6, matching
/// [`hole_notes`]'s own `over` field.
pub fn overblow_ok(hole: u8) -> bool {
    matches!(hole, 1 | 4 | 5 | 6)
}

/// Whether `hole` can be overdrawn — holes 7–10, matching [`hole_notes`].
pub fn overdraw_ok(hole: u8) -> bool {
    (7..=10).contains(&hole)
}

/// Whether `technique` is physically available on `hole` of `harp`.
///
/// Bends come from the reeds ([`max_bend`]); over-techniques stay by hole
/// number ([`overblow_ok`]/[`overdraw_ok`]) because *which* holes a player
/// overblows is a convention of the instrument, not reed physics — every
/// hole with draw above blow could in principle be overblown, but on 2 and
/// 3 the result duplicates a bend on the next hole, so nobody does.
pub fn technique_fits(technique: Technique, harp: &Harmonica, hole: u8) -> bool {
    match technique {
        Technique::Natural => true,
        Technique::Bend(depth) => depth <= max_bend(harp, hole) + f32::EPSILON,
        Technique::Overblow => overblow_ok(hole),
        Technique::Overdraw => overdraw_ok(hole),
        // Every chromatic hole has the slide.
        Technique::Slide => true,
    }
}

/// The breath direction an over-technique physically requires.
///
/// The technique is named for the breath, not for which reed the resulting
/// pitch sits near: an overblow is blown even though it sounds above the
/// *draw* reed, and an overdraw drawn though it sounds above the *blow*
/// reed (see [`hole_notes`]).
fn over_action(hole: u8) -> Option<(Action, Technique)> {
    if overblow_ok(hole) {
        Some((Action::Blow, Technique::Overblow))
    } else if overdraw_ok(hole) {
        Some((Action::Draw, Technique::Overdraw))
    } else {
        None
    }
}

/// Resolves `target` onto `harp` only if the harp can genuinely produce it,
/// `None` otherwise — so live recording can *discard* a detection the harp
/// can't have made rather than disguising it as the nearest playable note
/// (see [`map_pitch`] for the always-resolves variant an importer wants).
///
/// Resolution order is deliberate and pedagogical, not arbitrary. Easiest
/// first: an exact reed, then a bend, and only then an overblow/overdraw.
/// Overblows are an advanced technique — resolving one where a plain bend
/// would do would quietly turn a beginner melody into an overblow study.
pub fn map_pitch_playable(target: u8, harp: &Harmonica) -> Option<HoleAssignment> {
    playable_assignments(target, harp).into_iter().next()
}

/// Every way `harp` can sound `target`, easiest first — the order
/// [`map_pitch_playable`] picks its answer from. A pitch often has more
/// than one home (G4 is draw 2 *and* blow 3 on a C harp; a bent note may
/// also be an overblow), and a caller placing several notes at once needs
/// the alternatives when the first choice is already taken.
pub fn playable_assignments(target: u8, harp: &Harmonica) -> Vec<HoleAssignment> {
    let hole_count = harp.hole_count();
    let mut out = Vec::new();
    let mut assign = |hole, action, technique| {
        out.push(HoleAssignment { hole, action, technique });
    };

    for hole in 1..=hole_count {
        if harp.wind_direction_midi(hole, &Action::Blow) == Some(target) {
            assign(hole, Action::Blow, Technique::Natural);
        }
        if harp.wind_direction_midi(hole, &Action::Draw) == Some(target) {
            assign(hole, Action::Draw, Technique::Natural);
        }
    }

    match harp {
        Harmonica::Diatonic { .. } => {
            // Draw bends live on the low holes, blow bends on the high ones.
            for (range, action) in
                [(1..=hole_count.min(6), Action::Draw), (7..=hole_count, Action::Blow)]
            {
                for hole in range {
                    if let Some(reed) = harp.wind_direction_midi(hole, &action)
                        && reed > target
                    {
                        let depth = (reed - target) as f32;
                        if technique_fits(Technique::Bend(depth), harp, hole) {
                            assign(hole, action, Technique::Bend(depth));
                        }
                    }
                }
            }
            // Overblow/overdraw last: these are the notes a diatonic harp
            // otherwise simply doesn't have.
            for hole in 1..=hole_count {
                if let Some((action, technique)) = over_action(hole)
                    && hole_notes(harp, hole).over.as_deref().and_then(note_to_midi)
                        == Some(target as i32)
                {
                    assign(hole, action, technique);
                }
            }
        }
        Harmonica::Chromatic { .. } => {
            if let Some(natural) = target.checked_sub(1) {
                for hole in 1..=hole_count {
                    for action in [Action::Blow, Action::Draw] {
                        if harp.wind_direction_midi(hole, &action) == Some(natural) {
                            assign(hole, action, Technique::Slide);
                        }
                    }
                }
            }
        }
    }

    out
}

/// [`map_pitch_playable`] with a nearest-natural-note fallback, so this
/// always resolves to *something*.
///
/// What an importer wants — an authored note has to land somewhere — and
/// the opposite of what live recording wants, where a pitch the harp can't
/// make is noise rather than a note to relocate.
pub fn map_pitch(target: u8, harp: &Harmonica) -> HoleAssignment {
    if let Some(mapped) = map_pitch_playable(target, harp) {
        return mapped;
    }

    let mut best: Option<(HoleAssignment, u8)> = None;
    for hole in 1..=harp.hole_count() {
        for action in [Action::Blow, Action::Draw] {
            if let Some(m) = harp.wind_direction_midi(hole, &action) {
                let dist = m.abs_diff(target);
                if best.is_none_or(|(_, best_dist)| dist < best_dist) {
                    best = Some((
                        HoleAssignment { hole, action, technique: Technique::Natural },
                        dist,
                    ));
                }
            }
        }
    }
    best.map(|(assignment, _)| assignment)
        // Only reachable if the harp has no playable holes at all, which no
        // real Diatonic/Chromatic value is — hole 1 blow is as safe a
        // default as any.
        .unwrap_or(HoleAssignment { hole: 1, action: Action::Blow, technique: Technique::Natural })
}

/// Fraction of `midi_keys` reachable on `key`'s harp by an *exact* natural
/// blow/draw — the fitness [`suggest_key`] maximises. Empty input scores
/// `0.0` rather than dividing by zero.
pub fn key_fit_score(midi_keys: &[u8], key: &str, kind: HarpKind) -> f32 {
    let harp = harp_for_key(key, kind);
    key_fit_score_for_harp(midi_keys, &harp)
}

/// Fraction of `midi_keys` matching natural reeds on an already-built harp.
/// This variant lets alternate tunings and non-default hole counts use the
/// same scoring rule without pretending they are a standard instrument.
pub fn key_fit_score_for_harp(midi_keys: &[u8], harp: &Harmonica) -> f32 {
    if midi_keys.is_empty() {
        return 0.0;
    }
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

/// The [`HARP_KEYS`] entry needing the fewest bends, overblows and fallbacks
/// for `midi_keys`. Ties keep whichever key comes first in `HARP_KEYS`, so
/// the answer is deterministic.
pub fn suggest_key(midi_keys: &[u8], kind: HarpKind) -> &'static str {
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
mod tests;
