// SPDX-License-Identifier: MIT

//! A thin song-progress bar pinned to the top of the screen, shared by the 2D
//! and 3D gameplay views. The fill tracks the gameplay clock against the song's
//! end time.

use bevy::prelude::*;

use crate::menu::AppState;

use super::{GameplayClock, GameplayRoot, Paused, SongEnd};

/// The growing fill of the progress bar; its width is driven each frame.
#[derive(Component)]
pub struct ProgressFill;

/// Spawns the full-width progress bar at the very top of the screen. Tagged
/// `GameplayRoot` so it is torn down with the rest of the scene.
pub fn spawn_song_progress(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(50),
            GameplayRoot,
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                ProgressFill,
            ));
        });
}

/// Fills the bar from 0 at the song start to full at its end. Stays empty during
/// the countdown (negative clock) and for looping songs (no finite end).
fn update_progress(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    mut fills: Query<&mut Node, With<ProgressFill>>,
) {
    let progress = if song_end.0.is_finite() && song_end.0 > 0.0 {
        (clock.0 / song_end.0).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    for mut node in &mut fills {
        node.width = Val::Percent(progress * 100.0);
    }
}

pub struct SongProgressPlugin;

impl Plugin for SongProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_progress.run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        );
    }
}
