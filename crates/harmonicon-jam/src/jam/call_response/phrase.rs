// SPDX-License-Identifier: MIT

//! The call-phrase generator: a short harmonica phrase built from small
//! musical decisions — a rhythm cell per bar (rests, pickups, syncopation,
//! held and repeated notes), a two-to-four-note motif in a limited range,
//! and an answering bar that repeats the motif or sequences it up or down —
//! rather than one chord tone per beat. Every pitch is drawn from what the
//! player's own harp can produce with a plain blow or draw, consecutive
//! notes stay within a playable hole/breath move of each other, a note
//! outside the chord always resolves to a chord tone next, the phrase ends
//! on a chord tone, and the last bar leaves audible space before the
//! player's turn.
//!
//! Pure: form + harmony + harp + density + seed in, notes on the
//! `TICKS_PER_BEAT` grid out. Nothing here knows about audio or the ECS;
//! `super::drive_call_response` renders the result and fires it.

use std::collections::HashSet;

use harmonicon_core::chart::Action;
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::midi::midi_to_note;
use harmonicon_core::synth::TICKS_PER_BEAT;

use crate::jam::hole_map::note_class;

/// Eighth-note slots per 4/4 bar — the grid every rhythm cell is written on.
const SLOTS_PER_BAR: u8 = 8;
const TICKS_PER_BAR: usize = 4 * TICKS_PER_BEAT;
/// A held note: three eighths or longer. Gets breath (vibrato) when rendered.
const HELD_SLOTS: u8 = 3;
/// Widest a motif may reach from its anchor, in semitones — a fifth, so
/// the call stays in one comfortable neighbourhood of the harp.
const MOTIF_RANGE: i32 = 7;
/// How far an "answer up"/"answer down" aims its new anchor, in semitones
/// — a third, before snapping to the nearest chord tone of the answering
/// bar.
const ANSWER_SHIFT: i32 = 4;
/// Ticks shaved off every note so a repeated pitch re-articulates instead
/// of blurring into one long tone.
const NOTE_GAP_TICKS: usize = 1;

/// How dense the generated calls are. Musical density only — never a
/// level, never a score.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CallDensity {
    Sparse,
    #[default]
    Conversational,
    Busy,
}

impl CallDensity {
    /// The next density in cycle order, for a single cycling button.
    pub fn next(self) -> Self {
        match self {
            CallDensity::Sparse => CallDensity::Conversational,
            CallDensity::Conversational => CallDensity::Busy,
            CallDensity::Busy => CallDensity::Sparse,
        }
    }
}

/// One pitch the player's harp can sound with a plain blow or draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlayableNote {
    pub midi: u8,
    pub hole: u8,
    pub blow: bool,
}

/// Every pitch `harp` sounds with a plain blow or draw, hole by hole — the
/// vocabulary a call is allowed to use. Bends, overblows and the slide are
/// left out on purpose: a call must never ask for a note the player can't
/// simply reach.
pub fn playable_notes(harp: &Harmonica) -> Vec<PlayableNote> {
    let mut out = Vec::with_capacity(harp.hole_count() as usize * 2);
    for hole in 1..=harp.hole_count() {
        for (action, blow) in [(Action::Blow, true), (Action::Draw, false)] {
            if let Some(midi) = harp.wind_direction_midi(hole, &action) {
                out.push(PlayableNote { midi, hole, blow });
            }
        }
    }
    out
}

/// One note of a generated call, on the `TICKS_PER_BEAT` grid from the
/// start of the call. `held` marks a note long enough to want breath.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CallNote {
    pub tick: usize,
    pub len: usize,
    pub note: PlayableNote,
    pub held: bool,
}

/// Bars in one call: an opening bar and an answering bar.
pub const CALL_BARS: usize = 2;

/// Everything a call is generated from: the chord sounding in each of its
/// two bars, the jam's scale, the harp's whole blow/draw vocabulary, the
/// backing's feel, the density the player asked for, and the seed.
pub struct CallContext<'a> {
    pub playable: &'a [PlayableNote],
    pub opening_chord: &'a HashSet<String>,
    pub ending_chord: &'a HashSet<String>,
    pub scale: &'a HashSet<String>,
    pub swung: bool,
    pub density: CallDensity,
    pub seed: u64,
}

