// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use crate::{
    menu::{AppState, SelectedSong},
    song::{
        SongManifest,
        harmonica::{semitone, twelve_bar},
    },
    theme::{LoadedTheme, TwelveBarColors},
};

use super::{GameplayClock, Paused, ScoringConfig, current_bar_index, secs_per_bar};

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
            // Fixed px (not Vw/Vh) so `UiScale` affects these cells the same
            // way it affects everything else — viewport units resolve
            // straight from the physical window size and don't multiply by
            // the scale factor at all, so the grid used to stay a fixed size
            // on screen while its own text scaled independently.
            cell_width: Val::Px(120.0),
            cell_height: Val::Px(54.0),
            chord_font_size: 17.0,
            bar_num_font_size: 9.0,
            col_gap: 3.0,
        }
    }

    pub fn for_3d() -> Self {
        Self {
            cell_width: Val::Px(76.0),
            cell_height: Val::Px(52.0),
            chord_font_size: 24.0,
            bar_num_font_size: 11.0,
            col_gap: 4.0,
        }
    }
}

/// Background for `bar` (0-indexed) under `key`, colored by chord function
/// (I / IV / V) using `colors` — pulled from the active theme via
/// [`LoadedTheme::twelve_bar_colors`] so this stays consistent everywhere the
/// 12-bar-blues progression is drawn (Jam Session grid, song editor grid).
pub fn bar_bg(bar: usize, key: &str, colors: TwelveBarColors) -> Color {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    let chords = twelve_bar(key);
    if chords[bar] == v {
        colors.dominant
    } else if chords[bar] == iv {
        colors.subdominant
    } else {
        colors.tonic
    }
}

pub fn spawn_12_bar_grid(
    parent: &mut ChildSpawnerCommands,
    chords: &[String],
    key: &str,
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
            .with_children(|r| {
                for col in 0..4usize {
                    let idx = row * 4 + col;
                    r.spawn((
                        Node {
                            width: cfg.cell_width,
                            height: cfg.cell_height,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(bar_bg(idx, key, colors)),
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

pub fn update_bar(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    theme: Res<LoadedTheme>,
    mut cells: Query<(&BarCell, &mut BackgroundColor)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let bpm = manifest.chart.song.tempo_bpm as f64;
    let spb = secs_per_bar(bpm, config.beats_per_bar);
    let current = current_bar_index(clock.0, spb);
    let key = manifest.chart.song.key.as_str();
    let colors = theme.twelve_bar_colors();

    for (cell, mut bg) in &mut cells {
        *bg = if cell.0 == current {
            BackgroundColor(Color::srgba(0.75, 0.55, 0.08, 0.95))
        } else {
            BackgroundColor(bar_bg(cell.0, key, colors))
        };
    }
}

pub struct TwelveBarBluesPlugin;

impl Plugin for TwelveBarBluesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_bar.run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_bg_colours_the_one_four_five_chords_distinctly() {
        let colors = TwelveBarColors::default();
        // C 12-bar: bars are [I,I,I,I,IV,IV,I,I,V,IV,I,V] (0-indexed).
        let i = bar_bg(0, "C", colors); // tonic
        let iv = bar_bg(4, "C", colors); // subdominant
        let v = bar_bg(8, "C", colors); // dominant
        assert_ne!(i, iv);
        assert_ne!(i, v);
        assert_ne!(iv, v);
        // The last bar is also the V chord, so it shares the dominant colour.
        assert_eq!(bar_bg(11, "C", colors), v);
        // The IV bars share the subdominant colour.
        assert_eq!(bar_bg(9, "C", colors), iv);
    }
}
