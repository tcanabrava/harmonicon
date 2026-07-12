// SPDX-License-Identifier: MIT

//! A song-progress bar pinned to the top of the screen, shared by the 2D and
//! 3D gameplay views. It shows the song's whole waveform — pre-analyzed at
//! asset-load time, see `audio_system::waveform` and
//! `SongManifest::waveform` — so a player picking a loop range can see where
//! in the song they're aiming. A thin red playhead line (styled like the
//! Song Editor's `PlayheadLine`) sweeps across it to mark the current
//! position. A thin strip below the waveform marks every chart note's onset
//! as a tiny white rectangle, on the same timescale.
//!
//! The bar has two [`ProgressBarMode`]s. While playing it's pure
//! visualization. While paused it becomes editable: click-and-drag anywhere
//! on it to sweep out a new A–B loop range, shown live as a yellow
//! semi-transparent rectangle. Releasing the mouse fires [`RequestLoopRange`]
//! rather than writing `LoopConfig` directly, keeping the drag interaction
//! and the loop-adoption policy decoupled.

use bevy::picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::menu::AppState;

use super::adaptive_difficulty::{AdaptiveDifficulty, PhraseSection};
use super::{GameplayClock, GameplayRoot, LoopConfig, Paused, SongEnd, loop_range_valid};

/// Every bar keeps at least this much height (as a fraction 0..1) even during
/// silence, so the waveform reads as a continuous shape rather than gaps.
const WAVEFORM_FLOOR: f32 = 0.04;

/// Height (px) of the waveform section.
const WAVEFORM_HEIGHT: f32 = 26.0;

/// Height (px) of the note-marker strip below the waveform.
const NOTES_STRIP_HEIGHT: f32 = 10.0;

/// Height (px) of the per-phrase adaptive-difficulty strip below the note
/// markers — one rectangle per `adaptive_difficulty::PhraseSection`, filled
/// (semi-transparently, so it reads as a fill rather than a solid block)
/// dim-gray to green by how much of that phrase has been learned, with a
/// fully opaque border so adjacent sections stay visually distinct even
/// when their fill colors are close.
const PHRASE_STRIP_HEIGHT: f32 = 18.0;

/// Border thickness (px) on each phrase-section rectangle.
const PHRASE_RECT_BORDER: f32 = 1.5;

/// Total height (px) of the bar, pinned across the full width at the very
/// top of the screen (`top: 0`). `pub` so the gameplay HUDs can reserve this
/// much space at the top of their own layout instead of placing content
/// underneath it, where the bar — deliberately painted above them, see
/// [`BAR_Z_INDEX`] — would cover it.
pub const BAR_HEIGHT: f32 = WAVEFORM_HEIGHT + NOTES_STRIP_HEIGHT + PHRASE_STRIP_HEIGHT;

/// Width (px) of a single note marker.
const NOTE_MARKER_WIDTH: f32 = 2.0;

/// Above the pause overlay's own backdrop (`pause_menu::setup_pause_menu`,
/// `GlobalZIndex(200)`) so the bar renders on top of — and, since bevy_ui's
/// picking backend hit-tests in the same order it paints, actually receives
/// clicks through — the pause dimming rather than being buried under it.
const BAR_Z_INDEX: i32 = 250;

/// The moving playhead; a thin vertical line, styled like the Song Editor's
/// `PlayheadLine`. Its horizontal position is driven each frame.
#[derive(Component, Default, Clone)]
pub struct ProgressPlayhead;

/// Highlights the current A–B loop range within the bar — either the
/// committed `LoopConfig` range, or a live preview of an in-progress drag
/// (see [`LoopDrag`]). Absolutely positioned (unlike the waveform bars,
/// which occupy normal flex flow) so it can sit at an arbitrary offset
/// independent of them.
#[derive(Component, Default, Clone)]
pub struct LoopRangeMarker;

