// SPDX-License-Identifier: MIT

//! A "Let's Bend"-style harmonica diagram: holes as columns, with a row for
//! each way a hole can sound, each cell labelled with its note and lit up
//! live from the mic (via [`ActivePitches`]). Built from the selected harp's
//! blow/draw layout, so it follows the song's key. Diatonic harps get the
//! full bend/overblow/overdraw diagram (holes 1–10, [`ROWS`]); chromatic
//! harps get a simpler blow/draw + slide diagram sized to the harp's actual
//! hole count, since bends and overblow/overdraw don't exist on that harp.

use std::collections::HashSet;

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::audio_system::midi::note_to_midi;
use crate::localization::LocalizationExt;
use crate::song::chart::Action;
use crate::song::harmonica::{Harmonica, HoleNotes, hole_notes, valid_note};

use super::ActivePitches;

pub(super) const CELL_DEFAULT: Color = Color::srgba(0.12, 0.12, 0.16, 0.92);
const CELL_LIT: Color = Color::srgb(0.95, 0.85, 0.30);
/// Every row — including bends and over-blow/draw — is colored by which
/// breath direction actually produces it, not by technique. Overblow reads
/// as blue (you *blow* it, even though its pitch sits above the draw note)
/// and overdraw reads as orange (you *draw* it) — the two are easy to mix up
/// by name alone, so color is the disambiguator.
const BLOW_COLOR: Color = Color::srgb(0.45, 0.70, 1.0);
const DRAW_COLOR: Color = Color::srgb(1.0, 0.65, 0.30);
const LABEL_COLOR: Color = Color::srgb(0.55, 0.55, 0.65);

/// One note cell in the diagram; lit when its pitch is sounding. `None` for a
/// cell whose label failed to parse to a MIDI number (shouldn't happen in
/// practice — `spawn_note_cell` is only ever given already-resolved note
/// labels — but a cell that can never light is a safer failure than a panic).
#[derive(Component)]
pub struct HarpOverlayCell {
    pub(super) midi: Option<u8>,
}

/// Note label with its octave digit dropped: `"D#5" → "D#"`.
fn note_class(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// Which row a cell belongs to. `DrawBend`/`Overblow` are only ever valid for
/// holes 1–6 and `BlowBend`/`Overdraw` only for holes 7–10 — [`note_for`]
/// enforces that, since a hole's single `bends`/`over` fields don't carry
/// which family they belong to on their own. `pub(super)` (rather than
/// private) so [`DiagramCellTarget`] can carry it out to a sibling module —
/// the Bending Trainer's hole/technique picker.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Row {
    Overblow,
    DrawBend(usize),
    Blow,
    Draw,
    BlowBend(usize),
    Overdraw,
}

/// The diagram's rows, top to bottom, with their left-hand label and color.
///
/// Grouped by actual breath direction, not music theory or hole family:
/// blow sits above draw, and every row that's played by *blowing* (blow
/// bends, holes 7–10, plus overblow, holes 1/4/5/6) sits above the blow row;
/// every row played by *drawing* (draw bends, holes 1–6, plus overdraw,
/// holes 7–10) sits below the draw row. So the top half of the diagram is
/// always "blow" and the bottom half is always "draw" — position alone
/// tells a player which way to breathe, with color reinforcing it.
const ROWS: [(&str, Row, Color); 10] = [
    ("overblow \u{2191}", Row::Overblow, BLOW_COLOR),
    ("1\u{00BD} \u{2191}", Row::BlowBend(2), BLOW_COLOR),
    ("1 \u{2191}", Row::BlowBend(1), BLOW_COLOR),
    ("\u{00BD} \u{2191}", Row::BlowBend(0), BLOW_COLOR),
    ("blow \u{2191}", Row::Blow, BLOW_COLOR),
    ("draw \u{2193}", Row::Draw, DRAW_COLOR),
    ("\u{00BD} \u{2193}", Row::DrawBend(0), DRAW_COLOR),
    ("1 \u{2193}", Row::DrawBend(1), DRAW_COLOR),
    ("1\u{00BD} \u{2193}", Row::DrawBend(2), DRAW_COLOR),
    ("overdraw \u{2193}", Row::Overdraw, DRAW_COLOR),
];

