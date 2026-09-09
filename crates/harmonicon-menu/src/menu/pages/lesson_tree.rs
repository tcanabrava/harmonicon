// SPDX-License-Identifier: MIT

//! The skill tree: the curriculum drawn as a graph instead of a list.
//!
//! Tracks stack downward and each reads left to right, so the page scrolls
//! vertically and never sideways — see `docs/training_tree_plan.md` for why
//! that shape was measured before it was chosen (600 px wide against 896
//! tall, so nothing needs horizontal panning).
//!
//! [`layout`] decides everything — rows, columns, node state, edges — and
//! is pure. This file only turns that into nodes, the same split
//! `music_score` uses.
//!
//! **The list view is not replaced.** `responsive::is_compact` exists
//! because a phone cannot show fourteen rows of anything; `MenuPage::
//! Lessons` remains the compact presentation and this is the wide one.

pub(crate) mod layout;

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button as WidgetButton};
use bevy_fluent::Localization;

use harmonicon_app::profile::PlayerProfile;
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::lessons::AvailableLessons;
use harmonicon_song::lessons::graph::LessonGraph;

use crate::menu::pages::lessons::SelectedLesson;
use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_menu_root};

use layout::{NodeState, PlacedNode, layout};

/// Node diameter. Big enough for art to read at a glance, small enough that
/// the widest track (six nodes) still fits a narrow window.
const NODE_PX: f32 = 72.0;
/// Gap between nodes along a row, and between rows.
const NODE_GAP_PX: f32 = 26.0;
const ROW_GAP_PX: f32 = 18.0;
/// Width reserved for the track label at the left of each row.
const LABEL_PX: f32 = 96.0;
/// Thickness of a prerequisite connector.
const EDGE_PX: f32 = 2.0;
/// The mastery ring: five pips spaced around the node's edge.
const PIP_PX: f32 = 9.0;

const LOCKED_TINT: Color = Color::srgba(0.35, 0.35, 0.42, 0.55);
const AVAILABLE_RING: Color = Color::srgb(0.95, 0.80, 0.35);
const PASSED_RING: Color = Color::srgb(0.45, 0.85, 0.50);
const IDLE_RING: Color = Color::srgba(0.55, 0.58, 0.68, 0.7);
const PIP_FILLED: Color = Color::srgb(0.95, 0.80, 0.35);
const PIP_EMPTY: Color = Color::srgba(0.40, 0.43, 0.52, 0.8);
const EDGE_COLOR: Color = Color::srgba(0.55, 0.58, 0.68, 0.45);

/// Marks the tree's own scrolling canvas, so edges can be positioned
/// against it rather than against whichever row they start in.
#[derive(Component)]
pub(crate) struct LessonTreeCanvas;

/// Top-left corner of a node, in canvas pixels.
fn node_origin(row: usize, column: usize) -> (f32, f32) {
    (
        LABEL_PX + column as f32 * (NODE_PX + NODE_GAP_PX),
        row as f32 * (NODE_PX + ROW_GAP_PX),
    )
}

fn node_centre(row: usize, column: usize) -> (f32, f32) {
    let (x, y) = node_origin(row, column);
    (x + NODE_PX / 2.0, y + NODE_PX / 2.0)
}

/// The ring colour a node's state calls for.
fn ring_color(state: NodeState) -> Color {
    match state {
        NodeState::Locked => IDLE_RING,
        NodeState::Available => AVAILABLE_RING,
        NodeState::Passed | NodeState::Mastered => PASSED_RING,
    }
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
            spawn_notice(
                &mut commands,
                root,
                String::from(loc.msg_args("lesson-tree-broken", &[("error", e.to_string())])),
            );
            spawn_back_button(&mut commands, header, &loc.msg("back"), back_to_lessons);
            return;
        }
    };

    let placeholder: Handle<Image> = asset_server.load("icons/lesson_placeholder.png");
    let canvas = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                width: Val::Px(LABEL_PX + tree.columns() as f32 * (NODE_PX + NODE_GAP_PX)),
                height: Val::Px(tree.height() as f32 * (NODE_PX + ROW_GAP_PX)),
                ..default()
            },
            LessonTreeCanvas,
        ))
        .id();
    commands.entity(root).add_child(canvas);

    // Edges first, so a node's art always sits on top of its connectors.
    commands.entity(canvas).with_children(|parent| {
        for edge in &tree.edges {
            spawn_edge(parent, edge.from, edge.to);
        }
    });

    for row in &tree.rows {
        // Centred across however many sub-rows the track occupies.
        let (_, top) = node_origin(row.first_row, 0);
        let span = row.height as f32 * (NODE_PX + ROW_GAP_PX);
        commands.entity(canvas).with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(top + span / 2.0 - 10.0),
                    width: Val::Px(LABEL_PX - 12.0),
                    ..default()
                },
                Text::new(String::from(
                    loc.msg(&format!("lesson-track-{}", row.track)),
                )),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.75, 0.85)),
            ));
        });

        for node in &row.nodes {
            spawn_node(&mut commands, canvas, node, &placeholder, &loc);
        }
    }

    spawn_back_button(&mut commands, header, &loc.msg("back"), back_to_lessons);
}

