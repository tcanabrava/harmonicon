// SPDX-License-Identifier: MIT

//! The Options page: audio volume sliders plus 2D-note / 3D-note / harmonica
//! pickers with live previews. The 3D previews render each model to an off-screen
//! texture (one render layer per preview) shown as a UI image. Owns its page
//! lifecycle via [`OptionsPlugin`]; the menu shell only routes to it.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui::RelativeCursorPosition;

use crate::assets_management::{
    AvailableHarmonicas, AvailableNoteThemes2d, AvailableNoteThemes3d, GlobalFonts,
    SelectedHarmonicaModel, SelectedNoteTheme2d, SelectedNoteTheme3d,
};
use crate::settings::AudioSettings;

use super::{
    MenuButton, MenuPage, MenuRoot, btn_default, cleanup_menu, spawn_button, spawn_menu_root,
};

/// Owns the Options page: builds it on entry, tears it down on exit, and runs
/// the slider/preview interaction systems while it's open.
pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuPage::Options), setup_options_menu)
            .add_systems(OnExit(MenuPage::Options), cleanup_menu)
            .add_systems(
                Update,
                (
                    drag_sliders,
                    update_sliders,
                    drag_latency_slider,
                    update_latency_slider,
                    handle_theme_buttons_2d,
                    theme_button_visuals_2d,
                    handle_theme_buttons_3d,
                    theme_button_visuals_3d,
                    handle_harmonica_buttons,
                    harmonica_button_visuals,
                    propagate_preview_layers,
                )
                    .run_if(in_state(MenuPage::Options)),
            );
    }
}

// ── Components ──────────────────────────────────────────────────────────────

/// Which audio level a slider controls.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum VolumeSlider {
    Music,
    Metronome,
}

/// The growing fill of a slider track; its width mirrors the bound level.
#[derive(Component)]
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
#[derive(Component)]
struct HarmonicaButton(String);

/// Marks a preview scene root (a `WorldAssetRoot`); the propagation system forces
/// this `RenderLayers` onto all its descendants, since glTF scene children don't
/// inherit it and would otherwise be invisible to the preview camera.
#[derive(Component)]
struct PreviewSceneLayer(RenderLayers);

/// Marks the drag track of the input-latency slider.
#[derive(Component)]
struct LatencySlider;

/// The fill bar inside the latency slider track.
#[derive(Component)]
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
    themes_2d: Res<AvailableNoteThemes2d>,
    themes_3d: Res<AvailableNoteThemes3d>,
    harmonicas: Res<AvailableHarmonicas>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let root = spawn_menu_root(&mut commands, "Options", Some("Audio"));
    spawn_volume_slider(
        &mut commands,
        root,
        &font.gameplay,
        "Music",
        VolumeSlider::Music,
        settings.music_volume,
    );
    spawn_volume_slider(
        &mut commands,
        root,
        &font.gameplay,
        "Metronome",
        VolumeSlider::Metronome,
        settings.metronome_volume,
    );
    spawn_latency_slider(
        &mut commands,
        root,
        &font.gameplay,
        settings.input_latency_ms,
    );

    // The blow-note tint, so the previews read like an in-game note.
    let blow = Color::srgb(0.25, 0.55, 0.95);

    // 2D previews: the theme PNG, tinted directly in the UI image.
    let previews_2d: Vec<(Handle<Image>, String)> = themes_2d
        .0
        .iter()
        .map(|t| (asset_server.load(format!("notes/2d/{t}.png")), t.clone()))
        .collect();

    // 3D previews: the theme's glTF cube rendered (blow-tinted) to a texture, one
    // per theme on its own render layer, then shown as a UI image.
    let previews_3d: Vec<(Handle<Image>, String)> = themes_3d
        .0
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let handle = spawn_theme_3d_preview(
                &mut commands,
                &mut images,
                &mut materials,
                &asset_server,
                t,
                i + 1,
                blow,
            );
            (handle, t.clone())
        })
        .collect();

    // Harmonica previews: the model's glTF scene rendered to a texture (its own
    // materials, no tint). Layers are assigned after the 3D-note layers so the
    // preview cameras never capture each other's models.
    let harmonica_base_layer = previews_3d.len() + 1;
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

    // 2D previews are tinted here; 3D previews already baked the tint into the
    // rendered texture, so they're shown untinted (white).
    spawn_theme_row(
        &mut commands,
        root,
        &font.gameplay,
        "2D notes",
        &previews_2d,
        blow,
        |n| NoteTheme2dButton(n.to_string()),
    );
    spawn_theme_row(
        &mut commands,
        root,
        &font.gameplay,
        "3D notes",
        &previews_3d,
        Color::WHITE,
        |n| NoteTheme3dButton(n.to_string()),
    );
    spawn_theme_row(
        &mut commands,
        root,
        &font.gameplay,
        "Harmonica",
        &previews_harmonica,
        Color::WHITE,
        |n| HarmonicaButton(n.to_string()),
    );

    spawn_button(
        &mut commands,
        root,
        &font.gameplay,
        "Theme",
        MenuButton::Theme,
    );
    spawn_button(
        &mut commands,
        root,
        &font.gameplay,
        "Calibrate input lag",
        MenuButton::Calibrate,
    );
    spawn_button(
        &mut commands,
        root,
        &font.symbols,
        "\u{2190} Back",
        MenuButton::BackToMain,
    );
}

