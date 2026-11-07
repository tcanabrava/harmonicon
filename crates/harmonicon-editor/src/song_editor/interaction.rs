// SPDX-License-Identifier: MIT

use bevy::input_focus::InputFocus;
use bevy::picking::Pickable;
use bevy::picking::events::{PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::ui_render::prelude::MaterialNode;

use super::clipboard::NoteClipboard;
use super::grid::group_move_targets;
use super::material::EditorNoteMaterial;
use super::snap::snap_absolute_tick;
use super::state::{
    DEFAULT_INTENSITY, Dir, DragKind, DragState, Edge, EditorState, Expr, GridNote, HarpTechnique,
    Scroll, TimelineSelection, VIBRATO_HZ_MAX, VIBRATO_HZ_MIN, VIBRATO_HZ_STEP, WAH_HZ_MAX,
    WAH_HZ_MIN, WAH_HZ_STEP, apply_resize, enforce_direction, enforce_expr, forced_dir, max_bend,
    note_rect, overblow_ok, overdraw_ok, pitch_compatible, pitch_forced_dir,
};
use super::ui::{
    GridArea, GridContent, GroupMoveGhost, ModButton, MoveGhost, NoteView, ResizeGrip,
};
use super::{AppState, GRIP_D, HEADER_H, TICK_W, TICKS_PER_BEAT, note_pad, row_h};
use harmonicon_core::harmonica::Harmonica;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::{LoadedTheme, SongEditorColors};
use harmonicon_ui::dialogs::file_dialog::FileDialog;

// ── Note interaction ─────────────────────────────────────────────────────────

/// Whether either Ctrl key is currently held — the modifier that turns a
/// note click into a multi-selection toggle instead of an ordinary
/// select/add (see [`select_or_add_ctrl`]).
pub(super) fn ctrl_held(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight)
}

pub(super) fn shift_held(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
}

pub(super) fn select_or_add(state: &mut EditorState, hole: u8, tick: usize) {
    // A flute's fingering view draws keys, not pitches, so an empty row
    // there names no note to place (`state::FluteView`).
    if state.flute_fingering_view().is_some() {
        return;
    }
    if let Some(existing) =
        state.notes.iter().find(|n| n.row() == hole && n.tick <= tick && tick < n.tick + n.len)
    {
        state.select_only(existing.id);
        return;
    }

    let next_start =
        state.notes.iter().filter(|n| n.row() == hole && n.tick > tick).map(|n| n.tick).min();

    let len = next_start.map_or(TICKS_PER_BEAT, |start| (start - tick).min(TICKS_PER_BEAT)).max(1);

    if let Some(place) =
        super::instrument_adapter::for_instrument(&state.effective_instrument()).place
    {
        place(state, hole, tick, len);
        return;
    }

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
    let pitch = if pitch_compatible(state.sticky_pitch, &state.effective_harp(), hole) {
        state.sticky_pitch
    } else {
        HarpTechnique::Normal
    };
    // Overblow/Overdraw physically require a specific breath direction
    // (see `pitch_forced_dir`) — that always wins, even over whatever's
    // already sounding at this tick, since a mismatched pairing (e.g.
    // "overblow" on a note tagged Draw) can't exist for real.
    let forced = forced_dir(pitch, hole, &state.effective_harp());
    if let Some(forced) = forced {
        dir = forced;
    }
    let expr = state.sticky_expr;

    let id = state.next_id;
    state.next_id += 1;
    state.notes.push(GridNote::new(
        id,
        tick,
        len,
        expr,
        GridNote::harmonica_target(hole, dir, pitch),
    ));
    state.select_only(id);
    // A chord note whose direction was forced (above), or that's carrying
    // an armed sticky expr, must pull any simultaneous notes on other
    // holes into agreement too — direction and wah/vibrato are both
    // whole-player techniques, not per-hole.
    if forced.is_some() {
        enforce_direction(state, id);
    }
    if expr != Expr::None {
        enforce_expr(state, id);
        // The armed depth, like the armed rate: only meaningful with an
        // expression to have a depth of, and the default is never stored.
        if state.sticky_intensity != DEFAULT_INTENSITY {
            let depth = state.sticky_intensity.clone();
            state.expression_intensities.insert(id, depth);
        }
    }
}

