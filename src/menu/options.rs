// SPDX-License-Identifier: MIT

//! The Options page: audio volume sliders plus 2D-note / 3D-note / harmonica
//! pickers with live previews. The 3D previews render each model to an off-screen
//! texture (one render layer per preview) shown as a UI image. Owns its page
//! lifecycle via [`OptionsPlugin`]; the menu shell only routes to it.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui_widgets::{
    Slider, SliderRange, SliderStep, SliderValue, TrackClick, ValueChange, slider_self_update,
};
use bevy_fluent::Localization;

const TRACK_BG: Color = Color::srgb(0.14, 0.14, 0.22);

use crate::assets_management::{AvailableHarmonicas, SelectedHarmonicaModel, ShowNoteNumbers};
use crate::audio_system::audio_input::{self, MicStatus};
use crate::localization::LocalizationExt;
use crate::settings::AudioSettings;

use crate::theme::LoadedTheme;

use super::{AppState, MenuPage, MenuRoot, cleanup_menu, spawn_button, spawn_menu_root};

use crate::dialogs::algo_picker::{algo_labels, on_algo_selected, spawn_algo_explanation};
use crate::dialogs::button;
use crate::dialogs::button_material::ButtonMaterials;
use crate::dialogs::combobox;

/// Owns the Options page: builds it on entry, tears it down on exit, and runs
/// the slider/preview interaction systems while it's open.
pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuPage::Options), setup_options_menu)
            .add_systems(OnExit(MenuPage::Options), cleanup_menu)
            // Keep each slider's own SliderValue in sync as it's dragged or
            // stepped, so keyboard adjustment works from the current value.
            .add_observer(slider_self_update)
            // Sliders and harmonica buttons carry their own change/click/hover
            // behaviour as inline on(...) observers; these systems only mirror
            // settings/selection onto the visuals.
            .add_systems(
                Update,
                (
                    update_sliders,
                    update_latency_slider,
                    harmonica_button_visuals,
                    propagate_preview_layers,
                    update_mic_banner,
                    sync_mic_combobox,
                    update_note_numbers_label,
                )
                    .run_if(in_state(MenuPage::Options)),
            );
    }
}

// ── Components ──────────────────────────────────────────────────────────────

/// Which audio level a slider controls.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
enum VolumeSlider {
    #[default]
    Music,
    Metronome,
}

/// The growing fill of a slider track; its width mirrors the bound level.
#[derive(Component, Default, Clone)]
struct SliderFill(VolumeSlider);

/// The "NN%" readout beside a slider.
#[derive(Component)]
struct SliderValueLabel(VolumeSlider);

/// A harmonica-model choice button; carries the model name.
#[derive(Component, Default, Clone)]
struct HarmonicaButton(String);

/// The "no microphone" warning banner, shown only while [`MicStatus::Failed`].
/// See TODO.md: "No microphone = silent failure."
#[derive(Component)]
struct MicBanner;

/// The failure-reason text inside [`MicBanner`].
#[derive(Component)]
struct MicBannerText;

/// Marks a preview scene root (a `WorldAssetRoot`); the propagation system forces
/// this `RenderLayers` onto all its descendants, since glTF scene children don't
/// inherit it and would otherwise be invisible to the preview camera.
#[derive(Component)]
struct PreviewSceneLayer(RenderLayers);

/// Marks the drag track of the input-latency slider.
#[derive(Component, Default, Clone)]
struct LatencySlider;

/// The fill bar inside the latency slider track.
#[derive(Component, Default, Clone)]
struct LatencySliderFill;

/// The "Xms" readout beside the latency slider.
#[derive(Component)]
struct LatencySliderLabel;

/// The "Note labels: ..." readout beside the note-numbers toggle.
#[derive(Component)]
struct NoteNumbersLabel;

