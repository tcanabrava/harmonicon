// SPDX-License-Identifier: MIT

//! Shared menu scene helpers: the full-screen root container every page
//! spawns into (with its background image), the themed/plain button
//! widget, and the `MenuRoot` marker `cleanup_menu` despawns on page exit.

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer, Press};
use bevy::prelude::*;

use crate::dialogs::button;
use crate::dialogs::button_material::{
    ButtonMaterials, ButtonShaderLayer, ButtonVisual, ThemedButton, set_button_visual,
};
use crate::theme::LoadedTheme;

/// Marks every entity that belongs to a menu screen so `cleanup_menu` can
/// remove it in one sweep when the page changes. Shared with the `options` page.
#[derive(Component, Default, Clone)]
pub(super) struct MenuRoot;

fn menu_bg() -> Color {
    Color::srgb(0.05, 0.05, 0.08)
}

/// Spawn a full-screen centred column with a title and optional subtitle.
/// Returns the entity so the caller can add button children afterwards.
/// `menu_id` is matched against the theme's `menus` keys (e.g. "Main", "Play")
/// to look up the per-menu background image.
/// The menu root container as a `bsn!` [`Scene`]: a full-screen centred column.
fn menu_root_scene() -> impl Scene {
    bsn! {
        Node {
            width: {Val::Percent(100.0)},
            height: {Val::Percent(100.0)},
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
            row_gap: {Val::Px(16.0)},
        }
        BackgroundColor({menu_bg()})
        MenuRoot
    }
}

fn heading_scene(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text({text})
        TextFont { font_size: {FontSize::Px(size)} }
        TextColor({color})
    }
}

pub(crate) fn spawn_menu_root(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    theme: &LoadedTheme,
    menu_id: &str,
) -> Entity {
    // Root container + title (+ optional subtitle) as one composed scene. The
    // subtitle is conditional, so the two `Children [...]` shapes are spawned in
    // separate branches (each `bsn!` is a distinct concrete `Scene` type).
    let title = title.to_string();
    let root = if let Some(sub) = subtitle {
        commands
            .spawn_scene(bsn! {
                menu_root_scene()
                Children [
                    heading_scene(title, 52.0, Color::WHITE),
                    heading_scene(sub.to_string(), 20.0, Color::srgb(0.6, 0.6, 0.7)),
                ]
            })
            .id()
    } else {
        commands
            .spawn_scene(bsn! {
                menu_root_scene()
                Children [ heading_scene(title, 52.0, Color::WHITE) ]
            })
            .id()
    };

    // Background image behind all other content. Inserted at index 0 so it stays
    // the lowest layer regardless of when this command is applied.
    if let Some(bg) = theme.background_for(menu_id) {
        let bg_layer = commands
            .spawn((
                ImageNode::new(bg.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
            ))
            .id();
        commands.entity(root).insert_children(0, &[bg_layer]);
    }
    root
}

/// Spawn a single button as a child of `parent`, in the normal flex flow —
/// themes control appearance (background/effects) only, never layout, so
/// there's no per-button positioning to resolve here.
///
/// When the theme has shaders the button also gets a smoke background layer,
/// an optional icon, and audio on hover/click.
///
/// `on_click` is the button's own dedicated click behaviour, wired inline as the
/// `on(...)` callback (plain buttons) or via `observe` (themed buttons).
pub(crate) fn spawn_button<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    theme: &LoadedTheme,
    btn_mats: &ButtonMaterials,
    on_click: impl IntoObserverSystem<Pointer<Click>, (), M> + Clone + Sync + 'static,
) {
    let node = Node {
        min_width: Val::Px(260.0),
        padding: UiRect::axes(Val::Px(32.0), Val::Px(14.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };

    // Children are `Pickable::IGNORE` so the pointer always hits the button
    // itself (not the text/icon), keeping the hover/press observers below
    // robust — otherwise picking would target a child and the button would
    // flicker between hovered/unhovered.
    //
    // Themed buttons stay imperative (runtime shader-material handle, optional
    // icon, z-ordered smoke layer); plain buttons are authored with bsn!. Either
    // way the click rides along as the caller's dedicated `on_click`.
    if theme.has_shaders {
        println!("Creating a shader button with label: {label}");

        let e = commands.spawn((Button, node, ThemedButton)).id();

        // Smoke shader layer — absolute, behind content. Keep its entity so the
        // pointer observers can swap its material.
        let layer = commands
            .spawn((
                MaterialNode(btn_mats.idle.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
                ButtonShaderLayer,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(e).add_child(layer);

        commands.entity(e).with_children(|b| {
            // Icon from theme (optional)
            if let Some(ref icon) = theme.btn_icon {
                b.spawn((
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ImageNode {
                        image: icon.clone(),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }

            b.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });

        // Themed hover/press visuals via observers, not a
        // `Changed<Interaction>` system.
        commands.entity(e).observe(
            move |_: On<Pointer<Over>>, mats: Res<ButtonMaterials>, mut commands: Commands| {
                set_button_visual(&mut commands, layer, ButtonVisual::Hover, &mats);
            },
        );
        commands.entity(e).observe(
            move |_: On<Pointer<Out>>, mats: Res<ButtonMaterials>, mut commands: Commands| {
                set_button_visual(&mut commands, layer, ButtonVisual::Idle, &mats);
            },
        );
        commands.entity(e).observe(
            move |_: On<Pointer<Press>>, mats: Res<ButtonMaterials>, mut commands: Commands| {
                set_button_visual(&mut commands, layer, ButtonVisual::Click, &mats);
            },
        );

        // The caller's dedicated click behaviour.
        commands.entity(e).observe(on_click);
        commands.entity(parent).add_child(e);
    } else {
        println!("Creating a default button with label: {label}");
        // Plain button: authored declaratively; click + hover ride along as
        // inline on(...)
        let e = commands
            .spawn_scene(button::default(label, on_click))
            .insert(node)
            .id();
        commands.entity(parent).add_child(e);
    }
}

pub(crate) fn cleanup_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
