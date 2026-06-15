// SPDX-License-Identifier: MIT

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use harmonicon::assets_management::HarmonicaModelConfig;

#[derive(Resource)]
struct EditorState {
    model_name: String,
    config: HarmonicaModelConfig,
    selected: usize,
    dirty: bool,
}

#[derive(Resource)]
struct OrbitState {
    yaw: f32,
    pitch: f32,
    radius: f32,
    target: Vec3,
}

impl Default for OrbitState {
    fn default() -> Self {
        Self {
            yaw: 0.3,
            pitch: 0.4,
            radius: 15.0,
            target: Vec3::ZERO,
        }
    }
}

#[derive(Component)]
struct HoleIndicator(usize);

#[derive(Component)]
struct EditorCamera;

#[derive(Component)]
struct InfoText;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model_name = args
        .iter()
        .position(|a| a == "--model" || a == "-m")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    let path = format!("assets/harmonicas/3d/{model_name}/holes.json");
    let config: HarmonicaModelConfig = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| {
            eprintln!("No holes.json at {path}, starting empty");
            HarmonicaModelConfig {
                model_translation: [0.0, 0.0, 0.0],
                model_rotation_y_deg: 0.0,
                model_scale: 1.0,
                holes: vec![],
            }
        });

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Hole Editor — {model_name}"),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(EditorState {
            model_name,
            config,
            selected: 0,
            dirty: false,
        })
        .insert_resource(OrbitState {
            yaw: 0.3,
            pitch: 0.4,
            radius: 15.0,
            target: Vec3::ZERO,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                orbit_camera,
                handle_input,
                update_hole_meshes,
                update_info_text,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    state: Res<EditorState>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        EditorCamera,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            ..default()
        },
        Transform::from_xyz(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn(AmbientLight {
        brightness: 400.0,
        ..default()
    });

    let [tx, ty, tz] = state.config.model_translation;
    commands.spawn((
        WorldAssetRoot(asset_server.load(format!(
            "harmonicas/3d/{}/harmonica.glb#Scene0",
            state.model_name
        ))),
        Transform::from_xyz(tx, ty, tz)
            .with_rotation(Quat::from_rotation_y(
                state.config.model_rotation_y_deg.to_radians(),
            ))
            .with_scale(Vec3::splat(state.config.model_scale)),
    ));

    let unit_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for (i, hole) in state.config.holes.iter().enumerate() {
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.8, 0.2, 0.5),
            emissive: LinearRgba::new(0.0, 0.5, 0.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.spawn((
            Mesh3d(unit_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(hole.x, hole.y, hole.z)
                .with_scale(Vec3::new(hole.w, hole.h, hole.d)),
            HoleIndicator(i),
        ));
    }

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        InfoText,
    ));
}

