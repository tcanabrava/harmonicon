// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    prelude::*,
};

use crate::{
    menu::{AppState, AudioSettings, SelectedSong},
    song::SongManifest,
};

use super::{GameplayClock, Paused, ScoringConfig};

#[derive(Component)]
pub struct MetronomeBeat(pub usize);

/// Marks the click on/off toggle button in the metronome HUD block.
#[derive(Component)]
pub struct MetronomeMuteButton;

/// Marks the text inside the toggle button so it can be rewritten.
#[derive(Component)]
pub struct MetronomeMuteLabel;

/// Click samples, loaded once at startup. The downbeat carries the accent.
#[derive(Resource)]
pub struct MetronomeSounds {
    pub downbeat: Handle<AudioSource>,
    pub beat: Handle<AudioSource>,
}

/// When true the metronome stays visual-only.
#[derive(Resource, Default)]
pub struct MetronomeMuted(pub bool);

/// The last beat index a click was played for, so each beat clicks once.
#[derive(Resource, Default)]
pub struct LastClickedBeat(pub Option<i64>);

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Global beat index for a clock position, or `None` before the song starts.
pub fn beat_index(clock: f64, bpm: f64) -> Option<i64> {
    if clock < 0.0 || bpm <= 0.0 {
        return None;
    }
    Some((clock / (60.0 / bpm)).floor() as i64)
}

/// True when a beat is the first beat of its bar.
pub fn is_downbeat(beat: i64, beats_per_bar: f64) -> bool {
    let beats = (beats_per_bar.max(1.0)) as i64;
    beat.rem_euclid(beats) == 0
}

// ── UI ────────────────────────────────────────────────────────────────────────

pub fn spawn_metronome(
    parent: &mut ChildSpawnerCommands,
    beats_per_bar: usize,
    bpm: f32,
    font: &FontSource,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(format!("\u{2669} = {}", bpm as u32)),
                TextFont { font_size: FontSize::Px(13.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.65, 0.65, 0.70)),
            ));

            row.spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.12, 0.12, 0.16, 0.9)),
                BorderColor::all(Color::srgb(0.35, 0.35, 0.50)),
                MetronomeMuteButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("click: on"),
                    TextFont { font_size: FontSize::Px(11.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(0.65, 0.65, 0.70)),
                    MetronomeMuteLabel,
                ));
            });
        });

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

// ── Click playback ────────────────────────────────────────────────────────────

fn load_metronome_sounds(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(MetronomeSounds {
        downbeat: asset_server.load("sounds/metronome_high.ogg"),
        beat: asset_server.load("sounds/metronome_low.ogg"),
    });
}

fn reset_click_tracking(mut last: ResMut<LastClickedBeat>) {
    last.0 = None;
}

/// Plays one click per beat, accented on the downbeat. Follows the gameplay
/// clock, so it stays in sync through pause/resume and loop-region jumps.
fn click_metronome(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    muted: Res<MetronomeMuted>,
    sounds: Res<MetronomeSounds>,
    audio: Res<AudioSettings>,
    mut last: ResMut<LastClickedBeat>,
    mut commands: Commands,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let Some(current) = beat_index(clock.0, manifest.chart.song.tempo_bpm as f64) else {
        return;
    };
    if last.0 == Some(current) {
        return;
    }
    last.0 = Some(current);

    if muted.0 {
        return;
    }
    let sample = if is_downbeat(current, config.beats_per_bar) {
        sounds.downbeat.clone()
    } else {
        sounds.beat.clone()
    };
    commands.spawn((
        AudioPlayer::<AudioSource>(sample),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.metronome_volume)),
    ));
}

// ── Mute toggle ───────────────────────────────────────────────────────────────

fn toggle_mute_key(keyboard: Res<ButtonInput<KeyCode>>, mut muted: ResMut<MetronomeMuted>) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
    }
}

fn handle_mute_button(
    buttons: Query<&Interaction, (Changed<Interaction>, With<MetronomeMuteButton>)>,
    mut muted: ResMut<MetronomeMuted>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            muted.0 = !muted.0;
        }
    }
}

fn mute_button_hover(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MetronomeMuteButton>),
    >,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = BackgroundColor(match interaction {
            Interaction::Pressed => Color::srgba(0.25, 0.25, 0.40, 0.9),
            Interaction::Hovered => Color::srgba(0.20, 0.20, 0.32, 0.9),
            Interaction::None => Color::srgba(0.12, 0.12, 0.16, 0.9),
        });
    }
}

fn update_mute_label(
    muted: Res<MetronomeMuted>,
    mut labels: Query<(&mut Text, &mut TextColor), With<MetronomeMuteLabel>>,
) {
    // written every frame, like update_score_display: the mute state outlives
    // the label (it survives across songs), so a change guard would leave a
    // freshly spawned label stale.
    for (mut text, mut color) in &mut labels {
        if muted.0 {
            *text = Text::new("click: off");
            *color = TextColor(Color::srgb(0.40, 0.40, 0.45));
        } else {
            *text = Text::new("click: on");
            *color = TextColor(Color::srgb(0.65, 0.65, 0.70));
        }
    }
}

pub struct MetronomePlugin;

impl Plugin for MetronomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MetronomeMuted>()
            .init_resource::<LastClickedBeat>()
            .add_systems(Startup, load_metronome_sounds)
            .add_systems(OnEnter(AppState::Playing), reset_click_tracking)
            .add_systems(
                Update,
                (update_metronome, click_metronome).run_if(
                    in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0),
                ),
            )
            // The toggle stays responsive even while paused, like the pause menu.
            .add_systems(
                Update,
                (
                    toggle_mute_key,
                    handle_mute_button,
                    mute_button_hover,
                    update_mute_label,
                )
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_beat_before_the_song_starts() {
        assert_eq!(beat_index(-0.5, 120.0), None);
        assert_eq!(beat_index(-3.0, 60.0), None);
    }

    #[test]
    fn beat_zero_at_clock_zero() {
        assert_eq!(beat_index(0.0, 120.0), Some(0));
    }

    #[test]
    fn beat_advances_every_half_second_at_120bpm() {
        assert_eq!(beat_index(0.49, 120.0), Some(0));
        assert_eq!(beat_index(0.5, 120.0), Some(1));
        assert_eq!(beat_index(1.99, 120.0), Some(3));
    }

    #[test]
    fn invalid_bpm_gives_no_beat() {
        assert_eq!(beat_index(1.0, 0.0), None);
        assert_eq!(beat_index(1.0, -60.0), None);
    }

    #[test]
    fn downbeat_every_four_beats_in_common_time() {
        assert!(is_downbeat(0, 4.0));
        assert!(!is_downbeat(1, 4.0));
        assert!(!is_downbeat(3, 4.0));
        assert!(is_downbeat(4, 4.0));
        assert!(is_downbeat(8, 4.0));
    }

    #[test]
    fn downbeat_every_three_beats_in_waltz_time() {
        assert!(is_downbeat(0, 3.0));
        assert!(is_downbeat(3, 3.0));
        assert!(!is_downbeat(4, 3.0));
    }

    #[test]
    fn degenerate_bar_treats_every_beat_as_downbeat() {
        assert!(is_downbeat(0, 0.0));
        assert!(is_downbeat(7, 1.0));
    }
}