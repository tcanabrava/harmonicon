// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use crate::{
    assets_management::{
        GlobalFonts, HarmonicaModelConfig, HoleConfig, SelectedHarmonicaModel, SelectedNoteTheme3d,
    },
    menu::SelectedSong,
    song::NoteCube3dConfig,
    song::SongManifest,
    song::chart::{Action, HarpChart},
    song::harmonica::twelve_bar,
};

use super::countdown_overlay::spawn_countdown;
use super::gameplay_2d::{note_anim_mode, note_techniques};
use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::{build_legend_materials, spawn_modifier_legend};
use super::note_tail_2d::{NoteTail2dMaterial, tail_params};
use super::note_tail_3d::NoteTail3dMaterial;
use super::phrase_overlay::spawn_phrase_banner;
use super::song_progress_overlay::spawn_song_progress;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, ComboText, FeedbackText, GameplayRoot, HOLE_COUNT,
    HoleCell, HoleState, LOOKAHEAD, MusicStarted, ScheduledNote, ScoreText, ValidHarpNotes,
};

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
#[require(Transform, Visibility)]
pub(super) struct NoteVisual3D {
    time: f64,
    /// Z length of the cube head, used to land its front face on the hit line.
    head_depth: f32,
    /// Z length of the trailing tail ribbon, used to recycle once it has passed.
    tail_len: f32,
}

/// The cube head of a 3D note (child). Tinted gold/red on hit/miss.
#[derive(Component)]
pub(super) struct NoteHead3d;

/// The animated tail ribbon of a 3D note (child). Tinted gold/red on hit/miss.
#[derive(Component)]
pub(super) struct NoteTail3d;

#[derive(Component)]
pub(super) struct HoleMesh3D(Handle<StandardMaterial>);

/// Parent entity holding the GLB model and its hole overlays. Animated by
/// `groove_harmonica` so the whole harmonica bobs in time with the music.
#[derive(Component)]
#[require(Transform, Visibility)]
pub(super) struct HarmonicaGroove;

fn lane_x(hole: u8) -> f32 {
    (hole as f32 - 1.0) * LANE_WIDTH - (HOLE_COUNT as f32 * LANE_WIDTH) / 2.0 + LANE_WIDTH * 0.5
}

fn note_depth(duration: f64) -> f32 {
    ((duration as f32 / LOOKAHEAD as f32) * LANE_DEPTH).clamp(0.4, 12.0)
}

// ── Harmonica model config ────────────────────────────────────────────────────

/// The fallback layout when a model has no `holes.json`: holes evenly spaced
/// across the lanes at the harmonica's resting position.
fn default_model_layout() -> HarmonicaModelConfig {
    HarmonicaModelConfig {
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

fn load_model_config(model_name: &str) -> HarmonicaModelConfig {
    let path = format!("assets/harmonicas/3d/{model_name}/holes.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| {
            warn!("No holes.json for model '{model_name}', using default layout");
            default_model_layout()
        })
}

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup_camera_3d(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 14.0, 24.0).looking_at(Vec3::new(0.0, 0.0, HIT_Z - 18.0), Vec3::Y),
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
    // Semi-translucent floor so notes dipping below the lane (downward bends)
    // stay visible. Mesh + material built inline via `asset_value`.
    commands.spawn_scene(bsn! {
        Mesh3d({asset_value(Cuboid::new(total_width, 0.05, track_len))})
        MeshMaterial3d::<StandardMaterial>({asset_value(StandardMaterial {
            base_color: Color::srgba(0.08, 0.08, 0.12, 0.5),
            alpha_mode: AlphaMode::Blend,
            metallic: 0.3,
            perceptual_roughness: 0.8,
            ..default()
        })})
        Transform { translation: {Vec3::new(center_x, LANE_Y - 0.025, track_ctr_z)} }
        GameplayRoot
    });

    // Alternating-lane shading is per-hole (a loop), so it stays imperative.
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

fn create_hit_zone(commands: &mut Commands, center_x: f32, total_width: f32) {
    commands.spawn_scene(bsn! {
        Mesh3d({asset_value(Cuboid::new(total_width, 0.06, 2.8))})
        MeshMaterial3d::<StandardMaterial>({asset_value(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 0.6, 0.15),
            emissive: LinearRgba::new(0.8, 0.8, 0.2, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })})
        Transform { translation: {Vec3::new(center_x, LANE_Y + 0.03, HIT_Z)} }
        GameplayRoot
    });
}

