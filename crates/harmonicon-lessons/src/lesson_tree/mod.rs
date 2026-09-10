// SPDX-License-Identifier: MIT

//! The skill tree: the curriculum, and the only view of it.
//!
//! A spine of *unit* nodes runs left to right along the top — Unit 1, Unit
//! 2, … — and each unit's own lessons hang below it as a small layered
//! graph. Colour is the track; position is the prerequisite structure. See
//! [`layout`], which decides all of it and is pure; this file only turns
//! those answers into nodes, the same split `music_score` uses.
//!
//! **Why two levels.** Drawn as one flat graph the curriculum's eighteen
//! cross-unit prerequisites became edges four and five columns long, cutting
//! through whatever nodes and labels lay between them; crossing reduction
//! can't help when the endpoints are genuinely that far apart. Grouping by
//! unit turns those eighteen into four spine edges and leaves every other
//! edge local to one cluster. A cross-unit prerequisite is no longer drawn
//! at all — [`tooltip_for`] names it on the node instead.
//!
//! **The canvas is wider than the window on purpose**, which is why this
//! page builds a two-axis `spawn_scroll_area_xy` instead of taking
//! `spawn_menu_root`'s vertical one.
//!
//! **An edge runs straight from one node's boundary to the other's**, along
//! the line joining their centres, and each end is clipped by the radius of
//! the node it actually touches — see [`edge_span`]. Units and lessons are
//! different sizes, so a single shared radius would leave a spine edge
//! starting inside a unit's ring and a branch edge stopping short of the
//! lesson it points at.
//!
//! **That takes no shader and no mesh.** A straight run is one `Node`
//! rotated by `UiTransform::rotation`, a first-class UI field in Bevy 0.19.
//! `bevy_ui` has no line primitive, but a rotated rectangle is one.
//!
//! **There is no list view any more.** This replaced it rather than sitting
//! beside it, so a lesson has one home and unit gating is stated in one
//! place. The tree is wide, and a small screen reaches the rest of it by
//! scrolling; `responsive::is_compact` no longer routes anywhere.

mod layout;

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui::UiTransform;
use bevy::ui_widgets::{Activate, Button as WidgetButton};
use bevy_fluent::Localization;

use harmonicon_app::profile::PlayerProfile;
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::lessons::graph::LessonGraph;
use harmonicon_song::lessons::units::UnitChain;
use harmonicon_song::lessons::{AvailableLessons, LessonsRescanned};
use harmonicon_ui::dialogs::tooltip::Tooltip;

use crate::lesson_reader::SelectedLesson;
use harmonicon_menu::menu::MenuPage;
use harmonicon_menu::menu::scene::{spawn_back_button, spawn_menu_root_plain};
use harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy;

use layout::{EdgeKind, NodeState, PlacedNode, PlacedUnit, layout_with_collapsed};

use std::collections::{HashMap, HashSet};

/// Node diameter.
const NODE_PX: f32 = 64.0;
/// Column pitch. Must stay wider than `LABEL_PX`, or two neighbours in the
/// same row have their titles run together — the pitch is what separates
/// the *labels*, the nodes themselves being far narrower.
const COL_PX: f32 = 150.0;
/// Row pitch. Tall enough for the node, its pips and two lines of title
/// underneath without the next row's art crowding the text.
const ROW_PX: f32 = 122.0;
/// Canvas inset. Wide enough that the *first* column's labels still fit:
/// every label is centred on its node, so the margin has to cover half the
/// widest one (`UNIT_LABEL_PX`) less half a node, or Unit 1's title runs off
/// the left edge of the canvas and is clipped.
const MARGIN_PX: f32 = 90.0;
/// Title width and size. Narrower than the column pitch so two neighbours'
/// labels can't run together, and small enough that a long title wraps to
/// two lines rather than three.
const LABEL_PX: f32 = 132.0;
/// Two neighbours in one depth row sit exactly a column apart and each
/// label is centred on its node, so the pitch is the only thing keeping
/// their titles from running together.
const _: () = assert!(COL_PX > LABEL_PX);
const LABEL_FONT_PX: f32 = 11.0;

