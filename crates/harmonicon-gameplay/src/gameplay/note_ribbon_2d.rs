// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

/// A 2D note: one ribbon down its lane, as long as the note lasts, with a
/// bright cap at the bottom — the attack, carrying the tab label — and the
/// technique drawn along it in note time (see `note_ribbon_2d.wesl`). The
/// techniques legend draws its previews with the same material, so the two
/// can't disagree.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct NoteRibbon2dMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// `note_ribbon::ribbon_technique`: mode, rate in Hz, lean, and whether a
    /// vibrato rides a bend.
    #[uniform(1)]
    pub technique: Vec4,
    /// x = note duration (seconds), y = cap height (px), z = animation time
    /// (refreshed each frame by `animate_note_ribbons`), w = 1 to scroll the
    /// pattern with that time — the legend's previews, which don't move.
    #[uniform(2)]
    pub shape: Vec4,
    /// Live hold progress once the note is hit — see
    /// `note_feedback::hold_uniform` for the layout. Zero until then.
    #[uniform(3)]
    pub hold: Vec4,
    /// x = 1 when the ribbon lies along a horizontal highway, its attack at
    /// the left end; 0 for a vertical one, its attack at the bottom. See
    /// [`ribbon_axis`]. y = seconds of the note the hit line has consumed
    /// (`gameplay_2d::consumed_secs`): the ribbon drawn starts that far into
    /// the note, so its technique pattern still crosses the line on time.
    #[uniform(4)]
    pub axis: Vec4,
}

/// The `axis` uniform for a ribbon on a highway scrolling along `axis`.
pub fn ribbon_axis(axis: harmonicon_core::instrument::ScrollAxis) -> Vec4 {
    match axis {
        harmonicon_core::instrument::ScrollAxis::Vertical => Vec4::ZERO,
        harmonicon_core::instrument::ScrollAxis::Horizontal => Vec4::X,
    }
}

impl UiMaterial for NoteRibbon2dMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/note_ribbon_2d.wesl".into()
    }
}

pub struct NoteRibbon2dPlugin;

impl Plugin for NoteRibbon2dPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<NoteRibbon2dMaterial>::default());
    }
}

/// Drives every 2D ribbon's animation clock (`shape.z`) from the gameplay
/// clock, so the hold shimmer and the legend's scrolling previews run in
/// time with the song and freeze when paused — and stay frozen under
/// reduced motion.
pub fn animate_note_ribbons(
    clock: Res<super::GameplayClock>,
    reduced_motion: Res<harmonicon_platform::settings::ReducedMotion>,
    mut materials: ResMut<Assets<NoteRibbon2dMaterial>>,
) {
    if reduced_motion.0 {
        return;
    }
    let t = clock.get() as f32;
    for (_, material) in materials.iter_mut() {
        material.shape.z = t;
    }
}
