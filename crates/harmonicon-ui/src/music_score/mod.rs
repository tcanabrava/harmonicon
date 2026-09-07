// SPDX-License-Identifier: MIT

//! A scrolling music-notation staff, rendered with the
//! [Bravura](https://github.com/steinbergmedia/bravura) SMuFL font — shared
//! by the Song Editor and both gameplay render modes, all three of which
//! spawn [`spawn_music_score`] wherever their own layout calls for it and
//! drive it by writing [`MusicScoreNotes`]/[`MusicScorePlayhead`].
//!
//! Deliberately not full music engraving. It does draw: noteheads
//! (whole/half/filled by duration), stems, ledger lines, accidentals that
//! follow the bar rather than the note ([`accidentals`] — a sharp holds
//! for the rest of its bar, and a note cancelling one gets a natural),
//! ties across a bar line ([`split_at_bar_lines`]), bar lines
//! and a time signature ([`MusicScoreMeter`]), one of three clefs picked
//! from the music's own range ([`choose_clef`]), and beams joining short
//! notes within a beat ([`beam_groups`]).
//!
//! Durations round to the nearest standard value and are spelled with the
//! dots and flags that value actually takes ([`note_rhythm`]) — down to a
//! sixteenth, which is the shortest tier.
//!
//! It does not: slant a beam (they are horizontal, with every stem in a
//! group drawn to one shared line), draw a partial beam where a group
//! mixes durations (the whole group takes its shortest note's beam count),
//! spell anything shorter than a sixteenth or double-dotted, or change
//! clef or meter mid-piece — both are chosen once for the whole song,
//! because either changing under a moving playhead would be unreadable. This is a supplementary
//! visual, not a sight-reading tool (the Song Editor's own tab readout
//! already exists for players who want exact rhythm) — see
//! `docs/lessons_plan.md`'s framing of the tab readout for the same
//! reasoning applied here.
//!
//! Every glyph's relative geometry (notehead width, stem attachment point,
//! ledger-line extension) is taken directly from Bravura's own published
//! `bravura_metadata.json` (values in "staff spaces," SMuFL's own unit —
//! 1 staff space is the gap between two adjacent staff lines), not
//! estimated. The one thing that *couldn't* be derived that way:
//! [`GLYPH_BASELINE_CORRECTION`] — see its own doc comment for why, and
//! flag it as the first thing to adjust by eye if glyphs don't look
//! vertically centered on their staff position.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::ui_render::prelude::MaterialNode;

mod tie_material;
use tie_material::{TieMaterialHandle, TieMaterialPlugin};

mod notation;
use notation::glyph;
pub use notation::*;

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

/// Bevy positions a `Text` node's bounding-box top-left at the `Node`'s own
/// `top`/`left` — not the font's internal glyph baseline/origin
/// `bravura_metadata.json`'s coordinates are expressed relative to. There's
/// no published mapping between the two short of measuring Bravura's
/// OpenType vertical metrics against Bevy's text shaper (cosmic-text) at a
/// specific font size, so this constant is the correction — **estimated,
/// not measured** (no display to render against in this dev environment).
/// First thing to adjust by eye if glyphs don't look vertically centered
/// once actually visible; applied uniformly, so recalibrating is a
/// one-constant fix, not a hunt through per-glyph offsets.
const GLYPH_BASELINE_CORRECTION: f32 = GLYPH_FONT_PX * 0.5;

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
/// spans from one notehead to the next rather than a fixed size.
/// [`TIE_END_MARGIN_SP`] pulls each end in from its closest notehead,
/// clearing the glyph itself (splits the difference between `Filled`/
/// `Half`'s width and a wider `Whole`, rather than tracking each end's
/// own kind).
const TIE_END_MARGIN_SP: f32 = 1.3;
const TIE_GAP_SP: f32 = 0.15;
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
const PIXELS_PER_BEAT: f32 = 34.0;
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
fn visible_beats(panel_width_px: f32) -> (f64, f64) {
    let behind = (PLAYHEAD_X / PIXELS_PER_BEAT) as f64 + VISIBLE_BEATS_GRACE;
    let ahead = ((panel_width_px - PLAYHEAD_X).max(0.0) / PIXELS_PER_BEAT) as f64;
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

/// The staff's meter: what the time signature at the head reads, and how
/// often a bar line falls. Written by whichever bridge is driving the
/// staff; [`Default`] is 4/4, matching what every caller assumed back when
/// the module had no meter at all.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct MusicScoreMeter {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for MusicScoreMeter {
    fn default() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }
}

