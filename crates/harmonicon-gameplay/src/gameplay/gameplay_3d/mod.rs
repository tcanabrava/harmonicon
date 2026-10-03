// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use bevy::prelude::*;
use harmonicon_core::chart::{Action, HarpChart};

use harmonicon_app::app::{EffectiveHarmonica, SelectedSong};
use harmonicon_platform::assets_management::ShowNoteNumbers;
use harmonicon_platform::theme::{HUD_PANEL_BG, LoadedTheme, NoteColors, effective_note_colors};
use harmonicon_song::song::SongManifest;
use harmonicon_ui::music_score::{self, BravuraFont};

use super::adaptive_difficulty::AdaptiveDifficulty;
use super::countdown_overlay::spawn_countdown;
use super::gameplay_2d::harp_pitches;
use super::hud_panel::{
    ContextualPanels, HudPanel, LaneSurface, contextual_panels, spawn_hud_panel, used_modifiers,
};
use super::judge::{judged_instant, live_technique_status};
use super::modifier_legend::build_legend_materials;
use super::note_feedback::{
    Judged, JudgedState, hold_uniform, judged_now, judged_scale, judged_stamp,
};
use super::note_ribbon_2d::NoteRibbon2dMaterial;
use super::note_ribbon_3d::NoteRibbon3dMaterial;
use super::song_progress_overlay::{BAR_HEIGHT, NoteMarker, spawn_song_progress};
use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, GameplayRoot, HoleCell, HoleState, LOOKAHEAD,
    MusicStarted, PlayedHarp, ScheduledNote, ScoreReadoutAnchor, SongInfo, ValidHarpNotes,
    spawn_score_readout,
};
use harmonicon_platform::localization::Localization;

// ── 3D layout constants ───────────────────────────────────────────────────────

const LANE_WIDTH: f32 = 1.0;
/// A note's width as a fraction of its lane: wide enough that the head and
/// the tail's technique animation read at a glance, with a gap left so
/// neighbouring lanes stay distinct.
const NOTE_W: f32 = 0.85;
/// How deep the lane runs, in world units, over `LOOKAHEAD` seconds. Short
/// on purpose: with the camera in `scene::setup_camera_3d`, a note's on-screen
/// speed grows about 3.3x from the far end to the hit line (109 to 357 px/s
/// at 1080p). A 60-unit lane seen from a low camera made that 13x — notes
/// crawled at the horizon for two seconds, then crossed half the screen in
/// the last half second.
const LANE_DEPTH: f32 = 16.0;
const HIT_Z: f32 = 6.0;
/// Where the lane's hit plane lands on screen, as a fraction of window height
/// measured up from the bottom. The camera is fixed
/// (`Transform::from_xyz(0.0, 14.0, 16.0)` looking at the lane's origin), so
/// this is a constant of that camera rather than something worth projecting
/// per frame — but it has to be re-measured if the camera ever moves.
const HIT_PLANE_BOTTOM_PCT: f32 = 21.4;
const FAR_Z: f32 = HIT_Z - LANE_DEPTH; // -10
const LANE_Y: f32 = 1.6;
/// Hole pad thickness, and the hit line's height and thickness: with
/// `LANE_Y`, the tops of the solid surfaces a note ribbon passes over.
const PAD_H: f32 = 0.12;
const HIT_LINE_Y: f32 = LANE_Y + 0.07;
const HIT_LINE_H: f32 = 0.08;
/// Height of a note ribbon. It must sit clearly above every surface it
/// crosses: coplanar with the pads' top face, the depth test picked a
/// different winner per pixel each frame and the ribbon flickered as it
/// passed over them.
const RIBBON_Y: f32 = LANE_Y + PAD_H + 0.06;
/// Where the row of hole pads sits: just past the hit zone's near edge, so
/// a pad lights under the note being played.
const PAD_Z: f32 = HIT_Z + 1.9;

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
    pub(super) note_id: usize,
}

/// A hole-number label tracking a 3D note (`ShowNoteNumbers` on). 3D notes
/// are opaque meshes with nowhere to put a number *on* them, so this is a
/// separate UI `Text` entity, positioned every frame by projecting
/// `target`'s world position through the gameplay camera
/// (`update_note_hole_labels_3d`) rather than living in the note's own
/// entity hierarchy — UI layout doesn't propagate through 3D `Transform`
/// parents. Despawns itself once `target` no longer exists (scrolled past
/// and recycled), so nothing needs to reach back into it from
/// `update_notes_3d`.
#[derive(Component)]
pub(super) struct NoteHoleLabel3D {
    target: Entity,
    /// Which side of the ribbon the label sits on — the lane beside it that
    /// stays on the track (`technique_cue::beside_lane`).
    on_right: bool,
}

