// SPDX-License-Identifier: MIT

use bevy::input_focus::InputFocus;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::ui_render::prelude::MaterialNode;

use super::clipboard::{NoteClipboard, copy_selected, paste_targets};
use super::grid::group_move_targets;
use super::material::EditorNoteMaterial;
use super::state::{
    Dir, DragKind, EditorState, Expr, GridNote, Pitch, Scroll, TimelineSelection, VIBRATO_HZ_MAX,
    VIBRATO_HZ_MIN, VIBRATO_HZ_STEP, WAH_HZ_MAX, WAH_HZ_MIN, WAH_HZ_STEP, enforce_direction,
    enforce_expr, max_bend, note_rect, overblow_ok, overdraw_ok, pitch_compatible,
    pitch_forced_dir,
};
use super::ui::{GridArea, GridContent, GroupMoveGhost, ModButton, MoveGhost, NoteView};
use super::{AppState, HEADER_H, NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_ui::dialogs::file_dialog::FileDialog;

// ── Note interaction ─────────────────────────────────────────────────────────

/// Whether either Ctrl key is currently held — the modifier that turns a
/// note click into a multi-selection toggle instead of an ordinary
/// select/add (see [`select_or_add_ctrl`]).
pub(super) fn ctrl_held(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight)
}

pub(super) fn select_or_add(state: &mut EditorState, hole: u8, tick: usize) {
    if let Some(existing) = state
        .notes
        .iter()
        .find(|n| n.hole == hole && n.tick <= tick && tick < n.tick + n.len)
    {
        state.select_only(existing.id);
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
    state.select_only(id);
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

/// The Ctrl+click sibling of [`select_or_add`]: toggles an existing note at
/// `hole`/`tick` in or out of the current multi-selection instead of
/// replacing it outright — this is what lets more than one note be
/// selected at once. Clicking empty space still behaves like a plain click
/// (creates and exclusively selects a new note): there's nothing existing
/// to "add" a freshly-placed note to.
pub(super) fn select_or_add_ctrl(state: &mut EditorState, hole: u8, tick: usize) {
    if let Some(existing) = state
        .notes
        .iter()
        .find(|n| n.hole == hole && n.tick <= tick && tick < n.tick + n.len)
    {
        state.toggle_selected(existing.id);
        return;
    }
    select_or_add(state, hole, tick);
}

/// Deletes every currently-selected note (see `EditorState::selected`) —
/// the Delete key/mod-panel button act on the whole multi-selection, not
/// just one note.
pub(super) fn delete_selected(state: &mut EditorState) {
    if state.selected.is_empty() {
        return;
    }
    let ids = core::mem::take(&mut state.selected);
    state.notes.retain(|n| !ids.contains(&n.id));
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
        if let Some(&id) = state.selected.last() {
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

    let Some(&id) = state.selected.last() else {
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
/// specific hole to cap it against — uses [`DEEPEST_BEND`], the richest cap
/// any hole has, so cycling here is never cut short by a hole that isn't
/// even involved yet. `select_or_add` re-validates against the real hole
/// once a note actually gets placed.
/// The deepest bend any hole allows — hole 3's three semitones, the note
/// this instrument is played for. Derived from `max_bend` rather than
/// written twice, so widening a hole's range can't leave this behind.
pub(super) const DEEPEST_BEND: f32 = 3.0;

pub(super) fn cycle_sticky_bend(state: &mut EditorState) {
    let current = match state.sticky_pitch {
        Pitch::Bend(depth) => depth,
        _ => 0.0,
    };
    let next = current + 0.5;
    state.sticky_pitch = if next > DEEPEST_BEND + f32::EPSILON {
        Pitch::Normal
    } else {
        Pitch::Bend(next)
    };
}

/// Toggles `sticky_pitch` between `Pitch::Normal` and `pitch` — the
/// hole-free sticky-only equivalent of the selected-note Overblow/
/// Overdraw/Slide toggles below (which additionally gate on the selected
/// note's own hole via `overblow_ok`/`overdraw_ok`).
pub(super) fn cycle_sticky_pitch(state: &mut EditorState, pitch: Pitch) {
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

pub(super) fn cycle_sticky_wah(state: &mut EditorState) {
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

pub(super) fn cycle_sticky_vibrato(state: &mut EditorState) {
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

/// True while one of the meta form's free-text fields (`dialogs::text_input`,
/// built on `bevy_text::EditableText`) has real keyboard focus — the gate
/// every shortcut below checks so typing into a field never steals Delete/
/// Backspace/Escape/Ctrl+C/V/Z/Y or arrow-key panning. Checking for
/// `EditableText` specifically (not just any focused entity) is what
/// excludes the five click-to-cycle fields (`Key`/`Position`/...), which are
/// plain `WidgetButton`s that can also take keyboard focus via Tab but never
/// accept typed input.
pub(super) fn a_text_field_has_focus(
    focus: &InputFocus,
    fields: &Query<(), With<EditableText>>,
) -> bool {
    focus.get().is_some_and(|e| fields.contains(e))
}

/// Escape first deselects the current note (if any); pressed again with
/// nothing selected, it leaves the editor for the menu — same "back" rule
/// every other screen follows. Suppressed while a save/load dialog is open,
/// since that dialog handles its own Escape (closes itself).
pub(super) fn grid_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
    mut sel: ResMut<TimelineSelection>,
    file_dialog: Res<FileDialog>,
    mut next_state: ResMut<NextState<AppState>>,
    mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>,
    focus: Res<InputFocus>,
    fields: Query<(), With<EditableText>>,
) {
    if a_text_field_has_focus(&focus, &fields) {
        return;
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        delete_selected(&mut state);
    }
    if keyboard.just_pressed(KeyCode::Escape) && !file_dialog.open {
        if sel.drag.is_some() || state.timeline_split.is_some() {
            sel.drag = None;
            state.timeline_split = None;
        } else if !state.selected.is_empty() {
            state.selected.clear();
        } else {
            ret_play.0 = true;
            next_state.set(AppState::Menu);
        }
    }
}

/// Ctrl+C copies every selected note into [`NoteClipboard`] verbatim
/// (nothing deleted, unlike Delete); copying with nothing selected leaves
/// a previous clipboard untouched. Ctrl+V pastes it back at the tick under
/// the mouse — read from [`GridArea`]'s own `RelativeCursorPosition` the
/// same way a grid click resolves its tick, but without requiring a click,
/// so any hover position counts. Does nothing if the pointer isn't over
/// the grid, or nothing's been copied. See [`paste_targets`] for which
/// pasted notes get silently skipped (out-of-range hole, spot already
/// occupied); the notes that land become the new selection.
pub(super) fn handle_copy_paste(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
    mut clipboard: ResMut<NoteClipboard>,
    scroll: Res<Scroll>,
    grid_area: Query<(&RelativeCursorPosition, &ComputedNode), With<GridArea>>,
    focus: Res<InputFocus>,
    fields: Query<(), With<EditableText>>,
) {
    if a_text_field_has_focus(&focus, &fields) || !ctrl_held(&keyboard) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyC) && !state.selected.is_empty() {
        clipboard.0 = copy_selected(&state.notes, &state.selected);
    }
    if keyboard.just_pressed(KeyCode::KeyV) && !clipboard.0.is_empty() {
        let Ok((rel, computed)) = grid_area.single() else {
            return;
        };
        let Some(normalized) = rel.normalized else {
            return;
        };
        let width_px = computed.size().x * computed.inverse_scale_factor();
        let frac = (normalized.x + 0.5).clamp(0.0, 1.0);
        let tick = ((scroll.px + frac * width_px) / TICK_W).round().max(0.0) as usize;
        let hole_count = state.hole_count();
        let (pasted, next_id) =
            paste_targets(&clipboard.0, tick, hole_count, &state.notes, state.next_id);
        if !pasted.is_empty() {
            state.next_id = next_id;
            state.selected = pasted.iter().map(|n| n.id).collect();
            state.notes.extend(pasted);
        }
    }
}

/// `Ctrl+Z` undoes the last content edit (note placement/move/resize/
/// delete, paste, Erase/Remove, a whole recording take, ...); `Ctrl+Y`
/// redoes it — see `undo::UndoHistory` for what counts as an edit. Same
/// text-field-focus/`ctrl_held` gating as [`handle_copy_paste`], so typing
/// into a meta-form text field never steals these keys.
pub(super) fn handle_undo_redo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
    mut history: ResMut<super::undo::UndoHistory>,
    focus: Res<InputFocus>,
    fields: Query<(), With<EditableText>>,
) {
    if a_text_field_has_focus(&focus, &fields) || !ctrl_held(&keyboard) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        history.undo(&mut state);
    } else if keyboard.just_pressed(KeyCode::KeyY) {
        history.redo(&mut state);
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
    let Some(drag) = state.dragging.as_ref() else {
        return;
    };
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
    match &state.dragging {
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

/// The multi-select sibling of [`update_move_ghost`]: one preview rectangle
/// per *other* note in a group move (`DragState::group`), positioned by
/// shifting each member's own original hole/tick by the exact delta the
/// anchor moved by ([`group_move_targets`]) — the anchor's own preview is
/// still [`MoveGhost`]. Rebuilt from scratch every frame, like
/// `update_scrollbar_markers`, since there's no group to show most of the
/// time (an ordinary single-note drag leaves `group` empty and this is a
/// no-op after clearing any leftover ghosts from a previous drag).
pub(super) fn update_group_move_ghosts(
    mut commands: Commands,
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    content: Query<Entity, With<GridContent>>,
    old: Query<Entity, With<GroupMoveGhost>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(drag) = state.dragging.as_ref() else {
        return;
    };
    if drag.kind != DragKind::Move || drag.group.is_empty() {
        return;
    }
    let Ok(content) = content.single() else {
        return;
    };
    let colors = theme.song_editor_colors();
    let color = if drag.valid {
        colors.ghost_ok
    } else {
        colors.ghost_bad
    };
    let hole_delta = drag.target_hole as i32 - drag.start_hole as i32;
    let tick_delta = drag.target_tick as i32 - drag.start_tick as i32;
    let hole_count = state.hole_count();
    let targets = group_move_targets(&drag.group, hole_delta, tick_delta, hole_count);
    let new: Vec<Entity> = targets
        .iter()
        .map(|&(_, hole, tick, len, _)| {
            let left = tick as f32 * TICK_W + 1.0;
            let top = HEADER_H + (hole as f32 - 1.0) * ROW_H + NOTE_PAD;
            let width = len as f32 * TICK_W - 2.0;
            commands
                .spawn((
                    GroupMoveGhost,
                    ZIndex(2),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(left),
                        top: Val::Px(top),
                        width: Val::Px(width),
                        height: Val::Px(ROW_H - 2.0 * NOTE_PAD),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(color.with_alpha(0.30)),
                    BorderColor::all(color),
                    Pickable::IGNORE,
                ))
                .id()
        })
        .collect();
    commands.entity(content).add_children(&new);
}