/// Spawns each note as a 3D comet: an elongated cube head (from the theme's glTF)
/// tinted by blow/draw colour, trailing a flat ribbon that runs the technique's
/// animation via [`NoteTail3dMaterial`] — the 3D twin of the 2D head+tail comet.
pub fn create_note_visuals(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    tail_materials: &mut ResMut<Assets<NoteTail3dMaterial>>,
    head_mesh: &Handle<Mesh>,
    cfg: &NoteCube3dConfig,
    holes: &[HoleConfig],
    chart: &HarpChart,
) {
    for item in &chart.track {
        let t = super::resolve_item_time(item, &chart.timing);
        let tail_len = note_depth(item.duration);
        for event in &item.events {
            let is_blow = matches!(event.action, Action::Blow);
            let (r, g, b, emit_r, emit_g, emit_b) = if is_blow {
                (0.25f32, 0.55, 0.95, 0.1, 0.3, 1.2)
            } else {
                (0.95f32, 0.38, 0.15, 1.2, 0.2, 0.05)
            };
            let modifiers = event.modifiers.clone().unwrap_or_default();

            let hole_cfg = holes.get(event.hole.saturating_sub(1) as usize);
            let note_x = hole_cfg.map(|h| h.x).unwrap_or_else(|| lane_x(event.hole));
            let note_w = hole_cfg.map(|h| h.w).unwrap_or(LANE_WIDTH - LANE_GAP);

            // Head: the elongated cube (1.4 units long in Z), tinted blow/draw.
            let head_scale = note_w * cfg.head_scale;
            let head_depth = head_scale * 1.4;
            let head_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                emissive: LinearRgba::new(emit_r, emit_g, emit_b, 1.0),
                ..default()
            });

            // Tail: a flat ribbon driven by the same technique animation as 2D.
            let (vib, shift, wah) = note_techniques(event.modifiers.as_deref());
            let mode = note_anim_mode(event.modifiers.as_deref());
            let (mut params, mut wah_v) = tail_params(20.0, vib, shift, wah);
            params.z = 0.0; // animation clock, set each frame
            wah_v.z = mode; // which technique animation
            wah_v.w = t as f32 * 1.7; // per-note phase
            let tail_mat = tail_materials.add(NoteTail3dMaterial {
                color: Color::srgba(r, g, b, 0.9).to_linear(),
                params,
                wah: wah_v,
            });
            let tail_w = note_w * cfg.tail_width;
            let tail_mesh = meshes.add(Mesh::from(Plane3d::new(
                Vec3::Y,
                Vec2::new(tail_w * 0.5, tail_len * 0.5),
            )));

            let natural_pitch = event.note.clone().unwrap_or_else(|| {
                chart
                    .harmonica
                    .wind_direction_label(event.hole, &event.action)
            });
            // A bend targets the bent pitch, so the technique is scored not shown.
            let expected_pitch = super::target_pitch(&natural_pitch, &modifiers);

            commands
                .spawn((
                    Transform::from_xyz(note_x, LANE_Y + NOTE_H * 0.5, FAR_Z),
                    NoteVisual3D {
                        time: t,
                        head_depth,
                        tail_len,
                    },
                    ScheduledNote {
                        time: t,
                        duration: item.duration,
                        hole: event.hole,
                        is_blow,
                        expected_pitch,
                        hit: false,
                        missed: false,
                        held: 0.0,
                        sustain_scored: false,
                        modifiers,
                    },
                    GameplayRoot,
                ))
                .with_children(|note| {
                    // Cube head at the leading edge (parent origin).
                    note.spawn((
                        Mesh3d(head_mesh.clone()),
                        MeshMaterial3d(head_mat),
                        Transform::from_scale(Vec3::splat(head_scale)),
                        NoteHead3d,
                    ));
                    // Tail ribbon trailing behind the head (−Z), flat over the lane.
                    note.spawn((
                        Mesh3d(tail_mesh),
                        MeshMaterial3d(tail_mat),
                        Transform::from_xyz(
                            0.0,
                            -NOTE_H * 0.5 + 0.02,
                            -(head_depth * 0.5 + tail_len * 0.5),
                        ),
                        NoteTail3d,
                    ));
                });
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
    shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
    mut tail_materials: ResMut<Assets<NoteTail3dMaterial>>,
    note_theme: Res<SelectedNoteTheme3d>,
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
    setup_background(&mut commands, manifest.background.clone());

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
        &mut commands,
        &mut meshes,
        &mut materials,
        total_width,
        lane_width,
        track_len,
        center_x,
        track_ctr_z,
        holes,
    );

    create_hit_zone(&mut commands, center_x, total_width);

    // Comet head mesh + 3D tail layout: loaded here — on entering the 3D game —
    // from the song's own GLB if it ships a `3d/` folder, else the selected
    // theme's default. The handle lives only on the note entities, so it frees
    // when they despawn on leaving the song.
    let head_mesh: Handle<Mesh> = match &manifest.assets_3d {
        Some(path) => asset_server.load(format!("{}#Mesh0/Primitive0", path.to_string_lossy())),
        None => asset_server.load(format!("notes/3d/{}.glb#Mesh0/Primitive0", note_theme.0)),
    };
    let note_cfg = manifest.assets_3d_config.clone();
    create_note_visuals(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut tail_materials,
        &head_mesh,
        &note_cfg,
        holes,
        chart,
    );
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
        ts.split('/')
            .next()
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(4)
    };
    spawn_hud_overlay(
        &mut commands,
        chart,
        &chords,
        key,
        &font,
        chart.song.tempo_bpm,
        beats_per_bar,
        shape_materials,
    );
    spawn_song_progress(&mut commands);
    let harp_hint = crate::song::harmonica::harp_banner(&chart.harmonica, key);
    spawn_countdown(&mut commands, &font, Some(&harp_hint));
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
        .spawn((HarmonicaGroove, GameplayRoot))
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
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
) {
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let info = format!(
        "Key: {}  \u{2669} = {}  {}",
        key,
        chart.song.tempo_bpm as u32,
        chart.song.time_signature.as_deref().unwrap_or("4/4"),
    );
    let harp_info = chart.harmonica.display();
    let description = chart
        .metadata
        .as_ref()
        .and_then(|m| m.description.as_deref());
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
                    TextFont {
                        font_size: FontSize::Px(size),
                                                ..default()
                    },
                    TextColor(color),
                ));
            }
            if let Some(desc) = description {
                p.spawn((
                    Text::new(desc.to_string()),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                                                ..default()
                    },
                    TextColor(Color::srgb(0.50, 0.50, 0.55)),
                ));
            }
            if let Some(author) = chart_author {
                p.spawn((
                    Text::new(format!("Chart: {author}")),
                    TextFont {
                        font_size: FontSize::Px(9.0),
                                                ..default()
                    },
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
                    TextFont {
                        font_size: FontSize::Px(10.0),
                                                ..default()
                    },
                    TextColor(Color::srgb(0.50, 0.75, 1.00)),
                ));
                leg.spawn((
                    Text::new("\u{25A0} DRAW"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                                                ..default()
                    },
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

            // Animated tail previews for the techniques legend (built up front so the UI
            // closures only borrow a ready slice, not the material store).
            let legend_materials = build_legend_materials(&mut shape_materials);

            // Technique colour legend
            p.spawn(Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|leg| {
                spawn_modifier_legend(leg, font, &legend_materials);
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
                TextFont {
                    font_size: FontSize::Px(30.0),
                                        ..default()
                },
                TextColor(Color::WHITE),
                ScoreText,
            ));
            p.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(15.0),
                                        ..default()
                },
                TextColor(Color::srgb(0.90, 0.72, 0.20)),
                ComboText,
            ));
            p.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(22.0),
                                        ..default()
                },
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
    let backbeat = if (bi as i32).rem_euclid(2) == 1 {
        1.0
    } else {
        0.6
    };

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
    loop_cfg: Res<super::LoopConfig>,
    mut commands: Commands,
    mut notes: Query<(Entity, &NoteVisual3D, &mut Transform)>,
) {
    let elapsed = clock.0;
    for (entity, note, mut tf) in &mut notes {
        let remaining = (note.time - elapsed) as f32;
        // The head's front face lands on the hit line at the note's time.
        let z = HIT_Z - remaining / LOOKAHEAD as f32 * LANE_DEPTH - note.head_depth * 0.5;
        // Recycle once the whole comet (head + trailing tail) has passed the hit
        // zone — not while looping, where notes are replayed in place.
        if !loop_cfg.active && z > HIT_Z + note.head_depth * 0.5 + note.tail_len + 4.0 {
            commands.entity(entity).despawn();
            continue;
        }
        tf.translation.z = z;
    }
}

