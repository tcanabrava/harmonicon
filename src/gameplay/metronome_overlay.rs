use bevy::prelude::*;

use crate::{
    menu::{AppState, SelectedSong},
    song::SongManifest,
};

use super::{GameplayClock, Paused, ScoringConfig};

#[derive(Component)]
pub struct MetronomeBeat(pub usize);

pub fn spawn_metronome(
    parent: &mut ChildSpawnerCommands,
    beats_per_bar: usize,
    bpm: f32,
    font: &FontSource,
) {
    parent.spawn((
        Text::new(format!("\u{2669} = {}", bpm as u32)),
        TextFont { font_size: FontSize::Px(13.0), font: font.clone(), ..default() },
        TextColor(Color::srgb(0.65, 0.65, 0.70)),
    ));

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            for i in 0..beats_per_bar {
                let size = if i == 0 { Val::Px(28.0) } else { Val::Px(22.0) };
                row.spawn((
                    Node {
                        width: size,
                        height: size,
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.12, 0.16, 0.9)),
                    BorderColor::all(Color::srgb(0.35, 0.35, 0.50)),
                    MetronomeBeat(i),
                ));
            }
        });
}

pub fn update_metronome(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    mut beats: Query<(&MetronomeBeat, &mut BackgroundColor)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    if clock.0 < 0.0 {
        return;
    }

    let bpm = manifest.chart.song.tempo_bpm as f64;
    let beat_dur = 60.0 / bpm;
    let beats_per_bar = config.beats_per_bar as usize;
    let beat_pos = clock.0 / beat_dur;
    let current = beat_pos.floor() as usize % beats_per_bar;
    let phase = beat_pos.fract() as f32;

    for (cell, mut bg) in &mut beats {
        let brightness = if cell.0 == current {
            (1.0 - phase).powf(1.5)
        } else {
            0.0
        };
        let is_downbeat = cell.0 == 0;
        let base = if is_downbeat { 0.25 } else { 0.12 };
        *bg = BackgroundColor(Color::srgba(
            base + brightness * 0.9,
            base + brightness * if is_downbeat { 0.4 } else { 0.7 },
            base + brightness * if is_downbeat { 0.1 } else { 0.9 },
            0.9,
        ));
    }
}

pub struct MetronomePlugin;

impl Plugin for MetronomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_metronome.run_if(
                in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0),
            ),
        );
    }
}
