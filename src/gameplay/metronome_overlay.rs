// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    picking::Pickable,
    picking::events::{Click, Out, Over, Pointer},
    prelude::*,
};
use bevy_fluent::Localization;

use crate::{
    localization::LocalizationExt,
    menu::{AppState, SelectedSong},
    settings::AudioSettings,
    song::{SongManifest, chart::Feel},
};

use super::{GameplayClock, Paused};

/// The metronome's tempo, decoupled from the song so it can be driven by the
/// gameplay screens (set from the chart) or the standalone Bending Trainer (set
/// from its key/BPM controls).
#[derive(Resource)]
pub struct MetronomeTempo {
    pub bpm: f32,
    pub beats_per_bar: usize,
}

impl Default for MetronomeTempo {
    fn default() -> Self {
        Self {
            bpm: 90.0,
            beats_per_bar: 4,
        }
    }
}

/// Run condition: the metronome clicks/animates during gameplay (when not
/// paused) and in the Bending Trainer.
fn metronome_running(state: Res<State<AppState>>, paused: Res<Paused>) -> bool {
    match state.get() {
        AppState::Playing => !paused.0,
        AppState::BendingTrainer => true,
        _ => false,
    }
}

/// Run condition for the always-responsive bits (toggles, label refreshes).
fn metronome_ui_active(state: Res<State<AppState>>) -> bool {
    matches!(state.get(), AppState::Playing | AppState::BendingTrainer)
}

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

/// Marks the "♩ = NN" tempo readout so it tracks `MetronomeTempo` live.
#[derive(Component, Default, Clone)]
pub struct MetronomeTempoLabel;

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

/// True when a beat is the first beat of its bar.
pub const fn is_downbeat(beat: i64, beats_per_bar: f64) -> bool {
    let beats = (beats_per_bar.max(1.0)) as i64;
    beat.rem_euclid(beats) == 0
}

