use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;

use crate::{
    assets_management::{GlobalFonts, SelectedHarmonicaModel},
    menu::SelectedSong,
    song::SongManifest,
    song::chart::{Action, HarpChart, Modifier},
    song::harmonica::twelve_bar,
};

use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, ComboText,
    FeedbackText, GameplayRoot, HOLE_COUNT, HoleCell, HoleState, LOOKAHEAD,
    MusicStarted, ScheduledNote, ScoreText, ValidHarpNotes,
};
use super::countdown_overlay::spawn_countdown;
use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::spawn_modifier_legend;
use super::phrase_overlay::spawn_phrase_banner;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};

// ── 3D layout constants ───────────────────────────────────────────────────────

const LANE_WIDTH: f32 = 1.0;
const LANE_GAP: f32 = 0.06;
const LANE_DEPTH: f32 = 60.0;
const HIT_Z: f32 = 6.0;
const FAR_Z: f32 = HIT_Z - LANE_DEPTH; // -54
const LANE_Y: f32 = 1.6;
const NOTE_H: f32 = 0.18;
const HARP_Z: f32 = HIT_Z + 2.2;

// ── 3D-only marker components ─────────────────────────────────────────────────

#[derive(Component)]
pub struct GameplayCamera3D;

#[derive(Component)]
pub(super) struct NoteVisual3D {
    time: f64,
    depth: f32,
}

#[derive(Component)]
pub(super) struct HoleMesh3D(Handle<StandardMaterial>);

/// Parent entity holding the GLB model and its hole overlays. Animated by
/// `groove_harmonica` so the whole harmonica bobs in time with the music.
#[derive(Component)]
pub(super) struct HarmonicaGroove;

fn lane_x(hole: u8) -> f32 {
    (hole as f32 - 1.0) * LANE_WIDTH - (HOLE_COUNT as f32 * LANE_WIDTH) / 2.0 + LANE_WIDTH * 0.5
}

fn note_depth(duration: f64) -> f32 {
    ((duration as f32 / LOOKAHEAD as f32) * LANE_DEPTH).clamp(0.4, 12.0)
}

// ── Harmonica model config ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HoleConfig {
    x: f32,
    y: f32,
    z: f32,
    /// Width along the X axis.
    w: f32,
    /// Height along the Y axis.
    h: f32,
    /// Depth along the Z axis.
    d: f32,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HarmonicaModelConfig {
    /// World-space translation for the GLB scene root.
    model_translation: [f32; 3],
    /// Y-axis rotation applied to the GLB scene, in degrees.
    #[serde(default)]
    model_rotation_y_deg: f32,
    /// Uniform scale applied to the GLB scene.
    #[serde(default = "default_model_scale")]
    model_scale: f32,
    /// One entry per hole; index 0 = hole 1, index 9 = hole 10.
    holes: Vec<HoleConfig>,
}

fn default_model_scale() -> f32 {
    1.0
}

impl HarmonicaModelConfig {
    fn default_layout() -> Self {
        Self {
            model_translation: [0.0, LANE_Y + 0.45, HARP_Z],
            model_rotation_y_deg: 0.0,
            model_scale: 1.0,
            holes: (1u8..=HOLE_COUNT as u8)
                .map(|hole| HoleConfig {
                    x: lane_x(hole),
                    y: LANE_Y + 0.9 + 0.10,
                    z: HARP_Z,
                    w: LANE_WIDTH - LANE_GAP - 0.08,
                    h: 0.20,
                    d: 0.90,
                })
                .collect(),
        }
    }
}

fn load_model_config(model_name: &str) -> HarmonicaModelConfig {
    let path = format!("assets/harmonicas/3d/{model_name}/holes.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| {
            warn!("No holes.json for model '{model_name}', using default layout");
            HarmonicaModelConfig::default_layout()
        })
}

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup_camera_3d(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 14.0, 24.0)
            .looking_at(Vec3::new(0.0, 0.0, HIT_Z - 18.0), Vec3::Y),
        GameplayCamera3D,
        GameplayRoot,
        Name::new("Camera3d (gameplay 3D)"),
    ));
}

