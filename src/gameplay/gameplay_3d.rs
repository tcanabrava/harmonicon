// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::{
    app::SelectedSong,
    assets_management::{
        HarmonicaModelConfig, HoleConfig, SelectedHarmonicaModel, SelectedNoteTheme3d,
        ShowNoteNumbers,
    },
    localization::LocalizationExt,
    song::NoteCube3dConfig,
    song::SongManifest,
    song::chart::{Action, HarpChart},
    song::harmonica::twelve_bar,
    theme::{LoadedTheme, TwelveBarColors},
};

use super::adaptive_difficulty::AdaptiveDifficulty;
use super::countdown_overlay::spawn_countdown;
use super::gameplay_2d::{note_anim_mode, note_techniques};
use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::{build_legend_materials, spawn_modifier_legend};
use super::note_tail_2d::{NoteTail2dMaterial, tail_params};
use super::note_tail_3d::NoteTail3dMaterial;
use super::phrase_overlay::{spawn_phrase_banner, spawn_tab_ribbon};
use super::song_progress_overlay::{BAR_HEIGHT, NoteMarker, spawn_song_progress};
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, ComboText, FeedbackText, GameplayRoot, HoleCell,
    HoleState, LOOKAHEAD, MusicStarted, ScheduledNote, ScoreText, ValidHarpNotes,
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
    /// Index into `SongNotes::notes` — see the doc comment on the 2D
    /// `NoteVisual`, which this mirrors. `head_depth`/`tail_len` are cheap to
    /// recompute on demand from `NoteRenderAssets3D` + the note's own
    /// `hole`/`duration`, so there's nothing else this needs to carry.
    note_id: usize,
}

/// A hole-number label tracking a 3D note (`ShowNoteNumbers` on). 3D notes
/// are opaque meshes with nowhere to put a number *on* them the way the 2D
/// head can, so this is a separate UI `Text` entity, positioned every frame
/// by projecting `target`'s world position through the gameplay camera
/// (`update_note_hole_labels_3d`) rather than living in the note's own
/// entity hierarchy — UI layout doesn't propagate through 3D `Transform`
/// parents. Despawns itself once `target` no longer exists (the note scrolled
/// past and was recycled), so nothing needs to reach back into this entity
/// from `update_notes_3d`.
#[derive(Component)]
pub(super) struct NoteHoleLabel3D {
    target: Entity,
}

/// Chart-level (not per-note) 3D rendering config `spawn_visible_notes_3d`
/// needs once a note's `LOOKAHEAD` window arrives — set once at song load.
#[derive(Resource, Default)]
pub(super) struct NoteRenderAssets3D {
    head_mesh: Option<Handle<Mesh>>,
    cfg: Option<NoteCube3dConfig>,
    /// Per-hole x-position/width from the harmonica model's `holes.json`
    /// (falls back to `lane_x`/an even width when a hole has no entry).
    holes: Vec<HoleConfig>,
    hole_count: u8,
}

