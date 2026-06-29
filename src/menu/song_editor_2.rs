// SPDX-License-Identifier: MIT

//! Song authoring tool, launched from the main menu (`AppState::SongEditor`).
//!
//! Step 2 builds the metadata form: artist, song name, a music-file picker,
//! tempo, beats-per-bar, the harmonica key, and a 12-bar blues preview. Text
//! fields are edited in place (click to focus, type, backspace); the music
//! picker is a small in-app browser that scans common folders for ogg/mp3 so we
//! don't depend on a native dialog. Later steps add audio analysis, note
//! editing in the grid, and saving to a `.harpchart`.

use super::AppState;
use bevy::prelude::*;

// ── Colours ─────────────────────────────────────────────────────────────────

const FIELD_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.95);
const FIELD_BG_FOCUS: Color = Color::srgba(0.16, 0.16, 0.24, 1.0);
const BTN_BG: Color = Color::srgba(0.14, 0.14, 0.20, 0.95);
const ACCENT: Color = Color::srgb(0.95, 0.80, 0.35);
const LABEL: Color = Color::srgb(0.75, 0.75, 0.82);

// ── Setup ───────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>)
{
    commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(200.0, 100.0))),
            MeshMaterial2d(materials.add(Color::from(FIELD_BG))),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
}

fn cleanup(mut commands: Commands) {
}

pub struct SongEditor2Plugin;

impl Plugin for SongEditor2Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::SongEditor2), setup)
            .add_systems(OnExit(AppState::SongEditor2), cleanup);
    }
}