fn setup_lighting(commands: &mut Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            color: Color::srgb(1.0, 0.97, 0.90),
            ..default()
        },
        Transform::from_xyz(8.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        GameplayRoot,
    ));
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.15, 0.15, 0.22),
            brightness: 200.0,
            ..default()
        },
        GameplayRoot,
    ));
}

pub fn setup_background(
    commands: &mut Commands,
    background: Handle<Image>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let backdrop_mat = materials.add(StandardMaterial {
        base_color_texture: Some(background.clone()),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let backdrop_mesh = meshes.add(Rectangle::new(200.0, 140.0));
    commands.spawn((
        Mesh3d(backdrop_mesh),
        MeshMaterial3d(backdrop_mat),
        Transform::from_xyz(0.0, 14.0, FAR_Z - 2.0),
        GameplayRoot,
    ));
}

fn create_note_track(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    total_width: f32,
    lane_width: f32,
    track_len: f32,
    center_x: f32,
    track_ctr_z: f32,
    holes: &[HoleConfig],
) {
    // Semi-translucent so notes dipping below the lane (downward bends) stay visible.
    let lane_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.08, 0.08, 0.12, 0.5),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.3,
        perceptual_roughness: 0.8,
        ..default()
    });
    let floor_mesh = meshes.add(Cuboid::new(total_width, 0.05, track_len));
    commands.spawn((
        Mesh3d(floor_mesh),
        MeshMaterial3d(lane_mat.clone()),
        Transform::from_xyz(center_x, LANE_Y - 0.025, track_ctr_z),
        GameplayRoot,
    ));

    for (i, hole) in holes.iter().enumerate() {
        if i % 2 == 1 {
            continue;
        }
        let shade_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.04),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let shade_mesh = meshes.add(Cuboid::new(lane_width, 0.04, track_len));
        commands.spawn((
            Mesh3d(shade_mesh),
            MeshMaterial3d(shade_mat),
            Transform::from_xyz(hole.x, LANE_Y, track_ctr_z),
            GameplayRoot,
        ));
    }
}

fn create_hit_zone(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    center_x: f32,
    total_width: f32,
) {
    let hit_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 0.6, 0.15),
        emissive: LinearRgba::new(0.8, 0.8, 0.2, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let hit_mesh = meshes.add(Cuboid::new(total_width, 0.06, 2.8));
    commands.spawn((
        Mesh3d(hit_mesh),
        MeshMaterial3d(hit_mat),
        Transform::from_xyz(center_x, LANE_Y + 0.03, HIT_Z),
        GameplayRoot,
    ));
}

