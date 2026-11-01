// SPDX-License-Identifier: MIT

//! The static furniture of the 2D play area: the highway's lane stripes and
//! hit zone, the harmonica hole strip beneath it, and the blow/draw key.
//!
//! Split out of `gameplay_2d` because it is the half that runs *once*, at
//! song setup, and never again — the rest of that module spawns, moves and
//! retires note visuals every frame. The two share only the lane geometry,
//! and share it by construction: both take lane width from the song's
//! `HighwayLanes` layout, which is what keeps a hole number under its own
//! lane on any harp, 10-hole diatonic or 12-hole chromatic.

use bevy::prelude::*;

use harmonicon_core::chart::Action;
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::instrument::{LaneLayout, ScrollAxis};
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::HoleCell;
use super::gameplay_2d::HIT_H_PCT;
use super::highway_axis::placed;

/// Spawns the static highway furniture (lane stripes, dividers, hit zone) —
/// no notes. Notes are spawned later, lazily, by `spawn_visible_notes`.
/// `named_rows` is for a horizontal highway whose rows are strings: tinted
/// in their colours and named at the hit zone. A flute's rows are its keys,
/// which the drawing beside them names, so they stay plain.
pub(super) fn spawn_highway(hw: &mut ChildSpawnerCommands, layout: &LaneLayout, named_rows: bool) {
    // Lane count/width come from the instrument being played, not a fixed
    // 10 — a chromatic's 12+ holes need proportionally narrower lanes.
    let axis = layout.axis;
    let lane_pct = layout.lane_pct();
    let full = Val::Percent(100.0);

    for h in 0..layout.lanes.len() {
        let start_pct = h as f32 * lane_pct;
        let alpha = if h % 2 == 0 { 0.04f32 } else { 0.0f32 };
        hw.spawn((
            placed(axis, 0.0, full, start_pct, Val::Percent(lane_pct)),
            BackgroundColor(if axis == ScrollAxis::Horizontal && named_rows {
                super::guitar_visuals::string_color((h + 1) as u8).with_alpha(0.08)
            } else {
                Color::srgba(1.0, 1.0, 1.0, alpha)
            }),
        ));
        if h > 0 {
            hw.spawn((
                placed(axis, 0.0, full, start_pct, Val::Px(1.0)),
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
            ));
        }
    }

    // Hit zone: a band a note's head must reach, and the line at its far
    // edge that is the actual judgment instant.
    hw.spawn((
        placed(axis, 0.0, Val::Percent(HIT_H_PCT), 0.0, full),
        BackgroundColor(Color::srgba(1.0, 1.0, 0.55, 0.14)),
    ));
    // Full width, and warmer and heavier than a beat guide. This is the one
    // line on the highway that means "now"; the guides crossing it are
    // white, thinner and far dimmer, so the two can't be mistaken for each
    // other as they scroll past.
    hw.spawn((
        placed(axis, HIT_H_PCT, Val::Px(3.0), 0.0, full),
        BackgroundColor(Color::srgba(1.0, 0.98, 0.62, 0.85)),
    ));

    // A horizontal highway names its rows where tab does, at their start:
    // `e B G D A E` down the hit zone. A vertical one has its instrument
    // strip (the harmonica's holes) beneath it instead.
    if axis == ScrollAxis::Horizontal && named_rows {
        for (row, lane) in layout.lanes.iter().enumerate() {
            hw.spawn((
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..placed(
                        axis,
                        0.0,
                        Val::Percent(HIT_H_PCT),
                        row as f32 * lane_pct,
                        Val::Percent(lane_pct),
                    )
                },
                children![(
                    Text::new(lane.label.clone()),
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(super::guitar_visuals::string_color((row + 1) as u8)),
                )],
            ));
        }
    }
}

/// The hole strip under the highway, labelled with the notes of the harp
/// the player is *holding* — a substituted harp has different notes in the
/// same holes, and the strip is what the player reads them off.
pub(super) fn spawn_harmonica_strip(
    col: &mut ChildSpawnerCommands,
    harp: &Harmonica,
    loc: &Localization,
) {
    let hole_count = harp.hole_count();
    let lane_pct = 100.0 / hole_count as f32;
    col.spawn(Node { flex_direction: FlexDirection::Row, width: Val::Percent(100.0), ..default() })
        .with_children(|row| {
            for hole in 1u8..=hole_count {
                let b = harp.wind_direction_label(hole, &Action::Blow);
                let d = harp.wind_direction_label(hole, &Action::Draw);
                // Fixed px, not Vh — Vh resolves from the physical viewport and
                // doesn't respond to `UiScale`, unlike this cell's own text, so
                // the cell would stay a fixed size on screen while its labels
                // scaled independently.
                row.spawn_empty()
                    .apply_scene(bsn! {
                        Node {
                            width: {Val::Percent(lane_pct)},
                            height: {Val::Px(96.0)},
                            flex_direction: {FlexDirection::Column},
                            align_items: {AlignItems::Center},
                            justify_content: {JustifyContent::SpaceAround},
                            border: {UiRect::all(Val::Px(1.0))},
                        }
                        BackgroundColor({Color::srgb(0.10, 0.12, 0.16)})
                        ~{BorderColor::all(Color::srgb(0.28, 0.30, 0.40))}
                        HoleCell(hole)
                    })
                    .with_children(|cell| {
                        cell.spawn_empty().apply_scene(bsn! {
                            Text({b})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.50, 0.75, 1.00)})
                        });
                        cell.spawn_empty().apply_scene(bsn! {
                            Text({format!("{hole}")})
                            TextFont { font_size: {FontSize::Px(16.0)} }
                            TextColor({Color::WHITE})
                        });
                        cell.spawn_empty().apply_scene(bsn! {
                            Text({d})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(1.00, 0.62, 0.35)})
                        });
                    });
            }
        });

    spawn_blow_draw_legend(col, loc);
}

/// The "Blow / Draw" colour-key legend under the harmonica strip.
fn spawn_blow_draw_legend(parent: &mut ChildSpawnerCommands, loc: &Localization) {
    parent
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(20.0), ..default() })
        .with_children(|leg| {
            leg.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("gameplay-legend-blow"))})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.50, 0.75, 1.00)})
            });
            leg.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("gameplay-legend-draw"))})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(1.00, 0.62, 0.35)})
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::harmonica::richter_harp;

    /// Every `Text` under the strip, in spawn order.
    fn strip_texts(harp: &Harmonica) -> Vec<String> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::scene::ScenePlugin));
        let world = app.world_mut();
        let loc = Localization::default();
        world
            .commands()
            .spawn(Node::default())
            .with_children(|col| spawn_harmonica_strip(col, harp, &loc));
        world.flush();
        let mut texts: Vec<String> =
            world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect();
        texts.retain(|t| !t.is_empty());
        texts
    }

    #[test]
    fn the_strip_names_the_played_harps_notes_not_the_charts() {
        // A player holding an A harp for a chart written in C reads the
        // strip to know what each hole sounds — so it has to be the A harp's
        // notes on it, the same harp the judge and the hole glow use.
        let a = strip_texts(&richter_harp("A"));
        let c = strip_texts(&richter_harp("C"));
        assert!(a.contains(&"A4".to_string()) && !a.contains(&"C4".to_string()));
        assert!(c.contains(&"C4".to_string()) && !c.contains(&"A4".to_string()));
        assert_eq!(a.len(), c.len(), "same hole count, same rows");
    }
}
