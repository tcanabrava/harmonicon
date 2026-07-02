// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

/// Editor-specific note overlay material. Renders Vibrato as a horizontal sine
/// wave ribbon and Wah as an alternating thick/thin band. Drawn in
/// `assets/shaders/editor_note.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct EditorNoteMaterial {
    #[uniform(0)]
    pub(super) color: LinearRgba,
    /// x = mode (0 = vibrato, 1 = wah). y = the note's rendered width in
    /// pixels, so the shader can derive a constant wavelength (px per wave
    /// cycle) instead of stretching a fixed cycle count across the note —
    /// resizing the note then repeats/truncates the pattern rather than
    /// squeezing or stretching it. z/w unused.
    #[uniform(1)]
    pub(super) params: Vec4,
}

impl UiMaterial for EditorNoteMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/editor_note.wgsl".into()
    }
}

pub(super) struct EditorNoteMaterialPlugin;

impl Plugin for EditorNoteMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<EditorNoteMaterial>::default());
    }
}
