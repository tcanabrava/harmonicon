// SPDX-License-Identifier: MIT

//! Rendering for a single notation note and its marks.

use super::*;
use bevy::ui_render::prelude::MaterialNode;

pub(super) fn spawn_note_glyphs(
    parent: &mut ChildSpawnerCommands,
    bravura: &BravuraFont,
    tie_material: &TieMaterialHandle,
    note: &NotationNote,
    tied_from: Option<&NotationNote>,
    now: f64,
    clef: Clef,
    stem: StemRole,
    accidental: Accidental,
    scale: f32,
) {
    let beam = stem.beam;
    let x = ((note.start_beat - now) * scale as f64) as f32;
    let step = staff_step(note.midi, clef);
    let rhythm = note_rhythm(note.duration_beats);
    let kind = rhythm.head;
    let notehead_y = y_for_step(step);
    // Which way the stem goes — or would go, for a stemless whole note. A
    // tie is drawn on the other side, clear of it.
    let stem_up = stem.stem_up;
    // A highlighted note's own marks take the highlight; a beam stays white,
    // since it belongs to its whole group rather than to this note.
    let ink = if note.highlighted { HIGHLIGHT_COLOR } else { Color::WHITE };

    // Which accidental (if any) is decided over the whole song by
    // `notation::accidentals`, since it depends on what the bar has already
    // said — not on this note alone.
    if let Some((mark, width_sp)) = match accidental {
        Accidental::Sharp => Some((glyph::ACCIDENTAL_SHARP, ACCIDENTAL_SHARP_WIDTH_SP)),
        Accidental::Natural => Some((glyph::ACCIDENTAL_NATURAL, ACCIDENTAL_NATURAL_WIDTH_SP)),
        Accidental::None => None,
    } {
        parent.spawn((
            super::glyph(
                bravura,
                mark,
                x - (width_sp + ACCIDENTAL_GAP_SP) * STAFF_LINE_SPACING,
                notehead_y,
                ink,
            ),
            MusicScoreNoteGlyph,
        ));
    }

    parent.spawn((super::glyph(bravura, kind.glyph(), x, notehead_y, ink), MusicScoreNoteGlyph));

    // Tie mark: a real curved arc (see `tie_material`'s own doc comment),
    // spanning the actual pixel gap from the previous segment's notehead
    // to this one's — not a fixed size, since that gap varies with how
    // long the previous (tied-from) segment was.
    if note.tied_from_previous
        && let Some(prev) = tied_from
    {
        let prev_x = ((prev.start_beat - now) * scale as f64) as f32;
        let prev_width = note_rhythm(prev.duration_beats).head.width_sp();
        let gap = TIE_END_GAP_SP * STAFF_LINE_SPACING;
        let left = prev_x + prev_width * STAFF_LINE_SPACING + gap;
        let width = (x - gap - left).max(TIE_MIN_WIDTH_PX);
        let height = TIE_ARC_HEIGHT_SP * STAFF_LINE_SPACING;
        let offset = TIE_GAP_SP * STAFF_LINE_SPACING;
        let (top, material) = match (stem_up, note.highlighted) {
            (true, false) => (notehead_y + offset, &tie_material.below),
            (true, true) => (notehead_y + offset, &tie_material.below_highlighted),
            (false, false) => (notehead_y - offset - height, &tie_material.above),
            (false, true) => (notehead_y - offset - height, &tie_material.above_highlighted),
        };
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(width),
                height: Val::Px(height),
                ..default()
            },
            MaterialNode(material.clone()),
            MusicScoreNoteGlyph,
        ));
    }

    // Augmentation dot — right of the notehead, always in a space.
    if rhythm.dots > 0 {
        parent.spawn((
            super::glyph(
                bravura,
                glyph::AUGMENTATION_DOT,
                x + (kind.width_sp() + DOT_GAP_SP) * STAFF_LINE_SPACING,
                y_for_step(dot_step(step)),
                ink,
            ),
            MusicScoreNoteGlyph,
        ));
    }

    // In a chord only one head draws the stem; the rest sit on it.
    if kind.has_stem() && stem.draws_stem {
        let (anchor_x_sp, anchor_y_sp) =
            if stem_up { STEM_UP_ANCHOR_SP } else { STEM_DOWN_ANCHOR_SP };
        let stem_x = x + anchor_x_sp * STAFF_LINE_SPACING;
        let stem_notehead_y = notehead_y - anchor_y_sp * STAFF_LINE_SPACING;
        // A beamed stem stops at its group's shared beam line instead of
        // its own default length — that common tip is what lets one
        // straight beam join them.
        // A chord's stem runs from this, its far head, past the head
        // nearest the tip (`reach_step`, this note's own for a single note).
        let stem_len_px = match beam {
            Some(b) => (stem_notehead_y - y_for_step(b.beam_step)).abs(),
            None => {
                (stem_notehead_y - y_for_step(stem.reach_step)).abs()
                    + STEM_LENGTH_SP * STAFF_LINE_SPACING
            }
        };
        let stem_top = if stem_up { stem_notehead_y - stem_len_px } else { stem_notehead_y };
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(stem_x - STEM_THICKNESS_SP * STAFF_LINE_SPACING * 0.5),
                top: Val::Px(stem_top),
                width: Val::Px(STEM_THICKNESS_SP * STAFF_LINE_SPACING),
                height: Val::Px(stem_len_px),
                ..default()
            },
            BackgroundColor(ink),
            MusicScoreNoteGlyph,
        ));

        // The first note of a beam group draws the beam itself, spanning
        // to the last stem in the group.
        if let Some(b) = beam.filter(|b| b.is_first) {
            let beam_y = y_for_step(b.beam_step);
            let width = (b.span_beats * scale as f64) as f32;
            // One beam per flag the group's notes would otherwise carry —
            // a second for sixteenths, stacked inward from the stem tip so
            // the outermost always sits at the tip itself.
            for i in 0..b.beams.max(1) {
                let offset = i as f32 * (BEAM_THICKNESS_PX + BEAM_GAP_PX);
                let top =
                    if b.stem_up { beam_y + offset } else { beam_y - BEAM_THICKNESS_PX - offset };
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
            let stem_tip_y = if stem_up { stem_top } else { stem_top + stem_len_px };
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
                super::glyph(bravura, flag_glyph, stem_x, stem_tip_y, ink),
                MusicScoreNoteGlyph,
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