/// Chart-level (not per-note) 3D rendering config `spawn_visible_notes_3d`
/// needs once a note's `LOOKAHEAD` window arrives — set once at song load.
#[derive(Resource, Default)]
pub(super) struct NoteRenderAssets3D {
    hole_count: u8,
}

/// The tab text inside a [`NoteHoleLabel3D`]. Replaced by a check or cross
/// once the note is judged, as the 2D head label is.
#[derive(Component)]
pub(super) struct NoteHoleLabelText3D;

/// The ribbon that *is* a 3D note (child of its [`NoteVisual3D`], trailing
/// back from the front edge). Its width pops on a hit and narrows on a
/// miss; its material carries the hold state and the gold/red tint.
#[derive(Component)]
pub(super) struct NoteRibbon3d;

#[derive(Component)]
pub(super) struct HoleMesh3D(Handle<StandardMaterial>);

/// Note-building state bundled into one `SystemParam` so `setup` stays under
/// Bevy's function-system parameter arity limit — plain individual params
/// would put it one over once `AdaptiveDifficulty` joined the list.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct NoteBuildState<'w> {
    valid_notes: ResMut<'w, ValidHarpNotes>,
    played_harp: ResMut<'w, PlayedHarp>,
    song_notes: ResMut<'w, super::SongNotes>,
    render_assets: ResMut<'w, NoteRenderAssets3D>,
    adaptive: Res<'w, AdaptiveDifficulty>,
}

/// Display/theming context bundled into one `SystemParam`, same reason as
/// [`NoteBuildState`] — `setup` was one param away from Bevy's arity limit
/// once `CompactLayout` joined the list. Unrelated to note-building, so a
/// separate bundle rather than folding into that one.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct HudContext<'w> {
    loc: Res<'w, Localization>,
    song_info: Res<'w, SongInfo>,
    bravura: Option<Res<'w, BravuraFont>>,
    compact: Res<'w, harmonicon_platform::responsive::CompactLayout>,
}

pub fn setup(
    effective: Res<EffectiveHarmonica>,
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut note_build: NoteBuildState,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shape_materials: ResMut<Assets<NoteRibbon2dMaterial>>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>,
    hud: HudContext,
    lesson: Option<Res<harmonicon_song::lessons::LessonContext>>,
) {
    let compact = hud.compact.0;
    let Some(manifest): Option<&SongManifest> = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing (3D) state");
        return;
    };
    clock.set_free(-COUNTDOWN);
    music_started.0 = false;
    (*note_build.valid_notes, *note_build.played_harp) =
        ValidHarpNotes::for_played_harp(&effective, &manifest.chart);

    for (mut cam, _) in &mut cameras {
        cam.order = 1;
        cam.clear_color = ClearColorConfig::None;
    }

    let chart = &manifest.chart;
    // The instrument on screen is the one the player is holding, as for 2D.
    let played = effective.harp_for(chart);
    let hole_count = played.hole_count();

    setup_camera_3d(&mut commands);
    setup_lighting(&mut commands);
    setup_background(&mut commands, manifest.background.clone());

    // Lanes are the played harp's holes, evenly spaced and centred on x = 0.
    let total_width = f32::from(hole_count) * LANE_WIDTH;
    create_note_track(&mut commands, &mut meshes, &mut materials, hole_count);
    create_hit_zone(&mut commands, total_width);
    spawn_hole_pads(&mut commands, &mut meshes, &mut materials, hole_count);

    let (notes, assets) = build_song_notes_3d(&effective, chart, &note_build.adaptive);
    *note_build.song_notes = notes;
    *note_build.render_assets = assets;

    // The meter's own beat count, for the HUD's beat dots — from the one
    // reading of the chart's meter gameplay has (`bars::chart_meter`).
    let beats_per_bar = usize::from(super::bars::chart_meter(chart).numerator.max(1));
    let modifiers = used_modifiers(chart);
    let lyrics = harmonicon_core::lyrics::lyric_lines(chart);
    let aural = lesson.is_some_and(|lesson| lesson.aural);
    let panels = contextual_panels(
        LaneSurface::Lane3d,
        compact,
        aural,
        !modifiers.is_empty(),
        !lyrics.is_empty(),
    );
    // The same technique coach 2D has, pinned along the bottom under the
    // hole pads — the one place on screen that neither the lane nor the
    // side panel uses.
    if !aural && modifiers.iter().any(super::technique_cue::is_coachable) {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(12.0),
                    left: Val::Percent(18.0),
                    width: Val::Percent(64.0),
                    ..default()
                },
                GlobalZIndex(1),
                GameplayRoot,
            ))
            .with_children(super::technique_coach::spawn_technique_coach);
    }
    spawn_hud_overlay(
        &mut commands,
        &modifiers,
        chart.song.tempo_bpm,
        beats_per_bar,
        shape_materials,
        &hud.loc,
        &hud.song_info,
        panels,
    );
    let note_markers: Vec<NoteMarker> = if !panels.progress_notes {
        Vec::new()
    } else {
        note_build
            .song_notes
            .notes
            .iter()
            .map(|n| NoteMarker {
                time: n.time,
                duration: n.duration,
                hole: n.hole,
                is_blow: n.is_blow,
            })
            .collect()
    };
    spawn_song_progress(
        &mut commands,
        &manifest.waveform,
        manifest.music_duration_secs,
        &note_markers,
        played.hole_count(),
        &note_build.adaptive.sections,
        &note_build.adaptive.learned,
    );
    let staff = panels.notation_staff && hud.bravura.is_some();
    if staff && let Some(bravura) = &hud.bravura {
        super::gameplay_2d::spawn_gameplay_music_score(&mut commands, bravura);
    }
    if panels.lyrics {
        super::karaoke::spawn_karaoke(&mut commands, lyrics, super::karaoke::strip_top(staff));
    }
    let harp_hint =
        super::song_info::harp_banner_text(played, &effective.song_key_for(chart), &hud.loc);
    spawn_countdown(&mut commands, &hud.loc, Some(&harp_hint), Some(&hud.song_info));
}

