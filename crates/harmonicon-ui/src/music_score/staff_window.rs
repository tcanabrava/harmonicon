// SPDX-License-Identifier: MIT

//! Spawning one staff's visible window of notation: noteheads, stems,
//! beams, ledger lines, ties, rests, clef and time signature for the beats
//! [`super::spawn_window`] asks for, on the treble or bass staff.

use super::*;

pub(super) fn spawn_staff_window(
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
    clefs: &mut Query<(&mut Text, &mut Node), With<MusicScoreClef>>,
    time_sigs: &mut Query<(&MusicScoreTimeSig, &mut Text), Without<MusicScoreClef>>,
    forced: Option<Clef>,
    update_clef: bool,
    octave_annotation: i8,
    signature: &harmonicon_core::notation::KeySignature,
) {
    // One clef for the whole song, from its own range — see `choose_clef`.
    let clef = notation::resolve_clef(notes, forced);
    for (mut text, mut node) in clefs.iter_mut().filter(|_| update_clef) {
        let glyph = match (clef, octave_annotation) {
            (Clef::Bass, -1) => "\u{E064}", (Clef::Treble, -1) => "\u{E052}", _ => clef.glyph(),
        };
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
    let marks = accidentals_in_bars(notes, clef, bar_map, *signature);

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
