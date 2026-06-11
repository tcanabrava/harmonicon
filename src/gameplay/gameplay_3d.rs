use bevy::{audio::AudioSource, prelude::*};

use crate::{
    assets_management::{GlobalFonts, SelectedHarmonicaModel},
    menu::SelectedSong,
    song::SongManifest,
    song::chart::Action,
    song::harmonica::{semitone, twelve_bar},
};

use super::{
    ActivePitches, ActiveTargets, ComboText, CountdownOverlay, CountdownText, FeedbackText,
    GameplayRoot, HoleCell, HoleState, MusicPlayer, MusicStarted, ScoreText, ScheduledNote,
    ScoringConfig, ValidHarpNotes, COUNTDOWN, HOLE_COUNT, LOOKAHEAD,
    secs_per_bar, current_bar_index,
};

// ── 3D layout constants ───────────────────────────────────────────────────────

const LANE_WIDTH: f32 = 1.0;
const LANE_GAP: f32 = 0.06;
const LANE_DEPTH: f32 = 60.0;
const HIT_Z: f32 = 6.0;
const FAR_Z: f32 = HIT_Z - LANE_DEPTH;          // -54
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

#[derive(Component)]
pub(super) struct BarCell3D(usize);

fn lane_x(hole: u8) -> f32 {
    (hole as f32 - 1.0) * LANE_WIDTH - (HOLE_COUNT as f32 * LANE_WIDTH) / 2.0 + LANE_WIDTH * 0.5
}

fn note_depth(duration: f64) -> f32 {
    ((duration as f32 / LOOKAHEAD as f32) * LANE_DEPTH).clamp(0.4, 12.0)
}

// ── Harmonica model config ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
struct HoleConfig {
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

fn default_model_scale() -> f32 { 1.0 }

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
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing (3D) state");
        return;
    };
    clock.0 = -COUNTDOWN;
    music_started.0 = false;
    valid_notes.0 = manifest.chart.harmonica.build_valid_notes();

    // Make the Camera2d render on top without clearing the 3D scene
    for (mut cam, _) in &mut cameras {
        cam.order = 1;
        cam.clear_color = ClearColorConfig::None;
    }

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let chords = twelve_bar(key);
    let font = fonts.gameplay.clone();
    let model_cfg = load_model_config(&selected_model.0);

    // ── 3D Camera ────────────────────────────────────────────────────────────
    // Camera2d is set to order=1 in this function, so Camera3d at the default
    // order=0 renders first and the 2D HUD composites on top.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 14.0, 24.0)
            .looking_at(Vec3::new(0.0, 0.0, HIT_Z - 18.0), Vec3::Y),
        GameplayCamera3D,
        GameplayRoot,
        Name::new("Camera3d (gameplay 3D)"),
    ));

    // ── Lighting ─────────────────────────────────────────────────────────────
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

    // ── Background image (large unlit quad behind the lanes) ─────────────────
    // Rectangle is in the XY plane; position it upright at Z = FAR_Z - 2 so
    // it fills the horizon visible through the Camera3d.
    let backdrop_mat = materials.add(StandardMaterial {
        base_color_texture: Some(manifest.background.clone()),
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

    // ── Lane geometry derived from holes.json ────────────────────────────────
    let holes = &model_cfg.holes;
    let left_edge   = holes.first().map(|h| h.x - h.w * 0.5).unwrap_or(-5.0);
    let right_edge  = holes.last().map(|h| h.x + h.w * 0.5).unwrap_or(5.0);
    let total_width = right_edge - left_edge;
    let center_x    = (left_edge + right_edge) * 0.5;
    let lane_width = total_width / holes.len() as f32;

    // Extend the track to end at the holes' Z so it meets the harmonica face.
    let track_end_z  = holes.first().map(|h| h.z).unwrap_or(HARP_Z);
    let track_len    = track_end_z - FAR_Z;
    let track_ctr_z  = FAR_Z + track_len * 0.5;

    let lane_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.12),
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

    // Alternating lane shading
    for (i, hole) in holes.iter().enumerate() {
        if i % 2 == 1 { continue; }
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

    // ── Hit zone ─────────────────────────────────────────────────────────────
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

    // ── Note visuals ──────────────────────────────────────────────────────────
    for item in &chart.track {
        let t = item.time.unwrap_or_else(|| {
            let tick = item.tick.unwrap_or(0);
            crate::song::chart::tick_to_seconds(tick, chart.timing.resolution, &chart.timing.tempo_map)
        });
        let depth = note_depth(item.duration);
        for event in &item.events {
            use crate::song::chart::Action;
            let is_blow = matches!(event.action, Action::Blow);
            let (r, g, b, emit_r, emit_g, emit_b) = if is_blow {
                (0.25f32, 0.55, 0.95, 0.1, 0.3, 1.2)
            } else {
                (0.95f32, 0.38, 0.15, 1.2, 0.2, 0.05)
            };
            let note_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                emissive: LinearRgba::new(emit_r, emit_g, emit_b, 1.0),
                ..default()
            });
            let hole_cfg = holes.get(event.hole.saturating_sub(1) as usize);
            let note_x = hole_cfg.map(|h| h.x).unwrap_or_else(|| lane_x(event.hole));
            let note_w = hole_cfg.map(|h| h.w).unwrap_or(LANE_WIDTH - LANE_GAP);
            let note_mesh = meshes.add(Cuboid::new(note_w, NOTE_H, depth));
            let expected_pitch = event.note.clone().unwrap_or_else(|| {
                chart.harmonica.wind_direction_label(event.hole, &event.action)
            });
            // Spawn off-screen; update_notes_3d repositions each frame
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
                    modifiers: event.modifiers.clone().unwrap_or_default(),
                },
                GameplayRoot,
            ));
        }
    }

    // ── Harmonica 3D model ────────────────────────────────────────────────────
    spawn_harmonica_3d(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &selected_model.0,
        &model_cfg,
    );

    // ── 2D HUD overlay (renders via Camera2d on top) ──────────────────────────
    spawn_hud_overlay(
        &mut commands,
        chart,
        &chords,
        key,
        &font,
    );
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
    commands.spawn((
        WorldAssetRoot(asset_server.load(format!("harmonicas/3d/{model_name}/harmonica.glb#Scene0"))),
        Transform::from_xyz(tx, ty, tz)
            .with_rotation(Quat::from_rotation_y(config.model_rotation_y_deg.to_radians()))
            .with_scale(Vec3::splat(config.model_scale)),
        GameplayRoot,
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
        commands.spawn((
            Mesh3d(hole_mesh),
            MeshMaterial3d(hole_mat),
            Transform::from_xyz(hole_cfg.x, hole_cfg.y, hole_cfg.z),
            HoleCell(hole),
            HoleState::default(),
            HoleMesh3D(mat_handle),
            GameplayRoot,
        ));
    }
}

