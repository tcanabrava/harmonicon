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

    #[serde(default)]
    colors: ThemeColorsJson,
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
    #[serde(default)]
    buttons: Vec<ButtonEntryJson>,
}

#[derive(Deserialize, Clone, Debug)]
struct ButtonEntryJson {
    id: String,
    coords: CoordsJson,
}

#[derive(Deserialize, Clone, Debug)]
struct CoordsJson {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct ThemeColorsJson {
    pub song_editor: SongEditor,
}

#[derive(Deserialize, Clone, Debug)]
struct SongEditor {
    pub editor_bg: Color,
    pub hole_box: Color,
    pub lane_a: Color,
    pub lane_b: Color,
    pub grid_line: Color,
    pub bar_line: Color,
    pub accent: Color,
    pub label: Color,
    pub panel_bg: Color,
    pub btn_bg: Color,
    pub btn_active: Color,
    pub field_bg: Color,
    pub field_bg_focus: Color,
}

impl Default for SongEditor {
    fn default() -> Self {
        Self {
            editor_bg: Color::srgb(0.06, 0.06, 0.09),
            hole_box: Color::srgb(0.16, 0.16, 0.22),
            lane_a: Color::srgba(0.12, 0.12, 0.17, 1.0),
            lane_b: Color::srgba(0.10, 0.10, 0.14, 1.0),
            grid_line: Color::srgb(0.20, 0.20, 0.27),
            bar_line: Color::srgb(0.40, 0.40, 0.52),
            accent: Color::srgb(0.95, 0.80, 0.35),
            label: Color::srgb(0.75, 0.75, 0.82),
            panel_bg: Color::srgba(0.10, 0.10, 0.15, 1.0),
            btn_bg: Color::srgb(0.16, 0.16, 0.24),
            btn_active: Color::srgb(0.28, 0.42, 0.30),
            field_bg: Color::srgba(0.10, 0.10, 0.14, 1.0),
            field_bg_focus: Color::srgba(0.16, 0.16, 0.24, 1.0),
        }
    }
}

// ── Runtime resource ──────────────────────────────────────────────────────────

/// Pixel rect for a single button, read from the theme JSON.
#[derive(Clone, Debug)]
pub struct ButtonCoords {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Theme assets resolved at startup from `themes/<selected>/theme.json`.
/// Menus and buttons read this resource to apply backgrounds, icons, sounds,
/// shader flags, and per-button coordinates.
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
    /// True when the theme ships the three smoke-shader WGSL files.
    pub has_shaders: bool,
    /// Per-menu, per-button pixel coordinates: `menu_id → button_id → coords`.
    /// Button ids match the `MenuButton` variant names (e.g. "Play", "BackToMain").
    pub button_coords: HashMap<String, HashMap<String, ButtonCoords>>,
    pub colors: Option<ThemeColorsJson>,
}

impl LoadedTheme {
    /// Background for `menu_id`, falling back to the theme default.
    pub fn background_for(&self, menu_id: &str) -> Option<&Handle<Image>> {
        self.menu_backgrounds
            .get(menu_id)
            .or(self.default_background.as_ref())
    }

