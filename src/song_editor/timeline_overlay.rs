// SPDX-License-Identifier: MIT

//! The timeline tools' rendering half: the persistent split-line/highlight
//! overlay entities and the per-frame pass that redraws them from
//! [`TimelineSelection`]/`EditorState`, plus the one-time spawn of every
//! persistent timeline entity (including `timeline`'s interactive surface).
//! The interaction logic itself — the drag observers, span math, and
//! confirm-dialog flow — lives in the sibling `timeline` module.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use super::state::{EditorState, Side, TimelineDrag, TimelineSelection, normalize_range, split_side_range};
use super::timeline::{
    TimelineSurface, TimelineSurfaceGeometry, on_timeline_click_tempo, on_timeline_drag,
    on_timeline_drag_end, on_timeline_drag_start,
};
use super::{HEADER_H, TICK_W};

/// Persistent overlay entities (spawned once in `ui::setup`, like
/// `MoveGhost`/`PlayheadLine` — never despawned/respawned by `grid::
/// rebuild_grid`), updated every frame by [`update_timeline_overlays`].
#[derive(Component)]
pub(super) struct TimelineSplitLine;

#[derive(Component)]
pub(super) struct TimelineHighlight;

/// Bundle for the persistent interactive header surface — see `ui::setup`'s
/// single call site. Position/size are placeholders until the first
/// [`sync_timeline_surface`] run.
pub(super) fn timeline_surface_bundle() -> impl Bundle {
    (
        TimelineSurface,
        TimelineSurfaceGeometry {
            scroll_px: 0.0,
            width_px: 0.0,
        },
        RelativeCursorPosition::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(0.0),
            height: Val::Px(HEADER_H),
            ..default()
        },
        Pickable::default(),
    )
}

/// Spawns this module's persistent `GridContent` children, once, from
/// `ui::setup` — the split-line/highlight overlays (hidden until a tool
/// uses them; repositioned every frame by [`update_timeline_overlays`],
/// like the playhead/move ghost) and the [`TimelineSurface`] click/drag
/// catcher. The surface is deliberately persistent rather than respawned
/// by `grid::rebuild_grid` — see its type docs — and always present: its
/// observers no-op when no matching tool is active.
pub(super) fn spawn_persistent_entities(
    content: &mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>,
    hole_count: u8,
) {
    let grid_h = super::grid_height(hole_count);
    content.spawn((
        TimelineSplitLine,
        ZIndex(3),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            width: Val::Px(2.0),
            height: Val::Px(grid_h),
            ..default()
        },
        BackgroundColor(Color::srgb(0.95, 0.75, 0.20)),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
    content.spawn((
        TimelineHighlight,
        ZIndex(1),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            height: Val::Px(grid_h),
            ..default()
        },
        BackgroundColor(Color::srgba(0.95, 0.30, 0.20, 0.22)),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
    content
        .spawn(timeline_surface_bundle())
        .observe(on_timeline_drag_start)
        .observe(on_timeline_drag)
        .observe(on_timeline_drag_end)
        .observe(on_timeline_click_tempo);
}

// ── Per-frame overlay update ─────────────────────────────────────────────────

/// Redraws the split-line/highlight overlay entities every frame —
/// unconditional, like `playback::update_playhead_view`/`interaction::
/// update_move_ghost`. Purely a rendering pass: the live hover-side preview
/// reads `RelativeCursorPosition` fresh each frame as a local value rather
/// than writing it back to `EditorState` (unlike the in-progress drag span,
/// which genuinely is state — in [`TimelineSelection`]), so this never
/// marks `EditorState` "changed" and can't trigger needless grid rebuilds.
pub(super) fn update_timeline_overlays(
    state: Res<EditorState>,
    sel: Res<TimelineSelection>,
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
    if let Some(TimelineDrag { start, end, .. }) = sel.drag {
        hide(&mut split_lines);
        let (s, e) = normalize_range(start, end);
        set_highlight(&mut highlights, s, e.max(s + 1));
        return;
    }

    let Some(split) = state.timeline_split else {
        hide(&mut split_lines);
        hide(&mut highlights);
        return;
    };
    if let Ok((mut node, mut vis)) = split_lines.single_mut() {
        node.left = Val::Px(split as f32 * TICK_W);
        *vis = Visibility::Inherited;
    }

    let Some(hover) = surfaces.iter().find_map(|(geom, rel)| {
        rel.normalized.map(|n| geom.tick_at(n.x))
    }) else {
        hide(&mut highlights);
        return;
    };
    let side = if hover < split { Side::Left } else { Side::Right };
    let (start, end) = split_side_range(split, side, &state.notes);
    set_highlight(&mut highlights, start, end.max(start + 1));
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
