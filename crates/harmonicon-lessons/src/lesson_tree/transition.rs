// SPDX-License-Identifier: MIT

//! Collapse, compaction, viewport, and neighboring-unit transitions.

use std::collections::{HashMap, HashSet};

use bevy::a11y::AccessibilityNode;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, ScrollPosition, UiTransform, Val2};
use bevy::ui_widgets::Button as WidgetButton;

use harmonicon_platform::settings::ReducedMotion;

use super::{
    ClusterMember, Endpoint, LayoutOwner, MovingEdge, UnitButton, UnitChevron, set_edge_geometry,
};

const TRANSITION_SECONDS: f32 = 0.22;

/// How far a collapse or slide transition advances this frame, as a fraction
/// of the whole. Under Reduced Motion it is the whole transition, so every
/// unit jumps to its end state on the first frame.
pub(super) fn transition_step(delta_secs: f32, reduced_motion: bool) -> f32 {
    if reduced_motion { 1.0 } else { delta_secs / TRANSITION_SECONDS }
}

#[derive(Resource, Default)]
pub(crate) struct CollapsedUnits(pub(super) HashSet<String>);

#[derive(Resource, Default)]
pub(crate) struct UnitExpansions(pub(super) HashMap<String, f32>);

#[derive(Resource, Default)]
pub(crate) struct PendingCompaction(pub(super) HashSet<String>);

/// A request to lay the tree out again in place (`relayout_tree`): set by
/// expanding a unit, by a collapse once its close animation has finished
/// ([`compact_finished_units`]), and by the header locator.
///
/// `anchor_unit` is the unit that keeps its screen position while
/// everything else makes room around it. A collapse records it at the
/// click, before the relayout is `pending`.
#[derive(Resource, Default)]
pub(crate) struct RelayoutRequest {
    pub(super) pending: bool,
    pub(super) anchor_unit: Option<String>,
}

/// Each unit's canvas x in the layout currently on screen, which a relayout
/// slides every unit away from.
#[derive(Resource, Default)]
pub(crate) struct PreviousUnitPositions(pub(super) HashMap<String, f32>);

/// The canvas's own size in logical pixels, as the current layout set it.
///
/// Read instead of the scroll area's measured content size, which lags a
/// frame behind a relayout: layout only runs after the frame's systems.
#[derive(Resource, Default)]
pub(crate) struct CanvasSize(pub(super) Vec2);

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
    canvas: Res<CanvasSize>,
    mut scroller: Query<(&mut ScrollPosition, &ComputedNode), With<LessonTreeScroller>>,
) {
    let Some(target) = pending.canvas_position else {
        return;
    };
    let Some((mut position, computed)) = scroller.iter_mut().next() else {
        return;
    };
    let viewport = computed.size() * computed.inverse_scale_factor;
    if viewport.min_element() <= 0.0 || canvas.0.min_element() <= 0.0 {
        return;
    }

    position.0 = centred_scroll(target, viewport, canvas.0);
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
    if saved.0 != position.0 {
        saved.0 = position.0;
    }
}

/// The slides that carry every unit from where it is drawn now to its new
/// layout position, in screen space: each unit starts exactly where it was
/// on screen, whatever the relayout did to both its canvas x and the scroll.
///
/// A unit still mid-slide starts from its current drawn offset, so a rapid
/// second toggle continues from where the first left things. A unit that
/// would not move on screen gets no slide.
pub(super) fn screen_space_slides(
    old: &HashMap<String, f32>,
    old_slides: &HashMap<String, UnitSlide>,
    old_scroll: f32,
    new: &HashMap<String, f32>,
    new_scroll: f32,
) -> HashMap<String, UnitSlide> {
    new.iter()
        .filter_map(|(id, &new_x)| {
            let old_x = *old.get(id)?;
            let drawn = old_x + old_slides.get(id).map_or(0.0, slide_offset) - old_scroll;
            let from_px = drawn - (new_x - new_scroll);
            (from_px.abs() > 0.5).then(|| (id.clone(), UnitSlide { from_px, amount: 0.0 }))
        })
        .collect()
}

/// The horizontal scroll after a relayout: the one that keeps `anchor` where
/// it is drawn now, or failing an anchor, the current scroll clamped to the
/// new content width. `viewport_width` and `content_width` are logical
/// pixels, like the scroll itself.
pub(super) fn relayout_scroll(
    anchor: Option<&str>,
    old: &HashMap<String, f32>,
    old_slides: &HashMap<String, UnitSlide>,
    old_scroll: f32,
    new: &HashMap<String, f32>,
    viewport_width: f32,
    content_width: f32,
) -> f32 {
    let placed = anchor.and_then(|id| Some((id, *old.get(id)?, *new.get(id)?)));
    let Some((id, old_x, new_x)) = placed else {
        return old_scroll.clamp(0.0, (content_width - viewport_width).max(0.0));
    };
    let drawn = old_x + old_slides.get(id).map_or(0.0, slide_offset) - old_scroll;
    anchored_scroll(new_x, drawn, viewport_width, content_width)
}