/// One rectangle in the per-phrase adaptive-difficulty strip, spanning its
/// `PhraseSection`'s time range. `usize` is the section's ordinal index
/// (into `AdaptiveDifficulty::sections`/`learned`), used by
/// [`update_phrase_section_colors`] to re-tint it without respawning
/// whenever a phrase's learned fraction changes (e.g. a manual pause-menu
/// edit).
#[derive(Component, Clone, Copy)]
struct PhraseSectionRect(usize);

/// The single entity that accepts click-and-drag input for setting a loop
/// range — the bar's own root. Every visual child (waveform bars, note
/// markers, the loop marker, the playhead) is spawned `Pickable::IGNORE`,
/// so a click can never get swallowed by whichever tiny rectangle happens
/// to be on top; it always lands on this one surface instead.
#[derive(Component, Default, Clone)]
struct ProgressBarDragSurface;

/// The timescale the currently-drawn waveform bars are laid out on — the
/// song's real decoded audio duration (`SongManifest::music_duration_secs`),
/// set once when the bar is spawned. Deliberately *not* [`SongEnd`] (last
/// chart note + a fixed tail): a tightly-trimmed track ends before that tail
/// elapses, a padded one keeps going after it, and either way positioning the
/// playhead/loop marker against the wrong one of the two visibly drifts them
/// out of sync with the waveform they're drawn on top of.
#[derive(Resource, Default)]
pub struct AudioDuration(pub f64);

/// Whether the progress bar behaves as a static visualization (normal play)
/// or accepts click-and-drag to set an A–B loop range (while paused). Kept
/// as its own resource — rather than every system here reaching for `Paused`
/// directly — so "is the bar editable right now" is one explicit concept,
/// synced from `Paused` by [`sync_progress_bar_mode`].
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ProgressBarMode {
    #[default]
    Visualization,
    Edit,
}

/// An in-progress click-and-drag on the bar, sweeping out a candidate A–B
/// loop range. `origin_time`/`current_time` are the drag's two endpoints in
/// song-seconds, in the order the pointer visited them (not sorted — the
/// drag can go either direction), live-updated so [`update_loop_marker`] can
/// preview it before the mouse is released.
#[derive(Resource, Default)]
struct LoopDrag {
    active: bool,
    origin_time: f64,
    current_time: f64,
}