/// Current level for a given slider kind.
fn audio_level(settings: &AudioSettings, kind: VolumeSlider) -> f32 {
    match kind {
        VolumeSlider::Music => settings.music_volume,
        VolumeSlider::Metronome => settings.metronome_volume,
    }
}

// ── Page setup ────────────────────────────────────────────────────────────────

fn setup_options_menu(
    mut commands: Commands,
    loc: Res<Localization>,

    settings: Res<AudioSettings>,
    mic_status: Res<MicStatus>,
    harmonicas: Res<AvailableHarmonicas>,
    selected_harmonica: Res<SelectedHarmonicaModel>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    show_numbers: Res<ShowNoteNumbers>,
) {
    let root = spawn_menu_root(&mut commands, "Options", Some("Audio"), &theme, "Options");
    spawn_mic_banner(&mut commands, root, &mic_status);
    spawn_volume_slider(
        &mut commands,
        root,
        "Music",
        VolumeSlider::Music,
        settings.music_volume,
        set_music_volume,
    );
    spawn_volume_slider(
        &mut commands,
        root,
        "Metronome",
        VolumeSlider::Metronome,
        settings.metronome_volume,
        set_metronome_volume,
    );
    spawn_latency_slider(&mut commands, root, settings.input_latency_ms, &loc);
    spawn_mic_combobox(
        &mut commands,
        root,
        &audio_input::input_device_names(),
        connected_device_name(&mic_status),
    );

    // Harmonica previews: the model's glTF scene rendered to a texture (its own
    // materials, no tint). Layers are assigned after the 3D-note layers so the
    // preview cameras never capture each other's models.
    let harmonica_base_layer = 1;
    let previews_harmonica: Vec<(Handle<Image>, String)> = harmonicas
        .0
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let handle = spawn_harmonica_preview(
                &mut commands,
                &mut images,
                &asset_server,
                m,
                harmonica_base_layer + i,
            );
            (handle, m.clone())
        })
        .collect();

    spawn_harmonica_row(
        &mut commands,
        root,
        &previews_harmonica,
        &selected_harmonica.0,
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        "Pitch detect",
        &algo_labels(),
        settings.pitch_algorithm.label(),
        on_algo_selected,
    );
    spawn_algo_explanation(&mut commands, root, 560.0, settings.pitch_algorithm);

    spawn_note_numbers_toggle(&mut commands, root, show_numbers.0);

    spawn_button(
        &mut commands,
        root,
        "Theme",
        Some("Theme"),
        &theme,
        &btn_mats,
        "Options",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Theme),
    );
    spawn_button(
        &mut commands,
        root,
        "Calibrate input lag",
        Some("Calibrate"),
        &theme,
        &btn_mats,
        "Options",
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::Calibration)
        },
    );
    spawn_button(
        &mut commands,
        root,
        "\u{2190} Back",
        Some("BackToMain"),
        &theme,
        &btn_mats,
        "Options",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}

/// Flips whether falling notes show their hole number instead of the
/// blow/draw arrow (`gameplay_2d`/`gameplay_3d`'s note spawners read this).
fn toggle_note_numbers(_: On<Pointer<Click>>, mut show: ResMut<ShowNoteNumbers>) {
    show.0 = !show.0;
}

/// Pure so both readouts are unit-testable without spinning up an `App`.
fn note_numbers_label_text(show: bool) -> &'static str {
    if show {
        "Note labels: numbers"
    } else {
        "Note labels: arrows"
    }
}

/// A row with a pill button that flips [`ShowNoteNumbers`] plus a label
/// reflecting the current choice — same shape as the pause menu's
/// `WaitForNoteMode` toggle.
fn spawn_note_numbers_toggle(commands: &mut Commands, parent: Entity, show_numbers: bool) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(row).with_children(|r| {
        r.spawn_empty()
            .apply_scene(button::small("Note labels", toggle_note_numbers));
        r.spawn((
            Text::new(note_numbers_label_text(show_numbers)),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::WHITE),
            NoteNumbersLabel,
        ));
    });
    commands.entity(parent).add_child(row);
}