/// The Ctrl+click sibling of [`select_or_add`]: toggles an existing note at
/// `hole`/`tick` in or out of the current multi-selection instead of
/// replacing it outright — this is what lets more than one note be
/// selected at once. Clicking empty space still behaves like a plain click
/// (creates and exclusively selects a new note): there's nothing existing
/// to "add" a freshly-placed note to.
pub(super) fn select_or_add_ctrl(state: &mut EditorState, hole: u8, tick: usize) {
    if let Some(existing) =
        state.notes.iter().find(|n| n.row() == hole && n.tick <= tick && tick < n.tick + n.len)
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
    state.prune_selection();
}

pub(super) fn apply_modifier(state: &mut EditorState, kind: ModButton) {
    match kind {
        ModButton::Delete => {
            delete_selected(state);
            return;
        }
        ModButton::Depth => {
            state.cycle_depth();
            return;
        }
        // Phrase properties, reached through the selected note's onset.
        // No sticky meaning: a phrase is a set of placed notes.
        ModButton::Call => {
            let on = state.selected_call();
            state.set_selected_call(!on);
            return;
        }
        ModButton::Split => {
            let on = state.selected_split();
            state.set_selected_split(!on);
            return;
        }
        ModButton::Phrase => {
            if let Some(tick) = state.selected_note().map(|n| n.tick) {
                state.open_phrase_editor(tick);
            }
            return;
        }
        ModButton::TransposeUp => {
            super::transpose::transpose_selection(state, 1);
            return;
        }
        ModButton::TransposeDown => {
            super::transpose::transpose_selection(state, -1);
            return;
        }
        _ => {}
    }
    if let Some(modify) =
        super::instrument_adapter::for_instrument(&state.effective_instrument()).modify
    {
        modify(state, kind);
        return;
    }
    if matches!(kind, ModButton::Blow | ModButton::Draw) {
        let dir = if kind == ModButton::Blow { Dir::Blow } else { Dir::Draw };
        // Arms the sticky direction regardless of whether anything is
        // selected — a note to edit is optional, arming for future notes
        // isn't. An armed Overblow/Overdraw that no longer matches this
        // direction can't survive the switch (see `pitch_forced_dir`) —
        // clear it rather than leave e.g. "overblow" armed alongside Draw.
        state.sticky_dir = dir;
        if pitch_forced_dir(state.sticky_pitch).is_some_and(|d| d != dir) {
            state.sticky_pitch = HarpTechnique::Normal;
        }
        let harp = state.effective_harp();
        for id in state.selected.clone() {
            if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                n.set_dir(dir);
                // A bend on 1–6 is a draw: blowing it drops the bend.
                if forced_dir(n.technique(), n.hole(), &harp).is_some_and(|d| d != dir) {
                    n.set_technique(HarpTechnique::Normal);
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
        apply_sticky_modifier(state, kind);
        return;
    };

    let harp = state.effective_harp();
    let Some(anchor) = state.note_by_id(id).copied() else {
        return;
    };
    let ids = state.selected.clone();
    let selected: Vec<GridNote> =
        ids.iter().filter_map(|&i| state.note_by_id(i).copied()).collect();

    // The primary note decides where the button's cycle goes next, exactly
    // as with one note selected; every selected note then takes that value
    // if it fits its hole, and is counted as skipped if it doesn't.
    let mut skipped = 0;
    let mut breath_moved = false;
    match kind {
        ModButton::Wah | ModButton::Vibrato => {
            let target = next_expr(kind, anchor.expr);
            for n in state.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
                n.expr = target;
            }
            state.sticky_expr = target;
            for &i in &ids {
                enforce_expr(state, i);
            }
        }
        _ => {
            let Some(target) = next_pitch(kind, &anchor, &selected, &harp) else {
                return;
            };
            for n in state.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
                if target == HarpTechnique::Normal {
                    // Switching a technique off clears it where it is,
                    // and leaves every other note's technique alone.
                    if same_technique(kind, n.technique()) {
                        n.set_technique(HarpTechnique::Normal);
                    }
                } else if pitch_fits(target, n.hole(), &harp) {
                    n.set_technique(target);
                    if let Some(dir) = forced_dir(target, n.hole(), &harp) {
                        breath_moved |= n.dir() != dir;
                        n.set_dir(dir);
                    }
                } else {
                    skipped += 1;
                }
            }
            state.sticky_pitch = target;
            // Overblow/Overdraw force a direction — mirror it into the
            // sticky direction. A bend forces one only on its hole, so it
            // leaves the sticky alone. Either way, pull simultaneous notes
            // on other holes into agreement (direction is whole-player,
            // not per-hole).
            if let Some(dir) = pitch_forced_dir(target) {
                state.sticky_dir = dir;
                breath_moved = true;
            }
            if breath_moved {
                for &i in &ids {
                    enforce_direction(state, i);
                }
            }
        }
    }
    // One note that can't take a technique is the button doing nothing, as
    // it always has; in a selection it's worth saying which didn't change.
    if skipped > 0 && ids.len() > 1 {
        state.technique_notice = Some(skipped);
    }
}

