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
//! **Edges are real curves, and need no shader.** `bevy_math`'s
//! [`CubicBezier`] gives the curve, `iter_positions` samples it, and each
//! sample pair becomes a short `Node` rotated by `UiTransform::rotation` —
//! a first-class UI field in Bevy 0.19. `bevy_ui` has no line primitive,
//! which is a fact about *drawing*; it says nothing about whether the
//! engine can compute a curve, and it can.
//!
//! **There is no list view any more.** This replaced it rather than sitting
//! beside it, so a lesson has one home and unit gating is stated in one
//! place. The tree is wide, and a small screen reaches the rest of it by
//! scrolling; `responsive::is_compact` no longer routes anywhere.

pub(crate) mod layout;

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::math::cubic_splines::CubicBezier;
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

use crate::menu::pages::lesson_reader::SelectedLesson;
use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_menu_root_plain};
use harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy;

use layout::{EdgeKind, NodeState, PlacedNode, PlacedUnit, layout};

/// Node diameter.
const NODE_PX: f32 = 64.0;
/// Column pitch — wide enough that a curve has room to bend before it
/// arrives, which is what stops the edges reading as straight lines.
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
const LABEL_FONT_PX: f32 = 11.0;

/// Edge thickness.
const EDGE_PX: f32 = 3.5;
/// Curve pixels per straight piece. **Resolution scales with length**, the
/// way Bevy's own curve example does it — a fixed segment count made a long
/// edge's pieces longer than they were thick, and with rounded caps that
/// read as a string of beads rather than a line.
const EDGE_PX_PER_SEGMENT: f32 = 4.0;
const EDGE_MIN_SEGMENTS: usize = 24;
/// How far the control points reach horizontally, as a fraction of the
/// gap. Flat tangents at both ends are what make the curve leave and
/// arrive horizontally rather than pointing corner to corner.
const EDGE_TENSION: f32 = 0.55;

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
fn node_centre(column: usize, row: f32) -> Vec2 {
    Vec2::new(
        MARGIN_PX + column as f32 * COL_PX + NODE_PX / 2.0,
        MARGIN_PX + SPINE_LABEL_PX + row * ROW_PX + NODE_PX / 2.0,
    )
}

pub(crate) fn setup_lesson_tree(
    mut commands: Commands,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
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
        Ok(graph) => layout(&lessons.0, &graph, &chain, &profile),
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
            width: Val::Px(MARGIN_PX * 2.0 + tree.columns() as f32 * COL_PX),
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
            let (thickness, color) = match edge.kind {
                EdgeKind::Spine => (UNIT_EDGE_PX, SPINE_COLOR),
                EdgeKind::Branch => (EDGE_PX, EDGE_COLOR),
            };
            spawn_edge(
                parent,
                node_centre(edge.from.0, edge.from.1),
                node_centre(edge.to.0, edge.to.1),
                thickness,
                color,
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

/// One prerequisite edge, as a cubic Bezier.
///
/// The control points sit level with each end and reach toward the other,
/// so the curve leaves its source horizontally and arrives horizontally —
/// the shape that reads as flow rather than as a corner. Sampled with
/// `bevy_math`'s own `iter_positions` and emitted as short rotated
/// segments; `BorderRadius::MAX` rounds each one so the joins don't show.
fn spawn_edge(
    parent: &mut ChildSpawnerCommands,
    from: Vec2,
    to: Vec2,
    thickness: f32,
    color: Color,
) {
    // Start and end at the nodes' edges rather than their centres, so a
    // curve doesn't run underneath the art it connects.
    let radius = NODE_PX / 2.0;
    let start = Vec2::new(from.x + radius, from.y);
    let end = Vec2::new(to.x - radius, to.y);
    let reach = ((end.x - start.x) * EDGE_TENSION).max(24.0);

    let curve = CubicBezier::new([[
        start,
        Vec2::new(start.x + reach, start.y),
        Vec2::new(end.x - reach, end.y),
        end,
    ]])
    .to_curve();
    let Ok(curve) = curve else { return };

    // Scale the sampling with how far the curve actually travels, so a long
    // sweep is no coarser than a short hop.
    let span = (end - start).length() + (end.y - start.y).abs();
    let segments = ((span / EDGE_PX_PER_SEGMENT) as usize).max(EDGE_MIN_SEGMENTS);

    let points: Vec<Vec2> = curve.iter_positions(segments).collect();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let delta = b - a;
        let length = delta.length();
        if length < 0.01 {
            continue;
        }
        // Overlap neighbours by a whole thickness: consecutive rotated
        // rectangles leave a wedge at every joint where the angle changes,
        // and overlapping is what closes it without a mitre calculation.
        let drawn = length + thickness;
        let mid = (a + b) / 2.0;
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                // Positioned by its own top-left, so shift back by half the
                // segment to centre it on the midpoint before rotating —
                // `UiTransform::rotation` turns a node about its centre.
                left: Val::Px(mid.x - drawn / 2.0),
                top: Val::Px(mid.y - thickness / 2.0),
                width: Val::Px(drawn),
                height: Val::Px(thickness),
                ..default()
            },
            UiTransform {
                rotation: Rot2::radians(delta.y.atan2(delta.x)),
                ..default()
            },
            BackgroundColor(color),
        ));
    }
}

/// A unit: the major node the whole cluster below it hangs off.
///
/// Not a button — there is nothing to open, since a unit *is* the lessons
/// under it. It carries the gate's own terms instead ("3 / 8"), because a
/// lock whose conditions a player can't read is just an obstacle.
fn spawn_unit(commands: &mut Commands, canvas: Entity, unit: &PlacedUnit, loc: &Localization) {
    let centre = node_centre(unit.column, unit.row);
    let ring = if unit.locked { UNIT_SHUT } else { UNIT_OPEN };

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
        ))
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
        let first = node_centre(0, 0.0);
        assert!(
            first.x - UNIT_LABEL_PX / 2.0 >= 0.0,
            "a unit title would start at {} px, off the canvas",
            first.x - UNIT_LABEL_PX / 2.0
        );
        assert!(first.x - LABEL_PX / 2.0 >= 0.0);
    }

    #[test]
    fn the_top_inset_leaves_room_for_a_unit_title_above_the_spine() {
        // Unit titles are the only labels drawn *above* their node.
        let spine = node_centre(0, 0.0);
        assert!(spine.y - UNIT_PX / 2.0 - UNIT_FONT_PX * 2.0 >= 0.0);
    }
}
