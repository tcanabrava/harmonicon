// SPDX-License-Identifier: MIT

//! Where each part of the tree goes, and moving it there again after a unit
//! toggle without rebuilding the page.
//!
//! A toggle changes where things sit, never which things exist: the layout
//! keeps a collapsed unit's lessons, stacked on its spot. So every unit and
//! lesson part is spawned once, tagged with a [`TreePart`], and
//! [`relayout_tree`] moves the tagged entities. The scroll area, its
//! scrollbars and keyboard focus survive a toggle; only the edges are
//! recreated, since a collapsed cluster has none.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition};

use harmonicon_app::profile::PlayerProfile;
use harmonicon_song::lessons::graph::LessonGraph;
use harmonicon_song::lessons::units::UnitChain;
use harmonicon_song::lessons::{AvailableLessons, LessonManifest};

use super::edges::{EdgeStyle, Endpoint, LessonEdgeMaterial, spawn_edge};
use super::layout::{EdgeKind, TreeLayout, layout_with_collapsed};
use super::transition::{
    CanvasSize, CollapsedUnits, LessonTreeScroller, PendingCompaction, PendingLessonFocus,
    PreviousUnitPositions, RelayoutRequest, UnitSlides, relayout_scroll, screen_space_slides,
};
use super::{
    BADGE_GAP_PX, BADGE_PX, COL_PX, EDGE_COLOR, EDGE_PX, LABEL_PX, MARGIN_PX, NODE_PX,
    OPTIONAL_COLOR, PIP_PX, ROW_PX, SPINE_COLOR, SPINE_LABEL_PX, UNIT_EDGE_PX, UNIT_LABEL_PX,
    UNIT_PX, node_centre,
};

/// One positioned piece of the tree, and which layout entry places it.
#[derive(Component, Clone, Debug, PartialEq)]
pub(crate) struct TreePart {
    id: String,
    kind: PartKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PartKind {
    UnitButton,
    UnitLabel,
    LessonButton,
    LessonLabel,
    Badge,
    /// The review-due mark, on the side opposite [`PartKind::Badge`].
    DueBadge,
    /// A mastery pip, by training tier.
    Pip(usize),
}

impl PartKind {
    /// Whether the part's id names a unit rather than a lesson.
    fn is_unit(self) -> bool {
        matches!(self, PartKind::UnitButton | PartKind::UnitLabel)
    }

    /// Where this part's node starts, for an entry centred at `centre`. The
    /// one definition of each part's placement: spawning and relayout both
    /// read it, so a moved part cannot land somewhere a fresh build would
    /// not have put it.
    pub(super) fn top_left(self, centre: Vec2) -> Vec2 {
        match self {
            PartKind::UnitButton => centre - Vec2::splat(UNIT_PX / 2.0),
            PartKind::UnitLabel => {
                Vec2::new(centre.x - UNIT_LABEL_PX / 2.0, centre.y - UNIT_PX / 2.0 - 34.0)
            }
            PartKind::LessonButton => centre - Vec2::splat(NODE_PX / 2.0),
            PartKind::LessonLabel => {
                Vec2::new(centre.x - LABEL_PX / 2.0, centre.y + NODE_PX / 2.0 + 6.0)
            }
            PartKind::Badge => {
                Vec2::new(centre.x + NODE_PX / 2.0 + BADGE_GAP_PX, centre.y - BADGE_PX / 2.0)
            }
            PartKind::DueBadge => Vec2::new(
                centre.x - NODE_PX / 2.0 - BADGE_GAP_PX - BADGE_PX,
                centre.y - BADGE_PX / 2.0,
            ),
            PartKind::Pip(tier) => {
                let tiers = harmonicon_core::training::Tier::ALL.len();
                let t = tier as f32 / (tiers - 1) as f32;
                // Negative sweeps the arc upward: screen y grows downward.
                let angle = -(42.0 + t * 96.0_f32).to_radians();
                let radius = NODE_PX / 2.0 + 1.0;
                centre + radius * Vec2::new(angle.cos(), angle.sin()) - Vec2::splat(PIP_PX / 2.0)
            }
        }
    }

