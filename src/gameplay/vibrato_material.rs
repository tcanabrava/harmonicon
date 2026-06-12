use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

/// UI material that fills a note tile with a vibrato band: straight top/bottom
/// edges and sine-curved sides, drawn smoothly in `assets/shaders/vibrato.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct VibratoMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = sway amplitude (fraction of width), y = cycles, z = body half-width,
    /// w = phase. See the shader for the exact mapping.
    #[uniform(1)]
    pub params: Vec4,
}

impl UiMaterial for VibratoMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vibrato.wgsl".into()
    }
}

/// Builds the shader params for a vibrato note. More intensity widens the sway;
/// the wave count scales with the note's length so the wavelength stays roughly
/// constant however long the note is held.
pub fn vibrato_params(h_pct: f32, intensity: f32) -> Vec4 {
    let sway = 0.26 + intensity.clamp(0.0, 1.0) * 0.34; // total sway, fraction of width
    let amp = sway * 0.5; // peak centre offset from the middle
    let body_half = (1.0 - sway) * 0.5; // half-width of the filled body
    let cycles = (h_pct / 6.5).clamp(1.0, 6.0);
    Vec4::new(amp, cycles, body_half, 0.0)
}

pub struct VibratoMaterialPlugin;

impl Plugin for VibratoMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<VibratoMaterial>::default());
    }
}
