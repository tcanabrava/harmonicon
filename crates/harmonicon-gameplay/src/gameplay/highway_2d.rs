// SPDX-License-Identifier: MIT

//! The static furniture of the 2D play area: the highway's lane stripes and
//! hit zone, the harmonica hole strip beneath it, and the blow/draw key.
//!
//! Split out of `gameplay_2d` because it is the half that runs *once*, at
//! song setup, and never again — the rest of that module spawns, moves and
//! retires note visuals every frame. The two share only the lane geometry,
//! and share it by construction: both derive lane width from
//! `100.0 / hole_count`, which is what keeps a hole number under its own
//! lane on any harp, 10-hole diatonic or 12-hole chromatic.

use bevy::prelude::*;

use harmonicon_core::chart::Action;
use harmonicon_core::harmonica::Harmonica;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::HoleCell;
use super::gameplay_2d::HIT_H_PCT;

/// Spawns the static highway furniture (lane stripes, dividers, hit zone) —
/// no notes. Notes are spawned later, lazily, by `spawn_visible_notes`.
pub(super) fn spawn_highway(hw: &mut ChildSpawnerCommands, harp: &Harmonica) {
    // Lane count/width come from the harmonica being played, not a fixed
    // 10 — a chromatic's 12+ holes need proportionally narrower lanes.
    let hole_count = harp.hole_count() as usize;
    let lane_pct = 100.0 / hole_count as f32;

    for h in 0..hole_count {
        let left_pct = h as f32 * lane_pct;
        let alpha = if h % 2 == 0 { 0.04f32 } else { 0.0f32 };
        hw.spawn_empty().apply_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                left: {Val::Percent(left_pct)},
                top: {Val::Percent(0.0)},
                width: {Val::Percent(lane_pct)},
                height: {Val::Percent(100.0)},
            }
            BackgroundColor({Color::srgba(1.0, 1.0, 1.0, alpha)})
        });
        if h > 0 {
            hw.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    left: {Val::Percent(left_pct)},
                    top: {Val::Percent(0.0)},
                    width: {Val::Px(1.0)},
                    height: {Val::Percent(100.0)},
                }
                BackgroundColor({Color::srgba(1.0, 1.0, 1.0, 0.08)})
            });
        }
    }

    // Hit zone: a band a note's head must reach, and the line at its top
    // edge that is the actual judgment instant.
    hw.spawn_empty().apply_scene(bsn! {
        Node {
            position_type: {PositionType::Absolute},
            left: {Val::Percent(0.0)},
            bottom: {Val::Percent(0.0)},
            width: {Val::Percent(100.0)},
            height: {Val::Percent(HIT_H_PCT)},
        }
        BackgroundColor({Color::srgba(1.0, 1.0, 0.55, 0.14)})
    });
    // Full width, and warmer and heavier than a beat guide. This is the one
    // line on the highway that means "now"; the guides crossing it are
    // white, thinner and far dimmer, so the two can't be mistaken for each
    // other as they scroll past.
    hw.spawn_empty().apply_scene(bsn! {
        Node {
            position_type: {PositionType::Absolute},
            left: {Val::Percent(0.0)},
            bottom: {Val::Percent(HIT_H_PCT)},
            width: {Val::Percent(100.0)},
            height: {Val::Px(3.0)},
        }
        BackgroundColor({Color::srgba(1.0, 0.98, 0.62, 0.85)})
    });
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

    spawn_blow_draw_legend(col, loc, 20.0, 0.0);
}

/// The "Blow / Draw" colour-key legend — identical between the 2D harmonica
/// strip and the 3D HUD overlay, differing only in the row's own spacing
/// (`column_gap`/top `margin`, which each caller picks to match its
/// surrounding layout).
pub(super) fn spawn_blow_draw_legend(
    parent: &mut ChildSpawnerCommands,
    loc: &Localization,
    column_gap: f32,
    margin_top: f32,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(column_gap),
            margin: UiRect::top(Val::Px(margin_top)),
            ..default()
        })
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