/// `(note_w, head_depth, tail_len)` for `hole`/`duration` — everything
/// `spawn_visible_notes_3d` (at spawn) and `update_notes_3d` (every frame,
/// for positioning/recycling) need beyond what's already on `ScheduledNote`.
/// Recomputed on demand rather than cached on the entity, since it only
/// depends on data that's already cheap to look up: the hole's configured
/// width and the note's own duration.
fn note_dimensions(assets: &NoteRenderAssets3D, hole: u8, duration: f64) -> (f32, f32, f32) {
    let hole_cfg = assets.holes.get(hole.saturating_sub(1) as usize);
    let note_w = hole_cfg.map(|h| h.w).unwrap_or(LANE_WIDTH - LANE_GAP);
    let head_scale = note_w * assets.cfg.as_ref().map(|c| c.head_scale).unwrap_or(1.0);
    let head_depth = head_scale * 1.4;
    let tail_len = note_depth(duration);
    (note_w, head_depth, tail_len)
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

/// `hole_count` comes from the loaded chart's harmonica (10 for diatonic,
/// more for chromatic) — not a fixed constant, so lanes/notes land in the
/// right place regardless of harmonica type.
fn lane_x(hole: u8, hole_count: u8) -> f32 {
    (hole as f32 - 1.0) * LANE_WIDTH - (hole_count as f32 * LANE_WIDTH) / 2.0 + LANE_WIDTH * 0.5
}

fn note_depth(duration: f64) -> f32 {
    ((duration as f32 / LOOKAHEAD as f32) * LANE_DEPTH).clamp(0.4, 12.0)
}

// ── Harmonica model config ────────────────────────────────────────────────────

/// The fallback layout when a model has no `holes.json`: holes evenly spaced
/// across the lanes at the harmonica's resting position, sized to the
/// chart's actual hole count. (No bundled 3D model currently ships a
/// chromatic `holes.json`, so a chromatic chart's *note lanes* line up
/// correctly even though the harmonica prop itself still renders as
/// whichever diatonic model is selected — that needs a matching 3D asset,
/// not just code.)
fn default_model_layout(hole_count: u8) -> HarmonicaModelConfig {
    HarmonicaModelConfig {
        model_translation: [0.0, LANE_Y + 0.45, HARP_Z],
        model_rotation_y_deg: 0.0,
        model_scale: 1.0,
        holes: (1u8..=hole_count)
            .map(|hole| HoleConfig {
                x: lane_x(hole, hole_count),
                y: LANE_Y + 0.9 + 0.10,
                z: HARP_Z,
                w: LANE_WIDTH - LANE_GAP - 0.08,
                h: 0.20,
                d: 0.90,
            })
            .collect(),
    }
}

fn load_model_config(model_name: &str, hole_count: u8) -> HarmonicaModelConfig {
    let path = format!("assets/harmonicas/3d/{model_name}/holes.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| {
            warn!("No holes.json for model '{model_name}', using default layout");
            default_model_layout(hole_count)
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
/// Builds every note's score state (`SongNotes`) plus the render config
/// `spawn_visible_notes_3d` needs (`NoteRenderAssets3D`) — no entities yet.
/// Notes are spawned lazily, in a `LOOKAHEAD` window around the playhead,
/// mirroring the 2D highway (`gameplay_2d::spawn_visible_notes`).
fn build_song_notes_3d(
    chart: &HarpChart,
    head_mesh: Handle<Mesh>,
    cfg: NoteCube3dConfig,
    holes: Vec<HoleConfig>,
    adaptive: &AdaptiveDifficulty,
) -> (super::SongNotes, NoteRenderAssets3D) {
    let (notes, _) = super::build_scheduled_notes(chart, adaptive);
    let hole_count = chart.harmonica.hole_count();
    (
        super::SongNotes { notes, cursor: 0 },
        NoteRenderAssets3D {
            head_mesh: Some(head_mesh),
            cfg: Some(cfg),
            holes,
            hole_count,
        },
    )
}

/// Rebuilds `SongNotes` whenever `AdaptiveDifficulty` changes while a 3D
/// song is loaded — e.g. the pause menu's manual phrase override — so
/// unlocking/relocking notes takes effect immediately instead of only on
/// the next Restart. Score state (hit/missed/held/sustain_scored/pitch/amp
/// samples) carries over for notes that still exist in the rebuilt list
/// (matched by `(time, hole, is_blow)` — stable across a rebuild since both
/// lists derive from the same chart); newly unlocked notes start fresh.
/// `NoteRenderAssets3D` doesn't need touching — nothing about it depends on
/// which notes are unlocked.
///
/// Every current `NoteVisual3D` is despawned unconditionally rather than
/// reconciled in place: its `note_id` is a *positional* index into
/// `SongNotes::notes`, and the rebuild can shift that position for every
/// note after the edited phrase — a surviving entity would otherwise end up
/// rendering a different note's data under its old index.
/// `spawn_visible_notes_3d` re-spawns everything within `LOOKAHEAD` fresh
/// next frame, using the corrected indices.
pub(super) fn resync_notes_on_adaptive_change(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    adaptive: Res<AdaptiveDifficulty>,
    mut song_notes: ResMut<super::SongNotes>,
    visuals: Query<Entity, With<NoteVisual3D>>,
) {
    if !adaptive.is_changed() {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    super::adaptive_difficulty::rebuild_song_notes(&manifest.chart, &adaptive, &mut song_notes);
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
}

/// Spawns 3D note visuals for any note newly within the `LOOKAHEAD` window.
/// Self-healing across a loop wrap, same as the 2D version: no persistent
/// spawn cursor, just "is this note's window open, and does it already have
/// a visual" recomputed each frame.
pub fn spawn_visible_notes_3d(
    mut commands: Commands,
    clock: Res<super::GameplayClock>,
    song_notes: Res<super::SongNotes>,
    render_assets: Res<NoteRenderAssets3D>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tail_materials: ResMut<Assets<NoteTail3dMaterial>>,
    existing: Query<&NoteVisual3D>,
    show_numbers: Res<ShowNoteNumbers>,
) {
    if render_assets.head_mesh.is_none() {
        return;
    }
    let elapsed = clock.get();
    let already_spawned: HashSet<usize> = existing.iter().map(|v| v.note_id).collect();
    for i in super::notes_needing_spawn(&song_notes.notes, &already_spawned, elapsed) {
        spawn_note_visual_3d(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut tail_materials,
            &render_assets,
            i,
            &song_notes.notes[i],
            show_numbers.0,
        );
    }
}

/// A note's base (un-hit, un-missed) blow/draw appearance: `(r, g, b,
/// emissive_r, emissive_g, emissive_b)`. Shared by `spawn_note_visual_3d` and
/// `update_note_visuals_3d` so the two can't drift out of sync.
fn note_base_appearance(is_blow: bool) -> (f32, f32, f32, f32, f32, f32) {
    if is_blow {
        (0.25, 0.55, 0.95, 0.1, 0.3, 1.2)
    } else {
        (0.95, 0.38, 0.15, 1.2, 0.2, 0.05)
    }
}

/// Spawns one note as a 3D comet: an elongated cube head (from the theme's
/// glTF) tinted by blow/draw colour, trailing a flat ribbon that runs the
/// technique's animation via [`NoteTail3dMaterial`] — the 3D twin of the 2D
/// head+tail comet. Positioned once here (holes don't move); `update_notes_3d`
/// drives the Z position every frame.
fn spawn_note_visual_3d(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tail_materials: &mut Assets<NoteTail3dMaterial>,
    assets: &NoteRenderAssets3D,
    note_id: usize,
    note: &ScheduledNote,
    show_numbers: bool,
) {
    let head_mesh = assets.head_mesh.as_ref().expect("checked by caller");
    let cfg = assets.cfg.as_ref().expect("checked by caller");
    let (r, g, b, emit_r, emit_g, emit_b) = note_base_appearance(note.is_blow);

    let hole_cfg = assets.holes.get(note.hole.saturating_sub(1) as usize);
    let note_x = hole_cfg
        .map(|h| h.x)
        .unwrap_or_else(|| lane_x(note.hole, assets.hole_count));
    let (note_w, head_depth, tail_len) = note_dimensions(assets, note.hole, note.duration);

    // Head: the elongated cube (1.4 units long in Z), tinted blow/draw.
    let head_scale = note_w * cfg.head_scale;
    let head_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(r, g, b),
        emissive: LinearRgba::new(emit_r, emit_g, emit_b, 1.0),
        ..default()
    });

    // Tail: a flat ribbon driven by the same technique animation as 2D.
    let (vib, shift, wah) = note_techniques(Some(&note.modifiers));
    let mode = note_anim_mode(Some(&note.modifiers));
    let (mut params, mut wah_v) = tail_params(20.0, vib, shift, wah);
    params.z = 0.0; // animation clock, set each frame
    wah_v.z = mode; // which technique animation
    wah_v.w = note_id as f32 * 1.7; // per-note phase
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

    let note_entity = commands
        .spawn((
            Transform::from_xyz(note_x, LANE_Y + NOTE_H * 0.5, FAR_Z),
            NoteVisual3D { note_id },
            GameplayRoot,
        ))
        .with_children(|note_e| {
            // Cube head at the leading edge (parent origin).
            note_e.spawn((
                Mesh3d(head_mesh.clone()),
                MeshMaterial3d(head_mat),
                Transform::from_scale(Vec3::splat(head_scale)),
                NoteHead3d,
            ));
            // Tail ribbon trailing behind the head (−Z), flat over the lane.
            note_e.spawn((
                Mesh3d(tail_mesh),
                MeshMaterial3d(tail_mat),
                Transform::from_xyz(
                    0.0,
                    -NOTE_H * 0.5 + 0.02,
                    -(head_depth * 0.5 + tail_len * 0.5),
                ),
                NoteTail3d,
            ));
        })
        .id();

    // Hole-number label: a separate UI entity (see `NoteHoleLabel3D`'s doc
    // comment for why), positioned every frame by `update_note_hole_labels_3d`
    // — hidden until then, since it starts at the origin.
    if show_numbers {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                Visibility::Hidden,
                NoteHoleLabel3D {
                    target: note_entity,
                },
                GameplayRoot,
            ))
            .with_children(|l| {
                l.spawn((
                    // `+`/`-` for blow/draw — see the matching comment in
                    // `gameplay_2d::spawn_note_visual`.
                    Text::new(super::phrase_overlay::tab_label(
                        note.hole,
                        note.is_blow,
                        &[],
                    )),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
    }
}

/// Offset (logical px) from a note's projected screen position to where its
/// hole-number label's top-left corner should land — up and to the left, so
/// the label sits just outside the note instead of covering it.
const NOTE_LABEL_OFFSET: Vec2 = Vec2::new(-24.0, -20.0);

/// Converts a `Camera::world_to_viewport` result into the `Val::Px` a UI
/// `Node`'s `left`/`top` needs, offset to the label's anchor point.
/// `world_to_viewport` already resolves through `logical_viewport_rect()`,
/// so its result is in the same logical-window-pixel space `Val::Px` is —
/// *except* that bevy_ui additionally multiplies every `Val::Px` by
/// [`UiScale`] before converting to physical pixels
/// (`propagate_ui_target_cameras` in `bevy_ui`), a multiplier the camera
/// projection knows nothing about. Dividing by `ui_scale` here cancels that
/// back out, so the label lands under the note regardless of the player's
/// UI zoom level (`dialogs::ui_scale`, arrow keys).
fn note_label_position(viewport_px: Vec2, ui_scale: f32) -> Vec2 {
    viewport_px / ui_scale + NOTE_LABEL_OFFSET
}

/// Positions each [`NoteHoleLabel3D`] over its target note's current screen
/// position, or hides it once the note is behind the camera, or despawns it
/// once the note itself is gone (scrolled past and recycled).
///
/// Reads the note's local `Transform`, not `GlobalTransform`: `update_notes_3d`
/// (earlier in the same `Update` chain) writes `Transform.translation.z` every
/// frame, but `GlobalTransform` propagation only runs afterward, in
/// `PostUpdate` — reading it here would always be one frame stale, which
/// reads as the label trailing behind its note. Note root entities have no
/// transform parent, so the local `Transform` already *is* world space; no
/// propagation to wait on.
pub fn update_note_hole_labels_3d(
    mut commands: Commands,
    camera: Query<(&Camera, &GlobalTransform), With<GameplayCamera3D>>,
    ui_scale: Res<UiScale>,
    notes: Query<&Transform, With<NoteVisual3D>>,
    mut labels: Query<(Entity, &NoteHoleLabel3D, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };

    for (entity, label, mut node, mut visibility) in &mut labels {
        let Ok(note_transform) = notes.get(label.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(camera_transform, note_transform.translation) {
            Ok(viewport_px) => {
                let pos = note_label_position(viewport_px, ui_scale.0);
                node.left = Val::Px(pos.x);
                node.top = Val::Px(pos.y);
                // Guarded so change detection (and the visibility-propagation
                // it triggers) doesn't fire every frame for every label while
                // nothing about their visibility actually changed.
                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
            }
            Err(_) => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

/// Note-building state bundled into one `SystemParam` so `setup` stays under
/// Bevy's function-system parameter arity limit — plain individual params
/// would put it one over once `AdaptiveDifficulty` joined the list.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct NoteBuildState<'w> {
    valid_notes: ResMut<'w, ValidHarpNotes>,
    song_notes: ResMut<'w, super::SongNotes>,
    render_assets: ResMut<'w, NoteRenderAssets3D>,
    adaptive: Res<'w, AdaptiveDifficulty>,
}

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut note_build: NoteBuildState,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    selected_model: Res<SelectedHarmonicaModel>,
    shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
    note_theme: Res<SelectedNoteTheme3d>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let Some(manifest): Option<&SongManifest> = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing (3D) state");
        return;
    };
    clock.set_free(-COUNTDOWN);
    music_started.0 = false;
    note_build.valid_notes.0 = manifest.chart.harmonica.build_valid_notes();

    for (mut cam, _) in &mut cameras {
        cam.order = 1;
        cam.clear_color = ClearColorConfig::None;
    }

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let chords = twelve_bar(key);
    let model_cfg = load_model_config(&selected_model.0, chart.harmonica.hole_count());

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
        Some(path) => asset_server.load(path.clone().with_label("Mesh0/Primitive0")),
        None => asset_server.load(format!("notes/3d/{}.glb#Mesh0/Primitive0", note_theme.0)),
    };
    let note_cfg = manifest.assets_3d_config.clone();
    let (notes, assets) = build_song_notes_3d(
        chart,
        head_mesh,
        note_cfg,
        holes.clone(),
        &note_build.adaptive,
    );
    *note_build.song_notes = notes;
    *note_build.render_assets = assets;
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
        chart.song.tempo_bpm,
        beats_per_bar,
        shape_materials,
        theme.twelve_bar_colors(),
        &loc,
    );
    let note_markers: Vec<NoteMarker> = note_build
        .song_notes
        .notes
        .iter()
        .map(|n| NoteMarker {
            time: n.time,
            duration: n.duration,
            is_blow: n.is_blow,
        })
        .collect();
    spawn_song_progress(
        &mut commands,
        &manifest.waveform,
        manifest.music_duration_secs,
        &note_markers,
        &note_build.adaptive.sections,
        &note_build.adaptive.learned,
    );
    super::wait_freeze_overlay::spawn_wait_freeze_prompt(&mut commands);
    let harp_hint = crate::song::harmonica::harp_banner(&chart.harmonica, key);
    spawn_countdown(&mut commands, &loc, Some(&harp_hint));
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

    bpm: f32,
    beats_per_bar: usize,
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
    twelve_bar_colors: TwelveBarColors,
    loc: &Localization,
) {
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let info = String::from(
        loc.msg_args(
            "gameplay-chart-info",
            &[
                ("key", key.to_string()),
                ("bpm", (chart.song.tempo_bpm as u32).to_string()),
                (
                    "time_sig",
                    chart
                        .song
                        .time_signature
                        .as_deref()
                        .unwrap_or("4/4")
                        .to_string(),
                ),
            ],
        ),
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
                // Below the song-progress bar (`BAR_HEIGHT`, pinned at the very
                // top across the full width and always painted above the HUD —
                // see `BAR_Z_INDEX`) so its text is never covered by it.
                top: Val::Px(8.0 + BAR_HEIGHT),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                // Fixed so the panel doesn't grow or shrink with the current
                // song's title/description length — long text wraps instead.
                max_width: Val::Px(420.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(1),
            GameplayRoot,
        ))
        .with_children(|p| {
            for (text, size, color) in [
                (title.as_str(), 18.0f32, Color::WHITE),
                (info.as_str(), 15.0, Color::srgb(0.65, 0.70, 0.80)),
                (harp_info.as_str(), 15.0, Color::srgb(0.45, 0.72, 0.55)),
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
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.50, 0.50, 0.55)),
                ));
            }
            if let Some(author) = chart_author {
                p.spawn((
                    Text::new(String::from(loc.msg_args(
                        "gameplay-chart-author",
                        &[("author", author.to_string())],
                    ))),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.40, 0.40, 0.45)),
                ));
            }

            // Live phrase / groove banner (driven by phrase_overlay::update_phrase)
            spawn_phrase_banner(p);
            // Tab-notation ribbon for the current phrase (phrase_overlay::update_tab_ribbon)
            spawn_tab_ribbon(p);

            // Blow/draw legend
            super::gameplay_2d::spawn_blow_draw_legend(p, loc, 12.0, 4.0);

            // Metronome
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|metro| {
                spawn_metronome(metro, loc, beats_per_bar, bpm);
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
                spawn_modifier_legend(leg, loc, &legend_materials);
            });
        });

    // 12-bar blues grid + score, grouped top-right — clear of the note
    // highway, which sits center-screen, instead of stacked under the song
    // info: the grid's fixed width used to force the info panel above to
    // match it, growing/shrinking the whole panel with whatever else was in
    // it (in particular the song title/description).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0 + BAR_HEIGHT),
                right: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                column_gap: Val::Px(16.0),
                ..default()
            },
            GlobalZIndex(1),
            GameplayRoot,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .with_children(|grid| {
                spawn_12_bar_grid(
                    grid,
                    chords,
                    key,
                    crate::song::harmonica::Progression::Standard,
                    &GridConfig::for_3d(),
                    twelve_bar_colors,
                );
            });

            row.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
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
    if clock.get() < 0.0 {
        for mut tf in &mut groove {
            tf.translation = Vec3::ZERO;
            tf.rotation = Quat::IDENTITY;
        }
        return;
    }

    let bpm = manifest.chart.song.tempo_bpm.max(1.0);
    // Beats elapsed (fractional).
    let beat = (clock.get() / (60.0 / bpm as f64)) as f32;
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
    song_notes: Res<super::SongNotes>,
    render_assets: Res<NoteRenderAssets3D>,
    mut commands: Commands,
    mut notes: Query<(Entity, &NoteVisual3D, &mut Transform)>,
) {
    let elapsed = clock.get();
    for (entity, visual, mut tf) in &mut notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let (_, head_depth, tail_len) = note_dimensions(&render_assets, note.hole, note.duration);
        let remaining = (note.time - elapsed) as f32;
        // The head's front face lands on the hit line at the note's time.
        let z = HIT_Z - remaining / LOOKAHEAD as f32 * LANE_DEPTH - head_depth * 0.5;
        // Recycle once the whole comet (head + trailing tail) has passed the
        // hit zone. Score state lives independently in `SongNotes` now, so
        // this despawns unconditionally even while looping —
        // `spawn_visible_notes_3d` respawns it once the (rewound) clock
        // nears it again, with no state to lose.
        if z > HIT_Z + head_depth * 0.5 + tail_len + 4.0 {
            commands.entity(entity).despawn();
            continue;
        }
        tf.translation.z = z;
    }
}