/// Sigmoid bend profile (matches `note_shape.wgsl`): holds flat, transitions
/// sharply through the middle, then holds flat — a note bending pitch and settling.
fn bend_ease(t: f32) -> f32 {
    let x = ((t - 0.5) * 2.0 + 0.5).clamp(0.0, 1.0);
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// Builds a technique note body: a `width`×`height` slab whose cross-section is
/// swept along Z (its length) with its centre displaced by `offset(t)` returning
/// `(dx, dy)` (t in 0..=1), with flat end caps — the 3D analogue of the 2D shaped
/// tile. Vibrato sways on X; a bend arcs on Y (up or down).
fn swept_note_mesh(
    width: f32,
    height: f32,
    depth: f32,
    segments: usize,
    offset: impl Fn(f32) -> (f32, f32),
) -> Mesh {
    let (hw, hh, hd) = (width * 0.5, height * 0.5, depth * 0.5);

    // Corners of the cross-section at sample plane `r`: [TL, TR, BR, BL].
    let ring = |r: usize| -> [[f32; 3]; 4] {
        let t = r as f32 / segments as f32;
        let z = -hd + t * depth;
        let (dx, dy) = offset(t);
        [
            [dx - hw, dy + hh, z], // TL
            [dx + hw, dy + hh, z], // TR
            [dx + hw, dy - hh, z], // BR
            [dx - hw, dy - hh, z], // BL
        ]
    };

    // Non-indexed triangle soup so `compute_normals` yields crisp per-face (flat)
    // normals. Winding is CCW-outward for every face so nothing is back-culled.
    let mut positions: Vec<[f32; 3]> = Vec::new();
    for r in 0..segments {
        let a = ring(r); // nearer -Z
        let b = ring(r + 1); // nearer +Z
        let (atl, atr, abr, abl) = (a[0], a[1], a[2], a[3]);
        let (btl, btr, bbr, bbl) = (b[0], b[1], b[2], b[3]);
        positions.extend_from_slice(&[
            // top (+Y)
            atl, btl, btr, atl, btr, atr,
            // bottom (-Y)
            abl, abr, bbr, abl, bbr, bbl,
            // left (-X)
            atl, abl, bbl, atl, bbl, btl,
            // right (+X)
            atr, btr, bbr, atr, bbr, abr,
        ]);
    }
    // End caps: front ring faces -Z, back ring faces +Z.
    let f = ring(0);
    positions.extend_from_slice(&[f[0], f[1], f[2], f[0], f[2], f[3]]);
    let k = ring(segments);
    positions.extend_from_slice(&[k[0], k[2], k[1], k[0], k[3], k[2]]);

    let uvs = vec![[0.0_f32, 0.0]; positions.len()];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    // No indices -> flat normals (one normal per face), so the faceted body
    // reads as solid rather than smeared/glitchy.
    mesh.compute_normals();
    mesh
}

pub fn create_note_visuals(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    holes: &[HoleConfig],
    chart: &HarpChart,
) {
    for item in &chart.track {
        let t = super::resolve_item_time(item, &chart.timing);
        let depth = note_depth(item.duration);
        for event in &item.events {
            use crate::song::chart::Action;
            let is_blow = matches!(event.action, Action::Blow);
            let (r, g, b, emit_r, emit_g, emit_b) = if is_blow {
                (0.25f32, 0.55, 0.95, 0.1, 0.3, 1.2)
            } else {
                (0.95f32, 0.38, 0.15, 1.2, 0.2, 0.05)
            };
            let modifiers = event.modifiers.clone().unwrap_or_default();
            // Notes carrying a technique get a coloured emissive rim matching the
            // 2D badge palette, so bends/wah/etc. read at a glance on the lane too.
            let note_mat = if let Some(m) = modifiers.first() {
                let c = super::modifier_color(m).to_linear();
                materials.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    emissive: LinearRgba::new(c.red * 1.6, c.green * 1.6, c.blue * 1.6, 1.0),
                    ..default()
                })
            } else {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    emissive: LinearRgba::new(emit_r, emit_g, emit_b, 1.0),
                    ..default()
                })
            };
            let hole_cfg = holes.get(event.hole.saturating_sub(1) as usize);
            let note_x = hole_cfg.map(|h| h.x).unwrap_or_else(|| lane_x(event.hole));
            let note_w = hole_cfg.map(|h| h.w).unwrap_or(LANE_WIDTH - LANE_GAP);
            // A bend/vibrato note's body curves along its length (Z), like the 2D
            // shaped tile: vibrato sways as a sine, a bend arcs to one side.
            // Everything else stays a straight cuboid.
            let vibrato = modifiers.iter().find_map(|m| match m {
                Modifier::Vibrato { intensity, .. } => Some(intensity.unwrap_or(0.5)),
                _ => None,
            });
            // Pitch shift in semitones: negative bends down, positive (overblow/
            // overdraw) bends up. Sign sets the arc direction, magnitude its depth.
            let shift = modifiers.iter().find_map(|m| match m {
                Modifier::Bend { semitones, .. } => Some(*semitones),
                Modifier::Overblow | Modifier::Overdraw => Some(1.0),
                _ => None,
            });
            let note_mesh = if vibrato.is_some() || shift.is_some() {
                use std::f32::consts::TAU;
                let vib_amp = vibrato.map_or(0.0, |i| 0.18 + 0.22 * i.clamp(0.0, 1.0));
                // Y deflection in world units. Large so the bend reads clearly down
                // the lane from the camera; the S-curve below holds it as a plateau.
                let bend_amp = shift.map_or(0.0, |s| {
                    let mag = 0.8 + 4.0 * (s.abs() / 3.0).clamp(0.0, 1.0);
                    mag * if s < 0.0 { -1.0 } else { 1.0 }
                });
                let cycles = (depth / 2.0).clamp(1.0, 6.0);
                let segments = ((cycles * 8.0).ceil() as usize).clamp(12, 96);
                meshes.add(swept_note_mesh(note_w, NOTE_H, depth, segments, move |t| {
                    // Vibrato sways on X; the bend arcs on Y as a sigmoid (down for
                    // negative, up for overblow/overdraw) — hold, bend, hold.
                    let dx = vib_amp * (t * cycles * TAU).sin();
                    let dy = bend_amp * bend_ease(t);
                    (dx, dy)
                }))
            } else {
                meshes.add(Cuboid::new(note_w, NOTE_H, depth))
            };
            let expected_pitch = event.note.clone().unwrap_or_else(|| {
                chart
                    .harmonica
                    .wind_direction_label(event.hole, &event.action)
            });
            commands.spawn((
                Mesh3d(note_mesh),
                MeshMaterial3d(note_mat),
                Transform::from_xyz(note_x, LANE_Y + NOTE_H * 0.5, FAR_Z),
                NoteVisual3D { time: t, depth },
                ScheduledNote {
                    time: t,
                    hole: event.hole,
                    is_blow,
                    expected_pitch,
                    hit: false,
                    missed: false,
                    modifiers,
                },
                GameplayRoot,
            ));
        }
    }
}

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut valid_notes: ResMut<ValidHarpNotes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fonts: Res<GlobalFonts>,
    asset_server: Res<AssetServer>,
    selected_model: Res<SelectedHarmonicaModel>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>,
) {
    let Some(manifest): Option<&SongManifest> = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing (3D) state");
        return;
    };
    clock.0 = -COUNTDOWN;
    music_started.0 = false;
    valid_notes.0 = manifest.chart.harmonica.build_valid_notes();

    for (mut cam, _) in &mut cameras {
        cam.order = 1;
        cam.clear_color = ClearColorConfig::None;
    }

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let chords = twelve_bar(key);
    let font = fonts.gameplay.clone();
    let model_cfg = load_model_config(&selected_model.0);

    setup_camera_3d(&mut commands);
    setup_lighting(&mut commands);
    setup_background(&mut commands, manifest.background.clone(), &mut meshes, &mut materials);

    let holes = &model_cfg.holes;
    let left_edge = holes.first().map(|h| h.x - h.w * 0.5).unwrap_or(-5.0);
    let right_edge = holes.last().map(|h| h.x + h.w * 0.5).unwrap_or(5.0);
    let total_width = right_edge - left_edge;
    let center_x = (left_edge + right_edge) * 0.5;
    let lane_width = total_width / holes.len() as f32;

    let track_end_z = holes.first().map(|h| h.z).unwrap_or(HARP_Z);
    let track_len = track_end_z - FAR_Z;
    let track_ctr_z = FAR_Z + track_len * 0.5;

    create_note_track(
        &mut commands, &mut meshes, &mut materials,
        total_width, lane_width, track_len, center_x, track_ctr_z,
        holes,
    );

    create_hit_zone(&mut commands, &mut meshes, &mut materials, center_x, total_width);
    create_note_visuals(&mut commands, &mut meshes, &mut materials, holes, chart);
    spawn_harmonica_3d(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &selected_model.0,
        &model_cfg,
    );

    let beats_per_bar = {
        let ts = chart.song.time_signature.as_deref().unwrap_or("4/4");
        ts.split('/').next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(4)
    };
    spawn_hud_overlay(&mut commands, chart, &chords, key, &font, chart.song.tempo_bpm, beats_per_bar);
    spawn_countdown(&mut commands, &font);
}