/// Index of the current click subdivision ("tick"), or `None` before the song
/// starts. A tick is a whole beat in `Straight` feel, or a triplet-eighth (three
/// per beat) in `Shuffle` feel.
pub const fn tick_index(clock: f64, bpm: f64, feel: MetronomeFeel) -> Option<i64> {
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
pub const fn click_for_tick(tick: i64, beats_per_bar: f64, feel: MetronomeFeel) -> Option<(bool, f32)> {
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

pub fn spawn_metronome(parent: &mut ChildSpawnerCommands, beats_per_bar: usize, bpm: f32) {
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
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.65, 0.70)),
                // Refreshed live from MetronomeTempo (the trainer's BPM control).
                MetronomeTempoLabel,
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
                            TextFont { font_size: {FontSize::Px(15.0)} }
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
                            TextFont { font_size: {FontSize::Px(15.0)} }
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
    tempo: Res<MetronomeTempo>,
    mut beats: Query<(&MetronomeBeat, &mut BackgroundColor)>,
) {
    if clock.get() < 0.0 {
        return;
    }

    let bpm = tempo.bpm as f64;
    let beat_dur = 60.0 / bpm;
    let beats_per_bar = tempo.beats_per_bar;
    let beat_pos = clock.get() / beat_dur;
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
    tempo: Res<MetronomeTempo>,
    muted: Res<MetronomeMuted>,
    feel: Res<MetronomeFeel>,
    sounds: Res<MetronomeSounds>,
    audio: Res<AudioSettings>,
    mut last: ResMut<LastClickedTick>,
    mut commands: Commands,
) {
    let Some(current) = tick_index(clock.get(), tempo.bpm as f64, *feel) else {
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
    let Some((accent, gain)) = click_for_tick(current, tempo.beats_per_bar as f64, *feel) else {
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

/// Maps a chart's declared [`Feel`] onto the metronome's own feel type.
/// `None` (the common case — most charts don't declare one) means "leave
/// whatever the player currently has selected untouched", not "straight".
const fn feel_from_chart(chart_feel: Option<Feel>) -> Option<MetronomeFeel> {
    match chart_feel {
        Some(Feel::Straight) => Some(MetronomeFeel::Straight),
        Some(Feel::Shuffle) => Some(MetronomeFeel::Shuffle),
        None => None,
    }
}

/// On entering gameplay, seed the metronome tempo — and, when the chart
/// declares one, the feel — from the chosen song's chart. (The Bending
/// Trainer sets `MetronomeTempo` itself, from its own controls, and has no
/// chart to read a feel from.)
fn set_tempo_from_song(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut tempo: ResMut<MetronomeTempo>,
    mut feel: ResMut<MetronomeFeel>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    tempo.bpm = manifest.chart.song.tempo_bpm;
    let ts = manifest
        .chart
        .song
        .time_signature
        .as_deref()
        .unwrap_or("4/4");
    tempo.beats_per_bar = ts
        .split('/')
        .next()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(4);
    if let Some(chart_feel) = feel_from_chart(manifest.chart.song.feel) {
        *feel = chart_feel;
    }
}

/// Keep the "♩ = NN" readout in step with the live tempo.
fn update_tempo_label(
    tempo: Res<MetronomeTempo>,
    mut labels: Query<&mut Text, With<MetronomeTempoLabel>>,
) {
    for mut text in &mut labels {
        *text = Text::new(format!("\u{2669} = {}", tempo.bpm as u32));
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
    loc: Res<Localization>,
    mut labels: Query<(&mut Text, &mut TextColor), With<MetronomeMuteLabel>>,
) {
    // Written every frame: the mute state outlives the label (it survives
    // across songs), so a change guard would leave a freshly spawned label
    // stale.
    for (mut text, mut color) in &mut labels {
        if muted.0 {
            *text = Text::new(String::from(loc.msg("metronome-click-off")));
            *color = TextColor(Color::srgb(0.40, 0.40, 0.45));
        } else {
            *text = Text::new(String::from(loc.msg("metronome-click-on")));
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
            .init_resource::<MetronomeTempo>()
            .add_systems(Startup, load_metronome_sounds)
            .add_systems(
                OnEnter(AppState::Playing),
                (reset_click_tracking, set_tempo_from_song),
            )
            .add_systems(OnEnter(AppState::BendingTrainer), reset_click_tracking)
            // Clicks/beat animation: gameplay (unpaused) and the Bending Trainer.
            .add_systems(
                Update,
                (update_metronome, click_metronome).run_if(metronome_running),
            )
            // Toggles + label refreshes stay responsive even while paused. The
            // buttons' click/hover ride along as inline on(...) observers (see
            // spawn_metronome).
            .add_systems(
                Update,
                (
                    toggle_mute_key,
                    update_mute_label,
                    update_feel_label,
                    update_tempo_label,
                )
                    .run_if(metronome_ui_active),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feel_from_chart_maps_each_declared_feel() {
        assert_eq!(
            feel_from_chart(Some(crate::song::chart::Feel::Straight)),
            Some(MetronomeFeel::Straight)
        );
        assert_eq!(
            feel_from_chart(Some(crate::song::chart::Feel::Shuffle)),
            Some(MetronomeFeel::Shuffle)
        );
    }

    #[test]
    fn feel_from_chart_is_none_when_the_chart_declares_nothing() {
        assert_eq!(feel_from_chart(None), None);
    }

    #[test]
    fn no_tick_before_the_song_starts() {
        assert_eq!(tick_index(-0.5, 120.0, MetronomeFeel::Straight), None);
        assert_eq!(tick_index(-3.0, 60.0, MetronomeFeel::Straight), None);
    }

    #[test]
    fn tick_zero_at_clock_zero() {
        assert_eq!(tick_index(0.0, 120.0, MetronomeFeel::Straight), Some(0));
    }

    #[test]
    fn straight_feel_ticks_advance_every_beat_at_120bpm() {
        // In Straight feel a tick is a whole beat, so at 120bpm (0.5s/beat):
        assert_eq!(tick_index(0.49, 120.0, MetronomeFeel::Straight), Some(0));
        assert_eq!(tick_index(0.5, 120.0, MetronomeFeel::Straight), Some(1));
        assert_eq!(tick_index(1.99, 120.0, MetronomeFeel::Straight), Some(3));
    }

    #[test]
    fn invalid_bpm_gives_no_tick() {
        assert_eq!(tick_index(1.0, 0.0, MetronomeFeel::Straight), None);
        assert_eq!(tick_index(1.0, -60.0, MetronomeFeel::Straight), None);
    }

    #[test]
    fn shuffle_feel_ticks_three_times_per_beat() {
        // At 120bpm a beat is 0.5s, so a shuffle tick (triplet-eighth) is
        // 1/6s — three ticks land within the same beat.
        assert_eq!(tick_index(0.0, 120.0, MetronomeFeel::Shuffle), Some(0));
        assert_eq!(tick_index(0.49, 120.0, MetronomeFeel::Shuffle), Some(2));
        assert_eq!(tick_index(0.5, 120.0, MetronomeFeel::Shuffle), Some(3));
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
