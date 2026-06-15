// SPDX-License-Identifier: MIT

//! The in-game pause overlay: a translucent menu with Resume / Restart / Quit,
//! toggled with Escape. Shares the gameplay [`Paused`] flag (every gameplay
//! chain gates on it) and pauses/resumes the song's audio sink.

use bevy::prelude::*;

use crate::assets_management::GlobalFonts;
use crate::menu::{AppState, ReturnToSongList};

use super::{GameplayRoot, MusicPlayer, Paused};

/// Root of the pause overlay; toggled between hidden/visible.
#[derive(Component)]
pub(super) struct PauseMenuRoot;

#[derive(Component)]
pub(super) enum PauseButton {
    Resume,
    Restart,
    QuitSong,
}

/// Spawns the (initially hidden) pause overlay. Tagged `GameplayRoot` so it is
/// torn down with the rest of the scene.
pub(super) fn setup_pause_menu(mut commands: Commands, fonts: Res<GlobalFonts>) {
    let font = fonts.gameplay.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            GlobalZIndex(200),
            Visibility::Hidden,
            GameplayRoot,
            PauseMenuRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: FontSize::Px(52.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            spawn_pause_button(p, "Resume", PauseButton::Resume, &font);
            spawn_pause_button(p, "Restart", PauseButton::Restart, &font);
            spawn_pause_button(p, "Quit Song", PauseButton::QuitSong, &font);
        });
}

fn spawn_pause_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    btn: PauseButton,
    font: &FontSource,
) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(220.0),
                padding: UiRect::axes(Val::Px(28.0), Val::Px(12.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            btn,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// Escape toggles the pause state, the overlay's visibility, and the song audio.
pub(super) fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    paused.0 = !paused.0;
    for mut vis in &mut overlay {
        *vis = if paused.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for sink in &sinks {
        if paused.0 {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

pub(super) fn handle_pause_buttons(
    buttons: Query<(&Interaction, &PauseButton), Changed<Interaction>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            PauseButton::Resume => {
                paused.0 = false;
                for mut vis in &mut overlay {
                    *vis = Visibility::Hidden;
                }
                for sink in &sinks {
                    sink.play();
                }
            }
            PauseButton::Restart => {
                paused.0 = false;
                // Re-enter via SongLoading so the whole song setup runs fresh
                // (the asset is already loaded, so it resumes immediately).
                next_state.set(AppState::SongLoading);
            }
            PauseButton::QuitSong => {
                paused.0 = false;
                // Land back on the song list, not the main menu.
                return_to_song_list.0 = true;
                next_state.set(AppState::Menu);
            }
        }
    }
}

pub(super) fn pause_button_hover(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<PauseButton>),
    >,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = BackgroundColor(match interaction {
            Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
            Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
            Interaction::None => Color::srgb(0.14, 0.14, 0.22),
        });
    }
}
