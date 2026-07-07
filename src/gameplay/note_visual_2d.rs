// SPDX-License-Identifier: MIT

//! Shared UI hierarchy for a 2-D note: tail shader child + head image child.
//!
//! Both `gameplay_2d` (in-game notes) and the `note_editor` binary use
//! [`spawn_note_children`] so any layout change here is reflected everywhere.

use super::note_tail_2d::NoteTail2dMaterial;
use bevy::ui_render::prelude::MaterialNode;
use bevy::{asset::AssetPath, ecs::system::EntityCommands, prelude::*};

/// Everything needed to spawn the tail + head children of a 2-D note node.
pub struct NoteChildConfig {
    /// Horizontal centre of the tail base, as a fraction of the head's width.
    pub tail_x: f32,
    /// Vertical attach point on the head (0 = top, 1 = bottom).
    pub tail_y: f32,
    /// Base width of the tail, as a fraction of the head's width.
    pub tail_width: f32,
    /// Height of the tail child node.
    /// Gameplay uses `Val::Percent(100.0)` (resized each frame by `size_note_tails`).
    /// The editor uses `Val::Px(n)` for a fixed preview height.
    pub tail_height: Val,
    /// Already-allocated material handle for the tail shader. (Procedural per
    /// note, so it stays a handle — there's no asset path for it.)
    pub tail_material: Handle<NoteTail2dMaterial>,
    /// Asset path of the head PNG. The head node is a `bsn!` scene, so the image
    /// is loaded by path through the scene's `AssetServer` — callers pass a path
    /// (song-specific or theme default), not a pre-loaded `Handle`.
    pub head_image: AssetPath<'static>,
    /// Tint applied to the head image (use `Color::WHITE` for no tint).
    pub head_color: Color,
    /// Head destination rect within the note square, as percentages (0..100).
    /// Lets a theme nudge/resize the disc inside its lane cell when the source
    /// PNG isn't centred or filled. `(0, 0, 100, 100)` = fill the square.
    pub head_left: f32,
    pub head_top: f32,
    pub head_width: f32,
    pub head_height: f32,
}

/// Spawn the tail shader node and head image node as children of `parent`.
///
/// `on_tail` and `on_head` receive [`EntityCommands`] for the freshly spawned
/// entities so the caller can insert game-specific markers or sub-children.
///
/// ```text
/// parent
///  ├─ tail  (Node, MaterialNode<NoteTail2dMaterial>)  ← on_tail(cmd)
///  └─ head  (Node, ImageNode)                         ← on_head(cmd)
/// ```
pub fn spawn_note_children(
    parent: &mut ChildSpawnerCommands,
    cfg: &NoteChildConfig,
    on_tail: impl FnOnce(&mut EntityCommands),
    on_head: impl FnOnce(&mut EntityCommands),
) {
    let mut tail_cmd = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent((cfg.tail_x - cfg.tail_width * 0.5) * 100.0),
            bottom: Val::Percent((1.0 - cfg.tail_y) * 100.0),
            width: Val::Percent(cfg.tail_width * 100.0),
            height: cfg.tail_height,
            ..default()
        },
        MaterialNode(cfg.tail_material.clone()),
    ));
    on_tail(&mut tail_cmd);

    // Head node as a `bsn!` scene: the image loads by asset path via the scene's
    // `AssetServer`, so no `Handle` (or `AssetServer`) has to be threaded in.
    let mut head_cmd = parent.spawn_empty();
    head_cmd.apply_scene(bsn! {
        Node {
            position_type: {PositionType::Absolute},
            top: {Val::Percent(cfg.head_top)},
            left: {Val::Percent(cfg.head_left)},
            width: {Val::Percent(cfg.head_width)},
            height: {Val::Percent(cfg.head_height)},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
        }
        ImageNode {
            image: {cfg.head_image.clone()},
            color: {cfg.head_color},
        }
    });
    on_head(&mut head_cmd);
}
