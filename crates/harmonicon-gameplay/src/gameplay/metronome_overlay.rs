// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    input_focus::tab_navigation::TabIndex,
    picking::Pickable,
    picking::events::{PointerOut, PointerOver},
    prelude::*,
    ui_widgets::Activate,
    ui_widgets::Button as WidgetButton,
};

use harmonicon_app::app::{AppState, SelectedSong};
use harmonicon_audio::AudioSettings;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::{SongManifest, chart::Feel};
pub use harmonicon_ui::dialogs::metronome::{
    MetronomeFeel, click_for_tick, is_downbeat, tick_index,
};
use harmonicon_ui::music_score::MusicScoreMeter;

use super::bars::{chart_meter, pickup_lead_ticks, ticks_per_beat};

use super::{GameplayClock, GameplayLogic, Paused};

/// The metronome's tempo, decoupled from the song so it can be driven by the
/// gameplay screens (set from the chart) or the standalone Bending Trainer (set
/// from its key/BPM controls).
///
/// Holds the *meter*, not a beat count: every figure a click or a bar
/// tracker needs — how many beats to a bar, how long each is, how long the
/// bar is — comes off it through the methods below, so no caller can
/// derive one of them its own way. The metronome clicks the meter's own
/// beat: an eighth in 6/8, a half in 2/2. `bpm` still counts quarter
/// notes, the same unit a chart's `tempo_bpm` is in everywhere else.
#[derive(Resource)]
pub struct MetronomeTempo {
    pub bpm: f32,
    pub meter: MusicScoreMeter,
    /// Meter beats of bar 0 before the clock's 0 — what a chart's pickup
    /// leaves out (`bars::pickup_lead_ticks`), so the accent falls on bar
    /// 1's downbeat rather than on the pickup. Zero with no pickup, and for
    /// any caller that already counts its own clock from a bar line.
    pub lead_beats: f64,
}

impl Default for MetronomeTempo {
    fn default() -> Self {
        Self { bpm: 90.0, meter: MusicScoreMeter::default(), lead_beats: 0.0 }
    }
}

impl MetronomeTempo {
    /// `clock` on the bar grid: moved on by [`Self::lead_beats`], so a whole
    /// number of bars from 0 is a downbeat.
    pub fn grid_clock(&self, clock: f64) -> f64 {
        clock + self.lead_beats * self.beat_secs()
    }

    /// Beats in a bar, in the meter's own beat — the count the HUD's beat
    /// dots and the downbeat accent use.
    pub fn beats_per_bar(&self) -> usize {
        usize::from(self.meter.numerator.max(1))
    }

    /// Seconds per beat of the meter — what [`tick_index`] wants.
    pub fn beat_secs(&self) -> f64 {
        self.meter.beat_secs(f64::from(self.bpm))
    }

