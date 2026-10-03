// SPDX-License-Identifier: MIT

//! A scrolling music-notation staff, rendered with the
//! [Bravura](https://github.com/steinbergmedia/bravura) SMuFL font — shared
//! by the Song Editor and both gameplay render modes, all three of which
//! spawn [`spawn_music_score`] wherever their own layout calls for it and
//! drive it by writing [`MusicScoreNotes`]/[`MusicScorePlayhead`].
//!
//! Deliberately not full music engraving. It does draw: noteheads and rests
//! (whole/half/filled by duration), stems, ledger lines, accidentals that
//! follow the bar rather than the note ([`accidentals`] — a sharp holds
//! for the rest of its bar, and a note cancelling one gets a natural),
//! ties across bar lines and bar lines from [`MusicScoreBarMap`],
//! and a time signature ([`MusicScoreMeter`]), one of three clefs picked
//! from the music's own range ([`choose_clef`]), and beams joining short
//! notes within a beat, with a chord's notes on one stem ([`stem_roles`]).
//!
//! Durations round to the nearest standard value and are spelled with the
//! dots and flags that value actually takes ([`note_rhythm`]) — down to a
//! sixteenth, which is the shortest tier.
//!
//! It does not: slant a beam (they are horizontal, with every stem in a
//! group drawn to one shared line), draw a partial beam where a group
//! mixes durations (the whole group takes its shortest note's beam count),
//! spell anything shorter than a sixteenth or double-dotted, or change
//! clef or draw an inline time signature at a meter change. Bar lines do
//! follow the meter map. This is a supplementary
//! visual, not a sight-reading tool (the Song Editor's own tab readout
//! already exists for players who want exact rhythm) — see
//! `docs/lessons_plan.md`'s framing of the tab readout for the same
//! reasoning applied here.
//!
//! Every glyph's relative geometry (notehead width, stem attachment point,
//! ledger-line extension) is taken directly from Bravura's own published
//! `bravura_metadata.json` (values in "staff spaces," SMuFL's own unit —
//! 1 staff space is the gap between two adjacent staff lines), not
//! estimated. [`GLYPH_BASELINE_CORRECTION`], which maps a glyph's SMuFL
//! origin onto the top of the `Text` node Bevy places, comes from the
//! font's own vertical metrics and the pinned line height — see its doc
//! comment.

use bevy::prelude::*;
use bevy::text::{FontHinting, LineHeight};
use bevy::ui::ComputedNode;

mod note_glyphs;
mod tie_material;
use note_glyphs::spawn_note_glyphs;
use tie_material::{TieMaterialHandle, TieMaterialPlugin};

mod notation;
use notation::glyph;
pub use notation::*;

mod meter_map;
pub use meter_map::{BarPosition, MeterMap, MeterSegment};

mod rests;
pub use rests::{NotationRest, rests_between_notes};

// ── Layout constants ──────────────────────────────────────────────────────

/// Total height (px) of the score panel. `pub` so callers reserve this
/// much extra space below wherever they place it — the same "the overlay
/// paints on top of its own fixed footprint, callers pad around it"
/// pattern `gameplay::song_progress_overlay::BAR_HEIGHT` already
/// established.
pub const PANEL_HEIGHT: f32 = 120.0;

/// Pixel gap between two adjacent staff lines — the base unit everything
/// else in this module scales from. 1 "staff space" (SMuFL's own unit,
/// used throughout `bravura_metadata.json`) equals this many pixels,
/// by construction (see [`GLYPH_FONT_PX`]).
const STAFF_LINE_SPACING: f32 = 9.0;
/// Pixels per staff *step* (a line or a space is half a staff-space gap).
const STEP_PX: f32 = STAFF_LINE_SPACING / 2.0;
/// Distance from the panel's own top edge to the staff's top line (F5,
/// step 8) — chosen to leave headroom above for a handful of ledger
/// lines, since a harmonica's playable range routinely sits well outside
/// a single treble-clef staff.
const STAFF_TOP_MARGIN: f32 = 50.0;
/// SMuFL's own convention: a font's em size is set so that 4 staff spaces
/// equal 1 em ("staffSpace = 0.25 em") — this is what makes a glyph drawn
/// at this font size match a staff built from [`STAFF_LINE_SPACING`].
const GLYPH_FONT_PX: f32 = STAFF_LINE_SPACING * 4.0;

/// The line box every Bravura glyph is laid out in, in logical px — pinned
/// on each one (`LineHeight::Px`) because [`GLYPH_BASELINE_CORRECTION`] is
/// derived from it. Each also turns hinting off (`FontHinting::Disabled`;
/// UI text defaults it on), so an outline isn't snapped to the pixel grid
/// away from the exact position its stem and ledger lines are drawn at.
const GLYPH_LINE_HEIGHT_PX: f32 = 40.0;

/// How far below a `Text` node's top its glyphs' baseline sits — the SMuFL
/// origin every `bravura_metadata.json` coordinate is relative to, and the
/// point a notehead's staff position names.
///
/// Parley places the baseline at `ascent + floor((line_height - ascent -
/// descent) / 2)`, in physical pixels with ascent and descent each rounded
/// first. Bravura's ascent and descent are equal (`tests/glyph_coverage.rs`
/// pins that), so this is `floor(line_height / 2)` physical pixels: half
/// the line box, whatever the font's metrics. A fixed 40 px line makes that
/// a whole number of physical pixels at every common display scale (1,
/// 1.25, 1.5, 1.75, 2), so the baseline is exactly 20 px down at all of
/// them. A line height relative to the font size put it anywhere from 21 to
/// 21.6 depending on scale.
///
/// Stems, beams and ledger lines are plain rectangles placed from the same
/// staff positions, so they meet a glyph only if this is right.
const GLYPH_BASELINE_CORRECTION: f32 = GLYPH_LINE_HEIGHT_PX / 2.0;

