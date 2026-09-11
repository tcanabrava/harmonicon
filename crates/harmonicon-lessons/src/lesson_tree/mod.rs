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
mod transition;

use accesskit::{Node as AccessibilityKitNode, Role};
use bevy::a11y::AccessibilityNode;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui::{ScrollPosition, UiTransform};
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
pub(crate) use transition::*;

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
/// Gap between the dots of an elective branch. A dot is the edge's own
/// thickness across, so a dotted line carries the same weight as a solid
/// one and only the continuity differs.
const EDGE_DOT_GAP_PX: f32 = 7.0;

const PIP_PX: f32 = 9.0;
const LOCKED_TINT: Color = Color::srgba(0.35, 0.35, 0.42, 0.55);
const PIP_FILLED: Color = Color::srgb(0.95, 0.80, 0.35);
const PIP_EMPTY: Color = Color::srgba(0.40, 0.43, 0.52, 0.8);
const OPTIONAL_COLOR: Color = Color::srgb(0.62, 0.88, 0.82);
/// A track this build has no colour for. Only reachable from an externally
/// authored lesson — see [`declared_track_color`].
const UNKNOWN_TRACK_COLOR: Color = Color::srgb(0.65, 0.68, 0.78);
/// The elective badge drawn beside a node, and its clearance from the ring.
const BADGE_PX: f32 = 20.0;
const BADGE_GAP_PX: f32 = 2.0;
/// The badge has to stay inside its own column, or it collides with the
/// next node's title.
const _: () = assert!(NODE_PX / 2.0 + BADGE_GAP_PX + BADGE_PX < COL_PX - LABEL_PX / 2.0);
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

/// Any lesson node, label, mastery pip, or branch edge owned by a unit.
#[derive(Component)]
pub(crate) struct ClusterMember(String);

#[derive(Component)]
pub(crate) struct UnitChevron(String);

#[derive(Component)]
pub(crate) struct UnitButton(String);

/// Any visual whose horizontal position follows one unit's layout column.
#[derive(Component)]
pub(crate) struct LayoutOwner(String);

/// Endpoints retained in layout coordinates so a moving relationship remains
/// one continuous straight segment throughout a neighboring-unit slide.
#[derive(Component)]
pub(crate) struct MovingEdge {
    from: Endpoint,
    to: Endpoint,
    from_unit: String,
    to_unit: String,
    thickness: f32,
}

/// A track's colour. Grouping has to survive losing its row, and colour is
/// what the skill trees this is modelled on use for the same job.
fn track_color(track: &str) -> Color {
    declared_track_color(track).unwrap_or(UNKNOWN_TRACK_COLOR)
}