/// Spawns a minimal 2D UI panel in the top-left corner for song info + 12-bar
/// grid. Rendered by Camera2d on top of the 3D scene.
fn spawn_hud_overlay(
    commands: &mut Commands,
    chart: &crate::song::chart::HarpChart,
    chords: &[String],
    key: &str,
    font: &FontSource,
) {
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let info = format!(
        "Key: {}  \u{2669} = {}  {}",
        key,
        chart.song.tempo_bpm as u32,
        chart.song.time_signature.as_deref().unwrap_or("4/4"),
    );
    let harp_info    = chart.harmonica.display();
    let description  = chart.metadata.as_ref().and_then(|m| m.description.as_deref());
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
                (title.as_str(),    18.0f32, Color::WHITE),
                (info.as_str(),     12.0,    Color::srgb(0.65, 0.70, 0.80)),
                (harp_info.as_str(), 11.0,   Color::srgb(0.45, 0.72, 0.55)),
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

            // 12-bar blues grid
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|grid| {
                for row in 0..3usize {
                    grid.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for col in 0..4usize {
                            let idx = row * 4 + col;
                            r.spawn((
                                Node {
                                    width: Val::Px(38.0),
                                    height: Val::Px(26.0),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(bar_bg_3d(idx, key)),
                                BorderColor::all(Color::srgb(0.25, 0.25, 0.38)),
                                BarCell3D(idx),
                            ))
                            .with_children(|cell| {
                                cell.spawn((
                                    Text::new(chords[idx].clone()),
                                    TextFont { font_size: FontSize::Px(12.0), font: font.clone(), ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });
                        }
                    });
                }
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
        });

    // Score panel — top-right corner
    commands.spawn((
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

    // Countdown overlay (full-screen, on top of everything)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.05, 0.55)),
        GlobalZIndex(100),
        CountdownOverlay,
        GameplayRoot,
    ))
    .with_children(|ov| {
        ov.spawn((
            Text::new("GET READY"),
            TextFont { font_size: FontSize::Px(22.0), font: font.clone(), ..default() },
            TextColor(Color::srgba(0.85, 0.85, 1.0, 0.80)),
        ));
        ov.spawn((
            Text::new("3"),
            TextFont { font_size: FontSize::Px(120.0), font: font.clone(), ..default() },
            TextColor(Color::WHITE),
            CountdownText,
        ));
    });
}

fn bar_bg_3d(bar: usize, key: &str) -> Color {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    let chords = twelve_bar(key);
    if chords[bar] == v {
        Color::srgba(0.20, 0.10, 0.14, 0.85)
    } else if chords[bar] == iv {
        Color::srgba(0.10, 0.20, 0.14, 0.85)
    } else {
        Color::srgba(0.10, 0.16, 0.26, 0.85)
    }
}

// ── Per-frame systems ─────────────────────────────────────────────────────────

pub fn update_countdown(
    clock: Res<super::GameplayClock>,
    mut overlay: Query<&mut Visibility, With<CountdownOverlay>>,
    mut text: Query<(&mut Text, &mut TextFont), With<CountdownText>>,
    mut music_started: ResMut<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut commands: Commands,
) {
    if clock.0 >= 0.0 {
        for mut vis in &mut overlay {
            *vis = Visibility::Hidden;
        }
        if !music_started.0 {
            music_started.0 = true;
            if let Some(manifest) = manifests.get(&selected.0) {
                commands.spawn((
                    AudioPlayer::<AudioSource>(manifest.music.clone()),
                    PlaybackSettings::ONCE,
                    MusicPlayer,
                    GameplayRoot,
                ));
            }
        }
        return;
    }

    for mut vis in &mut overlay {
        *vis = Visibility::Visible;
    }

    let remaining = -clock.0;
    let n = remaining.ceil() as u32;
    let frac = remaining.fract() as f32;
    let font_size = 80.0 + (1.0 - frac) * 80.0;

    for (mut t, mut font) in &mut text {
        t.0 = format!("{n}");
        font.font_size = FontSize::Px(font_size);
    }
}

pub fn update_notes_3d(
    clock: Res<super::GameplayClock>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notes: Query<(&NoteVisual3D, &ScheduledNote, &MeshMaterial3d<StandardMaterial>, &mut Transform)>,
) {
    let elapsed = clock.0;
    for (note, scheduled, mat_handle, mut tf) in &mut notes {
        let remaining = (note.time - elapsed) as f32;
        let z = HIT_Z - remaining / LOOKAHEAD as f32 * LANE_DEPTH - note.depth * 0.5;
        tf.translation.z = z;

        // Flash the note gold when it has just been hit
        if scheduled.hit {
            if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                mat.emissive = LinearRgba::new(2.5, 2.0, 0.3, 1.0);
                mat.base_color = Color::srgb(1.0, 0.9, 0.3);
            }
        }
    }
}

