// SPDX-License-Identifier: MIT

//! Repeat signs and first/second-time endings, as the Song Editor authors
//! them: a timeline selection snapped to whole bars, then toggled into a
//! repeated passage or an ending.
//!
//! The editor keeps the score *as written* (`EditorState::repeats`, saved
//! to `timing.repeats`) and plays it that way too — only the song loader
//! plays the repeats out (`harmonicon_core::repeats::expand`). The editing
//! is pure, called by the timeline's Repeat and Ending buttons; [`spawn`]
//! draws the marks for `grid::rebuild_grid`.

use bevy::picking::Pickable;
use bevy::prelude::*;
use harmonicon_core::chart::{Ending, Repeat};
use harmonicon_ui::music_score::MeterMap;

use super::ui::GridItem;
use super::{TICK_W, WAVEFORM_TOP};

/// Repeat signs and ending brackets — apart from the tempo (orange), meter
/// (blue) and selection (yellow/red) marks the ruler already carries.
pub(super) const REPEAT_COLOR: Color = Color::srgb(0.80, 0.45, 0.95);

/// The most passes the Repeat button cycles to before removing the sign. The
/// format allows more; a tune repeated five times is rare enough to write
/// by hand.
pub(super) const MAX_EDITOR_PASSES: u32 = 4;

/// The bar line nearest `tick`. The song's start counts as one, so a
/// passage can repeat from the top of a tune that opens with a pickup.
pub(super) fn nearest_bar_line(map: &MeterMap, tick: u64) -> u64 {
    let bar = map.segment_at(tick).ticks_per_bar.max(1);
    std::iter::once(0)
        .chain(
            map.bar_starts(tick.saturating_sub(bar), tick + bar + 1)
                .into_iter()
                .map(|(start, _)| start),
        )
        .min_by_key(|start| start.abs_diff(tick))
        .unwrap_or(0)
}

/// A timeline selection as whole bars: each end moved to its nearest bar
/// line, and at least one bar long, since a repeat sign stands on a bar
/// line.
pub(super) fn bar_span(map: &MeterMap, start: usize, end: usize) -> (u64, u64) {
    let from = nearest_bar_line(map, start as u64);
    let to = nearest_bar_line(map, end as u64);
    if to > from {
        return (from, to);
    }
    let bar = map.segment_at(from).ticks_per_bar.max(1);
    let next =
        map.bar_starts(from + 1, from + 2 * bar + 1).first().map_or(from + bar, |&(tick, _)| tick);
    (from, next)
}

/// Endings inside a passage are taken on every pass but the last, and one
/// after its repeat sign on the last: first- and second-time bars, which is
/// what the editor writes. A chart may say otherwise, but re-editing a
/// repeat brings its endings back to this.
pub(super) fn fit_endings(repeat: &mut Repeat) {
    let passes = repeat.passes();
    for ending in &mut repeat.endings {
        ending.passes =
            if ending.end_tick <= repeat.end_tick { (1..passes).collect() } else { vec![passes] };
    }
}

/// The Repeat button on a selection of `start..end`. On a passage that is
/// already repeated it adds a pass, and past [`MAX_EDITOR_PASSES`] removes
/// the sign. Otherwise it repeats the passage twice, replacing any repeat
/// it overlaps, since the format has no nesting.
pub(super) fn toggle_repeat(repeats: &mut Vec<Repeat>, start: u64, end: u64) {
    if let Some(index) = repeats.iter().position(|r| r.start_tick == start && r.end_tick == end) {
        let passes = repeats[index].passes();
        if passes >= MAX_EDITOR_PASSES {
            repeats.remove(index);
        } else {
            repeats[index].times = Some(passes + 1);
            fit_endings(&mut repeats[index]);
        }
        return;
    }
    repeats.retain(|r| r.end_tick <= start || r.start_tick >= end);
    repeats.push(Repeat { start_tick: start, end_tick: end, times: None, endings: Vec::new() });
    repeats.sort_by_key(|r| r.start_tick);
}

/// The Ending button on a selection of `start..end`: removes an ending
/// already there, or adds one to the passage it belongs to — inside it (a
/// first-time bar) or starting on its repeat sign (a second-time bar).
/// Anywhere else it does nothing and says so.
pub(super) fn toggle_ending(repeats: &mut [Repeat], start: u64, end: u64) -> bool {
    for repeat in repeats.iter_mut() {
        if let Some(index) =
            repeat.endings.iter().position(|e| e.start_tick == start && e.end_tick == end)
        {
            repeat.endings.remove(index);
            return true;
        }
    }
    let Some(repeat) = repeats
        .iter_mut()
        .find(|r| (r.start_tick < start && end <= r.end_tick) || start == r.end_tick)
    else {
        return false;
    };
    repeat.endings.retain(|e| e.end_tick <= start || e.start_tick >= end);
    repeat.endings.push(Ending { start_tick: start, end_tick: end, passes: Vec::new() });
    repeat.endings.sort_by_key(|e| e.start_tick);
    fit_endings(repeat);
    true
}

