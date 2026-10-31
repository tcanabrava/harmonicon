// SPDX-License-Identifier: MIT

//! The guitar's timeline: the Song Editor grid's shape for a tab. One lane
//! per string, string 1 at the top as tab writes it; a note is a block as
//! long as it lasts, in its string's colour, labelled as tab writes it; a
//! beat and bar ruler from the editor's one meter map; and the playhead
//! sweeping across while the song plays. It shares the harmonica grid's
//! scale (`TICK_W`, `ROW_H`), so the two views read alike.
//!
//! The view is rebuilt whenever the document changes, so the horizontal
//! scroll is remembered across rebuilds ([`TimelineScroll`]); otherwise
//! every edit would throw the author back to the first bar.

use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, ScrollPosition};
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::instrument::guitar::Guitar;
use harmonicon_core::snap::{SnapMode, snap_absolute_tick};
use harmonicon_gameplay::gameplay::guitar_visuals::string_color;

use super::guitar_editor::{Action, note_block_button};
use super::playback::Playhead;
use super::state::{EditorState, GridNote};
use super::{NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};

/// Height of the beat and bar ruler above the lanes.
const RULER_H: f32 = 22.0;
/// Width of the string-name column left of the lanes.
const LABEL_W: f32 = 44.0;
/// Bars of empty timeline past the last note, room to keep writing.
const SPARE_BARS: usize = 4;
/// The shortest timeline drawn, in bars, so an empty song has a page to
/// start on.
const MIN_BARS: usize = 16;

/// The timeline's horizontal scroll, carried across rebuilds.
#[derive(Resource, Default)]
pub(super) struct TimelineScroll(pub(super) f32);

/// The scrolling node whose offset [`TimelineScroll`] remembers.
#[derive(Component)]
pub(super) struct TimelineScroller;

/// One string's lane; a click on it moves the cursor there.
#[derive(Component)]
pub(super) struct Lane {
    pub(super) string: u8,
    pub(super) total_ticks: usize,
}

/// The line that follows the song while it plays.
#[derive(Component)]
pub(super) struct GuitarPlayhead;

/// Where a note's block sits inside the lanes, in px: `(left, top, width,
/// height)`. String 1 is the top lane. The same arithmetic as the harmonica
/// grid's `note_rect`, so a note of the same length is the same width in
/// both views.
pub(super) fn block_rect(string: u8, tick: usize, len: usize) -> (f32, f32, f32, f32) {
    let lane = f32::from(string.saturating_sub(1));
    (
        tick as f32 * TICK_W + 1.0,
        RULER_H + lane * ROW_H + NOTE_PAD,
        (len as f32 * TICK_W - 2.0).max(TICK_W),
        ROW_H - 2.0 * NOTE_PAD,
    )
}

/// How many ticks the timeline spans: past the last note by
/// [`SPARE_BARS`], at least [`MIN_BARS`], always whole bars.
pub(super) fn timeline_ticks(last_end: usize, ticks_per_bar: usize) -> usize {
    let bar = ticks_per_bar.max(1);
    let bars = last_end.div_ceil(bar) + SPARE_BARS;
    bars.max(MIN_BARS) * bar
}

/// The tick a click lands on: `fraction` of the way across a timeline of
/// `total_ticks`, snapped to the editor's grid as a harmonica click is.
pub(super) fn click_tick(fraction: f32, total_ticks: usize, snap: SnapMode) -> usize {
    let raw = (fraction.clamp(0.0, 1.0) * total_ticks as f32) as usize;
    snap_absolute_tick(raw.min(total_ticks.saturating_sub(1)), snap)
}