/// Tints a 3D note's cube head and tail ribbon when it is hit or missed — gold on
/// a hit, dim red on a miss — mirroring the 2D path. Reacts only to scoring
/// changes, so it runs the frame the outcome lands.
pub fn update_note_visuals_3d(
    notes: Query<(&ScheduledNote, &Children), Changed<ScheduledNote>>,
    heads: Query<&MeshMaterial3d<StandardMaterial>, With<NoteHead3d>>,
    tails: Query<&MeshMaterial3d<NoteTail3dMaterial>, With<NoteTail3d>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut tail_materials: ResMut<Assets<NoteTail3dMaterial>>,
) {
    for (scheduled, children) in &notes {
        let tint = if scheduled.hit {
            Some((
                Color::srgb(1.0, 0.9, 0.3),
                LinearRgba::new(2.5, 2.0, 0.3, 1.0),
                Color::srgba(1.0, 0.85, 0.25, 0.95).to_linear(),
            ))
        } else if scheduled.missed {
            Some((
                Color::srgb(0.4, 0.12, 0.12),
                LinearRgba::new(0.2, 0.05, 0.05, 1.0),
                Color::srgba(0.5, 0.13, 0.13, 0.6).to_linear(),
            ))
        } else {
            None
        };
        let Some((base, emissive, tail_color)) = tint else {
            continue;
        };
        for child in children {
            if let Ok(h) = heads.get(*child)
                && let Some(mut m) = std_materials.get_mut(&h.0)
            {
                m.base_color = base;
                m.emissive = emissive;
            }
            if let Ok(h) = tails.get(*child)
                && let Some(mut m) = tail_materials.get_mut(&h.0)
            {
                m.color = tail_color;
            }
        }
    }
}

