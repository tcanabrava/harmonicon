// SPDX-License-Identifier: MIT

//! Guided, microphone-driven standard six-string guitar tuning.
use crate::menu::{
    routing::MenuPage,
    scene::{self, spawn_back_button, spawn_button, spawn_menu_root},
};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_audio::{
    audio_input::MicStatus,
    pitch_detect::{
        AudioFrame, PitchAlgorithm, PitchAlgorithmOverride, PitchEvent, PitchRange,
        PitchSilenceThreshold,
    },
};
use harmonicon_core::instrument::{
    bass,
    guitar::{self, Tuning},
};
use harmonicon_core::midi::{freq_to_midi, midi_to_freq_hz, midi_to_note};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};
use harmonicon_ui::dialogs::tab_bar::{TabSelect, spawn_tab_bar};

const INSTRUMENTS: [&str; 3] = ["tuner-classical", "tuner-electric", "song-instrument-bass"];
const EXTENDED_GUITAR: &[Tuning] = &[
    Tuning { id: "seven-string", strings: &[64, 59, 55, 50, 45, 40, 35] },
    Tuning { id: "eight-string", strings: &[64, 59, 55, 50, 45, 40, 35, 30] },
];
fn presets(instrument: usize) -> impl Iterator<Item = &'static Tuning> {
    let base = if instrument == 2 { bass::TUNINGS } else { guitar::TUNINGS };
    base.iter().chain(EXTENDED_GUITAR.iter().filter(move |_| instrument == 1))
}
#[derive(Resource, Default)]
struct SetupChoice {
    instrument: usize,
    preset: usize,
}
#[derive(Component)]
struct PresetPicker(Entity);
#[derive(Component)]
struct StringPicker;
const HOLD: f32 = 0.8;
const MAX_ADJUST_CENTS: f32 = 250.0;
const INPUT_THRESHOLD: f32 = 0.0002;

#[derive(Resource)]
struct Tuner {
    notes: Vec<u8>,
    selected: usize,
    tuned: Vec<bool>,
    held: f32,
    frequency: Option<f32>,
    age: f32,
    saved_range: Option<PitchRange>,
}
impl Default for Tuner {
    fn default() -> Self {
        Self::new(&guitar::TUNINGS[0])
    }
}
impl Tuner {
    fn new(preset: &Tuning) -> Self {
        Self {
            notes: preset.strings.iter().rev().copied().collect(),
            tuned: vec![false; preset.strings.len()],
            selected: 0,
            held: 0.0,
            frequency: None,
            age: 0.0,
            saved_range: None,
        }
    }
    fn range(&self) -> PitchRange {
        PitchRange::from_freqs(self.notes.iter().map(|&note| midi_to_freq_hz(note as f32)), 5.0)
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.held = 0.0;
        self.frequency = None;
        self.age = 0.0;
        self.tuned[index] = false;
    }
    fn listen(&mut self, frequency: Option<f32>, dt: f32) {
        self.frequency = frequency.filter(|f| f.is_finite() && *f > 0.0);
        if self.tuned.iter().all(|v| *v) {
            return;
        }
        if self.cents().is_some_and(|c| c.abs() <= 5.0) {
            self.held += dt;
            if self.held >= HOLD {
                self.tuned[self.selected] = true;
                if let Some(next) = self.tuned.iter().position(|v| !v) {
                    self.select(next);
                }
            }
        } else {
            self.held = 0.0;
        }
    }
    fn cents(&self) -> Option<f32> {
        self.frequency
            .map(|f| 1200.0 * (f / midi_to_freq_hz(self.notes[self.selected] as f32)).log2())
    }
}
#[derive(Component)]
enum Readout {
    Note,
    Target,
    Verdict,
    Detail,
    Signal,
}
const INK: Color = Color::srgb(0.91, 0.96, 0.97);
const MUTED: Color = Color::srgb(0.61, 0.72, 0.77);
const MINT: Color = Color::srgb(0.42, 0.87, 0.72);
#[derive(Component)]
struct InputFill;
#[derive(Component)]
struct StringLabel(usize);
#[derive(Component)]
struct MeterNeedle;