/// Arm a technique or expression for notes placed after an unselected edit.
pub(super) fn apply_sticky_modifier(state: &mut EditorState, kind: ModButton) {
    match kind {
        ModButton::Bend => cycle_sticky_bend(state),
        ModButton::Overblow => cycle_sticky_pitch(state, HarpTechnique::Overblow),
        ModButton::Overdraw => cycle_sticky_pitch(state, HarpTechnique::Overdraw),
        ModButton::Slide => cycle_sticky_pitch(state, HarpTechnique::Slide),
        ModButton::Wah | ModButton::Vibrato => {
            state.sticky_expr = next_expr(kind, state.sticky_expr);
        }
        _ => {}
    }
}

/// Where the Wah or Vibrato button's cycle goes from `current`: the next
/// rate step, or off past the fastest.
pub(super) fn next_expr(kind: ModButton, current: Expr) -> Expr {
    match kind {
        ModButton::Wah => {
            let next = match current {
                Expr::Wah(hz) => hz + WAH_HZ_STEP,
                _ => WAH_HZ_MIN,
            };
            if next > WAH_HZ_MAX + f32::EPSILON { Expr::None } else { Expr::Wah(next) }
        }
        _ => {
            let next = match current {
                Expr::Vibrato(hz) => hz + VIBRATO_HZ_STEP,
                _ => VIBRATO_HZ_MIN,
            };
            if next > VIBRATO_HZ_MAX + f32::EPSILON { Expr::None } else { Expr::Vibrato(next) }
        }
    }
}

/// Where a pitch-technique button's cycle goes from `anchor`, the primary
/// note. A bend steps half a semitone deeper, up to the deepest any
/// selected hole allows, then off; the others toggle. `None` when no
/// selected note can bend at all — the button does nothing.
pub(super) fn next_pitch(
    kind: ModButton,
    anchor: &GridNote,
    selected: &[GridNote],
    harp: &Harmonica,
) -> Option<HarpTechnique> {
    let toggle = |pitch: HarpTechnique| {
        if anchor.technique() == pitch { HarpTechnique::Normal } else { pitch }
    };
    Some(match kind {
        ModButton::Bend => {
            let cap = selected.iter().map(|n| max_bend(harp, n.hole())).fold(0.0, f32::max);
            if cap <= 0.0 {
                return None;
            }
            let next = anchor.bend() + 0.5;
            if next > cap + f32::EPSILON {
                HarpTechnique::Normal
            } else {
                HarpTechnique::Bend(next)
            }
        }
        ModButton::Overblow => toggle(HarpTechnique::Overblow),
        ModButton::Overdraw => toggle(HarpTechnique::Overdraw),
        ModButton::Slide => toggle(HarpTechnique::Slide),
        _ => return None,
    })
}

