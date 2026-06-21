// SPDX-License-Identifier: MIT

//! Song authoring tool, launched from the main menu (`AppState::SongEditor`).
//!
//! Built up across steps; for now it is a placeholder screen that opens and
//! returns to the menu with Esc. Later steps add the song metadata form, audio
//! analysis, the 12-bar note editor, and saving to a `.harpchart`.

use bevy::prelude::*;

use crate::assets_management::GlobalFonts;

use super::AppState;

/// Root of everything spawned for the editor, despawned on exit.
#[derive(Component)]
struct SongEditorRoot;

fn setup(mut commands: Commands, fonts: Res<GlobalFonts>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.09)),
            SongEditorRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Song Editor"),
                TextFont {
                    font_size: FontSize::Px(48.0),
                    font: fonts.gameplay.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            root.spawn((
                Text::new("Press Esc to return to the menu"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    font: fonts.gameplay.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.7)),
            ));
        });
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<SongEditorRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Esc returns to the menu.
fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
    }
}

pub struct SongEditorPlugin;

impl Plugin for SongEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::SongEditor), setup)
            .add_systems(OnExit(AppState::SongEditor), cleanup)
            .add_systems(
                Update,
                handle_input.run_if(in_state(AppState::SongEditor)),
            );
    }
}
