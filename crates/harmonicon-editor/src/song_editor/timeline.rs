// SPDX-License-Identifier: MIT

//! The timeline ruler's Select, Erase, Remove, Tempo, Meter, Repeat,
//! Ending and Record seek interactions. With Select active
//! (`EditorState::timeline_tool`), the header strip above the note grid
//! builds a range selection ([`TimelineSelection`]) two ways:
//!
//! - **Click, hover, click**: a plain click drops a split point
//!   (`EditorState::timeline_split`); hovering left or right of it previews
//!   that whole side (song start..split, or split..song end); clicking
//!   again on the highlighted side selects it.
//! - **Click, drag, release**: picks an explicit span instead, previewed
//!   live as it's dragged (and extendable mid-drag by wheel-scrolling the
//!   grid — see `sync_selection_with_scroll` and `drag_end_tick`'s
//!   `scroll_delta_px`), kept as the selection on release.
//!
//! The selection itself is non-destructive; the Erase/Remove buttons act on
//! it (`panel_widgets::timeline_tool_button`), each opening the confirm
//! dialog via [`request_confirm`].
//!
//! Both paths are driven entirely by `PointerDragStart`/`Drag`/`DragEnd`,
//! deliberately not `PointerClick` even for the "plain click" case:
//! `bevy_picking` fires `DragStart` on any nonzero pixel motion while
//! pressed, so mouse jitter during an intended click routinely produces a
//! same-tick drag anyway, and `Click` fires *alongside* `DragEnd` on the
//! same release (`Click` first) whenever the pointer is still over the
//! surface. Routing every decision through the one `Drag*` chain avoids
//! that race instead of coordinating two competing handlers;
//! [`on_timeline_drag_end`] tells a real drag apart from a same-tick click
//! by whether the span actually moved.
//!
//! Either way nothing is deleted until the confirm dialog
//! (`dialogs::confirm_dialog`) comes back `confirmed: true` — see
//! [`handle_timeline_confirm`]. The actual note-list surgery is the pure
//! `ranges::erase_range`/`ranges::remove_range` pair; this module is just
//! the interaction/UI wiring around them.

use bevy::picking::events::{PointerClick, PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::Activate;

use super::playback::{Playhead, secs_per_tick};
use super::ranges::{normalize_range, split_side_range};
use super::record::RecordState;
use super::save_feedback::SaveFeedback;
use super::state::{
    EditorState, Mode, Scroll, Side, TimelineDrag, TimelineSelection, TimelineTool,
    toggle_tempo_point,
};
use super::{BEAT_W, TICK_W};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::confirm_dialog::{ConfirmChosen, DialogId, OpenConfirmDialog};
use harmonicon_ui::music_score::MeterMap;

pub(super) const TIMELINE_CONFIRM_PURPOSE: DialogId = DialogId("song_editor_2_timeline_confirm");

// ── Components ────────────────────────────────────────────────────────────────

/// The invisible, header-strip-sized click/drag catcher. Spawned *once* in
/// `ui::setup` (like `MoveGhost`/`PlayheadLine`) rather than respawned by
/// `grid::rebuild_grid`: a rebuild mid-gesture would despawn the entity
/// `bevy_picking` has captured the drag on, killing `Drag`/`DragEnd`
/// delivery — and rebuilds *do* happen mid-gesture, since a wheel pan
/// during a Select drag must spawn the notes it scrolls into view.
/// [`sync_timeline_surface`] keeps it glued to the visible viewport.
#[derive(Component)]
pub(super) struct TimelineSurface;

/// The pixel geometry [`TimelineSurface`] currently covers, needed to
/// convert its own `RelativeCursorPosition` into an absolute tick. Kept in
/// lockstep with [`Scroll`] and the window width by
/// [`sync_timeline_surface`], the same numbers its `Node` position/size are
/// computed from.
#[derive(Component, Clone, Copy)]
pub(super) struct TimelineSurfaceGeometry {
    pub(super) scroll_px: f32,
    pub(super) width_px: f32,
}

impl TimelineSurfaceGeometry {
    /// `normalized_x` is a `RelativeCursorPosition::normalized.x` reading —
    /// **-0.5..0.5** across the surface's own width (its own doc comment;
    /// confirmed against the working pattern in `gameplay::
    /// song_progress_overlay::cursor_to_time`), *not* 0..1. Skipping the
    /// `+ 0.5` re-centering step collapses every click left of the
    /// surface's center down to its leftmost tick.
    pub(super) fn tick_at(&self, normalized_x: f32) -> usize {
        let frac = (normalized_x + 0.5).clamp(0.0, 1.0);
        let abs_px = self.scroll_px + frac * self.width_px;
        (abs_px / TICK_W).round().max(0.0) as usize
    }
}

/// Keeps the persistent [`TimelineSurface`] covering exactly the visible
/// slice of the header strip: its parent (`GridContent`) is translated left
/// by [`Scroll::px`], so `left = scroll.px` pins it to the viewport origin,
/// and its width tracks the window's visible beat span (same formula
/// `grid::rebuild_grid` windows its columns with). Skips the writes when
/// nothing changed so it doesn't dirty UI layout every frame.
pub(super) fn sync_timeline_surface(
    scroll: Res<Scroll>,
    windows: Query<&Window>,
    mut surfaces: Query<(&mut Node, &mut TimelineSurfaceGeometry), With<TimelineSurface>>,
) {
    let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);
    let width_px = (super::grid::visible_beats(win_w) + 1) as f32 * BEAT_W;
    for (mut node, mut geom) in &mut surfaces {
        if geom.scroll_px != scroll.px || geom.width_px != width_px {
            *geom = TimelineSurfaceGeometry { scroll_px: scroll.px, width_px };
            node.left = Val::Px(scroll.px);
            node.width = Val::Px(width_px);
        }
    }
}