/// A Bravura glyph with its SMuFL origin at `(left, origin_y)` px, drawn in
/// `ink`. Every Bravura `Text` is spawned through this, so none can miss the
/// pinned line height [`GLYPH_BASELINE_CORRECTION`] depends on, or the
/// `SkipFontFallback` opt-out that keeps the font fallback from replacing
/// its Private-Use-Area codepoints.
fn glyph(
    bravura: &BravuraFont,
    text: impl Into<String>,
    left: f32,
    origin_y: f32,
    ink: Color,
) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(origin_y - GLYPH_BASELINE_CORRECTION),
            ..default()
        },
        Text::new(text),
        TextFont {
            font: FontSource::Handle(bravura.0.clone()),
            font_size: FontSize::Px(GLYPH_FONT_PX),
            ..default()
        },
        LineHeight::Px(GLYPH_LINE_HEIGHT_PX),
        FontHinting::Disabled,
        TextColor(ink),
        crate::dialogs::font_fallback::SkipFontFallback,
    )
}

/// Notehead stem attachment points, in staff spaces relative to the
/// notehead's own origin (its bounding box's bottom-left corner) —
/// `bravura_metadata.json`'s `glyphsWithAnchors.noteheadBlack`
/// (`noteheadHalf` carries the same two anchors). Only `Filled`/`Half`
/// noteheads have a stem at all (see [`NoteheadKind::has_stem`]), and
/// both share these same anchors.
const STEM_UP_ANCHOR_SP: (f32, f32) = (1.18, 0.168);
const STEM_DOWN_ANCHOR_SP: (f32, f32) = (0.0, -0.168);
/// Standard engraving stem length, in staff spaces (3.5 staff spaces is
/// the conventional default length for an ordinary stem).
const STEM_LENGTH_SP: f32 = 3.5;
const STEM_THICKNESS_SP: f32 = 0.12; // engravingDefaults.stemThickness
/// engravingDefaults.beamThickness is 0.5 staff spaces.
const BEAM_THICKNESS_PX: f32 = 0.5 * STAFF_LINE_SPACING;
/// engravingDefaults.beamSpacing is 0.25 staff spaces.
const BEAM_GAP_PX: f32 = 0.25 * STAFF_LINE_SPACING;
/// Gap between a notehead's right edge and its augmentation dot.
const DOT_GAP_SP: f32 = 0.3;

/// How far a ledger line extends beyond the notehead on each side, and how
/// thick it is — both `bravura_metadata.json`'s own `engravingDefaults`
/// (`legerLineExtension`/`legerLineThickness`).
const LEDGER_EXTENSION_SP: f32 = 0.4;
const LEDGER_THICKNESS_SP: f32 = 0.16;

/// A tied-note connector: a real curved arc, drawn via
/// [`tie_material::TieMaterial`] (a `UiMaterial` fragment shader) rather
/// than a `bevy_ui` `Node`/`BackgroundColor` rectangle, which can only ever
/// be flat — SMuFL has no single tie codepoint (a real tie is a drawn
/// bezier arc, not a glyph, so there's nothing in `bravura_metadata.json`
/// to derive this from). [`TIE_GAP_SP`] is deliberately *not* a multiple
/// of 0.5 — staff lines/spaces sit at every 0.5 staff-space step, so a
/// half-integer gap would occasionally start the arc flush against a
/// staff line, visually merging with it. The arc's *width* isn't one of
/// these constants — [`spawn_note_glyphs`] derives it per tie from the
/// real pixel gap between the two tied noteheads' onset positions, so it
/// spans from one notehead to the next rather than a fixed size: it starts
/// [`TIE_END_GAP_SP`] past the first head's right edge (that head's own
/// width, so a whole note's wider head is cleared too) and ends the same
/// distance before the second head's left edge. [`TIE_GAP_SP`] is how far
/// from the heads' centre line its ends sit — below for stems up, above for
/// stems down, the side away from the stems.
const TIE_END_GAP_SP: f32 = 0.2;
const TIE_GAP_SP: f32 = 0.35;
const TIE_ARC_HEIGHT_SP: f32 = 0.7;
/// Never let the arc's own bounding box collapse to (near) nothing when
/// two tied segments' onsets land very close together in pixels.
const TIE_MIN_WIDTH_PX: f32 = 4.0;

/// `accidentalSharp`'s own bounding-box width (`glyphBBoxes.accidentalSharp`
/// — `bBoxNE.x - bBoxSW.x` = `0.996 - 0.0`), plus a small fixed gap before
/// the notehead it belongs to.
const ACCIDENTAL_SHARP_WIDTH_SP: f32 = 0.996;
/// Bravura's `accidentalNatural` is narrower than its sharp, so it is
/// placed on its own width rather than the sharp's — otherwise it would
/// float away from the notehead it belongs to. Transcribed from the font's
/// `glyphBBoxes` like the sharp above; the metadata file isn't bundled.
const ACCIDENTAL_NATURAL_WIDTH_SP: f32 = 0.664;
const ACCIDENTAL_GAP_SP: f32 = 0.2;

/// A highlighted note's colour — the Song Editor's selection. Gold like the
/// tie and the gameplay karaoke line's sung words, so it reads as "this
/// one" against the white of every other note.
const HIGHLIGHT_COLOR: Color = Color::srgb(0.98, 0.80, 0.25);

const CLEF_X: f32 = 8.0;
/// Where the time signature sits, clear of the clef glyph's own width.
const TIME_SIG_X: f32 = CLEF_X + 26.0;
/// The two digits straddle the middle line: numerator centred on the 4th
/// step, denominator on the 2nd, the conventional upper/lower placement.
const TIME_SIG_NUMERATOR_STEP: i32 = 6;
const TIME_SIG_DENOMINATOR_STEP: i32 = 2;
/// Where the "now" reference line sits, and where a note at the current
/// playhead position draws — notes scroll right to left through it, the
/// same "things move toward a fixed reference line" language the falling-
/// note highway and song-progress playhead already use elsewhere.
const PLAYHEAD_X: f32 = 56.0;
/// How wide a beat is drawn when a song has no reason to need more — and
/// the floor [`MusicScoreSpacing`] clamps to, so a sparse song is never
/// squeezed *tighter* than this.
const PIXELS_PER_BEAT: f32 = 34.0;
/// Limit the width required by very short notes and rests. The score scrolls,
/// so a readable sixteenth rest takes priority over showing extra bars.
const MAX_PIXELS_PER_BEAT: f32 = 128.0;
/// Extra trailing margin (beats) behind the playhead, on top of what the
/// panel's own on-screen space left of the reference line already fits —
/// so a just-played note doesn't vanish the instant its onset crosses the
/// reference line, only once it's fully done sounding.
const VISIBLE_BEATS_GRACE: f64 = 1.0;

