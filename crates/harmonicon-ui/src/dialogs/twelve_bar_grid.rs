// SPDX-License-Identifier: MIT

//! Reusable 12-bar blues grid presentation. Callers own time and decide which
//! bar, if any, is highlighted.

use bevy::prelude::*;
use harmonicon_core::harmonica::{Progression, progression_bars, semitone};
use harmonicon_platform::theme::TwelveBarColors;

#[derive(Component)]
pub struct BarCell(pub usize);

pub struct GridConfig {
    pub cell_width: Val,
    pub cell_height: Val,
    pub chord_font_size: f32,
    pub bar_num_font_size: f32,
    pub col_gap: f32,
}

impl GridConfig {
    pub fn for_2d() -> Self {
        Self {
            cell_width: Val::Px(120.0),
            cell_height: Val::Px(54.0),
            chord_font_size: 17.0,
            bar_num_font_size: 15.0,
            col_gap: 3.0,
        }
    }

    pub fn for_3d() -> Self {
        Self {
            cell_width: Val::Px(76.0),
            cell_height: Val::Px(52.0),
            chord_font_size: 24.0,
            bar_num_font_size: 15.0,
            col_gap: 4.0,
        }
    }
}

pub fn bar_bg(bar: usize, key: &str, progression: Progression, colors: TwelveBarColors) -> Color {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    let (root, _) = &progression_bars(key, progression)[bar];
    if *root == v {
        colors.dominant
    } else if *root == iv {
        colors.subdominant
    } else {
        colors.tonic
    }
}

pub fn spawn_12_bar_grid(
    parent: &mut ChildSpawnerCommands,
    chords: &[String],
    key: &str,
    progression: Progression,
    cfg: &GridConfig,
    colors: TwelveBarColors,
) {
    for row in 0..3usize {
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(cfg.col_gap),
                ..default()
            })
            .with_children(|row_node| {
                for col in 0..4usize {
                    let idx = row * 4 + col;
                    row_node
                        .spawn((
                            Node {
                                width: cfg.cell_width,
                                height: cfg.cell_height,
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(bar_bg(idx, key, progression, colors)),
                            BorderColor::all(Color::srgb(0.25, 0.25, 0.38)),
                            BarCell(idx),
                        ))
                        .with_children(|cell| {
                            cell.spawn((
                                Text::new(chords[idx].clone()),
                                TextFont {
                                    font_size: FontSize::Px(cfg.chord_font_size),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                            cell.spawn((
                                Text::new(format!("{}", idx + 1)),
                                TextFont {
                                    font_size: FontSize::Px(cfg.bar_num_font_size),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.45, 0.45, 0.55)),
                            ));
                        });
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_colours_distinguish_the_one_four_and_five() {
        let colors = TwelveBarColors::default();
        let i = bar_bg(0, "C", Progression::Standard, colors);
        let iv = bar_bg(4, "C", Progression::Standard, colors);
        let v = bar_bg(8, "C", Progression::Standard, colors);
        assert_ne!(i, iv);
        assert_ne!(i, v);
        assert_ne!(iv, v);
        assert_eq!(bar_bg(11, "C", Progression::Standard, colors), v);
        assert_eq!(bar_bg(9, "C", Progression::Standard, colors), iv);
    }

    #[test]
    fn bar_colours_follow_non_standard_progressions() {
        let colors = TwelveBarColors::default();
        let iv = bar_bg(4, "C", Progression::Standard, colors);
        assert_eq!(bar_bg(1, "C", Progression::QuickChange, colors), iv);
        assert_ne!(
            bar_bg(1, "C", Progression::Standard, colors),
            bar_bg(1, "C", Progression::QuickChange, colors)
        );
    }
}
