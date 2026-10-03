// SPDX-License-Identifier: MIT

//! A real curved tie, drawn as a [`UiMaterial`] fragment shader
//! (`assets/shaders/music_score_tie.wesl`) rather than the flat rectangle
//! plain `bevy_ui` `Node`/`BackgroundColor` primitives are limited to —
//! the same "custom shader for a shape a plain `Node` can't express"
//! pattern `gameplay::note_ribbon_2d::NoteRibbon2dMaterial` already
//! established for the falling-note comet tail, applied here to a small,
//! static shape instead of an animated one. Two shared material instances
//! cover every tie in the panel — one bowing down under the notes, one up
//! over them: unlike the note tail (animated per-note, so each note needs
//! its own material instance to carry its own uniform values), a tie's
//! shape never varies, so [`TieMaterialHandle`] is created once at startup
//! and every tie glyph just clones the `Handle` for its side.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct TieMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = arc depth (fraction of the node's own height the middle dips
    /// to), y = line thickness (fraction of the node's own height), z = 1
    /// for a tie above the notes (the arc bows up from the node's bottom
    /// edge), 0 below; w unused (padding — `AsBindGroup` uniforms round up
    /// to a `vec4` alignment regardless).
    #[uniform(1)]
    pub params: Vec4,
}

impl UiMaterial for TieMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/music_score_tie.wesl".into()
    }
}

/// The shared tie materials every tie glyph in the panel points at — one
/// per side, in white and in the highlight gold (a tie belongs to its note,
/// so it follows the note's highlight). See the module doc comment for why
/// shared instances are enough.
#[derive(Resource, Clone)]
pub struct TieMaterialHandle {
    /// Bows down, for a tie under the noteheads (stems up).
    pub below: Handle<TieMaterial>,
    /// Bows up, for a tie over them (stems down).
    pub above: Handle<TieMaterial>,
    pub below_highlighted: Handle<TieMaterial>,
    pub above_highlighted: Handle<TieMaterial>,
}

pub(super) struct TieMaterialPlugin;

impl Plugin for TieMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<TieMaterial>::default())
            .add_systems(Startup, load_tie_material);
    }
}

fn load_tie_material(mut materials: ResMut<Assets<TieMaterial>>, mut commands: Commands) {
    let mut tie = |color: Color, above: f32| {
        materials
            .add(TieMaterial { color: color.into(), params: Vec4::new(0.85, 0.16, above, 0.0) })
    };
    commands.insert_resource(TieMaterialHandle {
        below: tie(Color::WHITE, 0.0),
        above: tie(Color::WHITE, 1.0),
        below_highlighted: tie(super::HIGHLIGHT_COLOR, 0.0),
        above_highlighted: tie(super::HIGHLIGHT_COLOR, 1.0),
    });
}