fn spawn_harmonica_3d(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    model_name: &str,
    config: &HarmonicaModelConfig,
) {
    let [tx, ty, tz] = config.model_translation;

    // Everything that should "dance" lives under one parent. `groove_harmonica`
    // nudges this parent's Transform; children (model + holes) inherit the motion
    // so the hole overlays stay glued to the model face.
    commands
        .spawn((
            Transform::default(),
            Visibility::default(),
            HarmonicaGroove,
            GameplayRoot,
        ))
        .with_children(|groove| {
            groove.spawn((
                WorldAssetRoot(
                    asset_server.load(format!("harmonicas/3d/{model_name}/harmonica.glb#Scene0")),
                ),
                Transform::from_xyz(tx, ty, tz)
                    .with_rotation(Quat::from_rotation_y(
                        config.model_rotation_y_deg.to_radians(),
                    ))
                    .with_scale(Vec3::splat(config.model_scale)),
            ));

            for (i, hole_cfg) in config.holes.iter().enumerate() {
                let hole = (i + 1) as u8;
                let hole_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.10, 0.11, 0.15),
                    emissive: LinearRgba::new(0.0, 0.0, 0.0, 0.0),
                    metallic: 0.3,
                    perceptual_roughness: 0.6,
                    ..default()
                });
                let hole_mesh = meshes.add(Cuboid::new(hole_cfg.w, hole_cfg.h, hole_cfg.d));
                let mat_handle = hole_mat.clone();
                groove.spawn((
                    Mesh3d(hole_mesh),
                    MeshMaterial3d(hole_mat),
                    Transform::from_xyz(hole_cfg.x, hole_cfg.y, hole_cfg.z),
                    HoleCell(hole),
                    HoleState::default(),
                    HoleMesh3D(mat_handle),
                ));
            }
        });
}

