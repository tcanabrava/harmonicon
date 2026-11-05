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
    /// A snapshot of the previous fingering, retained until its exit ends.
    Outgoing,
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
/// first still to come) and the next different fingering, as indices into `notes`.
/// Computed from chart time, so a seek, a loop or an adaptive rebuild needs
/// no reset. A missed note hands the stage straight to the next.
fn stage_notes(notes: &[ScheduledNote], now: f64, flute: &Flute) -> (Option<usize>, Option<usize>) {
    let is_fingering = |n: &ScheduledNote| matches!(n.position, PlayPosition::Fingering(_));
    let mut ahead = notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.playable && !n.missed && n.time + n.duration > now && is_fingering(n));
    let Some((current, note)) = ahead.next() else { return (None, None) };
    let next =
        ahead.find(|(_, n)| n.time > note.time && !same_fingering(flute, note, n)).map(|(i, _)| i);
    (Some(current), next)
}

fn same_fingering(flute: &Flute, a: &ScheduledNote, b: &ScheduledNote) -> bool {
    match (fingering(a), fingering(b)) {
        (Some(a), Some(b)) => match (flute.keys(a), flute.keys(b)) {
            (Some(a), Some(b)) => a == b,
            _ => a == b,
        },
        _ => false,
    }
}

#[derive(Component)]
pub(super) struct StageRow(ChartSlot);

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
    /// Upright, the head joint at the top.
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
            UiTransform::IDENTITY,
            slot,
            Node {
                width: if vertical { Val::Px(thickness) } else { full },
                height: if vertical { full } else { Val::Px(thickness) },
                flex_shrink: 0.0,
                position_type: if slot == ChartSlot::Outgoing {
                    PositionType::Absolute
                } else {
                    PositionType::Relative
                },
                left: if slot == ChartSlot::Outgoing { Val::Px(0.0) } else { Val::Auto },
                top: if slot == ChartSlot::Outgoing { Val::Px(0.0) } else { Val::Auto },
                ..default()
            },
            if slot == ChartSlot::Outgoing { Visibility::Hidden } else { Visibility::Inherited },
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
                BackgroundColor(Color::srgb(0.40, 0.47, 0.54)),
            ));
            // A silver highlight and joint collars give the tube a readable
            // instrument silhouette without obscuring the fingering circles.
            for along in [2.0, 14.0, 83.0, 97.0] {
                drawing.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: if vertical {
                            Val::Px(tube_at - 3.0 * scale)
                        } else {
                            Val::Percent(along)
                        },
                        top: if vertical {
                            Val::Percent(along)
                        } else {
                            Val::Px(tube_at - 3.0 * scale)
                        },
                        width: if vertical {
                            Val::Px(tube_w + 6.0 * scale)
                        } else {
                            Val::Px(4.0 * scale)
                        },
                        height: if vertical {
                            Val::Px(4.0 * scale)
                        } else {
                            Val::Px(tube_w + 6.0 * scale)
                        },
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.72, 0.78, 0.83)),
                ));
            }
            drawing.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: if vertical { Val::Px(tube_at + 5.0 * scale) } else { Val::Percent(6.0) },
                    top: if vertical { Val::Percent(6.0) } else { Val::Px(tube_at + 5.0 * scale) },
                    width: Val::Px(16.0 * scale),
                    height: Val::Px(16.0 * scale),
                    border: UiRect::all(Val::Px(3.0 * scale)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.85, 0.89, 0.93)),
                BackgroundColor(Color::srgb(0.07, 0.10, 0.14)),
            ));
            for (row, key_row) in key_rows(flute.foot).into_iter().enumerate() {
                let Some(key) = key_row.key else { continue };
                let (along, length) = layout.span(row);
                let (along, length) = (16.0 + along * 0.80, length * 0.80);
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
                            width: Val::Px(if key_row.side == Side::Tube || vertical {
                                size
                            } else {
                                size * 1.7
                            }),
                            height: Val::Px(if key_row.side == Side::Tube || !vertical {
                                size
                            } else {
                                size * 1.7
                            }),
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
/// now, its name before it and its countdown bar under it, and above it a
/// smaller, dimmer chart for the next one, each lying along the screen with
/// the head joint at the left. Hidden until [`sync_flute_view`] shows it.
pub(super) fn spawn_flute_stage(
    parent: &mut ChildSpawnerCommands,
    flute: &Flute,
    loc: &Localization,
) {
    parent
        .spawn((
            FluteStage(*flute),
            Node {
                overflow: Overflow::clip(),
                width: Val::Percent(100.0),
                display: Display::None,
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.08, 0.11)),
        ))
        .with_children(|strip| {
            // The preview sits above the current chart, on the same axis:
            // eyes and hands can compare changes without scanning sideways.
            for (slot, scale) in [(ChartSlot::Next, 0.75), (ChartSlot::Now, 1.25)] {
                strip
                    .spawn((
                        StageRow(slot),
                        Visibility::Inherited,
                        Node {
                            width: Val::Percent(100.0),
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(16.0),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn(Node {
                            width: Val::Px(104.0),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|name| {
                            if slot == ChartSlot::Next {
                                name.spawn((
                                    Text::new(String::from(loc.msg("gameplay-flute-next"))),
                                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                                    TextColor(Color::srgb(0.65, 0.73, 0.83)),
                                ));
                            }
                            name.spawn((
                                StageNoteName(slot),
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(if slot == ChartSlot::Now {
                                        44.0
                                    } else {
                                        26.0
                                    }),
                                    ..default()
                                },
                                TextColor(if slot == ChartSlot::Now {
                                    Color::WHITE
                                } else {
                                    KEY_NEXT
                                }),
                            ));
                        });
                        row.spawn(Node {
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            min_width: Val::Px(0.0),
                            ..default()
                        })
                        .with_children(|c| {
                            c.spawn(Node {
                                width: Val::Percent(if slot == ChartSlot::Next {
                                    60.0
                                } else {
                                    100.0
                                }),
                                ..default()
                            })
                            .with_children(|chart| {
                                spawn_flute_chart(chart, flute, scale, slot, ChartAxis::Horizontal);
                                if slot == ChartSlot::Now {
                                    spawn_flute_chart(
                                        chart,
                                        flute,
                                        scale,
                                        ChartSlot::Outgoing,
                                        ChartAxis::Horizontal,
                                    );
                                }
                            });
                        });
                    });
            }
            strip
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::MAX,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.23, 0.30)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        StageCountdown,
                        Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                        BackgroundColor(COUNTDOWN_COMING),
                    ));
                });
        });
}

