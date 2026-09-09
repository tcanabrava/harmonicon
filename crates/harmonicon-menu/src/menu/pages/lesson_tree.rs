// SPDX-License-Identifier: MIT

//! The skill tree: the curriculum drawn as a layered graph.
//!
//! One root (`single-note`) fanning right through the prerequisite graph,
//! so a player sees the basics come first and the branches open out of
//! them. Columns are depth, rows are chosen to keep edges untangled, and
//! tracks are colour rather than rows — see [`layout`].
//!
//! [`layout`] decides everything and is pure; this file only turns that
//! into nodes, the same split `music_score` uses.
//!
//! **Edges are real curves, and need no shader.** `bevy_math`'s
//! [`CubicBezier`] gives the curve, `iter_positions` samples it, and each
//! sample pair becomes a short `Node` rotated by `UiTransform::rotation` —
//! a first-class UI field in Bevy 0.19. `bevy_ui` has no line primitive,
//! which is a fact about *drawing*; it says nothing about whether the
//! engine can compute a curve, and it can.
//!
//! **The list view is not replaced.** `responsive::is_compact` exists
//! because a phone cannot show a graph this wide; `MenuPage::Lessons`
//! remains the compact presentation and this is the wide one.

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
use harmonicon_song::lessons::AvailableLessons;
use harmonicon_song::lessons::graph::LessonGraph;
use harmonicon_ui::dialogs::tooltip::Tooltip;

use crate::menu::pages::lessons::SelectedLesson;
use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_menu_root};

use layout::{NodeState, PlacedNode, layout};

/// Node diameter.
const NODE_PX: f32 = 64.0;
/// Column pitch — wide enough that a curve has room to bend before it
/// arrives, which is what stops the edges reading as straight lines.
const COL_PX: f32 = 150.0;
/// Row pitch.
const ROW_PX: f32 = 92.0;
const MARGIN_PX: f32 = 24.0;

/// Edge thickness, and how many straight pieces approximate each curve.
/// Twenty is past the point more stops being visible at this scale.
const EDGE_PX: f32 = 3.5;
const EDGE_SEGMENTS: usize = 20;
/// How far the control points reach horizontally, as a fraction of the
/// gap. Flat tangents at both ends are what make the curve leave and
/// arrive horizontally rather than pointing corner to corner.
const EDGE_TENSION: f32 = 0.55;

const PIP_PX: f32 = 9.0;
const LOCKED_TINT: Color = Color::srgba(0.35, 0.35, 0.42, 0.55);
const PIP_FILLED: Color = Color::srgb(0.95, 0.80, 0.35);
const PIP_EMPTY: Color = Color::srgba(0.40, 0.43, 0.52, 0.8);
const EDGE_COLOR: Color = Color::srgba(0.62, 0.66, 0.78, 0.5);

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
fn node_centre(column: usize, row: f32) -> Vec2 {
    Vec2::new(
        MARGIN_PX + column as f32 * COL_PX + NODE_PX / 2.0,
        MARGIN_PX + row * ROW_PX + NODE_PX / 2.0,
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
    let (root, header, _page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("lesson-tree-title"),
        None,
        &theme,
        "LessonTree",
    );

    let manifests: Vec<_> = lessons.0.iter().map(|e| e.manifest.clone()).collect();
    let tree = match LessonGraph::build(&manifests) {
        Ok(graph) => layout(&lessons.0, &graph, &profile),
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
            spawn_back_button(&mut commands, header, &loc.msg("back"), back_to_lessons);
            return;
        }
    };

    let placeholder: Handle<Image> = asset_server.load("icons/lesson_placeholder.png");
    let canvas = commands
        .spawn(Node {
            position_type: PositionType::Relative,
            width: Val::Px(MARGIN_PX * 2.0 + tree.columns() as f32 * COL_PX),
            height: Val::Px(MARGIN_PX * 2.0 + tree.rows() * ROW_PX),
            ..default()
        })
        .id();
    commands.entity(root).add_child(canvas);

    // Edges first, so node art always sits on top of its connectors.
    commands.entity(canvas).with_children(|parent| {
        for edge in &tree.edges {
            spawn_edge(
                parent,
                node_centre(edge.from.0, edge.from.1),
                node_centre(edge.to.0, edge.to.1),
            );
        }
    });

    for node in &tree.nodes {
        spawn_node(&mut commands, canvas, node, &placeholder, &loc);
    }

    spawn_back_button(&mut commands, header, &loc.msg("back"), back_to_lessons);
}

fn back_to_lessons(_: On<Activate>, mut page: ResMut<NextState<MenuPage>>) {
    page.set(MenuPage::Lessons);
}

/// One prerequisite edge, as a cubic Bezier.
///
/// The control points sit level with each end and reach toward the other,
/// so the curve leaves its source horizontally and arrives horizontally —
/// the shape that reads as flow rather than as a corner. Sampled with
/// `bevy_math`'s own `iter_positions` and emitted as short rotated
/// segments; `BorderRadius::MAX` rounds each one so the joins don't show.
fn spawn_edge(parent: &mut ChildSpawnerCommands, from: Vec2, to: Vec2) {
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

    let points: Vec<Vec2> = curve.iter_positions(EDGE_SEGMENTS).collect();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let delta = b - a;
        let length = delta.length();
        if length < 0.01 {
            continue;
        }
        let mid = (a + b) / 2.0;
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                // Positioned by its own top-left, so shift back by half the
                // segment to centre it on the midpoint before rotating —
                // `UiTransform::rotation` turns a node about its centre.
                left: Val::Px(mid.x - length / 2.0),
                top: Val::Px(mid.y - EDGE_PX / 2.0),
                width: Val::Px(length),
                height: Val::Px(EDGE_PX),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            UiTransform {
                rotation: Rot2::radians(delta.y.atan2(delta.x)),
                ..default()
            },
            BackgroundColor(EDGE_COLOR),
        ));
    }
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
            Tooltip(format!(
                "{} · {}",
                loc.msg(&format!("lesson-track-{}", node.track)),
                loc.msg(&node.title_key)
            )),
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

    if node.has_trainings {
        spawn_mastery_ring(commands, canvas, node, centre);
    }
}

/// The five training tiers, as pips tucked under the node — the mastery
/// meter at node level.
fn spawn_mastery_ring(commands: &mut Commands, canvas: Entity, node: &PlacedNode, centre: Vec2) {
    let tiers = harmonicon_core::training::Tier::ALL.len();
    let filled = (node.mastery * tiers as f32).round() as usize;
    let radius = NODE_PX / 2.0 + 1.0;

    for tier in 0..tiers {
        // A tight arc across the bottom, where a pip can't be mistaken for
        // part of the art.
        let t = tier as f32 / (tiers - 1) as f32;
        let angle = (42.0 + t * 96.0_f32).to_radians();
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
