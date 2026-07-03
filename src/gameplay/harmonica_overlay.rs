// SPDX-License-Identifier: MIT

//! A "Let's Bend"-style harmonica diagram: holes 1–10 as columns, with a row
//! for each way a hole can sound — overblow/overdraw, blow, draw, and the
//! draw/blow bends — each cell labelled with its note and lit up live from the
//! mic (via [`ActivePitches`]). Built from the selected harp's blow/draw layout,
//! so it follows the song's key.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::song::chart::Action;
use crate::song::harmonica::Harmonica;

use super::ActivePitches;

const NOTES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

const CELL_DEFAULT: Color = Color::srgba(0.12, 0.12, 0.16, 0.92);
const CELL_LIT: Color = Color::srgb(0.95, 0.85, 0.30);
/// Every row — including bends and over-blow/draw — is colored by which
/// breath direction actually produces it, not by technique. Overblow reads
/// as blue (you *blow* it, even though its pitch sits above the draw note)
/// and overdraw reads as orange (you *draw* it) — the two are easy to mix up
/// by name alone, so color is the disambiguator.
const BLOW_COLOR: Color = Color::srgb(0.45, 0.70, 1.0);
const DRAW_COLOR: Color = Color::srgb(1.0, 0.65, 0.30);
const LABEL_COLOR: Color = Color::srgb(0.55, 0.55, 0.65);

/// One note cell in the diagram; lit when its `note` (e.g. `"C#4"`) is sounding.
#[derive(Component)]
pub struct HarpOverlayCell {
    note: String,
}

// ── Note math ───────────────────────────────────────────────────────────────

/// Parse a note label like `"C#4"` to a MIDI number (C4 = 60).
fn note_to_midi(s: &str) -> Option<i32> {
    let i = s.find(|c: char| c.is_ascii_digit())?;
    let (class, octave) = s.split_at(i);
    let octave: i32 = octave.parse().ok()?;
    let idx = NOTES.iter().position(|&n| n == class)?;
    Some((octave + 1) * 12 + idx as i32)
}

fn midi_to_note(m: i32) -> String {
    let octave = m.div_euclid(12) - 1;
    let idx = m.rem_euclid(12) as usize;
    format!("{}{}", NOTES[idx], octave)
}

/// Transpose a note label by `semis` semitones, e.g. `transpose("B4", 1) → "C5"`.
fn transpose(s: &str, semis: i32) -> Option<String> {
    note_to_midi(s).map(|m| midi_to_note(m + semis))
}

/// Note label with its octave digit dropped: `"D#5" → "D#"`.
fn note_class(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// Keep a harp label only if it's a real note (drops the `—` "not available".)
fn valid_note(s: String) -> Option<String> {
    note_to_midi(&s).map(|_| s)
}

// ── Per-hole note set ─────────────────────────────────────────────────────────

/// Every note one hole can produce, by technique. `pub` so other trainers
/// (e.g. the Bending Trainer's target picker) can reuse the same bend/overblow
/// math instead of re-deriving it.
pub struct HoleNotes {
    pub over: Option<String>,
    pub blow: Option<String>,
    pub draw: Option<String>,
    /// Bends, smallest first (½ step, whole, 1½). Draw bends on holes 1–6,
    /// blow bends on holes 7–10.
    pub bends: Vec<String>,
}

/// Which row a cell belongs to. `DrawBend`/`Overblow` are only ever valid for
/// holes 1–6 and `BlowBend`/`Overdraw` only for holes 7–10 — [`note_for`]
/// enforces that, since a hole's single `bends`/`over` fields don't carry
/// which family they belong to on their own.
#[derive(Clone, Copy)]
enum Row {
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

pub fn hole_notes(harp: &Harmonica, hole: u8) -> HoleNotes {
    let blow = valid_note(harp.wind_direction_label(hole, &Action::Blow));
    let draw = valid_note(harp.wind_direction_label(hole, &Action::Draw));

    // Overblow (holes 1,4,5,6) sits a semitone above the draw reed; overdraw
    // (holes 7–10) a semitone above the blow reed.
    let over = match hole {
        1 | 4 | 5 | 6 => draw.as_deref().and_then(|d| transpose(d, 1)),
        7..=10 => blow.as_deref().and_then(|b| transpose(b, 1)),
        _ => None,
    };

    // Bends fill the chromatic steps between blow and draw: drawn down from the
    // draw reed on holes 1–6, down from the blow reed on holes 7–10.
    let mut bends = Vec::new();
    if let (Some(b), Some(d)) = (&blow, &draw)
        && let (Some(bm), Some(dm)) = (note_to_midi(b), note_to_midi(d))
    {
        if hole <= 6 && dm > bm + 1 {
            bends = (1..dm - bm).map(|k| midi_to_note(dm - k)).collect();
        } else if hole >= 7 && bm > dm + 1 {
            bends = (1..bm - dm).map(|k| midi_to_note(bm - k)).collect();
        }
    }

    HoleNotes {
        over,
        blow,
        draw,
        bends,
    }
}

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
pub fn spawn_harmonica_overlay(
    parent: &mut ChildSpawnerCommands,
    harp: &Harmonica,
) {
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
                Text::new("Harmonica  \u{00B7}  lights up as you play"),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
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
                                    Some(note) => spawn_note_cell(row, note, color),
                                    None => spawn_empty(row),
                                }
                            }
                        });
                    }
                });
        });
}