/// A labelled row of theme buttons, each showing a preview image above its name.
/// `tint` colours the preview image (used to blow-tint the 2D PNGs; the 3D
/// previews bake the tint in, so they pass white).
fn spawn_theme_row<M: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
    label: &str,
    previews: &[(Handle<Image>, String)],
    tint: Color,
    make: impl Fn(&str) -> M,
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
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        for (image, name) in previews {
            r.spawn((
                Button,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(btn_default()),
                make(name),
            ))
            .with_children(|b| {
                b.spawn((
                    Node {
                        width: Val::Px(54.0),
                        height: Val::Px(54.0),
                        ..default()
                    },
                    ImageNode {
                        image: image.clone(),
                        color: tint,
                        ..default()
                    },
                ));
                b.spawn((
                    Text::new(name.clone()),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        font: font.clone(),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        }
    });

    commands.entity(parent).add_child(row);
}

// ── 3D model previews (render-to-texture) ──────────────────────────────────────

/// Renders a theme's 3D glTF cube (blow-tinted) to an off-screen texture so it
/// can be shown in the Options UI. Each preview gets its own render layer + a
/// camera, cube and light on it; all are `MenuRoot`-tagged so they're cleaned up
/// when the page changes. Returns the texture handle for the UI image.
fn spawn_theme_3d_preview(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    theme: &str,
    layer: usize,
    tint: Color,
) -> Handle<Image> {
    let handle = preview_target(images);
    let layers = RenderLayers::layer(layer);

    // Camera that renders only this layer into the texture, transparent around it.
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            order: -1,
            ..default()
        },
        RenderTarget::from(handle.clone()),
        Transform::from_xyz(2.0, 1.5, 2.8).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
        MenuRoot,
    ));

    // The cube head, blow-tinted, posed at a 3/4 angle so its form reads.
    let mesh: Handle<Mesh> = asset_server.load(format!("notes/3d/{theme}.glb#Mesh0/Primitive0"));
    let linear = tint.to_linear();
    let material = materials.add(StandardMaterial {
        base_color: tint,
        emissive: LinearRgba::new(linear.red * 0.2, linear.green * 0.2, linear.blue * 0.2, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, 0.5, 0.0)),
        layers.clone(),
        MenuRoot,
    ));

    spawn_preview_light(commands, layers);
    handle
}

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

// ── Theme / harmonica selection ─────────────────────────────────────────────

/// Apply a clicked 2D-theme button to the selected 2D-theme resource.
fn handle_theme_buttons_2d(
    buttons: Query<(&Interaction, &NoteTheme2dButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedNoteTheme2d>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            selected.0 = button.0.clone();
        }
    }
}

