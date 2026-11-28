// SPDX-License-Identifier: MIT

//! Guided, microphone-driven tuning for the instrument chosen on Select
//! Instrument. Only the fretted instruments have one (`tuner_family`); the
//! Play menu offers it for those alone, and the page opens on that
//! instrument's saved tuning and capo.
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
    InstrumentKind, bass, cavaquinho,
    fretted_strings::FrettedKind,
    guitar::{self, Tuning},
};
use harmonicon_core::midi::{freq_to_midi, midi_to_freq_hz, midi_to_note};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::MyInstruments;
use harmonicon_platform::theme::{LoadedTheme, ThemeSurface};
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};

const EXTENDED_GUITAR: &[Tuning] = &[
    Tuning { id: "seven-string", strings: &[64, 59, 55, 50, 45, 40, 35] },
    Tuning { id: "eight-string", strings: &[64, 59, 55, 50, 45, 40, 35, 30] },
];
/// The instrument family the tuner tunes for `kind`, if it has a tuner.
pub(crate) fn tuner_family(kind: InstrumentKind) -> Option<FrettedKind> {
    match kind {
        InstrumentKind::Guitar => Some(FrettedKind::Guitar),
        InstrumentKind::Bass => Some(FrettedKind::Bass),
        InstrumentKind::Cavaquinho => Some(FrettedKind::Cavaquinho),
        _ => None,
    }
}
fn presets(family: FrettedKind) -> impl Iterator<Item = &'static Tuning> {
    let base = match family {
        FrettedKind::Guitar => guitar::TUNINGS,
        FrettedKind::Bass => bass::TUNINGS,
        FrettedKind::Cavaquinho => cavaquinho::TUNINGS,
    };
    base.iter().chain(EXTENDED_GUITAR.iter().filter(move |_| family == FrettedKind::Guitar))
}
/// The page's starting choice: the family's saved tuning and capo, and the
/// split headstock a guitar usually has.
fn initial_choice(family: FrettedKind, mine: &MyInstruments) -> SetupChoice {
    let preset = mine
        .fretted_tuning(family)
        .and_then(|id| presets(family).position(|preset| preset.id == id))
        .unwrap_or(0);
    let capo = if family == FrettedKind::Bass { 0 } else { mine.fretted_capo(family).unwrap_or(0) };
    let layout = if family == FrettedKind::Guitar { PegLayout::Split } else { PegLayout::Inline };
    SetupChoice { family, preset, capo: capo.min(12), layout }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PegLayout {
    #[default]
    Inline,
    Split,
    Reverse,
}
impl PegLayout {
    const ALL: [Self; 3] = [Self::Inline, Self::Split, Self::Reverse];
    const LABELS: [&'static str; 3] =
        ["tuner-layout-inline", "tuner-layout-split", "tuner-layout-reverse"];

    fn peg(self, index: usize, count: usize) -> (bool, f32) {
        let left_count = count.div_ceil(2);
        let (right, rank, side_count) = match self {
            Self::Split if index >= left_count => (true, count - 1 - index, count - left_count),
            Self::Split => (false, index, left_count),
            Self::Inline => (false, index, count),
            Self::Reverse => (true, index, count),
        };
        let spacing = 190.0 / side_count.saturating_sub(1).max(1) as f32;
        (right, 225.0 - rank as f32 * spacing)
    }
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
struct SetupChoice {
    family: FrettedKind,
    preset: usize,
    capo: u8,
    layout: PegLayout,
}
impl Default for SetupChoice {
    fn default() -> Self {
        Self { family: FrettedKind::Guitar, preset: 0, capo: 0, layout: PegLayout::default() }
    }
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
    candidate: Option<(f32, u8)>,
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
            candidate: None,
            saved_range: None,
        }
    }
    fn with_capo(mut self, fret: u8) -> Self {
        for note in &mut self.notes {
            *note += fret;
        }
        self
    }
    fn range(&self) -> PitchRange {
        PitchRange::from_freqs(self.notes.iter().map(|&note| midi_to_freq_hz(note as f32)), 5.0)
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.held = 0.0;
        self.frequency = None;
        self.age = 0.0;
        self.candidate = None;
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
const MUTED: Color = Color::srgb(0.61, 0.72, 0.77);
const MINT: Color = Color::srgb(0.42, 0.87, 0.72);
#[derive(Component)]
struct InputFill;
#[derive(Component)]
struct StringLabel(usize);
#[derive(Component)]
struct StringWire(usize);
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
    mine: Res<MyInstruments>,
    mut page_state: ResMut<NextState<MenuPage>>,
) {
    let Some(family) = tuner_family(mine.active()) else {
        // The Play menu hides the tuner for these; a stray route goes back.
        page_state.set(MenuPage::Play);
        return;
    };
    commands.insert_resource(PitchSilenceThreshold(INPUT_THRESHOLD));
    commands.insert_resource(PitchAlgorithmOverride(PitchAlgorithm::Yin));
    let saved = *range;
    *tuner = Tuner { saved_range: Some(saved), ..default() };
    *choice = initial_choice(family, &mine);
    *range = tuner.range();
    let (root, header, page) = spawn_menu_root(
        &mut commands,
        &loc.msg("tuner-title"),
        Some(&loc.msg("tuner-subtitle")),
        &theme,
    );
    let palette = theme.palette();
    let card = commands
        .spawn((
            Node {
                width: Val::Px(860.0),
                max_width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(22.0)),
                ..default()
            },
            ThemeSurface::Panel,
            BorderColor::all(palette.border),
        ))
        .id();
    commands.entity(root).add_child(card);
    let picker = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                max_width: Val::Percent(100.0),
                ..default()
            },
            PresetPicker(page),
        ))
        .id();
    commands.entity(card).add_child(picker);
    let workspace = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceEvenly,
            align_items: AlignItems::Center,
            column_gap: Val::Px(18.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(card).add_child(workspace);
    let display = commands
        .spawn(Node {
            width: Val::Px(390.0),
            max_width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(workspace).add_child(display);
    spawn_readout(&mut commands, display, Readout::Target, 15.0, 24.0, palette.muted);
    spawn_readout(&mut commands, display, Readout::Note, 66.0, 80.0, palette.text);
    spawn_meter(&mut commands, display, &loc);
    spawn_readout(&mut commands, display, Readout::Verdict, 29.0, 38.0, palette.accent);
    spawn_readout(&mut commands, display, Readout::Detail, 16.0, 36.0, palette.muted);

    let instrument = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    commands.entity(workspace).add_child(instrument);
    let caption = commands
        .spawn((
            Text::new(loc.msg("tuner-peg-caption")),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(palette.muted),
        ))
        .id();
    let strings = commands
        .spawn((Node { width: Val::Px(256.0), height: Val::Px(300.0), ..default() }, StringPicker))
        .id();
    commands.entity(instrument).add_children(&[caption, strings]);
    spawn_readout(&mut commands, card, Readout::Signal, 13.0, 24.0, palette.muted);
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(5.0),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            ThemeSurface::Inset,
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
            BackgroundColor(palette.accent),
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
fn rebuild_pickers(
    mut commands: Commands,
    choice: Res<SetupChoice>,
    loc: Res<Localization>,
    mut tuner: ResMut<Tuner>,
    mut range: ResMut<PitchRange>,
    preset_picker: Query<(Entity, Ref<PresetPicker>)>,
    string_picker: Query<Entity, With<StringPicker>>,
    mut last: Local<Option<SetupChoice>>,
    mut backdrop_host: Local<Option<Entity>>,
) {
    let (Ok((picker, marker)), Ok(strings)) = (preset_picker.single(), string_picker.single())
    else {
        return;
    };
    let selection = *choice;
    let picker_new = marker.is_added();
    if *last == Some(selection) && !picker_new {
        return;
    }
    if last.is_none_or(|old| old.family != selection.family) || picker_new {
        commands.entity(picker).despawn_related::<Children>();
        let labels: Vec<_> = presets(choice.family)
            .map(|preset| loc.msg(&format!("guitar-tuning-{}", preset.id)).to_string())
            .collect();
        // The overlay sits outside the scrolling card. Replace it along
        // with the picker.
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
        let layout_labels = PegLayout::LABELS.map(|key| loc.msg(key).to_string());
        let current_layout =
            PegLayout::ALL.iter().position(|layout| *layout == choice.layout).unwrap_or(0);
        spawn_combobox(
            &mut commands,
            picker,
            host,
            &loc.msg("tuner-layout-label"),
            &layout_labels,
            &layout_labels[current_layout],
            |ev: On<ComboboxSelect>, mut choice: ResMut<SetupChoice>| {
                choice.layout = PegLayout::ALL[ev.index]
            },
        );
        if choice.family != FrettedKind::Bass {
            let capo_labels: Vec<_> = std::iter::once(loc.msg("guitar-capo-none").to_string())
                .chain((1..=12).map(|fret| {
                    loc.msg_args("guitar-capo-fret", &[("fret", fret.to_string())]).to_string()
                }))
                .collect();
            spawn_combobox(
                &mut commands,
                picker,
                host,
                &loc.msg("tuner-capo-label"),
                &capo_labels,
                &capo_labels[usize::from(choice.capo)],
                |ev: On<ComboboxSelect>, mut choice: ResMut<SetupChoice>| {
                    choice.capo = ev.index as u8
                },
            );
        }
    }
    let layout_only = last.is_some_and(|old| {
        !picker_new
            && old.family == selection.family
            && old.preset == selection.preset
            && old.capo == selection.capo
    });
    *last = Some(selection);
    let preset = presets(choice.family).nth(choice.preset).expect("valid tuning tab");
    if !layout_only {
        let (saved_range, selected) = (tuner.saved_range, tuner.selected);
        *tuner = Tuner { saved_range, ..Tuner::new(preset).with_capo(choice.capo) };
        if !picker_new {
            let index = selected.min(tuner.notes.len() - 1);
            tuner.select(index);
        }
        *range = tuner.range();
    }
    commands.entity(strings).despawn_related::<Children>();
    spawn_headstock(&mut commands, strings, &tuner.notes, choice.layout, choice.capo);
}

/// Strings run from the nut to the posts in physical string order.
/// Each bank chooses its spacing independently for odd string counts.
fn spawn_headstock(
    commands: &mut Commands,
    parent: Entity,
    notes: &[u8],
    layout: PegLayout,
    capo: u8,
) {
    let split = layout == PegLayout::Split;
    let wood = if split { Color::srgb(0.30, 0.17, 0.12) } else { Color::srgb(0.36, 0.25, 0.16) };
    let body_left = match layout {
        PegLayout::Split => 75.0,
        PegLayout::Inline => 91.0,
        PegLayout::Reverse => 59.0,
    };
    let center = body_left + 53.0;
    let neck = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center - 29.0),
                top: Val::Px(242.0),
                width: Val::Px(58.0),
                height: Val::Px(57.0),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.11, 0.09)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(neck);
    let body = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(body_left),
                top: Val::Px(6.0),
                width: Val::Px(106.0),
                height: Val::Px(252.0),
                border_radius: BorderRadius::all(Val::Px(if split { 16.0 } else { 30.0 })),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(wood),
            BorderColor::all(Color::srgb(0.60, 0.43, 0.29)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(body);
    let nut = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center - 29.0),
                top: Val::Px(246.0),
                width: Val::Px(58.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.82, 0.77, 0.64)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(nut);

    for (index, &note) in notes.iter().enumerate() {
        let (right, y) = layout.peg(index, notes.len());
        let post_x = if right { body_left + 93.0 } else { body_left + 13.0 };
        let nut_x = center - 22.0 + index as f32 * 44.0 / (notes.len() - 1) as f32;
        let neck_wire = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(nut_x - 0.5),
                    top: Val::Px(251.0),
                    width: Val::Px(1.0),
                    height: Val::Px(48.0),
                    ..default()
                },
                BackgroundColor(MUTED),
                StringWire(index),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(neck_wire);
        let delta = Vec2::new(post_x - nut_x, y - 250.0);
        let wire = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((nut_x + post_x - delta.length()) / 2.0),
                    top: Val::Px((250.0 + y) / 2.0 - 1.0),
                    width: Val::Px(delta.length()),
                    height: Val::Px(2.0),
                    ..default()
                },
                UiTransform::from_rotation(bevy::math::Rot2::radians(delta.y.atan2(delta.x))),
                BackgroundColor(MUTED),
                StringWire(index),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(wire);
        let post = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(post_x - 5.0),
                    top: Val::Px(y - 5.0),
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(MUTED),
                StringWire(index),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(post);
        let shaft = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(if right { post_x } else { body_left - 3.0 }),
                    top: Val::Px(y - 1.0),
                    width: Val::Px(16.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(MUTED),
                StringWire(index),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(shaft);
        let peg = spawn_button(
            commands,
            parent,
            &midi_to_note(note as i32),
            move |_: On<Activate>, mut tuner: ResMut<Tuner>| tuner.select(index),
        );
        commands.entity(peg).insert((
            StringLabel(index),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(if right { body_left + 106.0 } else { body_left - 58.0 }),
                top: Val::Px(y - 13.0),
                width: Val::Px(58.0),
                height: Val::Px(26.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ));
    }
    if capo > 0 {
        let clamp = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(center - 34.0),
                    top: Val::Px(268.0),
                    width: Val::Px(68.0),
                    height: Val::Px(9.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.48, 0.67, 0.66)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(clamp);
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
    let connected = mic.as_ref().is_some_and(|m| m.is_connected());
    let mut frequency = tuner.frequency;
    for event in events.read() {
        tuner.age = 0.0;
        let reading = match event.0.as_slice() {
            [pitch] if connected && pitch.frequency.is_finite() && pitch.frequency > 0.0 => {
                Some(pitch.frequency)
            }
            _ => None,
        };
        // Require distinct, agreeing capture frames before advising a turn.
        // An isolated transient or a render frame with no new audio cannot
        // count as another reading.
        tuner.candidate = reading.map(|hz| {
            let count = tuner
                .candidate
                .filter(|(previous, _)| (1200.0 * (hz / previous).log2()).abs() <= 22.0)
                .map_or(1, |(_, count)| count.saturating_add(1));
            (hz, count)
        });
        frequency = tuner.candidate.filter(|(_, count)| *count >= 3).map(|(hz, _)| hz);
    }
    if !connected || tuner.age > 0.15 {
        tuner.candidate = None;
        frequency = None;
    }
    tuner.listen(frequency, dt);
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
    theme: Res<LoadedTheme>,
    mut readout: Query<(&Readout, &mut Text, &mut TextColor)>,
    mut rows: Query<(&StringLabel, &Children, &mut BackgroundColor), Without<StringWire>>,
    mut wires: Query<(&StringWire, &mut BackgroundColor), Without<StringLabel>>,
    mut labels: Query<(&mut Text, &mut TextColor), Without<Readout>>,
) {
    let palette = theme.palette();
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
                    "tuner-tighten-peg"
                } else if c > 5.0 {
                    "tuner-loosen-peg"
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
            let tint =
                cents.filter(|c| c.abs() <= MAX_ADJUST_CENTS).map_or(palette.accent, pitch_color);
            if color.0 != tint {
                color.0 = tint;
            }
        }
    }
    for (wire, mut color) in &mut wires {
        let tint = if wire.0 == i { MINT } else { Color::srgba(0.68, 0.72, 0.71, 0.50) };
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (row, children, mut background) in &mut rows {
        let n = row.0;
        let Some(&note) = tuner.notes.get(n) else { continue };
        let surface = if n == i { palette.selected } else { palette.control };
        if background.0 != surface {
            background.0 = surface;
        }
        for child in children.iter() {
            if let Ok((mut text, mut color)) = labels.get_mut(child) {
                let note = midi_to_note(note as i32);
                let value = if tuner.tuned[n] { format!("{note} ✓") } else { note };
                if text.0 != value {
                    text.0 = value;
                }
                let tint = if tuner.tuned[n] || n == i { palette.accent } else { palette.text };
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
    fn capo_transposes_open_targets_and_split_layout_handles_odd_string_counts() {
        let tuner = Tuner::new(&guitar::TUNINGS[0]).with_capo(2);
        assert_eq!(tuner.notes, [42, 47, 52, 57, 61, 66]);
        assert_eq!(guitar::TUNINGS[0].strings[5], 40);
        for count in 4..=8 {
            for layout in PegLayout::ALL {
                let positions: Vec<_> = (0..count).map(|index| layout.peg(index, count)).collect();
                for (index, (side, y)) in positions.iter().copied().enumerate() {
                    assert!((35.0..=225.0).contains(&y));
                    for (other_side, other_y) in &positions[index + 1..] {
                        assert!(side != *other_side || (y - other_y).abs() >= 26.0);
                    }
                }
            }
        }
    }

    #[test]
    fn only_fretted_instruments_have_a_tuner() {
        let tunable: Vec<_> =
            InstrumentKind::ALL.into_iter().filter(|kind| tuner_family(*kind).is_some()).collect();
        assert_eq!(
            tunable,
            [InstrumentKind::Guitar, InstrumentKind::Bass, InstrumentKind::Cavaquinho]
        );
        for kind in tunable {
            assert_eq!(tuner_family(kind).unwrap().instrument_kind(), kind);
        }
    }

    #[test]
    fn the_page_opens_on_the_saved_tuning_and_capo() {
        let mine = MyInstruments {
            guitar_tuning: Some("drop-d".into()),
            guitar_capo: Some(3),
            bass_tuning: Some("bass-five".into()),
            bass_capo: Some(2),
            ..default()
        };
        let guitar = initial_choice(FrettedKind::Guitar, &mine);
        assert_eq!(presets(FrettedKind::Guitar).nth(guitar.preset).unwrap().id, "drop-d");
        assert_eq!((guitar.capo, guitar.layout), (3, PegLayout::Split));
        let bass = initial_choice(FrettedKind::Bass, &mine);
        assert_eq!(presets(FrettedKind::Bass).nth(bass.preset).unwrap().id, "bass-five");
        assert_eq!((bass.capo, bass.layout), (0, PegLayout::Inline));
        assert_eq!(initial_choice(FrettedKind::Cavaquinho, &mine).preset, 0);
    }

    #[test]
    fn presets_preserve_string_counts_and_detect_low_bass_b() {
        use harmonicon_audio::pitch_detect::{FftState, analyze_with_silence_threshold};
        assert_eq!(presets(FrettedKind::Guitar).next().unwrap().strings.len(), 6);
        assert_eq!(presets(FrettedKind::Guitar).last().unwrap().strings.len(), 8);
        assert_eq!(presets(FrettedKind::Bass).next().unwrap().strings.len(), 4);
        assert_eq!(presets(FrettedKind::Cavaquinho).next().unwrap().strings.len(), 4);
        let tuner = Tuner::new(presets(FrettedKind::Bass).nth(1).unwrap());
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
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(20));
            for frame in 0..3 {
                let pitch = &result.pitches[0];
                app.world_mut().write_message(PitchEvent(vec![
                    harmonicon_audio::pitch_detect::PitchInfo {
                        frequency: pitch.frequency,
                        midi: pitch.midi,
                        note: pitch.note.clone(),
                        octave: pitch.octave,
                    },
                ]));
                app.update();
                if frame == 0 && cents == -20.0 {
                    assert!(app.world().resource::<Tuner>().cents().is_none());
                }
            }
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
