// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::{modifier_abbrev, modifier_color};

/// `(abbreviation, full name, accent colour)` for every technique modifier, in
/// the same order/palette as the badges on the falling notes. Built from the
/// shared `modifier_abbrev`/`modifier_color` helpers so the legend can never
/// drift from what the notes actually show.
pub fn modifier_legend_entries() -> Vec<(&'static str, &'static str, Color)> {
    use crate::song::chart::Modifier::*;
    let items = [
        (Bend { semitones: -1.0, intensity: None }, "bend"),
        (Vibrato { oscillation_hz: 5.0, intensity: None }, "vibrato"),
        (WahWah { oscillation_hz: 3.0, intensity: None }, "wah-wah"),
        (Hold { intensity: None }, "hold"),
        (Overblow, "overblow"),
        (Overdraw, "overdraw"),
    ];
    items
        .iter()
        .map(|(m, name)| (modifier_abbrev(m), *name, modifier_color(m)))
        .collect()
}

/// Spawns a compact legend mapping each modifier badge colour to its meaning,
/// so players can decode the fast-falling note hints. Used by both the 2D and
/// 3D HUDs.
pub fn spawn_modifier_legend(parent: &mut ChildSpawnerCommands, font: &FontSource) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("TECHNIQUES"),
                TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.62)),
            ));

            // Wrapping row so the six entries pack into a small footprint.
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(10.0),
                row_gap: Val::Px(3.0),
                max_width: Val::Px(230.0),
                ..default()
            })
            .with_children(|wrap| {
                for (abbr, name, color) in modifier_legend_entries() {
                    wrap.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        // colour pill carrying the abbreviation, identical to the note badge
                        row.spawn((
                            Node { padding: UiRect::axes(Val::Px(3.0), Val::Px(0.5)), ..default() },
                            BackgroundColor(color),
                        ))
                        .with_children(|pill| {
                            pill.spawn((
                                Text::new(abbr),
                                TextFont { font_size: FontSize::Px(9.0), font: font.clone(), ..default() },
                                TextColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
                            ));
                        });
                        row.spawn((
                            Text::new(name),
                            TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                            TextColor(Color::srgb(0.70, 0.72, 0.78)),
                        ));
                    });
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_covers_all_six_modifiers() {
        assert_eq!(modifier_legend_entries().len(), 6);
    }

    #[test]
    fn legend_abbreviations_match_badge_palette() {
        let abbrs: Vec<&str> = modifier_legend_entries().iter().map(|(a, _, _)| *a).collect();
        assert_eq!(abbrs, ["\u{266D}", "vib", "wah", "hold", "ob", "od"]);
    }
}