/// How many beats of note fit within a panel that's `panel_width_px` wide
/// on screen, on each side of the "now" reference line — `(beats_behind,
/// beats_ahead)`. The score's visible window is sized to whatever the
/// panel can actually show on its own, independent of any other host UI
/// (the falling-note highway's own lookahead, the Song Editor grid's own
/// column count, ...): [`PLAYHEAD_X`] pixels of panel sit to the left of
/// the reference line, `panel_width_px - PLAYHEAD_X` to the right, and
/// [`rebuild_score_notes`] re-derives this every time the panel's own
/// rendered width changes (a resize), not just once.
fn visible_beats(panel_width_px: f32, pixels_per_beat: f32) -> (f64, f64) {
    let scale = pixels_per_beat.max(1.0);
    let behind = (PLAYHEAD_X / scale) as f64 + VISIBLE_BEATS_GRACE;
    let ahead = ((panel_width_px - PLAYHEAD_X).max(0.0) / scale) as f64;
    (behind, ahead)
}

fn y_for_step(step: i32) -> f32 {
    STAFF_TOP_MARGIN + (8 - step) as f32 * STEP_PX
}

// ── Resources ──────────────────────────────────────────────────────────────

/// The loaded Bravura font handle, `pub` so a caller's own setup system
/// can pass it into [`spawn_music_score`] (which needs it immediately, to
/// set the clef glyph's `TextFont`, rather than waiting a frame for
/// [`rebuild_score_notes`] to discover it).
#[derive(Resource, Clone)]
pub struct BravuraFont(pub Handle<Font>);

/// The chart's notes to draw, already converted to beat-based
/// [`NotationNote`]s by whichever caller populated this — see the module
/// doc comment for why the conversion happens at each call site instead of
/// here. Typically built once, at song/session setup, not every frame.
#[derive(Resource, Default)]
pub struct MusicScoreNotes(pub Vec<NotationNote>);

/// Bar grid for the same score as [`MusicScoreNotes`]. Both callers provide
/// their chart's meter map; gameplay uses the expanded performance chart.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct MusicScoreBarMap {
    pub meter: MeterMap,
    pub quarter_ticks: u32,
}

impl Default for MusicScoreBarMap {
    fn default() -> Self {
        Self { meter: MeterMap::constant("4/4", 12), quarter_ticks: 12 }
    }
}

impl MusicScoreBarMap {
    /// The bar boundaries in staff quarter-note beats. The first segment's
    /// phase puts a pickup at the end of its otherwise silent first bar.
    pub fn beats(&self, from: f64, to: f64) -> Vec<f64> {
        if self.quarter_ticks == 0 || to < from {
            return Vec::new();
        }
        let phase = self.meter.segments()[0].phase_ticks as f64;
        let q = self.quarter_ticks as f64;
        let start = ((from * q - phase).ceil().max(0.0)) as u64;
        let end = ((to * q - phase).floor().max(0.0)) as u64;
        self.meter
            .bar_starts(start, end.saturating_add(1))
            .into_iter()
            .filter(|&(tick, _)| tick > 0)
            .map(|(tick, _)| (tick as f64 + phase) / q)
            .collect()
    }

    /// Turns a tick-based note into staff segments, tying it at every bar
    /// boundary, including a meter change or a repeated passage boundary.
    pub fn split_note(
        &self,
        start: u64,
        end: u64,
        midi: u8,
        highlighted: bool,
    ) -> Vec<NotationNote> {
        let end = end.max(start.saturating_add(1));
        let phase = self.meter.segments()[0].phase_ticks as f64;
        let q = self.quarter_ticks.max(1) as f64;
        let mut boundaries = self.meter.bar_starts(start.saturating_add(1), end);
        boundaries.push((end, 0));
        let mut previous = start;
        boundaries
            .into_iter()
            .enumerate()
            .map(|(index, (tick, _))| {
                let note = NotationNote {
                    start_beat: (previous as f64 + phase) / q,
                    duration_beats: (tick - previous) as f64 / q,
                    midi,
                    tied_from_previous: index > 0,
                    highlighted,
                };
                previous = tick;
                note
            })
            .collect()
    }
}

#[cfg(test)]
mod bar_map_tests {
    use super::*;

    #[test]
    fn bar_lines_follow_meter_changes_and_do_not_duplicate_the_opening() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::new([(0, "4/4"), (48, "3/4")], 12),
            quarter_ticks: 12,
        };
        assert_eq!(bars.beats(0.0, 11.0), vec![4.0, 7.0, 10.0]);
    }

    #[test]
    fn pickup_and_window_edges_keep_the_bar_line_at_the_right_beat() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::with_pickup([(0, "4/4")], 12, 12),
            quarter_ticks: 12,
        };
        assert_eq!(bars.beats(0.0, 8.0), vec![4.0, 8.0]);
        assert_eq!(bars.beats(4.0, 4.0), vec![4.0]);
    }

    #[test]
    fn a_mid_bar_meter_change_is_a_line_and_a_tie_boundary() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::new([(0, "4/4"), (42, "3/4")], 12),
            quarter_ticks: 12,
        };
        assert_eq!(bars.beats(0.0, 7.0), vec![3.5, 6.5]);
        let parts = bars.split_note(36, 54, 60, false);
        assert_eq!(parts.iter().map(|n| n.start_beat).collect::<Vec<_>>(), vec![3.0, 3.5]);
        assert!(parts[1].tied_from_previous);
    }

    #[test]
    fn a_short_pause_widens_the_staff_until_its_rest_fits() {
        let bars = MusicScoreBarMap::default();
        let notes = vec![
            NotationNote {
                start_beat: 0.0,
                duration_beats: 1.0,
                midi: 60,
                tied_from_previous: false,
                highlighted: false,
            },
            NotationNote {
                start_beat: 1.25,
                duration_beats: 1.0,
                midi: 62,
                tied_from_previous: false,
                highlighted: true,
            },
        ];
        let rests = rests_between_notes(&notes, &bars);
        assert_eq!(rests.len(), 1);
        let scale = score_pixels_per_beat(&notes, &bars);
        assert!(scale > 68.0);
        assert!(rests[0].fits_slot(scale, STAFF_LINE_SPACING));
    }
}

