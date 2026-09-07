// SPDX-License-Identifier: MIT

//! The staff's decisions, with no Bevy in them: which clef the music wants,
//! where a pitch sits on that clef, which notehead a duration takes, where
//! bar lines fall, and which notes beam together.
//!
//! Split out from `mod.rs` when that file crossed the size budget
//! (`docs/physical_design_plan.md` rule 1). The split is also the module's
//! own testing seam — everything here is a pure function with unit tests,
//! and `mod.rs` is left as the translation of these answers into UI nodes.

use harmonicon_core::midi::midi_to_note;

// ── SMuFL glyphs (Bravura) ──────────────────────────────────────────────
//
// Codepoints are standardized by the SMuFL specification itself (every
// conformant SMuFL font, not just Bravura, uses the same ones) — see
// https://w3c.github.io/smufl/latest/tables/ or `glyphnames.json` in the
// https://github.com/w3c/smufl repo.

pub(super) mod glyph {
    pub const G_CLEF: &str = "\u{E050}";
    /// Treble clef with an "8" above it: notes sound an octave *higher*
    /// than written, which is how a 3.5-octave harmonica's top register
    /// stays near the staff instead of eleven ledger lines above it.
    pub const G_CLEF_8VA: &str = "\u{E053}";
    pub const F_CLEF: &str = "\u{E062}";
    /// `timeSig0`..`timeSig9` are consecutive, so a digit is this + n.
    pub const TIME_SIG_0: u32 = 0xE080;
    pub const NOTEHEAD_WHOLE: &str = "\u{E0A2}";
    pub const NOTEHEAD_HALF: &str = "\u{E0A3}";
    pub const NOTEHEAD_BLACK: &str = "\u{E0A4}";
    pub const ACCIDENTAL_SHARP: &str = "\u{E262}";
    pub const ACCIDENTAL_NATURAL: &str = "\u{E261}";
    pub const FLAG_8TH_UP: &str = "\u{E240}";
    pub const FLAG_8TH_DOWN: &str = "\u{E241}";
    pub const FLAG_16TH_UP: &str = "\u{E242}";
    pub const FLAG_16TH_DOWN: &str = "\u{E243}";
    pub const AUGMENTATION_DOT: &str = "\u{E1E7}";
}

// ── Pure notation logic ──────────────────────────────────────────────────

/// One note to draw on the staff. `start_beat`/`duration_beats` are in
/// quarter-note units — deliberately *not* ticks or seconds, so this
/// module never needs to know about a chart's tempo map or an editor's own
/// tick resolution; every caller converts its own time representation into
/// beats before handing notes over (see each call site: `song_editor`'s
/// ticks are already tempo-independent multiples of a beat; gameplay's
/// `ScheduledNote::time` in seconds goes through `song::chart::
/// seconds_to_tick` first). `midi` is the actual sounded pitch (bends/
/// overblow/overdraw/slide already resolved) — the same identity every
/// other pitch comparison in the codebase uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NotationNote {
    pub start_beat: f64,
    pub duration_beats: f64,
    pub midi: u8,
    /// True for every segment after the first when [`split_at_bar_lines`]
    /// split a longer note across a bar line — [`spawn_note_glyphs`] draws
    /// a short tie mark connecting it back to the segment immediately
    /// before it (always the note's own preceding beats, since a split
    /// segment starts exactly where the previous one ended). `false` for a
    /// note that was never split, and for a split note's own first segment.
    pub tied_from_previous: bool,
}

/// Splits `note` into one segment per bar it spans, so a note that would
/// otherwise be drawn as a single oversized notehead with no indication of
/// its true length instead becomes a run of bar-sized segments tied
/// together (see [`NotationNote::tied_from_previous`]). A note entirely
/// within one bar comes back unchanged (a one-element `Vec`). Splits only
/// at bar lines, not at every beat — this module's engraving is
/// deliberately coarse (see the module doc comment), so a note that starts
/// off the beat within a single bar still draws as one slightly-
/// mispositioned notehead.
pub fn split_at_bar_lines(note: NotationNote, beats_per_bar: f64) -> Vec<NotationNote> {
    if beats_per_bar <= 0.0 || note.duration_beats <= 0.0 {
        return vec![note];
    }
    let end = note.start_beat + note.duration_beats;
    let mut segments = Vec::new();
    let mut pos = note.start_beat;
    while pos < end {
        let bar_index = (pos / beats_per_bar).floor();
        let next_bar_start = (bar_index + 1.0) * beats_per_bar;
        let seg_end = next_bar_start.min(end);
        segments.push(NotationNote {
            start_beat: pos,
            duration_beats: seg_end - pos,
            midi: note.midi,
            tied_from_previous: !segments.is_empty(),
        });
        pos = seg_end;
    }
    segments
}

/// Which clef the staff is drawn in. Chosen once per song by
/// [`choose_clef`] from the music's own range, never mid-scroll — a clef
/// that changed under a moving playhead would be unreadable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Clef {
    /// Ordinary treble. Bottom line E4.
    #[default]
    Treble,
    /// Treble with an octave-up indicator: everything draws an octave
    /// lower than it sounds, so the bottom line reads as E5. The harmonica
    /// register that would otherwise sit far above the staff.
    Treble8va,
    /// Bass. Bottom line G2 — for low harps and the bass harmonica.
    Bass,
}