    /// The tag for this part of entry `id`.
    pub(super) fn part(self, id: &str) -> TreePart {
        TreePart { id: id.to_string(), kind: self }
    }
}

/// The canvas every node is positioned in.
#[derive(Component, Default, Clone)]
pub(crate) struct LessonTreeCanvas;

/// The canvas's first child, holding every edge. Edges are the one thing a
/// toggle does create and destroy, and new children draw on top of old ones
/// — so they live in a layer that already sits beneath every node.
#[derive(Component, Default, Clone)]
pub(crate) struct EdgeLayer;

/// Lays the tree out, with every unit in `collapsed` compacted.
pub(super) fn build_layout(
    lessons: &AvailableLessons,
    profile: &PlayerProfile,
    collapsed: &HashSet<String>,
) -> Result<TreeLayout, String> {
    let manifests: Vec<&LessonManifest> = lessons.0.iter().map(|e| &e.manifest).collect();
    let chain = UnitChain::build(&manifests);
    let graph = LessonGraph::build(&manifests).map_err(|e| e.to_string())?;
    Ok(layout_with_collapsed(&lessons.0, &graph, &chain, profile, collapsed))
}

/// The units the layout should compact: those collapsed, less any still
/// playing their close animation. Compacting one of those early would snap
/// its shrinking lessons to the unit's spot mid-animation; it compacts once
/// [`compact_finished_units`](super::transition::compact_finished_units)
/// sees it closed.
pub(super) fn laid_out_collapsed(
    collapsed: &CollapsedUnits,
    compacting: &PendingCompaction,
) -> HashSet<String> {
    collapsed.0.difference(&compacting.0).cloned().collect()
}

/// The canvas size a layout needs, in logical pixels.
pub(super) fn tree_canvas_size(tree: &TreeLayout) -> Vec2 {
    Vec2::new(
        MARGIN_PX * 2.0 + tree.columns() * COL_PX,
        MARGIN_PX * 2.0 + SPINE_LABEL_PX + tree.rows() * ROW_PX,
    )
}

/// Each unit's canvas x.
pub(super) fn unit_positions(tree: &TreeLayout) -> HashMap<String, f32> {
    tree.units.iter().map(|unit| (unit.id.clone(), node_centre(unit.column, unit.row).x)).collect()
}

/// Lays the tree out again after a toggle and moves what is already on
/// screen, instead of rebuilding the page.
///
/// The scroll area survives, so the anchored unit's scroll and every unit's
/// slide are settled together, in screen space, on this frame: each unit
/// starts exactly where it is drawn and glides to its new place, while the
/// anchored one stays put.
pub(crate) fn relayout_tree(
    mut commands: Commands,
    mut request: ResMut<RelayoutRequest>,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    collapsed: Res<CollapsedUnits>,
    compacting: Res<PendingCompaction>,
    mut positions: ResMut<PreviousUnitPositions>,
    mut slides: ResMut<UnitSlides>,
    mut focus: ResMut<PendingLessonFocus>,
    mut canvas_size: ResMut<CanvasSize>,
    mut materials: ResMut<Assets<LessonEdgeMaterial>>,
    mut canvas: Query<&mut Node, With<LessonTreeCanvas>>,
    edge_layer: Query<Entity, With<EdgeLayer>>,
    mut scroller: Query<(&mut ScrollPosition, &ComputedNode), With<LessonTreeScroller>>,
    mut parts: Query<(&TreePart, &mut Node), Without<LessonTreeCanvas>>,
) {
    if !request.pending {
        return;
    }
    request.pending = false;
    let anchor = request.anchor_unit.take();
    let (Ok((mut scroll, computed)), Ok(layer), Ok(mut canvas)) =
        (scroller.single_mut(), edge_layer.single(), canvas.single_mut())
    else {
        return;
    };
    // The graph was valid when the page was built, and a toggle doesn't
    // change it; a rescan that breaks it rebuilds the page instead.
    let Ok(tree) = build_layout(&lessons, &profile, &laid_out_collapsed(&collapsed, &compacting))
    else {
        return;
    };

    let size = tree_canvas_size(&tree);
    canvas.width = Val::Px(size.x);
    canvas.height = Val::Px(size.y);
    canvas_size.0 = size;

    let new_positions = unit_positions(&tree);
    let old_scroll = scroll.x;
    let new_scroll = relayout_scroll(
        anchor.as_deref(),
        &positions.0,
        &slides.0,
        old_scroll,
        &new_positions,
        computed.size().x * computed.inverse_scale_factor,
        size.x,
    );
    slides.0 = screen_space_slides(&positions.0, &slides.0, old_scroll, &new_positions, new_scroll);
    positions.0 = new_positions;
    scroll.x = new_scroll;

    let unit_centres: HashMap<&str, Vec2> = tree
        .units
        .iter()
        .map(|unit| (unit.id.as_str(), node_centre(unit.column, unit.row)))
        .collect();
    let lesson_centres: HashMap<&str, Vec2> = tree
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node_centre(node.column, node.row)))
        .collect();
    for (part, mut node) in &mut parts {
        let centres = if part.kind.is_unit() { &unit_centres } else { &lesson_centres };
        let Some(&centre) = centres.get(part.id.as_str()) else {
            continue;
        };
        let top_left = part.kind.top_left(centre);
        let (left, top) = (Val::Px(top_left.x), Val::Px(top_left.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }

    commands.entity(layer).despawn_related::<Children>();
    spawn_edges(&mut commands, layer, &tree, &mut materials);

    if let Some(id) = focus.lesson_id.as_deref() {
        focus.canvas_position = lesson_centres.get(id).copied();
        if focus.canvas_position.is_none() {
            focus.lesson_id = None;
        }
    }
}

/// Every edge of `tree`, as children of the edge layer.
pub(super) fn spawn_edges(
    commands: &mut Commands,
    layer: Entity,
    tree: &TreeLayout,
    materials: &mut Assets<LessonEdgeMaterial>,
) {
    let mut edge_materials = Vec::new();
    commands.entity(layer).with_children(|parent| {
        for edge in &tree.edges {
            // `EdgeKind` names which sort of node sits at each end, which is
            // what decides where the line has to stop — a unit's ring is
            // `UNIT_PX` across, a lesson's `NODE_PX`.
            let (from_radius, to_radius, thickness, color) = match edge.kind {
                EdgeKind::Spine => (UNIT_PX / 2.0, UNIT_PX / 2.0, UNIT_EDGE_PX, SPINE_COLOR),
                EdgeKind::UnitBranch => (UNIT_PX / 2.0, NODE_PX / 2.0, EDGE_PX, EDGE_COLOR),
                EdgeKind::Branch => (NODE_PX / 2.0, NODE_PX / 2.0, EDGE_PX, EDGE_COLOR),
            };
            // An elective branch is dotted and takes the badge's own
            // colour, so the line peeling off and the node it arrives at
            // say the same thing. Hue rather than dimming: dim already
            // means locked, and an elective is perfectly playable.
            let style = EdgeStyle {
                thickness,
                color: if edge.optional {
                    OPTIONAL_COLOR.with_alpha(EDGE_COLOR.alpha())
                } else {
                    color
                },
                dotted: edge.optional,
            };
            let owners = match edge.kind {
                EdgeKind::Spine => tree
                    .units
                    .iter()
                    .find(|unit| (unit.column, unit.row) == edge.from)
                    .zip(tree.units.iter().find(|unit| (unit.column, unit.row) == edge.to))
                    .map(|(from, to)| (from.id.as_str(), to.id.as_str())),
                EdgeKind::UnitBranch | EdgeKind::Branch => {
                    edge.unit_id.as_deref().map(|unit| (unit, unit))
                }
            };
            spawn_edge(
                parent,
                Endpoint { centre: node_centre(edge.from.0, edge.from.1), radius: from_radius },
                Endpoint { centre: node_centre(edge.to.0, edge.to.1), radius: to_radius },
                style,
                edge.unit_id.as_deref(),
                owners,
                materials,
                &mut edge_materials,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_song::lessons::LessonEntry;

    fn lesson(id: &str, unit: &str) -> LessonEntry {
        LessonEntry {
            manifest: LessonManifest {
                track: Some("bend".to_string()),
                ..LessonManifest::new(id, unit)
            },
            chart_asset_path: None,
        }
    }

    fn lesson_centre(tree: &TreeLayout, id: &str) -> Vec2 {
        let node = tree.nodes.iter().find(|node| node.id == id).unwrap();
        node_centre(node.column, node.row)
    }

    #[test]
    fn collapsing_a_unit_moves_the_existing_entities_instead_of_rebuilding() {
        // Unit `wide` holds two side-by-side lessons; `next` sits after it.
        // Compacting `wide` pulls `next` left.
        let lessons =
            AvailableLessons(vec![lesson("a", "wide"), lesson("b", "wide"), lesson("c", "next")]);
        let profile = PlayerProfile::default();
        let expanded = build_layout(&lessons, &profile, &HashSet::new()).unwrap();
        let compacted =
            build_layout(&lessons, &profile, &HashSet::from(["wide".to_string()])).unwrap();

        let mut world = World::new();
        world.insert_resource(lessons);
        world.insert_resource(profile);
        world.insert_resource(CollapsedUnits(HashSet::from(["wide".to_string()])));
        world.init_resource::<PendingCompaction>();
        world.insert_resource(RelayoutRequest {
            pending: true,
            anchor_unit: Some("wide".to_string()),
        });
        world.insert_resource(PreviousUnitPositions(unit_positions(&expanded)));
        world.init_resource::<UnitSlides>();
        world.init_resource::<PendingLessonFocus>();
        world.init_resource::<CanvasSize>();
        world.init_resource::<Assets<LessonEdgeMaterial>>();

        world.spawn((Node::default(), LessonTreeCanvas));
        let layer = world.spawn((Node::default(), EdgeLayer)).id();
        let stale_edge = world.spawn(ChildOf(layer)).id();
        world.spawn((LessonTreeScroller, ScrollPosition(Vec2::ZERO), ComputedNode::default()));
        let at = PartKind::LessonButton.top_left(lesson_centre(&expanded, "c"));
        let button = world
            .spawn((
                Node { left: Val::Px(at.x), top: Val::Px(at.y), ..default() },
                PartKind::LessonButton.part("c"),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(relayout_tree);
        schedule.run(&mut world);

        let want = PartKind::LessonButton.top_left(lesson_centre(&compacted, "c"));
        let node = world.get::<Node>(button).expect("the same entity survives");
        assert_eq!((node.left, node.top), (Val::Px(want.x), Val::Px(want.y)));
        assert!(want.x < at.x, "compacting the unit before it pulls the lesson left");
        assert!(world.get_entity(stale_edge).is_err(), "edges are recreated");
        assert!(
            world.resource::<UnitSlides>().0.contains_key("next"),
            "the moved unit glides rather than jumps"
        );
        assert_eq!(world.resource::<CanvasSize>().0, tree_canvas_size(&compacted));
        assert!(!world.resource::<RelayoutRequest>().pending);
    }
}
