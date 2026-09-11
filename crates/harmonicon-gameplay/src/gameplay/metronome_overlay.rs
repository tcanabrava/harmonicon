// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    input_focus::tab_navigation::TabIndex,
    picking::Pickable,
    picking::events::{Out, Over, Pointer},
    prelude::*,
    ui_widgets::Activate,
    ui_widgets::Button as WidgetButton,
};
use bevy_fluent::Localization;

use harmonicon_app::app::{AppState, SelectedSong};
use harmonicon_audio::AudioSettings;
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_song::song::{SongManifest, chart::Feel};
pub use harmonicon_ui::dialogs::metronome::{
    MetronomeFeel, click_for_tick, is_downbeat, tick_index,
};

use super::{GameplayClock, GameplayLogic, Paused};

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
// ── UI ────────────────────────────────────────────────────────────────────────

pub fn spawn_metronome(
    parent: &mut ChildSpawnerCommands,
    loc: &Localization,
    beats_per_bar: usize,
    bpm: f32,
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
                    WidgetButton
                    TabIndex(0)
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
                            Text({String::from(loc.msg("metronome-click-on"))})
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
                    WidgetButton
                    TabIndex(0)
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
                            Text({String::from(loc.msg(feel_label_key(MetronomeFeel::default())))})
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

/// How bright the current beat's dot should be at `t` (its fraction, `0.0`
/// to `1.0`, through that beat) — `Straight` decays once from the beat's
/// own onset; `Shuffle` decays *twice*: once from the beat click at `t=0`
/// (full brightness) and once from the swung "and" click at `t=2/3` (a
/// softer peak, matching that subdivision's own `0.55` audio gain in
/// [`click_for_tick`]) — so a shuffle-feel dot visibly pulses twice per
/// beat instead of once, matching what [`play_click_if_due`] actually
/// plays instead of a single beat-long decay that goes visually silent
/// exactly where the audio swings. `2.0/3.0` is the same long/short split
/// [`tick_index`]'s triplet-eighth division uses (a beat's first two
/// triplet-eighths are the "long" note, the third is the "short" one).
fn beat_brightness(t: f32, feel: MetronomeFeel) -> f32 {
    const SWUNG_AND_FRAC: f32 = 2.0 / 3.0;
    const SWUNG_AND_GAIN: f32 = 0.55;
    match feel {
        MetronomeFeel::Straight => (1.0 - t).powf(1.5),
        MetronomeFeel::Shuffle => {
            if t < SWUNG_AND_FRAC {
                (1.0 - t / SWUNG_AND_FRAC).powf(1.5)
            } else {
                SWUNG_AND_GAIN * (1.0 - (t - SWUNG_AND_FRAC) / (1.0 - SWUNG_AND_FRAC)).powf(1.5)
            }
        }
    }
}

pub fn update_metronome(
    clock: Res<GameplayClock>,
    tempo: Res<MetronomeTempo>,
    feel: Res<MetronomeFeel>,
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
    let t = beat_pos.fract() as f32;

    for (cell, mut bg) in &mut beats {
        let brightness = if cell.0 == current {
            beat_brightness(t, *feel)
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

/// Plays whichever click `clock` maps to, if any — the shared core behind
/// [`click_metronome`] (this module's own `GameplayClock`-driven system)
/// and `song_editor::metronome`'s `Playhead`-driven equivalent, so the
/// actual click-selection/gain/audio-spawn logic (plain quarters in
/// straight feel, or the swung long-short shuffle pattern, accent on the
/// downbeat) exists in exactly one place regardless of which clock is
/// counting. `last` ensures each tick only plays once; callers own their
/// own `last` so two independent clocks (gameplay's vs. the editor's)
/// can't suppress or double-fire each other's first click.
pub fn play_click_if_due(
    clock: f64,
    tempo: &MetronomeTempo,
    feel: MetronomeFeel,
    muted: bool,
    sounds: &MetronomeSounds,
    audio: &AudioSettings,
    last: &mut Option<i64>,
    commands: &mut Commands,
) {
    let Some(current) = tick_index(clock, tempo.bpm as f64, feel) else {
        return;
    };
    if *last == Some(current) {
        return;
    }
    *last = Some(current);

    if muted {
        return;
    }
    // Silent subdivisions (the skipped middle triplet of a shuffle) play nothing.
    let Some((accent, gain)) = click_for_tick(current, tempo.beats_per_bar as f64, feel) else {
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

/// Plays the metronome clicks for gameplay's own clock (`GameplayClock`),
/// so they stay in sync through pause/resume and loop-region jumps. See
/// [`play_click_if_due`] for the shared click logic itself.
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
    play_click_if_due(
        clock.get(),
        &tempo,
        *feel,
        muted.0,
        &sounds,
        &audio,
        &mut last.0,
        &mut commands,
    );
}

// ── Toggles (mute + feel) ──────────────────────────────────────────────────────

fn toggle_mute_key(keyboard: Res<ButtonInput<KeyCode>>, mut muted: ResMut<MetronomeMuted>) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
    }
}

// Button behaviour, wired inline as on(...) observers at spawn.
fn toggle_mute(_: On<Activate>, mut muted: ResMut<MetronomeMuted>) {
    muted.0 = !muted.0;
}

/// Flip the click subdivision between straight and shuffle. The label follows
/// via `update_feel_label`.
fn toggle_feel(_: On<Activate>, mut feel: ResMut<MetronomeFeel>) {
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

/// The Fluent key for `feel`'s button label — shared by the initial `bsn!`
/// placeholder and [`update_feel_label`] so the two can't drift apart.
fn feel_label_key(feel: MetronomeFeel) -> &'static str {
    match feel {
        MetronomeFeel::Straight => "metronome-feel-straight",
        MetronomeFeel::Shuffle => "metronome-feel-shuffle",
    }
}

/// Mirror the current feel onto its button label (written every frame, like
/// `update_mute_label`, so a freshly spawned label isn't stale across songs).
fn update_feel_label(
    feel: Res<MetronomeFeel>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<MetronomeFeelLabel>>,
) {
    let text = String::from(loc.msg(feel_label_key(*feel)));
    for mut label in &mut labels {
        *label = Text::new(text.clone());
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
                (update_metronome, click_metronome)
                    .after(GameplayLogic)
                    .run_if(metronome_running),
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
            feel_from_chart(Some(harmonicon_core::chart::Feel::Straight)),
            Some(MetronomeFeel::Straight)
        );
        assert_eq!(
            feel_from_chart(Some(harmonicon_core::chart::Feel::Shuffle)),
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

    // ── beat_brightness ──────────────────────────────────────────────────────

    #[test]
    fn straight_brightness_decays_once_from_the_beat_onset() {
        assert_eq!(beat_brightness(0.0, MetronomeFeel::Straight), 1.0);
        assert!(beat_brightness(0.999, MetronomeFeel::Straight) < 0.01);
    }

    #[test]
    fn shuffle_brightness_peaks_twice_per_beat() {
        // Full peak on the beat click...
        assert_eq!(beat_brightness(0.0, MetronomeFeel::Shuffle), 1.0);
        // ...decayed to near-nothing just before the swung "and"...
        assert!(beat_brightness(2.0 / 3.0 - 0.001, MetronomeFeel::Shuffle) < 0.01);
        // ...then a second, softer peak right at it, matching
        // click_for_tick's own 0.55 gain for that subdivision...
        assert!((beat_brightness(2.0 / 3.0, MetronomeFeel::Shuffle) - 0.55).abs() < 1e-4);
        // ...decaying again toward the next beat.
        assert!(beat_brightness(0.999, MetronomeFeel::Shuffle) < 0.01);
    }

    #[test]
    fn beat_brightness_never_leaves_the_unit_range() {
        for i in 0..1000 {
            let t = i as f32 / 1000.0;
            for feel in [MetronomeFeel::Straight, MetronomeFeel::Shuffle] {
                let b = beat_brightness(t, feel);
                assert!(
                    (0.0..=1.0).contains(&b),
                    "t={t} feel={feel:?} brightness={b}"
                );
            }
        }
    }
}