/// Edge thickness.
const EDGE_PX: f32 = 3.5;
/// Clearance two node boundaries need before an edge between them is worth
/// drawing at all.
const EDGE_MIN_LENGTH_PX: f32 = 1.0;

const PIP_PX: f32 = 9.0;
const LOCKED_TINT: Color = Color::srgba(0.35, 0.35, 0.42, 0.55);
const PIP_FILLED: Color = Color::srgb(0.95, 0.80, 0.35);
const PIP_EMPTY: Color = Color::srgba(0.40, 0.43, 0.52, 0.8);
const EDGE_COLOR: Color = Color::srgba(0.62, 0.66, 0.78, 0.5);

/// A unit node, drawn larger than a lesson because it is the level a player
/// navigates by.
const UNIT_PX: f32 = 92.0;
const UNIT_EDGE_PX: f32 = 6.0;
const UNIT_LABEL_PX: f32 = 176.0;
/// Vertical room reserved above the spine for the unit titles.
const SPINE_LABEL_PX: f32 = 44.0;
const UNIT_FONT_PX: f32 = 14.0;
const UNIT_OPEN: Color = Color::srgb(0.95, 0.82, 0.45);
const UNIT_SHUT: Color = Color::srgba(0.55, 0.57, 0.66, 0.55);
/// The spine reads as the backbone it is: brighter and thicker than the
/// branch edges, so the eye follows unit to unit before dropping into any
/// one cluster.
const SPINE_COLOR: Color = Color::srgba(0.95, 0.82, 0.45, 0.65);
/// Matching `menu::scene`'s own scrollbar colours — this page builds its
/// scroll area itself, so it can't inherit them.
const SCROLLBAR_TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);
const SCROLLBAR_THUMB: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);

const COLLAPSE_SECONDS: f32 = 0.22;

/// Units the player explicitly collapsed. Kept across page visits so opening
/// a lesson and returning does not discard the course-map view they chose.
#[derive(Resource, Default)]
pub(crate) struct CollapsedUnits(HashSet<String>);

/// Current animation amount per unit: 0 is collapsed, 1 is expanded.
#[derive(Resource, Default)]
pub(crate) struct UnitExpansions(HashMap<String, f32>);

/// Units whose close animation must finish before the layout reclaims their
/// columns. Tracking ids keeps rapid toggles independent.
#[derive(Resource, Default)]
pub(crate) struct PendingCompaction(HashSet<String>);

/// Any lesson node, label, mastery pip, or branch edge owned by a unit.
#[derive(Component)]
pub(crate) struct ClusterMember(String);

#[derive(Component)]
pub(crate) struct UnitChevron(String);

/// A track's colour. Grouping has to survive losing its row, and colour is
/// what the skill trees this is modelled on use for the same job.
fn track_color(track: &str) -> Color {
    match track {
        "tone" => Color::srgb(0.42, 0.78, 0.95),
        "hand" => Color::srgb(0.95, 0.62, 0.42),
        "tongue" => Color::srgb(0.72, 0.55, 0.95),
        "bend" => Color::srgb(0.95, 0.45, 0.52),
        "vibrato" => Color::srgb(0.95, 0.80, 0.42),
        "slide" => Color::srgb(0.55, 0.88, 0.72),
        "time" => Color::srgb(0.52, 0.70, 0.95),
        "form" => Color::srgb(0.88, 0.72, 0.45),
        "train" => Color::srgb(0.68, 0.78, 0.52),
        "scales" => Color::srgb(0.45, 0.85, 0.62),
        "theory" => Color::srgb(0.78, 0.68, 0.92),
        "harmony" => Color::srgb(0.92, 0.58, 0.78),
        "vocabulary" => Color::srgb(0.85, 0.65, 0.55),
        "improv" => Color::srgb(0.58, 0.82, 0.88),
        _ => Color::srgb(0.65, 0.68, 0.78),
    }
}

