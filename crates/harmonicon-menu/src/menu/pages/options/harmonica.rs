// SPDX-License-Identifier: MIT

//! The harmonica-model picker: one button per model, each with a live 3D
//! preview rendered to an off-screen texture on its own render layer.

use super::*;

/// A labelled row of harmonica-model choice buttons, each showing a rendered
/// preview above its name. Each button is a `bsn!` scene carrying its own
/// dedicated "select this model" click callback plus hover;
pub(super) fn spawn_harmonica_row(
    commands: &mut Commands,
    parent: Entity,
    loc: &Localization,
    previews: &[(Handle<Image>, &str)],
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
        r.spawn_empty().apply_scene(bsn! {
            Node { width: {Val::Px(OPTIONS_LABEL_WIDTH)} }
            Text({String::from(loc.msg("options-harmonica"))})
            TextFont { font_size: {FontSize::Px(20.0)} }
            TextColor({Color::WHITE})
            Tooltip({String::from(loc.msg("options-harmonica-tooltip"))})
        });
        for (image, name) in previews {
            let is_selected = *name == selected;
            r.spawn_empty().apply_scene(harmonica_button_scene(
                image.clone(),
                (*name).to_owned(),
                is_selected,
            ));
        }
    });

    commands.entity(parent).add_child(row);
}

/// One harmonica choice button: preview image + name, its dedicated "select
/// this model" click callback (capturing the name), and hover — all inline
/// `on(...)`.
pub(super) fn harmonica_button_scene(
    image: Handle<Image>,
    name: String,
    is_selected: bool,
) -> impl Scene {
    let color = if is_selected { button::CHOICE_SELECTED } else { button::color_default() };
    let label = name.clone();
    let pick = name.clone();
    bsn! {
        WidgetButton
        TabIndex(0)
        Node {
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            padding: {UiRect::axes(Val::Px(8.0), Val::Px(6.0))},
            row_gap: {Val::Px(4.0)},
        }
        BackgroundColor({color})
        HarmonicaButton({name})
        on(move |_: On<Activate>, mut selected: ResMut<SelectedHarmonicaModel>| {
            if selected.0 != pick {
                selected.0.clone_from(&pick);
            }
        })
        on(harm_over)
        on(harm_out)
        Children [
            Node { width: {Val::Px(54.0)}, height: {Val::Px(54.0)} }
            ImageNode { image: {image}, color: {Color::WHITE} }
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
            --
            Text({label})
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::WHITE})
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

// ── 3D model previews (render-to-texture) ──────────────────────────────────────

/// Renders a harmonica model's glTF scene to an off-screen texture for the
/// Options UI. Like the note preview, but the model is a multi-mesh scene with
/// its own materials, so it's spawned via `WorldAssetRoot` and shown untinted;
/// `scene::propagate_scene_layers` pushes the render layer onto the scene's children.
pub(super) fn spawn_harmonica_preview(
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
        Camera { clear_color: ClearColorConfig::Custom(Color::NONE), order: -1, ..default() },
        RenderTarget::from(handle.clone()),
        Transform::from_xyz(0.0, 1.6, 4.2).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
        MenuRoot,
    ));

    // The model scene, posed at a slight angle. Scene children get the render
    // layer from `scene::propagate_scene_layers` (they don't inherit it on spawn).
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
        crate::menu::scene::SceneLayer(layers.clone()),
        MenuRoot,
    ));

    spawn_preview_light(commands, layers);
    handle
}

/// Allocates a transparent render-target image for a 3D preview.
pub(super) fn preview_target(images: &mut Assets<Image>) -> Handle<Image> {
    let size = Extent3d { width: 128, height: 128, depth_or_array_layers: 1 };
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
pub(super) fn spawn_preview_light(commands: &mut Commands, layers: RenderLayers) {
    commands.spawn((
        DirectionalLight { illuminance: 6000.0, ..default() },
        Transform::from_xyz(3.0, 5.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers,
        MenuRoot,
    ));
}

/// Hover highlight for harmonica buttons, never overriding the green selection.
pub(super) fn harm_over(
    ev: On<PointerOver>,
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != selected.0
    {
        let wanted = BackgroundColor(button::CHOICE_HOVER);
        if *bg != wanted {
            *bg = wanted;
        }
    }
}

pub(super) fn harm_out(
    ev: On<PointerOut>,
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != selected.0
    {
        let wanted = BackgroundColor(button::color_default());
        if *bg != wanted {
            *bg = wanted;
        }
    }
}

/// Recolour the harmonica buttons when the selection changes (green = chosen).
pub(super) fn harmonica_button_visuals(
    selected: Res<SelectedHarmonicaModel>,
    mut buttons: Query<(&HarmonicaButton, &mut BackgroundColor)>,
) {
    if !selected.is_changed() {
        return;
    }
    for (button, mut bg) in &mut buttons {
        let wanted =
            if button.0 == selected.0 { button::CHOICE_SELECTED } else { button::color_default() };
        if bg.0 != wanted {
            bg.0 = wanted;
        }
    }
}