// ── Pure display helper ──────────────────────────────────────────────────────

/// A tick as "bar.beat" (1-indexed), matching the numbers already shown on
/// the ruler — used in the confirm dialog's message and the phrase
/// editor's title. Asks the meter *map*, as the ruler does, so the two
/// agree after a mid-song meter change as well as in a meter whose bar
/// isn't a whole number of quarter-note columns. A pickup's unnumbered bar
/// reads as bar 0, the name musicians use for it.
pub(super) fn describe_tick(tick: usize, map: &MeterMap) -> String {
    let pos = map.position(tick as u64);
    let bar = map.bar_number(pos.bar).unwrap_or(0);
    format!("{bar}.{}", pos.beat + 1)
}

pub(super) fn request_confirm(
    state: &mut EditorState,
    loc: &Localization,
    open: &mut MessageWriter<OpenConfirmDialog>,
    start: usize,
    end: usize,
) {
    let tool = state.timeline_tool;
    let key = match tool {
        TimelineTool::Erase => "editor-confirm-erase",
        TimelineTool::Remove => "editor-confirm-remove",
        TimelineTool::None => return,
        TimelineTool::Select => return,
        // Toggled directly by `on_timeline_click_tempo` — never goes
        // through the confirm-dialog path this function drives.
        TimelineTool::Tempo | TimelineTool::Meter => return,
    };
    let map = state.meter_map();
    state.pending_timeline_op = Some((tool, start, end));
    let message = loc
        .msg_args(key, &[("from", describe_tick(start, &map)), ("to", describe_tick(end, &map))])
        .to_string();
    open.write(OpenConfirmDialog { purpose: TIMELINE_CONFIRM_PURPOSE, message });
}

// ── Observers ─────────────────────────────────────────────────────────────────

fn hovered_tick(
    entity: Entity,
    geoms: &Query<&TimelineSurfaceGeometry>,
    rels: &Query<&RelativeCursorPosition>,
) -> Option<usize> {
    let geom = geoms.get(entity).ok()?;
    let rel = rels.get(entity).ok()?;
    Some(geom.tick_at(rel.normalized?.x))
}