/// Centre of a node, in canvas pixels.
///
/// The vertical inset is what leaves room for the unit titles, which sit
/// *above* their nodes on row 0 and would otherwise be cut off by the top
/// of the canvas — a lesson's title hangs below it, so nothing else needs
/// the space.
fn node_centre(column: f32, row: f32) -> Vec2 {
    Vec2::new(
        MARGIN_PX + column * COL_PX + NODE_PX / 2.0,
        MARGIN_PX + SPINE_LABEL_PX + row * ROW_PX + NODE_PX / 2.0,
    )
}

pub(crate) fn setup_lesson_tree(
    mut commands: Commands,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    collapsed: Res<CollapsedUnits>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    asset_server: Res<AssetServer>,
) {
    // `_plain` plus a two-axis scroll area of our own: the shared
    // `spawn_menu_root` scrolls vertically only, and this canvas outgrows
    // any window in both directions once every unit's cluster is laid out
    // beside the last.
    let (root, header, _page_root) = spawn_menu_root_plain(
        &mut commands,
        &loc.msg("lesson-tree-title"),
        None,
        &theme,
        "LessonTree",
    );

    let manifests: Vec<_> = lessons.0.iter().map(|e| e.manifest.clone()).collect();
    let chain = UnitChain::build(&manifests);
    let tree = match LessonGraph::build(&manifests) {
        Ok(graph) => layout_with_collapsed(&lessons.0, &graph, &chain, &profile, &collapsed.0),
        // A cycle or a dangling prerequisite. `tests/asset_layout.rs` fails
        // the build over either, so this only fires for a lesson dropped
        // into `~/Harmonicon/lessons` — say so rather than draw nothing.
        Err(e) => {
            let line = commands
                .spawn((
                    Text::new(String::from(
                        loc.msg_args("lesson-tree-broken", &[("error", e.to_string())]),
                    )),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.65, 0.45)),
                    Node {
                        max_width: Val::Px(560.0),
                        ..default()
                    },
                ))
                .id();
            commands.entity(root).add_child(line);
            spawn_back_button(&mut commands, header, &loc.msg("back"), back_to_play);
            return;
        }
    };

    let placeholder: Handle<Image> = asset_server.load("icons/lesson_placeholder.png");

    // A unit discovered while the app is running starts expanded. Existing
    // animation values survive page rebuilds and visits to the reader.
    let unit_ids: Vec<String> = tree.units.iter().map(|unit| unit.id.clone()).collect();
    commands.queue(move |world: &mut World| {
        let collapsed = world.resource::<CollapsedUnits>();
        let initial: Vec<(String, f32)> = unit_ids
            .iter()
            .map(|id| (id.clone(), if collapsed.0.contains(id) { 0.0 } else { 1.0 }))
            .collect();
        let mut expansions = world.resource_mut::<UnitExpansions>();
        for (id, value) in initial {
            expansions.0.entry(id).or_insert(value);
        }
    });

    // The plain root's content column sizes to its own content, so a canvas
    // larger than the window would push it past both screen edges and the
    // scroll area inside would never receive less room than the tree asks
    // for — the "min-height: auto" gotcha `scroll_area` documents, one level
    // further out. Nothing else shares this column, so reshape it here
    // rather than change what every plain page gets.
    commands.entity(root).insert(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        width: Val::Percent(100.0),
        min_height: Val::Px(0.0),
        min_width: Val::Px(0.0),
        flex_grow: 1.0,
        ..default()
    });

    let mut scroller = Entity::PLACEHOLDER;
    commands.entity(root).with_children(|parent| {
        scroller = spawn_scroll_area_xy(parent, SCROLLBAR_THUMB, SCROLLBAR_TRACK);
    });

    let canvas = commands
        .spawn(Node {
            position_type: PositionType::Relative,
            width: Val::Px(MARGIN_PX * 2.0 + tree.columns() * COL_PX),
            height: Val::Px(MARGIN_PX * 2.0 + SPINE_LABEL_PX + tree.rows() * ROW_PX),
            // Every node is positioned absolutely inside this box, so it
            // has to keep the height it asks for. Left to shrink — the
            // flexbox default inside the scroll column — the box collapses
            // to the viewport while its children keep their pixel offsets,
            // and the scroll extent is computed from the collapsed box: the
            // tree spills past both ends and neither can be scrolled to.
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    commands.entity(scroller).add_child(canvas);

    // Edges first, so node art always sits on top of its connectors.
    commands.entity(canvas).with_children(|parent| {
        for edge in &tree.edges {
            // `EdgeKind` names which sort of node sits at each end, which is
            // what decides where the line has to stop — a unit's ring is
            // `UNIT_PX` across, a lesson's `NODE_PX`.
            let (from_radius, to_radius, thickness, color) = match edge.kind {
                EdgeKind::Spine => (UNIT_PX / 2.0, UNIT_PX / 2.0, UNIT_EDGE_PX, SPINE_COLOR),
                EdgeKind::UnitBranch => (UNIT_PX / 2.0, NODE_PX / 2.0, EDGE_PX, EDGE_COLOR),
                EdgeKind::Branch => (NODE_PX / 2.0, NODE_PX / 2.0, EDGE_PX, EDGE_COLOR),
            };
            spawn_edge(
                parent,
                Endpoint {
                    centre: node_centre(edge.from.0, edge.from.1),
                    radius: from_radius,
                },
                Endpoint {
                    centre: node_centre(edge.to.0, edge.to.1),
                    radius: to_radius,
                },
                thickness,
                color,
                edge.unit_id.as_deref(),
            );
        }
    });

    for unit in &tree.units {
        spawn_unit(&mut commands, canvas, unit, &loc);
    }
    for node in &tree.nodes {
        spawn_node(&mut commands, canvas, node, &placeholder, &loc);
    }

    spawn_back_button(&mut commands, header, &loc.msg("back"), back_to_play);
}

fn back_to_play(_: On<Activate>, mut page: ResMut<NextState<MenuPage>>) {
    page.set(MenuPage::Play);
}

/// A lesson dropped into `~/Harmonicon/lessons` while this page is open
/// changes the graph under it, so force a same-page rebuild —
/// `NextState::set` re-fires `OnExit`/`OnEnter` even for a same-state
/// transition. Message-driven rather than gated on
/// `AvailableLessons::is_changed()`, for the staleness reason
/// [`LessonsRescanned`] documents.
pub(crate) fn rebuild_on_lessons_rescanned(
    mut rescanned: MessageReader<LessonsRescanned>,
    mut page: ResMut<NextState<MenuPage>>,
) {
    if rescanned.read().next().is_some() {
        page.set(MenuPage::LessonTree);
    }
}

/// One end of an edge: where a node sits, and how far its art reaches from
/// that centre.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Endpoint {
    centre: Vec2,
    radius: f32,
}

