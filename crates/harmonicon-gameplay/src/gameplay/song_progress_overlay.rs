// SPDX-License-Identifier: MIT

//! A song-progress bar pinned to the top of Jam Session's screen: a thin
//! red playhead sweeping the song's length, and the A–B loop range when
//! one is active. Deliberately nothing else — note lanes, the waveform and
//! per-phrase mastery were drawn here once and were more noise than help.
//!
//! A song with no music has a `music_duration_secs` of 0;
//! [`spawn_song_progress`] then times the bar by the chart's own extent.

use bevy::prelude::*;

use harmonicon_app::app::AppState;

use super::{GameplayClock, GameplayLogic, GameplayRoot, LoopConfig, Paused, SongEnd};

/// Total height (px) of the bar, pinned at the very top of the screen.
/// `pub` so HUDs can reserve this much space instead of placing content
/// under it, where the bar (painted above them — see [`BAR_Z_INDEX`])
/// would cover it.
pub const BAR_HEIGHT: f32 = 60.0;

/// Above the pause overlay's own backdrop (`pause_menu::setup_pause_menu`,
/// `GlobalZIndex(200)`), so the bar stays readable rather than dimmed while
/// paused.
const BAR_Z_INDEX: i32 = 250;

/// The moving playhead; a thin vertical line, styled like the Song Editor's
/// `PlayheadLine`. Its horizontal position is driven each frame.
#[derive(Component, Default, Clone)]
pub struct ProgressPlayhead;

/// Which end of the A–B range a handle marks.
#[derive(Clone, Copy)]
enum LoopEdge {
    A,
    B,
}

/// Solid, unlike the range wash it bounds: an edge has to be findable at a
/// glance, and a translucent edge on a translucent band is neither.
const LOOP_HANDLE_COLOR: Color = Color::srgb(0.4, 0.75, 1.0);

/// Highlights the active `LoopConfig` range within the bar.
#[derive(Component, Default, Clone)]
pub struct LoopRangeMarker;

/// The timescale the bar is laid out on — the real decoded audio duration,
/// set once when the bar spawns. Deliberately not [`SongEnd`] (last chart
/// note + a fixed tail): a tightly-trimmed track ends before that tail
/// elapses, a padded one keeps going after it, and the playhead should
/// follow the music.
#[derive(Resource, Default)]
pub struct AudioDuration(pub f64);

/// Spawns the full-width progress bar at the top of the screen, tagged
/// `GameplayRoot` so it's torn down with the scene. `duration_secs` is the
/// audio's real length if there is one (see [`AudioDuration`]);
/// `content_end_secs`, the end of the chart's last note, times a song with
/// no background music instead.
pub fn spawn_song_progress(
    commands: &mut Commands,
    duration_secs: f64,
    content_end_secs: f64,
) -> Entity {
    let duration_secs = effective_duration(duration_secs, content_end_secs);
    commands.insert_resource(AudioDuration(duration_secs));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(BAR_HEIGHT),
                ..default()
            },
            BackgroundColor(harmonicon_platform::theme::HUD_PANEL_BG),
            GlobalZIndex(BAR_Z_INDEX),
            GameplayRoot,
            // Display only: nothing here takes input.
            Pickable::IGNORE,
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.4, 0.75, 1.0, 0.06)),
                Visibility::Hidden,
                Pickable::IGNORE,
                LoopRangeMarker,
            ))
            .with_children(|range| {
                // A and B handles, children of the range so they ride along
                // with it as `update_loop_marker` moves and resizes it. A
                // translucent wash alone has no ends; these are what make
                // it read as *from here to here*.
                for (edge, label) in [(LoopEdge::A, "A"), (LoopEdge::B, "B")] {
                    let (left, right, cap_left, cap_right) = match edge {
                        LoopEdge::A => (Val::ZERO, Val::Auto, Val::Auto, Val::Px(3.0)),
                        LoopEdge::B => (Val::Auto, Val::ZERO, Val::Px(3.0), Val::Auto),
                    };
                    range.spawn_empty().apply_scene(bsn! {
                        Node {
                            position_type: {PositionType::Absolute},
                            top: {Val::ZERO},
                            left: {left},
                            right: {right},
                            width: {Val::Px(3.0)},
                            height: {Val::Percent(100.0)},
                            justify_content: {JustifyContent::FlexStart},
                        }
                        BackgroundColor({LOOP_HANDLE_COLOR})
                        ~{Pickable::IGNORE}
                        Children [
                            // The letter sits outside the handle, away from the range.
                            Node {
                                position_type: {PositionType::Absolute},
                                top: {Val::Px(-1.0)},
                                left: {cap_left},
                                right: {cap_right},
                                padding: {UiRect::axes(Val::Px(3.0), Val::ZERO)},
                            }
                            BackgroundColor({LOOP_HANDLE_COLOR})
                            ~{Pickable::IGNORE}
                            Children [
                                Text({label})
                                TextFont { font_size: {FontSize::Px(10.0)} }
                                TextColor({Color::srgb(0.08, 0.07, 0.03)})
                            ]
                        ]
                    });
                }
            });

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
                Pickable::IGNORE,
                ProgressPlayhead,
            ));
        })
        .id()
}

/// Sweeps the playhead from the left edge at song start to the right edge at
/// the real end of the audio ([`AudioDuration`], not [`SongEnd`] — see its
/// doc comment). Stays put at the left during the countdown (negative clock)
/// and before a song has set a finite `SongEnd`, and clamps at the right edge
/// once the audio is behind it.
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
    let left = Val::Percent(progress * 100.0);
    for mut node in &mut playheads {
        if node.left != left {
            node.left = left;
        }
    }
}

/// Left offset and width (fractions 0.0–1.0) for the loop marker, or `None`
/// if it shouldn't show — no finite song length, no known duration, or the
/// range isn't active. `song_end_finite` mirrors `update_progress`'s own
/// "does this song have an end" gate; `duration_secs` is [`AudioDuration`].
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

/// The timescale to lay the bar out on: `duration_secs` verbatim if there's
/// real audio, else `content_end_secs` (a song with no background music has
/// nothing to decode, so `duration_secs` reads `0.0`).
fn effective_duration(duration_secs: f64, content_end_secs: f64) -> f64 {
    if duration_secs > 0.0 { duration_secs } else { content_end_secs.max(0.0) }
}

/// Keeps the loop marker in step with `LoopConfig`.
fn update_loop_marker(
    loop_cfg: Res<LoopConfig>,
    song_end: Res<SongEnd>,
    duration: Res<AudioDuration>,
    mut markers: Query<(&mut Node, &mut Visibility), With<LoopRangeMarker>>,
) {
    if !loop_cfg.is_changed() && !song_end.is_changed() && !duration.is_changed() {
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
                update_progress
                    .after(GameplayLogic)
                    .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
            )
            .add_systems(Update, update_loop_marker.run_if(in_state(AppState::Playing)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── effective_duration (the no-background-music fallback) ────────────────

    #[test]
    fn effective_duration_prefers_real_audio_duration_when_present() {
        assert_eq!(effective_duration(64.0, 51.0), 64.0);
    }

    #[test]
    fn effective_duration_falls_back_to_the_charts_end_without_music() {
        assert_eq!(effective_duration(0.0, 24.0), 24.0);
    }

    #[test]
    fn effective_duration_is_zero_for_a_truly_empty_chart() {
        assert_eq!(effective_duration(0.0, 0.0), 0.0);
    }

    // ── loop_marker_geometry ─────────────────────────────────────────────────

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