    /// Pixel rect for `button_id` on `menu_id`, or `None` if not specified.
    pub fn button_coords(&self, menu_id: &str, button_id: &str) -> Option<&ButtonCoords> {
        self.button_coords.get(menu_id)?.get(button_id)
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedTheme>()
            // PreUpdate runs before StateTransition, so when the player
            // navigates back to a menu after changing themes the OnEnter
            // setup system will see the already-refreshed LoadedTheme.
            // resource_changed fires on the frame after SelectedTheme is
            // written — including the first frame after Startup, when
            // apply_loaded_settings restores the saved theme name.
            .add_systems(
                PreUpdate,
                load_theme.run_if(|s: Res<SelectedTheme>| s.is_changed()),
            );
    }
}

fn load_theme(
    mut theme: ResMut<LoadedTheme>,
    selected: Res<SelectedTheme>,
    asset_server: Res<AssetServer>,
) {
    // Clear previous theme data so no stale entries from the old theme survive.
    *theme = LoadedTheme::default();

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
        if !menu.buttons.is_empty() {
            let coords: HashMap<String, ButtonCoords> = menu
                .buttons
                .iter()
                .map(|b| {
                    (
                        b.id.clone(),
                        ButtonCoords {
                            x: b.coords.x,
                            y: b.coords.y,
                            width: b.coords.width,
                            height: b.coords.height,
                        },
                    )
                })
                .collect();
            theme.button_coords.insert(menu_id.clone(), coords);
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── JSON parsing ──────────────────────────────────────────────────────

    #[test]
    fn theme_json_parses_button_coords() {
        let json = r#"{
            "name": "Test",
            "default_background": { "image": "" },
            "menus": {
                "Main": {
                    "buttons": [
                        { "id": "Play",    "coords": { "x": 510, "y": 260, "width": 260, "height": 50 } },
                        { "id": "Options", "coords": { "x": 510, "y": 326, "width": 260, "height": 50 } }
                    ]
                }
            }
        }"#;
        let data: ThemeJson = serde_json::from_str(json).unwrap();
        let main = data.menus.get("Main").unwrap();
        assert_eq!(main.buttons.len(), 2);
        let play = &main.buttons[0];
        assert_eq!(play.id, "Play");
        assert_eq!(play.coords.x, 510.0);
        assert_eq!(play.coords.y, 260.0);
        assert_eq!(play.coords.width, 260.0);
        assert_eq!(play.coords.height, 50.0);
    }

    #[test]
    fn theme_json_shaders_field_is_optional() {
        let with_shaders = r#"{
            "name": "T",
            "default_background": { "image": "" },
            "default_menu_button": {
                "button_shaders": { "idle": "i.wgsl", "hover": "h.wgsl", "click": "c.wgsl" }
            }
        }"#;
        let d: ThemeJson = serde_json::from_str(with_shaders).unwrap();
        assert!(d.default_menu_button.button_shaders.is_some());

        let without = r#"{ "name": "T", "default_background": { "image": "" } }"#;
        let d: ThemeJson = serde_json::from_str(without).unwrap();
        assert!(d.default_menu_button.button_shaders.is_none());
    }

    #[test]
    fn theme_json_missing_menus_defaults_to_empty_map() {
        let json = r#"{ "name": "T", "default_background": { "image": "" } }"#;
        let d: ThemeJson = serde_json::from_str(json).unwrap();
        assert!(d.menus.is_empty());
    }