/// The meter spans fifty cents either side of the target. More distant
/// pitches stay at the edge while the numeric readout shows the full error.
fn meter_position(cents: f32) -> f32 {
    50.0 + cents.clamp(-50.0, 50.0)
}
fn pitch_color(cents: f32) -> Color {
    if cents.abs() <= 5.0 { MINT } else { Color::srgb(1.0, 0.65, 0.2) }
}

pub(crate) struct TunerPlugin;
impl Plugin for TunerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tuner>()
            .init_resource::<SetupChoice>()
            .add_systems(OnEnter(MenuPage::Tuner), setup)
            .add_systems(
                Update,
                (rebuild_pickers, listen, sync_meter, update)
                    .chain()
                    .run_if(in_state(MenuPage::Tuner)),
            )
            .add_systems(OnExit(MenuPage::Tuner), (scene::cleanup_menu, restore_range));
    }
}
fn setup(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    mut tuner: ResMut<Tuner>,
    mut range: ResMut<PitchRange>,
    mut choice: ResMut<SetupChoice>,
) {
    commands.insert_resource(PitchSilenceThreshold(INPUT_THRESHOLD));
    commands.insert_resource(PitchAlgorithmOverride(PitchAlgorithm::Yin));
    let saved = *range;
    *tuner = Tuner { saved_range: Some(saved), ..default() };
    *choice = SetupChoice::default();
    *range = tuner.range();
    let (root, header, page) = spawn_menu_root(
        &mut commands,
        &loc.msg("tuner-title"),
        Some(&loc.msg("tuner-subtitle")),
        &theme,
        "Tuner",
    );
    let card = commands
        .spawn((
            Node {
                width: Val::Px(650.0),
                max_width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(28.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(22.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.047, 0.078, 0.104)),
            BorderColor::all(Color::srgb(0.20, 0.32, 0.36)),
        ))
        .id();
    commands.entity(root).add_child(card);
    let labels = INSTRUMENTS.map(|key| loc.msg(key).to_string());
    let tabs = spawn_tab_bar(
        &mut commands,
        card,
        &labels,
        0,
        |ev: On<TabSelect>, mut choice: ResMut<SetupChoice>| {
            *choice = SetupChoice { instrument: ev.index, preset: 0 };
        },
    );
    commands.entity(tabs).insert(wrapping_row());
    let picker = commands.spawn((Node::default(), PresetPicker(page))).id();
    commands.entity(card).add_child(picker);
    spawn_readout(&mut commands, card, Readout::Target, 15.0, 24.0, MUTED);
    spawn_readout(&mut commands, card, Readout::Note, 72.0, 88.0, INK);
    spawn_meter(&mut commands, card, &loc);
    spawn_readout(&mut commands, card, Readout::Verdict, 29.0, 38.0, MINT);
    spawn_readout(&mut commands, card, Readout::Detail, 16.0, 43.0, MUTED);

    let strings = commands.spawn((wrapping_row(), StringPicker)).id();
    commands.entity(card).add_child(strings);
    spawn_readout(&mut commands, card, Readout::Signal, 13.0, 24.0, MUTED);
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(5.0),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.13, 0.21, 0.25)),
        ))
        .id();
    commands.entity(card).add_child(track);
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(MINT),
            InputFill,
        ))
        .id();
    commands.entity(track).add_child(fill);
    let restart = spawn_button(
        &mut commands,
        card,
        &loc.msg("tuner-restart"),
        |_: On<Activate>, mut tuner: ResMut<Tuner>| {
            tuner.tuned.fill(false);
            tuner.select(0);
        },
    );
    commands.entity(restart).insert(Node {
        padding: UiRect::axes(Val::Px(16.0), Val::Px(7.0)),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
fn spawn_readout(
    commands: &mut Commands,
    parent: Entity,
    slot: Readout,
    size: f32,
    height: f32,
    color: Color,
) {
    let row = commands
        .spawn((
            Text::default(),
            TextFont { font_size: FontSize::Px(size), ..default() },
            TextLayout::justify(Justify::Center),
            TextColor(color),
            slot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(row);
}
fn wrapping_row() -> Node {
    Node {
        width: Val::Percent(100.0),
        max_width: Val::Percent(100.0),
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::Center,
        column_gap: Val::Px(6.0),
        row_gap: Val::Px(6.0),
        ..default()
    }
}
fn rebuild_pickers(
    mut commands: Commands,
    choice: Res<SetupChoice>,
    loc: Res<Localization>,
    mut tuner: ResMut<Tuner>,
    mut range: ResMut<PitchRange>,
    preset_picker: Query<(Entity, Ref<PresetPicker>)>,
    string_picker: Query<Entity, With<StringPicker>>,
    mut last: Local<Option<(usize, usize)>>,
    mut backdrop_host: Local<Option<Entity>>,
) {
    let (Ok((picker, marker)), Ok(strings)) = (preset_picker.single(), string_picker.single())
    else {
        return;
    };
    let selection = (choice.instrument, choice.preset);
    let picker_new = marker.is_added();
    if *last == Some(selection) && !picker_new {
        return;
    }
    if last.is_none_or(|old| old.0 != selection.0) || picker_new {
        commands.entity(picker).despawn_related::<Children>();
        let labels: Vec<_> = presets(choice.instrument)
            .map(|preset| loc.msg(&format!("guitar-tuning-{}", preset.id)).to_string())
            .collect();
        // The overlay sits outside the scrolling card. Replace it along
        // with the picker when switching instrument families.
        if let Some(old_host) = backdrop_host.take().filter(|_| !picker_new) {
            commands.entity(old_host).despawn();
        }
        let host = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(marker.0).add_child(host);
        *backdrop_host = Some(host);
        spawn_combobox(
            &mut commands,
            picker,
            host,
            &loc.msg("tuner-tuning-label"),
            &labels,
            &labels[choice.preset],
            |ev: On<ComboboxSelect>, mut choice: ResMut<SetupChoice>| choice.preset = ev.index,
        );
    }
    *last = Some(selection);
    let preset = presets(choice.instrument).nth(choice.preset).expect("valid tuning tab");
    let saved_range = tuner.saved_range;
    *tuner = Tuner { saved_range, ..Tuner::new(preset) };
    *range = tuner.range();
    commands.entity(strings).despawn_related::<Children>();
    for (i, &note) in tuner.notes.iter().enumerate() {
        let tile = spawn_button(
            &mut commands,
            strings,
            &midi_to_note(note as i32),
            move |_: On<Activate>, mut tuner: ResMut<Tuner>| tuner.select(i),
        );
        commands.entity(tile).insert((
            StringLabel(i),
            Node {
                width: Val::Px(62.0),
                height: Val::Px(56.0),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ));
    }
}
fn spawn_meter(commands: &mut Commands, parent: Entity, loc: &Localization) {
    let panel = commands
        .spawn(Node {
            width: Val::Percent(88.0),
            max_width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(parent).add_child(panel);
    let captions =
        commands.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    commands.entity(panel).add_child(captions);
    for key in ["tuner-meter-flat", "tuner-meter-center", "tuner-meter-sharp"] {
        let text = commands
            .spawn((
                Text::new(loc.msg(key)),
                TextFont { font_size: FontSize::Px(18.0), ..default() },
            ))
            .id();
        commands.entity(captions).add_child(text);
    }
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.15, 0.2)),
        ))
        .id();
    commands.entity(panel).add_child(track);
    // The green band is the ±5-cent acceptance window.
    let center = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(45.0),
                width: Val::Percent(10.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.4, 0.23)),
        ))
        .id();
    commands.entity(track).add_child(center);
    for percent in [0.0, 25.0, 50.0, 75.0, 100.0] {
        let tick = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(percent),
                    width: Val::Px(1.0),
                    height: Val::Px(12.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
            ))
            .id();
        commands.entity(track).add_child(tick);
    }
    let needle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                width: Val::Px(4.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Visibility::Hidden,
            MeterNeedle,
        ))
        .id();
    commands.entity(track).add_child(needle);
}
fn restore_range(mut commands: Commands, mut tuner: ResMut<Tuner>, mut range: ResMut<PitchRange>) {
    commands.remove_resource::<PitchAlgorithmOverride>();
    commands.remove_resource::<PitchSilenceThreshold>();
    if let Some(saved) = tuner.saved_range.take() {
        *range = saved;
    }
}
fn listen(
    mut events: MessageReader<PitchEvent>,
    time: Res<Time>,
    mic: Option<Res<MicStatus>>,
    mut tuner: ResMut<Tuner>,
) {
    let dt = time.delta_secs();
    tuner.age += dt;
    let latest = events.read().last();
    let frequency = if let Some(event) = latest {
        tuner.age = 0.0;
        // Multiple simultaneous pitches cannot reliably identify an open string.
        if event.0.len() == 1 { Some(event.0[0].frequency) } else { None }
    } else if tuner.age <= 0.15 {
        tuner.frequency
    } else {
        None
    };
    let connected = mic.as_ref().is_some_and(|m| m.is_connected());
    tuner.listen(if connected { frequency } else { None }, dt);
}
fn sync_meter(
    tuner: Res<Tuner>,
    mut needle: Query<(&mut Node, &mut BackgroundColor, &mut Visibility), With<MeterNeedle>>,
) {
    for (mut node, mut color, mut visibility) in &mut needle {
        if let Some(cents) = tuner.cents().filter(|c| c.abs() <= MAX_ADJUST_CENTS) {
            node.left = Val::Percent(meter_position(cents));
            color.0 = pitch_color(cents);
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}
fn update(
    frame: Res<AudioFrame>,
    mut input: Query<&mut Node, With<InputFill>>,
    loc: Res<Localization>,
    mic: Option<Res<MicStatus>>,
    tuner: Res<Tuner>,
    mut readout: Query<(&Readout, &mut Text, &mut TextColor)>,
    mut rows: Query<(&StringLabel, &Children, &mut BackgroundColor)>,
    mut labels: Query<(&mut Text, &mut TextColor), Without<Readout>>,
) {
    let connected = mic.as_ref().is_some_and(|m| m.is_connected());
    let rms = (frame.samples.iter().map(|s| s * s).sum::<f32>()
        / frame.samples.len().max(1) as f32)
        .sqrt();
    let percent = if connected {
        ((20.0 * rms.max(1e-12).log10() + 80.0) / 80.0 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    for mut node in &mut input {
        node.width = Val::Percent(percent);
    }
    let signal = if let Some(MicStatus::Connected { device_name }) = mic.as_deref() {
        loc.msg_args(
            "tuner-input",
            &[("percent", format!("{percent:.0}")), ("device", device_name.clone())],
        )
    } else {
        loc.msg("tuner-mic-inactive")
    };
    let i = tuner.selected;
    let note = midi_to_note(tuner.notes[i] as i32);
    let target = loc.msg_args(
        "tuner-target-caption",
        &[
            ("number", (tuner.notes.len() - i).to_string()),
            ("frequency", format!("{:.1}", midi_to_freq_hz(tuner.notes[i] as f32))),
        ],
    );
    let cents = tuner.cents();
    let (verdict, detail) = if !connected {
        let detail = match mic.as_deref() {
            Some(MicStatus::AwaitingPermission) => loc.msg("tuner-permission"),
            Some(MicStatus::Failed { reason }) => {
                loc.msg_args("tuner-mic-failed", &[("reason", reason.clone())])
            }
            _ => loc.msg("tuner-no-mic"),
        };
        (loc.msg("tuner-mic-inactive"), detail)
    } else if tuner.tuned.iter().all(|v| *v) {
        (loc.msg("tuner-all-tuned"), loc.msg("tuner-recheck"))
    } else {
        match cents {
            Some(c) if c.abs() > MAX_ADJUST_CENTS => (
                loc.msg("tuner-check-string"),
                loc.msg_args(
                    "tuner-other-note",
                    &[(
                        "note",
                        tuner
                            .frequency
                            .and_then(freq_to_midi)
                            .map(midi_to_note)
                            .unwrap_or_default(),
                    )],
                ),
            ),
            Some(c) => {
                let key = if c < -5.0 {
                    "tuner-tune-up"
                } else if c > 5.0 {
                    "tuner-tune-down"
                } else {
                    "tuner-meter-center"
                };
                (
                    loc.msg(key),
                    loc.msg_args(
                        "tuner-pitch-detail",
                        &[
                            ("cents", format!("{c:+.0}")),
                            ("frequency", format!("{:.1}", tuner.frequency.unwrap())),
                        ],
                    ),
                )
            }
            None => (
                loc.msg("tuner-ready"),
                loc.msg(if rms >= INPUT_THRESHOLD {
                    "tuner-unclear"
                } else {
                    "tuner-instructions"
                }),
            ),
        }
    };
    for (slot, mut text, mut color) in &mut readout {
        let value: &str = match slot {
            Readout::Note => &note,
            Readout::Target => &target,
            Readout::Verdict => &verdict,
            Readout::Detail => &detail,
            Readout::Signal => &signal,
        };
        if text.0 != value {
            text.0 = value.to_string();
        }
        if matches!(slot, Readout::Verdict) {
            let tint = cents.filter(|c| c.abs() <= MAX_ADJUST_CENTS).map_or(MINT, pitch_color);
            if color.0 != tint {
                color.0 = tint;
            }
        }
    }
    for (row, children, mut background) in &mut rows {
        let n = row.0;
        let Some(&note) = tuner.notes.get(n) else { continue };
        let surface =
            if n == i { Color::srgb(0.12, 0.29, 0.28) } else { Color::srgb(0.09, 0.15, 0.19) };
        if background.0 != surface {
            background.0 = surface;
        }
        for child in children.iter() {
            if let Ok((mut text, mut color)) = labels.get_mut(child) {
                let value = loc
                    .msg_args(
                        "tuner-string-tile",
                        &[
                            ("number", (tuner.notes.len() - n).to_string()),
                            ("note", midi_to_note(note as i32)),
                            (
                                "marker",
                                if tuner.tuned[n] {
                                    "✓"
                                } else if n == i {
                                    "▶"
                                } else {
                                    " "
                                }
                                .to_string(),
                            ),
                        ],
                    )
                    .to_string();
                if text.0 != value {
                    text.0 = value;
                }
                let tint = if tuner.tuned[n] {
                    MINT
                } else if n == i {
                    MINT
                } else {
                    Color::WHITE
                };
                if color.0 != tint {
                    color.0 = tint;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_preserve_string_counts_and_detect_low_bass_b() {
        use harmonicon_audio::pitch_detect::{FftState, analyze_with_silence_threshold};
        assert_eq!(presets(0).next().unwrap().strings.len(), 6);
        assert_eq!(presets(1).last().unwrap().strings.len(), 8);
        assert_eq!(presets(2).next().unwrap().strings.len(), 4);
        let tuner = Tuner::new(presets(2).nth(1).unwrap());
        assert_eq!(tuner.notes, [23, 28, 33, 38, 43]);
        for sample_rate in [44100, 48000] {
            let target = midi_to_freq_hz(23.0);
            let samples: Vec<_> = (0..4096)
                .map(|i| {
                    let phase = std::f32::consts::TAU * target * i as f32 / sample_rate as f32;
                    0.02 * phase.sin() + 0.03 * (2.0 * phase).sin()
                })
                .collect();
            let result = analyze_with_silence_threshold(
                &samples,
                sample_rate,
                &mut FftState::default(),
                PitchAlgorithm::Yin,
                *tuner.range(),
                INPUT_THRESHOLD,
            );
            assert_eq!(result.pitches.len(), 1);
            assert!((result.pitches[0].frequency - target).abs() < 0.1);
        }
    }
    #[test]
    fn analyzed_guitar_audio_drives_meter_and_silence_hides_it() {
        use harmonicon_audio::pitch_detect::{FftState, analyze, analyze_with_silence_threshold};
        let mut app = App::new();
        app.init_resource::<Tuner>()
            .init_resource::<SetupChoice>()
            .init_resource::<Time>()
            .insert_resource(MicStatus::Connected { device_name: "test".into() })
            .add_message::<PitchEvent>()
            .add_systems(Update, (listen, sync_meter).chain());
        let needle = app
            .world_mut()
            .spawn((
                Node::default(),
                BackgroundColor(Color::WHITE),
                Visibility::Hidden,
                MeterNeedle,
            ))
            .id();
        let mut fft = FftState::default();
        for cents in [-20.0_f32, 20.0, 0.0] {
            let frequency = midi_to_freq_hz(40.0) * 2.0_f32.powf(cents / 1200.0);
            let samples: Vec<_> = (0..4096)
                .map(|i| {
                    let phase = std::f32::consts::TAU * frequency * i as f32 / 44100.0;
                    0.002 * phase.sin() + 0.0006 * (2.0 * phase).sin()
                })
                .collect();
            let range = *PitchRange::from_freqs([65.0, 400.0], 0.0);
            assert!(
                analyze(&samples, 44100, &mut fft, PitchAlgorithm::Yin, range).pitches.is_empty()
            );
            let result = analyze_with_silence_threshold(
                &samples,
                44100,
                &mut fft,
                PitchAlgorithm::Yin,
                range,
                INPUT_THRESHOLD,
            );
            assert_eq!(result.pitches.len(), 1);
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            app.world_mut().write_message(PitchEvent(result.pitches));
            app.update();
            let detected = app.world().resource::<Tuner>().cents().unwrap();
            assert!((detected - cents).abs() < 3.0, "{detected} vs {cents}");
            let entity = app.world().entity(needle);
            assert_eq!(entity.get::<Visibility>(), Some(&Visibility::Inherited));
            assert_eq!(entity.get::<Node>().unwrap().left, Val::Percent(meter_position(detected)));
            assert_eq!(entity.get::<BackgroundColor>().unwrap().0, pitch_color(cents));
        }
        app.world_mut().write_message(PitchEvent(Vec::new()));
        app.update();
        assert_eq!(app.world().entity(needle).get::<Visibility>(), Some(&Visibility::Hidden));
        assert_eq!(meter_position(-1200.0), 0.0);
        assert_eq!(meter_position(1200.0), 100.0);
    }
    #[test]
    fn requires_continuous_correct_pitch_and_advances_all_strings() {
        let mut tuner = Tuner::default();
        tuner.listen(Some(midi_to_freq_hz(40.0)), 0.5);
        tuner.listen(None, 0.1);
        tuner.listen(Some(midi_to_freq_hz(40.0)), 0.5);
        assert!(!tuner.tuned[0]);
        for note in tuner.notes.clone() {
            for _ in 0..10 {
                tuner.listen(Some(midi_to_freq_hz(note as f32)), 0.1);
            }
        }
        assert!(tuner.tuned.iter().all(|v| *v));
        tuner.select(2);
        assert!(!tuner.tuned[2]);
    }
    #[test]
    fn cents_preserve_flat_and_sharp_direction() {
        let mut tuner = Tuner::default();
        let target = midi_to_freq_hz(40.0);
        for cents in [-25.0_f32, 25.0] {
            tuner.listen(Some(target * 2.0_f32.powf(cents / 1200.0)), 0.1);
            assert!((tuner.cents().unwrap() - cents).abs() < 0.01);
            assert!(!tuner.tuned[0]);
        }
    }
    #[test]
    fn wrong_octave_and_invalid_frequency_never_complete() {
        let mut tuner = Tuner::default();
        for f in [f32::NAN, 0.0, -1.0, midi_to_freq_hz(52.0)] {
            tuner.listen(Some(f), 2.0);
        }
        assert!(!tuner.tuned[0]);
    }
}