/// A rhythm cell: `(first eighth slot, length in eighths)` per note, in
/// order, within one bar. Slots that no entry covers are rests.
type Cell = &'static [(u8, u8)];

// Opening cells (the first bar). Each density has rests and at least one
// pickup (a short note leading into a longer one on a beat).
const SPARSE_OPENINGS: &[Cell] =
    &[&[(0, 2), (4, 3)], &[(2, 2), (6, 2)], &[(0, 3)], &[(1, 1), (2, 4)], &[(3, 1), (4, 3)]];
const CONVERSATIONAL_OPENINGS: &[Cell] = &[
    &[(0, 1), (1, 1), (2, 3), (6, 2)],
    &[(0, 3), (3, 1), (4, 1), (6, 2)],
    &[(1, 1), (2, 1), (3, 1), (4, 2)],
    &[(0, 1), (2, 1), (4, 1), (6, 2)],
    &[(2, 1), (3, 1), (4, 2), (7, 1)],
];
const BUSY_OPENINGS: &[Cell] = &[
    &[(0, 1), (1, 1), (2, 1), (3, 1), (4, 2), (6, 1), (7, 1)],
    &[(0, 1), (1, 1), (2, 1), (4, 1), (5, 1), (6, 2)],
    &[(1, 1), (2, 1), (3, 1), (5, 1), (6, 1), (7, 1)],
    &[(0, 1), (1, 1), (3, 1), (4, 1), (5, 1), (6, 2)],
];

/// The last eighth slot an ending cell may reach (exclusive): the phrase
/// is over by beat 3 of its last bar, leaving two beats of air before the
/// player's turn.
const ENDING_LAST_SLOT: u8 = 4;

// Ending cells (the last bar). Every one finishes by `ENDING_LAST_SLOT`.
// Sparse includes an empty cell: the whole answer was in the first bar.
const SPARSE_ENDINGS: &[Cell] = &[&[(0, 4)], &[(0, 2)], &[(1, 3)], &[]];
const CONVERSATIONAL_ENDINGS: &[Cell] =
    &[&[(0, 1), (1, 1), (2, 2)], &[(0, 2), (2, 2)], &[(1, 1), (2, 2)], &[(0, 1), (2, 2)]];
const BUSY_ENDINGS: &[Cell] =
    &[&[(0, 1), (1, 1), (2, 1), (3, 1)], &[(0, 1), (1, 1), (2, 2)], &[(0, 1), (1, 1), (3, 1)]];

fn openings(density: CallDensity) -> &'static [Cell] {
    match density {
        CallDensity::Sparse => SPARSE_OPENINGS,
        CallDensity::Conversational => CONVERSATIONAL_OPENINGS,
        CallDensity::Busy => BUSY_OPENINGS,
    }
}

fn endings(density: CallDensity) -> &'static [Cell] {
    match density {
        CallDensity::Sparse => SPARSE_ENDINGS,
        CallDensity::Conversational => CONVERSATIONAL_ENDINGS,
        CallDensity::Busy => BUSY_ENDINGS,
    }
}

/// SplitMix64 — tiny, portable, and deterministic for a given seed, which
/// is the whole point: the same jam bar, harp and seed must yield the same
/// call on every platform, so tests can pin a phrase down.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`; `n` must be non-zero.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// Picks an index with probability proportional to its weight.
    fn weighted<T>(&mut self, candidates: &[(T, u32)]) -> usize {
        let total: u32 = candidates.iter().map(|(_, weight)| weight).sum();
        let mut roll = self.below(total.max(1) as usize) as u32;
        for (i, (_, weight)) in candidates.iter().enumerate() {
            if roll < *weight {
                return i;
            }
            roll -= *weight;
        }
        candidates.len() - 1
    }
}

/// How a pitch sits against the bar it sounds in. Ordered so `>=` reads as
/// "at least this consonant".
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Fit {
    Out,
    Scale,
    Chord,
}

fn fit(midi: u8, chord_tones: &HashSet<String>, scale: &HashSet<String>) -> Fit {
    let name = midi_to_note(i32::from(midi));
    let class = note_class(&name);
    if chord_tones.contains(class) {
        Fit::Chord
    } else if scale.contains(class) {
        Fit::Scale
    } else {
        Fit::Out
    }
}