/// The note `row` shows for `hole`, or `None` if that row doesn't apply to
/// this hole at all (wrong wing, or the technique isn't available here).
fn note_for(h: &HoleNotes, hole: u8, row: Row) -> Option<&str> {
    match row {
        Row::Overblow if hole <= 6 => h.over.as_deref(),
        Row::Overdraw if hole >= 7 => h.over.as_deref(),
        Row::Blow => h.blow.as_deref(),
        Row::Draw => h.draw.as_deref(),
        Row::DrawBend(i) if hole <= 6 => h.bends.get(i).map(String::as_str),
        Row::BlowBend(i) if hole >= 7 => h.bends.get(i).map(String::as_str),
        _ => None,
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Spawn the harmonica diagram as a child of `parent`, built for `harp`.
/// Diatonic harps get the full bend/overblow/overdraw diagram; chromatic
/// harps (no bends, no overblow/overdraw — just a slide button) get a
/// simpler diagram, see [`spawn_chromatic_overlay`].
pub fn spawn_harmonica_overlay(parent: &mut ChildSpawnerCommands, harp: &Harmonica, loc: &Localization) {
    if matches!(harp, Harmonica::Chromatic { .. }) {
        spawn_chromatic_overlay(parent, harp, loc);
        return;
    }
    let holes: Vec<HoleNotes> = (1..=10).map(|h| hole_notes(harp, h)).collect();

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .with_children(|panel| {
            panel.spawn((
                Text::new(String::from(loc.msg("harmonica-overlay-hint-view"))),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.70, 0.80)),
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    // Header: blank corner + hole numbers 1–10.
                    grid.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_label(row, "");
                        for hole in 1..=10u8 {
                            spawn_text_cell(row, &hole.to_string(), Color::WHITE);
                        }
                    });

                    // One row per technique.
                    for (label, kind, color) in ROWS {
                        grid.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_label(row, label);
                            for (hole, h) in (1..=10u8).zip(&holes) {
                                match note_for(h, hole, kind) {
                                    Some(note) => {
                                        spawn_note_cell(row, note, color);
                                    }
                                    None => spawn_empty(row),
                                }
                            }
                        });
                    }
                });
        });
}

/// Tags a selectable diagram's note cell with which (hole, row) it is, so a
/// single shared `on_click` observer can look this up via the clicked
/// entity (`ev.entity`) instead of needing a distinct closure per cell.
#[derive(Component, Clone, Copy)]
pub struct DiagramCellTarget {
    pub hole: u8,
    pub(super) row: Row,
}

/// Like [`spawn_harmonica_overlay`], but every note cell is clickable —
/// tagged with [`DiagramCellTarget`] and given `on_click` as a shared
/// `.observe(...)` — for a UI that lets the player pick a hole/technique
/// directly off the diagram (the Bending Trainer's target picker) instead of
/// separate stepper controls. Diatonic only, same as the plain diagram's
/// bend rows; a chromatic harp falls back to the ordinary, non-selectable
/// [`spawn_chromatic_overlay`] (dead code for the trainer today, which is
/// diatonic-only by design, but kept safe rather than assumed).
pub fn spawn_harmonica_overlay_selectable<M: 'static>(
    parent: &mut ChildSpawnerCommands,
    harp: &Harmonica,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M> + Clone + Sync + 'static,
    loc: &Localization,
) {
    if matches!(harp, Harmonica::Chromatic { .. }) {
        spawn_chromatic_overlay(parent, harp, loc);
        return;
    }
    let holes: Vec<HoleNotes> = (1..=10).map(|h| hole_notes(harp, h)).collect();

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .with_children(|panel| {
            panel.spawn((
                Text::new(String::from(loc.msg("harmonica-overlay-hint-select"))),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.70, 0.80)),
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    // Header: blank corner + hole numbers 1–10.
                    grid.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_label(row, "");
                        for hole in 1..=10u8 {
                            spawn_text_cell(row, &hole.to_string(), Color::WHITE);
                        }
                    });

                    // One row per technique.
                    for (label, kind, color) in ROWS {
                        grid.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_label(row, label);
                            for (hole, h) in (1..=10u8).zip(&holes) {
                                match note_for(h, hole, kind) {
                                    Some(note) => {
                                        spawn_note_cell(row, note, color)
                                            .insert(DiagramCellTarget { hole, row: kind })
                                            .observe(on_click.clone());
                                    }
                                    None => spawn_empty(row),
                                }
                            }
                        });
                    }
                });
        });
}

