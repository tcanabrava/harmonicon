// SPDX-License-Identifier: MIT

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use super::material::EditorNoteMaterial;
use super::playback::Playhead;
use super::state::{
    Dir, DragKind, EditorState, Expr, GridNote, Pitch, Scroll, VIBRATO_HZ_MAX, VIBRATO_HZ_MIN,
    VIBRATO_HZ_STEP, WAH_HZ_MAX, WAH_HZ_MIN, WAH_HZ_STEP, enforce_direction, enforce_expr,
    max_bend, note_rect, overblow_ok, overdraw_ok,
};
use super::ui::{GridContent, ModButton, MoveGhost, NoteView};
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
    let dir = state.dir_at(tick).unwrap_or(Dir::Blow);
    let id = state.next_id;
    state.next_id += 1;
    state.notes.push(GridNote {
        id,
        hole,
        tick,
        len,
        dir,
        pitch: Pitch::Normal,
        expr: Expr::None,
    });
    state.selected = Some(id);
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
    let Some(id) = state.selected else { return };

    if matches!(kind, ModButton::Blow | ModButton::Draw) {
        let dir = if kind == ModButton::Blow {
            Dir::Blow
        } else {
            Dir::Draw
        };
        if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
            n.dir = dir;
        }
        enforce_direction(state, id);
        return;
    }

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
            }
        }
        ModButton::Overdraw => {
            if overdraw_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overdraw {
                    Pitch::Normal
                } else {
                    Pitch::Overdraw
                };
            }
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
    if matches!(kind, ModButton::Wah | ModButton::Vibrato) {
        enforce_expr(state, id);
    }
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
) {
    if state.focus.is_some() {
        return;
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        delete_selected(&mut state);
    }
    if keyboard.just_pressed(KeyCode::Escape) && !file_dialog.open {
        if state.selected.is_some() {
            state.selected = None;
        } else {
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