impl Clef {
    /// The staff step (see [`staff_step`]) of this clef's own bottom line,
    /// as a MIDI note. The single place each clef's vertical base lives:
    /// [`staff_step`] subtracts it, and everything downstream
    /// ([`y_for_step`], [`ledger_line_steps`]) then works in
    /// already-clef-relative steps and needs no clef of its own.
    fn base_midi(self) -> u8 {
        match self {
            Clef::Treble => 64,    // E4
            Clef::Treble8va => 76, // E5 — drawn an octave below its sound
            Clef::Bass => 43,      // G2
        }
    }

    /// The SMuFL glyph to draw.
    pub fn glyph(self) -> &'static str {
        match self {
            Clef::Treble => glyph::G_CLEF,
            Clef::Treble8va => glyph::G_CLEF_8VA,
            Clef::Bass => glyph::F_CLEF,
        }
    }

    /// The staff step this glyph's own SMuFL origin is anchored on. A G
    /// clef's curl circles the G line (step 2 in its own staff); an F
    /// clef's two dots straddle the F line, the 4th line up (step 6).
    pub fn anchor_step(self) -> i32 {
        match self {
            Clef::Treble | Clef::Treble8va => 2,
            Clef::Bass => 6,
        }
    }
}

/// Diatonic staff step for `midi` in `clef`, where the staff's bottom line
/// is step 0 and each step is one staff position (a line or a space) —
/// *not* one semitone. This is what makes a sharp share its natural
/// neighbor's step, distinguished only by an accidental glyph (see
/// [`needs_sharp`]): staff position is decided by the note's *letter
/// name*, not its exact pitch.
pub fn staff_step(midi: u8, clef: Clef) -> i32 {
    let name = midi_to_note(midi as i32); // sharp-only spelling, e.g. "C#4", "E4"
    let bytes = name.as_bytes();
    let has_sharp = bytes.get(1) == Some(&b'#');
    let letter = bytes[0] as char;
    let octave: i32 = name[if has_sharp { 2 } else { 1 }..].parse().unwrap_or(4);
    let letter_index = match letter {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        'B' => 6,
        _ => 2,
    };
    let base = midi_to_note(clef.base_midi() as i32);
    let base_bytes = base.as_bytes();
    let base_sharp = base_bytes.get(1) == Some(&b'#');
    let base_octave: i32 = base[if base_sharp { 2 } else { 1 }..].parse().unwrap_or(4);
    let base_letter = match base_bytes[0] as char {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        _ => 6,
    };
    (octave * 7 + letter_index) - (base_octave * 7 + base_letter)
}

/// Picks the clef whose staff the music sits most comfortably inside.
///
/// Driven by the *median* pitch rather than the extremes, so one stray
/// high or low note can't drag the whole staff. The thresholds are fixed
/// (no hysteresis), so a given set of notes always yields the same clef.
/// Empty input keeps [`Clef::Treble`], the historical default.
pub fn choose_clef(notes: &[NotationNote]) -> Clef {
    if notes.is_empty() {
        return Clef::Treble;
    }
    let mut pitches: Vec<u8> = notes.iter().map(|n| n.midi).collect();
    pitches.sort_unstable();
    let median = pitches[pitches.len() / 2];
    match median {
        // At or above the treble staff's top line (F5 = 77) the music is
        // spending its time in ledger lines; read it an octave down.
        m if m >= 77 => Clef::Treble8va,
        // Below middle C the bass staff centres it better than treble does.
        m if m < 60 => Clef::Bass,
        _ => Clef::Treble,
    }
}

/// The accidental glyph drawn before a notehead, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Accidental {
    None,
    Sharp,
    Natural,
}

/// Which accidental each note needs, following the bar rather than the note.
///
/// An accidental holds for the rest of its bar, so a repeated F# is marked
/// once and not on every note — which is both what a reader expects and, on
/// a staff this cramped, a real amount of width back: a sharp is about as
/// wide as a notehead and is drawn into the gap left by the previous note.
///
/// **The naturals are the reason this can't just be "skip the repeats".**
/// Marking the first F# and nothing afterwards would leave a plain F later
/// in the same bar reading as F# too, since the alteration is still in
/// force. Every such note now gets an explicit natural. So this is a
/// correctness fix that usually also saves width, not purely a saving — a
/// bar alternating F# and F ends up with *more* glyphs than before, and
/// says something true where it previously said something false.
///
/// Keyed on staff *position*, not pitch: [`staff_step`] puts F4 and F#4 on
/// the same line, which is exactly what an accidental applies to. Expects
/// `notes` in start order, and reads the whole song rather than the visible
/// window so a note doesn't lose its accidental just because the note that
/// established it has scrolled off.
pub fn accidentals(notes: &[NotationNote], clef: Clef, beats_per_bar: f64) -> Vec<Accidental> {
    let mut out = Vec::with_capacity(notes.len());
    // Staff step -> is that position currently sharpened. Cleared at every
    // bar line, which is what makes this "per bar" at all.
    let mut altered: std::collections::HashMap<i32, bool> = std::collections::HashMap::new();
    let mut current_bar: Option<i64> = None;

    for note in notes {
        let bar = if beats_per_bar > 0.0 {
            (note.start_beat / beats_per_bar).floor() as i64
        } else {
            0
        };
        if current_bar != Some(bar) {
            altered.clear();
            current_bar = Some(bar);
        }

        let step = staff_step(note.midi, clef);
        let sharp = needs_sharp(note.midi);
        let was_sharp = altered.get(&step).copied().unwrap_or(false);
        out.push(if note.tied_from_previous {
            // The tie carries the alteration across the bar line; restating
            // it on the continuation is what the tie is for.
            Accidental::None
        } else if sharp && !was_sharp {
            Accidental::Sharp
        } else if !sharp && was_sharp {
            Accidental::Natural
        } else {
            Accidental::None
        });
        // Recorded even for a tied continuation, so a later note at this
        // position in the new bar is judged against what is actually
        // sounding.
        altered.insert(step, sharp);
    }
    out
}

