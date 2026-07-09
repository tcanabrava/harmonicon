// SPDX-License-Identifier: MIT

//! A song-progress bar pinned to the top of the screen, shared by the 2D and
//! 3D gameplay views. Rather than a flat growing fill, it shows the song's
//! whole waveform — pre-analyzed at asset-load time, see
//! `audio_system::waveform` and `SongManifest::waveform` — so a player
//! picking a loop range can see where in the song they're aiming. A thin red
//! playhead line (styled like the Song Editor's `PlayheadLine`) sweeps across
//! it to mark the current position. A thin strip below the waveform marks
//! every chart note's onset as a tiny white rectangle, on the same timescale.

use bevy::prelude::*;

use crate::menu::AppState;

use super::{GameplayClock, GameplayRoot, LoopConfig, Paused, SongEnd};

/// Every bar keeps at least this much height (as a fraction 0..1) even during
/// silence, so the waveform reads as a continuous shape rather than gaps.
const WAVEFORM_FLOOR: f32 = 0.04;

/// Height (px) of the waveform section.
const WAVEFORM_HEIGHT: f32 = 26.0;

/// Height (px) of the note-marker strip below the waveform.
const NOTES_STRIP_HEIGHT: f32 = 10.0;

/// Width (px) of a single note marker.
const NOTE_MARKER_WIDTH: f32 = 2.0;

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

/// The timescale the currently-drawn waveform bars are laid out on — the
/// song's real decoded audio duration (`SongManifest::music_duration_secs`),
/// set once when the bar is spawned. Deliberately *not* [`SongEnd`] (last
/// chart note + a fixed tail): a tightly-trimmed track ends before that tail
/// elapses, a padded one keeps going after it, and either way positioning the
/// playhead/loop marker against the wrong one of the two visibly drifts them
/// out of sync with the waveform they're drawn on top of.
#[derive(Resource, Default)]
pub struct AudioDuration(pub f64);

/// Spawns the full-width progress bar at the very top of the screen: the
/// song's waveform (from `waveform`, one entry per bar in 0..1, see
/// `SongManifest::waveform`) on top, a strip of tiny white note markers (from
/// `note_times`, seconds from song start) below it, with the loop marker and
/// playhead drawn over both. Tagged `GameplayRoot` so it is torn down with
/// the rest of the scene. `duration_secs` is the audio's real length
/// (`SongManifest::music_duration_secs`) — see [`AudioDuration`]; both the
/// waveform and the note markers are laid out on this same timescale, so they
/// stay aligned with each other and with the playhead.
pub fn spawn_song_progress(
    commands: &mut Commands,
    waveform: &[f32],
    duration_secs: f64,
    note_times: &[f64],
) {
    commands.insert_resource(AudioDuration(duration_secs));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(WAVEFORM_HEIGHT + NOTES_STRIP_HEIGHT),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(50),
            GameplayRoot,
        ))
        .with_children(|bar| {
            bar.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(WAVEFORM_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                for &amplitude in waveform {
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            height: Val::Percent(
                                amplitude.clamp(0.0, 1.0).max(WAVEFORM_FLOOR) * 100.0,
                            ),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.35, 0.75, 1.0, 0.65)),
                    ));
                }
            });

            bar.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(NOTES_STRIP_HEIGHT),
                ..default()
            })
            .with_children(|strip| {
                if duration_secs > 0.0 {
                    for &time in note_times {
                        let left = (time / duration_secs).clamp(0.0, 1.0) as f32 * 100.0;
                        strip.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(left),
                                top: Val::Px(0.0),
                                width: Val::Px(NOTE_MARKER_WIDTH),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        ));
                    }
                }
            });

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
/// the real end of the audio ([`AudioDuration`], not [`SongEnd`] — see its
/// doc comment). Stays put at the left during the countdown (negative clock)
/// and for looping songs (no finite `SongEnd`); once the real audio content
/// is behind it, clamps at the right edge rather than racing ahead of or
/// lagging the waveform underneath it.
fn update_progress(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    duration: Res<AudioDuration>,
    mut playheads: Query<&mut Node, With<ProgressPlayhead>>,
) {
    let progress = if song_end.0.is_finite() && song_end.0 > 0.0 && duration.0 > 0.0 {
        (clock.get() / duration.0).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    for mut node in &mut playheads {
        node.left = Val::Percent(progress * 100.0);
    }
}

/// Left offset and width (both fractions of the bar, 0.0–1.0) for the loop
/// marker, or `None` if it shouldn't be shown — no finite song length, no
/// known audio duration to lay it out against, or the range isn't currently
/// active. Split out from the system for unit testing without spinning up
/// rendering. `song_end_finite` gates on the same "does this song even have
/// an end" condition `update_progress` does; `duration_secs` ([`AudioDuration`])
/// is the timescale actually used for the fraction, so the marker lines up
/// with the waveform bars it's drawn over.
pub(super) fn loop_marker_geometry(
    active: bool,
    start_time: f64,
    end_time: f64,
    song_end_finite: bool,
    duration_secs: f64,
) -> Option<(f32, f32)> {
    if !active || !song_end_finite || duration_secs <= 0.0 {
        return None;
    }
    let left = (start_time / duration_secs).clamp(0.0, 1.0) as f32;
    let right = (end_time / duration_secs).clamp(0.0, 1.0) as f32;
    Some((left, (right - left).max(0.0)))
}

/// Keeps the loop marker in step with `LoopConfig`. Not gated on `Paused` —
/// the pause menu is the only place that changes `LoopConfig`, so this needs
/// to update while paused for the marker to be ready the instant Resume
/// hides the overlay.
fn update_loop_marker(
    loop_cfg: Res<LoopConfig>,
    song_end: Res<SongEnd>,
    duration: Res<AudioDuration>,
    mut markers: Query<(&mut Node, &mut Visibility), With<LoopRangeMarker>>,
) {
    if !loop_cfg.is_changed() && !song_end.is_changed() {
        return;
    }
    let geometry = loop_marker_geometry(
        loop_cfg.active,
        loop_cfg.start_time,
        loop_cfg.end_time,
        song_end.0.is_finite() && song_end.0 > 0.0,
        duration.0,
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
        app.init_resource::<AudioDuration>()
            .add_systems(
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
        assert_eq!(loop_marker_geometry(false, 8.0, 16.0, true, 60.0), None);
    }

    #[test]
    fn loop_marker_hidden_without_a_finite_song_end() {
        assert_eq!(loop_marker_geometry(true, 8.0, 16.0, false, 60.0), None);
    }

    #[test]
    fn loop_marker_hidden_without_a_known_audio_duration() {
        assert_eq!(loop_marker_geometry(true, 8.0, 16.0, true, 0.0), None);
    }

    #[test]
    fn loop_marker_geometry_is_a_fraction_of_the_audio_duration() {
        // 8s..16s of a 64s track → starts an eighth of the way in, an
        // eighth wide — driven by the real audio duration, not `SongEnd`.
        let (left, width) = loop_marker_geometry(true, 8.0, 16.0, true, 64.0).unwrap();
        assert!((left - 0.125).abs() < 1e-6);
        assert!((width - 0.125).abs() < 1e-6);
    }
}