/// Fired when a click-and-drag on the progress bar (in edit mode) is
/// released, proposing `start_time..end_time` as the new A–B loop range.
/// Consumed by [`apply_requested_loop_range`] — the only system that writes
/// `LoopConfig::start_time`/`end_time` here — so the drag interaction itself
/// only ever *requests* a range, never applies one directly.
#[derive(Message, Debug, Clone, Copy)]
pub struct RequestLoopRange {
    pub start_time: f64,
    pub end_time: f64,
}

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
    sections: &[PhraseSection],
    learned: &[f32],
) {
    commands.insert_resource(AudioDuration(duration_secs));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(BAR_HEIGHT),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(BAR_Z_INDEX),
            GameplayRoot,
            Button,
            RelativeCursorPosition::default(),
            ProgressBarDragSurface,
        ))
        .observe(on_drag_start)
        .observe(on_drag)
        .observe(on_drag_end)
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(WAVEFORM_HEIGHT),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
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
                        Pickable::IGNORE,
                    ));
                }
            });

            bar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(NOTES_STRIP_HEIGHT),
                    ..default()
                },
                Pickable::IGNORE,
            ))
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
                            Pickable::IGNORE,
                        ));
                    }
                }
            });

            bar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(PHRASE_STRIP_HEIGHT),
                    position_type: PositionType::Relative,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|strip| {
                if duration_secs > 0.0 {
                    for (i, section) in sections.iter().enumerate() {
                        let Some((left, width)) =
                            phrase_rect_geometry(section.start_time, section.end_time, duration_secs)
                        else {
                            continue;
                        };
                        let learned_frac = learned.get(i).copied().unwrap_or(0.0);
                        strip.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(left * 100.0),
                                top: Val::Px(0.0),
                                width: Val::Percent(width * 100.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(PHRASE_RECT_BORDER)),
                                ..default()
                            },
                            BackgroundColor(phrase_fill_color(learned_frac)),
                            BorderColor::all(PHRASE_RECT_BORDER_COLOR),
                            Pickable::IGNORE,
                            PhraseSectionRect(i),
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
                Pickable::IGNORE,
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
                Pickable::IGNORE,
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

/// Left offset and width (both fractions of the bar, 0.0–1.0) for the
/// committed loop marker, or `None` if it shouldn't be shown — no finite
/// song length, no known audio duration to lay it out against, or the range
/// isn't currently active. Split out from the system for unit testing
/// without spinning up rendering. `song_end_finite` gates on the same "does
/// this song even have an end" condition `update_progress` does;
/// `duration_secs` ([`AudioDuration`]) is the timescale actually used for the
/// fraction, so the marker lines up with the waveform bars it's drawn over.
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

/// Left offset and width (fractions 0..1) of an in-progress drag preview.
/// Always shown once dragging has started, regardless of width (even a
/// zero-width just-begun drag) — the point is live feedback for what's
/// currently selected, not a judgment call on whether it would validate as a
/// loop yet (that's `loop_range_valid`'s job, applied once the drag ends).
/// `None` only when there's no known audio duration to lay it out against.
pub(super) fn drag_marker_geometry(
    origin_time: f64,
    current_time: f64,
    duration_secs: f64,
) -> Option<(f32, f32)> {
    if duration_secs <= 0.0 {
        return None;
    }
    let a = (origin_time / duration_secs).clamp(0.0, 1.0) as f32;
    let b = (current_time / duration_secs).clamp(0.0, 1.0) as f32;
    let left = a.min(b);
    Some((left, (a.max(b) - left).max(0.0)))
}

/// Left offset and width (fractions 0..1) for a phrase section's rectangle,
/// laid out on the same `duration_secs` timescale as the waveform/note
/// markers — same shape as [`loop_marker_geometry`], but unconditional on an
/// "active"/`SongEnd` flag since every section is always shown. `None` only
/// when there's no known audio duration to lay it out against.
fn phrase_rect_geometry(start_time: f64, end_time: f64, duration_secs: f64) -> Option<(f32, f32)> {
    if duration_secs <= 0.0 {
        return None;
    }
    let left = (start_time / duration_secs).clamp(0.0, 1.0) as f32;
    let right = (end_time / duration_secs).clamp(0.0, 1.0) as f32;
    Some((left, (right - left).max(0.0)))
}

/// Semi-transparent fill for a phrase-section rectangle: dim gray
/// (unlearned) to green (fully learned), linearly interpolated by `learned`
/// (clamped to 0..=1). Low alpha so it reads as a tint over the bar rather
/// than a solid block — [`PHRASE_RECT_BORDER_COLOR`] is fully opaque so
/// sections stay visually distinct regardless of how close two fills land.
fn phrase_fill_color(learned: f32) -> Color {
    let t = learned.clamp(0.0, 1.0);
    Color::srgba(
        0.35 + (0.20 - 0.35) * t,
        0.35 + (0.85 - 0.35) * t,
        0.40 + (0.35 - 0.40) * t,
        0.45,
    )
}

/// Fully opaque border color for every phrase-section rectangle — constant
/// regardless of learned%, so the boundary between sections is always
/// crisp even when two neighbors' fills are nearly the same color.
const PHRASE_RECT_BORDER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 1.0);

/// Converts a `RelativeCursorPosition::normalized` reading (-0.5..0.5 within
/// the node's bounds, per its own doc comment; not clamped when the pointer
/// is outside them, which happens constantly mid-drag) into a clamped
/// 0..`duration_secs` time. `None` for an untracked cursor position or a
/// non-positive duration — nothing sensible to drag against yet.
fn cursor_to_time(normalized_x: Option<f32>, duration_secs: f64) -> Option<f64> {
    let x = normalized_x?;
    if duration_secs <= 0.0 {
        return None;
    }
    let frac = (x as f64 + 0.5).clamp(0.0, 1.0);
    Some(frac * duration_secs)
}

/// Kept in step with `Paused`: the bar is only editable while the game is
/// actually paused — dragging a loop range while notes keep flying by would
/// mean fighting the clock the whole time.
fn sync_progress_bar_mode(paused: Res<Paused>, mut mode: ResMut<ProgressBarMode>) {
    let wanted = if paused.0 {
        ProgressBarMode::Edit
    } else {
        ProgressBarMode::Visualization
    };
    if *mode != wanted {
        *mode = wanted;
    }
}

fn on_drag_start(
    ev: On<Pointer<DragStart>>,
    mode: Res<ProgressBarMode>,
    duration: Res<AudioDuration>,
    surfaces: Query<&RelativeCursorPosition, With<ProgressBarDragSurface>>,
    mut drag: ResMut<LoopDrag>,
) {
    if *mode != ProgressBarMode::Edit || ev.button != PointerButton::Primary {
        return;
    }
    let Ok(rel) = surfaces.get(ev.entity) else {
        return;
    };
    let Some(time) = cursor_to_time(rel.normalized.map(|n| n.x), duration.0) else {
        return;
    };
    *drag = LoopDrag {
        active: true,
        origin_time: time,
        current_time: time,
    };
}

fn on_drag(
    ev: On<Pointer<Drag>>,
    mode: Res<ProgressBarMode>,
    duration: Res<AudioDuration>,
    surfaces: Query<&RelativeCursorPosition, With<ProgressBarDragSurface>>,
    mut drag: ResMut<LoopDrag>,
) {
    if *mode != ProgressBarMode::Edit || !drag.active || ev.button != PointerButton::Primary {
        return;
    }
    let Ok(rel) = surfaces.get(ev.entity) else {
        return;
    };
    if let Some(time) = cursor_to_time(rel.normalized.map(|n| n.x), duration.0) {
        drag.current_time = time;
    }
}

/// Releasing the mouse ends the drag and requests the range it swept out —
/// see [`RequestLoopRange`]'s doc comment for why this doesn't just write
/// `LoopConfig` directly.
fn on_drag_end(
    ev: On<Pointer<DragEnd>>,
    mut drag: ResMut<LoopDrag>,
    mut requests: MessageWriter<RequestLoopRange>,
) {
    if !drag.active || ev.button != PointerButton::Primary {
        return;
    }
    drag.active = false;
    requests.write(RequestLoopRange {
        start_time: drag.origin_time.min(drag.current_time),
        end_time: drag.origin_time.max(drag.current_time),
    });
}

/// Applies a requested loop range to `LoopConfig`. `loop_range_valid` decides
/// `active`, so a degenerate (zero-width) drag cleanly ends up inactive
/// rather than a stale range with a confusing on-screen marker.
fn apply_requested_loop_range(
    mut requests: MessageReader<RequestLoopRange>,
    mut loop_cfg: ResMut<LoopConfig>,
) {
    for req in requests.read() {
        loop_cfg.start_time = req.start_time;
        loop_cfg.end_time = req.end_time;
        loop_cfg.active = loop_range_valid(req.start_time, req.end_time);
    }
}

/// Keeps the loop marker in step with either an in-progress drag (live
/// preview, takes priority) or the committed `LoopConfig` (once the drag
/// ends and `apply_requested_loop_range` has caught up — ordered `.before`
/// this system so both happen in the same frame). Not gated on `Paused` for
/// the `LoopConfig` branch — a range set before pausing again should still
/// render — but dragging itself is only ever possible while paused.
fn update_loop_marker(
    loop_cfg: Res<LoopConfig>,
    song_end: Res<SongEnd>,
    duration: Res<AudioDuration>,
    drag: Res<LoopDrag>,
    mut markers: Query<(&mut Node, &mut Visibility), With<LoopRangeMarker>>,
) {
    if !loop_cfg.is_changed() && !song_end.is_changed() && !drag.is_changed() {
        return;
    }
    let geometry = if drag.active {
        drag_marker_geometry(drag.origin_time, drag.current_time, duration.0)
    } else {
        loop_marker_geometry(
            loop_cfg.active,
            loop_cfg.start_time,
            loop_cfg.end_time,
            song_end.0.is_finite() && song_end.0 > 0.0,
            duration.0,
        )
    };
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

/// Re-tints each phrase-section rectangle when `AdaptiveDifficulty` changes
/// (a manual pause-menu edit, or a fresh song's initial load) without
/// respawning — the rectangles themselves are only ever (re)created in
/// [`spawn_song_progress`], since their count/geometry only changes when the
/// song itself does.
fn update_phrase_section_colors(
    adaptive: Res<AdaptiveDifficulty>,
    mut rects: Query<(&PhraseSectionRect, &mut BackgroundColor)>,
) {
    if !adaptive.is_changed() {
        return;
    }
    for (rect, mut color) in &mut rects {
        let learned = adaptive.learned.get(rect.0).copied().unwrap_or(0.0);
        *color = BackgroundColor(phrase_fill_color(learned));
    }
}

pub struct SongProgressPlugin;

impl Plugin for SongProgressPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioDuration>()
            .init_resource::<ProgressBarMode>()
            .init_resource::<LoopDrag>()
            .add_message::<RequestLoopRange>()
            .add_systems(
                Update,
                update_progress.run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
            )
            .add_systems(
                Update,
                (
                    sync_progress_bar_mode,
                    apply_requested_loop_range,
                    update_loop_marker,
                )
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                update_phrase_section_colors.run_if(in_state(AppState::Playing)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── phrase_rect_geometry / phrase_fill_color ──────────────────────────────

    #[test]
    fn phrase_rect_hidden_without_a_known_audio_duration() {
        assert_eq!(phrase_rect_geometry(0.0, 8.0, 0.0), None);
    }

    #[test]
    fn phrase_rect_geometry_is_a_fraction_of_the_audio_duration() {
        let (left, width) = phrase_rect_geometry(8.0, 16.0, 64.0).unwrap();
        assert!((left - 0.125).abs() < 1e-6);
        assert!((width - 0.125).abs() < 1e-6);
    }

    #[test]
    fn phrase_fill_color_is_dim_gray_when_unlearned() {
        let Color::Srgba(c) = phrase_fill_color(0.0) else {
            panic!("expected Srgba");
        };
        assert!((c.red - 0.35).abs() < 1e-6);
        assert!((c.green - 0.35).abs() < 1e-6);
    }

    #[test]
    fn phrase_fill_color_is_green_when_fully_learned() {
        let Color::Srgba(c) = phrase_fill_color(1.0) else {
            panic!("expected Srgba");
        };
        assert!((c.red - 0.20).abs() < 1e-6);
        assert!((c.green - 0.85).abs() < 1e-6);
    }

    #[test]
    fn phrase_fill_color_clamps_out_of_range_input() {
        assert_eq!(phrase_fill_color(-1.0), phrase_fill_color(0.0));
        assert_eq!(phrase_fill_color(2.0), phrase_fill_color(1.0));
    }

    // ── loop_marker_geometry (committed LoopConfig) ───────────────────────────

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

    // ── drag_marker_geometry (live drag preview) ──────────────────────────────

    #[test]
    fn drag_marker_hidden_without_a_known_audio_duration() {
        assert_eq!(drag_marker_geometry(4.0, 8.0, 0.0), None);
    }

    #[test]
    fn drag_marker_geometry_normalizes_direction() {
        // Dragging right-to-left (current before origin) should still yield
        // the same left/width as dragging left-to-right.
        let forward = drag_marker_geometry(8.0, 16.0, 64.0).unwrap();
        let backward = drag_marker_geometry(16.0, 8.0, 64.0).unwrap();
        assert_eq!(forward, backward);
        assert!((forward.0 - 0.125).abs() < 1e-6);
        assert!((forward.1 - 0.125).abs() < 1e-6);
    }

    #[test]
    fn drag_marker_geometry_shows_a_zero_width_preview_at_drag_start() {
        let (left, width) = drag_marker_geometry(10.0, 10.0, 64.0).unwrap();
        assert!((left - 10.0 / 64.0).abs() < 1e-6);
        assert_eq!(width, 0.0);
    }

    // ── cursor_to_time ─────────────────────────────────────────────────────────

    #[test]
    fn cursor_to_time_is_none_without_a_tracked_cursor() {
        assert_eq!(cursor_to_time(None, 60.0), None);
    }

    #[test]
    fn cursor_to_time_is_none_without_a_positive_duration() {
        assert_eq!(cursor_to_time(Some(0.0), 0.0), None);
    }

    #[test]
    fn cursor_to_time_maps_the_node_span_to_the_full_duration() {
        // -0.5 (left edge) .. 0.5 (right edge) maps to 0..duration.
        assert_eq!(cursor_to_time(Some(-0.5), 60.0), Some(0.0));
        assert_eq!(cursor_to_time(Some(0.5), 60.0), Some(60.0));
        assert_eq!(cursor_to_time(Some(0.0), 60.0), Some(30.0));
    }

    #[test]
    fn cursor_to_time_clamps_outside_the_node_bounds() {
        // Dragging past either edge of the bar clamps to the endpoints
        // instead of extrapolating past 0 or the duration.
        assert_eq!(cursor_to_time(Some(-2.0), 60.0), Some(0.0));
        assert_eq!(cursor_to_time(Some(2.0), 60.0), Some(60.0));
    }

    // ── apply_requested_loop_range ─────────────────────────────────────────────

    #[test]
    fn apply_requested_loop_range_activates_a_valid_range() {
        let mut world = World::new();
        world.insert_resource(LoopConfig::default());
        world.init_resource::<Messages<RequestLoopRange>>();
        world.write_message(RequestLoopRange {
            start_time: 8.0,
            end_time: 16.0,
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_requested_loop_range);
        schedule.run(&mut world);
        let cfg = world.resource::<LoopConfig>();
        assert!(cfg.active);
        assert_eq!(cfg.start_time, 8.0);
        assert_eq!(cfg.end_time, 16.0);
    }

    #[test]
    fn apply_requested_loop_range_leaves_a_degenerate_range_inactive() {
        let mut world = World::new();
        world.insert_resource(LoopConfig::default());
        world.init_resource::<Messages<RequestLoopRange>>();
        world.write_message(RequestLoopRange {
            start_time: 8.0,
            end_time: 8.0,
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_requested_loop_range);
        schedule.run(&mut world);
        assert!(!world.resource::<LoopConfig>().active);
    }

    // ── sync_progress_bar_mode ─────────────────────────────────────────────────

    #[test]
    fn sync_progress_bar_mode_follows_paused() {
        let mut world = World::new();
        world.insert_resource(Paused(true));
        world.insert_resource(ProgressBarMode::Visualization);
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_progress_bar_mode);
        schedule.run(&mut world);
        assert_eq!(*world.resource::<ProgressBarMode>(), ProgressBarMode::Edit);

        world.insert_resource(Paused(false));
        schedule.run(&mut world);
        assert_eq!(
            *world.resource::<ProgressBarMode>(),
            ProgressBarMode::Visualization
        );
    }
}
