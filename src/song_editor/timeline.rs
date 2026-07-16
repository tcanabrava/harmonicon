// SPDX-License-Identifier: MIT

//! The timeline ruler's Erase/Remove editing tool. With a tool selected
//! (`EditorState::timeline_tool`), the header strip above the note grid
//! becomes interactive two ways:
//!
//! - **Click, hover, click**: a plain click drops a split point; hovering
//!   left or right of it previews that whole side (song start..split, or
//!   split..song end); clicking again on the highlighted side requests
//!   confirmation of the tool's effect on that range.
//! - **Click, drag, release**: picks an explicit span instead, previewed
//!   live as it's dragged, requesting confirmation on release.
//!
//! Either way nothing is deleted until the confirm dialog
//! (`dialogs::confirm_dialog`) comes back `confirmed: true` — see
//! [`handle_timeline_confirm`]. The actual note-list surgery is the pure
//! `state::erase_range`/`state::remove_range` pair; this module is just the
//! interaction/UI wiring around them.

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy_fluent::prelude::Localization;

use super::state::{
    EditorState, Side, TimelineDrag, TimelineTool, erase_range, normalize_range, remove_range,
    split_side_range,
};
use super::{BEATS_PER_BAR, BEAT_W, HEADER_H, TICKS_PER_BEAT, TICK_W};
use crate::dialogs::confirm_dialog::{ConfirmChosen, DialogId, OpenConfirmDialog};
use crate::localization::LocalizationExt;

pub(super) const TIMELINE_CONFIRM_PURPOSE: DialogId = DialogId("song_editor_2_timeline_confirm");

// ── Components ────────────────────────────────────────────────────────────────

/// The invisible, header-strip-sized click/drag catcher `grid::rebuild_grid`
/// (re)spawns every rebuild — see [`TimelineSurfaceGeometry`] for how a
/// click on it becomes an absolute tick.
#[derive(Component)]
pub(super) struct TimelineSurface;

/// The pixel geometry a [`TimelineSurface`] was spawned with, needed to
/// convert its own `RelativeCursorPosition` (0..1 across *its* box) into an
/// absolute tick. Recorded at spawn time rather than re-derived from a
/// window/`ComputedNode` query, since the surface's own position/size were
/// computed from exactly these two numbers in the first place (see
/// `grid::rebuild_grid`).
#[derive(Component, Clone, Copy)]
pub(super) struct TimelineSurfaceGeometry {
    pub(super) scroll_beat: usize,
    pub(super) width_px: f32,
}

impl TimelineSurfaceGeometry {
    fn tick_at(&self, normalized_x: f32) -> usize {
        let abs_px =
            self.scroll_beat as f32 * BEAT_W + normalized_x.clamp(0.0, 1.0) * self.width_px;
        (abs_px / TICK_W).round().max(0.0) as usize
    }
}

/// Persistent overlay entities (spawned once in `ui::setup`, like
/// `MoveGhost`/`PlayheadLine` — never despawned/respawned by `grid::
/// rebuild_grid`), updated every frame by [`update_timeline_overlays`].
#[derive(Component)]
pub(super) struct TimelineSplitLine;

#[derive(Component)]
pub(super) struct TimelineHighlight;

/// Bundle for the (re)spawned interactive header surface — see
/// `grid::rebuild_grid`'s single call site.
pub(super) fn timeline_surface_bundle(scroll_beat: usize, width_px: f32) -> impl Bundle {
    (
        TimelineSurface,
        TimelineSurfaceGeometry {
            scroll_beat,
            width_px,
        },
        RelativeCursorPosition::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(scroll_beat as f32 * BEAT_W),
            top: Val::Px(0.0),
            width: Val::Px(width_px),
            height: Val::Px(HEADER_H),
            ..default()
        },
        Pickable::default(),
    )
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

fn request_confirm(
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
    };
    state.pending_timeline_op = Some((tool, start, end));
    let message = loc
        .msg_args(
            key,
            &[
                ("from", describe_tick(start)),
                ("to", describe_tick(end)),
            ],
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

pub(super) fn on_timeline_click(
    ev: On<Pointer<Click>>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    mut state: ResMut<EditorState>,
    loc: Res<Localization>,
    mut open: MessageWriter<OpenConfirmDialog>,
) {
    if !state.timeline_tool.is_active() {
        return;
    }
    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };

    match state.timeline_drag {
        None => state.timeline_drag = Some(TimelineDrag::Split { tick, hover: tick }),
        Some(TimelineDrag::Split { tick: split, .. }) => {
            let side = if tick < split { Side::Left } else { Side::Right };
            let (start, end) = split_side_range(split, side, &state.notes);
            state.timeline_drag = None;
            if end > start {
                request_confirm(&mut state, &loc, &mut open, start, end);
            }
        }
        Some(TimelineDrag::Span { .. }) => state.timeline_drag = None,
    }
}