/// A 34×22 cell shell. Returns its `EntityCommands` so callers add content.
fn cell<'a>(row: &'a mut ChildSpawnerCommands, bg: Color) -> EntityCommands<'a> {
    row.spawn((
        Node {
            width: Val::Px(34.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(bg),
    ))
}

/// A note cell: shows the note class, lights up live (carries `HarpOverlayCell`).
fn spawn_note_cell(row: &mut ChildSpawnerCommands, note: &str, color: Color) {
    cell(row, CELL_DEFAULT)
        .insert(HarpOverlayCell { note: note.to_string() })
        .with_children(|c| {
            c.spawn((
                Text::new(note_class(note).to_string()),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(color),
            ));
        });
}

/// A static text cell (header numbers), no highlight.
fn spawn_text_cell(row: &mut ChildSpawnerCommands, text: &str, color: Color) {
    cell(row, Color::NONE).with_children(|c| {
        c.spawn((
            Text::new(text.to_string()),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(color),
        ));
    });
}

/// The left-hand row label (narrower than a hole cell). Wide enough for the
/// longest label ("overblow ↑"/"overdraw ↓") at this font size.
fn spawn_label(row: &mut ChildSpawnerCommands, text: &str) {
    row.spawn(Node {
        width: Val::Px(58.0),
        height: Val::Px(22.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::FlexEnd,
        padding: UiRect::right(Val::Px(4.0)),
        ..default()
    })
    .with_children(|c| {
        c.spawn((
            Text::new(text.to_string()),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
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
    let played: HashSet<String> = active
        .0
        .iter()
        .map(|p| format!("{}{}", p.note, p.octave))
        .collect();

    for (cell, mut bg) in &mut cells {
        bg.0 = if played.contains(&cell.note) {
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

    #[test]
    fn transpose_crosses_octave() {
        assert_eq!(transpose("B4", 1).as_deref(), Some("C5"));
        assert_eq!(transpose("C5", -1).as_deref(), Some("B4"));
        assert_eq!(transpose("D4", -1).as_deref(), Some("C#4"));
    }

    #[test]
    fn hole_1_has_one_draw_bend() {
        // C harp hole 1: blow C4, draw D4 → single ½-step bend C#4.
        let h = hole_notes(&c_harp(), 1);
        assert_eq!(h.blow.as_deref(), Some("C4"));
        assert_eq!(h.draw.as_deref(), Some("D4"));
        assert_eq!(h.bends, vec!["C#4"]);
    }

    #[test]
    fn hole_3_has_three_draw_bends() {
        // blow G4, draw B4 → A#4, A4, G#4 (½, whole, 1½).
        let h = hole_notes(&c_harp(), 3);
        assert_eq!(h.bends, vec!["A#4", "A4", "G#4"]);
    }

    #[test]
    fn hole_10_has_two_blow_bends() {
        // blow C7, draw A6 → blow bends B6, A#6.
        let h = hole_notes(&c_harp(), 10);
        assert_eq!(h.bends, vec!["B6", "A#6"]);
        assert_eq!(h.over.as_deref(), Some("C#7"), "overdraw a semitone above blow");
    }

    #[test]
    fn hole_5_has_no_draw_bend() {
        // blow E5, draw F5 — only a semitone apart, nothing between.
        let h = hole_notes(&c_harp(), 5);
        assert!(h.bends.is_empty());
    }

    #[test]
    fn hole_1_overblow_is_a_semitone_above_draw() {
        assert_eq!(hole_notes(&c_harp(), 1).over.as_deref(), Some("D#4"));
    }

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
}
