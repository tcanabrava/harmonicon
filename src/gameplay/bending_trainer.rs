// SPDX-License-Identifier: MIT

//! Bending Trainer: a focused practice screen showing the harmonica bend
//! diagram + the metronome, with no backing track and no falling notes. Like
//! Jam Session it's a [`GameplayMode`](crate::menu::GameplayMode) song-based
//! screen (the picked song supplies the harp, key and tempo), reusing the
//! shared clock / mic / metronome pipeline — it just starts the clock at 0 and
//! suppresses the song's music so only the metronome ticks.

use bevy::prelude::*;

use crate::{
    assets_management::GlobalFonts,
    menu::SelectedSong,
    song::SongManifest,
    song::harmonica::harp_banner,
};

use super::harmonica_overlay::spawn_harmonica_overlay;
use super::metronome_overlay::spawn_metronome;
use super::{GameplayRoot, MusicStarted};

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    fonts: Res<GlobalFonts>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Bending Trainer");
        return;
    };

    // Start ticking immediately (no countdown) and flag music as already
    // "started" so `update_countdown` never spawns the backing track — this is
    // metronome-only practice.
    clock.0 = 0.0;
    music_started.0 = true;

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let bpm = chart.song.tempo_bpm;
    let beats_per_bar = {
        let ts = chart.song.time_signature.as_deref().unwrap_or("4/4");
        ts.split('/')
            .next()
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(4)
    };
    let harp_hint = harp_banner(&chart.harmonica, key);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
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
                BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.78)),
            ));

            root.spawn((
                Text::new("Bending Trainer"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    font: fonts.gameplay.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            root.spawn((
                Text::new(harp_hint),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    font: fonts.gameplay.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.80, 0.35)),
            ));

            spawn_harmonica_overlay(root, &chart.harmonica, &fonts.gameplay);

            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|metro| {
                spawn_metronome(metro, beats_per_bar, bpm, &fonts.gameplay);
            });

            root.spawn((
                Text::new("Esc to go back  \u{00B7}  M mutes the click"),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    font: fonts.gameplay.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));
        });
}