    /// Seconds per bar.
    pub fn bar_secs(&self) -> f64 {
        self.meter.bar_secs(f64::from(self.bpm))
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

#[derive(Component, Default, Clone)]
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
            // Refreshed live from MetronomeTempo (the trainer's BPM control).
            row.spawn_empty().apply_scene(bsn! {
                Text({format!("\u{2669} = {}", bpm as u32)})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.65, 0.65, 0.70)})
                MetronomeTempoLabel
            });

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
                        Text({String::from(loc.msg("metronome-click-on"))})
                        TextFont { font_size: {FontSize::Px(15.0)} }
                        TextColor({Color::srgb(0.65, 0.65, 0.70)})
                        MetronomeMuteLabel
                        Pickable { should_block_lower: {false}, is_hoverable: {false} }
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
                        Text({String::from(loc.msg(feel_label_key(MetronomeFeel::default())))})
                        TextFont { font_size: {FontSize::Px(15.0)} }
                        TextColor({Color::srgb(0.65, 0.65, 0.70)})
                        MetronomeFeelLabel
                        Pickable { should_block_lower: {false}, is_hoverable: {false} }
                    ]
                })
                .insert(BorderColor::all(PILL_BORDER));
        });

    parent
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
        .with_children(|row| {
            for i in 0..beats_per_bar {
                let size = if i == 0 { Val::Px(28.0) } else { Val::Px(22.0) };
                row.spawn_empty().apply_scene(bsn! {
                    Node {
                        width: {size},
                        height: {size},
                        border: {UiRect::all(Val::Px(1.5))},
                    }
                    BackgroundColor({Color::srgba(0.12, 0.12, 0.16, 0.9)})
                    ~{BorderColor::all(Color::srgb(0.35, 0.35, 0.50))}
                    MetronomeBeat(i)
                });
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

    let beat_dur = tempo.beat_secs();
    let beats_per_bar = tempo.beats_per_bar();
    let beat_pos = tempo.grid_clock(clock.get()) / beat_dur;
    let current = beat_pos.floor() as usize % beats_per_bar;
    let t = beat_pos.fract() as f32;

    for (cell, mut bg) in &mut beats {
        let brightness = if cell.0 == current { beat_brightness(t, *feel) } else { 0.0 };
        let is_downbeat = cell.0 == 0;
        let base = if is_downbeat { 0.25 } else { 0.12 };
        let color = Color::srgba(
            base + brightness * 0.9,
            base + brightness * if is_downbeat { 0.4 } else { 0.7 },
            base + brightness * if is_downbeat { 0.1 } else { 0.9 },
            0.9,
        );
        // Only the current beat's dot animates; the rest hold still.
        if bg.0 != color {
            bg.0 = color;
        }
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
    // Nothing before the music starts: the lead only places the grid.
    if clock < 0.0 {
        return;
    }
    let Some(current) = tick_index(tempo.grid_clock(clock), tempo.beat_secs(), feel) else {
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
    let Some((accent, gain)) = click_for_tick(current, tempo.beats_per_bar() as f64, feel) else {
        return;
    };
    let sample = if accent { sounds.downbeat.clone() } else { sounds.beat.clone() };
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
fn pill_over(ev: On<PointerOver>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(PILL_HOVER);
    }
}

fn pill_out(ev: On<PointerOut>, mut colors: Query<&mut BackgroundColor>) {
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
    tempo.meter = chart_meter(&manifest.chart);
    tempo.lead_beats = pickup_lead_ticks(&manifest.chart) as f64
        / ticks_per_beat(manifest.chart.timing.resolution, &tempo.meter) as f64;
    if let Some(chart_feel) = feel_from_chart(manifest.chart.song.feel) {
        *feel = chart_feel;
    }
}

/// Keep the "♩ = NN" readout in step with the live tempo — on a tempo
/// change, or for a label spawned since.
fn update_tempo_label(
    tempo: Res<MetronomeTempo>,
    mut labels: Query<(&mut Text, Ref<MetronomeTempoLabel>)>,
) {
    for (mut text, marker) in &mut labels {
        if tempo.is_changed() || marker.is_added() {
            *text = Text::new(format!("\u{2669} = {}", tempo.bpm as u32));
        }
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

/// Mirror the current feel onto its button label — on a change, or for a
/// label spawned since, since the feel outlives the label across songs.
fn update_feel_label(
    feel: Res<MetronomeFeel>,
    loc: Res<Localization>,
    mut labels: Query<(&mut Text, Ref<MetronomeFeelLabel>)>,
) {
    let all = feel.is_changed() || loc.is_changed();
    for (mut label, marker) in &mut labels {
        if all || marker.is_added() {
            *label = Text::new(String::from(loc.msg(feel_label_key(*feel))));
        }
    }
}

fn update_mute_label(
    muted: Res<MetronomeMuted>,
    loc: Res<Localization>,
    mut labels: Query<(&mut Text, &mut TextColor, Ref<MetronomeMuteLabel>)>,
) {
    // The mute state outlives the label (it survives across songs), so a
    // freshly spawned label is written too, not only a changed setting.
    let all = muted.is_changed() || loc.is_changed();
    for (mut text, mut color, marker) in &mut labels {
        if !all && !marker.is_added() {
            continue;
        }
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
            .add_systems(OnEnter(AppState::Playing), (reset_click_tracking, set_tempo_from_song))
            .add_systems(OnEnter(AppState::BendingTrainer), reset_click_tracking)
            // Clicks/beat animation: gameplay (unpaused) and the Bending Trainer.
            .add_systems(
                Update,
                (update_metronome, click_metronome).after(GameplayLogic).run_if(metronome_running),
            )
            // Toggles + label refreshes stay responsive even while paused. The
            // buttons' click/hover ride along as inline on(...) observers (see
            // spawn_metronome).
            .add_systems(
                Update,
                (toggle_mute_key, update_mute_label, update_feel_label, update_tempo_label)
                    .run_if(metronome_ui_active),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_label_fills_new_labels_and_leaves_idle_ones_alone() {
        let mut world = World::new();
        world.insert_resource(MetronomeMuted(true));
        world.insert_resource(Localization::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(update_mute_label);
        let label = |world: &mut World| {
            world.spawn((Text::new(""), TextColor(Color::WHITE), MetronomeMuteLabel)).id()
        };

        let first = label(&mut world);
        schedule.run(&mut world);
        // With no bundle loaded the key stands in for the text.
        assert_eq!(world.get::<Text>(first).unwrap().0, "metronome-click-off");

        // An idle frame must not touch it.
        world.get_mut::<Text>(first).unwrap().0 = "untouched".into();
        schedule.run(&mut world);
        assert_eq!(world.get::<Text>(first).unwrap().0, "untouched");

        // A label spawned later — the next song's — still starts correct.
        let second = label(&mut world);
        schedule.run(&mut world);
        assert_eq!(world.get::<Text>(second).unwrap().0, "metronome-click-off");
        assert_eq!(world.get::<Text>(first).unwrap().0, "untouched");
    }

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

    /// The beat length at 120 quarter notes a minute in x/4: half a second.
    const Q120: f64 = 0.5;

    #[test]
    fn no_tick_before_the_song_starts() {
        assert_eq!(tick_index(-0.5, Q120, MetronomeFeel::Straight), None);
        assert_eq!(tick_index(-3.0, 1.0, MetronomeFeel::Straight), None);
    }

    #[test]
    fn tick_zero_at_clock_zero() {
        assert_eq!(tick_index(0.0, Q120, MetronomeFeel::Straight), Some(0));
    }

    #[test]
    fn straight_feel_ticks_advance_every_beat_at_120bpm() {
        // In Straight feel a tick is a whole beat, so at 0.5s/beat:
        assert_eq!(tick_index(0.49, Q120, MetronomeFeel::Straight), Some(0));
        assert_eq!(tick_index(0.5, Q120, MetronomeFeel::Straight), Some(1));
        assert_eq!(tick_index(1.99, Q120, MetronomeFeel::Straight), Some(3));
    }

    #[test]
    fn invalid_beat_length_gives_no_tick() {
        assert_eq!(tick_index(1.0, 0.0, MetronomeFeel::Straight), None);
        assert_eq!(tick_index(1.0, -1.0, MetronomeFeel::Straight), None);
    }

    #[test]
    fn shuffle_feel_ticks_three_times_per_beat() {
        // A beat of 0.5s, so a shuffle tick (triplet-eighth) is 1/6s —
        // three ticks land within the same beat.
        assert_eq!(tick_index(0.0, Q120, MetronomeFeel::Shuffle), Some(0));
        assert_eq!(tick_index(0.49, Q120, MetronomeFeel::Shuffle), Some(2));
        assert_eq!(tick_index(0.5, Q120, MetronomeFeel::Shuffle), Some(3));
    }

    #[test]
    fn the_metronome_clicks_the_meters_own_beat() {
        use harmonicon_ui::music_score::parse_time_signature;
        // Greensleeves: 6/8 at 180. The old click ran at 60/180 = a third
        // of a second — a quarter — and accented every 6 of them: every
        // second bar. The beat is an eighth, six to a 1 s bar.
        let t = MetronomeTempo { bpm: 180.0, meter: parse_time_signature("6/8"), lead_beats: 0.0 };
        assert_eq!(t.beats_per_bar(), 6);
        assert!((t.beat_secs() - 1.0 / 6.0).abs() < 1e-9);
        assert!((t.bar_secs() - 1.0).abs() < 1e-9);
        // So the click at the start of bar 2 (1 s in) is tick 6, a downbeat.
        let tick = tick_index(1.0, t.beat_secs(), MetronomeFeel::Straight).unwrap();
        assert_eq!(tick, 6);
        assert!(is_downbeat(tick, t.beats_per_bar() as f64));

        // Für Elise: 3/8 at 120. A bar is 1.5 quarters, which no whole
        // number of quarter-note clicks could ever accent; three eighths
        // can.
        let t = MetronomeTempo { bpm: 120.0, meter: parse_time_signature("3/8"), lead_beats: 0.0 };
        assert_eq!(t.beats_per_bar(), 3);
        assert!((t.bar_secs() - 0.75).abs() < 1e-9);
        let tick = tick_index(0.75, t.beat_secs(), MetronomeFeel::Straight).unwrap();
        assert!(is_downbeat(tick, 3.0));
    }

    #[test]
    fn a_pickup_moves_the_accent_to_bar_one() {
        // A one-beat pickup in 4/4 at 120: the music's first beat is bar
        // 0's fourth, and the accent falls half a second in.
        let t = MetronomeTempo { bpm: 120.0, lead_beats: 3.0, ..Default::default() };
        let tick_at = |clock: f64| {
            tick_index(t.grid_clock(clock), t.beat_secs(), MetronomeFeel::Straight).unwrap()
        };
        assert!(!is_downbeat(tick_at(0.0), 4.0), "the pickup is unaccented");
        assert!(is_downbeat(tick_at(0.5), 4.0));
        assert!(is_downbeat(tick_at(2.5), 4.0));
    }

    #[test]
    fn in_common_time_nothing_changes() {
        let t = MetronomeTempo { bpm: 120.0, ..Default::default() };
        assert_eq!(t.beats_per_bar(), 4);
        assert!((t.beat_secs() - Q120).abs() < 1e-9);
        assert!((t.bar_secs() - 2.0).abs() < 1e-9);
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
                assert!((0.0..=1.0).contains(&b), "t={t} feel={feel:?} brightness={b}");
            }
        }
    }
}
