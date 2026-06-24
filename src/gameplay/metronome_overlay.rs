// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    picking::Pickable,
    picking::events::{Click, Out, Over, Pointer},
    prelude::*,
};

use crate::{
    menu::{AppState, SelectedSong},
    settings::AudioSettings,
    song::SongManifest,
};

use super::{GameplayClock, Paused, ScoringConfig};

#[derive(Component)]
pub struct MetronomeBeat(pub usize);

/// Marks the click on/off toggle button in the metronome HUD block.
#[derive(Component, Default, Clone)]
pub struct MetronomeMuteButton;

/// Marks the text inside the toggle button so it can be rewritten.
#[derive(Component, Default, Clone)]
pub struct MetronomeMuteLabel;

/// Marks the straight/shuffle feel toggle button.
#[derive(Component, Default, Clone)]
pub struct MetronomeFeelButton;

/// Marks the text inside the feel toggle so it can be rewritten.
#[derive(Component, Default, Clone)]
pub struct MetronomeFeelLabel;

// The two little HUD pill buttons share these idle/hover colours.
const PILL_IDLE: Color = Color::srgba(0.12, 0.12, 0.16, 0.9);
const PILL_HOVER: Color = Color::srgba(0.20, 0.20, 0.32, 0.9);
const PILL_BORDER: Color = Color::srgb(0.35, 0.35, 0.50);

/// Click subdivision. `Straight` clicks plain quarters; `Shuffle` splits each
/// beat into triplets and clicks the beat + the swung "and" (the long-short
/// "loping" blues groove). Defaults to shuffle since the songs are blues.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MetronomeFeel {
    Straight,
    #[default]
    Shuffle,
}

/// Click samples, loaded once at startup. The downbeat carries the accent.
#[derive(Resource)]
pub struct MetronomeSounds {
    pub downbeat: Handle<AudioSource>,
    pub beat: Handle<AudioSource>,
}

/// When true the metronome stays visual-only.
#[derive(Resource, Default)]
pub struct MetronomeMuted(pub bool);

/// The last tick index a click was played for, so each tick clicks once. A tick
/// is a beat in straight feel, or a triplet-eighth in shuffle feel.
#[derive(Resource, Default)]
pub struct LastClickedTick(pub Option<i64>);

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

/// Index of the current click subdivision ("tick"), or `None` before the song
/// starts. A tick is a whole beat in `Straight` feel, or a triplet-eighth (three
/// per beat) in `Shuffle` feel.
pub fn tick_index(clock: f64, bpm: f64, feel: MetronomeFeel) -> Option<i64> {
    if clock < 0.0 || bpm <= 0.0 {
        return None;
    }
    let beat_dur = 60.0 / bpm;
    let div = match feel {
        MetronomeFeel::Straight => beat_dur,
        MetronomeFeel::Shuffle => beat_dur / 3.0,
    };
    Some((clock / div).floor() as i64)
}

/// What to play for a given tick: `Some((accent, gain))` or `None` for a silent
/// subdivision. In shuffle feel a beat is three triplet-eighths; we click the
/// beat (sub 0, accented on the downbeat) and the swung "and" (sub 2, softer),
/// and stay silent on the middle triplet — the classic long-short shuffle.
pub fn click_for_tick(tick: i64, beats_per_bar: f64, feel: MetronomeFeel) -> Option<(bool, f32)> {
    match feel {
        MetronomeFeel::Straight => Some((is_downbeat(tick, beats_per_bar), 1.0)),
        MetronomeFeel::Shuffle => match tick.rem_euclid(3) {
            0 => Some((is_downbeat(tick.div_euclid(3), beats_per_bar), 1.0)),
            2 => Some((false, 0.55)),
            _ => None,
        },
    }
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
                TextFont {
                    font_size: FontSize::Px(13.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.65, 0.70)),
            ));

            // Click on/off toggle. Authored with bsn!; click + hover ride along
            // inline as on(...). The border colour is inserted after (bsn! can't
            // express BorderColor::all), and the label uses the default font.
            row.spawn_empty()
                .apply_scene(bsn! {
                    Button
                    Node {
                        padding: {UiRect::axes(Val::Px(6.0), Val::Px(2.0))},
                        border: {UiRect::all(Val::Px(1.5))},
                    }
                    BackgroundColor({PILL_IDLE})
                    MetronomeMuteButton
                    on(toggle_mute)
                    on(pill_over)
                    on(pill_out)
                    Children [
                        (
                            Text({"click: on".to_string()})
                            TextFont { font_size: {FontSize::Px(11.0)} }
                            TextColor({Color::srgb(0.65, 0.65, 0.70)})
                            MetronomeMuteLabel
                            Pickable { should_block_lower: {false}, is_hoverable: {false} }
                        )
                    ]
                })
                .insert(BorderColor::all(PILL_BORDER));

            // Straight ↔ shuffle feel toggle.
            row.spawn_empty()
                .apply_scene(bsn! {
                    Button
                    Node {
                        padding: {UiRect::axes(Val::Px(6.0), Val::Px(2.0))},
                        border: {UiRect::all(Val::Px(1.5))},
                    }
                    BackgroundColor({PILL_IDLE})
                    MetronomeFeelButton
                    on(toggle_feel)
                    on(pill_over)
                    on(pill_out)
                    Children [
                        (
                            Text({"feel: shuffle".to_string()})
                            TextFont { font_size: {FontSize::Px(11.0)} }
                            TextColor({Color::srgb(0.65, 0.65, 0.70)})
                            MetronomeFeelLabel
                            Pickable { should_block_lower: {false}, is_hoverable: {false} }
                        )
                    ]
                })
                .insert(BorderColor::all(PILL_BORDER));
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