/// Whether `midi` needs a sharp accidental drawn before its notehead. This
/// codebase spells every accidental as a sharp, never a flat (see
/// `audio_system::midi::midi_to_note`'s own doc comment), so a sharp is the
/// only accidental glyph the staff ever needs to draw.
pub fn needs_sharp(midi: u8) -> bool {
    midi_to_note(midi as i32).as_bytes().get(1) == Some(&b'#')
}

/// The staff steps (see [`staff_step`]) a ledger line is drawn at for a
/// note at `step` — empty while `step` is within the staff (`0..=8`).
/// Ledger lines fall at every *even* step from the staff's own edge out to
/// (and including, if even) `step` itself; a note sitting in the
/// intervening odd step (a space just outside the staff, e.g. D4 just
/// below the staff, or G5 just above it) needs none at all — the classic
/// "middle C gets one ledger line, the note in the space just below it
/// still reads against that same line" rule.
pub fn ledger_line_steps(step: i32) -> Vec<i32> {
    let mut out = Vec::new();
    if step < 0 {
        let mut s = -2;
        while s >= step {
            out.push(s);
            s -= 2;
        }
    } else if step > 8 {
        let mut s = 10;
        while s <= step {
            out.push(s);
            s += 2;
        }
    }
    out
}

/// Which notehead shape a note's duration gets. Deliberately coarse — see
/// the module doc comment on engraving scope: quarter notes and anything
/// shorter all get the same filled notehead + plain stem, with no flags or
/// beaming to distinguish an eighth from a quarter. Thresholds sit at the
/// midpoint between adjacent standard durations (2 and 4 beats) so a
/// slightly-off recorded/quantized duration still classifies as intended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteheadKind {
    Whole,
    Half,
    Filled,
}

impl NoteheadKind {
    pub(super) fn glyph(self) -> &'static str {
        match self {
            NoteheadKind::Whole => glyph::NOTEHEAD_WHOLE,
            NoteheadKind::Half => glyph::NOTEHEAD_HALF,
            NoteheadKind::Filled => glyph::NOTEHEAD_BLACK,
        }
    }

    /// A whole note conventionally draws no stem at all.
    pub(super) fn has_stem(self) -> bool {
        self != NoteheadKind::Whole
    }

    /// Notehead width, in staff spaces — `bravura_metadata.json`'s own
    /// `glyphBBoxes` (`noteheadWhole`/`noteheadHalf`/`noteheadBlack`).
    pub(super) fn width_sp(self) -> f32 {
        match self {
            NoteheadKind::Whole => 1.688,
            NoteheadKind::Half | NoteheadKind::Filled => 1.18,
        }
    }
}

/// What a duration is actually drawn as: which notehead, how many
/// augmentation dots, and how many flags (or beams) it carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rhythm {
    pub head: NoteheadKind,
    /// 0 or 1 — this module doesn't spell double-dotted durations.
    pub dots: u8,
    /// 0 (quarter or longer), 1 (eighth), or 2 (sixteenth or shorter).
    pub flags: u8,
}

/// Rounds `duration_beats` to the nearest standard note value and reports
/// how to draw it.
///
/// Each threshold is the midpoint between two adjacent standard durations,
/// the same rule the notehead choice already used, extended to cover the
/// dotted values and the sixteenth. Getting this wrong misinforms rather
/// than merely looks plain: before dots existed a dotted quarter (1.5)
/// drew as a half note, which *reads* as two beats, and a sixteenth drew
/// the eighth's single flag.
pub fn note_rhythm(duration_beats: f64) -> Rhythm {
    let (head, dots, flags) = if duration_beats >= 3.5 {
        (NoteheadKind::Whole, 0, 0) // whole
    } else if duration_beats >= 2.5 {
        (NoteheadKind::Half, 1, 0) // dotted half
    } else if duration_beats >= 1.75 {
        (NoteheadKind::Half, 0, 0) // half
    } else if duration_beats >= 1.25 {
        (NoteheadKind::Filled, 1, 0) // dotted quarter
    } else if duration_beats >= 0.875 {
        (NoteheadKind::Filled, 0, 0) // quarter
    } else if duration_beats >= 0.625 {
        (NoteheadKind::Filled, 1, 1) // dotted eighth
    } else if duration_beats >= 0.375 {
        (NoteheadKind::Filled, 0, 1) // eighth
    } else {
        (NoteheadKind::Filled, 0, 2) // sixteenth and shorter
    };
    Rhythm { head, dots, flags }
}

/// The staff step an augmentation dot sits at for a notehead on `step`.
///
/// A dot always goes in a *space*, never on a line: for a notehead already
/// in a space (odd step) that is its own step, and for one on a line (even
/// step) it is the space just above.
pub fn dot_step(step: i32) -> i32 {
    if step.rem_euclid(2) == 0 {
        step + 1
    } else {
        step
    }
}

pub fn notehead_kind(duration_beats: f64) -> NoteheadKind {
    note_rhythm(duration_beats).head
}

/// Whether a note gets a single eighth-note flag drawn at its stem tip.
/// Deliberately coarse per this module's "eighth notes only" scope (see the
/// module doc comment): anything shorter than an eighth still gets exactly
/// one flag, not two — there's no sixteenth-note (or shorter) flag tier.
/// Only meaningful for a note that already has a stem at all
/// (`NoteheadKind::Filled` — `Half`/`Whole` are always `>= 1.5` beats, well
/// above this threshold, so they never qualify regardless).
pub fn has_eighth_flag(duration_beats: f64) -> bool {
    note_rhythm(duration_beats).flags > 0
}

