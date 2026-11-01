// SPDX-License-Identifier: MIT

//! The flute on the play highway. Its rows are the flute's keys, top to
//! bottom down the tube (`instrument::flute::key_order`), and a vertical
//! drawing of the flute stands at the left of them with each key level
//! with its row. A note travels in as the shape of its fingering — a block
//! on every key it presses — and the drawing lights the keys of the note
//! at the hit line, the next one's ghosted, so the hand is ready in time.
//!
//! A note is still one [`super::NoteVisual`], judged by pitch like every
//! other: only what it draws differs (blocks rather than one ribbon), so
//! scoring, holds and the judged pop need nothing flute-specific beyond
//! the span and colours here.
use super::{GameplayClock, LOOKAHEAD, ScheduledNote, SongNotes};
use bevy::prelude::*;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::instrument::flute::{FingeringAssignment, Flute, Keys, key_order};
use harmonicon_platform::localization::{Localization, LocalizationExt};

/// A pending fingering block: a cool silver, the colour of the instrument.
const BLOCK: Color = Color::srgb(0.72, 0.80, 0.92);
const BLOCK_HIT: Color = Color::srgb(0.95, 0.80, 0.30);
const BLOCK_MISSED: Color = Color::srgba(0.55, 0.18, 0.18, 0.7);

/// The flute drawing's root, carrying the flute it draws: the note spawner
/// reads which rows a fingering presses from it.
#[derive(Component)]
pub(super) struct FluteDrawing(pub Flute);

#[derive(Component)]
pub(super) struct FluteKey(u16);

/// One block of a fingering note: the row of a key it presses.
#[derive(Component)]
pub(super) struct FingeringBlock;

/// "Now E4 · next G4", under the highway.
#[derive(Component)]
pub(super) struct FingeringLabel;

/// The note landing now — still sounding, or the first to come within the
/// lookahead — and the first after it. Computed from chart time, so a seek,
/// a loop or an adaptive rebuild needs no reset.
fn targets(
    notes: &[ScheduledNote],
    now: f64,
) -> (Option<FingeringAssignment>, Option<FingeringAssignment>) {
    let fingering = |n: &ScheduledNote| match n.position {
        PlayPosition::Fingering(at) => Some(at),
        _ => None,
    };
    let mut ahead = notes
        .iter()
        .filter(|n| {
            n.playable && !n.missed && n.time + n.duration > now && n.time <= now + LOOKAHEAD
        })
        .filter(|n| fingering(n).is_some());
    let Some(current) = ahead.next() else { return (None, None) };
    let next = ahead.find(|n| n.time > current.time).and_then(fingering);
    (fingering(current), next)
}

/// Finger plates are drawn large; the thumb, G♯, trill and pinky keys small.
fn is_plate(key: u16) -> bool {
    matches!(key, Keys::L1 | Keys::L2 | Keys::L3 | Keys::R1 | Keys::R2 | Keys::R3)
}

/// The flute standing upright beside the highway's rows: a tube, and on it
/// one key per row, each centred on its row.
pub(super) fn spawn_flute(parent: &mut ChildSpawnerCommands, flute: &Flute) {
    let keys = key_order(flute.foot);
    let row_pct = 100.0 / keys.len() as f32;
    parent
        .spawn((
            FluteDrawing(*flute),
            Node {
                width: Val::Px(92.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|drawing| {
            // The tube, behind the keys.
            drawing.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    width: Val::Px(24.0),
                    top: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.33, 0.38)),
            ));
            for (row, key) in keys.into_iter().enumerate() {
                let size = if is_plate(key) { 22.0 } else { 13.0 };
                drawing
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(row as f32 * row_pct),
                        height: Val::Percent(row_pct),
                        width: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|slot| {
                        // Centred on the tube, whatever its size.
                        slot.spawn(Node {
                            width: Val::Px(64.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        })
                        .with_children(|centre| {
                            centre.spawn((
                                FluteKey(key),
                                Node {
                                    width: Val::Px(size),
                                    height: Val::Px(size),
                                    border: UiRect::all(Val::Px(2.0)),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.75)),
                                BackgroundColor(Color::NONE),
                            ));
                        });
                        slot.spawn((
                            Text::new(harmonicon_core::instrument::flute::key_label(key)),
                            TextFont { font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
                        ));
                    });
            }
        });
}

/// A drawn key's fill: solid for the note at the hit line, a ghost for the
/// one after, open otherwise.
fn key_fill(now: bool, next: bool) -> Color {
    if now {
        Color::WHITE
    } else if next {
        Color::srgba(1.0, 1.0, 1.0, 0.3)
    } else {
        Color::NONE
    }
}