/// The colour this build gives `track`, or `None` if it has none of its own.
///
/// Separate from [`track_color`] so the fallback is *detectable*. Every
/// bundled track must have its own entry — `every_bundled_track_has_its_own_colour`
/// fails the build otherwise — while an externally authored lesson dropped
/// into `~/Harmonicon/lessons` may still name any track at all and gets
/// [`UNKNOWN_TRACK_COLOR`].
///
/// Hues are spread around the wheel rather than picked to taste: with
/// twenty tracks, two that sit close together are two groups a player
/// cannot tell apart. Saturation and value stay inside the range the
/// original palette established, so a new track doesn't read as louder
/// than the rest.
fn declared_track_color(track: &str) -> Option<Color> {
    Some(match track {
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
        "navigation" => Color::srgb(0.82, 0.78, 0.58),
        "positions" => Color::srgb(0.51, 0.86, 0.47),
        "ornaments" => Color::srgb(0.90, 0.56, 0.93),
        "ear" => Color::srgb(0.55, 0.55, 0.94),
        "accompaniment" => Color::srgb(0.46, 0.86, 0.80),
        "practice" => Color::srgb(0.89, 0.95, 0.47),
        _ => return None,
    })
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
    mut collapsed: ResMut<CollapsedUnits>,
    mut expansions: ResMut<UnitExpansions>,
    mut pending: ResMut<PendingCompaction>,
    mut anchor: ResMut<PendingViewportAnchor>,
    mut previous_positions: ResMut<PreviousUnitPositions>,
    mut slides: ResMut<UnitSlides>,
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

    let live_units: HashSet<&str> = tree.units.iter().map(|unit| unit.id.as_str()).collect();
    collapsed.0.retain(|id| live_units.contains(id.as_str()));
    expansions
        .0
        .retain(|id, _| live_units.contains(id.as_str()));
    pending.0.retain(|id| live_units.contains(id.as_str()));
    slides.0.retain(|id, _| live_units.contains(id.as_str()));
    if anchor
        .unit_id
        .as_deref()
        .is_some_and(|id| !live_units.contains(id))
    {
        *anchor = PendingViewportAnchor::default();
    }

    if let Some(unit_id) = anchor.unit_id.as_deref() {
        anchor.canvas_x = tree
            .units
            .iter()
            .find(|unit| unit.id == unit_id)
            .map(|unit| node_centre(unit.column, unit.row).x);
    }

    let anchor_id = anchor.unit_id.as_deref();
    let current_positions: HashMap<String, f32> = tree
        .units
        .iter()
        .map(|unit| (unit.id.clone(), node_centre(unit.column, unit.row).x))
        .collect();
    let mut next_slides = HashMap::new();
    for (id, &new_x) in &current_positions {
        let Some(&old_x) = previous_positions.0.get(id) else {
            continue;
        };
        let old_offset = slides.0.get(id).map_or(0.0, slide_offset);
        let from_px = if anchor_id == Some(id.as_str()) {
            0.0
        } else {
            old_x + old_offset - new_x
        };
        if from_px.abs() > 0.5 {
            next_slides.insert(
                id.clone(),
                UnitSlide {
                    from_px,
                    amount: 0.0,
                },
            );
        }
    }
    previous_positions.0 = current_positions;
    slides.0 = next_slides;

    let placeholder: Handle<Image> = asset_server.load("icons/lesson_placeholder.png");

    // A unit discovered while the app is running starts expanded. Existing
    // animation values survive page rebuilds and visits to the reader.
    let unit_ids: Vec<String> = tree.units.iter().map(|unit| unit.id.clone()).collect();
    for id in unit_ids {
        let initial = if collapsed.0.contains(&id) { 0.0 } else { 1.0 };
        expansions.0.entry(id).or_insert(initial);
    }

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
    commands.entity(scroller).insert(LessonTreeScroller);

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
                    .zip(
                        tree.units
                            .iter()
                            .find(|unit| (unit.column, unit.row) == edge.to),
                    )
                    .map(|(from, to)| (from.id.as_str(), to.id.as_str())),
                EdgeKind::UnitBranch | EdgeKind::Branch => {
                    edge.unit_id.as_deref().map(|unit| (unit, unit))
                }
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
                style,
                edge.unit_id.as_deref(),
                owners,
            );
        }
    });

    for unit in &tree.units {
        spawn_unit(
            &mut commands,
            canvas,
            unit,
            !collapsed.0.contains(&unit.id),
            &loc,
        );
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

fn set_edge_geometry(
    node: &mut Node,
    transform: &mut UiTransform,
    from: Endpoint,
    to: Endpoint,
    thickness: f32,
) -> bool {
    let Some((start, end)) = edge_span(from, to) else {
        return false;
    };
    let delta = end - start;
    let length = delta.length();
    let mid = (start + end) / 2.0;
    node.left = Val::Px(mid.x - length / 2.0);
    node.top = Val::Px(mid.y - thickness / 2.0);
    node.width = Val::Px(length);
    node.height = Val::Px(thickness);
    transform.rotation = Rot2::radians(delta.y.atan2(delta.x));
    true
}

/// How an edge is drawn, as opposed to where it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
struct EdgeStyle {
    thickness: f32,
    color: Color,
    /// Elective branches are dotted: the conventional way a dependency
    /// diagram says "you may skip this" without spending a second meaning
    /// on brightness, which here already distinguishes locked from open.
    dotted: bool,
}

/// Centres of the dots making up an elective branch, evenly spread so one
/// sits against each node's boundary and the spacing comes out equal.
///
/// Spacing is derived from the span rather than fixed, which is what stops
/// a short edge ending in a ragged half-gap. `diameter` is the dot size, so
/// the first and last centres are inset by half of it.
fn dot_centres(start: Vec2, end: Vec2, diameter: f32, gap: f32) -> Vec<Vec2> {
    let delta = end - start;
    let length = delta.length();
    // Centre-to-centre distance available once both end dots are inset.
    let travel = length - diameter;
    if travel <= 0.0 {
        return vec![(start + end) / 2.0];
    }
    let direction = delta / length;
    let count = ((travel / (diameter + gap)).round() as usize + 1).max(2);
    let spacing = travel / (count - 1) as f32;
    (0..count)
        .map(|i| start + direction * (diameter / 2.0 + spacing * i as f32))
        .collect()
}

/// An elective branch, as a run of round dots.
///
/// Each dot is its own entity carrying [`LayoutOwner`], not one
/// [`MovingEdge`]: a dotted edge is always inside a single cluster (the
/// spine is never elective), so both its endpoints take the same slide
/// offset and the whole run translates rigidly. `animate_unit_slides`
/// writes only `translation`, so each dot keeps the rotation set here.
fn spawn_dotted_edge(
    parent: &mut ChildSpawnerCommands,
    start: Vec2,
    end: Vec2,
    style: EdgeStyle,
    unit_id: Option<&str>,
) {
    let diameter = style.thickness;
    for centre in dot_centres(start, end, diameter, EDGE_DOT_GAP_PX) {
        let mut dot = parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x - diameter / 2.0),
                top: Val::Px(centre.y - diameter / 2.0),
                width: Val::Px(diameter),
                height: Val::Px(diameter),
                // A square with a maximal radius is a circle.
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(style.color),
        ));
        if let Some(unit_id) = unit_id {
            dot.insert((
                ClusterMember(unit_id.to_string()),
                LayoutOwner(unit_id.to_string()),
            ));
        }
    }
}

