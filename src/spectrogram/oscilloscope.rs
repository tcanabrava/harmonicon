//! Oscilloscope: a continuous, glowing time-domain trace of the harmonica
//! waveform, drawn by a UI material shader (one node, not per-sample dots).

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};

use super::{Spectrum, WAVE_POINTS};

/// Samples packed four-per-`Vec4` for the uniform array.
const PACKED: usize = WAVE_POINTS / 4;

/// UI material fed the waveform; the shader renders it as a glowing line.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct OscilloscopeMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub wave: [Vec4; PACKED],
}

impl UiMaterial for OscilloscopeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/oscilloscope.wgsl".into()
    }
}

/// Holds the single oscilloscope material so the trace node and the per-frame
/// updater share one asset.
#[derive(Resource)]
pub struct OscMaterial(pub Handle<OscilloscopeMaterial>);

/// Creates the shared material once at startup.
pub fn init_material(mut commands: Commands, mut materials: ResMut<Assets<OscilloscopeMaterial>>) {
    let handle = materials.add(OscilloscopeMaterial {
        color: Color::srgb(0.45, 1.0, 0.65).to_linear(),
        wave: [Vec4::ZERO; PACKED],
    });
    commands.insert_resource(OscMaterial(handle));
}

/// Spawns the full-panel trace node.
pub fn spawn(parent: &mut ChildSpawnerCommands, material: &Handle<OscilloscopeMaterial>) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        MaterialNode(material.clone()),
    ));
}

/// Feeds the latest waveform into the material each frame.
pub fn update_scope(
    spectrum: Res<Spectrum>,
    osc: Res<OscMaterial>,
    mut materials: ResMut<Assets<OscilloscopeMaterial>>,
) {
    let Some(mut mat) = materials.get_mut(&osc.0) else {
        return;
    };
    for i in 0..WAVE_POINTS {
        let v = spectrum.waveform.get(i).copied().unwrap_or(0.0);
        mat.wave[i / 4][i % 4] = v;
    }
}