/// Whether `pitch` is the technique `kind` switches — what switching it off
/// clears.
fn same_technique(kind: ModButton, pitch: HarpTechnique) -> bool {
    match kind {
        ModButton::Bend => matches!(pitch, HarpTechnique::Bend(_)),
        ModButton::Overblow => pitch == HarpTechnique::Overblow,
        ModButton::Overdraw => pitch == HarpTechnique::Overdraw,
        ModButton::Slide => pitch == HarpTechnique::Slide,
        _ => false,
    }
}

/// Whether a note on `hole` can play `pitch`.
pub(super) fn pitch_fits(pitch: HarpTechnique, hole: u8, harp: &Harmonica) -> bool {
    match pitch {
        HarpTechnique::Bend(depth) => depth <= max_bend(harp, hole) + f32::EPSILON,
        HarpTechnique::Overblow => overblow_ok(hole),
        HarpTechnique::Overdraw => overdraw_ok(hole),
        HarpTechnique::Slide | HarpTechnique::Normal => true,
    }
}

/// Puts a pending "some selected notes were skipped" count in the status
/// bar — the technique buttons' sibling of `transpose::report_transpose`.
pub(super) fn report_technique_skips(
    mut state: ResMut<EditorState>,
    loc: Res<Localization>,
    mut feedback: ResMut<super::save_feedback::SaveFeedback>,
) {
    if state.unplayable_notice {
        state.bypass_change_detection().unplayable_notice = false;
        feedback.set(
            loc.msg(
                super::instrument_adapter::for_instrument(&state.effective_instrument())
                    .invalid_message,
            ),
        );
    }
    if state.technique_notice.is_none() {
        return;
    }
    let Some(count) = state.bypass_change_detection().technique_notice.take() else {
        return;
    };
    feedback.set(loc.msg_args("editor-technique-skipped", &[("count", count.to_string())]));
}

/// The deepest bend any hole of `harp` allows — the cap for cycling a
/// bend with no hole to check against. Asked of the harp rather than kept
/// as a constant, since it's a property of the tuning: three semitones on
/// Richter (hole 3, the note this instrument is played for), the same on
/// natural minor but on different holes, and whatever a custom layout's
/// widest reed pair gives.
pub(super) fn deepest_bend(harp: &Harmonica) -> f32 {
    (1..=harp.hole_count()).map(|hole| max_bend(harp, hole)).fold(0.0, f32::max)
}

/// Cycles `sticky_pitch`'s bend depth with nothing selected, so there's no
/// specific hole to cap it against — uses [`deepest_bend`], the richest cap
/// any hole has, so cycling here is never cut short by a hole that isn't
/// even involved yet. `select_or_add` re-validates against the real hole
/// once a note actually gets placed.
pub(super) fn cycle_sticky_bend(state: &mut EditorState) {
    let current = match state.sticky_pitch {
        HarpTechnique::Bend(depth) => depth,
        _ => 0.0,
    };
    let next = current + 0.5;
    state.sticky_pitch = if next > deepest_bend(&state.effective_harp()) + f32::EPSILON {
        HarpTechnique::Normal
    } else {
        HarpTechnique::Bend(next)
    };
}