    #[test]
    fn theme_json_menu_without_buttons_defaults_to_empty_vec() {
        let json = r#"{
            "name": "T",
            "default_background": { "image": "" },
            "menus": { "Credits": { "background_image": "bg.png" } }
        }"#;
        let d: ThemeJson = serde_json::from_str(json).unwrap();
        assert!(d.menus["Credits"].buttons.is_empty());
        assert_eq!(d.menus["Credits"].background_image.as_deref(), Some("bg.png"));
    }

    // ── LoadedTheme::button_coords ────────────────────────────────────────

    fn theme_with_main_buttons() -> LoadedTheme {
        let mut theme = LoadedTheme::default();
        let mut btns = HashMap::new();
        btns.insert("Play".into(), ButtonCoords { x: 510.0, y: 260.0, width: 260.0, height: 50.0 });
        btns.insert("Quit".into(), ButtonCoords { x: 510.0, y: 458.0, width: 260.0, height: 50.0 });
        theme.button_coords.insert("Main".into(), btns);
        theme
    }

    #[test]
    fn button_coords_returns_correct_values() {
        let theme = theme_with_main_buttons();
        let c = theme.button_coords("Main", "Play").unwrap();
        assert_eq!(c.x, 510.0);
        assert_eq!(c.y, 260.0);
        assert_eq!(c.width, 260.0);
        assert_eq!(c.height, 50.0);
    }

    #[test]
    fn button_coords_returns_none_for_unknown_menu() {
        let theme = theme_with_main_buttons();
        assert!(theme.button_coords("Play", "Play").is_none());
    }

    #[test]
    fn button_coords_returns_none_for_unknown_button() {
        let theme = theme_with_main_buttons();
        assert!(theme.button_coords("Main", "BackToMain").is_none());
    }

    // ── LoadedTheme::background_for ───────────────────────────────────────

    #[test]
    fn background_for_returns_none_when_no_backgrounds_configured() {
        let theme = LoadedTheme::default();
        assert!(theme.background_for("Main").is_none());
        assert!(theme.background_for("Unknown").is_none());
    }

    #[test]
    fn background_for_falls_back_to_default_for_unconfigured_menu() {
        let mut theme = LoadedTheme::default();
        theme.default_background = Some(Handle::default());
        // "Unknown" has no per-menu entry → falls back to default_background
        assert!(theme.background_for("Unknown").is_some());
    }

    #[test]
    fn background_for_returns_some_for_menu_with_explicit_entry() {
        let mut theme = LoadedTheme::default();
        theme.menu_backgrounds.insert("Main".into(), Handle::default());
        assert!(theme.background_for("Main").is_some());
    }

    #[test]
    fn background_for_uses_menu_entry_when_both_are_set() {
        let mut theme = LoadedTheme::default();
        // Insert handles with distinct UUIDs so we can tell them apart.
        let default_h: Handle<Image> = bevy::asset::uuid::Uuid::from_u128(1).into();
        let main_h: Handle<Image> = bevy::asset::uuid::Uuid::from_u128(2).into();
        theme.default_background = Some(default_h.clone());
        theme.menu_backgrounds.insert("Main".into(), main_h.clone());

        // Menu-specific handle is returned for "Main".
        assert_eq!(theme.background_for("Main"), Some(&main_h));
        // Any other menu falls back to the default.
        assert_eq!(theme.background_for("Play"), Some(&default_h));
    }

    // ── Reactive reload behavior ──────────────────────────────────────────

    /// Counter resource incremented by a stub that uses the same run condition
    /// as the real load_theme. Verifies the condition fires exactly when
    /// SelectedTheme changes and is quiet otherwise.
    #[derive(Resource, Default)]
    struct ReloadCount(u32);

    fn count_reloads(mut c: ResMut<ReloadCount>) {
        c.0 += 1;
    }

    #[test]
    fn load_condition_fires_once_on_insert_then_only_on_change() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(SelectedTheme("default".into()))
            .init_resource::<ReloadCount>()
            .add_systems(
                PreUpdate,
                count_reloads.run_if(|s: Res<SelectedTheme>| s.is_changed()),
            );

        // First update: SelectedTheme was just inserted → is_changed fires.
        app.update();
        assert_eq!(app.world().resource::<ReloadCount>().0, 1, "should fire on insert");

        // No change → silent.
        app.update();
        assert_eq!(app.world().resource::<ReloadCount>().0, 1, "should not fire without change");

        // Change the theme → fires again.
        app.world_mut().resource_mut::<SelectedTheme>().0 = "dark".into();
        app.update();
        assert_eq!(app.world().resource::<ReloadCount>().0, 2, "should fire when theme changes");

        // No further change → silent.
        app.update();
        assert_eq!(app.world().resource::<ReloadCount>().0, 2, "should not fire without change");
    }

    #[test]
    fn load_condition_fires_for_each_distinct_change() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(SelectedTheme("default".into()))
            .init_resource::<ReloadCount>()
            .add_systems(
                PreUpdate,
                count_reloads.run_if(|s: Res<SelectedTheme>| s.is_changed()),
            );

        app.update(); // insert fires once
        for theme in ["dark", "light", "neon", "default"] {
            app.world_mut().resource_mut::<SelectedTheme>().0 = theme.into();
            app.update();
        }
        assert_eq!(app.world().resource::<ReloadCount>().0, 5);
    }
}
