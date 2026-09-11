// SPDX-License-Identifier: MIT

//! Collapse, compaction, viewport, and neighboring-unit transitions.

use std::collections::{HashMap, HashSet};

use bevy::a11y::AccessibilityNode;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, ScrollPosition, UiTransform, Val2};
use bevy::ui_widgets::Button as WidgetButton;

use harmonicon_menu::menu::MenuPage;

use super::{
    ClusterMember, Endpoint, LayoutOwner, MovingEdge, UnitButton, UnitChevron, set_edge_geometry,
};

const TRANSITION_SECONDS: f32 = 0.22;

#[derive(Resource, Default)]
pub(crate) struct CollapsedUnits(pub(super) HashSet<String>);

#[derive(Resource, Default)]
pub(crate) struct UnitExpansions(pub(super) HashMap<String, f32>);

#[derive(Resource, Default)]
pub(crate) struct PendingCompaction(pub(super) HashSet<String>);

#[derive(Resource, Default)]
pub(crate) struct PendingViewportAnchor {
    pub(super) unit_id: Option<String>,
    pub(super) screen_x: f32,
    pub(super) canvas_x: Option<f32>,
}

#[derive(Resource, Default)]
pub(crate) struct PreviousUnitPositions(pub(super) HashMap<String, f32>);

#[derive(Clone, Copy, Debug)]
pub(super) struct UnitSlide {
    pub(super) from_px: f32,
    pub(super) amount: f32,
}

#[derive(Resource, Default)]
pub(crate) struct UnitSlides(pub(super) HashMap<String, UnitSlide>);

#[derive(Component)]
pub(crate) struct LessonTreeScroller;

/// Last viewport position, retained while the reader or gameplay page owns
/// the screen. The tree itself is despawned on every page change.
#[derive(Resource, Default)]
pub(crate) struct LessonTreeViewport(pub(super) Vec2);

/// A lesson requested by the header locator. Its canvas position is filled
/// during tree construction, after a collapsed unit has been expanded and
/// the compact layout has consequently changed.
#[derive(Resource, Default)]
pub(crate) struct PendingLessonFocus {
    pub(super) lesson_id: Option<String>,
    pub(super) canvas_position: Option<Vec2>,
}

pub(crate) fn focus_pending_lesson(
    mut pending: ResMut<PendingLessonFocus>,
    mut scroller: Query<(&mut ScrollPosition, &ComputedNode), With<LessonTreeScroller>>,
) {
    let Some(target) = pending.canvas_position else {
        return;
    };
    let Some((mut position, computed)) = scroller.iter_mut().next() else {
        return;
    };
    let scale = computed.inverse_scale_factor;
    let viewport = computed.size() * scale;
    let content = computed.content_size() * scale;
    if viewport.min_element() <= 0.0 || content.min_element() <= 0.0 {
        return;
    }

    position.0 = centred_scroll(target, viewport, content);
    pending.lesson_id = None;
    pending.canvas_position = None;
}

pub(super) fn centred_scroll(target: Vec2, viewport: Vec2, content: Vec2) -> Vec2 {
    (target - viewport / 2.0).clamp(Vec2::ZERO, (content - viewport).max(Vec2::ZERO))
}

pub(crate) fn remember_viewport(
    scroller: Query<&ScrollPosition, With<LessonTreeScroller>>,
    mut saved: ResMut<LessonTreeViewport>,
) {
    let Some(position) = scroller.iter().next() else {
        return;
    };
    saved.0 = position.0;
}

pub(crate) fn restore_viewport_anchor(
    mut anchor: ResMut<PendingViewportAnchor>,
    mut scroller: Query<(&mut ScrollPosition, &ComputedNode), With<LessonTreeScroller>>,
) {
    let Some(canvas_x) = anchor.canvas_x else {
        return;
    };
    let Some((mut position, computed)) = scroller.iter_mut().next() else {
        return;
    };
    if computed.size().x <= 0.0 || computed.content_size().x <= 0.0 {
        return;
    }

    position.x = anchored_scroll(
        canvas_x,
        anchor.screen_x,
        computed.size().x,
        computed.content_size().x,
    );
    anchor.unit_id = None;
    anchor.canvas_x = None;
}

pub(super) fn anchored_scroll(
    canvas_x: f32,
    screen_x: f32,
    viewport_width: f32,
    content_width: f32,
) -> f32 {
    let max_scroll = (content_width - viewport_width).max(0.0);
    (canvas_x - screen_x).clamp(0.0, max_scroll)
}