/// Keeps the toggle's label in step with [`ShowNoteNumbers`].
fn update_note_numbers_label(
    show: Res<ShowNoteNumbers>,
    mut labels: Query<&mut Text, With<NoteNumbersLabel>>,
) {
    if !show.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(note_numbers_label_text(show.0));
    }
}

/// A labelled row of harmonica-model choice buttons, each showing a rendered
/// preview above its name. Each button is a `bsn!` scene carrying its own
/// dedicated "select this model" click callback plus hover;
fn spawn_harmonica_row(
    commands: &mut Commands,
    parent: Entity,

    previews: &[(Handle<Image>, String)],
    selected: &str,
) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(110.0),
                ..default()
            },
            Text::new("Harmonica"),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        for (image, name) in previews {
            let is_selected = name == selected;
            r.spawn_empty().apply_scene(harmonica_button_scene(
                image.clone(),
                name.clone(),
                is_selected,
            ));
        }
    });

    commands.entity(parent).add_child(row);
}

/// One harmonica choice button: preview image + name, its dedicated "select
/// this model" click callback (capturing the name), and hover — all inline
/// `on(...)`.
fn harmonica_button_scene(image: Handle<Image>, name: String, is_selected: bool) -> impl Scene {
    let color = if is_selected {
        button::CHOICE_SELECTED
    } else {
        button::color_default()
    };
    let label = name.clone();
    let pick = name.clone();
    bsn! {
        Button
        Node {
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            padding: {UiRect::axes(Val::Px(8.0), Val::Px(6.0))},
            row_gap: {Val::Px(4.0)},
        }
        BackgroundColor({color})
        HarmonicaButton({name})
        on(move |_: On<Pointer<Click>>, mut selected: ResMut<SelectedHarmonicaModel>| {
            selected.0 = pick.clone();
        })
        on(harm_over)
        on(harm_out)
        Children [
            (
                Node { width: {Val::Px(54.0)}, height: {Val::Px(54.0)} }
                ImageNode { image: {image}, color: {Color::WHITE} }
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            ),
            (
                Text({label})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

// ── 3D model previews (render-to-texture) ──────────────────────────────────────

/// Renders a harmonica model's glTF scene to an off-screen texture for the
/// Options UI. Like the note preview, but the model is a multi-mesh scene with
/// its own materials, so it's spawned via `WorldAssetRoot` and shown untinted;
/// `propagate_preview_layers` pushes the render layer onto the scene's children.
fn spawn_harmonica_preview(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    asset_server: &AssetServer,
    model: &str,
    layer: usize,
) -> Handle<Image> {
    let handle = preview_target(images);
    let layers = RenderLayers::layer(layer);

    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            order: -1,
            ..default()
        },
        RenderTarget::from(handle.clone()),
        Transform::from_xyz(0.0, 1.6, 4.2).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
        MenuRoot,
    ));

    // The model scene, posed at a slight angle. Scene children get the render
    // layer from `propagate_preview_layers` (they don't inherit it on spawn).
    commands.spawn((
        WorldAssetRoot(asset_server.load(format!("harmonicas/3d/{model}/harmonica.glb#Scene0"))),
        Transform::from_scale(Vec3::splat(0.1)).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -0.5,
            0.35,
            0.0,
        )),
        Visibility::default(),
        layers.clone(),
        PreviewSceneLayer(layers.clone()),
        MenuRoot,
    ));

    spawn_preview_light(commands, layers);
    handle
}

/// Allocates a transparent render-target image for a 3D preview.
fn preview_target(images: &mut Assets<Image>) -> Handle<Image> {
    let size = Extent3d {
        width: 128,
        height: 128,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    images.add(image)
}

/// A directional light on `layers` so a preview model is shaded, not flat.
fn spawn_preview_light(commands: &mut Commands, layers: RenderLayers) {
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            ..default()
        },
        Transform::from_xyz(3.0, 5.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers,
        MenuRoot,
    ));
}

