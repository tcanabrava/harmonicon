// SPDX-License-Identifier: MIT

//! The Options page: audio volume sliders plus 2D-note / 3D-note / harmonica
//! pickers with live previews. The 3D previews render each model to an off-screen
//! texture (one render layer per preview) shown as a UI image. Owns its page
//! lifecycle via [`OptionsPlugin`]; the menu shell only routes to it.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui_widgets::{
    Slider, SliderRange, SliderStep, SliderValue, TrackClick, ValueChange, slider_self_update,
};

const TRACK_BG: Color = Color::srgb(0.14, 0.14, 0.22);
const CHOICE_SELECTED: Color = Color::srgb(0.25, 0.45, 0.30);
const CHOICE_HOVER: Color = Color::srgb(0.20, 0.20, 0.32);

use crate::assets_management::{
    AvailableHarmonicas, GlobalFonts,
    SelectedHarmonicaModel
};
use crate::settings::AudioSettings;

use crate::theme::LoadedTheme;

use super::{
    AppState, MenuPage, MenuRoot, btn_default, button_material::ButtonMaterials, cleanup_menu,
    spawn_button, spawn_menu_root,
};

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

/// A 2D note-theme choice button; carries the theme name.
#[derive(Component)]
struct NoteTheme2dButton(String);

/// A 3D note-theme choice button; carries the theme name.
#[derive(Component)]
struct NoteTheme3dButton(String);

/// A harmonica-model choice button; carries the model name.
#[derive(Component, Default, Clone)]
struct HarmonicaButton(String);

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
    font: Res<GlobalFonts>,
    settings: Res<AudioSettings>,
    harmonicas: Res<AvailableHarmonicas>,
    selected_harmonica: Res<SelectedHarmonicaModel>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let root = spawn_menu_root(&mut commands, "Options", Some("Audio"), &theme, "Options");
    spawn_volume_slider(
        &mut commands,
        root,
        &font.gameplay,
        "Music",
        VolumeSlider::Music,
        settings.music_volume,
        set_music_volume,
    );
    spawn_volume_slider(
        &mut commands,
        root,
        &font.gameplay,
        "Metronome",
        VolumeSlider::Metronome,
        settings.metronome_volume,
        set_metronome_volume,
    );
    spawn_latency_slider(
        &mut commands,
        root,
        &font.gameplay,
        settings.input_latency_ms,
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
        &font.gameplay,
        &previews_harmonica,
        &selected_harmonica.0,
    );

    spawn_button(&mut commands, root, &font.gameplay, "Theme", Some("Theme"), &theme, &btn_mats, "Options",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Theme));
    spawn_button(&mut commands, root, &font.gameplay, "Calibrate input lag", Some("Calibrate"), &theme, &btn_mats, "Options",
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| state.set(AppState::Calibration));
    spawn_button(&mut commands, root, &font.symbols, "\u{2190} Back", Some("BackToMain"), &theme, &btn_mats, "Options",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main));
}

/// A labelled row of harmonica-model choice buttons, each showing a rendered
/// preview above its name. Each button is a `bsn!` scene carrying its own
/// dedicated "select this model" click callback plus hover; the row label keeps
/// the custom font (imperative), which `bsn!` can't set in 0.19.
fn spawn_harmonica_row(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
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
                font: font.clone(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        for (image, name) in previews {
            let is_selected = name == selected;
            r.spawn_empty()
                .apply_scene(harmonica_button_scene(image.clone(), name.clone(), is_selected));
        }
    });

    commands.entity(parent).add_child(row);
}

/// One harmonica choice button: preview image + name, its dedicated "select
/// this model" click callback (capturing the name), and hover — all inline
/// `on(...)`. (Default font for the name label: `bsn!` can't set it in 0.19.)
fn harmonica_button_scene(image: Handle<Image>, name: String, is_selected: bool) -> impl Scene {
    let color = if is_selected { CHOICE_SELECTED } else { btn_default() };
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
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity) {
        if btn.0 != selected.0 {
            *bg = BackgroundColor(CHOICE_HOVER);
        }
    }
}

fn harm_out(
    ev: On<Pointer<Out>>,
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity) {
        if btn.0 != selected.0 {
            *bg = BackgroundColor(btn_default());
        }
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
            CHOICE_SELECTED
        } else {
            btn_default()
        };
    }
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
    font: &FontSource,
    label: &str,
    kind: VolumeSlider,
    value: f32,
    on_change: impl IntoObserverSystem<ValueChange<f32>, (), M> + Clone + Send + Sync + 'static,
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
                font: font.clone(),
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
                font: font.clone(),
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
    on_change: impl IntoObserverSystem<ValueChange<f32>, (), M> + Clone + Send + Sync + 'static,
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
fn spawn_latency_slider(commands: &mut Commands, parent: Entity, font: &FontSource, value_ms: i32) {
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
            Text::new("Input lag"),
            TextFont {
                font_size: FontSize::Px(20.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });

    let track = commands
        .spawn_scene(latency_slider_scene(value_ms as f32, frac))
        .insert((SliderRange::new(0.0, LATENCY_MAX_MS as f32), SliderStep(1.0)))
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
                font: font.clone(),
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
