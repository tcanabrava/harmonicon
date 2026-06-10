mod gameplay_2d;
mod gameplay_3d;

use std::collections::HashSet;
use bevy::prelude::*;

use crate::{
    menu::{AppState, GameplayMode},
    pitch_detect::{PitchEvent, PitchInfo},
};

pub use gameplay_3d::GameplayCamera3D;

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayClock>()
            .init_resource::<ActivePitches>()
            .init_resource::<MusicStarted>()
            .init_resource::<ValidHarpNotes>()
            // Mode-gated setup
            .add_systems(
                OnEnter(AppState::Playing),
                gameplay_2d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
            )
            .add_systems(
                OnEnter(AppState::Playing),
                gameplay_3d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
            )
            // Cleanup: shared entity despawn + restore camera on 3D exit
            .add_systems(OnExit(AppState::Playing), cleanup_gameplay)
            .add_systems(
                OnExit(AppState::Playing),
                gameplay_3d::restore_camera
                    .run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
            )
            // Shared tick + pitch collection
            .add_systems(
                Update,
                (tick_clock, collect_pitches)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            // 2D update chain
            .add_systems(
                Update,
                (
                    gameplay_2d::update_countdown,
                    gameplay_2d::update_notes,
                    gameplay_2d::update_bar,
                    gameplay_2d::update_holes,
                )
                    .chain()
                    .run_if(
                        in_state(AppState::Playing)
                            .and(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                    ),
            )
            // 3D update chain
            .add_systems(
                Update,
                (
                    gameplay_3d::update_countdown,
                    gameplay_3d::update_notes_3d,
                    gameplay_3d::update_bar_3d,
                    gameplay_3d::update_holes_3d,
                )
                    .chain()
                    .run_if(
                        in_state(AppState::Playing)
                            .and(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                    ),
            );
    }
}

// ── Shared resources ──────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct GameplayClock(pub f64);

#[derive(Resource, Default)]
pub struct ActivePitches(pub Vec<PitchInfo>);

#[derive(Resource, Default)]
pub struct MusicStarted(pub bool);

#[derive(Resource, Default)]
pub struct ValidHarpNotes(pub HashSet<String>);

// ── Shared components ─────────────────────────────────────────────────────────

#[derive(Component)]
pub struct GameplayRoot;

#[derive(Component)]
pub struct NoteVisual {
    pub time: f64,
    pub height_pct: f32,
}

#[derive(Component)]
pub struct BarCell(pub usize);

#[derive(Component)]
pub struct HoleCell(pub u8);

#[derive(Component, Default)]
pub struct HoleState {
    pub brightness: f32,
    pub is_blow: bool,
}

#[derive(Component)]
pub struct CountdownOverlay;

#[derive(Component)]
pub struct CountdownText;

// ── Shared constants ──────────────────────────────────────────────────────────

pub const HOLE_COUNT: usize = 10;
pub const COUNTDOWN: f64 = 3.0;
pub const LANE_PCT: f32 = 100.0 / HOLE_COUNT as f32;
pub const HIT_H_PCT: f32 = 7.0;
pub const LOOKAHEAD: f64 = 3.0;

// ── Shared systems ────────────────────────────────────────────────────────────

fn tick_clock(mut clock: ResMut<GameplayClock>, time: Res<Time>) {
    clock.0 += time.delta_secs_f64();
}

fn collect_pitches(mut reader: MessageReader<PitchEvent>, mut active: ResMut<ActivePitches>) {
    for ev in reader.read() {
        active.0 = ev.0.clone();
    }
}

fn cleanup_gameplay(mut commands: Commands, roots: Query<Entity, With<GameplayRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}
