// SPDX-License-Identifier: MIT

//! Everything that moves the Song Editor's *view* rather than its content:
//! the grid's horizontal pan (keys, wheel, the scrollbar and its markers),
//! the two-finger pan that also offsets the whole editor vertically, and the
//! left tool sidebar's own drag/wheel scrolling.
//!

use bevy::input::mouse::MouseWheel;
use bevy::input::touch::Touches;
use bevy::input_focus::InputFocus;
use bevy::picking::events::PointerDrag;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::ComputedNode;

use super::interaction::a_text_field_has_focus;
use super::playback::Playhead;
use super::state::{Dir, EditorState, Scroll, ToolbarScroll};
use super::ui::{
    EditorRoot, EditorToolbar, EditorToolbarContent, GridArea, GridContent, GridScrollMarker,
    GridScrollThumb, GridScrollTrack,
};
use super::{BEAT_W, TICK_W};
use harmonicon_ui::dialogs::file_dialog::FileDialog;

pub(super) fn pan_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    file_dialog: Res<FileDialog>,
    mut scroll: ResMut<Scroll>,
    focus: Res<InputFocus>,
    fields: Query<(), With<EditableText>>,
) {
    if a_text_field_has_focus(&focus, &fields) || file_dialog.open {
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        scroll.px += BEAT_W;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        scroll.px = (scroll.px - BEAT_W).max(0.0);
    }
}

/// Pans the grid horizontally on wheel input — but only while the pointer
/// is actually over the grid ([`GridArea`]'s own [`Hovered`]). Without this
/// gate, scrolling anywhere else in the editor (the meta/lesson form, say)
/// would also pan the grid sideways while the vertical `ScrollArea` scrolls
/// the page — two unrelated scroll effects from one wheel gesture. Wheel
/// events are still drained every frame regardless of hover, so an
/// un-applied gesture can't linger and pan the grid late once the pointer
/// moves onto it.
pub(super) fn pan_wheel(
    mut wheel: MessageReader<MouseWheel>,
    file_dialog: Res<FileDialog>,
    mut scroll: ResMut<Scroll>,
    grid_hovered: Query<&Hovered, With<GridArea>>,
) {
    if file_dialog.open {
        wheel.clear();
        return;
    }
    let mut delta = 0.0;
    for ev in wheel.read() {
        delta += if ev.y != 0.0 { ev.y } else { ev.x };
    }
    let hovered = grid_hovered.single().is_ok_and(Hovered::get);
    if delta != 0.0 && hovered {
        scroll.px = (scroll.px - delta * BEAT_W).max(0.0);
    }
}

/// The pan a two-finger gesture asks for this frame, given every active
/// touch's per-frame movement — or `None` when the gesture isn't two fingers.
///
/// Two fingers rather than one because one finger is already spoken for
/// everywhere in the grid: placing a note, dragging one, resizing it, or
/// dragging a timeline span. Two is the only unambiguous "move the view"
/// gesture left, and it's what every drawing/DAW app on a tablet uses.
///
/// Averaging the two movements (rather than taking one finger's, or the
/// larger) is what makes this survive alongside a future pinch-zoom: fingers
/// moving *apart* have near-opposite deltas that average to roughly zero, so
/// a pinch barely pans, while a genuine two-finger drag has both fingers
/// moving together and averages to their shared motion.
pub(super) fn two_finger_pan_delta(touch_deltas: &[Vec2]) -> Option<Vec2> {
    match touch_deltas {
        [a, b] => Some((*a + *b) / 2.0),
        _ => None,
    }
}

/// Scrolls the tool sidebar by dragging anywhere on it.
///
/// A drag rather than a scrollbar because the sidebar is deliberately narrow
/// — a thumb inside a 56 px column is an unhittable target on a phone, and
/// there is no wheel there at all. Dragging the surface itself is the
/// standard touch idiom, and it costs desktop nothing (the wheel still works
/// via the ordinary hover path elsewhere).
///
/// Buttons inside the sidebar keep working: `bevy_picking` only starts a
/// drag once the pointer actually *moves* while pressed, so a stationary
/// press still resolves to the button's own `Activate`.
pub(super) fn drag_toolbar(
    ev: On<PointerDrag>,
    ui_scale: Res<UiScale>,
    mut scroll: ResMut<ToolbarScroll>,
    toolbars: Query<(&ComputedNode, &Children), With<EditorToolbar>>,
    child_nodes: Query<&ComputedNode>,
) {
    let Some(max_y) = toolbar_max_scroll(&toolbars, &child_nodes) else {
        return;
    };
    // Dragging up moves the content up, revealing what's below it — same
    // sign convention as `pan_touch`'s vertical axis.
    scroll.y_px = (scroll.y_px - ev.delta.y / ui_scale.0).clamp(0.0, max_y);
}