/// The visible run of an edge: the segment of the line joining two centres
/// that lies *between* the two nodes' boundaries.
///
/// Both ends are pulled back along that same line, each by its own radius,
/// so the edge points at what it connects from wherever that happens to be
/// — below, above, or off to one side. `None` when the boundaries already
/// meet and there is nothing left to draw.
fn edge_span(from: Endpoint, to: Endpoint) -> Option<(Vec2, Vec2)> {
    let offset = to.centre - from.centre;
    let gap = offset.length();
    // Tested against the gap rather than the resulting segment's length:
    // two nodes nearer than their combined radii would pull each end past
    // the other, and a backwards segment has a perfectly respectable
    // positive length while running through both nodes it claims to join.
    // This also covers two centres landing on the same point.
    if gap < from.radius + to.radius + EDGE_MIN_LENGTH_PX {
        return None;
    }
    let direction = offset / gap;
    Some((
        from.centre + direction * from.radius,
        to.centre - direction * to.radius,
    ))
}

/// One edge, as a single rotated rectangle.
///
/// A straight run needs no sampling and no joins, so there is exactly one
/// node per edge and no seam anywhere along it.
fn spawn_edge(
    parent: &mut ChildSpawnerCommands,
    from: Endpoint,
    to: Endpoint,
    thickness: f32,
    color: Color,
    unit_id: Option<&str>,
) {
    let Some((start, end)) = edge_span(from, to) else {
        return;
    };
    let delta = end - start;
    let length = delta.length();
    let mid = (start + end) / 2.0;

    let mut segment = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            // Positioned by its own top-left, so shift back by half its
            // extent to centre it on the midpoint before rotating —
            // `UiTransform::rotation` turns a node about its centre.
            left: Val::Px(mid.x - length / 2.0),
            top: Val::Px(mid.y - thickness / 2.0),
            width: Val::Px(length),
            height: Val::Px(thickness),
            // Rounds the two ends, which softens where a thick spine edge
            // meets a ring.
            border_radius: BorderRadius::MAX,
            ..default()
        },
        UiTransform {
            rotation: Rot2::radians(delta.y.atan2(delta.x)),
            ..default()
        },
        BackgroundColor(color),
    ));
    if let Some(unit_id) = unit_id {
        segment.insert(ClusterMember(unit_id.to_string()));
    }
}