pub(crate) fn animate_unit_expansion(
    mut commands: Commands,
    time: Res<Time>,
    collapsed: Res<CollapsedUnits>,
    mut expansions: ResMut<UnitExpansions>,
    mut members: Query<(&ClusterMember, &mut UiTransform, &mut Visibility)>,
    mut chevrons: Query<(&UnitChevron, &mut Text)>,
    mut unit_buttons: Query<(&UnitButton, &mut AccessibilityNode)>,
    mut lesson_buttons: Query<
        (
            Entity,
            &ClusterMember,
            &mut TabIndex,
            Has<InteractionDisabled>,
        ),
        With<WidgetButton>,
    >,
) {
    let step = time.delta_secs() / TRANSITION_SECONDS;
    for (id, amount) in &mut expansions.0 {
        *amount = expansion_after(*amount, collapsed.0.contains(id), step);
    }

    for (member, mut transform, mut visibility) in &mut members {
        let amount = expansions.0.get(&member.0).copied().unwrap_or(1.0);
        let eased = amount * amount * (3.0 - 2.0 * amount);
        transform.scale = Vec2::splat(eased.max(0.001));
        *visibility = if amount <= 0.0 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }

    for (chevron, mut text) in &mut chevrons {
        **text = if collapsed.0.contains(&chevron.0) {
            "▶".to_string()
        } else {
            "▼".to_string()
        };
    }
    for (button, mut accessibility) in &mut unit_buttons {
        accessibility.set_expanded(!collapsed.0.contains(&button.0));
    }
    for (entity, member, mut tab_index, disabled) in &mut lesson_buttons {
        let closing = collapsed.0.contains(&member.0);
        tab_index.0 = if closing { -1 } else { 0 };
        match (closing, disabled) {
            (true, false) => {
                commands.entity(entity).insert(InteractionDisabled);
            }
            (false, true) => {
                commands.entity(entity).remove::<InteractionDisabled>();
            }
            _ => {}
        }
    }
}

pub(super) fn slide_offset(slide: &UnitSlide) -> f32 {
    let eased = slide.amount * slide.amount * (3.0 - 2.0 * slide.amount);
    slide.from_px * (1.0 - eased)
}

pub(crate) fn animate_unit_slides(
    time: Res<Time>,
    mut slides: ResMut<UnitSlides>,
    mut owned: Query<(&LayoutOwner, &mut UiTransform), Without<MovingEdge>>,
    mut edges: Query<(&MovingEdge, &mut Node)>,
) {
    let step = time.delta_secs() / TRANSITION_SECONDS;
    for slide in slides.0.values_mut() {
        slide.amount = (slide.amount + step).min(1.0);
    }

    for (owner, mut transform) in &mut owned {
        let offset = slides.0.get(&owner.0).map_or(0.0, slide_offset);
        transform.translation = Val2::px(offset, 0.0);
    }
    for (edge, mut node) in &mut edges {
        let from_offset = slides.0.get(&edge.from_unit).map_or(0.0, slide_offset);
        let to_offset = slides.0.get(&edge.to_unit).map_or(0.0, slide_offset);
        let from = Endpoint {
            centre: edge.from.centre + Vec2::X * from_offset,
            ..edge.from
        };
        let to = Endpoint {
            centre: edge.to.centre + Vec2::X * to_offset,
            ..edge.to
        };
        set_edge_geometry(&mut node, from, to, edge.thickness);
    }
    slides.0.retain(|_, slide| slide.amount < 1.0);
}

pub(crate) fn compact_finished_units(
    expansions: Res<UnitExpansions>,
    mut pending: ResMut<PendingCompaction>,
    mut page: ResMut<NextState<MenuPage>>,
) {
    if pending.0.is_empty() {
        return;
    }
    let all_closed = pending
        .0
        .iter()
        .all(|id| expansions.0.get(id).is_none_or(|amount| *amount <= 0.0));
    if all_closed {
        pending.0.clear();
        page.set(MenuPage::LessonTree);
    }
}

pub(super) fn expansion_after(current: f32, collapsed: bool, step: f32) -> f32 {
    let target = if collapsed { 0.0 } else { 1.0 };
    if current < target {
        (current + step).min(target)
    } else if current > target {
        (current - step).max(target)
    } else {
        current
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    #[test]
    fn viewport_position_survives_after_the_scroller_is_gone() {
        let mut app = App::new();
        app.init_resource::<LessonTreeViewport>()
            .add_systems(Update, remember_viewport);
        let scroller = app
            .world_mut()
            .spawn((LessonTreeScroller, ScrollPosition(Vec2::new(420.0, 180.0))))
            .id();

        app.update();
        app.world_mut().despawn(scroller);
        app.update();

        assert_eq!(
            app.world().resource::<LessonTreeViewport>().0,
            Vec2::new(420.0, 180.0),
        );
    }

    #[test]
    fn lesson_focus_centres_and_clamps_to_the_scrollable_range() {
        let viewport = Vec2::new(300.0, 200.0);
        let content = Vec2::new(1_000.0, 600.0);

        assert_eq!(
            centred_scroll(Vec2::new(500.0, 300.0), viewport, content),
            Vec2::new(350.0, 200.0),
        );
        assert_eq!(
            centred_scroll(Vec2::new(20.0, 590.0), viewport, content),
            Vec2::new(0.0, 400.0),
        );
    }
}