/// Scrolls the tool sidebar on wheel input, while the pointer is over it.
///
/// The hover gate is what keeps this and [`pan_wheel`] from both firing on
/// one gesture: that one only pans while the *grid* is hovered, this one
/// only while the *toolbar* is, and the two never overlap. Wheel messages
/// are drained every frame either way, so an unapplied gesture can't linger
/// and scroll something later once the pointer moves.
pub(super) fn wheel_toolbar(
    mut wheel: MessageReader<MouseWheel>,
    file_dialog: Res<FileDialog>,
    mut scroll: ResMut<ToolbarScroll>,
    hovered: Query<&Hovered, With<EditorToolbar>>,
    toolbars: Query<(&ComputedNode, &Children), With<EditorToolbar>>,
    child_nodes: Query<&ComputedNode>,
) {
    if file_dialog.open {
        wheel.clear();
        return;
    }
    let mut delta = 0.0;
    for ev in wheel.read() {
        delta += if ev.y != 0.0 { ev.y } else { ev.x };
    }
    if delta == 0.0 || !hovered.single().is_ok_and(Hovered::get) {
        return;
    }
    let Some(max_y) = toolbar_max_scroll(&toolbars, &child_nodes) else {
        return;
    };
    // One notch moves by a button's rough height, so a flick covers the
    // palette rather than creeping a few pixels.
    scroll.y_px = (scroll.y_px - delta * TOOLBAR_WHEEL_STEP).clamp(0.0, max_y);
}

/// One wheel notch's worth of toolbar scroll, in logical px — roughly one
/// button plus its gap, so the palette moves a whole entry at a time.
const TOOLBAR_WHEEL_STEP: f32 = 44.0;

/// How far the sidebar's content can scroll before its bottom edge reaches
/// the viewport's, or `None` when there's no toolbar to measure.
///
/// Shared by [`drag_toolbar`] and [`wheel_toolbar`] so the two can't drift
/// into disagreeing about where the end of the list is.
fn toolbar_max_scroll(
    toolbars: &Query<(&ComputedNode, &Children), With<EditorToolbar>>,
    child_nodes: &Query<&ComputedNode>,
) -> Option<f32> {
    let (toolbar, children) = toolbars.single().ok()?;
    let inv = toolbar.inverse_scale_factor();
    let viewport_h = toolbar.size().y * inv;
    let content_h: f32 = children
        .iter()
        .filter_map(|child| child_nodes.get(child).ok())
        .map(|node| node.size().y * inv)
        .sum();
    Some((content_h - viewport_h).max(0.0))
}

/// Applies [`ToolbarScroll`] to the sidebar's content column, the same way
/// [`apply_scroll`] offsets the grid and the editor root.
pub(super) fn apply_toolbar_scroll(
    scroll: Res<ToolbarScroll>,
    mut content: Query<&mut Node, With<EditorToolbarContent>>,
) {
    if !scroll.is_changed() {
        return;
    }
    if let Ok(mut node) = content.single_mut() {
        node.top = Val::Px(-scroll.y_px);
    }
}

/// How far the editor can be panned up before its last child's bottom edge
/// reaches the bottom of the viewport — `0.0` when everything already fits,
/// so the gesture is inert on a screen with room to spare.
pub(super) fn vertical_overflow_px(viewport_h: f32, child_heights: &[f32]) -> f32 {
    (child_heights.iter().sum::<f32>() - viewport_h).max(0.0)
}