/// Removing `start..end` and closing the gap: a sign inside the removed
/// stretch goes with it, and everything after moves back.
pub(super) fn close_gap(repeats: &mut Vec<Repeat>, start: u64, end: u64) {
    let span = end.saturating_sub(start);
    if span == 0 {
        return;
    }
    let overlaps = |from: u64, to: u64| from < end && to > start;
    let shift = |tick: u64| if tick >= end { tick - span } else { tick };
    repeats.retain(|r| !overlaps(r.start_tick, r.end_tick));
    for repeat in repeats.iter_mut() {
        repeat.endings.retain(|e| !overlaps(e.start_tick, e.end_tick));
        repeat.start_tick = shift(repeat.start_tick);
        repeat.end_tick = shift(repeat.end_tick);
        for ending in &mut repeat.endings {
            ending.start_tick = shift(ending.start_tick);
            ending.end_tick = shift(ending.end_tick);
        }
    }
}

/// The ruler's label for an ending: its passes, as "1." or "1, 2.".
pub(super) fn ending_label(ending: &Ending) -> String {
    let passes: Vec<String> = ending.passes.iter().map(u32::to_string).collect();
    format!("{}.", passes.join(", "))
}

/// Draws the repeats overlapping `first_tick..last_tick`: a heavy line down
/// the grid at each end of a passage, "‖:" and ":‖ ×n" on the waveform
/// band's top edge beside them, and each ending as a bracket along that
/// edge labelled with its passes.
pub(super) fn spawn(
    commands: &mut Commands,
    items: &mut Vec<Entity>,
    repeats: &[Repeat],
    first_tick: usize,
    last_tick: usize,
    grid_h: f32,
) {
    let (first, last) = (first_tick as u64, last_tick as u64);
    let visible = |tick: u64| (first..last).contains(&tick);
    let mut rect = |left: f32, top: f32, width: f32, height: f32| {
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(left),
                        top: Val::Px(top),
                        width: Val::Px(width),
                        height: Val::Px(height),
                        ..default()
                    },
                    BackgroundColor(REPEAT_COLOR),
                    Pickable::IGNORE,
                ))
                .id(),
        );
    };
    for repeat in repeats {
        let end = repeat.endings.iter().map(|e| e.end_tick).fold(repeat.end_tick, u64::max);
        if end < first || repeat.start_tick > last {
            continue;
        }
        for tick in [repeat.start_tick, repeat.end_tick] {
            if visible(tick) {
                rect(tick as f32 * TICK_W, 0.0, 3.0, grid_h);
            }
        }
        for ending in &repeat.endings {
            let x = ending.start_tick as f32 * TICK_W;
            let width = (ending.end_tick - ending.start_tick) as f32 * TICK_W - 4.0;
            rect(x, WAVEFORM_TOP, width.max(2.0), 2.0);
            rect(x, WAVEFORM_TOP, 2.0, 10.0);
        }
    }

    let mut label = |text: String, left: f32| {
        items.push(
            commands
                .spawn_empty()
                .apply_scene(bsn! {
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(left)},
                        top: {Val::Px(WAVEFORM_TOP + 3.0)},
                    }
                    Text({text})
                    // An absolute node shrinks to its left offset's room,
                    // which wrapped ":‖ ×2" onto two lines.
                    ~{TextLayout::no_wrap()}
                    TextFont { font_size: {FontSize::Px(11.0)} }
                    TextColor({REPEAT_COLOR})
                    ~{Pickable::IGNORE}
                })
                .insert(GridItem)
                .id(),
        );
    };
    for repeat in repeats {
        if visible(repeat.start_tick) {
            label("\u{2016}:".to_string(), repeat.start_tick as f32 * TICK_W + 6.0);
        }
        if visible(repeat.end_tick) {
            label(
                format!(":\u{2016} \u{00D7}{}", repeat.passes()),
                repeat.end_tick as f32 * TICK_W - 40.0,
            );
        }
        for ending in &repeat.endings {
            if ending.end_tick > first && ending.start_tick < last {
                label(ending_label(ending), ending.start_tick as f32 * TICK_W + 5.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAR: u64 = 48; // 4/4 at 12 ticks a beat

    fn map() -> MeterMap {
        MeterMap::new([(0, "4/4")], 12)
    }

    #[test]
    fn a_selection_snaps_to_the_nearest_bar_lines() {
        assert_eq!(bar_span(&map(), 5, 90), (0, 2 * BAR));
        assert_eq!(bar_span(&map(), 40, 150), (BAR, 3 * BAR));
    }

    #[test]
    fn a_selection_inside_one_bar_becomes_that_bar() {
        assert_eq!(bar_span(&map(), 50, 60), (BAR, 2 * BAR));
    }

    #[test]
    fn with_a_pickup_the_song_start_is_still_a_bar_line() {
        // One-beat pickup: bar lines at 12, 60, 108.
        let map = MeterMap::with_pickup([(0, "4/4")], 12, 12);
        assert_eq!(nearest_bar_line(&map, 3), 0);
        assert_eq!(nearest_bar_line(&map, 9), 12);
        assert_eq!(bar_span(&map, 14, 100), (12, 108));
    }

    #[test]
    fn the_repeat_button_adds_passes_then_removes_the_sign() {
        let mut repeats = Vec::new();
        toggle_repeat(&mut repeats, 0, BAR);
        assert_eq!(repeats[0].passes(), 2);
        toggle_repeat(&mut repeats, 0, BAR);
        toggle_repeat(&mut repeats, 0, BAR);
        assert_eq!(repeats[0].passes(), MAX_EDITOR_PASSES);
        toggle_repeat(&mut repeats, 0, BAR);
        assert!(repeats.is_empty());
    }

    #[test]
    fn a_new_repeat_replaces_the_ones_it_overlaps() {
        let mut repeats = Vec::new();
        toggle_repeat(&mut repeats, 0, BAR);
        toggle_repeat(&mut repeats, 2 * BAR, 3 * BAR);
        toggle_repeat(&mut repeats, 0, 3 * BAR);
        let spans: Vec<(u64, u64)> = repeats.iter().map(|r| (r.start_tick, r.end_tick)).collect();
        assert_eq!(spans, vec![(0, 3 * BAR)]);
    }

    #[test]
    fn first_and_second_time_bars() {
        let mut repeats = Vec::new();
        toggle_repeat(&mut repeats, 0, 4 * BAR);
        assert!(toggle_ending(&mut repeats, 3 * BAR, 4 * BAR));
        assert!(toggle_ending(&mut repeats, 4 * BAR, 5 * BAR));
        let passes: Vec<Vec<u32>> = repeats[0].endings.iter().map(|e| e.passes.clone()).collect();
        assert_eq!(passes, vec![vec![1], vec![2]]);

        // A third pass: the first-time bar now serves passes 1 and 2.
        toggle_repeat(&mut repeats, 0, 4 * BAR);
        let passes: Vec<Vec<u32>> = repeats[0].endings.iter().map(|e| e.passes.clone()).collect();
        assert_eq!(passes, vec![vec![1, 2], vec![3]]);
        assert_eq!(ending_label(&repeats[0].endings[0]), "1, 2.");
    }

    #[test]
    fn the_ending_button_removes_an_ending_and_refuses_a_stray_one() {
        let mut repeats = Vec::new();
        toggle_repeat(&mut repeats, 0, 2 * BAR);
        assert!(toggle_ending(&mut repeats, BAR, 2 * BAR));
        assert!(toggle_ending(&mut repeats, BAR, 2 * BAR));
        assert!(repeats[0].endings.is_empty());
        assert!(!toggle_ending(&mut repeats, 5 * BAR, 6 * BAR), "no passage there");
        assert!(!toggle_ending(&mut repeats, 0, BAR), "an ending can't start the passage it ends");
    }

    #[test]
    fn removing_a_range_moves_later_signs_back_and_drops_the_ones_inside() {
        let mut repeats = Vec::new();
        toggle_repeat(&mut repeats, 0, BAR);
        toggle_repeat(&mut repeats, 2 * BAR, 3 * BAR);
        toggle_repeat(&mut repeats, 4 * BAR, 6 * BAR);
        toggle_ending(&mut repeats, 5 * BAR, 6 * BAR);
        close_gap(&mut repeats, 2 * BAR, 3 * BAR);
        let spans: Vec<(u64, u64)> = repeats.iter().map(|r| (r.start_tick, r.end_tick)).collect();
        assert_eq!(spans, vec![(0, BAR), (3 * BAR, 5 * BAR)]);
        assert_eq!(repeats[1].endings[0].start_tick, 4 * BAR);
    }
}
