// SPDX-License-Identifier: MIT

//! The 3D scene around the notes: camera, lighting, backdrop, the note
//! track and hit zone, the row of hole pads and their glow, and the camera
//! restore on exit.

use super::*;

// ── Setup ─────────────────────────────────────────────────────────────────────

pub(super) fn setup_camera_3d(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        // High and close, looking down the lane: steep enough that the
        // far end isn't crushed into the horizon (see `LANE_DEPTH` for the
        // measured speed ratio), shallow enough that it still reads as depth.
        Transform::from_xyz(0.0, 14.0, 16.0).looking_at(Vec3::new(0.0, LANE_Y, 0.0), Vec3::Y),
        GameplayCamera3D,
        GameplayRoot,
        Name::new("Camera3d (gameplay 3D)"),
    ));
}

pub(super) fn setup_lighting(commands: &mut Commands) {
    commands.spawn((
        DirectionalLight { illuminance: 8_000.0, color: Color::srgb(1.0, 0.97, 0.90), ..default() },
        Transform::from_xyz(8.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        GameplayRoot,
    ));
    commands.spawn((
        AmbientLight { color: Color::srgb(0.15, 0.15, 0.22), brightness: 200.0, ..default() },
        GameplayRoot,
    ));
}

pub fn setup_background(commands: &mut Commands, background: Handle<Image>) {
    // Mesh and material are built inline with `asset_value`, so the scene adds
    // them via the `AssetServer` at spawn time — no `Assets` params to thread.
    commands.spawn_scene(bsn! {
        Mesh3d({asset_value(Rectangle::new(200.0, 140.0))})
        MeshMaterial3d::<StandardMaterial>({asset_value(StandardMaterial {
            base_color_texture: Some(background),
            unlit: true,
            cull_mode: None,
            ..default()
        })})
        Transform { translation: {Vec3::new(0.0, 14.0, FAR_Z - 2.0)} }
        GameplayRoot
    });
}

pub(super) fn create_note_track(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    hole_count: u8,
) {
    let total_width = f32::from(hole_count) * LANE_WIDTH;
    // The track runs from the far end to just past the hole pads.
    let track_len = PAD_Z + 0.6 - FAR_Z;
    let track_ctr_z = FAR_Z + track_len * 0.5;
    // Semi-translucent floor so notes dipping below the lane (downward bends)
    // stay visible. Mesh + material built inline via `asset_value`.
    commands.spawn_scene(bsn! {
        Mesh3d({asset_value(Cuboid::new(total_width, 0.05, track_len))})
        MeshMaterial3d::<StandardMaterial>({asset_value(StandardMaterial {
            base_color: Color::srgba(0.08, 0.08, 0.12, 0.7),
            alpha_mode: AlphaMode::Blend,
            metallic: 0.3,
            perceptual_roughness: 0.8,
            ..default()
        })})
        Transform { translation: {Vec3::new(0.0, LANE_Y - 0.025, track_ctr_z)} }
        GameplayRoot
    });

    // Alternating-lane shading is per-hole (a loop), so it stays imperative.
    let shade_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.05),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let shade_mesh = meshes.add(Cuboid::new(LANE_WIDTH, 0.04, track_len));
    for hole in (1..=hole_count).step_by(2) {
        commands.spawn((
            Mesh3d(shade_mesh.clone()),
            MeshMaterial3d(shade_mat.clone()),
            Transform::from_xyz(lane_x(hole, hole_count), LANE_Y, track_ctr_z),
            GameplayRoot,
        ));
    }
}

pub(super) fn create_hit_zone(commands: &mut Commands, total_width: f32) {
    commands.spawn_scene(bsn! {
        Mesh3d({asset_value(Cuboid::new(total_width, 0.06, 2.8))})
        MeshMaterial3d::<StandardMaterial>({asset_value(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 0.6, 0.15),
            emissive: LinearRgba::new(0.8, 0.8, 0.2, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })})
        Transform { translation: {Vec3::new(0.0, LANE_Y + 0.03, HIT_Z)} }
        GameplayRoot
    });
    // The line inside the band that is the judgment instant — a note's
    // front face reaches it exactly on time, as 2D's hit line.
    commands.spawn_scene(bsn! {
        Mesh3d({asset_value(Cuboid::new(total_width, HIT_LINE_H, 0.08))})
        MeshMaterial3d::<StandardMaterial>({asset_value(StandardMaterial {
            base_color: Color::srgb(1.0, 0.98, 0.62),
            emissive: LinearRgba::new(2.0, 1.9, 1.0, 1.0),
            unlit: true,
            ..default()
        })})
        Transform { translation: {Vec3::new(0.0, HIT_LINE_Y, HIT_Z)} }
        GameplayRoot
    });
}

/// One flat pad per hole at the near end of its lane, lit by
/// `update_holes_3d` when that hole sounds (blow blue, draw orange) and
/// dimly when a note in its lane is due — the 3D stand-in for 2D's hole
/// strip.
pub(super) fn spawn_hole_pads(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hole_count: u8,
) {
    let pad_mesh = meshes.add(Cuboid::new(LANE_WIDTH * NOTE_W, PAD_H, 0.9));
    for hole in 1..=hole_count {
        let pad_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.11, 0.15),
            emissive: LinearRgba::new(0.0, 0.0, 0.0, 0.0),
            metallic: 0.3,
            perceptual_roughness: 0.6,
            ..default()
        });
        commands.spawn((
            Mesh3d(pad_mesh.clone()),
            MeshMaterial3d(pad_mat.clone()),
            Transform::from_xyz(lane_x(hole, hole_count), LANE_Y + PAD_H * 0.5, PAD_Z),
            HoleCell(hole),
            HoleMesh3D(pad_mat),
            GameplayRoot,
        ));
    }
}

// ── Per-frame systems ─────────────────────────────────────────────────────────

pub fn update_holes_3d(
    mut glow: super::super::gameplay_2d::HoleGlow,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cells: Query<(&HoleCell, &HoleMesh3D, &mut HoleState)>,
) {
    if !glow.begin_frame() {
        return;
    }
    for (cell, hole_mat, mut state) in &mut cells {
        glow.step(cell.0, &mut state);
        let b = state.brightness;

        let (emissive, base_color) = if state.is_blow {
            (
                LinearRgba::new(0.05 + 0.15 * b, 0.10 + 0.50 * b, 0.10 + 2.0 * b, 1.0),
                Color::srgb(0.05 + 0.20 * b, 0.08 + 0.40 * b, 0.08 + 0.75 * b),
            )
        } else {
            (
                LinearRgba::new(0.05 + 2.0 * b, 0.05 + 0.40 * b, 0.02, 1.0),
                Color::srgb(0.08 + 0.78 * b, 0.06 + 0.25 * b, (0.08 - 0.04 * b).max(0.0)),
            )
        };
        // An unchanged write through `get_mut` still re-uploads the material.
        if materials
            .get(&hole_mat.0)
            .is_some_and(|m| m.emissive != emissive || m.base_color != base_color)
            && let Some(mut mat) = materials.get_mut(&hole_mat.0)
        {
            mat.emissive = emissive;
            mat.base_color = base_color;
        }
    }
}

/// Called on `OnExit(AppState::Playing)` — restores Camera2d to its normal
/// state so the 2D menu renders correctly after leaving 3D gameplay.
pub fn restore_camera(mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>) {
    for (mut cam, _) in &mut cameras {
        cam.order = 0;
        cam.clear_color = ClearColorConfig::Default;
    }
}