/// Which row of the chromatic diagram a cell belongs to.
#[derive(Clone, Copy)]
enum ChromaticRow {
    BlowSlide,
    Blow,
    Draw,
    DrawSlide,
}

/// The chromatic diagram's rows, top to bottom: slide sits further from the
/// blow/draw center on each wing, mirroring the diatonic diagram's convention
/// that the altered pitch sits away from center and color marks breath
/// direction, not technique.
const CHROMATIC_ROWS: [(&str, ChromaticRow, Color); 4] = [
    ("slide \u{2191}", ChromaticRow::BlowSlide, BLOW_COLOR),
    ("blow \u{2191}", ChromaticRow::Blow, BLOW_COLOR),
    ("draw \u{2193}", ChromaticRow::Draw, DRAW_COLOR),
    ("slide \u{2193}", ChromaticRow::DrawSlide, DRAW_COLOR),
];

/// The note `row` shows for `hole`, or `None` if the harp has no layout data
/// for that cell (`wind_direction_label`/`slide_label` return `"—"`).
fn chromatic_note_for(harp: &Harmonica, hole: u8, row: ChromaticRow) -> Option<String> {
    let label = match row {
        ChromaticRow::Blow => harp.wind_direction_label(hole, &Action::Blow),
        ChromaticRow::Draw => harp.wind_direction_label(hole, &Action::Draw),
        ChromaticRow::BlowSlide => harp.slide_label(hole, &Action::Blow),
        ChromaticRow::DrawSlide => harp.slide_label(hole, &Action::Draw),
    };
    valid_note(label)
}

/// Spawn the simpler chromatic diagram: blow/draw plus the slide-raised
/// pitch on each side, sized to `harp`'s actual hole count (12 or 16 — the
/// bend/overblow/overdraw diagram in [`spawn_harmonica_overlay`] only applies
/// to the fixed 10-hole diatonic layout).
fn spawn_chromatic_overlay(parent: &mut ChildSpawnerCommands, harp: &Harmonica, loc: &Localization) {
    let hole_count = harp.hole_count();

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .with_children(|panel| {
            panel.spawn((
                Text::new(String::from(loc.msg("harmonica-overlay-hint-view"))),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.70, 0.80)),
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    // Header: blank corner + hole numbers.
                    grid.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_label(row, "");
                        for hole in 1..=hole_count {
                            spawn_text_cell(row, &hole.to_string(), Color::WHITE);
                        }
                    });

                    // One row per technique.
                    for (label, kind, color) in CHROMATIC_ROWS {
                        grid.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_label(row, label);
                            for hole in 1..=hole_count {
                                match chromatic_note_for(harp, hole, kind) {
                                    Some(note) => {
                                        spawn_note_cell(row, &note, color);
                                    }
                                    None => spawn_empty(row),
                                }
                            }
                        });
                    }
                });
        });
}

/// A 44×28 cell shell. Returns its `EntityCommands` so callers add content.
/// Every cell gets a (transparent by default) border, so a selectable
/// diagram (see [`spawn_harmonica_overlay_selectable`]) can color one in
/// without changing the cell's box model on the way in/out of selection.
fn cell<'a>(row: &'a mut ChildSpawnerCommands, bg: Color) -> EntityCommands<'a> {
    row.spawn((
        Node {
            width: Val::Px(44.0),
            height: Val::Px(28.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(Color::NONE),
    ))
}

/// A note cell: shows the note class, lights up live (carries `HarpOverlayCell`).
/// Returns its `EntityCommands` so a selectable diagram can additionally tag
/// it with [`DiagramCellTarget`] and an `on_click` observer.
fn spawn_note_cell<'a>(
    row: &'a mut ChildSpawnerCommands,
    note: &str,
    color: Color,
) -> EntityCommands<'a> {
    let mut ec = cell(row, CELL_DEFAULT);
    ec.insert(HarpOverlayCell {
        midi: note_to_midi(note).and_then(|m| u8::try_from(m).ok()),
    });
    ec.with_children(|c| {
        c.spawn((
            Text::new(note_class(note).to_string()),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(color),
        ));
    });
    ec
}