/// The scroll that keeps a unit at `screen_x` once it sits at `canvas_x`,
/// clamped to what the content can scroll.
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
    reduced_motion: Res<ReducedMotion>,
    collapsed: Res<CollapsedUnits>,
    mut expansions: ResMut<UnitExpansions>,
    mut members: Query<(&ClusterMember, &mut UiTransform, &mut Visibility)>,
    mut chevrons: Query<(&UnitChevron, &mut Text)>,
    mut unit_buttons: Query<(&UnitButton, &mut AccessibilityNode)>,
    mut lesson_buttons: Query<
        (Entity, &ClusterMember, &mut TabIndex, Has<InteractionDisabled>),
        With<WidgetButton>,
    >,
) {
    // Runs every frame on the tree, so every write below is guarded: once
    // no unit is mid-transition, an idle tree touches nothing (unguarded, it
    // re-ran transform and visibility propagation over every node).
    let step = transition_step(time.delta_secs(), reduced_motion.0);
    let moving = expansions
        .0
        .iter()
        .any(|(id, &amount)| expansion_after(amount, collapsed.0.contains(id), step) != amount);
    if moving {
        for (id, amount) in &mut expansions.0 {
            *amount = expansion_after(*amount, collapsed.0.contains(id), step);
        }
    }

    for (member, mut transform, mut visibility) in &mut members {
        let amount = expansions.0.get(&member.0).copied().unwrap_or(1.0);
        let eased = amount * amount * (3.0 - 2.0 * amount);
        let scale = Vec2::splat(eased.max(0.001));
        if transform.scale != scale {
            transform.scale = scale;
        }
        let wanted = if amount <= 0.0 { Visibility::Hidden } else { Visibility::Visible };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }

    for (chevron, mut text) in &mut chevrons {
        let label = if collapsed.0.contains(&chevron.0) { "▶" } else { "▼" };
        if text.0 != label {
            text.0.clear();
            text.0.push_str(label);
        }
    }
    for (button, mut accessibility) in &mut unit_buttons {
        let expanded = !collapsed.0.contains(&button.0);
        if accessibility.is_expanded() != Some(expanded) {
            accessibility.set_expanded(expanded);
        }
    }
    for (entity, member, mut tab_index, disabled) in &mut lesson_buttons {
        let closing = collapsed.0.contains(&member.0);
        let wanted = if closing { -1 } else { 0 };
        if tab_index.0 != wanted {
            tab_index.0 = wanted;
        }
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
    reduced_motion: Res<ReducedMotion>,
    mut slides: ResMut<UnitSlides>,
    mut owned: Query<(&LayoutOwner, &mut UiTransform), Without<MovingEdge>>,
    mut edges: Query<(&MovingEdge, &mut Node)>,
) {
    // A relayout replaces the slides, possibly with none — a unit left
    // mid-slide may already be where the new layout wants it. One pass on
    // that frame still resets every offset and places fresh edges.
    if slides.0.is_empty() && !slides.is_changed() {
        return;
    }
    let step = transition_step(time.delta_secs(), reduced_motion.0);
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
        let from = Endpoint { centre: edge.from.centre + Vec2::X * from_offset, ..edge.from };
        let to = Endpoint { centre: edge.to.centre + Vec2::X * to_offset, ..edge.to };
        set_edge_geometry(&mut node, from, to, edge.thickness);
    }
    slides.0.retain(|_, slide| slide.amount < 1.0);
}