fn back_to_lessons(_: On<Activate>, mut page: ResMut<NextState<MenuPage>>) {
    page.set(MenuPage::Lessons);
}

fn spawn_notice(commands: &mut Commands, root: Entity, text: String) {
    let line = commands
        .spawn((
            Text::new(text),
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
}

/// A prerequisite connector, drawn as two rectangles rather than one
/// diagonal: Bevy UI has no line primitive, and an orthogonal route is what
/// the skill trees this is modelled on use anyway.
fn spawn_edge(parent: &mut ChildSpawnerCommands, from: (usize, usize), to: (usize, usize)) {
    let (fx, fy) = node_centre(from.0, from.1);
    let (tx, ty) = node_centre(to.0, to.1);

    // Vertical leg at the source's x, then a horizontal leg into the target.
    let (top, height) = (fy.min(ty), (ty - fy).abs().max(EDGE_PX));
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(fx - EDGE_PX / 2.0),
            top: Val::Px(top),
            width: Val::Px(EDGE_PX),
            height: Val::Px(height),
            ..default()
        },
        BackgroundColor(EDGE_COLOR),
    ));
    let (left, width) = (fx.min(tx), (tx - fx).abs().max(EDGE_PX));
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(ty - EDGE_PX / 2.0),
            width: Val::Px(width),
            height: Val::Px(EDGE_PX),
            ..default()
        },
        BackgroundColor(EDGE_COLOR),
    ));
}

fn spawn_node(
    commands: &mut Commands,
    canvas: Entity,
    node: &PlacedNode,
    placeholder: &Handle<Image>,
    loc: &Localization,
) {
    let (x, y) = node_origin(node.row, node.column);
    let locked = node.state == NodeState::Locked;
    let id = node.id.clone();

    let button = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(NODE_PX),
                height: Val::Px(NODE_PX),
                border: UiRect::all(Val::Px(3.0)),
                // A square node with a maximal radius is a circle.
                border_radius: BorderRadius::MAX,
                ..default()
            },
            ImageNode {
                image: placeholder.clone(),
                // Desaturation is not available on a UI image, so a locked
                // node is dimmed by tint instead — it still reads as "not
                // yet" beside its lit neighbours.
                color: if locked { LOCKED_TINT } else { Color::WHITE },
                ..default()
            },
            BorderColor::all(ring_color(node.state)),
            // A real widget button with a tab stop, never a bare `Node`
            // with a click observer — see the root `CLAUDE.md`.
            WidgetButton,
            TabIndex(0),
            Tooltip(String::from(loc.msg(&node.title_key))),
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
        spawn_mastery_ring(commands, canvas, node);
    }
}

/// The five training tiers, as pips spaced around the node's edge — the
/// mastery meter at node level.
///
/// Plain positioned squares with a maximal radius, not an arc shader: five
/// dots around a circle need no new material, and `music_score::
/// tie_material` is the precedent if this ever wants a real arc.
fn spawn_mastery_ring(commands: &mut Commands, canvas: Entity, node: &PlacedNode) {
    let tiers = harmonicon_core::training::Tier::ALL.len();
    let filled = (node.mastery * tiers as f32).round() as usize;
    let (cx, cy) = node_centre(node.row, node.column);
    let radius = NODE_PX / 2.0 + 1.0;

    for tier in 0..tiers {
        // A tight arc hugging the bottom of the node, where a pip can't be
        // mistaken for part of the art. Deliberately narrow: a wider spread
        // reached past the node's own width and ran into the next node's
        // pips, which read as scattered dots rather than one meter.
        let t = tier as f32 / (tiers - 1) as f32;
        let angle = (42.0 + t * 96.0_f32).to_radians();
        let px = cx + radius * angle.cos() - PIP_PX / 2.0;
        let py = cy + radius * angle.sin() - PIP_PX / 2.0;
        commands.entity(canvas).with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(px),
                    top: Val::Px(py),
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

use harmonicon_ui::dialogs::tooltip::Tooltip;