/// The countdown bar before its note starts.
const COUNTDOWN_COMING: Color = Color::srgb(0.45, 0.70, 1.0);

/// The note the strip is asking for, as an index into `SongNotes`, for the
/// staff to draw in the highlight colour. `None` while the strip is hidden.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct StaffHighlight(pub Option<usize>);

/// Motion is presentation-only: chart time still selects and scores notes.
#[derive(Default)]
pub(super) struct StageMotion {
    seen: Option<(Entity, f64)>,
    current: Option<usize>,
    next: Option<usize>,
    elapsed: f32,
    duration: f32,
    outgoing: Option<ScheduledNote>,
    snapshot: Option<ScheduledNote>,
    promote: bool,
}

/// Quintic easing starts and finishes with zero velocity and acceleration.
fn motion_ease(progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn promotion_transform(progress: f32) -> UiTransform {
    let remaining = 1.0 - motion_ease(progress);
    UiTransform {
        translation: Val2::new(Val::Percent(-20.0 * remaining), Val::Px(-96.0 * remaining)),
        scale: Vec2::splat(1.0 - 0.4 * remaining),
        ..UiTransform::IDENTITY
    }
}

fn exit_transform(progress: f32) -> UiTransform {
    UiTransform::from_translation(Val2::new(Val::Px(0.0), Val::Px(160.0 * motion_ease(progress))))
}

/// Paints the strip from the notes and the clock: each chart's keys, the
/// two names, the countdown bar, and which note the staff highlights.
pub(super) fn update_flute_stage(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    mine: Res<MyInstruments>,
    stage: Query<(Entity, &FluteStage)>,
    time: Res<Time>,
    mut motion: Local<StageMotion>,
    mut drawings: Query<(&ChartSlot, &mut UiTransform, &mut Visibility), With<FluteDrawing>>,
    mut rows: Query<(&StageRow, &mut Visibility), Without<FluteDrawing>>,
    mut keys: Query<(&FluteKey, &mut BackgroundColor)>,
    mut names: Query<(&StageNoteName, &mut Text)>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), (With<StageCountdown>, Without<FluteKey>)>,
    mut highlight: ResMut<StaffHighlight>,
) {
    let Ok((entity, FluteStage(flute))) = stage.single() else { return };
    let shown = mine.flute_view == FluteView::Fingering;
    let now = clock.get();
    let (current, next) = stage_notes(&notes.notes, now, flute);
    let continuous = motion
        .seen
        .is_some_and(|(seen, previous)| seen == entity && now >= previous && now - previous < 0.5);
    if !shown || !continuous {
        motion.elapsed = 1.0;
        motion.duration = 0.36;
        motion.outgoing = None;
        motion.promote = false;
    } else if current != motion.current
        && !motion
            .snapshot
            .as_ref()
            .zip(current.map(|i| &notes.notes[i]))
            .is_some_and(|(previous, current)| same_fingering(flute, previous, current))
    {
        motion.outgoing = motion.snapshot.clone();
        motion.promote = current.is_some() && current == motion.next;
        motion.elapsed = 0.0;
        // Finish within the note interval so fast passages don't pile up.
        let interval = current
            .zip(next)
            .map(|(a, b)| notes.notes[b].time - notes.notes[a].time)
            .unwrap_or(0.5);
        motion.duration = (interval as f32 * 0.75).clamp(0.08, 0.36);
    } else {
        motion.elapsed += time.delta_secs();
    }
    motion.seen = shown.then_some((entity, now));
    motion.current = current;
    motion.next = next;
    motion.snapshot = current.map(|i| notes.notes[i].clone());
    let progress = (motion.elapsed / motion.duration.max(0.08)).clamp(0.0, 1.0);
    for (row, mut visibility) in &mut rows {
        *visibility = if row.0 == ChartSlot::Next && next.is_none() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    for (slot, mut transform, mut visibility) in &mut drawings {
        let visible = match slot {
            ChartSlot::Now => current.is_some(),
            ChartSlot::Next => next.is_some() && (!motion.promote || progress >= 0.45),
            ChartSlot::Outgoing => motion.outgoing.is_some() && progress < 1.0,
        };
        *visibility = if visible && shown { Visibility::Inherited } else { Visibility::Hidden };
        *transform = match slot {
            ChartSlot::Now if motion.promote => promotion_transform(progress),
            ChartSlot::Now => UiTransform::IDENTITY,
            ChartSlot::Outgoing => exit_transform(progress),
            ChartSlot::Next => UiTransform::from_translation(Val2::new(
                Val::Px(0.0),
                Val::Px(-36.0 * (1.0 - motion_ease((progress - 0.45) / 0.55))),
            )),
        };
    }
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
            ChartSlot::Outgoing if pressed(motion.outgoing.as_ref(), key.key) => {
                if motion.outgoing.as_ref().is_some_and(|n| n.hit) { KEY_HELD } else { KEY_NOW }
            }
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

/// Keeps the flute's pitch rows in sync with the currently unlocked notes,
/// including coverage changes while paused. Rebuilds furniture and ribbons
/// only when the pitch set changes; ordinary scoring updates keep the layout.
/// The view preference separately controls the fingering strip's visibility.
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
    if *last == Some((entity, view)) && !notes.is_changed() {
        return;
    }
    let first = last.is_none_or(|(seen, _)| seen != entity);
    *last = Some((entity, view));
    let display = if view == FluteView::Fingering { Display::Flex } else { Display::None };
    if node.display != display {
        node.display = display;
    }
    let used = notes.notes.iter().filter_map(|n| n.expected_pitch).collect();
    let rows = LaneLayout::pitch_rows(&used);
    if !first && lanes.0.as_ref() == Some(&rows) {
        return;
    }
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

    #[test]
    fn coverage_changes_rebuild_pitch_rows_without_toggling_flute_view() {
        let mut app = App::new();
        app.init_resource::<MyInstruments>()
            .init_resource::<SongNotes>()
            .init_resource::<super::super::HighwayLanes>()
            .add_systems(Update, sync_flute_view);
        app.world_mut().spawn((FluteStage(Flute::default()), Node::default()));
        let furniture = app
            .world_mut()
            .spawn((super::super::highway_2d::HighwayFurniture, Node::default()))
            .id();
        let mut first = note(1.0, 64, 0);
        first.expected_pitch = Some(64);
        app.world_mut().resource_mut::<SongNotes>().notes = vec![first];
        app.update();
        let first_layout = app.world().resource::<super::super::HighwayLanes>().0.clone().unwrap();
        assert_eq!(first_layout.lanes.len(), 1);
        let mut added = note(2.0, 67, 0);
        added.expected_pitch = Some(67);
        app.world_mut().resource_mut::<SongNotes>().notes.push(added);
        let old_visual = app.world_mut().spawn(super::super::NoteVisual { note_id: 0 }).id();
        app.update();
        let layout = app.world().resource::<super::super::HighwayLanes>().0.as_ref().unwrap();
        assert_eq!(
            layout.lanes.iter().map(|lane| lane.pitch).collect::<Vec<_>>(),
            vec![Some(67), Some(64)]
        );
        assert!(
            app.world().get_entity(old_visual).is_err(),
            "ribbons must respawn against updated rows"
        );
        let children = app.world().get::<Children>(furniture).unwrap().to_vec();
        app.world_mut().resource_mut::<SongNotes>().notes[0].hit = true;
        app.update();
        assert_eq!(
            app.world().get::<Children>(furniture).unwrap().to_vec(),
            children,
            "ordinary hit updates must not rebuild the highway"
        );
        app.world_mut().resource_mut::<SongNotes>().notes.pop();
        app.update();
        assert_eq!(
            app.world().resource::<super::super::HighwayLanes>().0.as_ref().unwrap(),
            &first_layout
        );
    }

    #[test]
    fn preview_skips_repeated_fingerings_without_skipping_attacks() {
        let flute = Flute::default();
        let notes = vec![note(1.0, 64, 0), note(2.0, 64, 0), note(3.0, 67, 0)];
        assert_eq!(stage_notes(&notes, 1.1, &flute), (Some(0), Some(2)));
        assert_eq!(stage_notes(&notes, 2.1, &flute), (Some(1), Some(2)));
        assert_eq!(stage_notes(&notes[..2], 1.1, &flute), (Some(0), None));
        assert_eq!(stage_notes(&notes, 0.0, &flute), (Some(0), Some(2)));
    }

    #[test]
    fn previous_fingering_survives_until_its_exit_completes() {
        let mut app = App::new();
        app.init_resource::<GameplayClock>()
            .init_resource::<SongNotes>()
            .init_resource::<Time>()
            .init_resource::<MyInstruments>()
            .init_resource::<StaffHighlight>()
            .add_systems(Update, update_flute_stage);
        app.world_mut().resource_mut::<MyInstruments>().flute_view = FluteView::Fingering;
        app.world_mut().resource_mut::<SongNotes>().notes =
            vec![note(1.0, 64, 0), note(2.0, 67, 0)];
        let flute = Flute::default();
        let loc = Localization::default();
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|p| spawn_flute_stage(p, &flute, &loc));
        app.world_mut().flush();
        app.world_mut().resource_mut::<GameplayClock>().set_free(1.4);
        app.update();
        app.world_mut().resource_mut::<GameplayClock>().set_free(1.6);
        app.update();
        let world = app.world_mut();
        let outgoing = world
            .query::<(&ChartSlot, &Visibility, &UiTransform)>()
            .iter(world)
            .find(|(slot, _, _)| **slot == ChartSlot::Outgoing)
            .unwrap();
        assert_eq!(*outgoing.1, Visibility::Inherited);
        assert_eq!(outgoing.2.translation, Val2::ZERO, "exit starts at old position");
        let old_keys = flute.keys(FingeringAssignment { note: 64, alternate: 0 }).unwrap();
        for (key, color) in world
            .query::<(&FluteKey, &BackgroundColor)>()
            .iter(world)
            .filter(|(key, _)| key.slot == ChartSlot::Outgoing)
        {
            assert_eq!(color.0, if old_keys.pressed(key.key) { KEY_NOW } else { Color::NONE });
        }
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(400));
        app.world_mut().resource_mut::<GameplayClock>().set_free(1.7);
        app.update();
        let world = app.world_mut();
        let outgoing = world
            .query::<(&ChartSlot, &Visibility)>()
            .iter(world)
            .find(|(slot, _)| **slot == ChartSlot::Outgoing)
            .unwrap();
        assert_eq!(*outgoing.1, Visibility::Hidden, "hide only after sliding out");
        app.world_mut().resource_mut::<GameplayClock>().set_free(0.0);
        app.update();
        let world = app.world_mut();
        let outgoing = world
            .query::<(&ChartSlot, &Visibility)>()
            .iter(world)
            .find(|(slot, _)| **slot == ChartSlot::Outgoing)
            .unwrap();
        assert_eq!(*outgoing.1, Visibility::Hidden, "seeks clear stale outgoing charts");
        app.world_mut().resource_mut::<SongNotes>().notes[1] = note(2.0, 64, 0);
        app.world_mut().resource_mut::<GameplayClock>().set_free(1.4);
        app.update();
        app.world_mut().resource_mut::<GameplayClock>().set_free(1.6);
        app.update();
        let world = app.world_mut();
        let preview = world
            .query::<(&StageRow, &Visibility)>()
            .iter(world)
            .find(|(row, _)| row.0 == ChartSlot::Next)
            .unwrap();
        assert_eq!(*preview.1, Visibility::Hidden, "hide preview heading as well as drawing");
        let outgoing = world
            .query::<(&ChartSlot, &Visibility)>()
            .iter(world)
            .find(|(slot, _)| **slot == ChartSlot::Outgoing)
            .unwrap();
        assert_eq!(*outgoing.1, Visibility::Hidden, "repeated fingering stays in place");
    }

    #[test]
    fn easing_and_exit_preserve_continuity() {
        assert_eq!(motion_ease(0.0), 0.0);
        assert_eq!(motion_ease(1.0), 1.0);
        assert!(motion_ease(0.01) < 0.0001, "gentle acceleration");
        assert!(1.0 - motion_ease(0.99) < 0.0001, "gentle settling");
        assert_eq!(exit_transform(0.0).translation, Val2::ZERO);
        assert_eq!(exit_transform(1.0).translation.y, Val::Px(160.0));
        for step in 1..=100 {
            assert!(motion_ease(step as f32 / 100.0) >= motion_ease((step - 1) as f32 / 100.0));
        }
    }

    #[test]
    fn promotion_starts_at_preview_size_and_settles_without_overshoot() {
        let start = promotion_transform(0.0);
        assert_eq!(start.scale, Vec2::splat(0.6));
        assert_eq!(start.translation.y, Val::Px(-96.0));
        let end = promotion_transform(1.0);
        assert_eq!(end.scale, Vec2::ONE);
        assert_eq!(end.translation, Val2::ZERO);
        assert_eq!(promotion_transform(2.0).scale, Vec2::ONE);
        assert!(promotion_transform(0.5).scale.x > start.scale.x);
    }

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
        assert_eq!(
            stage_notes(&notes, -9.0, &Flute::default()),
            (Some(0), Some(1)),
            "the first, however far off"
        );
        assert_eq!(
            stage_notes(&notes, 1.2, &Flute::default()),
            (Some(0), Some(1)),
            "held through its duration"
        );
        assert_eq!(
            stage_notes(&notes, 2.6, &Flute::default()),
            (Some(2), None),
            "the last has no next"
        );
        assert_eq!(stage_notes(&notes, 9.0, &Flute::default()), (None, None), "the song is over");
        assert_eq!(
            stage_notes(&notes, 0.0, &Flute::default()),
            (Some(0), Some(1)),
            "a loop back reads afresh"
        );
    }

    #[test]
    fn a_missed_note_hands_the_stage_to_the_next() {
        let mut notes = vec![note(1.0, 64, 0), note(2.0, 67, 0)];
        notes[0].missed = true;
        assert_eq!(stage_notes(&notes, 1.2, &Flute::default()), (Some(1), None));
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