/// Toggles `sticky_pitch` between `HarpTechnique::Normal` and `pitch` — the
/// hole-free sticky-only equivalent of the selected-note Overblow/
/// Overdraw/Slide toggles below (which additionally gate on the selected
/// note's own hole via `overblow_ok`/`overdraw_ok`).
pub(super) fn cycle_sticky_pitch(state: &mut EditorState, pitch: HarpTechnique) {
    state.sticky_pitch = if state.sticky_pitch == pitch { HarpTechnique::Normal } else { pitch };
    // Arming Overblow/Overdraw with nothing selected must arm the
    // direction it requires too — otherwise a subsequently placed note
    // could still end up with e.g. `sticky_pitch: Overblow` alongside a
    // stale `sticky_dir: Draw` from something clicked earlier.
    if let Some(dir) = pitch_forced_dir(state.sticky_pitch) {
        state.sticky_dir = dir;
    }
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
    if file_dialog.open || a_text_field_has_focus(&focus, &fields) {
        return;
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        delete_selected(&mut state);
    }
    // Ctrl+↑/↓ transposes the selection (or everything) a semitone;
    // with Shift, an octave.
    if ctrl_held(&keyboard) {
        let step = if shift_held(&keyboard) { 12 } else { 1 };
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            super::transpose::transpose_selection(&mut state, step);
        } else if keyboard.just_pressed(KeyCode::ArrowDown) {
            super::transpose::transpose_selection(&mut state, -step);
        }
    } else if super::piano_note::is_piano(&state) {
        // Plain ↑/↓ scroll a piano's window up and down the keyboard, a
        // key at a time, or an octave with Shift.
        let step = if shift_held(&keyboard) { 12 } else { 1 };
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            super::piano_note::scroll_keys(&mut state, -step);
        } else if keyboard.just_pressed(KeyCode::ArrowDown) {
            super::piano_note::scroll_keys(&mut state, step);
        }
    }
    if keyboard.just_pressed(KeyCode::Escape) && !file_dialog.open {
        if state.phrase_editor.is_some() {
            state.phrase_editor = None;
        } else if sel.drag.is_some() || state.timeline_split.is_some() {
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
    file_dialog: Res<FileDialog>,
    mut state: ResMut<EditorState>,
    mut clipboard: ResMut<NoteClipboard>,
    scroll: Res<Scroll>,
    grid_area: Query<(&RelativeCursorPosition, &ComputedNode), With<GridArea>>,
    focus: Res<InputFocus>,
    fields: Query<(), With<EditableText>>,
) {
    if file_dialog.open || a_text_field_has_focus(&focus, &fields) || !ctrl_held(&keyboard) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyC) && !state.selected.is_empty() {
        *clipboard = state.copy_selection();
    }
    if keyboard.just_pressed(KeyCode::KeyV) && !clipboard.is_empty() {
        let Ok((rel, computed)) = grid_area.single() else {
            return;
        };
        let Some(normalized) = rel.normalized else {
            return;
        };
        let width_px = computed.size().x * computed.inverse_scale_factor();
        let frac = (normalized.x + 0.5).clamp(0.0, 1.0);
        let tick = ((scroll.px + frac * width_px) / TICK_W).round().max(0.0) as usize;
        state.paste(&clipboard, tick);
    }
}

/// `Ctrl+Z` undoes the last content edit (note placement/move/resize/
/// delete, paste, Erase/Remove, a whole recording take, ...); `Ctrl+Y`
/// redoes it — see `undo::UndoHistory` for what counts as an edit. Same
/// text-field-focus/`ctrl_held` gating as [`handle_copy_paste`], so typing
/// into a meta-form text field never steals these keys.
pub(super) fn handle_undo_redo(
    keyboard: Res<ButtonInput<KeyCode>>,
    file_dialog: Res<FileDialog>,
    mut commands: Commands,
    focus: Res<InputFocus>,
    fields: Query<(), With<EditableText>>,
) {
    if file_dialog.open || a_text_field_has_focus(&focus, &fields) || !ctrl_held(&keyboard) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        commands.write_message(super::state::DocumentCommand::Undo);
    } else if keyboard.just_pressed(KeyCode::KeyY) {
        commands.write_message(super::state::DocumentCommand::Redo);
    }
}

// ── Resize grips ──────────────────────────────────────────────────────────────

/// Where the selected note's `edge` grip sits, as `(left, top)` in
/// `GridContent`'s coordinate space — see [`ResizeGrip`] for why the grips
/// live outside the note instead of inside it.
///
/// The left grip is clamped to the content origin so a note at tick 0 keeps
/// a reachable one: `GridArea` clips its overflow, and the grid can't scroll
/// left of 0, so an unclamped grip there would be permanently off-screen.
/// That one note trades part of its body for the grip; every other note
/// keeps all of it.
pub(super) fn resize_grip_position(
    note: &GridNote,
    edge: Edge,
    rows: u8,
    row: usize,
) -> (f32, f32) {
    let (left, top, width, height) = note_rect(note, row, rows);
    let x = match edge {
        Edge::Left => (left - GRIP_D).max(0.0),
        Edge::Right => left + width,
    };
    (x, top + (height - GRIP_D) / 2.0)
}

