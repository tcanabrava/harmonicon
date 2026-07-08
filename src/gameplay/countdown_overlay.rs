// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    prelude::*,
};

use crate::{
    menu::{AppState, GameplayMode, SelectedSong},
    settings::AudioSettings,
    song::SongManifest,
};

use super::jam_session::JamLoop;
use super::{GameplayClock, GameplayRoot, MusicPlayer, MusicStarted, Paused};

#[derive(Component, Default, Clone)]
pub struct CountdownOverlay;

#[derive(Component)]
pub struct CountdownText;

pub fn spawn_countdown(commands: &mut Commands, harp_hint: Option<&str>) {
    // The full-screen overlay shell is static and font/handle-free, so it's a
    // `bsn!` scene. The countdown text children carry a custom `FontSource`,
    // which `bsn!` can't take directly in 0.19-rc.3, so they stay imperative.
    let overlay = commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                width: {Val::Percent(100.0)},
                height: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
                justify_content: {JustifyContent::Center},
                row_gap: {Val::Px(12.0)},
            }
            BackgroundColor({Color::srgba(0.0, 0.0, 0.05, 0.55)})
            GlobalZIndex(100)
            CountdownOverlay
            GameplayRoot
        })
        .id();
    commands.entity(overlay).with_children(|ov| {
        ov.spawn((
            Text::new("GET READY"),
            TextFont {
                font_size: FontSize::Px(22.0),
                ..default()
            },
            TextColor(Color::srgba(0.85, 0.85, 1.0, 0.80)),
        ));
        // Which physical harp to grab (2D/3D pass this; jam shows it elsewhere).
        if let Some(hint) = harp_hint {
            ov.spawn((
                Text::new(hint.to_string()),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.80, 0.35)),
            ));
        }
        ov.spawn((
            Text::new("3"),
            TextFont {
                font_size: FontSize::Px(120.0),
                ..default()
            },
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
    mode: Res<GameplayMode>,
    jam_loop: Res<JamLoop>,
    mut commands: Commands,
) {
    if clock.get() >= 0.0 {
        for mut vis in &mut overlay {
            *vis = Visibility::Hidden;
        }
        if !music_started.0 {
            music_started.0 = true;
            if let Some(manifest) = manifests.get(&selected.0) {
                // Only Jam Session offers looping — scored modes end the song
                // and move on to the results screen.
                let settings = if *mode == GameplayMode::JamSession && jam_loop.0 {
                    PlaybackSettings::LOOP
                } else {
                    PlaybackSettings::ONCE
                };
                commands.spawn((
                    AudioPlayer::<AudioSource>(manifest.music.clone()),
                    settings.with_volume(Volume::Linear(audio.music_volume)),
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

    let remaining = -clock.get();
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
            update_countdown.run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        );
    }
}