/// Highlight the selected 2D-theme button; the rest follow normal hover styling.
fn theme_button_visuals_2d(
    selected: Res<SelectedNoteTheme2d>,
    mut buttons: Query<(&Interaction, &NoteTheme2dButton, &mut BackgroundColor)>,
) {
    for (interaction, button, mut bg) in &mut buttons {
        *bg = BackgroundColor(choice_button_color(button.0 == selected.0, interaction));
    }
}

/// Apply a clicked 3D-theme button to the selected 3D-theme resource.
fn handle_theme_buttons_3d(
    buttons: Query<(&Interaction, &NoteTheme3dButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedNoteTheme3d>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            selected.0 = button.0.clone();
        }
    }
}

/// Highlight the selected 3D-theme button; the rest follow normal hover styling.
fn theme_button_visuals_3d(
    selected: Res<SelectedNoteTheme3d>,
    mut buttons: Query<(&Interaction, &NoteTheme3dButton, &mut BackgroundColor)>,
) {
    for (interaction, button, mut bg) in &mut buttons {
        *bg = BackgroundColor(choice_button_color(button.0 == selected.0, interaction));
    }
}

/// Apply a clicked harmonica button to the selected-model resource.
fn handle_harmonica_buttons(
    buttons: Query<(&Interaction, &HarmonicaButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedHarmonicaModel>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            selected.0 = button.0.clone();
        }
    }
}

/// Highlight the selected harmonica button; the rest follow normal hover styling.
fn harmonica_button_visuals(
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&Interaction, &HarmonicaButton, &mut BackgroundColor)>,
) {
    for (interaction, button, mut bg) in &mut buttons {
        *bg = BackgroundColor(choice_button_color(button.0 == selected.0, interaction));
    }
}

/// Background colour for a selector button: a green tint when it's the current
/// choice, otherwise the usual hover styling.
fn choice_button_color(selected: bool, interaction: &Interaction) -> Color {
    if selected {
        Color::srgb(0.25, 0.45, 0.30)
    } else {
        match interaction {
            Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
            Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
            Interaction::None => btn_default(),
        }
    }
}

// ── Volume sliders ──────────────────────────────────────────────────────────

/// One labelled slider row: `<name>  [====       ]  NN%`. The track is a `Button`
/// so it reports `Interaction`, and carries `RelativeCursorPosition` so the drag
/// system can read the cursor's position along it.
fn spawn_volume_slider(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
    label: &str,
    kind: VolumeSlider,
    value: f32,
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

        r.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            RelativeCursorPosition::default(),
            kind,
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(value * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                SliderFill(kind),
            ));
        });

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

        r.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            RelativeCursorPosition::default(),
            LatencySlider,
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(frac * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.80, 0.55, 0.25)),
                LatencySliderFill,
            ));
        });

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

/// While the latency track is pressed, set `input_latency_ms` from cursor position.
fn drag_latency_slider(
    mut settings: ResMut<AudioSettings>,
    sliders: Query<(&Interaction, &RelativeCursorPosition), With<LatencySlider>>,
) {
    for (interaction, rel) in &sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(norm) = rel.normalized else {
            continue;
        };
        let ms = ((norm.x + 0.5).clamp(0.0, 1.0) * LATENCY_MAX_MS as f32).round() as i32;
        if settings.input_latency_ms != ms {
            settings.input_latency_ms = ms;
        }
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

/// While a slider track is pressed, set its level from the cursor's position
/// along the track. Only writes when the value actually changes so resting on a
/// pressed slider doesn't re-trigger downstream change detection every frame.
fn drag_sliders(
    mut settings: ResMut<AudioSettings>,
    sliders: Query<(&Interaction, &RelativeCursorPosition, &VolumeSlider)>,
) {
    for (interaction, rel, kind) in &sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(norm) = rel.normalized else {
            continue;
        };
        let frac = (norm.x + 0.5).clamp(0.0, 1.0);
        if (audio_level(&settings, *kind) - frac).abs() <= f32::EPSILON {
            continue;
        }
        match kind {
            VolumeSlider::Music => settings.music_volume = frac,
            VolumeSlider::Metronome => settings.metronome_volume = frac,
        }
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
