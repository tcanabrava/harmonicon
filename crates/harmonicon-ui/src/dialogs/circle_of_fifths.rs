// SPDX-License-Identifier: MIT

//! A circle-of-fifths diagram: the 12 keys arranged so each neighbor is a
//! perfect fifth away, with a reference harmonica key and its available
//! `song::harmonica::Position`s highlighted — making visible the
//! relationship already driving `Position::interval_below_jam_key` (2nd
//! position is one step clockwise around the circle from the harp's own
//! key, 3rd is two steps, and so on). First used by the circle-of-fifths
//! lesson (`assets/lessons/04_scales/06_circle_of_fifths`); generic and
//! lesson-agnostic per this crate's "new reusable widgets go in dialogs/"
//! convention, so it can be reused wherever else a key relationship needs
//! showing.
//!
//! This is this crate's first *circular* UI layout — every other
//! `PositionType::Absolute` use elsewhere positions nodes at a fixed edge/
//! percentage offset, never N points computed around a ring. The angle
//! math ([`circle_point`]) is unit-tested, but the actual rendered result
//! hasn't been visually verified.

use std::f32::consts::TAU;

use bevy::prelude::*;

use harmonicon_core::harmonica::{Position, semitone};
use harmonicon_platform::theme::CircleOfFifthsColors;

/// The diagram's fixed square size, in px — small enough to sit comfortably
/// in a lesson reader card, large enough for 12 legible labels.
const SIZE_PX: f32 = 240.0;
/// How far each key point sits from center, as a fraction of the diagram's
/// own half-size (`1.0` would touch the diagram's edge exactly) — leaves
/// room for each label's own width so nothing clips.
const RADIUS_FRAC: f32 = 0.80;
/// Each key label's own square footprint, centered on its circle point.
const LABEL_PX: f32 = 34.0;

/// The `(x, y)` fraction (`0.0..=1.0` of a square diagram's own size,
/// `(0.5, 0.5)` = center) of the `index`-th of `total` evenly-spaced points
/// around a circle of radius `radius_frac` (as a fraction of the half-size,
/// so `1.0` reaches the diagram's edge), starting at 12 o'clock and
/// proceeding *clockwise* as `index` increases. Pure so the angle math is
/// directly testable independent of actually spawning/seeing any UI — see
/// this module's own doc comment for why that matters here specifically.
pub fn circle_point(index: usize, total: usize, radius_frac: f32) -> (f32, f32) {
    let angle = -std::f32::consts::FRAC_PI_2 + (index as f32 / total.max(1) as f32) * TAU;
    let x = 0.5 + radius_frac * 0.5 * angle.cos();
    let y = 0.5 + radius_frac * 0.5 * angle.sin();
    (x, y)
}

/// Spawns a circle-of-fifths diagram highlighting `harp_key` (the
/// reference harmonica's own key, placed at 12 o'clock) and, one step
/// clockwise per `interval_below_jam_key` step, each of `positions` — pass
/// `song::harmonica::Position::all()` for the full set. The diagram never
/// hardcodes which positions exist: it reads each one's own
/// `interval_below_jam_key` generically, so extending that enum needs no
/// changes here.
pub fn spawn_circle_of_fifths(
    parent: &mut ChildSpawnerCommands,
    harp_key: &str,
    positions: &[Position],
    colors: CircleOfFifthsColors,
) -> Entity {
    // Circle-of-fifths order starting at `harp_key`: index i is i fifths
    // (7 semitones) clockwise from it (C, G, D, A, E, B, F#, C#, G#, D#,
    // A#, F, back to C).
    let keys: Vec<String> = (0..12).map(|i| semitone(harp_key, i * 7)).collect();

    // Which circle index each position highlights. `interval_below_jam_key`
    // is in semitones; converting that to "how many fifths around the
    // circle" multiplies by 7's own modular inverse mod 12 — 7 * 7 = 49 =
    // 1 (mod 12), so 7 *is* its own inverse. This is what lets the diagram
    // place any position (including one like `Twelfth`, whose interval
    // isn't a small multiple of 7 on its own) with one formula instead of a
    // lookup table.
    let position_at_step: Vec<(usize, &'static str)> = positions
        .iter()
        .map(|p| {
            let steps = (p.interval_below_jam_key() * 7).rem_euclid(12) as usize;
            (steps, p.label())
        })
        .collect();

    parent
        .spawn(Node {
            width: Val::Px(SIZE_PX),
            height: Val::Px(SIZE_PX),
            ..default()
        })
        .with_children(|circle| {
            for (i, key) in keys.iter().enumerate() {
                let (x, y) = circle_point(i, keys.len(), RADIUS_FRAC);
                let is_harp_key = i == 0;
                let position_label = position_at_step
                    .iter()
                    .find(|(steps, _)| *steps == i)
                    .map(|(_, label)| *label);
                let key_color = if is_harp_key {
                    colors.harp_key
                } else if position_label.is_some() {
                    colors.position
                } else {
                    colors.base
                };

                circle
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x * SIZE_PX - LABEL_PX / 2.0),
                        top: Val::Px(y * SIZE_PX - LABEL_PX / 2.0),
                        width: Val::Px(LABEL_PX),
                        height: Val::Px(LABEL_PX),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|cell| {
                        cell.spawn((
                            Text::new(key.clone()),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(key_color),
                        ));
                        if let Some(label) = position_label {
                            cell.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(colors.position),
                            ));
                        } else if is_harp_key {
                            cell.spawn((
                                Text::new("harp"),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(colors.harp_key),
                            ));
                        }
                    });
            }
        })
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_zero_sits_at_twelve_oclock() {
        let (x, y) = circle_point(0, 12, 1.0);
        assert!((x - 0.5).abs() < 1e-5, "x={x}");
        assert!(y < 0.5, "y={y}"); // above center: smaller y is up.
    }

    #[test]
    fn a_quarter_turn_lands_on_the_right() {
        let (x, y) = circle_point(3, 12, 1.0);
        assert!(x > 0.5, "x={x}");
        assert!((y - 0.5).abs() < 1e-4, "y={y}");
    }

    #[test]
    fn a_half_turn_lands_at_the_bottom() {
        let (x, y) = circle_point(6, 12, 1.0);
        assert!((x - 0.5).abs() < 1e-4, "x={x}");
        assert!(y > 0.5, "y={y}");
    }

    #[test]
    fn every_point_sits_at_the_requested_radius() {
        for i in 0..12 {
            let (x, y) = circle_point(i, 12, 0.8);
            let r = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
            assert!((r - 0.4).abs() < 1e-4, "index {i}: r={r}");
        }
    }

    #[test]
    fn twelve_evenly_spaced_points_cover_the_full_circle() {
        // No two of the 12 points coincide, and consecutive points are all
        // the same angular distance apart (mechanically verifiable even
        // though the rendered diagram itself can't be seen here).
        let points: Vec<(f32, f32)> = (0..12).map(|i| circle_point(i, 12, 1.0)).collect();
        for i in 0..12 {
            for j in (i + 1)..12 {
                let (x1, y1) = points[i];
                let (x2, y2) = points[j];
                let d = ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt();
                assert!(d > 1e-3, "points {i} and {j} coincide");
            }
        }
    }
}