pub(super) fn on_timeline_drag_start(
    ev: On<Pointer<DragStart>>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    mut state: ResMut<EditorState>,
) {
    if !state.timeline_tool.is_active() {
        return;
    }
    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };
    state.timeline_drag = Some(TimelineDrag::Span {
        start: tick,
        end: tick,
    });
}

pub(super) fn on_timeline_drag(
    ev: On<Pointer<Drag>>,
    geoms: Query<&TimelineSurfaceGeometry>,
    rels: Query<&RelativeCursorPosition>,
    mut state: ResMut<EditorState>,
) {
    if !state.timeline_tool.is_active() {
        return;
    }
    let Some(tick) = hovered_tick(ev.entity, &geoms, &rels) else {
        return;
    };
    if let Some(TimelineDrag::Span { start, .. }) = state.timeline_drag {
        state.timeline_drag = Some(TimelineDrag::Span { start, end: tick });
    }
}

pub(super) fn on_timeline_drag_end(
    _ev: On<Pointer<DragEnd>>,
    mut state: ResMut<EditorState>,
    loc: Res<Localization>,
    mut open: MessageWriter<OpenConfirmDialog>,
) {
    if !state.timeline_tool.is_active() {
        return;
    }
    if let Some(TimelineDrag::Span { start, end }) = state.timeline_drag {
        state.timeline_drag = None;
        let (start, end) = normalize_range(start, end);
        if end > start {
            request_confirm(&mut state, &loc, &mut open, start, end);
        }
    }
}

/// Reacts to the confirm dialog's answer for [`TIMELINE_CONFIRM_PURPOSE`] —
/// applies `state::erase_range`/`state::remove_range` on `confirmed: true`,
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
        };
        state.selected = None;
    }
}

// ── Per-frame overlay update ─────────────────────────────────────────────────

/// Keeps the live hover tick current while a split point is pending (so the
/// highlighted side follows the pointer), and redraws the split-line/
/// highlight overlay entities every frame — unconditional, like
/// `playback::update_playhead_view`/`interaction::update_move_ghost`, so it
/// still runs while `timeline_drag`'s own mutation below is what's making
/// `EditorState` look "changed" every frame (`grid::rebuild_grid` itself
/// early-returns on `timeline_drag.is_some()`, the same guard note dragging
/// already relies on, so that mutation doesn't thrash a full grid rebuild).
pub(super) fn update_timeline_overlays(
    mut state: ResMut<EditorState>,
    surfaces: Query<(&TimelineSurfaceGeometry, &RelativeCursorPosition), With<TimelineSurface>>,
    mut split_lines: Query<
        (&mut Node, &mut Visibility),
        (With<TimelineSplitLine>, Without<TimelineHighlight>),
    >,
    mut highlights: Query<
        (&mut Node, &mut Visibility),
        (With<TimelineHighlight>, Without<TimelineSplitLine>),
    >,
) {
    if let Some(TimelineDrag::Split { tick, hover }) = state.timeline_drag {
        for (geom, rel) in &surfaces {
            if let Some(norm) = rel.normalized {
                let live_hover = geom.tick_at(norm.x);
                if live_hover != hover {
                    state.timeline_drag = Some(TimelineDrag::Split {
                        tick,
                        hover: live_hover,
                    });
                }
                break;
            }
        }
    }

    match state.timeline_drag {
        Some(TimelineDrag::Split { tick, hover }) => {
            if let Ok((mut node, mut vis)) = split_lines.single_mut() {
                node.left = Val::Px(tick as f32 * TICK_W);
                *vis = Visibility::Inherited;
            }
            let side = if hover < tick { Side::Left } else { Side::Right };
            let (start, end) = split_side_range(tick, side, &state.notes);
            set_highlight(&mut highlights, start, end.max(start + 1));
        }
        Some(TimelineDrag::Span { start, end }) => {
            hide(&mut split_lines);
            let (s, e) = normalize_range(start, end);
            set_highlight(&mut highlights, s, e.max(s + 1));
        }
        None => {
            hide(&mut split_lines);
            hide(&mut highlights);
        }
    }
}

fn hide<F: bevy::ecs::query::QueryFilter>(q: &mut Query<(&mut Node, &mut Visibility), F>) {
    if let Ok((_, mut vis)) = q.single_mut() {
        *vis = Visibility::Hidden;
    }
}

fn set_highlight(
    q: &mut Query<
        (&mut Node, &mut Visibility),
        (With<TimelineHighlight>, Without<TimelineSplitLine>),
    >,
    start: usize,
    end: usize,
) {
    if let Ok((mut node, mut vis)) = q.single_mut() {
        node.left = Val::Px(start as f32 * TICK_W);
        node.width = Val::Px((end - start) as f32 * TICK_W);
        *vis = Visibility::Inherited;
    }
}
