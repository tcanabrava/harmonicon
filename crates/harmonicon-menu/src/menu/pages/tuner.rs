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
use harmonicon_core::midi::{midi_to_freq_hz, midi_to_note};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

const NOTES: [u8; 6] = [40, 45, 50, 55, 59, 64];
const HOLD: f32 = 0.8;
const INPUT_THRESHOLD: f32 = 0.0002;

#[derive(Resource, Default)]
struct Tuner {
    electric: bool,
    selected: usize,
    tuned: [bool; 6],
    held: f32,
    frequency: Option<f32>,
    age: f32,
    saved_range: Option<PitchRange>,
}
impl Tuner {
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
        self.frequency.map(|f| 1200.0 * (f / midi_to_freq_hz(NOTES[self.selected] as f32)).log2())
    }
}
#[derive(Component)]
struct Readout;
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
    if cents.abs() <= 5.0 { Color::srgb(0.2, 0.9, 0.45) } else { Color::srgb(1.0, 0.65, 0.2) }
}

pub(crate) struct TunerPlugin;
impl Plugin for TunerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tuner>()
            .add_systems(OnEnter(MenuPage::Tuner), setup)
            .add_systems(
                Update,
                (listen, sync_meter, update).chain().run_if(in_state(MenuPage::Tuner)),
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
) {
    commands.insert_resource(PitchSilenceThreshold(INPUT_THRESHOLD));
    commands.insert_resource(PitchAlgorithmOverride(PitchAlgorithm::Yin));
    let saved = *range;
    *tuner = Tuner { saved_range: Some(saved), ..default() };
    *range = PitchRange::from_freqs([65.0, 400.0], 0.0);
    let (root, header, _) = spawn_menu_root(
        &mut commands,
        &loc.msg("tuner-title"),
        Some(&loc.msg("tuner-subtitle")),
        &theme,
        "Tuner",
    );
    let choices = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(choices);
    for (label, electric) in [("tuner-classical", false), ("tuner-electric", true)] {
        spawn_button(
            &mut commands,
            choices,
            &loc.msg(label),
            move |_: On<Activate>, mut tuner: ResMut<Tuner>| {
                let saved_range = tuner.saved_range;
                *tuner = Tuner { electric, saved_range, ..default() };
            },
        );
    }
    let instructions = commands
        .spawn((
            Text::new(loc.msg("tuner-instructions")),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            Node { max_width: Val::Px(700.0), ..default() },
        ))
        .id();
    commands.entity(root).add_child(instructions);
    let track = commands
        .spawn((
            Node {
                width: Val::Px(470.0),
                max_width: Val::Percent(100.0),
                height: Val::Px(12.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.15, 0.2)),
        ))
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgb(0.2, 0.85, 0.4)),
            InputFill,
        ))
        .id();
    commands.entity(track).add_child(fill);
    commands.entity(root).add_child(track);
    spawn_meter(&mut commands, root, &loc);
    let readout = commands
        .spawn((
            Text::new(loc.msg("tuner-pluck")),
            TextFont { font_size: FontSize::Px(22.0), ..default() },
            Readout,
            Node { max_width: Val::Px(740.0), ..default() },
        ))
        .id();
    commands.entity(root).add_child(readout);
    for (i, note) in NOTES.iter().enumerate() {
        let row = spawn_button(
            &mut commands,
            root,
            &loc.msg_args(
                "tuner-string",
                &[("number", (6 - i).to_string()), ("note", midi_to_note(*note as i32))],
            ),
            move |_: On<Activate>, mut tuner: ResMut<Tuner>| tuner.select(i),
        );
        commands.entity(row).insert(Node {
            width: Val::Px(470.0),
            max_width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            ..default()
        });
        // The button helper's label is its direct Text child; mark it for live progress.
        commands.entity(row).insert(StringLabel(i));
    }
    spawn_button(
        &mut commands,
        root,
        &loc.msg("tuner-restart"),
        |_: On<Activate>, mut tuner: ResMut<Tuner>| {
            tuner.tuned = [false; 6];
            tuner.select(0);
        },
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
fn spawn_meter(commands: &mut Commands, parent: Entity, loc: &Localization) {
    let panel = commands
        .spawn(Node {
            width: Val::Px(470.0),
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
                height: Val::Px(44.0),
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
        if let Some(cents) = tuner.cents() {
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
    mut readout: Query<&mut Text, With<Readout>>,
    rows: Query<(&StringLabel, &Children)>,
    mut labels: Query<&mut Text, Without<Readout>>,
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
    let signal = loc.msg_args(
        "tuner-input",
        &[
            ("percent", format!("{percent:.0}")),
            (
                "device",
                match mic.as_deref() {
                    Some(MicStatus::Connected { device_name }) => device_name.clone(),
                    _ => String::new(),
                },
            ),
        ],
    );
    let i = tuner.selected;
    let instrument =
        loc.msg(if tuner.electric { "tuner-electric-tip" } else { "tuner-classical-tip" });
    let status = if !connected {
        match mic.as_deref() {
            Some(MicStatus::AwaitingPermission) => loc.msg("tuner-permission"),
            Some(MicStatus::Failed { reason }) => {
                loc.msg_args("tuner-mic-failed", &[("reason", reason.clone())])
            }
            _ => loc.msg("tuner-no-mic"),
        }
    } else if tuner.tuned.iter().all(|v| *v) {
        loc.msg("tuner-complete")
    } else {
        let feedback = match tuner.cents() {
            None => {
                loc.msg(if rms >= INPUT_THRESHOLD { "tuner-unclear" } else { "tuner-listening" })
            }
            Some(c) if c.abs() <= 5.0 => loc.msg_args(
                "tuner-hold",
                &[("percent", format!("{:.0}", (tuner.held / HOLD * 100.0).min(100.0)))],
            ),
            Some(c) => loc.msg_args(
                "tuner-adjust",
                &[
                    (
                        "direction",
                        loc.msg(if c < 0.0 { "tuner-tighten" } else { "tuner-loosen" }).to_string(),
                    ),
                    ("cents", format!("{c:+.0}")),
                    ("frequency", format!("{:.1}", tuner.frequency.unwrap())),
                ],
            ),
        };
        loc.msg_args(
            "tuner-target",
            &[
                ("number", (6 - i).to_string()),
                ("note", midi_to_note(NOTES[i] as i32)),
                ("frequency", format!("{:.2}", midi_to_freq_hz(NOTES[i] as f32))),
                ("feedback", feedback.to_string()),
            ],
        )
    };
    for mut text in &mut readout {
        **text = format!("{signal}\n{instrument}\n{status}");
    }
    for (row, children) in &rows {
        let n = row.0;
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child) {
                **text = loc
                    .msg_args(
                        "tuner-row",
                        &[
                            ("selected", if n == i { "→" } else { " " }.to_string()),
                            ("number", (6 - n).to_string()),
                            ("note", midi_to_note(NOTES[n] as i32)),
                            ("frequency", format!("{:.2}", midi_to_freq_hz(NOTES[n] as f32))),
                            (
                                "status",
                                if tuner.tuned[n] {
                                    loc.msg("tuner-tuned").to_string()
                                } else {
                                    String::new()
                                },
                            ),
                        ],
                    )
                    .to_string();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analyzed_guitar_audio_drives_meter_and_silence_hides_it() {
        use harmonicon_audio::pitch_detect::{FftState, analyze, analyze_with_silence_threshold};
        let mut app = App::new();
        app.init_resource::<Tuner>()
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
        for note in NOTES {
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