pub fn update_bar_3d(
    clock: Res<super::GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    mut cells: Query<(&BarCell3D, &mut BackgroundColor)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let bpm     = manifest.chart.song.tempo_bpm as f64;
    let spb     = secs_per_bar(bpm, config.beats_per_bar);
    let current = current_bar_index(clock.0, spb);
    let key     = manifest.chart.song.key.as_str();

    for (cell, mut bg) in &mut cells {
        *bg = if cell.0 == current {
            BackgroundColor(Color::srgba(0.75, 0.55, 0.08, 0.95))
        } else {
            BackgroundColor(bar_bg_3d(cell.0, key))
        };
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
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let chart = &manifest.chart;
    let dt = time.delta_secs();

    let attack = 1.0 - (-dt * 25.0_f32).exp();
    let decay  = 1.0 - (-dt *  4.0_f32).exp();

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
            if name == blow { blow_hit = true; }
            if name == draw { draw_hit = true; }
        }

        let hint = targets.0.iter().find(|(h, _)| *h == cell.0).map(|(_, b)| *b);
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
                mat.emissive = LinearRgba::new(0.05 + 0.15 * b, 0.10 + 0.50 * b, 0.10 + 2.0 * b, 1.0);
                mat.base_color = Color::srgb(0.05 + 0.20 * b, 0.08 + 0.40 * b, 0.08 + 0.75 * b);
            } else {
                mat.emissive = LinearRgba::new(0.05 + 2.0 * b, 0.05 + 0.40 * b, 0.02, 1.0);
                mat.base_color = Color::srgb(0.08 + 0.78 * b, 0.06 + 0.25 * b, (0.08 - 0.04 * b).max(0.0));
            }
        }
    }
}

/// Called on `OnExit(AppState::Playing)` — restores Camera2d to its normal
/// state so the 2D menu renders correctly after leaving 3D gameplay.
pub fn restore_camera(
    mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>,
) {
    for (mut cam, _) in &mut cameras {
        cam.order = 0;
        cam.clear_color = ClearColorConfig::Default;
    }
}
