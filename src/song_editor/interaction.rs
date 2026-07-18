// SPDX-License-Identifier: MIT

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::picking::events::{Drag, Pointer};
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::ui_render::prelude::MaterialNode;

use super::material::EditorNoteMaterial;
use super::playback::Playhead;
use super::state::{
    Dir, DragKind, EditorState, Expr, GridNote, Pitch, Scroll, VIBRATO_HZ_MAX, VIBRATO_HZ_MIN,
    VIBRATO_HZ_STEP, WAH_HZ_MAX, WAH_HZ_MIN, WAH_HZ_STEP, enforce_direction, enforce_expr,
    max_bend, note_rect, overblow_ok, overdraw_ok, pitch_compatible, pitch_forced_dir,
};
use super::ui::{GridContent, GridScrollThumb, GridScrollTrack, ModButton, MoveGhost, NoteView};
use super::{AppState, BEAT_W, HEADER_H, NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};
use crate::dialogs::file_dialog::FileDialog;
use crate::theme::LoadedTheme;

// ── Note interaction ─────────────────────────────────────────────────────────

pub(super) fn select_or_add(state: &mut EditorState, hole: u8, tick: usize) {
    if let Some(existing) = state
        .notes
        .iter()
        .find(|n| n.hole == hole && n.tick <= tick && tick < n.tick + n.len)
    {
        state.selected = Some(existing.id);
        return;
    }

    let next_start = state
        .notes
        .iter()
        .filter(|n| n.hole == hole && n.tick > tick)
        .map(|n| n.tick)
        .min();

    let len = next_start
        .map_or(TICKS_PER_BEAT, |start| (start - tick).min(TICKS_PER_BEAT))
        .max(1);

    // Whatever's already sounding at this exact tick (on another hole)
    // wins over the armed sticky direction — a brand-new chord note has to
    // match its siblings, not fight them. `sticky_dir` only applies when
    // there's nothing there yet to match.
    let mut dir = state.dir_at(tick).unwrap_or(state.sticky_dir);
    // A sticky pitch that doesn't fit *this particular* hole (e.g. armed
    // Overblow while placing a note on hole 8) silently falls back to
    // Normal for just this note — same "silently do nothing on an
    // incompatible hole" rule clicking the button on a selected note
    // already has — rather than rejecting the whole placement.
    let pitch = if pitch_compatible(state.sticky_pitch, hole) {
        state.sticky_pitch
    } else {
        Pitch::Normal
    };
    // Overblow/Overdraw physically require a specific breath direction
    // (see `pitch_forced_dir`) — that always wins, even over whatever's
    // already sounding at this tick, since a mismatched pairing (e.g.
    // "overblow" on a note tagged Draw) can't exist for real.
    if let Some(forced) = pitch_forced_dir(pitch) {
        dir = forced;
    }
    let expr = state.sticky_expr;

    let id = state.next_id;
    state.next_id += 1;
    state.notes.push(GridNote {
        id,
        hole,
        tick,
        len,
        dir,
        pitch,
        expr,
    });
    state.selected = Some(id);
    // A chord note whose direction was forced (above), or that's carrying
    // an armed sticky expr, must pull any simultaneous notes on other
    // holes into agreement too — direction and wah/vibrato are both
    // whole-player techniques, not per-hole.
    if pitch_forced_dir(pitch).is_some() {
        enforce_direction(state, id);
    }
    if expr != Expr::None {
        enforce_expr(state, id);
    }
}

pub(super) fn delete_selected(state: &mut EditorState) {
    if let Some(id) = state.selected.take() {
        state.notes.retain(|n| n.id != id);
    }
}