/// A unit: the major node the whole cluster below it hangs off.
///
/// Activating it expands or collapses the lessons belonging to the unit.
fn spawn_unit(commands: &mut Commands, canvas: Entity, unit: &PlacedUnit, loc: &Localization) {
    let centre = node_centre(unit.column, unit.row);
    let ring = if unit.locked { UNIT_SHUT } else { UNIT_OPEN };
    let unit_id = unit.id.clone();

    // not-a-widget-button: a unit is a label and a gate, not an action.
    let node = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x - UNIT_PX / 2.0),
                top: Val::Px(centre.y - UNIT_PX / 2.0),
                width: Val::Px(UNIT_PX),
                height: Val::Px(UNIT_PX),
                border: UiRect::all(Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(ring),
            BackgroundColor(Color::srgba(0.10, 0.11, 0.16, 0.85)),
            WidgetButton,
            TabIndex(0),
        ))
        .observe(
            move |_: On<Activate>,
                  mut collapsed: ResMut<CollapsedUnits>,
                  mut pending: ResMut<PendingCompaction>,
                  mut page: ResMut<NextState<MenuPage>>| {
                if collapsed.0.remove(&unit_id) {
                    pending.0.remove(&unit_id);
                    // Expand from the compact layout first; the existing zero
                    // expansion amount then animates the newly placed cluster.
                    page.set(MenuPage::LessonTree);
                } else {
                    collapsed.0.insert(unit_id.clone());
                    pending.0.insert(unit_id.clone());
                }
            },
        )
        .id();
    commands.entity(canvas).add_child(node);

    let progress = commands
        .spawn((
            Text::new(String::from(loc.msg_args(
                "lesson-tree-unit-progress",
                &[
                    ("done", unit.completed.to_string()),
                    ("needed", unit.required.to_string()),
                ],
            ))),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(ring),
        ))
        .id();
    commands.entity(node).add_child(progress);

    let chevron = commands
        .spawn((
            Text::new("▼"),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ring),
            UnitChevron(unit.id.clone()),
        ))
        .id();
    commands.entity(node).add_child(chevron);

    let label = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x - UNIT_LABEL_PX / 2.0),
                top: Val::Px(centre.y - UNIT_PX / 2.0 - 34.0),
                width: Val::Px(UNIT_LABEL_PX),
                ..default()
            },
            Text::new(String::from(loc.msg(&unit.title_key))),
            TextFont {
                font_size: FontSize::Px(UNIT_FONT_PX),
                ..default()
            },
            TextLayout::justify(Justify::Center),
            TextColor(ring),
        ))
        .id();
    commands.entity(canvas).add_child(label);
}