/// The Tempo tool's whole interaction: a plain click toggles a tempo-change
/// point at the clicked tick (see `state::toggle_tempo_point`) — no confirm
/// dialog, no drag-span selection, unlike Select/Erase/Remove. Reacting to
/// `PointerClick` directly (rather than routing through `Drag*` like
/// every other timeline tool) is safe *only* because this tool never cares
/// about a drag span at all; the module doc's "`Click`/`DragEnd` race" only
/// matters for tools that read `EditorState::timeline_drag`, which this one
/// doesn't touch.
// not-a-widget-button: the timeline is a continuous drag surface, so the
// click position itself is the input; there is no discrete Button to focus.
pub(super) fn on_timeline_click_tempo(
    ev: On<PointerClick>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    mut state: ResMut<EditorState>,
) {
    if state.mode != Mode::Edit || state.timeline_tool != TimelineTool::Tempo {
        return;
    }
    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };
    toggle_tempo_point(&mut state, tick);
}

/// Cycles the nearest meter change, or starts one on the nearest beat of the
/// meter in force. Cycling back to the preceding meter removes the point;
/// tick zero remains owned by the Details picker.
pub(super) fn cycle_meter_point(state: &mut EditorState, tick: usize) {
    use harmonicon_ui::music_score::TIME_SIGNATURES;
    const SNAP_TICKS: usize = super::TICKS_PER_BEAT / 2;
    if tick < SNAP_TICKS {
        return;
    }
    let map = state.meter_map();
    let next_signature = |current: &str| -> String {
        let idx = TIME_SIGNATURES.iter().position(|&s| s == current);
        TIME_SIGNATURES[idx.map_or(0, |i| (i + 1) % TIME_SIGNATURES.len())].to_string()
    };
    let nearest = state
        .meter_changes
        .iter()
        .enumerate()
        .filter(|&(_, (change_tick, _))| change_tick.abs_diff(tick) <= SNAP_TICKS)
        .min_by_key(|&(_, (change_tick, _))| change_tick.abs_diff(tick));
    if let Some((index, (change_tick, current))) = nearest {
        let before = map.meter_at(change_tick.saturating_sub(1) as u64);
        let before = format!("{}/{}", before.numerator, before.denominator);
        let next = next_signature(current);
        if next == before {
            state.meter_changes.remove(index);
        } else {
            state.meter_changes[index].1 = next;
        }
        return;
    }
    let segment = map.segment_at(tick as u64);
    let beat = segment.ticks_per_beat as usize;
    let offset = tick - segment.start_tick as usize;
    let snapped = segment.start_tick as usize + (offset + beat / 2) / beat * beat;
    let current = format!("{}/{}", segment.meter.numerator, segment.meter.denominator);
    state.meter_changes.push((snapped, next_signature(&current)));
}

/// The Meter tool's whole interaction — the Tempo tool's sibling, same
/// plain-`Click` reasoning as [`on_timeline_click_tempo`] above.
// not-a-widget-button: same continuous drag surface as the tempo handler.
pub(super) fn on_timeline_click_meter(
    ev: On<PointerClick>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    mut state: ResMut<EditorState>,
) {
    if state.mode != Mode::Edit || state.timeline_tool != TimelineTool::Meter {
        return;
    }
    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };
    cycle_meter_point(&mut state, tick);
}

/// Record mode's own use of the ruler: a click parks the playhead (the red
/// `PlayheadLine`) at the clicked tick, and the next take records from
/// there — see `record::start_record`, which reads `Playhead::elapsed` as
/// its start position. Armed as a *paused* transport (`playing` + `paused`)
/// so the line is visible while parked; every route out of Record mode
/// already stops the transport, so the armed state can't leak into
/// Play/Edit. Ignored while a take is actually running — the playhead is
/// the recording's own cursor then.
// not-a-widget-button: same continuous drag surface as the tempo handler
// above — the seek target is the x position, not a focusable control.
pub(super) fn on_timeline_click_seek(
    ev: On<PointerClick>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    state: Res<EditorState>,
    record: Res<RecordState>,
    mut playhead: ResMut<Playhead>,
) {
    if state.mode != Mode::Record || record.active {
        return;
    }
    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };
    let spt = secs_per_tick(&state);
    playhead.secs_per_tick = spt;
    playhead.elapsed = tick as f32 * spt;
    playhead.playing = true;
    playhead.paused = true;
}