/// How wide one beat is drawn, for the song currently loaded.
///
/// Derived once per song from its note density and rests rather than fixed,
/// because a piece of sixteenths or short pauses needs more room per beat
/// than a piece of quarters. Kept
/// in a resource, not recomputed inside [`rebuild_score_notes`], because
/// that runs on every playhead change (i.e. every frame) while this only
/// moves when the notes do.
#[derive(Resource)]
pub struct MusicScoreSpacing(pub f32);

impl Default for MusicScoreSpacing {
    fn default() -> Self {
        Self(PIXELS_PER_BEAT)
    }
}

/// Recomputes [`MusicScoreSpacing`] when notes or bar positions change.
/// Note density alone misses a short rest between otherwise sparse notes.
fn derive_score_spacing(
    notes: Res<MusicScoreNotes>,
    bar_map: Res<MusicScoreBarMap>,
    mut spacing: ResMut<MusicScoreSpacing>,
) {
    let derived = score_pixels_per_beat(&notes.0, &bar_map);
    // Written through `set_if_neq` semantics by hand: this system runs on
    // any note change, and a needless write would re-trigger the rebuild
    // that reads it.
    if spacing.0 != derived {
        spacing.0 = derived;
    }
}

fn score_pixels_per_beat(notes: &[NotationNote], bar_map: &MusicScoreBarMap) -> f32 {
    let note_scale =
        pixels_per_beat(notes, STAFF_LINE_SPACING, PIXELS_PER_BEAT, MAX_PIXELS_PER_BEAT);
    let rest_scale = rests_between_notes(notes, bar_map)
        .into_iter()
        .map(|rest| rest.required_pixels_per_beat(STAFF_LINE_SPACING))
        .fold(PIXELS_PER_BEAT, f32::max);
    note_scale.max(rest_scale.ceil()).min(MAX_PIXELS_PER_BEAT)
}

/// The staff's meter: what the time signature at the head reads. Bar lines
/// come from [`MusicScoreBarMap`]. Written by whichever bridge is driving the
/// staff; [`Default`] is 4/4, matching what every caller assumed back when
/// the module had no meter at all.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct MusicScoreMeter {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for MusicScoreMeter {
    fn default() -> Self {
        Self { numerator: 4, denominator: 4 }
    }
}

impl MusicScoreMeter {
    /// Quarter-note beats per bar — the unit [`NotationNote::start_beat`]
    /// counts in, so a 6/8 bar is 3 quarter-note beats, not 6.
    pub fn beats_per_bar(self) -> f64 {
        self.numerator as f64 * 4.0 / self.denominator.max(1) as f64
    }

    /// Ticks spanned by one beat *of this meter* — the unit its upper
    /// number counts, so an eighth in 7/8 and a quarter in 4/4 — given
    /// `quarter_ticks` ticks to a quarter note.
    ///
    /// Exact or nothing, unlike [`beats_per_bar`](Self::beats_per_bar),
    /// whose `f64` the Song Editor had to round to a whole number of
    /// quarters: a 7/8 bar is 3.5 of them, so rounding put its bar line
    /// half a beat out. `None` when the meter's beat isn't a whole number
    /// of ticks (a 32nd-note beat at 12 ticks to the quarter would be 1.5),
    /// which is why [`TIME_SIGNATURES`] stops at /16.
    pub fn ticks_per_beat(self, quarter_ticks: u32) -> Option<u32> {
        let whole_note = quarter_ticks.checked_mul(4)?;
        let denominator = u32::from(self.denominator);
        (denominator > 0 && whole_note.is_multiple_of(denominator))
            .then(|| whole_note / denominator)
    }

    /// Ticks spanned by one bar. `None` under the same conditions as
    /// [`ticks_per_beat`](Self::ticks_per_beat).
    pub fn ticks_per_bar(self, quarter_ticks: u32) -> Option<u32> {
        self.ticks_per_beat(quarter_ticks)?.checked_mul(u32::from(self.numerator))
    }

    /// Seconds spanned by one beat *of this meter* at `bpm` quarter notes a
    /// minute — an eighth's worth in 7/8, a quarter's in 4/4, a half's in
    /// 2/2.
    ///
    /// `bpm` counts quarter notes because that's what a chart's `tempo_bpm`
    /// means everywhere else (`harmonicon_core::chart::tick_to_seconds`
    /// converts `ticks / resolution` quarters at `60 / bpm` seconds each).
    /// This is the one place a metronome should get its beat length from:
    /// a click driven by `60 / bpm` directly clicks quarters whatever the
    /// meter, and a 3/8 bar is one and a half of those — its downbeat
    /// never lands on a click at all.
    pub fn beat_secs(self, bpm: f64) -> f64 {
        60.0 / bpm.max(f64::EPSILON) * 4.0 / f64::from(self.denominator.max(1))
    }

    /// Seconds spanned by one bar at `bpm` quarter notes a minute.
    pub fn bar_secs(self, bpm: f64) -> f64 {
        self.beat_secs(bpm) * f64::from(self.numerator)
    }
}