impl MusicScoreMeter {
    /// Quarter-note beats per bar — the unit [`NotationNote::start_beat`]
    /// counts in, so a 6/8 bar is 3 quarter-note beats, not 6.
    pub fn beats_per_bar(self) -> f64 {
        self.numerator as f64 * 4.0 / self.denominator.max(1) as f64
    }
}

/// Parses `"6/8"` into its two halves.
///
/// The denominator matters here and nowhere else yet: every other reader
/// of this field (`gameplay::bars::parse_beats`, both music-score bridges)
/// takes `split('/').next()` and throws it away, because all they wanted
/// was a bar length in quarter notes. A drawn time signature needs both
/// digits. Anything unparseable falls back to 4/4 rather than failing —
/// a malformed signature should cost a wrong staff head, not a crash.
pub fn parse_time_signature(s: &str) -> MusicScoreMeter {
    let mut parts = s.split('/');
    let numerator = parts.next().and_then(|n| n.trim().parse().ok());
    let denominator = parts.next().and_then(|d| d.trim().parse().ok());
    match (numerator, denominator) {
        (Some(n), Some(d)) if n > 0 && d > 0 => MusicScoreMeter {
            numerator: n,
            denominator: d,
        },
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

/// The (persistent) child of the panel that [`rebuild_score_notes`]
/// despawns and respawns the note glyphs under — everything *except* this
/// layer (the staff lines, the clef, the reference line) is spawned once
/// by [`spawn_music_score`] and never touched again.
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
            .add_systems(Startup, load_bravura_font)
            .add_systems(
                Update,
                rebuild_score_notes.run_if(
                    resource_changed::<MusicScoreNotes>
                        .or_else(resource_changed::<MusicScoreMeter>)
                        .or_else(resource_changed::<MusicScorePlayhead>)
                        .or_else(panel_width_changed),
                ),
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
/// layer [`rebuild_score_notes`] fills in every time [`MusicScoreNotes`]/
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
        // Shared with `gameplay::song_progress_overlay::spawn_song_
        // progress`'s own bar background — the two used to be two
        // independently-tuned near-blacks, which read as separate widgets
        // rather than one panel.
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
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(CLEF_X),
                top: Val::Px(y_for_step(clef.anchor_step()) - GLYPH_BASELINE_CORRECTION),
                ..default()
            },
            MusicScoreClef,
            Text::new(clef.glyph()),
            TextFont {
                font: FontSource::Handle(bravura.0.clone()),
                font_size: FontSize::Px(GLYPH_FONT_PX),
                ..default()
            },
            TextColor(Color::WHITE),
            crate::dialogs::font_fallback::SkipFontFallback,
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
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(TIME_SIG_X),
                    top: Val::Px(y_for_step(step) - GLYPH_BASELINE_CORRECTION),
                    ..default()
                },
                MusicScoreTimeSig { numerator },
                Text::new(time_sig_glyphs(digit)),
                TextFont {
                    font: FontSource::Handle(bravura.0.clone()),
                    font_size: FontSize::Px(GLYPH_FONT_PX),
                    ..default()
                },
                TextColor(Color::WHITE),
                crate::dialogs::font_fallback::SkipFontFallback,
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
        // Notes layer: positioned so its own local x=0 IS the playhead —
        // every note glyph spawned inside it is placed at
        // `(note.start_beat - now) * PIXELS_PER_BEAT`, which can (and
        // routinely does) go negative for a note just behind the
        // playhead; `overflow: clip_x()` on the panel itself keeps that
        // from spilling past the panel's own left edge.
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

fn rebuild_score_notes(
    mut commands: Commands,
    bravura: Option<Res<BravuraFont>>,
    tie_material: Option<Res<TieMaterialHandle>>,
    notes: Res<MusicScoreNotes>,
    playhead: Res<MusicScorePlayhead>,
    panels: Query<&ComputedNode, With<MusicScorePanel>>,
    layers: Query<Entity, With<MusicScoreNotesLayer>>,
    existing: Query<Entity, With<MusicScoreNoteGlyph>>,
    meter: Res<MusicScoreMeter>,
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
    // SPACING`, `PIXELS_PER_BEAT`, ...) feeds `Val::Px`, which is logical
    // px, so this needs `inverse_scale_factor()` to match — same
    // conversion `gameplay_2d::size_note_tails` already applies for the
    // same reason.
    let Some(panel_width) = panels
        .iter()
        .next()
        .map(|n| n.size().x * n.inverse_scale_factor())
    else {
        return;
    };
    // One clef for the whole song, from its own range — see `choose_clef`.
    let clef = choose_clef(&notes.0);
    for (mut text, mut node) in &mut clefs {
        let glyph = clef.glyph();
        if text.0 != glyph {
            text.0 = glyph.to_string();
            node.top = Val::Px(y_for_step(clef.anchor_step()) - GLYPH_BASELINE_CORRECTION);
        }
    }

    for (slot, mut text) in &mut time_sigs {
        let digit = if slot.numerator {
            meter.numerator
        } else {
            meter.denominator
        };
        let glyphs = time_sig_glyphs(digit);
        if text.0 != glyphs {
            text.0 = glyphs;
        }
    }

    // Over every note, not just the visible ones: a group that straddles
    // the window edge must still agree on one direction and beam line, and
    // an accidental's effect on the rest of its bar has to be known even
    // when the note that established it has already scrolled off.
    let beams = beam_groups(&notes.0, clef);
    let marks = accidentals(&notes.0, clef, meter.beats_per_bar());

    let (beats_behind, beats_ahead) = visible_beats(panel_width);
    for glyph in &existing {
        commands.entity(glyph).despawn();
    }

    let now = playhead.0;
    for layer in &layers {
        commands.entity(layer).with_children(|parent| {
            // `prev` tracks the immediately-preceding element of `notes.0`
            // regardless of visibility — `split_at_bar_lines` always
            // produces a split note's segments as consecutive entries, so
            // this is reliably "the segment `note` was tied from" whenever
            // `note.tied_from_previous` is set, even if that segment itself
            // scrolled out of the visible window and wasn't spawned.
            // Bar lines first, so a notehead always paints over one
            // rather than under it.
            for beat in bar_line_beats(now - beats_behind, now + beats_ahead, meter.beats_per_bar())
            {
                let x = ((beat - now) * PIXELS_PER_BEAT as f64) as f32;
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

            let mut prev: Option<&NotationNote> = None;
            for (i, note) in notes.0.iter().enumerate() {
                let visible = note.start_beat + note.duration_beats >= now - beats_behind
                    && note.start_beat <= now + beats_ahead;
                if visible {
                    spawn_note_glyphs(
                        parent,
                        &bravura,
                        &tie_material,
                        note,
                        prev,
                        now,
                        clef,
                        beams[i],
                        marks[i],
                    );
                }
                prev = Some(note);
            }
        });
    }
}

fn spawn_note_glyphs(
    parent: &mut ChildSpawnerCommands,
    bravura: &BravuraFont,
    tie_material: &TieMaterialHandle,
    note: &NotationNote,
    prev: Option<&NotationNote>,
    now: f64,
    clef: Clef,
    beam: Option<BeamPlacement>,
    accidental: Accidental,
) {
    let x = ((note.start_beat - now) * PIXELS_PER_BEAT as f64) as f32;
    let step = staff_step(note.midi, clef);
    let rhythm = note_rhythm(note.duration_beats);
    let kind = rhythm.head;
    let notehead_y = y_for_step(step);

    // Which accidental (if any) is decided over the whole song by
    // `notation::accidentals`, since it depends on what the bar has already
    // said — not on this note alone.
    if let Some((mark, width_sp)) = match accidental {
        Accidental::Sharp => Some((glyph::ACCIDENTAL_SHARP, ACCIDENTAL_SHARP_WIDTH_SP)),
        Accidental::Natural => Some((glyph::ACCIDENTAL_NATURAL, ACCIDENTAL_NATURAL_WIDTH_SP)),
        Accidental::None => None,
    } {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x - (width_sp + ACCIDENTAL_GAP_SP) * STAFF_LINE_SPACING),
                top: Val::Px(notehead_y - GLYPH_BASELINE_CORRECTION),
                ..default()
            },
            Text::new(mark),
            TextFont {
                font: FontSource::Handle(bravura.0.clone()),
                font_size: FontSize::Px(GLYPH_FONT_PX),
                ..default()
            },
            TextColor(Color::WHITE),
            MusicScoreNoteGlyph,
            crate::dialogs::font_fallback::SkipFontFallback,
        ));
    }

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(notehead_y - GLYPH_BASELINE_CORRECTION),
            ..default()
        },
        Text::new(kind.glyph()),
        TextFont {
            font: FontSource::Handle(bravura.0.clone()),
            font_size: FontSize::Px(GLYPH_FONT_PX),
            ..default()
        },
        TextColor(Color::WHITE),
        MusicScoreNoteGlyph,
        crate::dialogs::font_fallback::SkipFontFallback,
    ));

    // Tie mark: a real curved arc (see `tie_material`'s own doc comment),
    // spanning the actual pixel gap from the previous segment's notehead
    // to this one's — not a fixed size, since that gap varies with how
    // long the previous (tied-from) segment was.
    if note.tied_from_previous
        && let Some(prev) = prev
    {
        let prev_x = ((prev.start_beat - now) * PIXELS_PER_BEAT as f64) as f32;
        let margin = TIE_END_MARGIN_SP * STAFF_LINE_SPACING;
        let left = prev_x + margin;
        let width = (x - margin - left).max(TIE_MIN_WIDTH_PX);
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(notehead_y + TIE_GAP_SP * STAFF_LINE_SPACING),
                width: Val::Px(width),
                height: Val::Px(TIE_ARC_HEIGHT_SP * STAFF_LINE_SPACING),
                ..default()
            },
            MaterialNode(tie_material.0.clone()),
            MusicScoreNoteGlyph,
        ));
    }

    // Augmentation dot — right of the notehead, always in a space.
    if rhythm.dots > 0 {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x + (kind.width_sp() + DOT_GAP_SP) * STAFF_LINE_SPACING),
                top: Val::Px(y_for_step(dot_step(step)) - GLYPH_BASELINE_CORRECTION),
                ..default()
            },
            Text::new(glyph::AUGMENTATION_DOT),
            TextFont {
                font: FontSource::Handle(bravura.0.clone()),
                font_size: FontSize::Px(GLYPH_FONT_PX),
                ..default()
            },
            TextColor(Color::WHITE),
            MusicScoreNoteGlyph,
            crate::dialogs::font_fallback::SkipFontFallback,
        ));
    }

    if kind.has_stem() {
        let stem_up = beam.map_or(step < MIDDLE_LINE_STEP, |b| b.stem_up);
        let (anchor_x_sp, anchor_y_sp) = if stem_up {
            STEM_UP_ANCHOR_SP
        } else {
            STEM_DOWN_ANCHOR_SP
        };
        let stem_x = x + anchor_x_sp * STAFF_LINE_SPACING;
        let stem_notehead_y = notehead_y - anchor_y_sp * STAFF_LINE_SPACING;
        // A beamed stem stops at its group's shared beam line instead of
        // its own default length — that common tip is what lets one
        // straight beam join them.
        let stem_len_px = match beam {
            Some(b) => (stem_notehead_y - y_for_step(b.beam_step)).abs(),
            None => STEM_LENGTH_SP * STAFF_LINE_SPACING,
        };
        let stem_top = if stem_up {
            stem_notehead_y - stem_len_px
        } else {
            stem_notehead_y
        };
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(stem_x - STEM_THICKNESS_SP * STAFF_LINE_SPACING * 0.5),
                top: Val::Px(stem_top),
                width: Val::Px(STEM_THICKNESS_SP * STAFF_LINE_SPACING),
                height: Val::Px(stem_len_px),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            MusicScoreNoteGlyph,
        ));

        // The first note of a beam group draws the beam itself, spanning
        // to the last stem in the group.
        if let Some(b) = beam.filter(|b| b.is_first) {
            let beam_y = y_for_step(b.beam_step);
            let width = (b.span_beats * PIXELS_PER_BEAT as f64) as f32;
            // One beam per flag the group's notes would otherwise carry —
            // a second for sixteenths, stacked inward from the stem tip so
            // the outermost always sits at the tip itself.
            for i in 0..b.beams.max(1) {
                let offset = i as f32 * (BEAM_THICKNESS_PX + BEAM_GAP_PX);
                let top = if b.stem_up {
                    beam_y + offset
                } else {
                    beam_y - BEAM_THICKNESS_PX - offset
                };
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(stem_x - STEM_THICKNESS_SP * STAFF_LINE_SPACING * 0.5),
                        top: Val::Px(top),
                        width: Val::Px(width + STEM_THICKNESS_SP * STAFF_LINE_SPACING),
                        height: Val::Px(BEAM_THICKNESS_PX),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                    MusicScoreNoteGlyph,
                ));
            }
        }

        // A beamed note takes the beam instead of a flag — drawing both
        // would be a double rhythm marking.
        if beam.is_none() && rhythm.flags > 0 {
            // The flag attaches at the stem's tip, the end away from the
            // notehead — `stem_top` itself for an up-stem (the rect's own
            // top edge), or `stem_top + stem_len_px` for a down-stem (its
            // bottom edge). Both `flag8thUp`/`flag8thDown`'s own SMuFL
            // origin sits right at that same attachment point
            // (`glyphsWithAnchors.flag8thUp/Down`'s `stemUpNW`/
            // `stemDownSW`, both within 0.15 staff spaces of (0, 0)), so
            // no extra offset beyond the shared `GLYPH_BASELINE_
            // CORRECTION` every other glyph in this module already needs.
            let stem_tip_y = if stem_up {
                stem_top
            } else {
                stem_top + stem_len_px
            };
            // A sixteenth takes the two-flag glyph rather than two copies
            // of the eighth's — Bravura draws the pair as one shape with
            // the correct spacing between them.
            let flag_glyph = match (rhythm.flags, stem_up) {
                (1, true) => glyph::FLAG_8TH_UP,
                (1, false) => glyph::FLAG_8TH_DOWN,
                (_, true) => glyph::FLAG_16TH_UP,
                (_, false) => glyph::FLAG_16TH_DOWN,
            };
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(stem_x),
                    top: Val::Px(stem_tip_y - GLYPH_BASELINE_CORRECTION),
                    ..default()
                },
                Text::new(flag_glyph),
                TextFont {
                    font: FontSource::Handle(bravura.0.clone()),
                    font_size: FontSize::Px(GLYPH_FONT_PX),
                    ..default()
                },
                TextColor(Color::WHITE),
                MusicScoreNoteGlyph,
                crate::dialogs::font_fallback::SkipFontFallback,
            ));
        }
    }

    let ledger_width_px = (kind.width_sp() + 2.0 * LEDGER_EXTENSION_SP) * STAFF_LINE_SPACING;
    let ledger_left = x - LEDGER_EXTENSION_SP * STAFF_LINE_SPACING;
    for ledger_step in ledger_line_steps(step) {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ledger_left),
                top: Val::Px(y_for_step(ledger_step)),
                width: Val::Px(ledger_width_px),
                height: Val::Px(LEDGER_THICKNESS_SP * STAFF_LINE_SPACING),
                ..default()
            },
            BackgroundColor(Color::srgba(0.75, 0.75, 0.80, 0.6)),
            MusicScoreNoteGlyph,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These cover this half's own layout constants and the Bevy-bound
    // `MusicScoreMeter`; the pure notation maths is tested in `notation`.
    #[test]
    fn parse_time_signature_keeps_the_denominator_other_callers_discard() {
        let m = parse_time_signature("6/8");
        assert_eq!((m.numerator, m.denominator), (6, 8));
    }
    #[test]
    fn parse_time_signature_falls_back_to_four_four_when_malformed() {
        for bad in ["", "4", "x/y", "4/0", "0/4", "//"] {
            assert_eq!(
                parse_time_signature(bad),
                MusicScoreMeter::default(),
                "{bad:?}"
            );
        }
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
        let (_, narrow_ahead) = visible_beats(200.0);
        let (_, wide_ahead) = visible_beats(2000.0);
        assert!(
            wide_ahead > narrow_ahead * 5.0,
            "a much wider panel should show proportionally more beats ahead"
        );
    }
    #[test]
    fn visible_beats_ahead_never_goes_negative_for_a_panel_narrower_than_playhead_x() {
        let (_, ahead) = visible_beats(10.0); // narrower than PLAYHEAD_X itself
        assert!(ahead >= 0.0);
    }
    #[test]
    fn visible_beats_behind_is_independent_of_panel_width() {
        // The space behind the playhead is bounded by PLAYHEAD_X, which is
        // fixed — widening the panel only grows what's visible ahead.
        let (behind_narrow, _) = visible_beats(200.0);
        let (behind_wide, _) = visible_beats(2000.0);
        assert_eq!(behind_narrow, behind_wide);
    }
}
