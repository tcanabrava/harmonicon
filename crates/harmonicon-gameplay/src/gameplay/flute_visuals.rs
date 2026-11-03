// SPDX-License-Identifier: MIT

//! The flute in Play 2D: the note highway, one row per pitch the song uses
//! with each row named, and under it a **fingering strip**. The strip is a
//! fingering chart lying along the screen for the note to play now, with
//! its name and a bar counting down to it and then through it, and a
//! smaller, dimmer chart for the note after it. The highway says when and
//! which note; the strip says how to finger it.
//!
//! The pause menu's "Flute view" hides or shows the strip. It is kept as
//! one of "My instruments" (`settings::FluteView`: `Fingering` shows it,
//! `Notes` is the highway alone), and [`sync_flute_view`] applies it
//! mid-song.
use super::{GameplayClock, ScheduledNote, SongNotes};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_core::instrument::flute::{FingeringAssignment, Flute, Side, key_rows};
use harmonicon_core::instrument::{InstrumentModel, LaneLayout, PlayPosition};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::{FluteView, MyInstruments};

/// A pending flute note's ribbon: a cool silver, the colour of the
/// instrument.
pub(super) const FLUTE_COLOR: Color = Color::srgb(0.72, 0.80, 0.92);
/// A key held for the note to play now while it sounds and is heard.
const KEY_HELD: Color = Color::srgb(0.98, 0.80, 0.25);
/// A key of the note to play now, before or while it sounds.
const KEY_NOW: Color = Color::WHITE;
/// A key of the note after it.
const KEY_NEXT: Color = Color::srgba(1.0, 1.0, 1.0, 0.45);
/// Seconds before a note's start its countdown bar begins to fill.
const LEAD_SECS: f64 = 1.5;

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
    stage: Query<(), With<FluteStage>>,
    mut rows: Query<&mut Node, With<FluteViewRow>>,
    mut labels: Query<&mut Text, With<FluteViewLabel>>,
) {
    let want = if stage.is_empty() { Display::None } else { Display::Flex };
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

/// The fingering strip under the highway, carrying the flute.
#[derive(Component)]
pub(super) struct FluteStage(pub Flute);

/// Which chart a drawing is: the note to play now, or the one after it.
/// The Note Trainer's single drawing is a `Now`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ChartSlot {
    #[default]
    Now,
    Next,
}

/// A fingering chart's root, carrying the flute it draws.
#[derive(Component)]
pub(super) struct FluteDrawing(pub Flute);

/// One key of a drawn chart.
#[derive(Component)]
pub(super) struct FluteKey {
    key: u16,
    slot: ChartSlot,
}

/// The name of the note a stage chart shows.
#[derive(Component)]
pub(super) struct StageNoteName(ChartSlot);

/// The fill of the now-chart's countdown bar.
#[derive(Component)]
pub(super) struct StageCountdown;

/// What the stage shows: the note to play now (still sounding, or the
/// first still to come) and the one after it, as indices into `notes`.
/// Computed from chart time, so a seek, a loop or an adaptive rebuild needs
/// no reset. A missed note hands the stage straight to the next.
fn stage_notes(notes: &[ScheduledNote], now: f64) -> (Option<usize>, Option<usize>) {
    let is_fingering = |n: &ScheduledNote| matches!(n.position, PlayPosition::Fingering(_));
    let mut ahead = notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.playable && !n.missed && n.time + n.duration > now && is_fingering(n));
    let Some((current, note)) = ahead.next() else { return (None, None) };
    let next = ahead.find(|(_, n)| n.time > note.time).map(|(i, _)| i);
    (Some(current), next)
}

fn fingering(note: &ScheduledNote) -> Option<FingeringAssignment> {
    match note.position {
        PlayPosition::Fingering(at) => Some(at),
        _ => None,
    }
}

/// The countdown bar, `(fill 0..=1, sounding)`: before the note it fills
/// over the last [`LEAD_SECS`] to its start, and while it sounds it empties
/// over the note's length, so it reads "get ready" and then "hold".
fn countdown(note: &ScheduledNote, now: f64) -> (f32, bool) {
    if now < note.time {
        (1.0 - ((note.time - now) / LEAD_SECS).clamp(0.0, 1.0) as f32, false)
    } else {
        let held = ((now - note.time) / note.duration.max(1e-3)).clamp(0.0, 1.0);
        (1.0 - held as f32, true)
    }
}

/// Which way a chart lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChartAxis {
    /// Upright, the head joint at the top: the Note Trainer's.
    Vertical,
    /// Lying down, the head joint at the left, as a horizontal printed
    /// chart draws it: the score view's stage.
    Horizontal,
}