fn spawn_hud_overlay(
    commands: &mut Commands,
    modifiers: &[harmonicon_core::chart::Modifier],
    bpm: f32,
    beats_per_bar: usize,
    mut shape_materials: ResMut<Assets<NoteRibbon2dMaterial>>,
    loc: &Localization,
    song_info: &SongInfo,
    panels: ContextualPanels,
) {
    // The same panel 2D carries, on the same side of the screen. It used to
    // sit top-left here and right in 2D, with the same contents in a
    // different order — the drift `hud_panel` exists to stop. All of it is
    // supplementary, so compact mode skips the panel outright rather than
    // trimming it piecemeal (`contextual_panels`).
    if panels.side_panel {
        let legend_materials = if panels.technique_legend {
            build_legend_materials(&mut shape_materials, modifiers)
        } else {
            Vec::new()
        };
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    // Below the song-progress bar (`BAR_HEIGHT`, pinned at the
                    // very top across the full width and always painted above
                    // the HUD — see `BAR_Z_INDEX`) so its text is never covered.
                    top: Val::Px(8.0 + BAR_HEIGHT + music_score::PANEL_HEIGHT),
                    right: Val::Px(8.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    // Fixed so the panel doesn't grow or shrink with the
                    // current song's title length — long text wraps instead.
                    max_width: Val::Px(420.0),
                    ..default()
                },
                // `HUD_PANEL_BG`, not another hand-tuned near-black: 2D
                // darkens the whole screen behind its panel, but here the
                // song's own artwork shows through, and a 55%-black wash
                // leaves the technique legend unreadable over a bright one.
                BackgroundColor(HUD_PANEL_BG),
                GlobalZIndex(1),
                GameplayRoot,
            ))
            .with_children(|panel| {
                spawn_hud_panel(
                    panel,
                    HudPanel {
                        song_info,
                        loc,
                        beats_per_bar,
                        bpm,
                        legend_materials: &legend_materials,
                        // True here: no hole strip to print the key under,
                        // unlike 2D — so it goes in the panel.
                        blow_draw_legend: panels.blow_draw_in_panel,
                    },
                );
            });
    }

    // Score/combo/judgment, at the height of the lane's own hit plane rather
    // than in a screen corner. The 3D hit zone is a mesh at `HIT_Z`, so its
    // screen position comes from the (fixed) camera rather than from layout —
    // hence a measured fraction of the window rather than a UI anchor, unlike
    // 2D where the highway node's own bottom is the hit line.
    let readout_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(1),
            GameplayRoot,
            Pickable::IGNORE,
        ))
        .id();
    spawn_score_readout(
        commands,
        readout_root,
        ScoreReadoutAnchor {
            left: Val::Percent(0.0),
            width: Val::Percent(100.0),
            bottom: Val::Percent(HIT_PLANE_BOTTOM_PCT),
        },
    );
    // Same anchor for the wait-for-note card, a little above the hit plane.
    super::wait_freeze_overlay::spawn_wait_freeze_prompt(
        commands,
        readout_root,
        Val::Percent(HIT_PLANE_BOTTOM_PCT + 8.0),
    );
}

mod notes;
mod scene;
#[cfg(test)]
mod tests;

pub use notes::*;
pub use scene::*;