fn reset_click_tracking(mut last: ResMut<LastClickedTick>) {
    last.0 = None;
}

/// Plays the metronome clicks: plain quarters in straight feel, or the swung
/// long-short pattern in shuffle feel (beat + the "and", accent on the
/// downbeat). Follows the gameplay clock, so it stays in sync through
/// pause/resume and loop-region jumps.
fn click_metronome(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    muted: Res<MetronomeMuted>,
    feel: Res<MetronomeFeel>,
    sounds: Res<MetronomeSounds>,
    audio: Res<AudioSettings>,
    mut last: ResMut<LastClickedTick>,
    mut commands: Commands,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let Some(current) = tick_index(clock.0, manifest.chart.song.tempo_bpm as f64, *feel) else {
        return;
    };
    if last.0 == Some(current) {
        return;
    }
    last.0 = Some(current);

    if muted.0 {
        return;
    }
    // Silent subdivisions (the skipped middle triplet of a shuffle) play nothing.
    let Some((accent, gain)) = click_for_tick(current, config.beats_per_bar, *feel) else {
        return;
    };
    let sample = if accent {
        sounds.downbeat.clone()
    } else {
        sounds.beat.clone()
    };
    commands.spawn((
        AudioPlayer::<AudioSource>(sample),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.metronome_volume * gain)),
    ));
}

// ── Toggles (mute + feel) ──────────────────────────────────────────────────────

fn toggle_mute_key(keyboard: Res<ButtonInput<KeyCode>>, mut muted: ResMut<MetronomeMuted>) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
    }
}

// Button behaviour, wired inline as on(...) observers at spawn.
fn toggle_mute(_: On<Pointer<Click>>, mut muted: ResMut<MetronomeMuted>) {
    muted.0 = !muted.0;
}

/// Flip the click subdivision between straight and shuffle. The label follows
/// via `update_feel_label`.
fn toggle_feel(_: On<Pointer<Click>>, mut feel: ResMut<MetronomeFeel>) {
    *feel = match *feel {
        MetronomeFeel::Shuffle => MetronomeFeel::Straight,
        MetronomeFeel::Straight => MetronomeFeel::Shuffle,
    };
}

/// Shared hover highlight for the small HUD pill buttons.
fn pill_over(ev: On<Pointer<Over>>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(PILL_HOVER);
    }
}

fn pill_out(ev: On<Pointer<Out>>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(PILL_IDLE);
    }
}

/// Mirror the current feel onto its button label (written every frame, like
/// `update_mute_label`, so a freshly spawned label isn't stale across songs).
fn update_feel_label(
    feel: Res<MetronomeFeel>,
    mut labels: Query<&mut Text, With<MetronomeFeelLabel>>,
) {
    let label = match *feel {
        MetronomeFeel::Straight => "feel: straight",
        MetronomeFeel::Shuffle => "feel: shuffle",
    };
    for mut text in &mut labels {
        *text = Text::new(label);
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
            .init_resource::<LastClickedTick>()
            .init_resource::<MetronomeFeel>()
            .add_systems(Startup, load_metronome_sounds)
            .add_systems(OnEnter(AppState::Playing), reset_click_tracking)
            .add_systems(
                Update,
                (update_metronome, click_metronome)
                    .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
            )
            // The toggles stay responsive even while paused, like the pause
            // menu. The buttons' click/hover ride along as inline on(...)
            // observers (see spawn_metronome); only the keyboard shortcut and
            // the label refreshes are systems here.
            .add_systems(
                Update,
                (toggle_mute_key, update_mute_label, update_feel_label)
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