/// The meters the Song Editor offers, commonest first.
///
/// A fixed list rather than a free-text field because a time signature is
/// not two arbitrary numbers: the lower one names a *note value*, and note
/// values come from halving a whole note (1, 2, 4, 8, 16, …), so only a
/// power of two can appear there. Typing `4/3` asks for four thirds of a
/// whole note, which standard notation has no symbol for — and since
/// [`parse_time_signature`] only rejects what won't parse at all, such a
/// string would be silently rounded into a bar length that matches nothing.
/// Offering the real ones makes that unrepresentable.
///
/// Not exhaustive, and not meant to be — an exhaustive list is every
/// numerator crossed with five denominators, far too many to pick from.
/// `meters_are_all_well_formed` is what keeps this honest.
pub const TIME_SIGNATURES: [&str; 10] =
    ["4/4", "3/4", "2/4", "2/2", "6/8", "9/8", "12/8", "5/4", "7/8", "5/8"];

/// Parses `"6/8"` into its two halves — **the only place in the tree that
/// reads a time-signature string.** Everything that needs a bar length,
/// beat count or beat duration asks the returned [`MusicScoreMeter`]
/// rather than parsing the string itself: `gameplay::bars::chart_meter`
/// for a chart, `EditorState::meter` for the editor. Taking the numerator
/// alone as a beat count would give six for a 6/8 bar that is three
/// quarters long.
///
/// Anything unparseable falls back to 4/4 rather than failing — a chart
/// already on disk with a malformed signature should still open, costing
/// a wrong staff head rather than a crash. What a *user* can enter is
/// constrained upstream by [`TIME_SIGNATURES`].
pub fn parse_time_signature(s: &str) -> MusicScoreMeter {
    let mut parts = s.split('/');
    let numerator = parts.next().and_then(|n| n.trim().parse().ok());
    let denominator = parts.next().and_then(|d| d.trim().parse().ok());
    match (numerator, denominator) {
        (Some(n), Some(d)) if n > 0 && d > 0 => MusicScoreMeter { numerator: n, denominator: d },
        _ => MusicScoreMeter::default(),
    }
}

/// The current "now" position, in the same beat units as
/// [`MusicScoreNotes`] — updated every frame by whichever caller is
/// currently active (the Song Editor's own playhead, or the gameplay
/// clock converted through the chart's tempo map).
#[derive(Resource, Default)]
pub struct MusicScorePlayhead(pub f64);

/// Tags the panel's own root node, so [`rebuild_score_notes`] can read its
/// current rendered width (via `ComputedNode`) to size the visible window —
/// see [`visible_beats`].
#[derive(Component)]
struct MusicScorePanel;

/// The (persistent) child of the panel that holds the note glyphs.
/// [`rebuild_score_notes`] slides it as the playhead moves and respawns
/// its glyphs only when they run out — everything *except* this layer (the
/// staff lines, the clef, the reference line) is spawned once by
/// [`spawn_music_score`] and never moved.
#[derive(Component)]
struct MusicScoreNotesLayer;

/// The clef glyph, so [`rebuild_score_notes`] can swap it when
/// [`choose_clef`] picks a different one for the newly loaded notes.
#[derive(Component)]
struct MusicScoreClef;

/// The two time-signature digits at the staff head, updated alongside the
/// clef. `numerator: true` is the upper digit.
#[derive(Component)]
struct MusicScoreTimeSig {
    numerator: bool,
}

/// Tags every entity [`rebuild_score_notes`] spawns, so the next rebuild
/// knows what to despawn first.
#[derive(Component)]
struct MusicScoreNoteGlyph;

// ── Plugin ───────────────────────────────────────────────────────────────

pub struct MusicScorePlugin;

impl Plugin for MusicScorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TieMaterialPlugin)
            .init_resource::<MusicScoreNotes>()
            .init_resource::<MusicScorePlayhead>()
            .init_resource::<MusicScoreMeter>()
            .init_resource::<MusicScoreBarMap>()
            .init_resource::<MusicScoreSpacing>()
            .add_systems(Startup, load_bravura_font)
            .add_systems(
                Update,
                derive_score_spacing.run_if(
                    resource_changed::<MusicScoreNotes>
                        .or_else(resource_changed::<MusicScoreBarMap>),
                ),
            )
            .add_systems(
                Update,
                rebuild_score_notes
                    .run_if(
                        resource_changed::<MusicScoreNotes>
                            .or_else(resource_changed::<MusicScoreMeter>)
                            .or_else(resource_changed::<MusicScoreBarMap>)
                            .or_else(resource_changed::<MusicScorePlayhead>)
                            .or_else(resource_changed::<MusicScoreSpacing>)
                            .or_else(panel_width_changed),
                    )
                    .after(derive_score_spacing),
            );
    }
}

fn load_bravura_font(mut fonts: ResMut<Assets<Font>>, mut commands: Commands) {
    const BYTES: &[u8] = include_bytes!("../../../../assets/fonts/Bravura.otf");
    commands.insert_resource(BravuraFont(fonts.add(Font::from_bytes(BYTES.to_vec()))));
}

// ── Spawning ───────────────────────────────────────────────────────────────