pub(super) fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    if kind == ModButton::Delete {
        delete_selected(state);
        return;
    }
    if matches!(kind, ModButton::Blow | ModButton::Draw) {
        let dir = if kind == ModButton::Blow {
            Dir::Blow
        } else {
            Dir::Draw
        };
        // Arms the sticky direction regardless of whether anything is
        // selected — a note to edit is optional, arming for future notes
        // isn't. An armed Overblow/Overdraw that no longer matches this
        // direction can't survive the switch (see `pitch_forced_dir`) —
        // clear it rather than leave e.g. "overblow" armed alongside Draw.
        state.sticky_dir = dir;
        if pitch_forced_dir(state.sticky_pitch).is_some_and(|d| d != dir) {
            state.sticky_pitch = Pitch::Normal;
        }
        if let Some(id) = state.selected {
            if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                n.dir = dir;
                if pitch_forced_dir(n.pitch).is_some_and(|d| d != dir) {
                    n.pitch = Pitch::Normal;
                }
            }
            enforce_direction(state, id);
        }
        return;
    }

    let Some(id) = state.selected else {
        // Nothing to edit, but every pitch/expr button still needs to
        // arm/cycle for notes not yet placed — cycles `sticky_pitch`/
        // `sticky_expr` directly instead of a selected note's own field.
        match kind {
            ModButton::Bend => cycle_sticky_bend(state),
            ModButton::Overblow => cycle_sticky_pitch(state, Pitch::Overblow),
            ModButton::Overdraw => cycle_sticky_pitch(state, Pitch::Overdraw),
            ModButton::Slide => cycle_sticky_pitch(state, Pitch::Slide),
            ModButton::Wah => cycle_sticky_wah(state),
            ModButton::Vibrato => cycle_sticky_vibrato(state),
            _ => {}
        }
        return;
    };

    let Some(note) = state.selected_note_mut() else {
        return;
    };
    match kind {
        ModButton::Blow | ModButton::Draw => unreachable!(),
        ModButton::Bend => {
            let max = max_bend(note.hole);
            if max <= 0.0 {
                return;
            }
            let next = note.bend() + 0.5;
            note.pitch = if next > max + f32::EPSILON {
                Pitch::Normal
            } else {
                Pitch::Bend(next)
            };
        }
        ModButton::Overblow => {
            if overblow_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overblow {
                    Pitch::Normal
                } else {
                    Pitch::Overblow
                };
                // Overblow only exists while blowing — force it so the
                // note can't end up "overblow" while tagged Draw.
                if note.pitch == Pitch::Overblow {
                    note.dir = Dir::Blow;
                }
            }
        }
        ModButton::Overdraw => {
            if overdraw_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overdraw {
                    Pitch::Normal
                } else {
                    Pitch::Overdraw
                };
                if note.pitch == Pitch::Overdraw {
                    note.dir = Dir::Draw;
                }
            }
        }
        ModButton::Slide => {
            note.pitch = if note.pitch == Pitch::Slide {
                Pitch::Normal
            } else {
                Pitch::Slide
            };
        }
        ModButton::Wah => {
            let next = match note.expr {
                Expr::Wah(hz) => hz + WAH_HZ_STEP,
                _ => WAH_HZ_MIN,
            };
            note.expr = if next > WAH_HZ_MAX + f32::EPSILON {
                Expr::None
            } else {
                Expr::Wah(next)
            };
        }
        ModButton::Vibrato => {
            let next = match note.expr {
                Expr::Vibrato(hz) => hz + VIBRATO_HZ_STEP,
                _ => VIBRATO_HZ_MIN,
            };
            note.expr = if next > VIBRATO_HZ_MAX + f32::EPSILON {
                Expr::None
            } else {
                Expr::Vibrato(next)
            };
        }
        ModButton::Delete => unreachable!(),
    }
    // Read the note's resulting pitch/expr/dir out before writing to
    // `state` again below — `note` is still borrowing it at this point.
    let (new_pitch, new_expr, new_dir) = (note.pitch, note.expr, note.dir);

    // Arm sticky to match whatever the selected note now holds, so the
    // next *added* note (`select_or_add`) picks up the same setting.
    match kind {
        ModButton::Bend | ModButton::Overblow | ModButton::Overdraw | ModButton::Slide => {
            state.sticky_pitch = new_pitch;
            // Overblow/Overdraw forced `note.dir` above — mirror that into
            // the sticky direction too, and pull any simultaneous notes on
            // other holes into agreement (direction is whole-player, not
            // per-hole).
            if pitch_forced_dir(new_pitch).is_some() {
                state.sticky_dir = new_dir;
                enforce_direction(state, id);
            }
        }
        ModButton::Wah | ModButton::Vibrato => {
            state.sticky_expr = new_expr;
            enforce_expr(state, id);
        }
        _ => {}
    }
}