/// The SMuFL glyphs spelling `n` — `timeSig0`..`timeSig9` are consecutive
/// codepoints, so each decimal digit maps straight onto one. Multi-digit
/// numerators (7/8, 12/8) therefore just concatenate.
/// Where one note sits inside a beam group, and the group's shared
/// decisions. Computed once per group by [`beam_groups`] so the per-note
/// spawn code stays a translation step: it never re-decides direction or
/// length, it just draws what it is handed.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BeamPlacement {
    /// Shared by every note in the group — a beam with stems going both
    /// ways is not a beam.
    pub stem_up: bool,
    /// Staff step the beam sits at. Every stem in the group is drawn to
    /// this, which is what makes the beam a single straight line.
    pub beam_step: i32,
    /// True for the first note of the group, which is the one that draws
    /// the beam itself (spanning to the last note's stem).
    pub is_first: bool,
    /// Beats from this note's stem to the group's last one — the beam's
    /// own width. Zero for every note but the first.
    pub span_beats: f64,
    /// How many beams to stack: the most flags any note in the group would
    /// have carried, so a group containing a sixteenth gets two.
    pub beams: u8,
}

/// The staff's middle line, in steps — the pivot stem direction turns on:
/// a note below it takes an up stem, above it a down stem, so stems point
/// back toward the staff instead of running off it.
pub(super) const MIDDLE_LINE_STEP: i32 = 4;

/// How far a stem reaches past its notehead, in staff steps — 3.5 staff
/// spaces is the conventional length, and a staff space is 2 steps.
const STEM_LENGTH_STEPS: i32 = 7;

/// Groups consecutive notes into beams, returning one entry per input note
/// (`None` where the note is not beamed).
///
/// Beams a maximal run that is all flag-worthy ([`has_eighth_flag`]),
/// gapless (each note starting where the last ended), and inside one beat
/// — the ordinary rule that keeps the beat visible when reading. A run of
/// one keeps its flag instead, since a beam needs two stems to span.
///
/// Direction follows the standard rule: whichever note of the group sits
/// furthest from the middle line decides for all of them, so the beam
/// leans away from the staff rather than through it.
pub fn beam_groups(notes: &[NotationNote], clef: Clef) -> Vec<Option<BeamPlacement>> {
    let mut out: Vec<Option<BeamPlacement>> = vec![None; notes.len()];
    let mut i = 0;
    while i < notes.len() {
        if !has_eighth_flag(notes[i].duration_beats) {
            i += 1;
            continue;
        }
        // Extend while the next note continues this beat without a gap.
        let mut j = i;
        while j + 1 < notes.len() {
            let cur = &notes[j];
            let next = &notes[j + 1];
            let contiguous = (next.start_beat - (cur.start_beat + cur.duration_beats)).abs() < 1e-6;
            let same_beat = cur.start_beat.floor() == next.start_beat.floor();
            if !has_eighth_flag(next.duration_beats) || !contiguous || !same_beat {
                break;
            }
            j += 1;
        }
        if j > i {
            let group = &notes[i..=j];
            let steps: Vec<i32> = group.iter().map(|n| staff_step(n.midi, clef)).collect();
            // Furthest from the middle line (step 4) decides for the group.
            let extreme = *steps
                .iter()
                .max_by_key(|s| (*s - MIDDLE_LINE_STEP).abs())
                .unwrap_or(&MIDDLE_LINE_STEP);
            let stem_up = extreme < MIDDLE_LINE_STEP;
            // A horizontal beam has to clear the group's own extreme note,
            // so measure from whichever stem would be longest.
            let beam_step = if stem_up {
                steps.iter().max().unwrap_or(&0) + STEM_LENGTH_STEPS
            } else {
                steps.iter().min().unwrap_or(&0) - STEM_LENGTH_STEPS
            };
            let last = &notes[j];
            let span_beats = last.start_beat - notes[i].start_beat;
            // The group is drawn with as many beams as its shortest note
            // needs; a plain eighth beside a sixteenth still gets one beam
            // through it, which is the ordinary simplification here (real
            // engraving would use a partial second beam).
            let beams = group
                .iter()
                .map(|n| note_rhythm(n.duration_beats).flags)
                .max()
                .unwrap_or(1);
            for (k, slot) in out[i..=j].iter_mut().enumerate() {
                *slot = Some(BeamPlacement {
                    stem_up,
                    beam_step,
                    is_first: k == 0,
                    span_beats: if k == 0 { span_beats } else { 0.0 },
                    beams,
                });
            }
        }
        i = j + 1;
    }
    out
}

pub(super) fn time_sig_glyphs(n: u8) -> String {
    n.to_string()
        .chars()
        .filter_map(|c| c.to_digit(10))
        .filter_map(|d| char::from_u32(glyph::TIME_SIG_0 + d))
        .collect()
}

