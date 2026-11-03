// SPDX-License-Identifier: MIT

//! The flute on the play highway. Its rows are the flute's keys, top to
//! bottom down the tube (`instrument::flute::key_rows`), and a vertical
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
use bevy::ui_widgets::Activate;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::instrument::flute::{FingeringAssignment, Flute, Side, key_rows};
use harmonicon_core::instrument::{InstrumentModel, LaneLayout};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::{FluteView, MyInstruments};

/// A pending flute note, block or ribbon: a cool silver, the colour of
/// the instrument.
pub(super) const FLUTE_COLOR: Color = Color::srgb(0.72, 0.80, 0.92);
const BLOCK_HIT: Color = Color::srgb(0.95, 0.80, 0.30);
const BLOCK_MISSED: Color = Color::srgba(0.55, 0.18, 0.18, 0.7);

/// The pause menu's "Flute view" row, shown only on a flute part.
#[derive(Component, Default, Clone)]
pub(super) struct FluteViewRow;

#[derive(Component, Default, Clone)]
pub(super) struct FluteViewLabel;

/// Flips the flute view. It is one of "My instruments", so it persists:
/// the next flute part, and the next session, open on it.
pub(super) fn on_toggle_flute_view(_: On<Activate>, mut mine: ResMut<MyInstruments>) {
    mine.flute_view = match mine.flute_view {
        FluteView::Fingering => FluteView::Notes,
        FluteView::Notes => FluteView::Fingering,
    };
}

pub(super) fn flute_view_label_text(loc: &Localization, view: FluteView) -> String {
    String::from(loc.msg(match view {
        FluteView::Fingering => "pause-flute-view-fingering",
        FluteView::Notes => "pause-flute-view-notes",
    }))
}

/// Shows the pause menu's flute-view row only while a flute part plays,
/// and keeps its readout current.
pub(super) fn update_flute_view_row(
    mine: Res<MyInstruments>,
    loc: Res<Localization>,
    drawing: Query<(), With<FluteDrawing>>,
    mut rows: Query<&mut Node, With<FluteViewRow>>,
    mut labels: Query<&mut Text, With<FluteViewLabel>>,
) {
    let want = if drawing.is_empty() { Display::None } else { Display::Flex };
    for mut node in &mut rows {
        if node.display != want {
            node.display = want;
        }
    }
    let text = flute_view_label_text(&loc, mine.flute_view);
    for mut label in &mut labels {
        if label.0 != text {
            label.0.clone_from(&text);
        }
    }
}

/// Lays the highway out for the player's [`FluteView`] whenever it changes,
/// including the first frame of a flute part: the lanes, the furniture in
/// their container, the flute drawing and its label shown or hidden, and
/// every note visual despawned for `spawn_visible_notes` to respawn in the
/// new shape. Score state lives in `SongNotes`, so a switch mid-song loses
/// nothing.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_flute_view(
    mut last: Local<Option<FluteView>>,
    mine: Res<MyInstruments>,
    notes: Res<SongNotes>,
    mut lanes: ResMut<super::HighwayLanes>,
    mut drawing: Query<(&FluteDrawing, &mut Node)>,
    mut labels: Query<&mut Node, (With<FingeringLabel>, Without<FluteDrawing>)>,
    furniture: Query<(Entity, Option<&Children>), With<super::highway_2d::HighwayFurniture>>,
    visuals: Query<Entity, With<super::NoteVisual>>,
    mut commands: Commands,
) {
    let Ok((FluteDrawing(flute), mut drawing_node)) = drawing.single_mut() else {
        *last = None;
        return;
    };
    let view = mine.flute_view;
    if *last == Some(view) {
        return;
    }
    *last = Some(view);
    let fingering = view == FluteView::Fingering;
    let (layout, style) = if fingering {
        (InstrumentModel::lane_layout(flute), super::highway_2d::RowStyle::Keys)
    } else {
        let used = notes.notes.iter().filter_map(|n| n.expected_pitch).collect();
        (LaneLayout::pitch_rows(&used), super::highway_2d::RowStyle::Pitches)
    };
    for (container, children) in &furniture {
        for &child in children.into_iter().flatten() {
            commands.entity(child).despawn();
        }
        commands
            .entity(container)
            .with_children(|f| super::highway_2d::spawn_highway(f, &layout, style));
    }
    for visual in &visuals {
        commands.entity(visual).despawn();
    }
    lanes.0 = Some(layout);
    let display = if fingering { Display::Flex } else { Display::None };
    drawing_node.display = display;
    for mut node in &mut labels {
        node.display = display;
    }
}

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

