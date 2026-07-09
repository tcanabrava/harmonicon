// SPDX-License-Identifier: MIT

//! A song-progress bar pinned to the top of the screen, shared by the 2D and
//! 3D gameplay views. Rather than a flat growing fill, it shows the song's
//! whole waveform — pre-analyzed at asset-load time, see
//! `audio_system::waveform` and `SongManifest::waveform` — so a player
//! picking a loop range can see where in the song they're aiming. A thin red
//! playhead line (styled like the Song Editor's `PlayheadLine`) sweeps across
//! it to mark the current position.

use bevy::prelude::*;

use crate::menu::AppState;

use super::{GameplayClock, GameplayRoot, LoopConfig, Paused, SongEnd};

/// Every bar keeps at least this much height (as a fraction 0..1) even during
/// silence, so the waveform reads as a continuous shape rather than gaps.
const WAVEFORM_FLOOR: f32 = 0.04;

/// The moving playhead; a thin vertical line, styled like the Song Editor's
/// `PlayheadLine`. Its horizontal position (not width, unlike the old flat
/// fill) is driven each frame.
#[derive(Component, Default, Clone)]
pub struct ProgressPlayhead;

/// Highlights the pause menu's A–B loop range within the bar. Absolutely
/// positioned (unlike the waveform bars, which occupy normal flex flow) so it
/// can sit at an arbitrary offset independent of them.
#[derive(Component, Default, Clone)]
pub struct LoopRangeMarker;

/// Spawns the full-width progress bar at the very top of the screen: the
/// song's waveform (from `waveform`, one entry per bar in 0..1, see
/// `SongManifest::waveform`) with the loop marker and playhead drawn over it.
/// Tagged `GameplayRoot` so it is torn down with the rest of the scene.
pub fn spawn_song_progress(commands: &mut Commands, waveform: &[f32]) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(50),
            GameplayRoot,
        ))
        .with_children(|bar| {
            for &amplitude in waveform {
                bar.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        height: Val::Percent(amplitude.clamp(0.0, 1.0).max(WAVEFORM_FLOOR) * 100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.35, 0.75, 1.0, 0.65)),
                ));
            }

            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.85, 0.35, 0.45)),
                Visibility::Hidden,
                LoopRangeMarker,
            ));

            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(2.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.30, 0.30)),
                ProgressPlayhead,
            ));
        });
}

/// Sweeps the playhead from the left edge at song start to the right edge at
/// its end. Stays put at the left during the countdown (negative clock) and
/// for looping songs (no finite end).
fn update_progress(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    mut playheads: Query<&mut Node, With<ProgressPlayhead>>,
) {
    let progress = if song_end.0.is_finite() && song_end.0 > 0.0 {
        (clock.get() / song_end.0).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    for mut node in &mut playheads {
        node.left = Val::Percent(progress * 100.0);
    }
}

/// Left offset and width (both fractions of the bar, 0.0–1.0) for the loop
/// marker, or `None` if it shouldn't be shown — no finite song length, or the
/// range isn't currently active. Split out from the system for unit testing
/// without spinning up rendering.
pub(super) fn loop_marker_geometry(
    active: bool,
    start_time: f64,
    end_time: f64,
    song_end: f64,
) -> Option<(f32, f32)> {
    if !active || !song_end.is_finite() || song_end <= 0.0 {
        return None;
    }
    let left = (start_time / song_end).clamp(0.0, 1.0) as f32;
    let right = (end_time / song_end).clamp(0.0, 1.0) as f32;
    Some((left, (right - left).max(0.0)))
}

/// Keeps the loop marker in step with `LoopConfig`. Not gated on `Paused` —
/// the pause menu is the only place that changes `LoopConfig`, so this needs
/// to update while paused for the marker to be ready the instant Resume
/// hides the overlay.
fn update_loop_marker(
    loop_cfg: Res<LoopConfig>,
    song_end: Res<SongEnd>,
    mut markers: Query<(&mut Node, &mut Visibility), With<LoopRangeMarker>>,
) {
    if !loop_cfg.is_changed() && !song_end.is_changed() {
        return;
    }
    let geometry = loop_marker_geometry(
        loop_cfg.active,
        loop_cfg.start_time,
        loop_cfg.end_time,
        song_end.0,
    );
    for (mut node, mut vis) in &mut markers {
        match geometry {
            Some((left, width)) => {
                node.left = Val::Percent(left * 100.0);
                node.width = Val::Percent(width * 100.0);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

pub struct SongProgressPlugin;

impl Plugin for SongProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_progress.run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        )
        .add_systems(
            Update,
            update_loop_marker.run_if(in_state(AppState::Playing)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_marker_hidden_when_inactive() {
        assert_eq!(loop_marker_geometry(false, 8.0, 16.0, 60.0), None);
    }

    #[test]
    fn loop_marker_hidden_without_a_finite_song_end() {
        assert_eq!(loop_marker_geometry(true, 8.0, 16.0, f64::INFINITY), None);
        assert_eq!(loop_marker_geometry(true, 8.0, 16.0, 0.0), None);
    }

    #[test]
    fn loop_marker_geometry_is_a_fraction_of_the_song() {
        // 8s..16s of a 60s song → starts an eighth of the way in, an
        // eighth wide.
        let (left, width) = loop_marker_geometry(true, 8.0, 16.0, 64.0).unwrap();
        assert!((left - 0.125).abs() < 1e-6);
        assert!((width - 0.125).abs() < 1e-6);
    }
}