/// Head/emissive/tail appearance for a 3D note visual: gold while hit, dim
/// red while missed, otherwise its base blow/draw appearance
/// ([`note_base_appearance`]). Pulled out of `update_note_visuals_3d` so the
/// tint decision is unit-testable without spinning up rendering — mirrors
/// [`gameplay_2d::note_tint`].
fn note_tint_3d(hit: bool, missed: bool, is_blow: bool) -> (Color, LinearRgba, LinearRgba) {
    if hit {
        (
            Color::srgb(1.0, 0.9, 0.3),
            LinearRgba::new(2.5, 2.0, 0.3, 1.0),
            Color::srgba(1.0, 0.85, 0.25, 0.95).to_linear(),
        )
    } else if missed {
        (
            Color::srgb(0.4, 0.12, 0.12),
            LinearRgba::new(0.2, 0.05, 0.05, 1.0),
            Color::srgba(0.5, 0.13, 0.13, 0.6).to_linear(),
        )
    } else {
        let (r, g, b, emit_r, emit_g, emit_b) = note_base_appearance(is_blow);
        (
            Color::srgb(r, g, b),
            LinearRgba::new(emit_r, emit_g, emit_b, 1.0),
            Color::srgba(r, g, b, 0.9).to_linear(),
        )
    }
}