/// A static text cell (header numbers), no highlight.
fn spawn_text_cell(row: &mut ChildSpawnerCommands, text: &str, color: Color) {
    cell(row, Color::NONE).with_children(|c| {
        c.spawn((
            Text::new(text.to_string()),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(color),
        ));
    });
}

/// The left-hand row label (narrower than a hole cell). Wide enough for the
/// longest label ("overblow ↑"/"overdraw ↓") at this font size.
fn spawn_label(row: &mut ChildSpawnerCommands, text: &str) {
    row.spawn(Node {
        width: Val::Px(90.0),
        height: Val::Px(28.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::FlexEnd,
        padding: UiRect::right(Val::Px(4.0)),
        ..default()
    })
    .with_children(|c| {
        c.spawn((
            Text::new(text.to_string()),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(LABEL_COLOR),
        ));
    });
}

/// A blank spacer where a hole has no note for this row.
fn spawn_empty(row: &mut ChildSpawnerCommands) {
    cell(row, Color::NONE);
}

// ── Live highlight ──────────────────────────────────────────────────────────

/// Light every cell whose note is currently sounding (from the mic), reusing the
/// same [`ActivePitches`] the scored modes detect.
pub fn update_harmonica_overlay(
    active: Res<ActivePitches>,
    mut cells: Query<(&HarpOverlayCell, &mut BackgroundColor)>,
) {
    let played: HashSet<u8> = active.0.iter().map(|p| p.midi).collect();

    for (cell, mut bg) in &mut cells {
        bg.0 = if cell.midi.is_some_and(|m| played.contains(&m)) {
            CELL_LIT
        } else {
            CELL_DEFAULT
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_harp() -> Harmonica {
        serde_json::from_str(
            r#"{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
                "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                          "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}"#,
        )
        .unwrap()
    }

    // `hole_notes`/`transpose` are now `crate::song::harmonica` — their
    // bend/overblow derivation is tested there, not duplicated here.

    // ── ROWS layout ──────────────────────────────────────────────────────────────

    #[test]
    fn rows_has_two_wings_of_five_around_no_gap() {
        assert_eq!(ROWS.len(), 10);
    }

    #[test]
    fn top_half_is_always_blow_and_bottom_half_is_always_draw() {
        // The whole point of this layout: position alone tells a player
        // which way to breathe. Overblow is executed by *blowing* even
        // though its pitch sits above the draw note (and overdraw is the
        // mirror case) — that name-vs-action mismatch is exactly the
        // confusion this is meant to resolve, so it must still land on the
        // blow side here despite the name starting with "over".
        let expected_by_label: &[(&str, Color)] = &[
            ("overblow \u{2191}", BLOW_COLOR),
            ("1\u{00BD} \u{2191}", BLOW_COLOR),
            ("1 \u{2191}", BLOW_COLOR),
            ("\u{00BD} \u{2191}", BLOW_COLOR),
            ("blow \u{2191}", BLOW_COLOR),
            ("draw \u{2193}", DRAW_COLOR),
            ("\u{00BD} \u{2193}", DRAW_COLOR),
            ("1 \u{2193}", DRAW_COLOR),
            ("1\u{00BD} \u{2193}", DRAW_COLOR),
            ("overdraw \u{2193}", DRAW_COLOR),
        ];
        for (i, (label, _row, color)) in ROWS.iter().enumerate() {
            assert_eq!(*label, expected_by_label[i].0, "row {i} label");
            assert_eq!(*color, expected_by_label[i].1, "row {i} ({label}) color");
        }
        // The split is exactly down the middle: rows 0-4 blow, 5-9 draw.
        assert!(ROWS[..5].iter().all(|(_, _, c)| *c == BLOW_COLOR));
        assert!(ROWS[5..].iter().all(|(_, _, c)| *c == DRAW_COLOR));
    }

    #[test]
    fn bend_depth_increases_moving_away_from_the_blow_draw_center() {
        fn bend_index(row: Row) -> Option<usize> {
            match row {
                Row::DrawBend(i) | Row::BlowBend(i) => Some(i),
                _ => None,
            }
        }
        // Above blow: overblow, then 1½ → 1 → ½ heading down into `blow`.
        assert_eq!(bend_index(ROWS[1].1), Some(2));
        assert_eq!(bend_index(ROWS[2].1), Some(1));
        assert_eq!(bend_index(ROWS[3].1), Some(0));
        // Below draw: `draw`, then ½ → 1 → 1½ heading down into overdraw.
        assert_eq!(bend_index(ROWS[6].1), Some(0));
        assert_eq!(bend_index(ROWS[7].1), Some(1));
        assert_eq!(bend_index(ROWS[8].1), Some(2));
    }

    #[test]
    fn each_hole_lights_up_only_one_wing() {
        // Regression: DrawBend/Overblow and BlowBend/Overdraw read from the
        // same underlying `bends`/`over` fields, so without a wing check in
        // `note_for` a hole's draw bends would also leak into (and be
        // mislabeled within) the blow-bend rows, and vice versa.
        let harp = c_harp();
        for hole in 1..=10u8 {
            let h = hole_notes(&harp, hole);
            for (_, row, _) in ROWS {
                let draw_side = matches!(row, Row::Overblow | Row::DrawBend(_));
                let blow_side = matches!(row, Row::Overdraw | Row::BlowBend(_));
                if (draw_side && hole >= 7) || (blow_side && hole <= 6) {
                    assert!(
                        note_for(&h, hole, row).is_none(),
                        "hole {hole} shouldn't show anything in the wrong wing's row"
                    );
                }
            }
        }
    }

    #[test]
    fn note_for_only_answers_for_the_hole_it_actually_applies_to() {
        let harp = c_harp();
        // Hole 1 (draw-bend family): shows its draw bend and overblow...
        let h1 = hole_notes(&harp, 1);
        assert_eq!(note_for(&h1, 1, Row::DrawBend(0)), Some("C#4"));
        assert_eq!(note_for(&h1, 1, Row::Overblow), Some("D#4"));
        // ...but never as a blow bend or overdraw.
        assert_eq!(note_for(&h1, 1, Row::BlowBend(0)), None);
        assert_eq!(note_for(&h1, 1, Row::Overdraw), None);

        // Hole 10 (blow-bend family): the mirror image.
        let h10 = hole_notes(&harp, 10);
        assert_eq!(note_for(&h10, 10, Row::BlowBend(0)), Some("B6"));
        assert_eq!(note_for(&h10, 10, Row::Overdraw), Some("C#7"));
        assert_eq!(note_for(&h10, 10, Row::DrawBend(0)), None);
        assert_eq!(note_for(&h10, 10, Row::Overblow), None);
    }

    // ── Chromatic diagram ────────────────────────────────────────────────────

    fn c_chromatic_harp() -> Harmonica {
        serde_json::from_str(
            r#"{"type":"chromatic","holes":12,
                "layout":{"blow":["C4","D4","E4","F4","G4","A4","B4","C5","D5","E5","F5","G5"],
                          "draw":["D4","E4","F#4","G4","A4","B4","C#5","D5","E5","F#5","G5","A5"],
                          "blow_slide":["C#4","D#4","F4","F#4","G#4","A#4","C5","C#5","D#5","F5","F#5","G#5"],
                          "draw_slide":["D#4","F4","G4","G#4","A#4","C5","D5","D#5","F5","G5","G#5","A#5"]}}"#,
        )
        .unwrap()
    }

    #[test]
    fn chromatic_note_for_reads_blow_and_draw() {
        let harp = c_chromatic_harp();
        assert_eq!(
            chromatic_note_for(&harp, 1, ChromaticRow::Blow).as_deref(),
            Some("C4")
        );
        assert_eq!(
            chromatic_note_for(&harp, 1, ChromaticRow::Draw).as_deref(),
            Some("D4")
        );
    }

    #[test]
    fn chromatic_note_for_reads_the_slide_tables() {
        let harp = c_chromatic_harp();
        assert_eq!(
            chromatic_note_for(&harp, 1, ChromaticRow::BlowSlide).as_deref(),
            Some("C#4")
        );
        assert_eq!(
            chromatic_note_for(&harp, 1, ChromaticRow::DrawSlide).as_deref(),
            Some("D#4")
        );
    }

    #[test]
    fn chromatic_note_for_is_none_for_a_diatonic_harp() {
        // A diatonic harp has no slide tables at all.
        let harp = c_harp();
        assert_eq!(chromatic_note_for(&harp, 1, ChromaticRow::BlowSlide), None);
    }
}