/// A fingering chart drawn as a printed one is: the six finger holes as
/// large circles on the tube's line, the left hand nearer the head joint
/// and the right hand past a gap. The small keys sit off the line: the
/// thumb key beside the first hole, on the left when upright and below
/// when lying down; the G♯, trill, E♭ and foot keys on the other side, to
/// the right or above. `scale` 1 is the Note Trainer's size. Along the
/// tube the keys fill the parent (its height, or its width when lying
/// down).
pub(super) fn spawn_flute_chart(
    parent: &mut ChildSpawnerCommands,
    flute: &Flute,
    scale: f32,
    slot: ChartSlot,
    axis: ChartAxis,
) {
    let tube_at = 30.0 * scale;
    let tube_w = 26.0 * scale;
    let hole = 22.0 * scale;
    let side_key = 12.0 * scale;
    let thickness = 92.0 * scale;
    let border = (2.0 * scale).round().max(2.0);
    let layout = InstrumentModel::lane_layout(flute);
    let vertical = axis == ChartAxis::Vertical;
    // Across the tube, in px from the chart's left (upright) or top (lying
    // down) edge. Lying down is the upright chart turned a quarter
    // anticlockwise, so its left side (the thumb) ends up below.
    let thumb_side = tube_at - side_key - 6.0 * scale;
    let key_side = tube_at + tube_w + 6.0 * scale;
    let (before, after) = if vertical { (thumb_side, key_side) } else { (key_side, thumb_side) };
    let full = Val::Percent(100.0);
    parent
        .spawn((
            FluteDrawing(*flute),
            slot,
            Node {
                width: if vertical { Val::Px(thickness) } else { full },
                height: if vertical { full } else { Val::Px(thickness) },
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|drawing| {
            // The tube, behind the keys.
            let (left, top, width, height) = if vertical {
                (Val::Px(tube_at), Val::Auto, Val::Px(tube_w), full)
            } else {
                (Val::Auto, Val::Px(tube_at), full, Val::Px(tube_w))
            };
            drawing.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left,
                    top,
                    width,
                    height,
                    border_radius: BorderRadius::all(Val::Px(8.0 * scale)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.33, 0.38)),
            ));
            for (row, key_row) in key_rows(flute.foot).into_iter().enumerate() {
                let Some(key) = key_row.key else { continue };
                let (along, length) = layout.span(row);
                let (size, across) = match key_row.side {
                    Side::Tube => (hole, tube_at + (tube_w - hole) * 0.5),
                    Side::Left => (side_key, before),
                    Side::Right => (side_key, after),
                };
                let cell = if vertical {
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(along),
                        height: Val::Percent(length),
                        left: Val::Px(across),
                        align_items: AlignItems::Center,
                        ..default()
                    }
                } else {
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(along),
                        width: Val::Percent(length),
                        top: Val::Px(across),
                        justify_content: JustifyContent::Center,
                        ..default()
                    }
                };
                drawing.spawn(cell).with_children(|cell| {
                    cell.spawn((
                        FluteKey { key, slot },
                        Node {
                            width: Val::Px(size),
                            height: Val::Px(size),
                            border: UiRect::all(Val::Px(border)),
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

/// The Note Trainer's chart, at its natural size.
pub(super) fn spawn_flute(parent: &mut ChildSpawnerCommands, flute: &Flute) {
    spawn_flute_chart(parent, flute, 1.0, ChartSlot::Now, ChartAxis::Vertical);
}

/// A drawn key's fill: solid for the note at hand, a ghost for the one
/// after, open otherwise.
fn key_fill(now: bool, next: bool) -> Color {
    if now {
        KEY_NOW
    } else if next {
        Color::srgba(1.0, 1.0, 1.0, 0.3)
    } else {
        Color::NONE
    }
}

/// Fills the Note Trainer's chart: solid for `now`'s fingering, ghosted for
/// `next`'s.
pub(super) fn paint_flute_keys(
    flute: &Flute,
    now: Option<FingeringAssignment>,
    next: Option<FingeringAssignment>,
    keys: &mut Query<(&FluteKey, &mut BackgroundColor)>,
) {
    let pressed = |at: Option<FingeringAssignment>, key| {
        at.and_then(|at| flute.keys(at)).is_some_and(|k| k.pressed(key))
    };
    for (key, mut fill) in keys.iter_mut().filter(|(key, _)| key.slot == ChartSlot::Now) {
        let want = key_fill(pressed(now, key.key), pressed(next, key.key));
        if fill.0 != want {
            fill.0 = want;
        }
    }
}

/// The fingering strip under the highway: the chart for the note to play
/// now, its name before it and its countdown bar under it, and beside it a
/// smaller, dimmer chart for the next one, each lying along the screen with
/// the head joint at the left. Hidden until [`sync_flute_view`] shows it.
pub(super) fn spawn_flute_stage(
    parent: &mut ChildSpawnerCommands,
    flute: &Flute,
    loc: &Localization,
) {
    const NOW_SCALE: f32 = 1.6;
    const NOW_LENGTH: f32 = 560.0;
    const NEXT_SCALE: f32 = 1.0;
    const NEXT_LENGTH: f32 = 360.0;
    // Wide enough for the longest name ("C#6"), so a chart never shifts as
    // the names change.
    const NAME_W: f32 = 120.0;
    let name_cell = || Node {
        width: Val::Px(NAME_W),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::FlexEnd,
        flex_shrink: 0.0,
        ..default()
    };
    parent
        .spawn((
            FluteStage(*flute),
            Node {
                width: Val::Percent(100.0),
                display: Display::None,
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(24.0),
                padding: UiRect::vertical(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.09)),
        ))
        .with_children(|strip| {
            strip.spawn(name_cell()).with_children(|cell| {
                cell.spawn((
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(52.0), ..default() },
                    TextColor(Color::WHITE),
                    StageNoteName(ChartSlot::Now),
                ));
            });
            strip
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|column| {
                    column.spawn(Node { width: Val::Px(NOW_LENGTH), ..default() }).with_children(
                        |c| {
                            spawn_flute_chart(
                                c,
                                flute,
                                NOW_SCALE,
                                ChartSlot::Now,
                                ChartAxis::Horizontal,
                            );
                        },
                    );
                    column
                        .spawn((
                            Node {
                                width: Val::Px(NOW_LENGTH),
                                height: Val::Px(10.0),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.19, 0.24)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(COUNTDOWN_COMING),
                                StageCountdown,
                            ));
                        });
                });
            // Space between the two, so the next chart reads as apart.
            strip.spawn(Node { width: Val::Px(40.0), ..default() });
            strip.spawn(name_cell()).with_children(|cell| {
                cell.spawn((
                    Text::new(String::from(loc.msg("gameplay-flute-next"))),
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(Color::srgb(0.62, 0.66, 0.74)),
                ));
                cell.spawn((
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(30.0), ..default() },
                    TextColor(Color::srgb(0.70, 0.74, 0.82)),
                    StageNoteName(ChartSlot::Next),
                ));
            });
            strip
                .spawn(Node { width: Val::Px(NEXT_LENGTH), flex_shrink: 0.0, ..default() })
                .with_children(|c| {
                    spawn_flute_chart(c, flute, NEXT_SCALE, ChartSlot::Next, ChartAxis::Horizontal);
                });
        });
}

/// The countdown bar before its note starts.
const COUNTDOWN_COMING: Color = Color::srgb(0.45, 0.70, 1.0);

/// The note the strip is asking for, as an index into `SongNotes`, for the
/// staff to draw in the highlight colour. `None` while the strip is hidden.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct StaffHighlight(pub Option<usize>);

/// Paints the strip from the notes and the clock: each chart's keys, the
/// two names, the countdown bar, and which note the staff highlights.
pub(super) fn update_flute_stage(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    mine: Res<MyInstruments>,
    stage: Query<&FluteStage>,
    mut keys: Query<(&FluteKey, &mut BackgroundColor)>,
    mut names: Query<(&StageNoteName, &mut Text)>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), (With<StageCountdown>, Without<FluteKey>)>,
    mut highlight: ResMut<StaffHighlight>,
) {
    let Ok(FluteStage(flute)) = stage.single() else { return };
    let shown = mine.flute_view == FluteView::Fingering;
    let now = clock.get();
    let (current, next) = stage_notes(&notes.notes, now);
    let want = StaffHighlight(current.filter(|_| shown));
    if *highlight != want {
        *highlight = want;
    }
    if !shown {
        return;
    }
    let current = current.map(|i| &notes.notes[i]);
    let next = next.map(|i| &notes.notes[i]);
    let (fill, sounding) = current.map_or((0.0, false), |n| countdown(n, now));
    let held = sounding && current.is_some_and(|n| n.hit);
    let pressed = |note: Option<&ScheduledNote>, key: u16| {
        note.and_then(fingering).and_then(|at| flute.keys(at)).is_some_and(|k| k.pressed(key))
    };
    for (key, mut colour) in &mut keys {
        let want = match key.slot {
            ChartSlot::Now if pressed(current, key.key) => {
                if held {
                    KEY_HELD
                } else {
                    KEY_NOW
                }
            }
            ChartSlot::Next if pressed(next, key.key) => KEY_NEXT,
            _ => Color::NONE,
        };
        if colour.0 != want {
            colour.0 = want;
        }
    }
    let name = |note: Option<&ScheduledNote>| {
        note.and_then(fingering)
            .map_or_else(String::new, |at| harmonicon_core::midi::midi_to_note(i32::from(at.note)))
    };
    for (slot, mut text) in &mut names {
        let want = name(if slot.0 == ChartSlot::Now { current } else { next });
        if text.0 != want {
            text.0 = want;
        }
    }
    for (mut node, mut colour) in &mut bars {
        let width = Val::Percent(fill * 100.0);
        if node.width != width {
            node.width = width;
        }
        let want = if sounding { KEY_HELD } else { COUNTDOWN_COMING };
        if colour.0 != want {
            colour.0 = want;
        }
    }
}