/// The flute standing upright beside the highway's rows, drawn as a
/// fingering chart draws it: the six finger holes as large circles on the
/// tube's line, the left hand above the gap and the right hand below; the
/// thumb key as a small circle to the left of the first hole; the G♯,
/// trill, E♭ and foot keys as small circles to the right. Each sits in its
/// own row, at the same height the highway gives that row
/// (`InstrumentModel::lane_layout`, weighted by `flute::key_rows`).
pub(super) fn spawn_flute(parent: &mut ChildSpawnerCommands, flute: &Flute) {
    const TUBE_LEFT: f32 = 30.0;
    const TUBE_W: f32 = 26.0;
    const HOLE: f32 = 22.0;
    const SIDE_KEY: f32 = 12.0;
    let rows = key_rows(flute.foot);
    let layout = InstrumentModel::lane_layout(flute);
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
                    left: Val::Px(TUBE_LEFT),
                    width: Val::Px(TUBE_W),
                    top: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.33, 0.38)),
            ));
            for (row, key_row) in rows.into_iter().enumerate() {
                let Some(key) = key_row.key else { continue };
                let (top, height) = layout.span(row);
                let (size, left) = match key_row.side {
                    Side::Tube => (HOLE, TUBE_LEFT + (TUBE_W - HOLE) * 0.5),
                    Side::Left => (SIDE_KEY, TUBE_LEFT - SIDE_KEY - 6.0),
                    Side::Right => (SIDE_KEY, TUBE_LEFT + TUBE_W + 6.0),
                };
                drawing
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(top),
                        height: Val::Percent(height),
                        left: Val::Px(left),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|slot| {
                        slot.spawn((
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

/// Fills the drawing's keys: solid for `now`'s fingering, ghosted for
/// `next`'s. Shared by the play highway and the Note Trainer.
pub(super) fn paint_flute_keys(
    flute: &Flute,
    now: Option<FingeringAssignment>,
    next: Option<FingeringAssignment>,
    keys: &mut Query<(&FluteKey, &mut BackgroundColor)>,
) {
    let pressed = |at: Option<FingeringAssignment>, key| {
        at.and_then(|at| flute.keys(at)).is_some_and(|k| k.pressed(key))
    };
    for (key, mut fill) in keys.iter_mut() {
        let want = key_fill(pressed(now, key.0), pressed(next, key.0));
        if fill.0 != want {
            fill.0 = want;
        }
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
    paint_flute_keys(flute, now, next, &mut keys);
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

/// Where a fingering note's name goes across the highway, `(start, size)`
/// in percent: the row of the gap between the hands, the one row no
/// fingering presses, so the name never sits on a block.
pub(super) fn name_row_span(flute: &Flute, layout: &LaneLayout) -> (f32, f32) {
    key_rows(flute.foot)
        .iter()
        .position(|row| row.key.is_none())
        .map_or((0.0, 100.0), |gap| layout.span(gap))
}

/// One fingering note's blocks, as children of its note visual, which
/// spans every row: one block per key the fingering presses, in that key's
/// row, and the note's name in the cap at its attack end.
pub(super) fn spawn_fingering_blocks(
    note_e: &mut ChildSpawnerCommands,
    flute: &Flute,
    at: FingeringAssignment,
    layout: &LaneLayout,
    fill: f32,
) {
    // Each block fills `fill` of its own row, so a side key's block is as
    // much thinner than a finger hole's as its row is.
    for row in flute.pressed_rows(at) {
        let (start, size) = layout.span(row);
        note_e.spawn((
            FingeringBlock,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                top: Val::Percent(start + size * (1.0 - fill) * 0.5),
                height: Val::Percent(size * fill),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(FLUTE_COLOR),
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
        FLUTE_COLOR
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
        assert_eq!(block_color(&n), FLUTE_COLOR);
        n.hit = true;
        assert_eq!(block_color(&n), BLOCK_HIT);
        n.missed = true;
        assert_eq!(block_color(&n), BLOCK_MISSED);
    }
}
