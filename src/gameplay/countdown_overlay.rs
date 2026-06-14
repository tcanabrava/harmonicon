// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    prelude::*,
};

use crate::{
    menu::{AppState, AudioSettings, SelectedSong},
    song::SongManifest,
};

use super::{GameplayClock, GameplayRoot, MusicPlayer, MusicStarted, Paused};

#[derive(Component)]
pub struct CountdownOverlay;

#[derive(Component)]
pub struct CountdownText;

pub fn spawn_countdown(commands: &mut Commands, font: &FontSource) {
    commands
        .spawn((
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

pub fn update_countdown(
    clock: Res<GameplayClock>,
    mut overlay: Query<&mut Visibility, With<CountdownOverlay>>,
    mut text: Query<(&mut Text, &mut TextFont), With<CountdownText>>,
    mut music_started: ResMut<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
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
                    PlaybackSettings::ONCE.with_volume(Volume::Linear(audio.music_volume)),
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

pub struct CountdownPlugin;

impl Plugin for CountdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_countdown.run_if(
                in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0),
            ),
        );
    }
}