/// Spawns the timeline under `parent` for `guitar`'s strings and `state`'s
/// notes, scrolled to `scroll_x`.
pub(super) fn spawn(
    parent: &mut ChildSpawnerCommands,
    state: &EditorState,
    guitar: &Guitar,
    scroll_x: f32,
) {
    let meter = state.meter_map();
    let ticks_per_bar = meter
        .meter_at(0)
        .ticks_per_bar(TICKS_PER_BEAT as u32)
        .map_or(4 * TICKS_PER_BEAT, |t| t as usize);
    let last_end = state.notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    let total_ticks = timeline_ticks(last_end, ticks_per_bar);
    let width = total_ticks as f32 * TICK_W;
    let strings = guitar.string_count();
    let height = RULER_H + f32::from(strings) * ROW_H;

    parent
        .spawn(Node { flex_direction: FlexDirection::Row, width: Val::Percent(100.0), ..default() })
        .with_children(|row| {
            // String names stay put while the lanes scroll.
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                padding: UiRect::top(Val::Px(RULER_H)),
                ..default()
            })
            .with_children(|col| {
                for (i, label) in guitar.string_labels().into_iter().enumerate() {
                    col.spawn((
                        Node {
                            height: Val::Px(ROW_H),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        children![(
                            Text::new(label),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(string_color(i as u8 + 1)),
                        )],
                    ));
                }
            });

            let area = harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy(
                row,
                Color::srgb(0.5, 0.6, 0.8),
                Color::srgb(0.1, 0.12, 0.18),
            );
            row.commands()
                .entity(area)
                .insert((TimelineScroller, ScrollPosition(Vec2::new(scroll_x, 0.0))))
                .with_children(|scroll| {
                    scroll
                        .spawn(Node {
                            position_type: PositionType::Relative,
                            width: Val::Px(width),
                            min_width: Val::Px(width),
                            height: Val::Px(height),
                            min_height: Val::Px(height),
                            flex_shrink: 0.0,
                            ..default()
                        })
                        .with_children(|canvas| {
                            spawn_lanes(canvas, strings, total_ticks, width);
                            spawn_ruler(canvas, &meter, total_ticks, height);
                            for note in &state.notes {
                                spawn_block(canvas, note, state.selected.contains(&note.id));
                            }
                            canvas.spawn((
                                GuitarPlayhead,
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(0.0),
                                    width: Val::Px(2.0),
                                    height: Val::Px(height),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(1.0, 0.85, 0.3)),
                                Visibility::Hidden,
                                Pickable::IGNORE,
                            ));
                        });
                });
        });
}

/// One clickable lane per string, alternately shaded like the harmonica
/// grid's rows.
fn spawn_lanes(canvas: &mut ChildSpawnerCommands, strings: u8, total_ticks: usize, width: f32) {
    for string in 1..=strings {
        let shade = if string % 2 == 0 { 0.07 } else { 0.10 };
        canvas
            .spawn((
                Lane { string, total_ticks },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(RULER_H + f32::from(string - 1) * ROW_H),
                    width: Val::Px(width),
                    height: Val::Px(ROW_H),
                    ..default()
                },
                BackgroundColor(Color::srgb(shade, shade + 0.01, shade + 0.03)),
                RelativeCursorPosition::default(),
            ))
            .observe(super::guitar_editor::on_lane_click);
        // The string itself, a thin line through the lane as tab draws it.
        canvas.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(RULER_H + f32::from(string - 1) * ROW_H + ROW_H * 0.5),
                width: Val::Px(width),
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(string_color(string).with_alpha(0.35)),
            Pickable::IGNORE,
        ));
    }
}

/// Beat lines, heavier bar lines and bar numbers, from the editor's meter
/// map so a meter change or a pickup lines up as it does on the harmonica
/// grid.
fn spawn_ruler(
    canvas: &mut ChildSpawnerCommands,
    meter: &harmonicon_ui::music_score::MeterMap,
    total_ticks: usize,
    height: f32,
) {
    let line = |canvas: &mut ChildSpawnerCommands, tick: u64, alpha: f32, w: f32| {
        canvas.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(tick as f32 * TICK_W),
                top: Val::Px(RULER_H),
                width: Val::Px(w),
                height: Val::Px(height - RULER_H),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, alpha)),
            Pickable::IGNORE,
        ));
    };
    for (tick, _) in meter.beat_starts(0, total_ticks as u64) {
        line(canvas, tick, 0.07, 1.0);
    }
    for (tick, bar) in meter.bar_starts(0, total_ticks as u64) {
        line(canvas, tick, 0.25, 2.0);
        if let Some(number) = meter.bar_number(bar) {
            canvas.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(tick as f32 * TICK_W + 3.0),
                    top: Val::Px(2.0),
                    ..default()
                },
                Text::new(number.to_string()),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.95, 0.75, 0.35)),
                Pickable::IGNORE,
            ));
        }
    }
}

