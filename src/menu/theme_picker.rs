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

use bevy::prelude::*;

use crate::assets_management::{AvailableThemes, GlobalFonts, SelectedTheme};
use crate::theme::LoadedTheme;

use super::{
    MenuButton, MenuPage, MenuRoot, btn_default, button_material::ButtonMaterials, cleanup_menu,
    spawn_button,
};

pub struct ThemePickerPlugin;

impl Plugin for ThemePickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuPage::Theme), setup)
            .add_systems(OnExit(MenuPage::Theme), cleanup_menu)
            .add_systems(
                Update,
                (handle_buttons, update_button_visuals, update_preview)
                    .run_if(in_state(MenuPage::Theme)),
            );
    }
}

// ── Components ─────────────────────────────────────────────────────────────────

/// Carries the theme name for each list button.
#[derive(Component)]
struct ThemeButton(String);

/// Marks the preview `ImageNode` so `update_preview` can swap its handle.
#[derive(Component)]
struct ThemePreviewImage;

// ── Setup ──────────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    fonts: Res<GlobalFonts>,
    themes: Res<AvailableThemes>,
    selected: Res<SelectedTheme>,
    asset_server: Res<AssetServer>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let font = fonts.gameplay.clone();

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
            TextFont { font_size: FontSize::Px(48.0), font: font.clone(), ..default() },
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

    for name in &themes.0 {
        let is_selected = *name == selected.0;
        let btn = commands
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                    justify_content: JustifyContent::FlexStart,
                    ..default()
                },
                BackgroundColor(if is_selected {
                    Color::srgb(0.25, 0.45, 0.30)
                } else {
                    btn_default()
                }),
                ThemeButton(name.clone()),
            ))
            .id();
        commands.entity(btn).with_children(|b| {
            b.spawn((
                Text::new(name.clone()),
                TextFont { font_size: FontSize::Px(19.0), font: font.clone(), ..default() },
                TextColor(Color::WHITE),
            ));
        });
        commands.entity(left).add_child(btn);
    }

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

    let preview_handle: Handle<Image> =
        asset_server.load(format!("themes/{}/preview.png", selected.0));
    let preview = commands
        .spawn((
            Node {
                width: Val::Px(512.0),
                height: Val::Px(288.0),
                ..default()
            },
            ImageNode { image: preview_handle, ..default() },
            ThemePreviewImage,
        ))
        .id();
    commands.entity(right).add_child(preview);

    // ── Back button ────────────────────────────────────────────────────────────
    spawn_button(
        &mut commands,
        root,
        &fonts.symbols,
        "\u{2190} Back to Options",
        MenuButton::BackToOptions,
        &theme,
        &btn_mats,
        "Theme",
    );
}

// ── Update systems ─────────────────────────────────────────────────────────────

fn handle_buttons(
    buttons: Query<(&Interaction, &ThemeButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedTheme>,
) {
    for (interaction, btn) in &buttons {
        if *interaction == Interaction::Pressed {
            selected.0 = btn.0.clone();
        }
    }
}

fn update_button_visuals(
    selected: Res<SelectedTheme>,
    mut buttons: Query<(&Interaction, &ThemeButton, &mut BackgroundColor)>,
) {
    if !selected.is_changed() {
        return;
    }
    for (interaction, btn, mut bg) in &mut buttons {
        bg.0 = if btn.0 == selected.0 {
            Color::srgb(0.25, 0.45, 0.30)
        } else {
            match interaction {
                Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
                _ => btn_default(),
            }
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
    let handle: Handle<Image> =
        asset_server.load(format!("themes/{}/preview.png", selected.0));
    for mut img in &mut previews {
        img.image = handle.clone();
    }
}
