// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use bevy::prelude::*;
use serde::Deserialize;

use crate::assets_management::SelectedTheme;

// ── JSON data structures ──────────────────────────────────────────────────────

#[derive(Deserialize, Clone, Debug)]
struct ThemeJson {
    name: String,
    #[serde(default)]
    default_menu_button: ButtonThemeJson,
    default_background: BackgroundThemeJson,
    #[serde(default)]
    menus: HashMap<String, MenuThemeJson>,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct ButtonThemeJson {
    #[serde(default)]
    background_image: Option<ImageRefJson>,
    #[serde(default)]
    icon: Option<ImageRefJson>,
    #[serde(default)]
    button_shaders: Option<ButtonShadersJson>,
    #[serde(default)]
    button_sounds: Option<ButtonSoundsJson>,
}

#[derive(Deserialize, Clone, Debug)]
struct ImageRefJson {
    image_file: String,
}

#[derive(Deserialize, Clone, Debug)]
struct ButtonShadersJson {
    #[allow(dead_code)]
    hover: String,
    #[allow(dead_code)]
    click: String,
    #[allow(dead_code)]
    idle: String,
}

#[derive(Deserialize, Clone, Debug)]
struct ButtonSoundsJson {
    hover: String,
    click: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct BackgroundThemeJson {
    #[serde(default)]
    image: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct MenuThemeJson {
    #[serde(default)]
    background_image: Option<String>,
}

// ── Runtime resource ──────────────────────────────────────────────────────────

/// Theme assets resolved at startup from `themes/<selected>/theme.json`.
/// Menus and buttons read this resource to apply backgrounds, icons, sounds,
/// and the smoke-shader flag.
#[derive(Resource, Default)]
pub struct LoadedTheme {
    /// Background per named menu key (e.g. "Main", "Play", "Credits").
    pub menu_backgrounds: HashMap<String, Handle<Image>>,
    /// Fallback background used when a menu has no specific entry.
    pub default_background: Option<Handle<Image>>,
    /// Default button icon shown alongside the label.
    pub btn_icon: Option<Handle<Image>>,
    /// Sound played when a button is hovered.
    pub btn_sound_hover: Option<Handle<AudioSource>>,
    /// Sound played when a button is clicked.
    pub btn_sound_click: Option<Handle<AudioSource>>,
    /// True when the theme ships the three smoke-shader WGSL files. When false
    /// buttons fall back to a plain `BackgroundColor`.
    pub has_shaders: bool,
}

impl LoadedTheme {
    /// Background for `menu_id`, falling back to the theme default.
    pub fn background_for(&self, menu_id: &str) -> Option<&Handle<Image>> {
        self.menu_backgrounds
            .get(menu_id)
            .or(self.default_background.as_ref())
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedTheme>()
            .add_systems(Startup, load_theme);
    }
}

fn load_theme(
    mut theme: ResMut<LoadedTheme>,
    selected: Res<SelectedTheme>,
    asset_server: Res<AssetServer>,
) {
    let json_path = format!("assets/themes/{}/theme.json", selected.0);

    let text = match std::fs::read_to_string(&json_path) {
        Ok(t) => t,
        Err(e) => {
            warn!("Could not read {json_path}: {e}");
            return;
        }
    };

    let data: ThemeJson = match serde_json::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to parse {json_path}: {e}");
            return;
        }
    };

    let prefix = format!("themes/{}/", selected.0);

    if !data.default_background.image.is_empty() {
        theme.default_background = Some(
            asset_server.load(format!("{prefix}{}", data.default_background.image)),
        );
    }

    for (menu_id, menu) in &data.menus {
        if let Some(bg) = &menu.background_image {
            theme.menu_backgrounds.insert(
                menu_id.clone(),
                asset_server.load(format!("{prefix}{bg}")),
            );
        }
    }

    if let Some(icon) = &data.default_menu_button.icon {
        theme.btn_icon = Some(asset_server.load(format!("{prefix}{}", icon.image_file)));
    }

    if let Some(sounds) = &data.default_menu_button.button_sounds {
        theme.btn_sound_hover =
            Some(asset_server.load(format!("{prefix}{}", sounds.hover)));
        theme.btn_sound_click =
            Some(asset_server.load(format!("{prefix}{}", sounds.click)));
    }

    theme.has_shaders = data.default_menu_button.button_shaders.is_some();

    info!("Loaded theme '{}' from {json_path}", data.name);
}