/// Pans the editor with a two-finger drag — the touch equivalent of
/// [`pan_wheel`], and the only way to reach the mod panel on a phone.
///
/// Two axes, and they move different things, because the editor has two
/// unrelated overflows:
/// - **x** scrolls within the grid ([`Scroll::px`]), like [`pan_wheel`].
/// - **y** moves the *whole editor* ([`Scroll::y_px`]). What's off-screen
///   vertically is the fixed chrome — grid plus mod panel — which sits
///   outside the form's `ScrollArea` by design, so no inner scroll can
///   reach it. See [`Scroll::y_px`].
///
/// Unlike [`pan_wheel`] this is *not* gated on the grid being hovered. That
/// gate exists because one wheel gesture would otherwise both scroll the
/// meta form and pan the grid; a two-finger drag has no such conflict, since
/// nothing else in the editor responds to two fingers. It also avoids
/// depending on hover semantics for touch pointers, which only exist for the
/// duration of the touch itself.
pub(super) fn pan_touch(
    touches: Res<Touches>,
    ui_scale: Res<UiScale>,
    file_dialog: Res<FileDialog>,
    mut scroll: ResMut<Scroll>,
    roots: Query<(&ComputedNode, &Children), With<EditorRoot>>,
    child_nodes: Query<&ComputedNode>,
) {
    if file_dialog.open {
        return;
    }
    let mut active = touches.iter();
    let (Some(first), Some(second), None) = (active.next(), active.next(), active.next()) else {
        return;
    };
    let pan = two_finger_pan_delta(&[first.delta(), second.delta()]).unwrap();
    // Dragging right should carry the content right with the fingers, which
    // means scrolling *back* towards the start. Divided by `UiScale` for the
    // same reason `drag_grid_scrollbar` does it: touch deltas are window
    // pixels, `Scroll::px` is content pixels.
    scroll.px = (scroll.px - pan.x / ui_scale.0).max(0.0);

    let Ok((root, children)) = roots.single() else {
        return;
    };
    // `ComputedNode` sizes are physical; everything else here is logical.
    let inv = root.inverse_scale_factor();
    let viewport_h = root.size().y * inv;
    let content_h: f32 = children
        .iter()
        .filter_map(|child| child_nodes.get(child).ok())
        .map(|node| node.size().y * inv)
        .sum();
    let max_y = vertical_overflow_px(viewport_h, &[content_h]);
    // Dragging *up* (negative y) should reveal what's below, so the offset
    // grows as the fingers move up.
    scroll.y_px = (scroll.y_px - pan.y / ui_scale.0).clamp(0.0, max_y);
}

pub(super) fn apply_scroll(
    scroll: Res<Scroll>,
    mut state: ResMut<EditorState>,
    mut content: Query<&mut Node, With<GridContent>>,
    mut roots: Query<&mut Node, (With<EditorRoot>, Without<GridContent>)>,
) {
    if let Ok(mut node) = content.single_mut() {
        let left = Val::Px(-scroll.px);
        if node.left != left {
            node.left = left;
        }
    }
    // Shifts the entire editor up so the fixed chrome below the grid (mod
    // panel, status bar) can be reached on a screen too short to show it —
    // see `Scroll::y_px`. Untouched at 0, which is every desktop case.
    if let Ok(mut node) = roots.single_mut() {
        let top = Val::Px(-scroll.y_px);
        if node.top != top {
            node.top = top;
        }
    }
    let base = (scroll.px / BEAT_W) as usize;
    if state.scroll_beat != base {
        state.scroll_beat = base;
    }
}

pub(super) fn auto_scroll(
    playhead: Res<Playhead>,
    windows: Query<&Window>,
    mut scroll: ResMut<Scroll>,
) {
    if !playhead.playing || playhead.secs_per_tick <= 0.0 {
        return;
    }
    const FOLLOW_LEAD: f32 = 0.7;
    let view_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0) - super::HOLE_COL_W;
    let head_px = playhead.elapsed / playhead.secs_per_tick * TICK_W;
    let target = head_px - FOLLOW_LEAD * view_w;
    if target > scroll.px {
        scroll.px = target;
    }
}

// ── Horizontal scrollbar ─────────────────────────────────────────────────────

/// Whether the grid's horizontal scrollbar should be shown at all — only
/// once the notes' total span (`total_px`) is wider than what's currently
/// visible (`view_w`); an empty or short song has nothing to scroll to.
fn scrollbar_needed(total_px: f32, view_w: f32) -> bool {
    total_px > view_w
}

/// The narrowest a scrollbar thumb is ever drawn, regardless of how long the
/// song is relative to the view — a proportionally-accurate but vanishingly
/// thin thumb would be unusable to grab.
const MIN_THUMB_W: f32 = 24.0;

