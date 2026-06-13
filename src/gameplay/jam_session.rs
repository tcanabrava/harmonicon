use bevy::prelude::*;

use crate::{
    assets_management::GlobalFonts, menu::SelectedSong, song::SongManifest,
    song::harmonica::twelve_bar,
};

use crate::spectrogram::{SpectrogramStyle, spawn_spectrogram};

use super::countdown_overlay::spawn_countdown;
use super::metronome_overlay::spawn_metronome;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{COUNTDOWN, GameplayRoot, MusicStarted};

/// Free-play screen: left half shows the 12-bar chart and the metronome stacked
/// vertically; the right half is reserved for a future jam feature. The shared
/// gameplay clock/music/pause systems run for this mode too, so the chart tracks
/// the song and the metronome clicks — there are just no falling notes.
pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    spectrogram_style: Res<SpectrogramStyle>,
    fonts: Res<GlobalFonts>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Jam Session");
        return;
    };
    clock.0 = -COUNTDOWN;
    music_started.0 = false;

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let bpm = chart.song.tempo_bpm;
    let chords = twelve_bar(key);
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let beats_per_bar = {
        let ts = chart.song.time_signature.as_deref().unwrap_or("4/4");
        ts.split('/').next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(4)
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            ImageNode::new(manifest.background.clone()),
            GameplayRoot,
        ))
        .with_children(|root| {
            // Dark overlay for legibility.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.70)),
            ));

            // ── Left half: 12-bar chart + metronome, vertical ────────────────
            root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            })
            .with_children(|left| {
                left.spawn((
                    Text::new(title),
                    TextFont { font_size: FontSize::Px(20.0), font: fonts.gameplay.clone(), ..default() },
                    TextColor(Color::WHITE),
                ));
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|grid| {
                    spawn_12_bar_grid(grid, &chords, key, &fonts.gameplay, &GridConfig::for_2d());
                });
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|metro| {
                    spawn_metronome(metro, beats_per_bar, bpm, &fonts.gameplay);
                });
            });

            // ── Right half: live spectrogram of the harmonica input ──────────
            root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                ..default()
            })
            .with_children(|right| {
                spawn_spectrogram(right, *spectrogram_style);
            });
        });

    spawn_countdown(&mut commands, &fonts.gameplay);
}