fn spawn_hud_overlay(
    commands: &mut Commands,
    chart: &crate::song::chart::HarpChart,
    chords: &[String],
    key: &str,
    font: &FontSource,
    bpm: f32,
    beats_per_bar: usize,
) {
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let info = format!(
        "Key: {}  \u{2669} = {}  {}",
        key,
        chart.song.tempo_bpm as u32,
        chart.song.time_signature.as_deref().unwrap_or("4/4"),
    );
    let harp_info = chart.harmonica.display();
    let description = chart.metadata.as_ref().and_then(|m| m.description.as_deref());
    let chart_author = chart.metadata.as_ref().and_then(|m| m.author.as_deref());

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(10),
            GameplayRoot,
        ))
        .with_children(|p| {
            for (text, size, color) in [
                (title.as_str(), 18.0f32, Color::WHITE),
                (info.as_str(), 12.0, Color::srgb(0.65, 0.70, 0.80)),
                (harp_info.as_str(), 11.0, Color::srgb(0.45, 0.72, 0.55)),
            ] {
                p.spawn((
                    Text::new(text.to_string()),
                    TextFont { font_size: FontSize::Px(size), font: font.clone(), ..default() },
                    TextColor(color),
                ));
            }
            if let Some(desc) = description {
                p.spawn((
                    Text::new(desc.to_string()),
                    TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(0.50, 0.50, 0.55)),
                ));
            }
            if let Some(author) = chart_author {
                p.spawn((
                    Text::new(format!("Chart: {author}")),
                    TextFont { font_size: FontSize::Px(9.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(0.40, 0.40, 0.45)),
                ));
            }

            // Live phrase / groove banner (driven by phrase_overlay::update_phrase)
            spawn_phrase_banner(p, font);

            // 12-bar blues grid
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|grid| {
                spawn_12_bar_grid(grid, chords, key, font, &GridConfig::for_3d());
            });

            // Blow/draw legend
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|leg| {
                leg.spawn((
                    Text::new("\u{25A0} BLOW"),
                    TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(0.50, 0.75, 1.00)),
                ));
                leg.spawn((
                    Text::new("\u{25A0} DRAW"),
                    TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(1.00, 0.62, 0.35)),
                ));
            });

            // Metronome
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|metro| {
                spawn_metronome(metro, beats_per_bar, bpm, font);
            });

            // Technique colour legend
            p.spawn(Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|leg| {
                spawn_modifier_legend(leg, font);
            });
        });

    // Score panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                right: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(20),
            GameplayRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("0"),
                TextFont { font_size: FontSize::Px(30.0), font: font.clone(), ..default() },
                TextColor(Color::WHITE),
                ScoreText,
            ));
            p.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(15.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.90, 0.72, 0.20)),
                ComboText,
            ));
            p.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(22.0), font: font.clone(), ..default() },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                FeedbackText,
            ));
        });

}

// ── Per-frame systems ─────────────────────────────────────────────────────────

/// Deterministic pseudo-random in -1..1 from an integer-ish input. The classic
/// `fract(sin(x) * big)` hash — repeatable per beat, so the groove is stable but
/// looks improvised.
fn hash11(n: f32) -> f32 {
    let x = (n * 127.1).sin() * 43758.547;
    x.fract() * 2.0 - 1.0
}