/// Track, title, and — for a locked lesson — what it is still waiting on.
///
/// That last part is load-bearing: a prerequisite in another unit is no
/// longer drawn as an edge (see [`layout`]'s header), so this is where a
/// player finds out why a node is dark. Naming them beats a line across
/// the screen even when there *is* one to follow.
fn tooltip_for(node: &PlacedNode, loc: &Localization) -> String {
    let head = format!(
        "{} · {}",
        loc.msg(&format!("lesson-track-{}", node.track)),
        loc.msg(&node.title_key)
    );
    if node.state != NodeState::Locked || node.unmet.is_empty() {
        return head;
    }
    let wants: Vec<String> = node
        .unmet
        .iter()
        .map(|key| String::from(loc.msg(key)))
        .collect();
    format!(
        "{head}\n{}",
        loc.msg_args("lesson-tree-needs", &[("lessons", wants.join(", "))])
    )
}

fn spawn_node(
    commands: &mut Commands,
    canvas: Entity,
    node: &PlacedNode,
    placeholder: &Handle<Image>,
    loc: &Localization,
) {
    let centre = node_centre(node.column, node.row);
    let locked = node.state == NodeState::Locked;
    let id = node.id.clone();

    // The track's colour, dimmed while locked and brightened once passed,
    // so state reads without giving up the grouping colour carries.
    let base = track_color(&node.track);
    let ring = match node.state {
        NodeState::Locked => base.with_alpha(0.35),
        NodeState::Available => base,
        NodeState::Passed | NodeState::Mastered => base.lighter(0.15),
    };

    let button = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x - NODE_PX / 2.0),
                top: Val::Px(centre.y - NODE_PX / 2.0),
                width: Val::Px(NODE_PX),
                height: Val::Px(NODE_PX),
                border: UiRect::all(Val::Px(if node.state == NodeState::Mastered {
                    4.0
                } else {
                    3.0
                })),
                // A square node with a maximal radius is a circle.
                border_radius: BorderRadius::MAX,
                ..default()
            },
            ImageNode {
                image: placeholder.clone(),
                // Desaturation isn't available on a UI image, so a locked
                // node is dimmed by tint instead — it still reads as "not
                // yet" beside its lit neighbours.
                color: if locked { LOCKED_TINT } else { Color::WHITE },
                ..default()
            },
            BorderColor::all(ring),
            // A real widget button with a tab stop, never a bare `Node`
            // with a click observer — see the root `CLAUDE.md`.
            WidgetButton,
            TabIndex(0),
            Tooltip(tooltip_for(node, loc)),
        ))
        .observe(
            move |_: On<Activate>,
                  mut selected: ResMut<SelectedLesson>,
                  mut page: ResMut<NextState<MenuPage>>| {
                // Even a locked node opens: the reader explains what it
                // wants, which is more use than a node that does nothing.
                selected.0 = Some(id.clone());
                page.set(MenuPage::LessonReader);
            },
        )
        .id();
    commands.entity(canvas).add_child(button);
    commands
        .entity(button)
        .insert(ClusterMember(node.unit_id.clone()));

    // The title, under the node. Small and wrapped to the column's own
    // width — the art alone says nothing about which lesson this is, and a
    // tooltip only helps a player already pointing at it.
    let label = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x - LABEL_PX / 2.0),
                top: Val::Px(centre.y + NODE_PX / 2.0 + 6.0),
                width: Val::Px(LABEL_PX),
                ..default()
            },
            Text::new(String::from(loc.msg(&node.title_key))),
            TextFont {
                font_size: FontSize::Px(LABEL_FONT_PX),
                ..default()
            },
            TextLayout::justify(Justify::Center),
            TextColor(if locked {
                Color::srgba(0.62, 0.65, 0.74, 0.55)
            } else {
                Color::srgb(0.86, 0.89, 0.95)
            }),
        ))
        .id();
    commands.entity(canvas).add_child(label);
    commands
        .entity(label)
        .insert(ClusterMember(node.unit_id.clone()));

    if node.has_trainings {
        spawn_mastery_ring(commands, canvas, node, centre);
    }
}