/// Whether a harmonica player can move from `a` to `b` cleanly inside a
/// phrase: at most two holes apart, and a breath-direction change only on
/// the same or an adjacent hole.
fn transition_ok(a: PlayableNote, b: PlayableNote) -> bool {
    let d = a.hole.abs_diff(b.hole);
    d <= 2 && (a.blow == b.blow || d <= 1)
}

/// The eighth-slot grid in ticks. A swung bar splits each beat 2:1 (long
/// eighth on the beat), a straight one evenly — the same ratio the
/// generated backing and the shuffle metronome use.
fn slot_tick(slot: u8, swung: bool) -> usize {
    debug_assert!(slot <= SLOTS_PER_BAR);
    let beat = usize::from(slot / 2);
    let off = if slot % 2 == 1 {
        if swung { TICKS_PER_BEAT * 2 / 3 } else { TICKS_PER_BEAT / 2 }
    } else {
        0
    };
    beat * TICKS_PER_BEAT + off
}

/// Ticks of one cell entry, minus the articulation gap.
fn slot_span(slot: u8, len: u8, swung: bool) -> (usize, usize) {
    let start = slot_tick(slot, swung);
    let end = slot_tick(slot + len, swung);
    (start, (end - start).saturating_sub(NOTE_GAP_TICKS).max(1))
}

/// Whether a chord tone lies one playable move away from `from` — what
/// makes a non-chord note usable at all: it has somewhere to resolve.
fn can_resolve(ctx: &CallContext, chord_tones: &HashSet<String>, from: PlayableNote) -> bool {
    ctx.playable
        .iter()
        .any(|&q| fit(q.midi, chord_tones, ctx.scale) == Fit::Chord && transition_ok(from, q))
}

/// Whether `p` may be sung over `chord_tones` at all: a chord tone always,
/// anything else only if a chord tone is within reach to resolve to.
fn usable(ctx: &CallContext, chord_tones: &HashSet<String>, p: PlayableNote) -> bool {
    fit(p.midi, chord_tones, ctx.scale) == Fit::Chord || can_resolve(ctx, chord_tones, p)
}

/// The pitch nearest `target` that the harp can play, at least `min_fit`
/// consonant with `chord_tones`, able to resolve, and — when there is a
/// previous note — reachable from it. Ties break toward the more consonant
/// note, then the one that keeps the breath direction and stays closest to
/// the previous hole (the same pitch often lives in two places, e.g. draw 2
/// and blow 3), then the lower.
fn snap(
    ctx: &CallContext,
    chord_tones: &HashSet<String>,
    target: i32,
    prev: Option<PlayableNote>,
    min_fit: Fit,
) -> Option<PlayableNote> {
    ctx.playable
        .iter()
        .copied()
        .filter(|p| fit(p.midi, chord_tones, ctx.scale) >= min_fit)
        .filter(|&p| usable(ctx, chord_tones, p))
        .filter(|&p| prev.is_none_or(|prev| transition_ok(prev, p)))
        .min_by_key(|p| {
            let f = fit(p.midi, chord_tones, ctx.scale);
            let (turns, reach) =
                prev.map_or((false, 0), |prev| (prev.blow != p.blow, prev.hole.abs_diff(p.hole)));
            ((i32::from(p.midi) - target).abs(), std::cmp::Reverse(f), turns, reach, p.midi)
        })
}

/// The call's first note: a chord tone from the middle of the harp, where
/// most of the melody holes are. Falls back to any chord tone at all, and
/// to `None` when the chord has no representable tone on this harp.
fn anchor(ctx: &CallContext, chord_tones: &HashSet<String>, rng: &mut Rng) -> Option<PlayableNote> {
    let max_hole = ctx.playable.iter().map(|p| p.hole).max().unwrap_or(0);
    let (lo, hi) = (max_hole / 4, max_hole - max_hole / 4);
    let chord: Vec<PlayableNote> = ctx
        .playable
        .iter()
        .copied()
        .filter(|p| fit(p.midi, chord_tones, ctx.scale) == Fit::Chord)
        .collect();
    let middle: Vec<PlayableNote> =
        chord.iter().copied().filter(|p| (lo..=hi).contains(&p.hole)).collect();
    let pool = if middle.is_empty() { &chord } else { &middle };
    (!pool.is_empty()).then(|| *rng.pick(pool))
}