/// Cycles `sticky_pitch`'s bend depth with nothing selected, so there's no
/// specific hole to cap it against — uses 1.5, the richest cap any hole has
/// (holes 2/3/10, see `max_bend`), so cycling here is never cut short by a
/// hole that isn't even involved yet. `select_or_add` re-validates against
/// the real hole once a note actually gets placed.
fn cycle_sticky_bend(state: &mut EditorState) {
    let current = match state.sticky_pitch {
        Pitch::Bend(depth) => depth,
        _ => 0.0,
    };
    let next = current + 0.5;
    state.sticky_pitch = if next > 1.5 + f32::EPSILON {
        Pitch::Normal
    } else {
        Pitch::Bend(next)
    };
}

/// Toggles `sticky_pitch` between `Pitch::Normal` and `pitch` — the
/// hole-free sticky-only equivalent of the selected-note Overblow/
/// Overdraw/Slide toggles below (which additionally gate on the selected
/// note's own hole via `overblow_ok`/`overdraw_ok`).
fn cycle_sticky_pitch(state: &mut EditorState, pitch: Pitch) {
    state.sticky_pitch = if state.sticky_pitch == pitch {
        Pitch::Normal
    } else {
        pitch
    };
    // Arming Overblow/Overdraw with nothing selected must arm the
    // direction it requires too — otherwise a subsequently placed note
    // could still end up with e.g. `sticky_pitch: Overblow` alongside a
    // stale `sticky_dir: Draw` from something clicked earlier.
    if let Some(dir) = pitch_forced_dir(state.sticky_pitch) {
        state.sticky_dir = dir;
    }
}

fn cycle_sticky_wah(state: &mut EditorState) {
    let next = match state.sticky_expr {
        Expr::Wah(hz) => hz + WAH_HZ_STEP,
        _ => WAH_HZ_MIN,
    };
    state.sticky_expr = if next > WAH_HZ_MAX + f32::EPSILON {
        Expr::None
    } else {
        Expr::Wah(next)
    };
}

fn cycle_sticky_vibrato(state: &mut EditorState) {
    let next = match state.sticky_expr {
        Expr::Vibrato(hz) => hz + VIBRATO_HZ_STEP,
        _ => VIBRATO_HZ_MIN,
    };
    state.sticky_expr = if next > VIBRATO_HZ_MAX + f32::EPSILON {
        Expr::None
    } else {
        Expr::Vibrato(next)
    };
}

// ── Keyboard / scroll systems ─────────────────────────────────────────────────

/// Escape first deselects the current note (if any); pressed again with
/// nothing selected, it leaves the editor for the menu — same "back" rule
/// every other screen follows. Suppressed while a save/load dialog is open,
/// since that dialog handles its own Escape (closes itself).
pub(super) fn grid_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
    file_dialog: Res<FileDialog>,
    mut next_state: ResMut<NextState<AppState>>,
    mut ret_play: ResMut<crate::app::ReturnToPlay>,
) {
    if state.focus.is_some() {
        return;
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        delete_selected(&mut state);
    }
    if keyboard.just_pressed(KeyCode::Escape) && !file_dialog.open {
        if state.timeline_drag.is_some() || state.timeline_split.is_some() {
            state.timeline_drag = None;
            state.timeline_split = None;
        } else if state.selected.is_some() {
            state.selected = None;
        } else {
            ret_play.0 = true;
            next_state.set(AppState::Menu);
        }
    }
}

pub(super) fn type_into_field(
    mut keys: MessageReader<KeyboardInput>,
    mut state: ResMut<EditorState>,
) {
    let Some(field) = state.focus else {
        keys.clear();
        return;
    };
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            bevy::input::keyboard::Key::Character(s) => {
                for c in s.chars() {
                    if !c.is_control() {
                        state.field_text_mut(field).push(c);
                    }
                }
            }
            bevy::input::keyboard::Key::Space => state.field_text_mut(field).push(' '),
            bevy::input::keyboard::Key::Backspace => {
                state.field_text_mut(field).pop();
            }
            bevy::input::keyboard::Key::Enter | bevy::input::keyboard::Key::Escape => {
                state.focus = None;
            }
            _ => {}
        }
    }
}

pub(super) fn pan_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    state: Res<EditorState>,
    file_dialog: Res<FileDialog>,
    mut scroll: ResMut<Scroll>,
) {
    if state.focus.is_some() || file_dialog.open {
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        scroll.px += BEAT_W;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        scroll.px = (scroll.px - BEAT_W).max(0.0);
    }
}