/// The scrollbar thumb's width and left offset, in the same px unit as
/// `scroll_px`/`total_px`/`view_w`/`track_w` (the caller's job to keep
/// consistent — see `update_grid_scrollbar`). `total_px` is floored at
/// `view_w` so a song shorter than the view (or empty) still yields a
/// full-width thumb instead of dividing by something smaller than the
/// view — [`scrollbar_needed`] decides whether to show it at all. The
/// thumb's left offset is clamped to the track so it can't run past the
/// track's right edge even if `scroll_px` is momentarily larger than the
/// song supports (e.g. right after deleting notes shortens it out from
/// under the current scroll position).
fn scrollbar_thumb(scroll_px: f32, total_px: f32, view_w: f32, track_w: f32) -> (f32, f32) {
    let total_px = total_px.max(view_w).max(1.0);
    let width = (view_w / total_px * track_w).clamp(MIN_THUMB_W.min(track_w), track_w);
    let max_left = (track_w - width).max(0.0);
    let left = (scroll_px / total_px * track_w).clamp(0.0, max_left);
    (width, left)
}

/// Keeps the scrollbar track's visibility and the thumb's size/position in
/// step with [`Scroll`] and the notes' current span — shown only while
/// there's more song than fits in view (see [`scrollbar_needed`]).
pub(super) fn update_grid_scrollbar(
    scroll: Res<Scroll>,
    state: Res<EditorState>,
    windows: Query<&Window>,
    ui_scale: Res<UiScale>,
    mut tracks: Query<(&ComputedNode, &mut Visibility), With<GridScrollTrack>>,
    mut thumbs: Query<&mut Node, With<GridScrollThumb>>,
) {
    let Ok((track, mut vis)) = tracks.single_mut() else {
        return;
    };
    let Ok(mut thumb) = thumbs.single_mut() else {
        return;
    };
    let view_w =
        windows.iter().next().map(|w| w.width() / ui_scale.0).unwrap_or(1280.0) - super::HOLE_COL_W;
    let total_px = super::ranges::song_end_tick(&state.notes) as f32 * TICK_W;

    if !scrollbar_needed(total_px, view_w) {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    }
    if *vis != Visibility::Visible {
        *vis = Visibility::Visible;
    }
    let track_w = track.size().x * track.inverse_scale_factor();
    let (width, left) = scrollbar_thumb(scroll.px, total_px, view_w, track_w);
    if thumb.width != Val::Px(width) || thumb.left != Val::Px(left) {
        thumb.width = Val::Px(width);
        thumb.left = Val::Px(left);
    }
}

// Same blow/draw hues the gameplay legend and note comets use — named so
// `meta_form`'s color legend can show the same swatches rather than
// duplicating the literals.
pub(super) const SCROLLBAR_BLOW_COLOR: Color = Color::srgba(0.50, 0.75, 1.00, 0.9);
pub(super) const SCROLLBAR_DRAW_COLOR: Color = Color::srgba(1.00, 0.62, 0.35, 0.9);

/// One note's marker geometry on the scrollbar track, as percentages of
/// the track's width: `(left, width)`. Pure tick math — the track's pixel
/// width never enters, so markers need no re-layout on resize. Width is
/// floored so a short note in a long song still shows up as at least a
/// speck, and clamped so a floored marker near the end can't poke past the
/// track.
pub(super) fn scrollbar_marker(tick: usize, len: usize, end_tick: usize) -> (f32, f32) {
    let end = end_tick.max(1) as f32;
    let left = (tick as f32 / end * 100.0).min(100.0);
    let width = (len as f32 / end * 100.0).max(0.3).min(100.0 - left);
    (left, width)
}