/// One step of the motif walk from `prev`: a different pitch within
/// `MOTIF_RANGE` of the anchor, reachable in one move, weighted toward chord
/// tones. A note outside the chord may only follow a chord tone (so it
/// resolves at once), and an out-of-scale "blue" neighbour only when the
/// phrase was dealt one. `None` when nothing qualifies — the caller repeats.
fn step(
    ctx: &CallContext,
    chord_tones: &HashSet<String>,
    anchor_midi: i32,
    prev: PlayableNote,
    allow_blue: bool,
    rng: &mut Rng,
) -> Option<PlayableNote> {
    let prev_fit = fit(prev.midi, chord_tones, ctx.scale);
    let candidates: Vec<(PlayableNote, u32)> = ctx
        .playable
        .iter()
        .copied()
        .filter(|&p| p.midi != prev.midi && transition_ok(prev, p))
        .filter(|p| (i32::from(p.midi) - anchor_midi).abs() <= MOTIF_RANGE)
        .filter(|&p| usable(ctx, chord_tones, p))
        .filter_map(|p| {
            let weight = match fit(p.midi, chord_tones, ctx.scale) {
                Fit::Chord => 4,
                Fit::Scale if prev_fit == Fit::Chord => 2,
                Fit::Out if prev_fit == Fit::Chord && allow_blue => 1,
                _ => return None,
            };
            Some((p, weight))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    Some(candidates[rng.weighted(&candidates)].0)
}

/// The opening bar's pitches: a motif of two to four notes walked out from
/// the anchor, then — if the cell has more notes — repeats and further steps
/// mixed by the seed, so a longer cell reads as "motif, said again".
fn opening_pitches(
    ctx: &CallContext,
    chord_tones: &HashSet<String>,
    count: usize,
    rng: &mut Rng,
) -> Vec<PlayableNote> {
    let Some(first) = anchor(ctx, chord_tones, rng) else {
        return Vec::new();
    };
    let anchor_midi = i32::from(first.midi);
    let allow_blue = rng.below(4) == 0;
    let motif_len = (2 + rng.below(3)).min(count.max(1));
    let mut notes = vec![first];
    while notes.len() < count {
        let prev = *notes.last().unwrap_or(&first);
        let in_motif = notes.len() < motif_len;
        // Only a chord tone may be repeated — a note that still has to
        // resolve does so next, rather than being said twice.
        let prev_is_chord = fit(prev.midi, chord_tones, ctx.scale) == Fit::Chord;
        let repeat = !in_motif && prev_is_chord && rng.below(2) == 0;
        let next = if repeat {
            prev
        } else {
            step(ctx, chord_tones, anchor_midi, prev, allow_blue, rng)
                .or_else(|| resolve_from(ctx, chord_tones, prev))
                .unwrap_or(prev)
        };
        notes.push(next);
    }
    notes
}

/// Where `prev` goes when the walk has nowhere else to step: itself if it
/// is a chord tone, else the nearest chord tone within reach (which
/// [`usable`] guarantees exists for any note the walk chose).
fn resolve_from(
    ctx: &CallContext,
    chord_tones: &HashSet<String>,
    prev: PlayableNote,
) -> Option<PlayableNote> {
    if fit(prev.midi, chord_tones, ctx.scale) == Fit::Chord {
        return Some(prev);
    }
    snap(ctx, chord_tones, i32::from(prev.midi), Some(prev), Fit::Chord)
}

/// How the answering bar relates to the opening one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Contour {
    Repeat,
    Up,
    Down,
}

/// The answering bar: the opening's contour (its intervals from its first
/// note) re-sung from a new anchor — the same one, or the nearest chord
/// tone of this bar a step up or down — each note snapped to what the harp
/// can play and reach. A non-chord note is only ever followed by a chord
/// tone, and the cell's last note must be one.
fn answer_pitches(
    ctx: &CallContext,
    chord_tones: &HashSet<String>,
    opening: &[PlayableNote],
    count: usize,
    contour: Contour,
    rng: &mut Rng,
) -> Vec<PlayableNote> {
    if count == 0 {
        return Vec::new();
    }
    let Some(&first) = opening.first() else {
        return opening_pitches(ctx, chord_tones, count, rng);
    };
    let base = i32::from(first.midi);
    let anchor_target = match contour {
        Contour::Repeat => base,
        Contour::Up => base + ANSWER_SHIFT,
        Contour::Down => base - ANSWER_SHIFT,
    };
    let prev_note = opening.last().copied();
    let Some(new_anchor) = snap(ctx, chord_tones, anchor_target, prev_note, Fit::Chord)
        .or_else(|| snap(ctx, chord_tones, anchor_target, None, Fit::Chord))
    else {
        return Vec::new();
    };
    let shift = i32::from(new_anchor.midi) - base;
    let mut notes = vec![new_anchor];
    for i in 1..count {
        let source = opening.get(i).or(opening.last()).copied().unwrap_or(first);
        let target = i32::from(source.midi) + shift;
        let prev = *notes.last().unwrap_or(&new_anchor);
        let last = i + 1 == count;
        let min_fit = if last || fit(prev.midi, chord_tones, ctx.scale) != Fit::Chord {
            Fit::Chord
        } else {
            Fit::Scale
        };
        let next = snap(ctx, chord_tones, target, Some(prev), min_fit)
            .or_else(|| resolve_from(ctx, chord_tones, prev))
            .unwrap_or(prev);
        notes.push(next);
    }
    notes
}

/// Forces the phrase's final note onto a chord tone of its own bar, the
/// nearest one the harp can reach from the note before it.
fn resolve_last(ctx: &CallContext, chord_tones: &HashSet<String>, notes: &mut [PlayableNote]) {
    let n = notes.len();
    if n == 0 || fit(notes[n - 1].midi, chord_tones, ctx.scale) == Fit::Chord {
        return;
    }
    let prev = (n >= 2).then(|| notes[n - 2]);
    let target = i32::from(notes[n - 1].midi);
    if let Some(fixed) = snap(ctx, chord_tones, target, prev, Fit::Chord)
        .or_else(|| snap(ctx, chord_tones, target, None, Fit::Chord))
    {
        notes[n - 1] = fixed;
    }
}

/// Lays `pitches` onto `cell` in bar `bar`, one per entry.
fn lay_out(cell: Cell, pitches: &[PlayableNote], bar: usize, swung: bool) -> Vec<CallNote> {
    cell.iter()
        .zip(pitches)
        .map(|(&(slot, len), &note)| {
            let (start, ticks) = slot_span(slot, len, swung);
            CallNote {
                tick: bar * TICKS_PER_BAR + start,
                len: ticks,
                note,
                held: len >= HELD_SLOTS,
            }
        })
        .collect()
}

/// Generates one call: the opening bar's cell and motif, then the ending
/// bar's cell answered by contour, the whole thing resolved onto a chord
/// tone. Empty when the harp can't sound a single tone of the opening
/// chord.
pub fn generate_call(ctx: &CallContext) -> Vec<CallNote> {
    let mut rng = Rng(ctx.seed);
    let opening_cell = *rng.pick(openings(ctx.density));
    let ending_cell = *rng.pick(endings(ctx.density));
    let contour = *rng.pick(&[Contour::Repeat, Contour::Up, Contour::Down]);
    let mut opening = opening_pitches(ctx, ctx.opening_chord, opening_cell.len(), &mut rng);
    if opening.is_empty() {
        return Vec::new();
    }
    let mut answer =
        answer_pitches(ctx, ctx.ending_chord, &opening, ending_cell.len(), contour, &mut rng);
    if answer.is_empty() {
        // The whole phrase is the opening bar; it still has to land.
        resolve_last(ctx, ctx.opening_chord, &mut opening);
    } else {
        resolve_last(ctx, ctx.ending_chord, &mut answer);
    }
    let mut out = lay_out(opening_cell, &opening, 0, ctx.swung);
    out.extend(lay_out(ending_cell, &answer, 1, ctx.swung));
    out
}

/// The tick at which the last note of `notes` ends (0 for an empty call).
pub fn call_end_tick(notes: &[CallNote]) -> usize {
    notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0)
}

/// The latest tick a call may still be sounding — two beats before the
/// player's turn (see `ENDING_LAST_SLOT`).
pub fn call_space_tick() -> usize {
    (CALL_BARS - 1) * TICKS_PER_BAR + slot_tick(ENDING_LAST_SLOT, false)
}

#[cfg(test)]
mod tests;