/// Spawns the persistent part of the score panel — background, the 5
/// staff lines, the clef, the "now" reference line, and an empty notes
/// layer [`rebuild_score_notes`] fills and scrolls as [`MusicScoreNotes`]/
/// [`MusicScorePlayhead`] change. The panel carries no assumption about
/// where it sits on screen beyond its own fixed [`PANEL_HEIGHT`] — each
/// caller places it in their own layout (below `song_progress_overlay`'s
/// bar in gameplay; wherever fits in the Song Editor's own chrome).
pub fn spawn_music_score(parent: &mut ChildSpawnerCommands, bravura: &BravuraFont) -> Entity {
    let mut root = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(PANEL_HEIGHT),
            overflow: Overflow::clip_x(),
            // A top border rather than a separate divider node — this
            // panel is its own top-level entity (see `gameplay_2d::
            // spawn_gameplay_music_score`), not a sibling row inside the
            // song-progress bar it sits directly below, so there's no
            // shared parent to slot a divider `Node` into between the two.
            border: UiRect::top(Val::Px(1.0)),
            ..default()
        },
        // Shared with the song-progress bar, so the two read as one panel.
        BackgroundColor(harmonicon_platform::theme::HUD_PANEL_BG),
        BorderColor::all(harmonicon_platform::theme::HUD_DIVIDER_COLOR),
        MusicScorePanel,
    ));
    let root_id = root.id();
    root.with_children(|panel| {
        // The 5 staff lines: steps 0, 2, 4, 6, 8.
        for line in 0..5 {
            let step = line * 2;
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(y_for_step(step)),
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.75, 0.75, 0.80, 0.6)),
            ));
        }
        // Clef — its glyph's own SMuFL origin sits on the line it names
        // (see `Clef::anchor_step`). Spawned as treble and re-pointed by
        // `rebuild_score_notes` once there are notes to judge the range by.
        let clef = Clef::default();
        panel.spawn((
            glyph(bravura, clef.glyph(), CLEF_X, y_for_step(clef.anchor_step()), Color::WHITE),
            MusicScoreClef,
        ));
        // Time signature, beside the clef. Both digits are re-texted by
        // `rebuild_score_notes` when the meter changes; 4/4 to begin with,
        // matching `MusicScoreMeter::default`.
        let meter = MusicScoreMeter::default();
        for (numerator, step, digit) in [
            (true, TIME_SIG_NUMERATOR_STEP, meter.numerator),
            (false, TIME_SIG_DENOMINATOR_STEP, meter.denominator),
        ] {
            panel.spawn((
                glyph(bravura, time_sig_glyphs(digit), TIME_SIG_X, y_for_step(step), Color::WHITE),
                MusicScoreTimeSig { numerator },
            ));
        }
        // "Now" reference line — notes scroll toward/through this the same
        // way the falling-note highway approaches its own hit line.
        panel.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PLAYHEAD_X),
                top: Val::Px(0.0),
                width: Val::Px(1.0),
                height: Val::Px(PANEL_HEIGHT),
                ..default()
            },
            BackgroundColor(Color::srgba(0.95, 0.80, 0.35, 0.5)),
        ));
        // Notes layer: its local x=0 is the beat the glyphs were last
        // spawned around, and `rebuild_score_notes` slides it so that beat
        // tracks the playhead. Glyphs sit at `(note.start_beat - origin) *
        // MusicScoreSpacing`, which routinely goes negative or runs past
        // the right edge (a panel-width margin is spawned on each side);
        // `overflow: clip_x()` on the panel itself clips both.
        panel.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PLAYHEAD_X),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            MusicScoreNotesLayer,
        ));
    });
    root_id
}

/// `run_if` gate: whether the panel's own on-screen width just changed
/// (first layout pass after spawn, or a window resize) — see
/// [`visible_beats`]. `Changed<ComputedNode>` fires for both, since Bevy's
/// UI layout system writes a fresh `ComputedNode` whenever a node's
/// computed size changes, insertion included.
fn panel_width_changed(panel: Query<(), (With<MusicScorePanel>, Changed<ComputedNode>)>) -> bool {
    !panel.is_empty()
}

/// What [`rebuild_score_notes`] last spawned glyphs for: which notes layer,
/// at what panel width, and the beat span covered. Glyphs are placed
/// relative to `origin` (the playhead at that rebuild), so while the
/// playhead stays inside `lo..hi` only the layer needs to move.
struct SpawnedWindow {
    layer: Entity,
    panel_width: f32,
    origin: f64,
    lo: f64,
    hi: f64,
}

/// Keeps the notes layer showing the current playhead. The playhead moves
/// every frame during playback, so this does *not* rebuild on every move:
/// glyphs are spawned one panel-width beyond each visible edge, relative to
/// a fixed origin, and an ordinary playhead change just slides the layer.
/// A rebuild (despawn and respawn every glyph, recompute clef, beams and
/// accidentals) happens only when the notes, meter, spacing, panel width or
/// layer change, or when the playhead scrolls out of the spawned span.
fn rebuild_score_notes(
    mut commands: Commands,
    mut window: Local<Option<SpawnedWindow>>,
    bravura: Option<Res<BravuraFont>>,
    tie_material: Option<Res<TieMaterialHandle>>,
    notes: Res<MusicScoreNotes>,
    playhead: Res<MusicScorePlayhead>,
    panels: Query<&ComputedNode, With<MusicScorePanel>>,
    mut layers: Query<(Entity, &mut Node), (With<MusicScoreNotesLayer>, Without<MusicScoreClef>)>,
    existing: Query<Entity, With<MusicScoreNoteGlyph>>,
    meter: Res<MusicScoreMeter>,
    bar_map: Res<MusicScoreBarMap>,
    spacing: Res<MusicScoreSpacing>,
    mut clefs: Query<(&mut Text, &mut Node), With<MusicScoreClef>>,
    mut time_sigs: Query<(&MusicScoreTimeSig, &mut Text), Without<MusicScoreClef>>,
) {
    let Some(bravura) = bravura else { return };
    let Some(tie_material) = tie_material else {
        return;
    };
    // No `ComputedNode` yet means the panel hasn't been through a layout
    // pass at all (the very first frame after spawn) — nothing to size the
    // window against yet, so skip this pass; `panel_width_changed` fires
    // again the moment layout catches up and gives it one. `ComputedNode`
    // sizes are physical px; every length in this module (`STAFF_LINE_
    // SPACING`, `MusicScoreSpacing`, ...) feeds `Val::Px`, which is logical
    // px, so this needs `inverse_scale_factor()` to match — same
    // conversion `gameplay_2d::size_note_ribbons` already applies for the
    // same reason.
    let Some(panel_width) = panels.iter().next().map(|n| n.size().x * n.inverse_scale_factor())
    else {
        return;
    };
    let Some((layer, _)) = layers.iter().next() else {
        return;
    };
    let scale = spacing.0;
    let now = playhead.0;
    let (beats_behind, beats_ahead) = visible_beats(panel_width, scale);

    let covered = window.as_ref().is_some_and(|w| {
        w.layer == layer
            && w.panel_width == panel_width
            && now - beats_behind >= w.lo
            && now + beats_ahead <= w.hi
    });
    if !covered
        || notes.is_changed()
        || meter.is_changed()
        || bar_map.is_changed()
        || spacing.is_changed()
    {
        let span = beats_behind + beats_ahead;
        let lo = now - beats_behind - span;
        let hi = now + beats_ahead + span;
        spawn_window(
            &mut commands,
            &bravura,
            &tie_material,
            &notes.0,
            &meter,
            &bar_map,
            scale,
            layer,
            now,
            lo,
            hi,
            &existing,
            &mut clefs,
            &mut time_sigs,
        );
        *window = Some(SpawnedWindow { layer, panel_width, origin: now, lo, hi });
    }

    let Some(origin) = window.as_ref().map(|w| w.origin) else {
        return;
    };
    // Local x=0 of the layer is `origin`; shifting it by how far the
    // playhead has moved since keeps the reference line on "now".
    let left = Val::Px(PLAYHEAD_X + ((origin - now) * scale as f64) as f32);
    for (_, mut node) in &mut layers {
        if node.left != left {
            node.left = left;
        }
    }
}

