// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use harmonicon_app::app::{AppState, GameplayMode, JamProgression, SelectedSong};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::{SongManifest, harmonica::Progression};
use harmonicon_ui::dialogs::twelve_bar_grid::{BarCell, bar_bg};
pub use harmonicon_ui::dialogs::twelve_bar_grid::{GridConfig, spawn_12_bar_grid};

use super::{BarChanged, CurrentBar, GameplayLogic, Paused};

/// Recolors the grid only when `track_current_bar` reports a bar change —
/// otherwise this rewrote `BackgroundColor` on all 12 cells every frame
/// forever for a bar that only advances every few seconds.
pub fn update_bar(
    mut changed: MessageReader<BarChanged>,
    current: Res<CurrentBar>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    theme: Res<LoadedTheme>,
    mode: Res<GameplayMode>,
    jam_progression: Res<JamProgression>,
    mut cells: Query<(&BarCell, &mut BackgroundColor)>,
) {
    if changed.read().count() == 0 {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let key = manifest.chart.song.key.as_str();
    let colors = theme.twelve_bar_colors();
    // Only Jam Session's own progression is ever anything but Standard —
    // scored gameplay's grid is an educational reference independent of the
    // loaded chart, same reasoning as `twelve_bar`'s doc comment.
    let progression = if *mode == GameplayMode::JamSession {
        jam_progression.0
    } else {
        Progression::Standard
    };

    for (cell, mut bg) in &mut cells {
        *bg = if cell.0 == current.0 {
            BackgroundColor(Color::srgba(0.75, 0.55, 0.08, 0.95))
        } else {
            BackgroundColor(bar_bg(cell.0, key, progression, colors))
        };
    }
}

pub struct TwelveBarBluesPlugin;

impl Plugin for TwelveBarBluesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_bar
                .after(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        );
    }
}