/// Every bar-line position (in beats) strictly inside `from..=to`.
///
/// Beat 0 is a bar line but never drawn — it's the start of the piece, not
/// a division within it, and drawing it would put a line through the clef
/// on the first frame.
pub fn bar_line_beats(from_beat: f64, to_beat: f64, beats_per_bar: f64) -> Vec<f64> {
    if beats_per_bar <= 0.0 || to_beat < from_beat {
        return Vec::new();
    }
    let first = (from_beat / beats_per_bar).ceil().max(1.0) as i64;
    let last = (to_beat / beats_per_bar).floor() as i64;
    (first..=last).map(|b| b as f64 * beats_per_bar).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_step_places_e4_at_the_bottom_line() {
        assert_eq!(staff_step(64, Clef::Treble), 0); // E4 = MIDI 64
    }

    #[test]
    fn staff_step_places_middle_c_two_steps_below_the_staff() {
        assert_eq!(staff_step(60, Clef::Treble), -2); // C4 = MIDI 60
    }

    #[test]
    fn staff_step_places_f5_at_the_top_line() {
        assert_eq!(staff_step(77, Clef::Treble), 8); // F5 = MIDI 77
    }

    #[test]
    fn staff_step_shares_the_same_step_as_its_sharp() {
        // C4 and C#4 sit on the same staff position; only the accidental differs.
        assert_eq!(staff_step(60, Clef::Treble), staff_step(61, Clef::Treble));
    }

    #[test]
    fn each_clef_puts_its_own_bottom_line_at_step_zero() {
        assert_eq!(staff_step(64, Clef::Treble), 0); // E4, treble bottom line
        assert_eq!(staff_step(76, Clef::Treble8va), 0); // E5, drawn an octave down
        assert_eq!(staff_step(43, Clef::Bass), 0); // G2, bass bottom line
    }

    #[test]
    fn the_8va_clef_draws_a_note_an_octave_lower_than_treble_would() {
        // C7 is six ledger lines above the treble staff; under 8va it
        // lands on the same step C6 would in plain treble.
        assert_eq!(
            staff_step(96, Clef::Treble8va),
            staff_step(84, Clef::Treble)
        );
    }

    #[test]
    fn bass_clef_pulls_a_low_note_back_onto_the_staff() {
        // G3 is three ledger lines below the treble staff, and the second
        // line from the top in bass.
        assert!(staff_step(55, Clef::Treble) < 0);
        assert_eq!(staff_step(55, Clef::Bass), 7);
    }

    #[test]
    fn choose_clef_reads_the_median_not_the_extremes() {
        // One stray high note must not drag the whole staff up.
        let mut ns: Vec<NotationNote> = (0..9).map(|i| note_at(60 + i, 0.0)).collect();
        ns.push(note_at(100, 0.0));
        assert_eq!(choose_clef(&ns), Clef::Treble);
    }

    #[test]
    fn choose_clef_picks_8va_for_the_shipped_harmonica_range() {
        // Measured median across the bundled charts is F5 (MIDI 77) —
        // exactly the treble top line, i.e. half the music in ledger lines.
        let ns = vec![note_at(72, 0.0), note_at(77, 0.0), note_at(88, 0.0)];
        assert_eq!(choose_clef(&ns), Clef::Treble8va);
    }

    #[test]
    fn choose_clef_picks_bass_below_middle_c_and_treble_otherwise() {
        assert_eq!(
            choose_clef(&[note_at(50, 0.0), note_at(55, 0.0), note_at(57, 0.0)]),
            Clef::Bass
        );
        assert_eq!(
            choose_clef(&[note_at(64, 0.0), note_at(67, 0.0), note_at(71, 0.0)]),
            Clef::Treble
        );
    }

    #[test]
    fn choose_clef_falls_back_to_treble_with_nothing_to_judge() {
        assert_eq!(choose_clef(&[]), Clef::Treble);
    }

    // ── time signature / bar lines ───────────────────────────────────────

    #[test]
    fn time_sig_glyphs_maps_each_digit_and_handles_two_of_them() {
        assert_eq!(time_sig_glyphs(4), "\u{E084}");
        assert_eq!(time_sig_glyphs(12), "\u{E081}\u{E082}");
    }

    #[test]
    fn bar_line_beats_lands_on_every_bar_boundary_in_the_window() {
        assert_eq!(bar_line_beats(0.0, 9.0, 4.0), vec![4.0, 8.0]);
    }

    #[test]
    fn bar_line_beats_skips_beat_zero() {
        // Beat 0 starts the piece rather than dividing it, and a line
        // there would cut through the clef.
        assert!(!bar_line_beats(0.0, 4.0, 4.0).contains(&0.0));
    }

    #[test]
    fn bar_line_beats_includes_a_boundary_exactly_at_either_end() {
        assert_eq!(bar_line_beats(4.0, 8.0, 4.0), vec![4.0, 8.0]);
    }

    #[test]
    fn bar_line_beats_is_empty_for_a_window_inside_one_bar() {
        assert!(bar_line_beats(4.5, 7.5, 4.0).is_empty());
    }

    #[test]
    fn bar_line_beats_is_empty_rather_than_looping_on_a_degenerate_meter() {
        assert!(bar_line_beats(0.0, 100.0, 0.0).is_empty());
        assert!(bar_line_beats(9.0, 0.0, 4.0).is_empty());
    }

    // ── beaming ──────────────────────────────────────────────────────────

    /// An eighth note (0.5 beats) at a pitch and time.
    fn eighth(midi: u8, start_beat: f64) -> NotationNote {
        NotationNote {
            start_beat,
            duration_beats: 0.5,
            midi,
            tied_from_previous: false,
        }
    }

    #[test]
    fn two_eighths_in_one_beat_are_beamed_together() {
        let ns = [eighth(64, 0.0), eighth(65, 0.5)];
        let b = beam_groups(&ns, Clef::Treble);
        assert!(b[0].is_some() && b[1].is_some());
        assert!(b[0].unwrap().is_first);
        assert!(!b[1].unwrap().is_first);
    }

    #[test]
    fn a_beam_group_never_crosses_a_beat_boundary() {
        // Four eighths: 0.0/0.5 share beat 0, 1.0/1.5 share beat 1. That
        // is two beams of two, not one of four — the point of beaming is
        // to keep the beat visible.
        let ns = [
            eighth(64, 0.0),
            eighth(64, 0.5),
            eighth(64, 1.0),
            eighth(64, 1.5),
        ];
        let b = beam_groups(&ns, Clef::Treble);
        assert!(b[0].unwrap().is_first);
        assert!(!b[1].unwrap().is_first);
        assert!(b[2].unwrap().is_first, "a new beat starts a new beam");
        assert!(!b[3].unwrap().is_first);
    }

    #[test]
    fn a_gap_between_notes_breaks_the_beam() {
        // Second eighth starts half a beat late, so they are not adjacent.
        let ns = [eighth(64, 0.0), eighth(64, 0.75)];
        assert_eq!(beam_groups(&ns, Clef::Treble), vec![None, None]);
    }

    #[test]
    fn a_lone_short_note_keeps_its_flag_instead_of_a_beam() {
        let ns = [eighth(64, 0.0), note(1.0, 1.0)];
        assert!(beam_groups(&ns, Clef::Treble)[0].is_none());
    }

    #[test]
    fn a_quarter_note_is_never_beamed() {
        let ns = [note(0.0, 1.0), note(1.0, 1.0)];
        assert_eq!(beam_groups(&ns, Clef::Treble), vec![None, None]);
    }

    #[test]
    fn the_note_furthest_from_the_middle_line_sets_the_groups_stem_direction() {
        // C4 sits far below the middle line, G4 just below it: the group
        // follows C4 and stems up, even though they average out close.
        let low = beam_groups(&[eighth(60, 0.0), eighth(67, 0.5)], Clef::Treble);
        assert!(low[0].unwrap().stem_up);

        // A5 is far above the middle line, so the same pairing flips.
        let high = beam_groups(&[eighth(81, 0.0), eighth(71, 0.5)], Clef::Treble);
        assert!(!high[0].unwrap().stem_up);
    }

    #[test]
    fn every_note_in_a_group_shares_one_direction_and_beam_line() {
        let ns = [eighth(60, 0.0), eighth(67, 0.5)];
        let b = beam_groups(&ns, Clef::Treble);
        let (first, second) = (b[0].unwrap(), b[1].unwrap());
        assert_eq!(first.stem_up, second.stem_up);
        assert_eq!(first.beam_step, second.beam_step);
    }

    #[test]
    fn the_beam_clears_the_groups_own_extreme_note() {
        // Stems up: the beam must sit above the *highest* notehead, or the
        // tallest stem would poke through it.
        let ns = [eighth(60, 0.0), eighth(67, 0.5)];
        let b = beam_groups(&ns, Clef::Treble)[0].unwrap();
        assert!(b.stem_up);
        assert!(b.beam_step > staff_step(67, Clef::Treble));
    }

    #[test]
    fn a_group_of_sixteenths_gets_two_beams() {
        let sixteenth = |start: f64| NotationNote {
            start_beat: start,
            duration_beats: 0.25,
            midi: 64,
            tied_from_previous: false,
        };
        let b = beam_groups(&[sixteenth(0.0), sixteenth(0.25)], Clef::Treble);
        assert_eq!(b[0].unwrap().beams, 2);
    }

    #[test]
    fn a_group_of_eighths_gets_one_beam() {
        let b = beam_groups(&[eighth(64, 0.0), eighth(64, 0.5)], Clef::Treble);
        assert_eq!(b[0].unwrap().beams, 1);
    }

    #[test]
    fn a_mixed_group_takes_its_shortest_notes_beam_count() {
        let ns = [
            eighth(64, 0.0),
            NotationNote {
                start_beat: 0.5,
                duration_beats: 0.25,
                midi: 64,
                tied_from_previous: false,
            },
        ];
        assert_eq!(beam_groups(&ns, Clef::Treble)[0].unwrap().beams, 2);
    }

    #[test]
    fn only_the_first_note_carries_the_span_to_draw() {
        let ns = [eighth(64, 0.0), eighth(64, 0.5)];
        let b = beam_groups(&ns, Clef::Treble);
        assert_eq!(b[0].unwrap().span_beats, 0.5);
        assert_eq!(b[1].unwrap().span_beats, 0.0);
    }

    #[test]
    fn beam_groups_returns_one_slot_per_note() {
        let ns = [eighth(64, 0.0), eighth(64, 0.5), note(1.0, 2.0)];
        assert_eq!(beam_groups(&ns, Clef::Treble).len(), ns.len());
    }

    // ── accidentals ──────────────────────────────────────────────────────

    /// Quarter notes, one per beat, at the given pitches.
    fn run(midis: &[u8]) -> Vec<NotationNote> {
        midis
            .iter()
            .enumerate()
            .map(|(i, &midi)| NotationNote {
                start_beat: i as f64,
                duration_beats: 1.0,
                midi,
                tied_from_previous: false,
            })
            .collect()
    }

    #[test]
    fn a_repeated_sharp_is_marked_once_per_bar() {
        // The width this buys back: a sharp is about as wide as a notehead
        // and is drawn into the gap the previous note left.
        let notes = run(&[66, 66, 66, 66]); // F#4 four times, one bar of 4/4
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![
                Accidental::Sharp,
                Accidental::None,
                Accidental::None,
                Accidental::None
            ]
        );
    }

    #[test]
    fn the_next_bar_restates_it() {
        let notes = run(&[66, 66, 66, 66, 66]); // the fifth note is bar two
        let marks = accidentals(&notes, Clef::Treble, 4.0);
        assert_eq!(marks[4], Accidental::Sharp);
    }

    #[test]
    fn a_natural_after_a_sharp_in_the_same_bar_is_marked() {
        // Without this the plain F would read as F#, since the alteration
        // holds for the rest of the bar — the staff would be saying
        // something false rather than merely something redundant.
        let notes = run(&[66, 65]); // F#4 then F4
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![Accidental::Sharp, Accidental::Natural]
        );
    }

    #[test]
    fn a_natural_needs_no_mark_when_nothing_sharpened_it() {
        let notes = run(&[65, 65]);
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![Accidental::None, Accidental::None]
        );
    }

    #[test]
    fn a_sharp_elsewhere_on_the_staff_does_not_cancel_this_one() {
        // An accidental applies to its own line or space, not to the whole
        // staff: C#4 has no bearing on F4.
        let notes = run(&[61, 65]); // C#4 then F4
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![Accidental::Sharp, Accidental::None]
        );
    }

    #[test]
    fn a_sharp_carries_across_an_octave_boundary_no_further_than_its_own_step() {
        // F#4 and F#5 sit on different lines, so each needs its own mark.
        let notes = run(&[66, 78]);
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![Accidental::Sharp, Accidental::Sharp]
        );
    }

    #[test]
    fn a_tie_across_a_bar_line_is_not_restated() {
        // `split_at_bar_lines` puts the continuation in the next bar, where
        // the bar state has just been cleared — the tie is what carries the
        // alteration over, so restating it would be wrong.
        let notes = vec![
            NotationNote {
                start_beat: 3.0,
                duration_beats: 1.0,
                midi: 66,
                tied_from_previous: false,
            },
            NotationNote {
                start_beat: 4.0,
                duration_beats: 1.0,
                midi: 66,
                tied_from_previous: true,
            },
        ];
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![Accidental::Sharp, Accidental::None]
        );
    }

    #[test]
    fn a_note_after_a_tied_continuation_is_judged_against_what_is_sounding() {
        // The tied-in F# leaves that line sharpened, so the following plain
        // F in the same bar still needs its natural.
        let notes = vec![
            NotationNote {
                start_beat: 4.0,
                duration_beats: 1.0,
                midi: 66,
                tied_from_previous: true,
            },
            NotationNote {
                start_beat: 5.0,
                duration_beats: 1.0,
                midi: 65,
                tied_from_previous: false,
            },
        ];
        assert_eq!(
            accidentals(&notes, Clef::Treble, 4.0),
            vec![Accidental::None, Accidental::Natural]
        );
    }

    #[test]
    fn a_meaningless_bar_length_does_not_panic_or_divide_by_zero() {
        let notes = run(&[66, 66]);
        assert_eq!(accidentals(&notes, Clef::Treble, 0.0).len(), 2);
    }

    #[test]
    fn there_is_exactly_one_decision_per_note() {
        let notes = run(&[60, 61, 62, 63, 64, 65, 66]);
        assert_eq!(accidentals(&notes, Clef::Treble, 4.0).len(), notes.len());
    }

    #[test]
    fn needs_sharp_is_true_only_for_a_sharp_spelling() {
        assert!(!needs_sharp(60)); // C4
        assert!(needs_sharp(61)); // C#4
        assert!(!needs_sharp(64)); // E4
    }

    #[test]
    fn ledger_line_steps_is_empty_within_the_staff() {
        assert!(ledger_line_steps(0).is_empty());
        assert!(ledger_line_steps(4).is_empty());
        assert!(ledger_line_steps(8).is_empty());
    }

    #[test]
    fn ledger_line_steps_middle_c_gets_exactly_one() {
        assert_eq!(ledger_line_steps(-2), vec![-2]);
    }

    #[test]
    fn ledger_line_steps_the_space_just_outside_the_staff_needs_none() {
        // D4 (step -1, the space directly below the bottom line) and G5
        // (step 9, the space directly above the top line) both sit closer
        // to the staff than any ledger line would be.
        assert!(ledger_line_steps(-1).is_empty());
        assert!(ledger_line_steps(9).is_empty());
    }

    #[test]
    fn ledger_line_steps_a_note_in_the_gap_beyond_the_first_ledger_still_gets_it() {
        // B3 (step -3, a space) is one step further than middle C (-2) —
        // it still reads against that same first ledger line.
        assert_eq!(ledger_line_steps(-3), vec![-2]);
        // A3 (step -4, on a line) needs two.
        assert_eq!(ledger_line_steps(-4), vec![-2, -4]);
    }

    #[test]
    fn ledger_line_steps_above_the_staff_mirrors_below() {
        assert_eq!(ledger_line_steps(10), vec![10]); // A5
        assert_eq!(ledger_line_steps(11), vec![10]); // B5, a space
        assert_eq!(ledger_line_steps(12), vec![10, 12]); // C6
    }

    #[test]
    fn notehead_kind_classifies_by_duration() {
        assert_eq!(notehead_kind(4.0), NoteheadKind::Whole);
        // 3 beats is a *dotted half*, not a whole — a whole note is 4.
        assert_eq!(notehead_kind(3.0), NoteheadKind::Half);
        assert_eq!(notehead_kind(2.0), NoteheadKind::Half);
        // Likewise 1.5 is a dotted quarter, not a half.
        assert_eq!(notehead_kind(1.5), NoteheadKind::Filled);
        assert_eq!(notehead_kind(1.0), NoteheadKind::Filled);
        assert_eq!(notehead_kind(0.25), NoteheadKind::Filled);
    }

    #[test]
    fn whole_notes_have_no_stem() {
        assert!(!NoteheadKind::Whole.has_stem());
        assert!(NoteheadKind::Half.has_stem());
        assert!(NoteheadKind::Filled.has_stem());
    }

    #[test]
    fn has_eighth_flag_is_true_for_anything_shorter_than_a_quarter() {
        assert!(!has_eighth_flag(1.0)); // quarter note
        assert!(has_eighth_flag(0.75)); // dotted eighth — one flag and a dot
        assert!(has_eighth_flag(0.5)); // eighth note
        assert!(has_eighth_flag(0.25)); // sixteenth — two flags now
    }

    #[test]
    fn note_rhythm_spells_the_dotted_durations() {
        // These are the cases the old thresholds got wrong: both used to
        // draw as the *next longer* undotted value, which reads as more
        // beats than the note actually lasts.
        let dotted_half = note_rhythm(3.0);
        assert_eq!(dotted_half.head, NoteheadKind::Half);
        assert_eq!(dotted_half.dots, 1);

        let dotted_quarter = note_rhythm(1.5);
        assert_eq!(dotted_quarter.head, NoteheadKind::Filled);
        assert_eq!(dotted_quarter.dots, 1);
        assert_eq!(dotted_quarter.flags, 0);

        let dotted_eighth = note_rhythm(0.75);
        assert_eq!(dotted_eighth.dots, 1);
        assert_eq!(dotted_eighth.flags, 1);
    }

    #[test]
    fn note_rhythm_gives_a_sixteenth_its_second_flag() {
        assert_eq!(note_rhythm(0.5).flags, 1);
        assert_eq!(note_rhythm(0.25).flags, 2);
        // Shorter than a sixteenth still rounds up to two — this module
        // has no thirty-second tier.
        assert_eq!(note_rhythm(0.1).flags, 2);
    }

    #[test]
    fn note_rhythm_leaves_the_plain_durations_undotted() {
        for beats in [4.0, 2.0, 1.0, 0.5, 0.25] {
            assert_eq!(note_rhythm(beats).dots, 0, "{beats} beats");
        }
    }

    #[test]
    fn a_dot_always_sits_in_a_space_never_on_a_line() {
        // Even steps are lines, odd are spaces.
        assert_eq!(dot_step(0), 1); // bottom line -> space above
        assert_eq!(dot_step(4), 5); // middle line -> space above
        assert_eq!(dot_step(3), 3); // already a space -> stays
        assert_eq!(dot_step(-2), -1); // below the staff, same rule
    }

    #[test]
    fn has_eighth_flag_never_applies_to_half_or_whole_notes() {
        // Half/whole are always well above the flag threshold, so a note
        // long enough to have no stem at all never picks one up either.
        assert!(!has_eighth_flag(2.0));
        assert!(!has_eighth_flag(4.0));
    }

    /// A note at a given pitch, for the clef/beam tests (the `note`
    /// helper below fixes MIDI 60 because its own tests only vary time).
    fn note_at(midi: u8, start_beat: f64) -> NotationNote {
        NotationNote {
            start_beat,
            duration_beats: 0.5,
            midi,
            tied_from_previous: false,
        }
    }

    fn note(start_beat: f64, duration_beats: f64) -> NotationNote {
        NotationNote {
            start_beat,
            duration_beats,
            midi: 60,
            tied_from_previous: false,
        }
    }

    #[test]
    fn split_at_bar_lines_leaves_a_note_within_one_bar_untouched() {
        let segments = split_at_bar_lines(note(1.0, 2.0), 4.0);
        assert_eq!(segments, vec![note(1.0, 2.0)]);
        assert!(!segments[0].tied_from_previous);
    }

    #[test]
    fn split_at_bar_lines_splits_a_note_crossing_one_bar_line() {
        // Starts at beat 2 of bar 0 (4 beats/bar), lasts 4 beats: ends at
        // beat 6, i.e. beat 2 of bar 1. Splits into [2, 4) and [4, 6).
        let segments = split_at_bar_lines(note(2.0, 4.0), 4.0);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_beat, 2.0);
        assert_eq!(segments[0].duration_beats, 2.0);
        assert!(!segments[0].tied_from_previous);
        assert_eq!(segments[1].start_beat, 4.0);
        assert_eq!(segments[1].duration_beats, 2.0);
        assert!(segments[1].tied_from_previous);
    }

    #[test]
    fn split_at_bar_lines_splits_a_note_spanning_several_bars() {
        // Starts at beat 3 of bar 0, lasts 10 beats: ends at beat 13
        // (beat 1 of bar 3). Segments: [3,4), [4,8), [8,12), [12,13).
        let segments = split_at_bar_lines(note(3.0, 10.0), 4.0);
        let expected: Vec<(f64, f64)> = vec![(3.0, 1.0), (4.0, 4.0), (8.0, 4.0), (12.0, 1.0)];
        let actual: Vec<(f64, f64)> = segments
            .iter()
            .map(|s| (s.start_beat, s.duration_beats))
            .collect();
        assert_eq!(actual, expected);
        assert!(!segments[0].tied_from_previous);
        assert!(segments[1..].iter().all(|s| s.tied_from_previous));
    }

    #[test]
    fn split_at_bar_lines_preserves_midi() {
        let segments = split_at_bar_lines(
            NotationNote {
                start_beat: 2.0,
                duration_beats: 4.0,
                midi: 67,
                tied_from_previous: false,
            },
            4.0,
        );
        assert!(segments.iter().all(|s| s.midi == 67));
    }

    #[test]
    fn split_at_bar_lines_on_a_note_starting_exactly_on_a_bar_line_needs_no_split() {
        let segments = split_at_bar_lines(note(4.0, 4.0), 4.0);
        assert_eq!(segments, vec![note(4.0, 4.0)]);
    }
}