pub(super) fn on_timeline_drag_start(
    ev: On<PointerDragStart>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    state: Res<EditorState>,
    scroll: Res<Scroll>,
    mut sel: ResMut<TimelineSelection>,
) {
    // The timeline tools are Edit-mode concepts (their toggle buttons live
    // in the Edit tool strip); a still-armed tool must not hijack ruler
    // clicks in Record mode, whose own seek handler owns them there.
    if state.mode != Mode::Edit || state.timeline_tool != TimelineTool::Select {
        return;
    }

    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };
    sel.drag = Some(TimelineDrag {
        start: tick,
        end: tick,
        scroll_px: scroll.px,
        pointer_px: 0.0,
        live: true,
    });
}

/// The current end tick of a span drag: `distance_x` is raw window pixels
/// of pointer motion from the press, divided by `UiScale` — same
/// quantity/correction note-move dragging uses in `grid.rs`. Deliberately
/// reused rather than re-deriving the tick from `RelativeCursorPosition`: a
/// drag routinely carries the pointer well outside the ruler's thin
/// `HEADER_H`-tall box (down over the note grid), and `distance` keeps
/// tracking correctly regardless. `scroll_delta_px` is how far the grid has
/// scrolled *under* the pointer since the press (same logical px `Scroll`
/// uses, not scale-divided) — without it, a mid-drag wheel pan would leave
/// the span end pinned to the press-time content instead of following
/// what's now under the pointer.
pub(super) fn drag_end_tick(
    start: usize,
    distance_x: f32,
    ui_scale: f32,
    scroll_delta_px: f32,
) -> usize {
    let motion_px = distance_x / ui_scale.max(f32::EPSILON) + scroll_delta_px;
    let delta_ticks = (motion_px / TICK_W).round() as i64;
    (start as i64 + delta_ticks).max(0) as usize
}

pub(super) fn on_timeline_drag(
    ev: On<PointerDrag>,
    state: Res<EditorState>,
    scroll: Res<Scroll>,
    mut sel: ResMut<TimelineSelection>,
    ui_scale: Res<UiScale>,
) {
    if !state.timeline_tool.is_active() {
        return;
    }
    let Some(TimelineDrag { start, scroll_px, .. }) = sel.drag else {
        return;
    };
    let end = drag_end_tick(start, ev.distance.x, ui_scale.0, scroll.px - scroll_px);
    sel.drag = Some(TimelineDrag {
        start,
        end,
        scroll_px,
        pointer_px: ev.distance.x / ui_scale.0.max(f32::EPSILON),
        live: true,
    });
}

/// Re-derives an in-progress drag's `end` whenever the grid scrolls —
/// `PointerDrag` only fires on pointer *motion*, so a wheel pan under a
/// stationary held pointer would otherwise leave the span's end stale
/// (including at release, silently dropping the scrolled-to extent). Same
/// math as [`on_timeline_drag`], fed the stored [`TimelineDrag::pointer_px`]
/// (already scale-corrected, hence the `1.0`) instead of a fresh event.
pub(super) fn sync_selection_with_scroll(scroll: Res<Scroll>, mut sel: ResMut<TimelineSelection>) {
    if !scroll.is_changed() {
        return;
    }
    let Some(drag) = sel.drag else {
        return;
    };
    if !drag.live {
        return;
    }
    let end = drag_end_tick(drag.start, drag.pointer_px, 1.0, scroll.px - drag.scroll_px);
    if end != drag.end {
        sel.drag = Some(TimelineDrag { end, ..drag });
    }
}