/// Rebuilds the scrollbar's note markers (see [`GridScrollMarker`])
/// whenever the notes change: one small rectangle per note, horizontal =
/// its time span across the whole song, vertical = its hole lane — the
/// scrollbar as a minimap, blow/draw keeping their usual colours. All
/// percent-positioned (see [`scrollbar_marker`]) and `Pickable::IGNORE`,
/// so they don't care about the track's pixel size or steal drags from
/// the thumb.
pub(super) fn update_scrollbar_markers(
    mut commands: Commands,
    state: Res<EditorState>,
    tracks: Query<Entity, With<GridScrollTrack>>,
    markers: Query<Entity, With<GridScrollMarker>>,
) {
    let Ok(track) = tracks.single() else {
        return;
    };
    for e in &markers {
        commands.entity(e).despawn();
    }
    let end_tick = super::ranges::song_end_tick(&state.notes);
    if end_tick == 0 {
        return;
    }
    let lanes = state.hole_count().max(1) as f32;
    let new: Vec<Entity> = state
        .notes
        .iter()
        .map(|n| {
            let (left, width) = scrollbar_marker(n.tick, n.len, end_tick);
            // Each lane gets an equal slice of the track's height; the
            // marker fills its lane's slice so adjacent lanes stay distinct.
            let lane = (n.hole.saturating_sub(1)) as f32;
            commands
                .spawn((
                    GridScrollMarker,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(left),
                        width: Val::Percent(width),
                        top: Val::Percent(lane / lanes * 100.0),
                        height: Val::Percent(100.0 / lanes),
                        ..default()
                    },
                    BackgroundColor(match n.dir {
                        Dir::Blow => SCROLLBAR_BLOW_COLOR,
                        Dir::Draw => SCROLLBAR_DRAW_COLOR,
                    }),
                    Pickable::IGNORE,
                ))
                .id()
        })
        .collect();
    commands.entity(track).add_children(&new);
}

/// Drags the thumb to scroll the grid — the drag delta (screen px) is
/// scaled from track-space into content-space (`total_px / track_w`) so
/// dragging the thumb all the way across the track scrolls the full song,
/// not just `track_w` worth of it.
pub(super) fn drag_grid_scrollbar(
    ev: On<PointerDrag>,
    ui_scale: Res<UiScale>,
    state: Res<EditorState>,
    tracks: Query<&ComputedNode, With<GridScrollTrack>>,
    mut scroll: ResMut<Scroll>,
) {
    let Ok(track) = tracks.single() else {
        return;
    };
    let track_w = track.size().x * track.inverse_scale_factor();
    let total_px = super::ranges::song_end_tick(&state.notes) as f32 * TICK_W;
    if track_w <= 0.0 {
        return;
    }
    let delta_px = ev.delta.x / ui_scale.0;
    scroll.px = (scroll.px + delta_px * (total_px / track_w)).max(0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── scrollbar_needed ─────────────────────────────────────────────────────

    #[test]
    fn scrollbar_not_needed_when_the_song_fits_the_view() {
        assert!(!scrollbar_needed(800.0, 1000.0));
        assert!(!scrollbar_needed(1000.0, 1000.0));
    }

    #[test]
    fn scrollbar_needed_when_the_song_is_wider_than_the_view() {
        assert!(scrollbar_needed(1200.0, 1000.0));
    }

    // ── scrollbar_thumb ──────────────────────────────────────────────────────

    #[test]
    fn thumb_width_is_proportional_to_the_visible_fraction() {
        // Twice as much song as fits in view -> half-width thumb.
        let (width, _) = scrollbar_thumb(0.0, 2000.0, 1000.0, 500.0);
        assert!((width - 250.0).abs() < 0.01);
    }

    #[test]
    fn thumb_width_is_never_smaller_than_the_minimum() {
        // 100x as much song as fits in view -> a proportional thumb would be
        // a sliver, but it's floored at MIN_THUMB_W.
        let (width, _) = scrollbar_thumb(0.0, 100_000.0, 1000.0, 500.0);
        assert_eq!(width, MIN_THUMB_W);
    }

    #[test]
    fn thumb_left_tracks_the_scroll_fraction() {
        // Scrolled a quarter of the way through a song twice the view width.
        let (_, left) = scrollbar_thumb(500.0, 2000.0, 1000.0, 500.0);
        assert!((left - 125.0).abs() < 0.01);
    }

    #[test]
    fn thumb_left_never_runs_past_the_tracks_right_edge() {
        // A scroll position beyond what the (now-shorter) song supports —
        // e.g. right after notes were deleted — must still clamp on-track.
        let (width, left) = scrollbar_thumb(10_000.0, 2000.0, 1000.0, 500.0);
        assert!(left + width <= 500.0 + 0.01);
    }

    #[test]
    fn thumb_fills_the_track_when_the_song_is_shorter_than_the_view() {
        let (width, left) = scrollbar_thumb(0.0, 200.0, 1000.0, 500.0);
        assert_eq!(width, 500.0);
        assert_eq!(left, 0.0);
    }
}
