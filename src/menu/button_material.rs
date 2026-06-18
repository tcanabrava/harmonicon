// SPDX-License-Identifier: MIT

//! Three `UiMaterial` types for themed menu buttons — idle, hover, and click —
//! each driven by the matching smoke shader in `themes/default/shaders/`.
//! All themed buttons share one material instance per state so their animations
//! stay synchronised.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

use crate::theme::LoadedTheme;

// ── Material types ────────────────────────────────────────────────────────────

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ButtonIdleMaterial {
    #[uniform(0)]
    pub time: f32,
}

impl UiMaterial for ButtonIdleMaterial {
    fn fragment_shader() -> ShaderRef {
        "themes/default/shaders/button_idle.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ButtonHoverMaterial {
    #[uniform(0)]
    pub time: f32,
}

impl UiMaterial for ButtonHoverMaterial {
    fn fragment_shader() -> ShaderRef {
        "themes/default/shaders/button_hover.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ButtonClickMaterial {
    #[uniform(0)]
    pub time: f32,
}

impl UiMaterial for ButtonClickMaterial {
    fn fragment_shader() -> ShaderRef {
        "themes/default/shaders/button_click.wgsl".into()
    }
}

// ── Shared handles ────────────────────────────────────────────────────────────

/// One material instance per state, shared by all themed buttons.
#[derive(Resource)]
pub struct ButtonMaterials {
    pub idle: Handle<ButtonIdleMaterial>,
    pub hover: Handle<ButtonHoverMaterial>,
    pub click: Handle<ButtonClickMaterial>,
}

// ── Marker components ─────────────────────────────────────────────────────────

/// Present on buttons spawned via `spawn_button` when the theme has shaders.
/// The interaction system reads this to find buttons that need material swaps.
#[derive(Component)]
pub struct ThemedButton;

/// Marks the absolutely-positioned child that holds the active shader material.
#[derive(Component)]
pub struct ButtonShaderLayer;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct ButtonMaterialPlugin;

impl Plugin for ButtonMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<ButtonIdleMaterial>::default())
            .add_plugins(UiMaterialPlugin::<ButtonHoverMaterial>::default())
            .add_plugins(UiMaterialPlugin::<ButtonClickMaterial>::default())
            .add_systems(Startup, setup_button_materials)
            .add_systems(
                Update,
                (tick_button_materials, handle_themed_button_interaction),
            );
    }
}

fn setup_button_materials(
    mut commands: Commands,
    mut idle: ResMut<Assets<ButtonIdleMaterial>>,
    mut hover: ResMut<Assets<ButtonHoverMaterial>>,
    mut click: ResMut<Assets<ButtonClickMaterial>>,
) {
    commands.insert_resource(ButtonMaterials {
        idle: idle.add(ButtonIdleMaterial { time: 0.0 }),
        hover: hover.add(ButtonHoverMaterial { time: 0.0 }),
        click: click.add(ButtonClickMaterial { time: 0.0 }),
    });
}

/// Advance the `time` uniform on all three shared materials every frame so the
/// smoke animations play even on buttons that are currently off-screen.
fn tick_button_materials(
    time: Res<Time>,
    mats: Option<Res<ButtonMaterials>>,
    mut idle: ResMut<Assets<ButtonIdleMaterial>>,
    mut hover: ResMut<Assets<ButtonHoverMaterial>>,
    mut click: ResMut<Assets<ButtonClickMaterial>>,
) {
    let Some(mats) = mats else { return };
    let t = time.elapsed_secs();
    if let Some(mut m) = idle.get_mut(&mats.idle) {
        m.time = t;
    }
    if let Some(mut m) = hover.get_mut(&mats.hover) {
        m.time = t;
    }
    if let Some(mut m) = click.get_mut(&mats.click) {
        m.time = t;
    }
}

/// Swap the shader material on a button's `ButtonShaderLayer` child when the
/// interaction state changes, and play the matching sound.
fn handle_themed_button_interaction(
    mut commands: Commands,
    mats: Option<Res<ButtonMaterials>>,
    theme: Res<LoadedTheme>,
    buttons: Query<
        (Entity, &Interaction, &Children),
        (Changed<Interaction>, With<ThemedButton>),
    >,
    shader_layers: Query<(), With<ButtonShaderLayer>>,
) {
    let Some(mats) = mats else { return };

    for (_entity, interaction, children) in &buttons {
        // children.iter() yields Entity; find passes &Entity to the closure
        let Some(layer) = children.iter().find(|&c| shader_layers.contains(c))
        else {
            continue;
        };

        commands
            .entity(layer)
            .remove::<MaterialNode<ButtonIdleMaterial>>()
            .remove::<MaterialNode<ButtonHoverMaterial>>()
            .remove::<MaterialNode<ButtonClickMaterial>>();

        match interaction {
            Interaction::None => {
                commands
                    .entity(layer)
                    .insert(MaterialNode(mats.idle.clone()));
            }
            Interaction::Hovered => {
                commands
                    .entity(layer)
                    .insert(MaterialNode(mats.hover.clone()));
                if let Some(ref snd) = theme.btn_sound_hover {
                    commands.spawn((
                        AudioPlayer::<AudioSource>(snd.clone()),
                        PlaybackSettings::DESPAWN,
                    ));
                }
            }
            Interaction::Pressed => {
                commands
                    .entity(layer)
                    .insert(MaterialNode(mats.click.clone()));
                if let Some(ref snd) = theme.btn_sound_click {
                    commands.spawn((
                        AudioPlayer::<AudioSource>(snd.clone()),
                        PlaybackSettings::DESPAWN,
                    ));
                }
            }
        }
    }
}