fn orbit_camera(
    mut orbit: ResMut<OrbitState>,
    mut cameras: Query<&mut Transform, With<EditorCamera>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut scroll: MessageReader<MouseWheel>,
) {
    for e in scroll.read() {
        orbit.radius = (orbit.radius - e.y * 0.5).clamp(1.0, 60.0);
    }

    let mut delta = Vec2::ZERO;
    for e in motion.read() {
        delta += e.delta;
    }

    if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle) {
        orbit.yaw -= delta.x * 0.005;
        orbit.pitch = (orbit.pitch - delta.y * 0.005).clamp(-1.4, 1.4);
    } else if buttons.pressed(MouseButton::Left) {
        // Pan: move the look-at target in the camera's view plane.
        let right = Vec3::new(-orbit.yaw.sin(), 0.0, orbit.yaw.cos());
        let screen_up = Vec3::new(
            -orbit.yaw.cos() * orbit.pitch.sin(),
            orbit.pitch.cos(),
            -orbit.yaw.sin() * orbit.pitch.sin(),
        );
        let factor = orbit.radius * 0.0015;
        orbit.target += right * (-delta.x * factor) + screen_up * (delta.y * factor);
    }

    for mut tf in &mut cameras {
        let x = orbit.radius * orbit.yaw.cos() * orbit.pitch.cos() + orbit.target.x;
        let y = orbit.radius * orbit.pitch.sin() + orbit.target.y;
        let z = orbit.radius * orbit.yaw.sin() * orbit.pitch.cos() + orbit.target.z;
        *tf = Transform::from_xyz(x, y, z).looking_at(orbit.target, Vec3::Y);
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut state: ResMut<EditorState>,
    mut orbit: ResMut<OrbitState>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        *orbit = OrbitState::default();
    }
    let n = state.config.holes.len();
    if n == 0 {
        return;
    }

    for (key, idx) in [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
        (KeyCode::Digit9, 8),
        (KeyCode::Digit0, 9),
    ] {
        if keys.just_pressed(key) && idx < n {
            state.selected = idx;
        }
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let speed = if alt { 1.0f32 } else { 0.1 };
    let step = speed * time.delta_secs();

    let idx = state.selected;
    let mut changed = false;

    macro_rules! mv {
        ($field:ident, $delta:expr) => {{
            state.config.holes[idx].$field += $delta;
            changed = true;
        }};
    }

    if keys.pressed(KeyCode::ArrowLeft) {
        if shift { mv!(w, -step) } else { mv!(x, -step) }
    }
    if keys.pressed(KeyCode::ArrowRight) {
        if shift { mv!(w, step) } else { mv!(x, step) }
    }
    if keys.pressed(KeyCode::ArrowUp) {
        if shift { mv!(h, step) } else { mv!(y, step) }
    }
    if keys.pressed(KeyCode::ArrowDown) {
        if shift { mv!(h, -step) } else { mv!(y, -step) }
    }
    if keys.pressed(KeyCode::PageUp) {
        if shift { mv!(d, step) } else { mv!(z, step) }
    }
    if keys.pressed(KeyCode::PageDown) {
        if shift { mv!(d, -step) } else { mv!(z, -step) }
    }

    if changed {
        state.dirty = true;
    }

    if keys.just_pressed(KeyCode::KeyS) {
        let path = format!("assets/harmonicas/3d/{}/holes.json", state.model_name);
        match serde_json::to_string_pretty(&state.config) {
            Ok(json) => match std::fs::write(&path, &json) {
                Ok(_) => {
                    info!("Saved {path}");
                    state.dirty = false;
                }
                Err(e) => error!("Write failed: {e}"),
            },
            Err(e) => error!("Serialize failed: {e}"),
        }
    }
}

fn update_hole_meshes(
    state: Res<EditorState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut indicators: Query<(
        &HoleIndicator,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (ind, mut tf, mat_handle) in &mut indicators {
        let Some(hole) = state.config.holes.get(ind.0) else {
            continue;
        };
        tf.translation = Vec3::new(hole.x, hole.y, hole.z);
        tf.scale = Vec3::new(hole.w, hole.h, hole.d);

        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            if ind.0 == state.selected {
                mat.base_color = Color::srgba(1.0, 0.85, 0.1, 0.8);
                mat.emissive = LinearRgba::new(2.0, 1.5, 0.0, 1.0);
            } else {
                mat.base_color = Color::srgba(0.1, 0.8, 0.2, 0.4);
                mat.emissive = LinearRgba::new(0.0, 0.5, 0.0, 1.0);
            }
        }
    }
}

fn update_info_text(state: Res<EditorState>, mut query: Query<&mut Text, With<InfoText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let dirty = if state.dirty { " [unsaved]" } else { "" };
    let mut s = format!("Hole Editor — {}{dirty}\n", state.model_name);
    s += "LMB drag: pan   RMB/MMB drag: orbit   Scroll: zoom   R: reset camera\n";
    s += "1-0: select hole   Arrows: move X/Y   PgUp/Dn: move Z\n";
    s += "Shift+Arrows: resize W/H   Shift+PgUp/Dn: resize D\n";
    s += "Alt: 10x speed   S: save\n\n";

    for (i, hole) in state.config.holes.iter().enumerate() {
        let marker = if i == state.selected { "►" } else { " " };
        s += &format!(
            "{marker} {:2}  x={:7.3}  y={:7.3}  z={:7.3}  w={:6.3}  h={:6.3}  d={:6.3}\n",
            i + 1,
            hole.x,
            hole.y,
            hole.z,
            hole.w,
            hole.h,
            hole.d,
        );
    }

    text.0 = s;
}