/// Forces each preview scene's render layer onto all of its descendants. glTF
/// scene children spawn a frame or two after the root and don't inherit
/// `RenderLayers`, so without this the preview camera would never see them.
fn propagate_preview_layers(
    mut commands: Commands,
    roots: Query<(Entity, &PreviewSceneLayer)>,
    children: Query<&Children>,
    already_layered: Query<(), With<RenderLayers>>,
) {
    for (root, layer) in &roots {
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children.get(entity) {
                for child in kids {
                    if already_layered.get(*child).is_err() {
                        commands.entity(*child).insert(layer.0.clone());
                    }
                    stack.push(*child);
                }
            }
        }
    }
}

/// Hover highlight for harmonica buttons, never overriding the green selection.
fn harm_over(
    ev: On<Pointer<Over>>,
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != selected.0
    {
        *bg = BackgroundColor(button::CHOICE_HOVER);
    }
}

fn harm_out(
    ev: On<Pointer<Out>>,
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != selected.0
    {
        *bg = BackgroundColor(button::color_default());
    }
}

/// Recolour the harmonica buttons when the selection changes (green = chosen).
fn harmonica_button_visuals(
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if !selected.is_changed() {
        return;
    }
    for (button, mut bg) in &mut buttons {
        bg.0 = if button.0 == selected.0 {
            button::CHOICE_SELECTED
        } else {
            button::color_default()
        };
    }
}

// ── Microphone picker / status banner ───────────────────────────────────────

/// The name of the device actually connected right now, or `None` while
/// [`MicStatus::Failed`]. Used (rather than the raw `AudioSettings::input_device`
/// preference) so the picker highlights reality — if a saved device went
/// missing and capture fell back to the default, that's what lights up.
fn connected_device_name(status: &MicStatus) -> Option<&str> {
    match status {
        MicStatus::Connected { device_name } => Some(device_name.as_str()),
        MicStatus::Failed { .. } => None,
    }
}

/// A dismiss-free warning banner, visible only while the microphone failed to
/// open, with a Retry button that re-runs `audio_input::start_capture`.
fn spawn_mic_banner(commands: &mut Commands, parent: Entity, status: &MicStatus) {
    let visible = matches!(status, MicStatus::Failed { .. });
    let text = mic_banner_text(status);

    let banner = commands
        .spawn((
            Node {
                width: Val::Px(560.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(10.0)),
                display: if visible {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            BackgroundColor(Color::srgba(0.45, 0.12, 0.12, 0.85)),
            MicBanner,
        ))
        .id();

    commands.entity(banner).with_children(|b| {
        b.spawn((
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.85, 0.85)),
            MicBannerText,
        ));
        b.spawn_empty().apply_scene(mic_retry_button_scene());
    });

    commands.entity(parent).add_child(banner);
}

fn mic_banner_text(status: &MicStatus) -> String {
    match status {
        MicStatus::Failed { reason } => format!("No microphone: {reason}"),
        MicStatus::Connected { .. } => String::new(),
    }
}

