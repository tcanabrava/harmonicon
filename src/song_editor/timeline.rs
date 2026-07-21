// SPDX-License-Identifier: MIT

//! The timeline ruler's Select editing tool. With Select active
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
//! Both paths are driven entirely by `Pointer<DragStart>`/`Drag`/`DragEnd`
//! — deliberately *not* `Pointer<Click>` at all, even for the "plain click"
//! case: `bevy_picking` fires `DragStart` on any nonzero pixel motion while
//! pressed, so ordinary mouse jitter during an intended click routinely
//! produces a same-tick drag anyway, and a `Click` event fires *alongside*
//! `DragEnd` on the same release whenever the pointer is still over the
//! surface at release (true for most drags — only a large enough motion
//! carries it off the ruler's thin `HEADER_H`-tall strip), with `Click`
//! firing first. Routing every decision through the one `Drag*` chain
//! avoids that race outright instead of coordinating two competing
//! handlers; [`on_timeline_drag_end`] tells a real drag apart from a
//! same-tick click by whether the span actually moved.
//!
//! Either way nothing is deleted until the confirm dialog
//! (`dialogs::confirm_dialog`) comes back `confirmed: true` — see
//! [`handle_timeline_confirm`]. The actual note-list surgery is the pure
//! `ranges::erase_range`/`ranges::remove_range` pair; this module is just
//! the interaction/UI wiring around them.

use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy_fluent::prelude::Localization;

use super::playback::{Playhead, secs_per_tick};
use super::ranges::{erase_range, normalize_range, remove_range, split_side_range};
use super::record::RecordState;
use super::state::{
    EditorState, Mode, Scroll, Side, TimelineDrag, TimelineSelection, TimelineTool,
    toggle_tempo_point,
};
use super::{BEAT_W, BEATS_PER_BAR, TICK_W, TICKS_PER_BEAT};
use crate::dialogs::confirm_dialog::{ConfirmChosen, DialogId, OpenConfirmDialog};
use crate::localization::LocalizationExt;

pub(super) const TIMELINE_CONFIRM_PURPOSE: DialogId = DialogId("song_editor_2_timeline_confirm");

// ── Components ────────────────────────────────────────────────────────────────

/// The invisible, header-strip-sized click/drag catcher. Spawned *once* in
/// `ui::setup` (like `MoveGhost`/`PlayheadLine`) rather than respawned by
/// `grid::rebuild_grid`: a rebuild mid-gesture would despawn the entity
/// `bevy_picking` has captured the drag on, killing its `Drag`/`DragEnd`
/// delivery — and rebuilds *do* now happen mid-gesture, since a wheel pan
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
            *geom = TimelineSurfaceGeometry {
                scroll_px: scroll.px,
                width_px,
            };
            node.left = Val::Px(scroll.px);
            node.width = Val::Px(width_px);
        }
    }
}

// ── Pure display helper ──────────────────────────────────────────────────────