/// One edge, as a single rotated rectangle.
///
/// A straight run needs no sampling and no joins, so there is exactly one
/// node per edge and no seam anywhere along it.
fn spawn_edge(
    parent: &mut ChildSpawnerCommands,
    from: Endpoint,
    to: Endpoint,
    style: EdgeStyle,
    unit_id: Option<&str>,
    owners: Option<(&str, &str)>,
) {
    let Some((start, end)) = edge_span(from, to) else {
        return;
    };
    let EdgeStyle {
        thickness, color, ..
    } = style;
    if style.dotted {
        spawn_dotted_edge(parent, start, end, style, unit_id);
        return;
    }
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
    if let Some((from_unit, to_unit)) = owners {
        segment.insert(MovingEdge {
            from,
            to,
            from_unit: from_unit.to_string(),
            to_unit: to_unit.to_string(),
            thickness,
        });
    }
    if let Some(unit_id) = unit_id {
        segment.insert(ClusterMember(unit_id.to_string()));
    }
}

/// A unit: the major node the whole cluster below it hangs off.
///
/// Activating it expands or collapses the lessons belonging to the unit.
fn spawn_unit(
    commands: &mut Commands,
    canvas: Entity,
    unit: &PlacedUnit,
    expanded: bool,
    loc: &Localization,
) {
    let centre = node_centre(unit.column, unit.row);
    // Locked still wins: an all-elective unit sits behind the units before
    // it like any other, and has to read that way while it does.
    let ring = match (unit.locked, unit.elective_only) {
        (true, _) => UNIT_SHUT,
        (false, true) => OPTIONAL_COLOR,
        (false, false) => UNIT_OPEN,
    };
    let unit_id = unit.id.clone();
    let mut accessibility = AccessibilityKitNode::new(Role::Button);
    accessibility.set_label(String::from(loc.msg(&unit.title_key)));
    accessibility.set_expanded(expanded);

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
            AccessibilityNode(accessibility),
            UnitButton(unit.id.clone()),
            LayoutOwner(unit.id.clone()),
        ))
        .observe(
            move |_: On<Activate>,
                  mut collapsed: ResMut<CollapsedUnits>,
                  mut pending: ResMut<PendingCompaction>,
                  mut anchor: ResMut<PendingViewportAnchor>,
                  scroller: Query<&ScrollPosition, With<LessonTreeScroller>>,
                  mut page: ResMut<NextState<MenuPage>>| {
                let scroll_x = scroller.iter().next().map_or(0.0, |position| position.x);
                anchor.unit_id = Some(unit_id.clone());
                anchor.screen_x = centre.x - scroll_x;
                anchor.canvas_x = None;
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
            LayoutOwner(unit.id.clone()),
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
    let mut head = format!(
        "{} · {}",
        loc.msg(&format!("lesson-track-{}", node.track)),
        loc.msg(&node.title_key)
    );
    if node.optional {
        head = format!("{} · {head}", loc.msg("lesson-tree-optional"));
    }
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
    commands.entity(button).insert((
        ClusterMember(node.unit_id.clone()),
        LayoutOwner(node.unit_id.clone()),
    ));

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
    commands.entity(label).insert((
        ClusterMember(node.unit_id.clone()),
        LayoutOwner(node.unit_id.clone()),
    ));

    if node.optional {
        // Beside the node at mid-height, not over its top-right shoulder:
        // the mastery pips arc from 42° to 138°, so the whole upper corner
        // belongs to them, and the title owns everything below. Clearing
        // the button's own bounds also keeps it off the click target.
        let badge = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(centre.x + NODE_PX / 2.0 + BADGE_GAP_PX),
                    top: Val::Px(centre.y - BADGE_PX / 2.0),
                    ..default()
                },
                Text::new("◇"),
                TextFont {
                    font_size: FontSize::Px(BADGE_PX),
                    ..default()
                },
                TextColor(OPTIONAL_COLOR),
                ClusterMember(node.unit_id.clone()),
                LayoutOwner(node.unit_id.clone()),
            ))
            .id();
        commands.entity(canvas).add_child(badge);
    }

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
                LayoutOwner(node.unit_id.clone()),
            ));
        });
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

    #[test]
    fn every_bundled_track_has_its_own_colour() {
        // Colour is the only thing carrying track grouping, now that the
        // tree places nodes by prerequisite depth rather than by track. A
        // bundled lesson falling through to `UNKNOWN_TRACK_COLOR` doesn't
        // fail anything — it just joins an undifferentiated grey pile, and
        // five tracks once did exactly that. The fallback itself stays:
        // an externally authored lesson may name any track at all.
        //
        // Built from `CARGO_MANIFEST_DIR`, not the working directory —
        // `assets/` is two levels up from a crate, and a wrong runtime
        // path is not a compile error.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/lessons");
        let units =
            std::fs::read_dir(&root).unwrap_or_else(|e| panic!("reading {}: {e}", root.display()));

        let mut seen = 0usize;
        let mut uncoloured: Vec<String> = Vec::new();
        for unit in units.flatten() {
            for lesson in std::fs::read_dir(unit.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                let path = lesson.path().join("lesson.json");
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let manifest = harmonicon_song::lessons::parse_lesson(&bytes)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                seen += 1;
                if let Some(track) = manifest.track.as_deref()
                    && declared_track_color(track).is_none()
                {
                    uncoloured.push(track.to_string());
                }
            }
        }

        assert!(
            seen > 0,
            "no bundled lessons found under {}",
            root.display()
        );
        uncoloured.sort();
        uncoloured.dedup();
        assert!(
            uncoloured.is_empty(),
            "bundled tracks with no colour of their own, so they all draw \
             the same grey: {uncoloured:?}"
        );
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
    fn a_dotted_edge_starts_and_ends_against_the_nodes_it_joins() {
        // A dot sits against each boundary, so a dotted branch reaches its
        // node exactly as far as a solid one does.
        let (start, end) = (Vec2::new(100.0, 100.0), Vec2::new(100.0, 240.0));
        let dots = dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX);
        assert!(dots.len() > 2, "expected a run of dots, got {}", dots.len());
        assert!((dots[0].distance(start) - EDGE_PX / 2.0).abs() < 1.0e-4);
        assert!((dots[dots.len() - 1].distance(end) - EDGE_PX / 2.0).abs() < 1.0e-4);
    }

    #[test]
    fn dots_are_evenly_spaced_however_long_the_edge() {
        // Spacing is derived from the span rather than fixed, which is what
        // stops a short edge finishing on a ragged half-gap.
        for length in [20.0_f32, 58.0, 137.0, 394.0] {
            let (start, end) = (Vec2::ZERO, Vec2::new(length, 0.0));
            let dots = dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX);
            let gaps: Vec<f32> = dots.windows(2).map(|w| w[0].distance(w[1])).collect();
            let first = gaps[0];
            assert!(
                gaps.iter().all(|g| (g - first).abs() < 1.0e-3),
                "uneven spacing over {length}px: {gaps:?}"
            );
        }
    }

    #[test]
    fn dots_follow_a_diagonal_edge() {
        // Every dot has to land *on* the line, not on a bounding box of it.
        let (start, end) = (Vec2::new(40.0, 40.0), Vec2::new(200.0, 160.0));
        let direction = (end - start).normalize();
        for dot in dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX) {
            let along = (dot - start).dot(direction);
            assert!(
                (start + direction * along).distance(dot) < 1.0e-3,
                "dot {dot} sits off the line"
            );
        }
    }

    #[test]
    fn a_span_too_short_for_two_dots_draws_one() {
        let (start, end) = (Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
        assert_eq!(
            dot_centres(start, end, EDGE_PX, EDGE_DOT_GAP_PX),
            vec![Vec2::new(1.0, 0.0)]
        );
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

    #[test]
    fn viewport_anchor_preserves_the_units_screen_coordinate() {
        assert_eq!(anchored_scroll(700.0, 250.0, 600.0, 1_400.0), 450.0);
    }

    #[test]
    fn viewport_anchor_respects_both_scroll_bounds() {
        assert_eq!(anchored_scroll(100.0, 250.0, 600.0, 1_400.0), 0.0);
        assert_eq!(anchored_scroll(1_300.0, 250.0, 600.0, 1_400.0), 800.0);
        assert_eq!(anchored_scroll(700.0, 250.0, 900.0, 600.0), 0.0);
    }

    #[test]
    fn neighboring_unit_slide_uses_a_smooth_complete_transition() {
        let start = UnitSlide {
            from_px: 300.0,
            amount: 0.0,
        };
        let middle = UnitSlide {
            amount: 0.5,
            ..start
        };
        let end = UnitSlide {
            amount: 1.0,
            ..start
        };
        assert_eq!(slide_offset(&start), 300.0);
        assert_eq!(slide_offset(&middle), 150.0);
        assert_eq!(slide_offset(&end), 0.0);
    }
}