fn mic_retry_button_scene() -> impl Scene {
    bsn! {
        Button
        Node { padding: {UiRect::axes(Val::Px(12.0), Val::Px(6.0))} }
        BackgroundColor({button::color_default()})
        on(|_: On<Pointer<Click>>, mut commands: Commands| {
            commands.queue(audio_input::start_capture);
        })
        Children [
            (
                Text({"Retry".to_string()})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

/// Show/hide the banner and refresh its reason text when `MicStatus` changes
/// (e.g. after a Retry click or a device-picker selection).
fn update_mic_banner(
    status: Res<MicStatus>,
    mut banners: Query<&mut Node, With<MicBanner>>,
    mut texts: Query<&mut Text, With<MicBannerText>>,
) {
    if !status.is_changed() {
        return;
    }
    let visible = matches!(*status, MicStatus::Failed { .. });
    for mut node in &mut banners {
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    let text = mic_banner_text(&status);
    for mut t in &mut texts {
        **t = text.clone();
    }
}

/// Marks the Options page's microphone combobox root, so [`sync_mic_combobox`]
/// can find it to push `MicStatus` changes into its display — e.g. after
/// Retry reconnects to a different actual device than was last picked, or a
/// saved device disappears and capture silently falls back to the default.
#[derive(Component)]
struct MicCombobox;

/// Wires the shared [`combobox`] widget to the microphone device list:
/// picking an option persists it to `AudioSettings` and reconnects capture
/// immediately.
fn spawn_mic_combobox(
    commands: &mut Commands,
    parent: Entity,
    devices: &[String],
    connected: Option<&str>,
) {
    let root = combobox::spawn_combobox(
        commands,
        parent,
        "Microphone",
        devices,
        connected.unwrap_or("None"),
        on_mic_selected,
    );
    commands.entity(root).insert(MicCombobox);
}

fn on_mic_selected(
    ev: On<combobox::ComboboxSelect>,
    mut settings: ResMut<AudioSettings>,
    mut commands: Commands,
) {
    settings.input_device = ev.value.clone();
    commands.queue(audio_input::start_capture);
}

fn sync_mic_combobox(
    status: Res<MicStatus>,
    combo: Query<Entity, With<MicCombobox>>,
    mut values: Query<&mut combobox::ComboboxValue>,
) {
    if !status.is_changed() {
        return;
    }
    let Ok(root) = combo.single() else { return };
    let Ok(mut value) = values.get_mut(root) else {
        return;
    };
    value.0 = connected_device_name(&status).unwrap_or("None").to_string();
}

// ── Dedicated slider callbacks ────────────────────────────────────────────────

fn set_music_volume(ev: On<ValueChange<f32>>, mut settings: ResMut<AudioSettings>) {
    settings.music_volume = ev.value;
}

fn set_metronome_volume(ev: On<ValueChange<f32>>, mut settings: ResMut<AudioSettings>) {
    settings.metronome_volume = ev.value;
}

fn set_input_latency(ev: On<ValueChange<f32>>, mut settings: ResMut<AudioSettings>) {
    settings.input_latency_ms = ev.value.round() as i32;
}

// ── Volume sliders ──────────────────────────────────────────────────────────

/// One labelled volume row: `<name>  [====    ]  NN%`. The track is authored as
/// a `bsn!` `Slider` whose value change is handled by the given dedicated `on`
/// callback; the label/readout stay imperative (they carry the custom font,
/// which `bsn!` can't set in 0.19).
fn spawn_volume_slider<M: 'static>(
    commands: &mut Commands,
    parent: Entity,

    label: &str,
    kind: VolumeSlider,
    value: f32,
    on_change: impl IntoObserverSystem<ValueChange<f32>, (), M> + Clone + Sync + 'static,
) {
    let row = commands
        .spawn(Node {
            width: Val::Px(420.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .id();

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(110.0),
                ..default()
            },
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });

    // SliderRange/SliderStep have no Default, so they can't be bsn! patches —
    // insert them after the scene is spawned.
    let track = commands
        .spawn_scene(volume_slider_scene(kind, value, on_change))
        .insert((SliderRange::new(0.0, 1.0), SliderStep(0.01)))
        .id();
    commands.entity(row).add_child(track);

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(50.0),
                ..default()
            },
            Text::new(format!("{:.0}%", value * 100.0)),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.7)),
            SliderValueLabel(kind),
        ));
    });

    commands.entity(parent).add_child(row);
}