pub(super) fn pan_wheel(
    mut wheel: MessageReader<MouseWheel>,
    file_dialog: Res<FileDialog>,
    mut scroll: ResMut<Scroll>,
) {
    if file_dialog.open {
        wheel.clear();
        return;
    }
    let mut delta = 0.0;
    for ev in wheel.read() {
        delta += if ev.y != 0.0 { ev.y } else { ev.x };
    }
    if delta != 0.0 {
        scroll.px = (scroll.px - delta * BEAT_W).max(0.0);
    }
}

pub(super) fn apply_scroll(
    scroll: Res<Scroll>,
    mut state: ResMut<EditorState>,
    mut content: Query<&mut Node, With<GridContent>>,
) {
    if let Ok(mut node) = content.single_mut() {
        node.left = Val::Px(-scroll.px);
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
/// full-width thumb rather than dividing by something smaller than the
/// view — [`scrollbar_needed`] is what actually decides whether to show it
/// at all. The thumb's left offset is clamped to the track so it can't run
/// past the track's own right edge even if `scroll_px` is momentarily
/// larger than the song supports (e.g. right after deleting notes shortens
/// it out from under the current scroll position).
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
    let view_w = windows
        .iter()
        .next()
        .map(|w| w.width() / ui_scale.0)
        .unwrap_or(1280.0)
        - super::HOLE_COL_W;
    let total_px = super::state::song_end_tick(&state.notes) as f32 * TICK_W;

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
    thumb.width = Val::Px(width);
    thumb.left = Val::Px(left);
}

/// Drags the thumb to scroll the grid — the drag delta (screen px) is
/// scaled from track-space into content-space (`total_px / track_w`) so
/// dragging the thumb all the way across the track scrolls the full song,
/// not just `track_w` worth of it.
pub(super) fn drag_grid_scrollbar(
    ev: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    state: Res<EditorState>,
    tracks: Query<&ComputedNode, With<GridScrollTrack>>,
    mut scroll: ResMut<Scroll>,
) {
    let Ok(track) = tracks.single() else {
        return;
    };
    let track_w = track.size().x * track.inverse_scale_factor();
    let total_px = super::state::song_end_tick(&state.notes) as f32 * TICK_W;
    if track_w <= 0.0 {
        return;
    }
    let delta_px = ev.delta.x / ui_scale.0;
    scroll.px = (scroll.px + delta_px * (total_px / track_w)).max(0.0);
}

// ── Resize live-update ────────────────────────────────────────────────────────

/// Live width/position during a resize drag. Also nudges the vibrato/wah
/// material's width uniform so the wave pattern's rhythm updates as-you-drag
/// instead of only snapping correct once `rebuild_grid` runs after release.
pub(super) fn live_resize(
    state: Res<EditorState>,
    mut notes: Query<(
        &NoteView,
        &mut Node,
        Option<&MaterialNode<EditorNoteMaterial>>,
    )>,
    mut note_mats: ResMut<Assets<EditorNoteMaterial>>,
) {
    let Some(drag) = state.dragging else { return };
    if !matches!(drag.kind, DragKind::Resize(_)) {
        return;
    }
    let Some(note) = state.note_by_id(drag.id) else {
        return;
    };
    let (left, _top, width, _height) = note_rect(note);
    for (view, mut node, mat) in &mut notes {
        if view.0 == drag.id {
            node.left = Val::Px(left);
            node.width = Val::Px(width);
            if let Some(handle) = mat
                && let Some(mut m) = note_mats.get_mut(&handle.0)
            {
                m.params.y = width;
            }
        }
    }
}

pub(super) fn update_move_ghost(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut ghost: Query<
        (
            &mut Node,
            &mut Visibility,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<MoveGhost>,
    >,
) {
    let Ok((mut node, mut vis, mut bg, mut border)) = ghost.single_mut() else {
        return;
    };
    match state.dragging {
        Some(drag) if drag.kind == DragKind::Move => {
            let colors = theme.song_editor_colors();
            let left = drag.target_tick as f32 * TICK_W + 1.0;
            let top = HEADER_H + (drag.target_hole as f32 - 1.0) * ROW_H + NOTE_PAD;
            node.left = Val::Px(left);
            node.top = Val::Px(top);
            node.width = Val::Px(drag.start_len as f32 * TICK_W - 2.0);
            *vis = Visibility::Inherited;
            let color = if drag.valid {
                colors.ghost_ok
            } else {
                colors.ghost_bad
            };
            bg.0 = color.with_alpha(0.30);
            *border = BorderColor::all(color);
        }
        _ => *vis = Visibility::Hidden,
    }
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