/// Sways the harmonica a few millimeters in all directions, in time with the
/// song's tempo, so it looks like it's grooving to the music. The motion has a
/// blues shuffle: a triplet bounce, a backbeat accent, and per-beat randomness
/// so it never settles into a metronomic rocking-chair arc.
pub fn groove_harmonica(
    clock: Res<super::GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut groove: Query<&mut Transform, With<HarmonicaGroove>>,
) {
    use std::f32::consts::{PI, TAU};

    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    // Hold still during the countdown (clock is negative); only dance once the
    // music has started.
    if clock.0 < 0.0 {
        for mut tf in &mut groove {
            tf.translation = Vec3::ZERO;
            tf.rotation = Quat::IDENTITY;
        }
        return;
    }

    let bpm = manifest.chart.song.tempo_bpm.max(1.0);
    // Beats elapsed (fractional).
    let beat = (clock.0 / (60.0 / bpm as f64)) as f32;
    let bi = beat.floor();
    let frac = beat.fract();

    // Per-beat random accent, smoothly interpolated across the beat (smoothstep)
    // so each beat lands a little differently — the "improvised" blues feel.
    let s = frac * frac * (3.0 - 2.0 * frac);
    let accent = hash11(bi) + (hash11(bi + 1.0) - hash11(bi)) * s;

    // Backbeat emphasis on beats 2 & 4 (the blues snare hits), the off-beats get
    // a stronger kick than the downbeats.
    let backbeat = if (bi as i32).rem_euclid(2) == 1 { 1.0 } else { 0.6 };

    // Triplet shuffle: a strong hit on the beat plus a lighter swung hit on the
    // last triplet (the "and-a"). This is what gives the bounce its blues swing
    // instead of an even, sea-saw oscillation.
    let shuffle = (beat * TAU).sin() * 0.7 + (beat * TAU * 1.5).sin() * 0.3;
    let bob = shuffle * backbeat * (0.6 + 0.4 * accent);

    // Quasi-periodic sway/nod: layering sines at incommensurate (non-integer)
    // ratios means they never line up the same way twice, so the side-to-side and
    // fore/aft never repeat into a clean rocking arc. The accent nudges them too.
    let sway = (beat * PI).sin() * 0.6 + (beat * PI * 0.37).sin() * 0.25 + accent * 0.35;
    let nod = (beat * PI * 0.73 + PI * 0.25).cos() * 0.6 + (beat * TAU * 0.21).sin() * 0.4;

    for mut tf in &mut groove {
        tf.translation = Vec3::new(sway * 0.03, bob * 0.022, nod * 0.018);
        tf.rotation = Quat::from_rotation_z(sway * 0.018)
            * Quat::from_rotation_x(nod * 0.012)
            * Quat::from_rotation_y(accent * 0.012);
    }
}

pub fn update_notes_3d(
    clock: Res<super::GameplayClock>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notes: Query<(
        &NoteVisual3D,
        &ScheduledNote,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
    )>,
) {
    let elapsed = clock.0;
    for (note, scheduled, mat_handle, mut tf) in &mut notes {
        let remaining = (note.time - elapsed) as f32;
        let z = HIT_Z - remaining / LOOKAHEAD as f32 * LANE_DEPTH - note.depth * 0.5;
        tf.translation.z = z;

        if scheduled.hit {
            if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                mat.emissive = LinearRgba::new(2.5, 2.0, 0.3, 1.0);
                mat.base_color = Color::srgb(1.0, 0.9, 0.3);
            }
        }
    }
}