/// A note's block, selectable, in its string's colour.
fn spawn_block(canvas: &mut ChildSpawnerCommands, note: &GridNote, selected: bool) {
    let Some(PlayPosition::Fret(at)) = note.target.position() else { return };
    let (left, top, width, height) = block_rect(at.string, note.tick, note.len);
    canvas
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                height: Val::Px(height),
                border: UiRect::all(Val::Px(if selected { 2.0 } else { 0.0 })),
                ..default()
            },
            BorderColor::all(Color::WHITE),
        ))
        .with_children(|cell| {
            note_block_button(
                cell,
                at.tab_label(),
                Action::Select(note.id),
                width,
                height,
                string_color(at.string),
            );
        });
}

/// Keeps [`TimelineScroll`] in step with the scroller, so the next rebuild
/// opens where the author was.
pub(super) fn remember_scroll(
    scroller: Query<&ScrollPosition, With<TimelineScroller>>,
    mut saved: ResMut<TimelineScroll>,
) {
    if let Ok(position) = scroller.single()
        && saved.0 != position.0.x
    {
        saved.0 = position.0.x;
    }
}

/// Moves the playhead with the song, and hides it when nothing plays.
pub(super) fn update_playhead(
    playhead: Res<Playhead>,
    mut line: Query<(&mut Node, &mut Visibility), With<GuitarPlayhead>>,
) {
    let Ok((mut node, mut visibility)) = line.single_mut() else { return };
    if !playhead.playing || playhead.secs_per_tick <= 0.0 {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    let left = Val::Px(playhead.elapsed / playhead.secs_per_tick * TICK_W);
    if node.left != left {
        node.left = left;
    }
    visibility.set_if_neq(Visibility::Inherited);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_spans_its_note_on_its_strings_lane() {
        let (left, top, width, height) = block_rect(3, TICKS_PER_BEAT, 2 * TICKS_PER_BEAT);
        assert_eq!(left, TICKS_PER_BEAT as f32 * TICK_W + 1.0);
        assert_eq!(top, RULER_H + 2.0 * ROW_H + NOTE_PAD);
        assert_eq!(width, 2.0 * TICKS_PER_BEAT as f32 * TICK_W - 2.0);
        assert_eq!(height, ROW_H - 2.0 * NOTE_PAD);
    }

    /// The same note is the same width as on the harmonica grid.
    #[test]
    fn a_block_is_as_wide_as_the_harmonica_grids_note() {
        let note = GridNote {
            id: 0,
            tick: 5,
            len: 7,
            expr: super::super::state::Expr::None,
            target: harmonicon_core::note_target::NoteTarget::Sounding {
                midi: 60.0,
                placement: None,
            },
        };
        let (left, _, width, _) = super::super::state::note_rect(&note, 0);
        let (block_left, _, block_width, _) = block_rect(1, 5, 7);
        assert_eq!((block_left, block_width), (left, width));
    }

    #[test]
    fn a_very_short_note_still_has_a_block_to_click() {
        let (_, _, width, _) = block_rect(1, 0, 0);
        assert!(width >= TICK_W);
    }

    #[test]
    fn the_timeline_runs_past_the_last_note_in_whole_bars() {
        let bar = 4 * TICKS_PER_BEAT;
        assert_eq!(timeline_ticks(0, bar), MIN_BARS * bar, "an empty song gets a page");
        assert_eq!(timeline_ticks(20 * bar, bar), (20 + SPARE_BARS) * bar);
        assert_eq!(
            timeline_ticks(20 * bar + 1, bar),
            (21 + SPARE_BARS) * bar,
            "a part-bar rounds up"
        );
    }

    #[test]
    fn a_click_lands_on_the_snapped_tick_under_it() {
        let total = 16 * 4 * TICKS_PER_BEAT;
        assert_eq!(click_tick(0.0, total, SnapMode::Sixteenth), 0);
        // A quarter of the way across, exactly on a beat.
        assert_eq!(click_tick(0.25, total, SnapMode::Sixteenth), total / 4);
        // Past either end clamps into the timeline.
        assert!(click_tick(1.5, total, SnapMode::Sixteenth) <= total);
        assert_eq!(click_tick(-1.0, total, SnapMode::Sixteenth), 0);
    }
}