/// Drives every 3D tail's animation clock (`params.z`) from the gameplay clock,
/// so the ribbons flow in time with the song and freeze on pause.
pub fn animate_note_tails_3d(
    clock: Res<super::GameplayClock>,
    mut materials: ResMut<Assets<NoteTail3dMaterial>>,
) {
    let t = clock.0 as f32;
    for (_, material) in materials.iter_mut() {
        material.params.z = t;
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

    let harp_pitches: Vec<&crate::audio_system::pitch_detect::PitchInfo> = active
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

        let factor = if target > state.brightness {
            attack
        } else {
            decay
        };
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
    fn lanes_are_centered_and_ordered() {
        // The ten lanes straddle x = 0 symmetrically.
        assert!((lane_x(1) + lane_x(HOLE_COUNT as u8)).abs() < 1e-6);
        // ...and march left-to-right with hole number.
        assert!(lane_x(2) > lane_x(1));
        assert!(lane_x(HOLE_COUNT as u8) > lane_x(1));
    }

    #[test]
    fn lane_spacing_is_one_lane_width() {
        assert!((lane_x(2) - lane_x(1) - LANE_WIDTH).abs() < 1e-6);
    }

    #[test]
    fn note_depth_scales_with_duration() {
        // Half a lookahead of duration is proportional, inside the clamp band.
        assert!((note_depth(0.3) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn note_depth_is_clamped() {
        assert_eq!(note_depth(0.0), 0.4); // tiny notes keep a visible minimum
        assert_eq!(note_depth(100.0), 12.0); // long notes are capped
    }

    #[test]
    fn leaving_3d_restores_the_2d_camera() {
        // While in 3D the shared Camera2d is pushed behind (order 1) and stops
        // clearing; restore_camera must return it to the menu's defaults.
        let mut world = World::new();
        let cam = world
            .spawn((
                Camera2d,
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                Transform::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(restore_camera);
        schedule.run(&mut world);

        let camera = world.get::<Camera>(cam).unwrap();
        assert_eq!(camera.order, 0);
        assert!(matches!(camera.clear_color, ClearColorConfig::Default));
    }
}