/// A tick as "bar.beat" (1-indexed), matching the numbers already shown on
/// the ruler — used in the confirm dialog's message.
fn describe_tick(tick: usize) -> String {
    let beat = tick / TICKS_PER_BEAT;
    let bar = beat / BEATS_PER_BAR + 1;
    let beat_in_bar = beat % BEATS_PER_BAR + 1;
    format!("{bar}.{beat_in_bar}")
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
        TimelineTool::Tempo => return,
    };
    state.pending_timeline_op = Some((tool, start, end));
    let message = loc
        .msg_args(
            key,
            &[("from", describe_tick(start)), ("to", describe_tick(end))],
        )
        .to_string();
    open.write(OpenConfirmDialog {
        purpose: TIMELINE_CONFIRM_PURPOSE,
        message,
    });
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
/// `Pointer<Click>` directly (rather than routing through `Drag*` like
/// every other timeline tool) is safe *only* because this tool never cares
/// about a drag span at all; the module doc's "`Click`/`DragEnd` race" only
/// matters for tools that read `EditorState::timeline_drag`, which this one
/// doesn't touch.
pub(super) fn on_timeline_click_tempo(
    ev: On<Pointer<Click>>,
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

/// Record mode's own use of the ruler: a click parks the playhead (the red
/// `PlayheadLine`) at the clicked tick, and the next take records from
/// there — see `record::start_record`, which reads `Playhead::elapsed` as
/// its start position. Armed as a *paused* transport (`playing` + `paused`)
/// so the line is visible while parked; every route out of Record mode
/// already stops the transport, so the armed state can't leak into
/// Play/Edit. Ignored while a take is actually running — the playhead is
/// the recording's own cursor then.
pub(super) fn on_timeline_click_seek(
    ev: On<Pointer<Click>>,
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
    ev: On<Pointer<DragStart>>,
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

/// The current end tick of a span drag: `distance_x` raw window pixels of
/// pointer motion from the press — the same quantity/correction note-move
/// dragging already uses in `grid.rs` (`ev.distance` divided by `UiScale`,
/// the arrow-key UI zoom), deliberately reused here rather than re-deriving
/// the current tick from `RelativeCursorPosition`: a drag routinely carries
/// the pointer well outside the ruler's own thin `HEADER_H`-tall box (down
/// over the note grid, since that's a natural drag motion), and `distance`
/// keeps tracking correctly regardless of where the pointer physically ends
/// up — plus `scroll_delta_px`, how far the grid has scrolled *under* the
/// pointer since the press (in the same logical px `Scroll` uses, so not
/// scale-divided): a mid-drag wheel pan moves the content, not the pointer,
/// and without this term the span end would stay pinned to wherever the
/// content sat at press time instead of following what's now under the
/// pointer.
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
    ev: On<Pointer<Drag>>,
    state: Res<EditorState>,
    scroll: Res<Scroll>,
    mut sel: ResMut<TimelineSelection>,
    ui_scale: Res<UiScale>,
) {
    if !state.timeline_tool.is_active() {
        return;
    }
    let Some(TimelineDrag {
        start, scroll_px, ..
    }) = sel.drag
    else {
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
/// `Pointer<Drag>` only fires on pointer *motion*, so a wheel pan under a
/// stationary held pointer (the natural "scroll to reach more of the song
/// mid-selection" gesture) would otherwise leave the span's end stale until
/// the mouse happens to move again — including at release, silently
/// dropping the scrolled-to extent from the selection. Same math as
/// [`on_timeline_drag`], fed the stored [`TimelineDrag::pointer_px`]
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
    _ev: On<Pointer<DragEnd>>,
    mut state: ResMut<EditorState>,
    mut sel: ResMut<TimelineSelection>,
) {
    let Some(drag) = sel.drag else {
        return;
    };
    let (s, e) = normalize_range(drag.start, drag.end);
    if e > s {
        // A real drag: an explicit span, superseding any stale split point
        // from an earlier, abandoned click sequence. It stays in
        // `TimelineSelection` as the persisted selection, but frozen —
        // released spans must not keep tracking the grid scrolling under
        // them.
        sel.drag = Some(TimelineDrag {
            live: false,
            ..drag
        });
        state.timeline_split = None;
        return;
    }
    // The span never moved a tick — an ordinary click (see the module docs
    // for why this, not `Pointer<Click>`, is what decides that). Not yet a
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
                sel.drag = Some(TimelineDrag {
                    start,
                    end,
                    scroll_px: 0.0,
                    pointer_px: 0.0,
                    live: false,
                });
            }
        }
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
        state.notes = match tool {
            TimelineTool::Erase => erase_range(&state.notes, start, end),
            TimelineTool::Remove => remove_range(&state.notes, start, end),
            TimelineTool::None => continue,
            TimelineTool::Select => continue,
            TimelineTool::Tempo => continue,
        };
        state.selected.clear();
    }
}