/// Only the Select tool ever has a drag in flight (see
/// [`on_timeline_drag_start`]), so this is Select's release logic: a span
/// that genuinely moved becomes the persisted selection; a same-tick
/// "drag" is really a click, driving the two-click split flow — first
/// click places the split point, second click turns the hovered side into
/// the selection. Either way the result is a frozen [`TimelineSelection`]
/// span for the Erase/Remove buttons to act on — nothing here opens the
/// confirm dialog itself.
pub(super) fn on_timeline_drag_end(
    _ev: On<PointerDragEnd>,
    mut state: ResMut<EditorState>,
    mut sel: ResMut<TimelineSelection>,
) {
    let Some(drag) = sel.drag else {
        return;
    };
    let (s, e) = normalize_range(drag.start, drag.end);
    if e > s {
        // A real drag: an explicit span, superseding any stale split point
        // from an earlier abandoned click sequence. Stays in
        // `TimelineSelection` as the persisted selection, but frozen —
        // released spans must not keep tracking scroll.
        sel.drag = Some(TimelineDrag { live: false, ..drag });
        state.timeline_split = None;
        return;
    }
    // The span never moved a tick — an ordinary click (see the module docs
    // for why this, not `PointerClick`, is what decides that). Not yet a
    // selection, so drop it rather than leaving a zero-width span shadowing
    // the split-point overlay.
    sel.drag = None;
    match state.timeline_split {
        None => state.timeline_split = Some(s),
        Some(split) => {
            let side = if s < split { Side::Left } else { Side::Right };
            let (start, end) = split_side_range(split, side, &state.notes);
            state.timeline_split = None;
            if end > start {
                sel.drag =
                    Some(TimelineDrag { start, end, scroll_px: 0.0, pointer_px: 0.0, live: false });
            }
        }
    }
}

/// The timeline selection as whole bars, or `None` after telling the player
/// to make one — what the Repeat and Ending buttons act on.
fn selected_bars(
    state: &EditorState,
    sel: &TimelineSelection,
    feedback: &mut SaveFeedback,
    loc: &Localization,
) -> Option<(u64, u64)> {
    let Some(TimelineDrag { start, end, .. }) = sel.drag else {
        feedback.set(loc.msg("editor-repeat-needs-selection"));
        return None;
    };
    let (start, end) = normalize_range(start, end);
    Some(super::repeat_marks::bar_span(&state.meter_map(), start, end))
}

/// The Repeat button: repeats the selected bars, or adds a pass to a
/// passage already repeated — see `repeat_marks::toggle_repeat`. The
/// selection stays, so pressing again keeps counting.
pub(super) fn on_repeat_button(
    _: On<Activate>,
    loc: Res<Localization>,
    sel: Res<TimelineSelection>,
    mut state: ResMut<EditorState>,
    mut feedback: ResMut<SaveFeedback>,
) {
    if let Some((start, end)) = selected_bars(&state, &sel, &mut feedback, &loc) {
        super::repeat_marks::toggle_repeat(&mut state.repeats, start, end);
    }
}

/// The Ending button: makes the selected bars a first- or second-time
/// ending of the passage they belong to — see `repeat_marks::toggle_ending`.
pub(super) fn on_ending_button(
    _: On<Activate>,
    loc: Res<Localization>,
    sel: Res<TimelineSelection>,
    mut state: ResMut<EditorState>,
    mut feedback: ResMut<SaveFeedback>,
) {
    if let Some((start, end)) = selected_bars(&state, &sel, &mut feedback, &loc)
        && !super::repeat_marks::toggle_ending(&mut state.repeats, start, end)
    {
        feedback.set(loc.msg("editor-ending-needs-repeat"));
    }
}

/// Reacts to the confirm dialog's answer for [`TIMELINE_CONFIRM_PURPOSE`] —
/// applies `ranges::erase_range`/`ranges::remove_range` on `confirmed: true`,
/// otherwise just drops the pending request. Ignores every other purpose,
/// so this can share `ConfirmChosen` with any future confirm-dialog user.
pub(super) fn handle_timeline_confirm(
    mut chosen: MessageReader<ConfirmChosen>,
    mut state: ResMut<EditorState>,
) {
    for ev in chosen.read() {
        if ev.purpose != TIMELINE_CONFIRM_PURPOSE {
            continue;
        }
        let Some((tool, start, end)) = state.pending_timeline_op.take() else {
            continue;
        };
        if !ev.confirmed {
            continue;
        }
        match tool {
            TimelineTool::Erase => state.erase_notes_in(start, end),
            TimelineTool::Remove => state.remove_range_closing_gap(start, end),
            TimelineTool::None
            | TimelineTool::Select
            | TimelineTool::Tempo
            | TimelineTool::Meter => {
                continue;
            }
        }
    }
}
