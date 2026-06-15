// SPDX-License-Identifier: MIT

use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// 3D mesh material for a note's comet tail — the `Material` twin of the 2D
/// `NoteShapeMaterial`. Drawn on a flat ribbon behind the cube head; the shader
/// carves the animated tail shape via alpha. Same uniform layout as the 2D
/// material so the technique animations stay in lockstep.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct NoteTail3dMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = vibrato amplitude, y = vibrato cycles, z = animation time in seconds
    /// (refreshed each frame by `animate_note_tails_3d`), w = bend amplitude.
    #[uniform(1)]
    pub params: Vec4,
    /// x = wah depth, y = wah cycles, z = animation mode, w = per-note phase.
    #[uniform(2)]
    pub wah: Vec4,
}

impl Material for NoteTail3dMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/note_tail_3d.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub struct NoteTail3dPlugin;

impl Plugin for NoteTail3dPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<NoteTail3dMaterial>::default());
    }
}