pub(super) fn update_flute(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    loc: Res<Localization>,
    drawing: Query<&FluteDrawing>,
    mut keys: Query<(&FluteKey, &mut BackgroundColor)>,
    mut labels: Query<&mut Text, With<FingeringLabel>>,
) {
    let Ok(FluteDrawing(flute)) = drawing.single() else { return };
    let (now, next) = targets(&notes.notes, clock.get());
    let pressed = |at: Option<FingeringAssignment>, key| {
        at.and_then(|at| flute.keys(at)).is_some_and(|k| k.pressed(key))
    };
    for (key, mut fill) in &mut keys {
        let want = key_fill(pressed(now, key.0), pressed(next, key.0));
        if fill.0 != want {
            fill.0 = want;
        }
    }
    let name = |at: Option<FingeringAssignment>| {
        at.map_or_else(
            || "—".to_string(),
            |at| harmonicon_core::midi::midi_to_note(i32::from(at.note)),
        )
    };
    let want = String::from(
        loc.msg_args("gameplay-flute-fingering", &[("now", name(now)), ("next", name(next))]),
    );
    for mut text in &mut labels {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// One fingering note's blocks, as children of its note visual, which
/// spans every row: one block per key the fingering presses, in that key's
/// row, and the note's name in the cap at its attack end.
pub(super) fn spawn_fingering_blocks(
    note_e: &mut ChildSpawnerCommands,
    flute: &Flute,
    at: FingeringAssignment,
    lane_pct: f32,
    block_pct: f32,
) {
    for row in flute.pressed_rows(at) {
        note_e.spawn((
            FingeringBlock,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                top: Val::Percent(row as f32 * lane_pct + (lane_pct - block_pct) * 0.5),
                height: Val::Percent(block_pct),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(BLOCK),
        ));
    }
}

/// A block's colour for its note's state: gold once hit, dim red once
/// missed, silver while it is coming.
fn block_color(note: &ScheduledNote) -> Color {
    if note.missed {
        BLOCK_MISSED
    } else if note.hit {
        BLOCK_HIT
    } else {
        BLOCK
    }
}

pub(super) fn update_fingering_blocks(
    notes: Res<SongNotes>,
    visuals: Query<(&super::NoteVisual, &Children)>,
    mut blocks: Query<&mut BackgroundColor, With<FingeringBlock>>,
) {
    for (visual, children) in &visuals {
        let Some(note) = notes.notes.get(visual.note_id) else { continue };
        let want = block_color(note);
        for child in children {
            if let Ok(mut fill) = blocks.get_mut(*child)
                && fill.0 != want
            {
                fill.0 = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time: f64, midi: u8, alternate: u8) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.5,
            position: PlayPosition::Fingering(FingeringAssignment { note: midi, alternate }),
            ..default()
        }
    }

    #[test]
    fn the_drawing_shows_the_landing_note_then_the_next_and_rewinds() {
        let notes = vec![note(1.0, 64, 0), note(2.0, 67, 0), note(3.0, 86, 1)];
        let at = |n: u8, a: u8| Some(FingeringAssignment { note: n, alternate: a });
        assert_eq!(targets(&notes, -9.0), (None, None), "nothing within the lookahead");
        assert_eq!(targets(&notes, 0.0), (at(64, 0), at(67, 0)));
        assert_eq!(targets(&notes, 1.2), (at(64, 0), at(67, 0)), "held through its duration");
        assert_eq!(targets(&notes, 2.6), (at(86, 1), None), "the last has no next");
        assert_eq!(targets(&notes, 0.0), (at(64, 0), at(67, 0)), "a loop back reads afresh");
    }

    #[test]
    fn a_missed_note_hands_the_drawing_to_the_next() {
        let mut notes = vec![note(1.0, 64, 0), note(2.0, 67, 0)];
        notes[0].missed = true;
        assert_eq!(targets(&notes, 1.2).0.map(|a| a.note), Some(67));
    }

    #[test]
    fn a_key_held_now_outranks_its_ghost_and_blocks_follow_judgment() {
        assert_eq!(key_fill(true, true), Color::WHITE);
        assert_ne!(key_fill(false, true), Color::NONE);
        assert_eq!(key_fill(false, false), Color::NONE);
        let mut n = note(1.0, 64, 0);
        assert_eq!(block_color(&n), BLOCK);
        n.hit = true;
        assert_eq!(block_color(&n), BLOCK_HIT);
        n.missed = true;
        assert_eq!(block_color(&n), BLOCK_MISSED);
    }
}