/// Despawns the previous glyphs and spawns every glyph in `lo..hi` beats,
/// positioned relative to `origin`, after re-pointing the clef and time
/// signature.
fn spawn_window(
    commands: &mut Commands,
    bravura: &BravuraFont,
    tie_material: &TieMaterialHandle,
    notes: &[NotationNote],
    meter: &MusicScoreMeter,
    bar_map: &MusicScoreBarMap,
    scale: f32,
    layer: Entity,
    origin: f64,
    lo: f64,
    hi: f64,
    existing: &Query<Entity, With<MusicScoreNoteGlyph>>,
    clefs: &mut Query<(&mut Text, &mut Node), With<MusicScoreClef>>,
    time_sigs: &mut Query<(&MusicScoreTimeSig, &mut Text), Without<MusicScoreClef>>,
) {
    // One clef for the whole song, from its own range — see `choose_clef`.
    let clef = choose_clef(notes);
    for (mut text, mut node) in clefs.iter_mut() {
        let glyph = clef.glyph();
        if text.0 != glyph {
            text.0 = glyph.to_string();
            node.top = Val::Px(y_for_step(clef.anchor_step()) - GLYPH_BASELINE_CORRECTION);
        }
    }

    for (slot, mut text) in time_sigs.iter_mut() {
        let digit = if slot.numerator { meter.numerator } else { meter.denominator };
        let glyphs = time_sig_glyphs(digit);
        if text.0 != glyphs {
            text.0 = glyphs;
        }
    }

    // Over every note, not just the visible ones: a group that straddles
    // the window edge must still agree on one direction and beam line, and
    // an accidental's effect on the rest of its bar has to be known even
    // when the note that established it has already scrolled off.
    let stems = stem_roles(notes, clef);
    let marks = accidentals(notes, clef, meter.beats_per_bar());

    for glyph in existing {
        commands.entity(glyph).despawn();
    }

    commands.entity(layer).with_children(|parent| {
        // Bar lines first, so a notehead always paints over one rather than
        // under it.
        for beat in bar_map.beats(lo, hi) {
            // Leave a small gap before a downbeat notehead: at the same x
            // the line appears to run through the first note of the bar.
            let x = ((beat - origin) * scale as f64) as f32 - STAFF_LINE_SPACING;
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(y_for_step(8)),
                    width: Val::Px(1.0),
                    // Top line to bottom line: 8 steps, i.e. the four
                    // spaces between the five staff lines.
                    height: Val::Px(8.0 * STEP_PX),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.75, 0.75, 0.80, 0.45)),
                MusicScoreNoteGlyph,
            ));
        }

        for rest in rests_between_notes(notes, bar_map) {
            if rest.start_beat > hi || rest.start_beat + rest.duration_beats < lo {
                continue;
            }
            if !rest.fits_slot(scale, STAFF_LINE_SPACING) {
                continue;
            }
            let x = ((rest.start_beat - origin) * scale as f64) as f32;
            parent.spawn((
                glyph(bravura, rest.glyph(), x, y_for_step(rest.staff_step()), Color::WHITE),
                MusicScoreNoteGlyph,
            ));
            if rest.dots > 0 {
                parent.spawn((
                    glyph(
                        bravura,
                        glyph::AUGMENTATION_DOT,
                        x + 1.5 * STAFF_LINE_SPACING,
                        y_for_step(5),
                        Color::WHITE,
                    ),
                    MusicScoreNoteGlyph,
                ));
            }
        }

        for (i, note) in notes.iter().enumerate() {
            let visible = note.start_beat + note.duration_beats >= lo && note.start_beat <= hi;
            if visible {
                // Looked up over every note, not just the visible ones: the
                // segment a tie starts from may have scrolled off already.
                let tied_from = note.tied_from_previous.then(|| tied_from(notes, i)).flatten();
                spawn_note_glyphs(
                    parent,
                    bravura,
                    tie_material,
                    note,
                    tied_from,
                    origin,
                    clef,
                    stems[i],
                    marks[i],
                    scale,
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // These cover this half's own layout constants and the Bevy-bound
    // `MusicScoreMeter`; the pure notation maths is tested in `notation`.
    #[test]
    fn the_glyph_baseline_is_a_whole_physical_pixel_at_common_scales() {
        // Parley puts the baseline at floor(line box / 2) physical pixels
        // for a font whose ascent equals its descent. Where that half is
        // fractional the floor drops it, and glyphs drift from the
        // rectangles drawn at exact staff positions.
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let physical = GLYPH_LINE_HEIGHT_PX * scale / 2.0;
            assert_eq!(physical.fract(), 0.0, "scale {scale}");
            assert_eq!(physical.floor() / scale, GLYPH_BASELINE_CORRECTION);
        }
    }
    #[test]
    fn parse_time_signature_keeps_the_denominator_other_callers_discard() {
        let m = parse_time_signature("6/8");
        assert_eq!((m.numerator, m.denominator), (6, 8));
    }
    #[test]
    fn parse_time_signature_falls_back_to_four_four_when_malformed() {
        for bad in ["", "4", "x/y", "4/0", "0/4", "//"] {
            assert_eq!(parse_time_signature(bad), MusicScoreMeter::default(), "{bad:?}");
        }
    }
    #[test]
    fn meters_are_all_well_formed() {
        for s in TIME_SIGNATURES {
            let m = parse_time_signature(s);
            assert_ne!((m.numerator, m.denominator), (0, 0), "{s} did not parse at all");
            assert!(m.numerator > 0, "{s} has no beats in a bar");
            assert!(
                m.denominator.is_power_of_two(),
                "{s}: the lower number must name a note value, so a power of two"
            );
            assert!(m.denominator <= 16, "{s} is finer than the editor's tick grid can place");
            assert_eq!(
                format!("{}/{}", m.numerator, m.denominator),
                s,
                "{s} does not round-trip, so a picked value would not match"
            );
        }
    }

    #[test]
    fn every_offered_meter_has_an_exact_tick_length() {
        // The whole point of the picker's list: at the editor's 12 ticks to
        // a quarter, every meter it offers divides evenly, so no bar
        // boundary ever has to be rounded.
        for s in TIME_SIGNATURES {
            let m = parse_time_signature(s);
            assert!(m.ticks_per_beat(12).is_some(), "{s}: beat is not a whole number of ticks");
            assert!(m.ticks_per_bar(12).is_some(), "{s}: bar is not either");
        }
    }

    #[test]
    fn tick_lengths_follow_the_note_value_the_lower_number_names() {
        // 12 ticks to a quarter: a 4/4 beat is a quarter (12), a 7/8 beat
        // an eighth (6), a 2/2 beat a half (24).
        let tpb = |s: &str| parse_time_signature(s).ticks_per_beat(12);
        assert_eq!(tpb("4/4"), Some(12));
        assert_eq!(tpb("7/8"), Some(6));
        assert_eq!(tpb("2/2"), Some(24));
        // And a bar is that times the upper number — 7/8 is 42 ticks, which
        // is 3.5 quarters, exactly the half-beat `beats_per_bar` had to
        // round away.
        assert_eq!(parse_time_signature("7/8").ticks_per_bar(12), Some(42));
        assert_eq!(parse_time_signature("4/4").ticks_per_bar(12), Some(48));
        assert_eq!(parse_time_signature("5/8").ticks_per_bar(12), Some(30));
    }

    #[test]
    fn a_beat_finer_than_the_tick_grid_has_no_exact_length() {
        // 32nd notes at 12 ticks to a quarter would be 1.5 ticks.
        assert_eq!(parse_time_signature("4/32").ticks_per_beat(12), None);
        assert_eq!(parse_time_signature("4/32").ticks_per_bar(12), None);
        // A coarser grid can express them.
        assert_eq!(parse_time_signature("4/32").ticks_per_beat(8), Some(1));
    }

    #[test]
    fn beat_secs_is_the_meters_own_beat_not_a_quarter() {
        // 120 quarters a minute: a quarter is 0.5 s.
        let q = 0.5;
        assert!((parse_time_signature("4/4").beat_secs(120.0) - q).abs() < 1e-9);
        // In 6/8 the beat is an eighth — half that.
        assert!((parse_time_signature("6/8").beat_secs(120.0) - q / 2.0).abs() < 1e-9);
        // In 2/2 it's a half — twice.
        assert!((parse_time_signature("2/2").beat_secs(120.0) - q * 2.0).abs() < 1e-9);
    }

    #[test]
    fn bar_secs_matches_the_tick_length_of_the_bar() {
        // The two exact representations must agree: a bar's seconds at
        // `bpm` equals its ticks at `60 / bpm` seconds per quarter.
        for s in TIME_SIGNATURES {
            let m = parse_time_signature(s);
            let via_ticks = f64::from(m.ticks_per_bar(12).unwrap()) / 12.0 * (60.0 / 180.0);
            assert!(
                (m.bar_secs(180.0) - via_ticks).abs() < 1e-9,
                "{s}: bar_secs {} vs ticks {via_ticks}",
                m.bar_secs(180.0)
            );
        }
        // The two bundled odd-meter charts, concretely.
        assert!((parse_time_signature("6/8").bar_secs(180.0) - 1.0).abs() < 1e-9);
        assert!((parse_time_signature("3/8").bar_secs(120.0) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn beat_secs_survives_a_zero_bpm() {
        assert!(parse_time_signature("4/4").beat_secs(0.0).is_finite());
    }

    #[test]
    fn the_meter_list_has_no_duplicates() {
        let mut seen: Vec<&str> = TIME_SIGNATURES.to_vec();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "duplicate entry in TIME_SIGNATURES");
    }

    #[test]
    fn beats_per_bar_counts_quarter_notes_not_signature_beats() {
        // A 6/8 bar is six *eighths* — three quarter-note beats, which is
        // the unit NotationNote::start_beat is in.
        assert_eq!(parse_time_signature("6/8").beats_per_bar(), 3.0);
        assert_eq!(parse_time_signature("4/4").beats_per_bar(), 4.0);
        assert_eq!(parse_time_signature("3/4").beats_per_bar(), 3.0);
    }
    #[test]
    fn visible_beats_ahead_scales_with_panel_width() {
        let (_, narrow_ahead) = visible_beats(200.0, PIXELS_PER_BEAT);
        let (_, wide_ahead) = visible_beats(2000.0, PIXELS_PER_BEAT);
        assert!(
            wide_ahead > narrow_ahead * 5.0,
            "a much wider panel should show proportionally more beats ahead"
        );
    }
    #[test]
    fn visible_beats_ahead_never_goes_negative_for_a_panel_narrower_than_playhead_x() {
        let (_, ahead) = visible_beats(10.0, PIXELS_PER_BEAT); // narrower than PLAYHEAD_X itself
        assert!(ahead >= 0.0);
    }
    #[test]
    fn visible_beats_behind_is_independent_of_panel_width() {
        // The space behind the playhead is bounded by PLAYHEAD_X, which is
        // fixed — widening the panel only grows what's visible ahead.
        let (behind_narrow, _) = visible_beats(200.0, PIXELS_PER_BEAT);
        let (behind_wide, _) = visible_beats(2000.0, PIXELS_PER_BEAT);
        assert_eq!(behind_narrow, behind_wide);
    }
}