/// The five training tiers, as pips arced over the node — the mastery
/// meter at node level.
///
/// Above rather than below, because the title now occupies the space under
/// every node and pips sitting in it read as punctuation.
fn spawn_mastery_ring(commands: &mut Commands, canvas: Entity, node: &PlacedNode, centre: Vec2) {
    let tiers = harmonicon_core::training::Tier::ALL.len();
    let filled = (node.mastery * tiers as f32).round() as usize;
    let radius = NODE_PX / 2.0 + 1.0;

    for tier in 0..tiers {
        let t = tier as f32 / (tiers - 1) as f32;
        // Negative sweeps the arc upward: screen y grows downward.
        let angle = -(42.0 + t * 96.0_f32).to_radians();
        commands.entity(canvas).with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(centre.x + radius * angle.cos() - PIP_PX / 2.0),
                    top: Val::Px(centre.y + radius * angle.sin() - PIP_PX / 2.0),
                    width: Val::Px(PIP_PX),
                    height: Val::Px(PIP_PX),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(if tier < filled { PIP_FILLED } else { PIP_EMPTY }),
                ClusterMember(node.unit_id.clone()),
            ));
        });
    }
}

/// Advances all unit transitions and applies their eased scale/visibility to
/// every entity in the corresponding cluster.
pub(crate) fn animate_unit_expansion(
    time: Res<Time>,
    collapsed: Res<CollapsedUnits>,
    mut expansions: ResMut<UnitExpansions>,
    mut members: Query<(&ClusterMember, &mut UiTransform, &mut Visibility)>,
    mut chevrons: Query<(&UnitChevron, &mut Text)>,
) {
    let step = time.delta_secs() / COLLAPSE_SECONDS;
    for (id, amount) in &mut expansions.0 {
        *amount = expansion_after(*amount, collapsed.0.contains(id), step);
    }

    for (member, mut transform, mut visibility) in &mut members {
        let amount = expansions.0.get(&member.0).copied().unwrap_or(1.0);
        // Smoothstep has zero velocity at both ends, so rapid toggles reverse
        // without a visible snap.
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
}

/// Rebuilds once every closing unit has finished. Waiting for zero preserves
/// the close animation; the next layout then removes the hidden width from the
/// canvas and moves later units left.
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

fn expansion_after(current: f32, collapsed: bool, step: f32) -> f32 {
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
mod tests {
    use super::*;

    #[test]
    fn the_margin_leaves_room_for_a_first_column_label() {
        // Every label is centred on its node, so the first column's widest
        // one has to fit between the node's centre and the canvas edge —
        // otherwise Unit 1's title is clipped, which is exactly what a
        // narrower margin did.
        let first = node_centre(0.0, 0.0);
        assert!(
            first.x - UNIT_LABEL_PX / 2.0 >= 0.0,
            "a unit title would start at {} px, off the canvas",
            first.x - UNIT_LABEL_PX / 2.0
        );
        assert!(first.x - LABEL_PX / 2.0 >= 0.0);
    }

    #[test]
    fn a_fractional_column_lands_between_two_whole_ones() {
        // A unit centres over its cluster, so its column is a half-step
        // whenever the widest lesson row holds an even number of nodes.
        // `node_centre` therefore has to interpolate across the grid rather
        // than index into it.
        let left = node_centre(0.0, 0.0);
        let right = node_centre(1.0, 0.0);
        let middle = node_centre(0.5, 0.0);
        assert_eq!(middle.x, (left.x + right.x) / 2.0);
        assert_eq!(middle.y, left.y);
    }

    #[test]
    fn the_top_inset_leaves_room_for_a_unit_title_above_the_spine() {
        // Unit titles are the only labels drawn *above* their node.
        let spine = node_centre(0.0, 0.0);
        assert!(spine.y - UNIT_PX / 2.0 - UNIT_FONT_PX * 2.0 >= 0.0);
    }

    fn lesson(column: f32, row: f32) -> Endpoint {
        Endpoint {
            centre: node_centre(column, row),
            radius: NODE_PX / 2.0,
        }
    }

    fn unit(column: f32, row: f32) -> Endpoint {
        Endpoint {
            centre: node_centre(column, row),
            radius: UNIT_PX / 2.0,
        }
    }

    #[test]
    fn an_edge_leaves_the_bottom_of_a_parent_and_arrives_at_the_top_of_its_child() {
        // The tree flows downward, so a child directly below its parent is
        // the ordinary case: the line has to run down the gap between them
        // rather than out of either one's flank.
        let (from, to) = (lesson(2.5, 1.0), lesson(2.5, 2.0));
        let (start, end) = edge_span(from, to).expect("nodes a whole row apart");
        assert_eq!(start, from.centre + Vec2::Y * NODE_PX / 2.0);
        assert_eq!(end, to.centre - Vec2::Y * NODE_PX / 2.0);
    }

    #[test]
    fn an_edge_points_the_way_its_centres_do() {
        // The invariant that keeps a line off the art it connects: whatever
        // direction the child lies in, both ends move along *that* line. A
        // child down and to the left is left by the parent's lower-left.
        let (from, to) = (lesson(2.5, 2.0), lesson(0.0, 3.0));
        let (start, end) = edge_span(from, to).expect("nodes a whole row apart");
        let along = (end - start).normalize();
        let centres = (to.centre - from.centre).normalize();
        assert!(
            along.distance(centres) < 1.0e-5,
            "edge runs {along} but its nodes lie {centres} apart",
        );
        assert!(start.x < from.centre.x, "the edge left the wrong side");
        assert!(start.y > from.centre.y, "the edge left the wrong side");
    }

    #[test]
    fn each_end_is_clipped_by_the_radius_of_the_node_it_touches() {
        // A spine edge meets two unit rings; a unit branch meets a ring at
        // the top and a lesson at the bottom. One shared radius would leave
        // the wider node's end buried inside its own art.
        let (spine_start, spine_end) =
            edge_span(unit(0.0, 0.0), unit(7.0, 0.0)).expect("two units apart on the spine");
        assert_eq!(spine_start.x - node_centre(0.0, 0.0).x, UNIT_PX / 2.0);
        assert_eq!(node_centre(7.0, 0.0).x - spine_end.x, UNIT_PX / 2.0);

        let (branch_start, branch_end) =
            edge_span(unit(2.5, 0.0), lesson(2.5, 1.0)).expect("a unit above its root lesson");
        assert_eq!(branch_start.y - node_centre(2.5, 0.0).y, UNIT_PX / 2.0);
        assert_eq!(node_centre(2.5, 1.0).y - branch_end.y, NODE_PX / 2.0);
    }

    #[test]
    fn nodes_too_close_to_separate_draw_no_edge() {
        // Nearer than their combined radii, the pull-backs would cross and
        // the segment would run backwards through both nodes.
        let touching = Endpoint {
            centre: Vec2::new(100.0, 100.0),
            radius: 40.0,
        };
        let overlapping = Endpoint {
            centre: Vec2::new(110.0, 100.0),
            radius: 40.0,
        };
        assert_eq!(edge_span(touching, overlapping), None);
        assert_eq!(edge_span(touching, touching), None);
    }

    #[test]
    fn expansion_moves_toward_the_requested_state_without_overshooting() {
        assert_eq!(expansion_after(1.0, true, 0.25), 0.75);
        assert_eq!(expansion_after(0.1, true, 0.25), 0.0);
        assert_eq!(expansion_after(0.0, false, 0.25), 0.25);
        assert_eq!(expansion_after(0.9, false, 0.25), 1.0);
    }
}