/// Spawns the two persistent grips into `GridContent`. Called once from
/// `ui::setup`, alongside the other persistent overlay entities.
pub(super) fn spawn_resize_grips(
    content: &mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>,
    colors: SongEditorColors,
) {
    for edge in [Edge::Left, Edge::Right] {
        content
            .spawn((
                ResizeGrip(edge),
                // Above the notes (`ZIndex(1)`) so a grip overlapping a
                // neighbouring note is still what the pointer hits.
                ZIndex(4),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(GRIP_D),
                    height: Val::Px(GRIP_D),
                    border: UiRect::all(Val::Px(2.0)),
                    // Half of a square node's side is a circle.
                    border_radius: BorderRadius::all(Val::Px(GRIP_D / 2.0)),
                    ..default()
                },
                BackgroundColor(colors.accent),
                BorderColor::all(Color::BLACK),
                Visibility::Hidden,
            ))
            // Not `On<Activate>` on a real widget: this is a drag surface,
            // not a button — it has no click behaviour to give a keyboard
            // user, and `bevy_ui_widgets::Button` would only add one.
            // not-a-widget-button: resize grip, drag-only
            .observe(move |_: On<PointerDragStart>, mut state: ResMut<EditorState>| {
                if state.dragging.is_some() || state.locked() {
                    return;
                }
                let Some(note) = state.selected_note().copied() else {
                    return;
                };
                state.dragging = Some(DragState::new(note.id, DragKind::Resize(edge), &note));
            })
            .observe(
                move |ev: On<PointerDrag>,
                      mut state: ResMut<EditorState>,
                      ui_scale: Res<UiScale>| {
                    let Some(drag) = state.dragging.as_ref() else {
                        return;
                    };
                    if drag.kind != DragKind::Resize(edge) {
                        return;
                    }
                    let id = drag.id;
                    let hole = drag.start_hole;
                    let mut left_bound = 0usize;
                    let mut right_bound: Option<usize> = None;
                    for n in &state.notes {
                        if n.id == id || n.row() != hole {
                            continue;
                        }
                        if n.tick < drag.start_tick {
                            left_bound = left_bound.max(n.tick + n.len);
                        } else {
                            right_bound = Some(right_bound.map_or(n.tick, |r| r.min(n.tick)));
                        }
                    }
                    // `ev.distance` is raw window pixels but `TICK_W` is a
                    // logical size `UiScale` multiplies up — same correction
                    // the move drag applies.
                    let steps = ((ev.distance.x / ui_scale.0) / TICK_W).round() as i32;
                    let (tick, len) = apply_resize(
                        drag.start_tick,
                        drag.start_len,
                        edge,
                        steps,
                        left_bound,
                        right_bound,
                    );
                    // Snap whichever edge moved, then re-clamp to the bounds
                    // `apply_resize` already enforced — snapping can push a
                    // value back out of them.
                    let mode = state.snap_mode;
                    let (tick, len) = match edge {
                        Edge::Right => {
                            let mut end = snap_absolute_tick(tick + len, mode).max(tick + 1);
                            if let Some(rb) = right_bound {
                                end = end.min(rb);
                            }
                            (tick, end - tick)
                        }
                        Edge::Left => {
                            let end = tick + len;
                            let start = snap_absolute_tick(tick, mode).min(end - 1).max(left_bound);
                            (start, end - start)
                        }
                    };
                    // A guitar note also stops where the next step would
                    // hold a string another note needs, or outgrow the hand.
                    let instrument = state.effective_instrument();
                    if !state.plays_harmonica()
                        && let Some(current) = state.note_by_id(id).copied()
                        && !super::note_edit::fits(
                            &instrument,
                            &state.notes,
                            &GridNote { tick, len, ..current },
                            &[],
                        )
                    {
                        return;
                    }
                    if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                        n.tick = tick;
                        n.len = len;
                    }
                },
            )
            .observe(move |_: On<PointerDragEnd>, mut state: ResMut<EditorState>| {
                let Some(drag) = state.dragging.as_ref() else {
                    return;
                };
                if drag.kind != DragKind::Resize(edge) {
                    return;
                }
                let id = drag.id;
                state.dragging = None;
                enforce_direction(&mut state, id);
                enforce_expr(&mut state, id);
            });
    }
}