/// Tints a 3D note's cube head and tail ribbon when it is hit or missed —
/// gold on a hit, dim red on a miss — mirroring the 2D path, and restores
/// the base blow/draw appearance otherwise (see [`note_tint_3d`]).
/// `ScheduledNote` isn't an ECS component (score state lives in
/// `SongNotes`), so this re-syncs every currently-spawned note's tint each
/// frame rather than reacting to `Changed<ScheduledNote>` — cheap since only
/// a `LOOKAHEAD` window's worth of notes are ever spawned.
pub fn update_note_visuals_3d(
    song_notes: Res<super::SongNotes>,
    notes: Query<(&NoteVisual3D, &Children)>,
    heads: Query<&MeshMaterial3d<StandardMaterial>, With<NoteHead3d>>,
    tails: Query<&MeshMaterial3d<NoteTail3dMaterial>, With<NoteTail3d>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut tail_materials: ResMut<Assets<NoteTail3dMaterial>>,
) {
    for (visual, children) in &notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let (base, emissive, tail_color) = note_tint_3d(note.hit, note.missed, note.is_blow);
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
    let t = clock.get() as f32;
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
    let harp_pitches = super::gameplay_2d::harp_pitches(&active, &valid_notes);

    for (cell, hole_mat, mut state) in &mut cells {
        let blow = chart.harmonica.wind_direction_midi(cell.0, &Action::Blow);
        let draw = chart.harmonica.wind_direction_midi(cell.0, &Action::Draw);
        let hint = targets
            .0
            .iter()
            .find(|(h, _)| *h == cell.0)
            .map(|(_, b)| *b);

        super::gameplay_2d::step_hole_glow(
            &mut state,
            blow,
            draw,
            hint,
            &harp_pitches,
            attack,
            decay,
        );
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
mod tests;
