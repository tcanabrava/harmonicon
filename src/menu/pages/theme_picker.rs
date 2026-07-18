// SPDX-License-Identifier: MIT

//! Theme picker screen.
//!
//! Layout:
//!
//!   ┌─ THEME ─────────────────────────────────────────────────┐
//!   │ ┌─ theme list ──┐  ┌─ preview ───────────────────────┐ │
//!   │ │ default  ●    │  │                                  │ │
//!   │ │ dark          │  │   [themes/<name>/preview.png]    │ │
//!   │ │ light         │  │                                  │ │
//!   │ └───────────────┘  └──────────────────────────────────┘ │
//!   │                      ← Back to Options                   │
//!   └─────────────────────────────────────────────────────────┘

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::assets_management::{AvailableThemes, SelectedTheme};
use crate::dialogs::button;
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::theme::{LoadedTheme, theme_source_prefix};

use crate::menu::routing::MenuPage;
use crate::menu::scene::{MenuRoot, cleanup_menu, spawn_button};

const THEME_SELECTED: Color = Color::srgb(0.25, 0.45, 0.30);
const THEME_HOVER: Color = Color::srgb(0.20, 0.20, 0.32);

pub struct ThemePickerPlugin;

impl Plugin for ThemePickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuPage::Theme), setup)
            .add_systems(OnExit(MenuPage::Theme), cleanup_menu)
            // Theme buttons carry their own select/hover behaviour as inline
            // on(...) observers; these systems only react to the selection.
            .add_systems(
                Update,
                (update_button_visuals, update_preview).run_if(in_state(MenuPage::Theme)),
            );
    }
}

// ── Components ─────────────────────────────────────────────────────────────────

/// Carries the theme name for each list button.
#[derive(Component, Default, Clone)]
struct ThemeButton(String);

/// Marks the preview `ImageNode` so `update_preview` can swap its handle.
#[derive(Component)]
struct ThemePreviewImage;

// ── Setup ──────────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    themes: Res<AvailableThemes>,
    selected: Res<SelectedTheme>,
    asset_server: Res<AssetServer>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    // ── Root: full-screen column ───────────────────────────────────────────────
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.0)),
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            MenuRoot,
        ))
        .id();

    // ── Title ──────────────────────────────────────────────────────────────────
    let title = commands
        .spawn((
            Text::new("Theme"),
            TextFont {
                font_size: FontSize::Px(48.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(root).add_child(title);

    // ── Content row (list | preview) ───────────────────────────────────────────
    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            width: Val::Percent(100.0),
            column_gap: Val::Px(24.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(content);

    // Left panel — scrollable theme list
    let left = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Px(240.0),
            row_gap: Val::Px(8.0),
            overflow: Overflow::clip_y(),
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        })
        .id();
    commands.entity(content).add_child(left);

    commands.entity(left).with_children(|l| {
        for name in &themes.0 {
            let is_selected = *name == selected.0;
            l.spawn_empty()
                .apply_scene(theme_button_scene(name.clone(), is_selected));
        }
    });

    // Right panel — preview image
    let right = commands
        .spawn(Node {
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    commands.entity(content).add_child(right);

    let preview_handle: Handle<Image> = asset_server.load(format!(
        "{}themes/{}/preview.png",
        theme_source_prefix(&selected.0),
        selected.0
    ));
    let preview = commands
        .spawn((
            Node {
                width: Val::Px(512.0),
                height: Val::Px(288.0),
                ..default()
            },
            ImageNode {
                image: preview_handle,
                ..default()
            },
            ThemePreviewImage,
        ))
        .id();
    commands.entity(right).add_child(preview);

    // ── Back button ────────────────────────────────────────────────────────────
    spawn_button(
        &mut commands,
        root,
        &loc.msg("theme-back-to-options"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );
}

// ── Update systems ─────────────────────────────────────────────────────────────

/// One theme-list button: its label, its dedicated "select this theme" click
/// callback (capturing the name), and hover highlight — all inline `on(...)`.
fn theme_button_scene(name: String, is_selected: bool) -> impl Scene {
    let color = if is_selected {
        THEME_SELECTED
    } else {
        button::color_default()
    };
    let label = name.clone();
    let pick = name.clone();
    bsn! {
        Button
        Node {
            width: {Val::Percent(100.0)},
            padding: {UiRect::axes(Val::Px(16.0), Val::Px(10.0))},
            justify_content: {JustifyContent::FlexStart},
        }
        BackgroundColor({color})
        ThemeButton({name})
        on(move |_: On<Pointer<Click>>, mut selected: ResMut<SelectedTheme>| {
            selected.0 = pick.clone();
        })
        on(theme_over)
        on(theme_out)
        Children [
            (
                Text({label})
                TextFont { font_size: {FontSize::Px(19.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

/// Hover highlight, but never override the green of the currently-selected theme.
fn theme_over(
    ev: On<Pointer<Over>>,
    selected: Res<SelectedTheme>,
    mut buttons: Query<(&ThemeButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != selected.0
    {
        *bg = BackgroundColor(THEME_HOVER);
    }
}

fn theme_out(
    ev: On<Pointer<Out>>,
    selected: Res<SelectedTheme>,
    mut buttons: Query<(&ThemeButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != selected.0
    {
        *bg = BackgroundColor(button::color_default());
    }
}

/// Recolour the list whenever the selection changes (green for the chosen one).
fn update_button_visuals(
    selected: Res<SelectedTheme>,
    mut buttons: Query<(&ThemeButton, &mut BackgroundColor)>,
) {
    if !selected.is_changed() {
        return;
    }
    for (btn, mut bg) in &mut buttons {
        bg.0 = if btn.0 == selected.0 {
            THEME_SELECTED
        } else {
            button::color_default()
        };
    }
}

/// When the selected theme changes, reload the preview image.
fn update_preview(
    selected: Res<SelectedTheme>,
    asset_server: Res<AssetServer>,
    mut previews: Query<&mut ImageNode, With<ThemePreviewImage>>,
) {
    if !selected.is_changed() {
        return;
    }
    let handle: Handle<Image> = asset_server.load(format!(
        "{}themes/{}/preview.png",
        theme_source_prefix(&selected.0),
        selected.0
    ));
    for mut img in &mut previews {
        img.image = handle.clone();
    }
}