/// Lays a flute part out on its first frame, and shows or hides the
/// fingering strip whenever the player's [`FluteView`] changes: pitch rows
/// on the highway for the notes the song uses, and the strip under it in
/// `Fingering`. Keyed on the strip's entity as well as the view, since a
/// `Local` outlives the song and the next flute part still has to be laid
/// out.
pub(super) fn sync_flute_view(
    mut last: Local<Option<(Entity, FluteView)>>,
    mine: Res<MyInstruments>,
    notes: Res<SongNotes>,
    mut lanes: ResMut<super::HighwayLanes>,
    mut stage: Query<(Entity, &mut Node), With<FluteStage>>,
    furniture: Query<(Entity, Option<&Children>), With<super::highway_2d::HighwayFurniture>>,
    visuals: Query<Entity, With<super::NoteVisual>>,
    mut commands: Commands,
) {
    let Ok((entity, mut node)) = stage.single_mut() else {
        *last = None;
        return;
    };
    let view = mine.flute_view;
    if *last == Some((entity, view)) {
        return;
    }
    let first = last.is_none_or(|(seen, _)| seen != entity);
    *last = Some((entity, view));
    node.display = if view == FluteView::Fingering { Display::Flex } else { Display::None };
    if !first {
        return;
    }
    let used = notes.notes.iter().filter_map(|n| n.expected_pitch).collect();
    let rows = LaneLayout::pitch_rows(&used);
    for (container, children) in &furniture {
        for &child in children.into_iter().flatten() {
            commands.entity(child).despawn();
        }
        commands.entity(container).with_children(|f| {
            super::highway_2d::spawn_highway(f, &rows, super::highway_2d::RowStyle::Pitches);
        });
    }
    // Spawned against the instrument's own lanes before this ran.
    for visual in &visuals {
        commands.entity(visual).despawn();
    }
    lanes.0 = Some(rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time: f64, midi: u8, alternate: u8) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.5,
            playable: true,
            position: PlayPosition::Fingering(FingeringAssignment { note: midi, alternate }),
            ..default()
        }
    }

    #[test]
    fn the_stage_shows_the_note_at_hand_then_the_next_and_rewinds() {
        let notes = vec![note(1.0, 64, 0), note(2.0, 67, 0), note(3.0, 86, 1)];
        assert_eq!(stage_notes(&notes, -9.0), (Some(0), Some(1)), "the first, however far off");
        assert_eq!(stage_notes(&notes, 1.2), (Some(0), Some(1)), "held through its duration");
        assert_eq!(stage_notes(&notes, 2.6), (Some(2), None), "the last has no next");
        assert_eq!(stage_notes(&notes, 9.0), (None, None), "the song is over");
        assert_eq!(stage_notes(&notes, 0.0), (Some(0), Some(1)), "a loop back reads afresh");
    }

    #[test]
    fn a_missed_note_hands_the_stage_to_the_next() {
        let mut notes = vec![note(1.0, 64, 0), note(2.0, 67, 0)];
        notes[0].missed = true;
        assert_eq!(stage_notes(&notes, 1.2), (Some(1), None));
    }

    /// The bar fills over the lead to the note's start, then empties
    /// through the note while it sounds.
    #[test]
    fn the_countdown_reads_get_ready_then_hold() {
        let n = note(10.0, 64, 0);
        assert_eq!(countdown(&n, 5.0), (0.0, false), "well before: empty");
        let (half, sounding) = countdown(&n, 10.0 - LEAD_SECS / 2.0);
        assert!((half - 0.5).abs() < 1e-6 && !sounding);
        assert_eq!(countdown(&n, 10.0), (1.0, true), "at the start: full, and sounding");
        let (rest, sounding) = countdown(&n, 10.25);
        assert!((rest - 0.5).abs() < 1e-6 && sounding, "half the note is left");
    }

    #[test]
    fn a_key_held_now_outranks_its_ghost() {
        assert_eq!(key_fill(true, true), KEY_NOW);
        assert_ne!(key_fill(false, true), Color::NONE);
        assert_eq!(key_fill(false, false), Color::NONE);
    }
}