/// Once every closing unit has finished its close animation, lays the tree
/// out again so the collapsed clusters give their columns back.
pub(crate) fn compact_finished_units(
    expansions: Res<UnitExpansions>,
    mut pending: ResMut<PendingCompaction>,
    mut relayout: ResMut<RelayoutRequest>,
) {
    if pending.0.is_empty() {
        return;
    }
    let all_closed =
        pending.0.iter().all(|id| expansions.0.get(id).is_none_or(|amount| *amount <= 0.0));
    if all_closed {
        pending.0.clear();
        relayout.pending = true;
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
mod motion_tests {
    use super::*;

    /// One frame of the two transition systems, with a unit collapsing and
    /// another sliding. `Time`'s default delta is zero, so only Reduced
    /// Motion can move anything.
    fn one_frame(reduced_motion: bool) -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .insert_resource(ReducedMotion(reduced_motion))
            .insert_resource(CollapsedUnits(HashSet::from(["closing".to_string()])))
            .insert_resource(UnitExpansions(HashMap::from([("closing".to_string(), 1.0)])))
            .insert_resource(UnitSlides(HashMap::from([(
                "sliding".to_string(),
                UnitSlide { from_px: 120.0, amount: 0.0 },
            )])))
            .add_systems(Update, (animate_unit_expansion, animate_unit_slides));
        app.update();
        app
    }

    #[test]
    fn reduced_motion_jumps_every_transition_to_its_end_on_the_first_frame() {
        let app = one_frame(true);
        assert_eq!(app.world().resource::<UnitExpansions>().0["closing"], 0.0);
        assert!(app.world().resource::<UnitSlides>().0.is_empty(), "a finished slide is dropped");
    }

    #[test]
    fn an_emptied_slide_set_still_clears_stale_offsets() {
        // A relayout can leave a unit exactly where it was drawn mid-slide,
        // so it gets no new slide; its old offset must not linger.
        let mut app = App::new();
        app.init_resource::<Time>()
            .insert_resource(ReducedMotion(false))
            .init_resource::<UnitSlides>()
            .add_systems(Update, animate_unit_slides);
        let node = app
            .world_mut()
            .spawn((
                LayoutOwner("unit".to_string()),
                UiTransform { translation: Val2::px(50.0, 0.0), ..default() },
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<UiTransform>(node).unwrap().translation, Val2::px(0.0, 0.0));
    }

    /// Where a unit is drawn on screen: its canvas position plus its slide
    /// offset, minus the scroll.
    fn screen_x(canvas_x: f32, slide: Option<&UnitSlide>, scroll: f32) -> f32 {
        canvas_x + slide.map_or(0.0, slide_offset) - scroll
    }

    fn positions(units: &[(&str, f32)]) -> HashMap<String, f32> {
        units.iter().map(|(id, x)| (id.to_string(), *x)).collect()
    }

    #[test]
    fn a_relayout_starts_every_unit_where_it_was_drawn() {
        // Expanding `toggled` widens its cluster: it moves 100 px right in
        // canvas coordinates, `right` 200 px, and `left` not at all, and the
        // anchor scrolls 100 px so `toggled` keeps its screen spot.
        let old = positions(&[("left", 200.0), ("toggled", 500.0), ("right", 900.0)]);
        let new = positions(&[("left", 200.0), ("toggled", 600.0), ("right", 1_100.0)]);
        let (old_scroll, new_scroll) = (300.0, 400.0);

        let slides = screen_space_slides(&old, &HashMap::new(), old_scroll, &new, new_scroll);

        for (id, &after) in &new {
            assert_eq!(
                screen_x(after, slides.get(id), new_scroll),
                screen_x(old[id], None, old_scroll),
                "{id} must start its slide where it was drawn"
            );
        }
        assert!(!slides.contains_key("toggled"), "the anchor does not move");
        assert!(
            slides.contains_key("left"),
            "a unit whose canvas x is unchanged still moves on screen"
        );
    }

    #[test]
    fn the_anchor_keeps_its_screen_spot_and_the_rest_clamps() {
        let old = positions(&[("toggled", 500.0)]);
        let new = positions(&[("toggled", 600.0)]);
        let none = HashMap::new();
        // Drawn at 200 on screen before; scroll 400 keeps it there.
        assert_eq!(
            relayout_scroll(Some("toggled"), &old, &none, 300.0, &new, 800.0, 2_000.0),
            400.0,
        );
        // No anchor: the scroll stays put, within the narrower content.
        assert_eq!(relayout_scroll(None, &old, &none, 900.0, &new, 800.0, 1_200.0), 400.0,);
    }

    #[test]
    fn a_relayout_mid_slide_continues_from_the_drawn_position() {
        // A second toggle lands while `unit` is halfway through sliding in
        // from 200 px to the right: it must start from there, not snap.
        let old = positions(&[("unit", 500.0)]);
        let halfway =
            HashMap::from([("unit".to_string(), UnitSlide { from_px: 200.0, amount: 0.5 })]);
        let new = positions(&[("unit", 700.0)]);

        let slides = screen_space_slides(&old, &halfway, 0.0, &new, 0.0);

        assert_eq!(
            screen_x(700.0, slides.get("unit"), 0.0),
            screen_x(500.0, halfway.get("unit"), 0.0),
        );
    }

    #[test]
    fn without_reduced_motion_transitions_advance_with_time() {
        let app = one_frame(false);
        assert_eq!(app.world().resource::<UnitExpansions>().0["closing"], 1.0);
        assert_eq!(app.world().resource::<UnitSlides>().0["sliding"].amount, 0.0);
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    #[test]
    fn viewport_position_survives_after_the_scroller_is_gone() {
        let mut app = App::new();
        app.init_resource::<LessonTreeViewport>().add_systems(Update, remember_viewport);
        let scroller = app
            .world_mut()
            .spawn((LessonTreeScroller, ScrollPosition(Vec2::new(420.0, 180.0))))
            .id();

        app.update();
        app.world_mut().despawn(scroller);
        app.update();

        assert_eq!(app.world().resource::<LessonTreeViewport>().0, Vec2::new(420.0, 180.0),);
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