/// The volume slider track itself: a `bsn!` `Slider` with its fill, wired to the
/// given value-change callback inline via `on(...)`.
fn volume_slider_scene<M: 'static>(
    kind: VolumeSlider,
    value: f32,
    on_change: impl IntoObserverSystem<ValueChange<f32>, (), M> + Clone + Sync + 'static,
) -> impl Scene {
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        SliderValue({value})
        Node { width: {Val::Px(220.0)}, height: {Val::Px(14.0)} }
        BackgroundColor({TRACK_BG})
        on(on_change)
        Children [
            (
                Node { width: {Val::Percent(value * 100.0)}, height: {Val::Percent(100.0)} }
                BackgroundColor({Color::srgb(0.35, 0.75, 1.0)})
                SliderFill({kind})
                // Don't let the fill steal the slider's pointer events.
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

// ── Input-latency slider ──────────────────────────────────────────────────────

const LATENCY_MAX_MS: i32 = 200;

/// One labelled slider row for the mic input-latency offset.
/// The track maps 0–200 ms linearly; the label shows "Xms".
fn spawn_latency_slider(
    commands: &mut Commands,
    parent: Entity,
    value_ms: i32,
    loc: &Localization,
) {
    let frac = (value_ms as f32 / LATENCY_MAX_MS as f32).clamp(0.0, 1.0);

    let row = commands
        .spawn(Node {
            width: Val::Px(420.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .id();

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(110.0),
                ..default()
            },
            Text::new(String::from(loc.msg("options-input-lag"))),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });

    let track = commands
        .spawn_scene(latency_slider_scene(value_ms as f32, frac))
        .insert((
            SliderRange::new(0.0, LATENCY_MAX_MS as f32),
            SliderStep(1.0),
        ))
        .id();
    commands.entity(row).add_child(track);

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(50.0),
                ..default()
            },
            Text::new(format!("{}ms", value_ms)),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.7)),
            LatencySliderLabel,
        ));
    });

    commands.entity(parent).add_child(row);
}

/// The latency slider track: a `bsn!` `Slider` + fill, wired to `set_input_latency`.
fn latency_slider_scene(value: f32, frac: f32) -> impl Scene {
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        SliderValue({value})
        Node { width: {Val::Px(220.0)}, height: {Val::Px(14.0)} }
        BackgroundColor({TRACK_BG})
        LatencySlider
        on(set_input_latency)
        Children [
            (
                Node { width: {Val::Percent(frac * 100.0)}, height: {Val::Percent(100.0)} }
                BackgroundColor({Color::srgb(0.80, 0.55, 0.25)})
                LatencySliderFill
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

/// Mirror `input_latency_ms` onto the fill bar and label.
fn update_latency_slider(
    settings: Res<AudioSettings>,
    mut fills: Query<&mut Node, With<LatencySliderFill>>,
    mut labels: Query<&mut Text, With<LatencySliderLabel>>,
) {
    if !settings.is_changed() {
        return;
    }
    let frac = (settings.input_latency_ms as f32 / LATENCY_MAX_MS as f32).clamp(0.0, 1.0);
    for mut node in &mut fills {
        node.width = Val::Percent(frac * 100.0);
    }
    for mut text in &mut labels {
        text.0 = format!("{}ms", settings.input_latency_ms);
    }
}

/// Mirror the current levels onto the slider fills and percentage readouts.
fn update_sliders(
    settings: Res<AudioSettings>,
    mut fills: Query<(&mut Node, &SliderFill)>,
    mut labels: Query<(&mut Text, &SliderValueLabel)>,
) {
    for (mut node, fill) in &mut fills {
        node.width = Val::Percent(audio_level(&settings, fill.0) * 100.0);
    }
    for (mut text, label) in &mut labels {
        text.0 = format!("{:.0}%", audio_level(&settings, label.0) * 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_numbers_label_reflects_the_current_choice() {
        assert_eq!(note_numbers_label_text(false), "Note labels: arrows");
        assert_eq!(note_numbers_label_text(true), "Note labels: numbers");
    }
}