/// Moves the two grips onto the selected note every frame, and hides them
/// when there is nothing to resize.
///
/// Shown only for a selection of *exactly one* note: with several selected a
/// drag moves the whole group as a rigid shape (`DragState::group`), so
/// "which note's edge is this" has no answer. Hidden while the grid is
/// locked (Perform/Record, or the user's Lock toggle) for the same reason
/// `rebuild_grid` spawns notes `Pickable::IGNORE` there.
pub(super) fn update_resize_grips(
    state: Res<EditorState>,
    mut grips: Query<(&ResizeGrip, &mut Node, &mut Visibility)>,
) {
    // A piano's note scrolled out of its window has nothing to grip.
    let target = (state.selected.len() == 1 && !state.locked())
        .then(|| state.selected_note().copied())
        .flatten()
        .filter(|note| state.row_visible(note.row()));
    for (grip, mut node, mut vis) in &mut grips {
        let Some(note) = target else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let (left, top) =
            resize_grip_position(&note, grip.0, state.display_rows(), state.display_row(&note));
        if node.left != Val::Px(left) || node.top != Val::Px(top) {
            node.left = Val::Px(left);
            node.top = Val::Px(top);
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}

// ── Resize live-update ────────────────────────────────────────────────────────

/// Live width/position during a resize drag. Also nudges the vibrato/wah
/// material's width uniform so the wave pattern's rhythm updates as-you-drag
/// instead of only snapping correct once `rebuild_grid` runs after release.
pub(super) fn live_resize(
    state: Res<EditorState>,
    mut notes: Query<(&NoteView, &mut Node, Option<&MaterialNode<EditorNoteMaterial>>)>,
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
    let (left, _top, width, _height) =
        note_rect(note, state.display_row(note), state.display_rows());
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
        (&mut Node, &mut Visibility, &mut BackgroundColor, &mut BorderColor),
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
            let (top, height) = ghost_span(&state, drag.target_hole);
            node.left = Val::Px(left);
            node.top = Val::Px(top);
            node.height = Val::Px(height);
            node.width = Val::Px(drag.start_len as f32 * TICK_W - 2.0);
            *vis = Visibility::Inherited;
            let color = if drag.valid { colors.ghost_ok } else { colors.ghost_bad };
            bg.0 = color.with_alpha(0.30);
            *border = BorderColor::all(color);
        }
        _ => *vis = Visibility::Hidden,
    }
}

/// Where a move preview landing on `row` sits, `(top, height)`: its row,
/// or every row in a flute's fingering view, where a note is a column.
fn ghost_span(state: &EditorState, row: u8) -> (f32, f32) {
    let rows = state.display_rows();
    let (row_h, pad) = (row_h(rows), note_pad(rows));
    if state.flute_fingering_view().is_some() {
        (HEADER_H + pad, f32::from(rows) * row_h - 2.0 * pad)
    } else {
        // A note row, drawn where a piano's window puts it.
        let drawn = f32::from(row) - 1.0 - f32::from(state.row_offset);
        (HEADER_H + drawn * row_h + pad, row_h - 2.0 * pad)
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
    let color = if drag.valid { colors.ghost_ok } else { colors.ghost_bad };
    let hole_delta = drag.target_hole as i32 - drag.start_hole as i32;
    let tick_delta = drag.target_tick as i32 - drag.start_tick as i32;
    let hole_count = state.row_count();
    let targets = group_move_targets(&drag.group, hole_delta, tick_delta, hole_count);
    let new: Vec<Entity> = targets
        .iter()
        .map(|&(_, hole, tick, len, _)| {
            let left = tick as f32 * TICK_W + 1.0;
            let (top, height) = ghost_span(&state, hole);
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
                        height: Val::Px(height),
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