pub fn update_holes_3d(
    time: Res<Time>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    targets: Res<ActiveTargets>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cells: Query<(&HoleCell, &HoleMesh3D, &mut HoleState)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let dt = time.delta_secs();

    let attack = 1.0 - (-dt * 25.0_f32).exp();
    let decay = 1.0 - (-dt * 4.0_f32).exp();

    let harp_pitches: Vec<&crate::pitch_detect::PitchInfo> = active
        .0
        .iter()
        .filter(|p| valid_notes.0.contains(&format!("{}{}", p.note, p.octave)))
        .collect();

    for (cell, hole_mat, mut state) in &mut cells {
        let blow = chart.harmonica.wind_direction_label(cell.0, &Action::Blow);
        let draw = chart.harmonica.wind_direction_label(cell.0, &Action::Draw);

        let mut blow_hit = false;
        let mut draw_hit = false;
        for p in &harp_pitches {
            let name = format!("{}{}", p.note, p.octave);
            if name == blow {
                blow_hit = true;
            }
            if name == draw {
                draw_hit = true;
            }
        }

        let hint = targets
            .0
            .iter()
            .find(|(h, _)| *h == cell.0)
            .map(|(_, b)| *b);
        let hint_floor = if hint.is_some() { 0.18f32 } else { 0.0 };

        let (target, is_blow) = if blow_hit {
            (1.0f32, true)
        } else if draw_hit {
            (1.0f32, false)
        } else if let Some(is_blow_hint) = hint {
            (hint_floor, is_blow_hint)
        } else {
            (0.0f32, state.is_blow)
        };

        if blow_hit || draw_hit {
            state.is_blow = is_blow;
        }

        let factor = if target > state.brightness { attack } else { decay };
        state.brightness += (target - state.brightness) * factor;
        let b = state.brightness;

        if let Some(mut mat) = materials.get_mut(&hole_mat.0) {
            if state.is_blow {
                mat.emissive =
                    LinearRgba::new(0.05 + 0.15 * b, 0.10 + 0.50 * b, 0.10 + 2.0 * b, 1.0);
                mat.base_color = Color::srgb(0.05 + 0.20 * b, 0.08 + 0.40 * b, 0.08 + 0.75 * b);
            } else {
                mat.emissive = LinearRgba::new(0.05 + 2.0 * b, 0.05 + 0.40 * b, 0.02, 1.0);
                mat.base_color =
                    Color::srgb(0.08 + 0.78 * b, 0.06 + 0.25 * b, (0.08 - 0.04 * b).max(0.0));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swept_mesh_has_expected_topology() {
        // 16 segments, non-indexed triangle soup: per segment 4 faces * 2 tris *
        // 3 verts = 24, plus 2 caps * 2 tris * 3 = 12.
        let mesh = swept_note_mesh(0.8, 0.18, 4.0, 16, |_| (0.0, 0.0));
        let verts = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
        assert_eq!(verts, 16 * 24 + 12);

        // Flat normals require a non-indexed mesh, and must be populated.
        assert!(mesh.indices().is_none());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    }

    #[test]
    fn swept_mesh_more_segments_more_vertices() {
        let n = |segs| {
            swept_note_mesh(0.8, 0.18, 4.0, segs, |_| (0.0, 0.0))
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .len()
        };
        assert!(n(48) > n(12));
    }

    #[test]
    fn bend_ease_is_a_centered_sigmoid() {
        // Anchored 0->1, symmetric, with flat plateaus outside the central band.
        assert_eq!(bend_ease(0.0), 0.0);
        assert_eq!(bend_ease(1.0), 1.0);
        assert!((bend_ease(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(bend_ease(0.2), 0.0, "holds flat before the transition");
        assert_eq!(bend_ease(0.8), 1.0, "holds flat after the transition");
        // Monotonic non-decreasing.
        let mut prev = -1.0;
        for i in 0..=20 {
            let v = bend_ease(i as f32 / 20.0);
            assert!(v >= prev - 1e-6);
            prev = v;
        }
    }

    #[test]
    fn bend_arcs_on_y_axis() {
        // A positive bend lifts the body in +Y; a negative bend dips it in -Y.
        let extreme_y = |dy_amp: f32, pick: fn(&[f32]) -> f32| {
            let mesh = swept_note_mesh(0.8, 0.18, 4.0, 16, move |t| (0.0, dy_amp * t * t));
            let pos = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
            if let bevy::render::mesh::VertexAttributeValues::Float32x3(v) = pos {
                pick(&v.iter().map(|p| p[1]).collect::<Vec<_>>())
            } else {
                panic!("expected Float32x3 positions");
            }
        };
        let max = |ys: &[f32]| ys.iter().copied().fold(f32::MIN, f32::max);
        let min = |ys: &[f32]| ys.iter().copied().fold(f32::MAX, f32::min);
        assert!(extreme_y(0.6, max) > extreme_y(0.0, max), "up-bend raises top");
        assert!(extreme_y(-0.6, min) < extreme_y(0.0, min), "down-bend lowers bottom");
    }
}
