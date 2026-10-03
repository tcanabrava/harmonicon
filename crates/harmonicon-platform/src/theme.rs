// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use bevy::{
    asset::{AssetLoader, LoadState, io::Reader},
    prelude::*,
};
use serde::Deserialize;
use thiserror::Error;

use crate::assets_management::SelectedTheme;

// ── JSON data structures ──────────────────────────────────────────────────────

/// Deliberately an [`Asset`] itself (not just a plain struct parsed with
/// `std::fs::read_to_string`) so it loads through [`AssetServer`] like every
/// other song/theme sub-asset — `std::fs` can't read anything under wasm
/// (there's no local filesystem inside a browser), where `AssetServer`
/// transparently fetches over HTTP instead. See [`ThemeJsonLoader`].
#[derive(Asset, TypePath, Deserialize, Clone, Debug)]
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
    // The schema's `button_def.background_image` isn't rendered yet, so it
    // isn't read; serde skips it.
    #[serde(default)]
    icon: Option<ImageRefJson>,
    /// Only whether a theme declares shaders is read (`has_shaders`); the
    /// shader paths aren't wired up to rendering yet.
    #[serde(default)]
    button_shaders: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    button_sounds: Option<ButtonSoundsJson>,
}

#[derive(Deserialize, Clone, Debug)]
struct ImageRefJson {
    image_file: String,
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

/// Per-theme color overrides for optional subsystems. Add further fields here
/// as other screens grow theme support.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ThemeColorsJson {
    #[serde(default)]
    pub song_editor: SongEditorColors,
    #[serde(default)]
    pub twelve_bar: TwelveBarColors,
    #[serde(default)]
    pub notes: NoteColors,
    #[serde(default)]
    pub circle_of_fifths: CircleOfFifthsColors,
}

/// Chord-function colors for a 12-bar blues progression (I / IV / V), read
/// from a theme's `theme.json` under `"colors": { "twelve_bar": { ... } }`.
/// Shared by the Jam Session bar grid (`gameplay::twelve_bar_blues_overlay`)
/// and the song editor's grid background, so both reflect the same theme.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct TwelveBarColors {
    /// I (tonic) — the "home" chord most bars sit on.
    #[serde(deserialize_with = "hex_color")]
    pub tonic: Color,
    /// IV (subdominant).
    #[serde(deserialize_with = "hex_color")]
    pub subdominant: Color,
    /// V (dominant).
    #[serde(deserialize_with = "hex_color")]
    pub dominant: Color,
}

impl Default for TwelveBarColors {
    fn default() -> Self {
        Self {
            tonic: Color::srgba(0.10, 0.16, 0.26, 0.85),
            subdominant: Color::srgba(0.10, 0.20, 0.14, 0.85),
            dominant: Color::srgba(0.20, 0.10, 0.14, 0.85),
        }
    }
}

/// Colors for `dialogs::circle_of_fifths`' diagram, read from a theme's
/// `theme.json` under `"colors": { "circle_of_fifths": { ... } }`.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct CircleOfFifthsColors {
    /// The 11 key points not currently highlighted.
    #[serde(deserialize_with = "hex_color")]
    pub base: Color,
    /// The reference harp key's own point.
    #[serde(deserialize_with = "hex_color")]
    pub harp_key: Color,
    /// A position's point (one per step clockwise from the harp key).
    #[serde(deserialize_with = "hex_color")]
    pub position: Color,
}

impl Default for CircleOfFifthsColors {
    fn default() -> Self {
        Self {
            base: Color::srgba(0.30, 0.30, 0.36, 0.85),
            harp_key: Color::srgb(0.95, 0.80, 0.35),
            position: Color::srgb(0.35, 0.75, 0.95),
        }
    }
}

/// Parses a `"#RRGGBB"` / `"#RRGGBBAA"` string into a [`Color`]. `Color`'s own
/// derived `Deserialize` only accepts its verbose tagged-enum form (e.g.
/// `{"Srgba":{"red":0.06,"green":0.06,"blue":0.09,"alpha":1.0}}`), which is
/// unusable for hand-authored theme JSON — this is what lets theme.json write
/// plain hex strings instead, matching `theme_schema.dtd.json`.
fn hex_color<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    bevy::color::Srgba::hex(&s).map(Color::from).map_err(serde::de::Error::custom)
}

/// Blow/draw colors for scored note visuals (the falling-note highway in
/// Play2D/Play3D), read from a theme's `theme.json` under `"colors": {
/// "notes": { ... } }`. Distinct from [`SongEditorColors`]'s own blow/draw
/// fields (`lane_a`/`lane_b`) — the Song Editor is an authoring tool, not
/// the thing a colorblind player stares at while trying to hit notes in
/// real time, so it's untouched by [`crate::settings::ColorblindPalette`]
/// (see [`effective_note_colors`]).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct NoteColors {
    #[serde(deserialize_with = "hex_color")]
    pub blow: Color,
    #[serde(deserialize_with = "hex_color")]
    pub draw: Color,
}

impl Default for NoteColors {
    fn default() -> Self {
        Self { blow: Color::srgb(0.25, 0.55, 0.95), draw: Color::srgb(0.95, 0.38, 0.15) }
    }
}

/// A fixed, theme-independent blow/draw pair for
/// [`crate::settings::ColorblindPalette`]: blue paired with a bright,
/// high-luminance yellow rather than orange. Blue-vs-orange (any theme's
/// default [`NoteColors`]) already reads fine under red-green colorblindness
/// — that pairing is why it was picked in the first place — but orange and
/// blue sit at similar perceived brightness, so telling them apart still
/// leans on hue discrimination a colorblind player may have little of.
/// Blue-vs-yellow adds a large luminance gap on top of the hue difference,
/// which keeps working even under full monochromacy.
pub const COLORBLIND_NOTE_COLORS: NoteColors =
    NoteColors { blow: Color::srgb(0.0, 0.45, 0.75), draw: Color::srgb(0.95, 0.75, 0.0) };

/// Which blow/draw [`NoteColors`] a note visual should actually use: the
/// fixed [`COLORBLIND_NOTE_COLORS`] pair when the player has the
/// colorblind-palette setting on, otherwise whatever the active theme
/// provides (`theme_colors`, from [`LoadedTheme::note_colors`]). Pure so
/// `gameplay_2d`/`gameplay_3d`'s note spawners/tinters can resolve it once
/// per frame instead of re-deriving the same branch in each of them.
pub fn effective_note_colors(theme_colors: NoteColors, colorblind: bool) -> NoteColors {
    if colorblind { COLORBLIND_NOTE_COLORS } else { theme_colors }
}

/// Shared background for the gameplay song-timeline HUD's panels —
/// `gameplay::song_progress_overlay::spawn_song_progress`'s bar and
/// `music_score::spawn_music_score`'s staff panel — so they read as one
/// panel rather than two unrelated widgets.
pub const HUD_PANEL_BG: Color = Color::srgba(0.05, 0.05, 0.08, 0.85);

/// Shared 1px seam between the song-timeline HUD's stacked rows (waveform /
/// note-lanes / staff) — subtle enough not to compete with any of their own
/// content, present enough that the boundary between them reads as a
/// deliberate seam rather than an accidental gap.
pub const HUD_DIVIDER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);

/// The surface a modal's own content sits on — confirm/file dialogs and the
/// pause menu. **Near-opaque on purpose.** A modal that only dims what is
/// behind it still lets that content compete with its own (the pause
/// menu's small labels over the song-info panel's text, say). Dimming
/// controls *emphasis*; only an opaque surface controls what is legible.
pub const MODAL_PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.11, 0.98);

/// The wash a modal paints over the screen behind its [`MODAL_PANEL_BG`]
/// surface, dark enough to push a busy gameplay scene back without hiding
/// where the player is.
pub const MODAL_BACKDROP_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.78);

/// Border around a [`MODAL_PANEL_BG`] surface — the edge that makes it read
/// as a raised card rather than a hole cut in the backdrop.
pub const MODAL_PANEL_BORDER: Color = Color::srgb(0.30, 0.30, 0.40);

/// Colors for the song editor grid/panel, read from a theme's `theme.json`
/// under `"colors": { "song_editor": { ... } }`. The struct-level
/// `#[serde(default)]` means a theme can override just a few fields (e.g.
/// only `"accent"`) and every other field falls back to
/// [`SongEditorColors::default`] — the editor's original hardcoded palette.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct SongEditorColors {
    #[serde(deserialize_with = "hex_color")]
    pub editor_bg: Color,
    #[serde(deserialize_with = "hex_color")]
    pub hole_box: Color,
    #[serde(deserialize_with = "hex_color")]
    pub lane_a: Color,
    #[serde(deserialize_with = "hex_color")]
    pub lane_b: Color,
    #[serde(deserialize_with = "hex_color")]
    pub grid_line: Color,
    #[serde(deserialize_with = "hex_color")]
    pub bar_line: Color,
    #[serde(deserialize_with = "hex_color")]
    pub half_line: Color,
    #[serde(deserialize_with = "hex_color")]
    pub quarter_line: Color,
    /// Triplet-subdivision gridlines (ticks 4/8 of a beat) — distinct in
    /// *hue* from `quarter_line`'s straight-16th positions (ticks 3/6/9) so
    /// which family the grid is currently drawing reads at a glance. Only
    /// one family is on screen at a time, chosen by the active
    /// `harmonicon_core::snap::SnapMode`, so this belongs at roughly
    /// `half_line`'s weight rather than brighter: in Shuffle and Triplet it
    /// *is* the beat's only sub-beat line, not an extra layered over one.
    #[serde(deserialize_with = "hex_color")]
    pub triplet_line: Color,
    #[serde(deserialize_with = "hex_color")]
    pub accent: Color,
    #[serde(deserialize_with = "hex_color")]
    pub label: Color,
    #[serde(deserialize_with = "hex_color")]
    pub panel_bg: Color,
    #[serde(deserialize_with = "hex_color")]
    pub btn_bg: Color,
    #[serde(deserialize_with = "hex_color")]
    pub btn_active: Color,
    #[serde(deserialize_with = "hex_color")]
    pub field_bg: Color,
    #[serde(deserialize_with = "hex_color")]
    pub field_bg_focus: Color,
    #[serde(deserialize_with = "hex_color")]
    pub ghost_ok: Color,
    #[serde(deserialize_with = "hex_color")]
    pub ghost_bad: Color,
    /// Distinct per-button backgrounds for the transport strip
    /// (`song_editor::panel_widgets::transport_button`'s callers) — unlike
    /// `btn_bg` (uniform across every mod/mode/timeline-tool button), these
    /// are colour-coded per action for quick visual recognition.
    #[serde(deserialize_with = "hex_color")]
    pub transport_back: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_save: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_load: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_play: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_pause: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_stop: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_practice: Color,
    #[serde(deserialize_with = "hex_color")]
    pub transport_record: Color,
}

impl Default for SongEditorColors {
    fn default() -> Self {
        Self {
            editor_bg: Color::srgb(0.06, 0.06, 0.09),
            hole_box: Color::srgb(0.16, 0.16, 0.22),
            lane_a: Color::srgba(0.12, 0.12, 0.17, 1.0),
            lane_b: Color::srgba(0.10, 0.10, 0.14, 1.0),
            grid_line: Color::srgb(0.20, 0.20, 0.27),
            bar_line: Color::srgb(0.40, 0.40, 0.52),
            half_line: Color::srgb(0.17, 0.17, 0.23),
            quarter_line: Color::srgb(0.13, 0.13, 0.18),
            // A warm brown, deliberately distinct in hue (not brightness)
            // from the cool blue-grey grid_line/half_line/quarter_line
            // family, so triplet positions read as a different *kind* of
            // grid point. Kept a shade under `half_line` so it never
            // out-weighs the beat lines it sits between.
            triplet_line: Color::srgb(0.20, 0.149, 0.094),
            accent: Color::srgb(0.95, 0.80, 0.35),
            label: Color::srgb(0.75, 0.75, 0.82),
            panel_bg: Color::srgba(0.10, 0.10, 0.15, 1.0),
            btn_bg: Color::srgb(0.16, 0.16, 0.24),
            btn_active: Color::srgb(0.28, 0.42, 0.30),
            field_bg: Color::srgba(0.10, 0.10, 0.14, 1.0),
            field_bg_focus: Color::srgba(0.16, 0.16, 0.24, 1.0),
            ghost_ok: Color::srgb(0.98, 0.85, 0.20),
            ghost_bad: Color::srgb(0.90, 0.25, 0.20),
            transport_back: Color::srgb(0.22, 0.22, 0.28),
            transport_save: Color::srgb(0.18, 0.28, 0.45),
            transport_load: Color::srgb(0.24, 0.30, 0.20),
            transport_play: Color::srgb(0.20, 0.40, 0.24),
            transport_pause: Color::srgb(0.36, 0.32, 0.16),
            transport_stop: Color::srgb(0.36, 0.20, 0.20),
            transport_practice: Color::srgb(0.25, 0.18, 0.42),
            transport_record: Color::srgb(0.42, 0.15, 0.15),
        }
    }
}

// ── Runtime resource ──────────────────────────────────────────────────────────

/// Theme assets resolved at startup from `themes/<selected>/theme.json`.
/// Menus and buttons read this resource to apply backgrounds, icons, sounds,
/// and shader flags — themes control appearance only; layout (where a
/// button sits) is always the page's own flex flow, never theme-specified.
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
    pub colors: Option<ThemeColorsJson>,
}

impl LoadedTheme {
    /// Background for `menu_id`, falling back to the theme default.
    pub fn background_for(&self, menu_id: &str) -> Option<&Handle<Image>> {
        self.menu_backgrounds.get(menu_id).or(self.default_background.as_ref())
    }

    /// Song editor colors for the active theme, or [`SongEditorColors::default`]
    /// if the theme's `theme.json` has no `"colors"` block at all.
    pub fn song_editor_colors(&self) -> SongEditorColors {
        self.colors.as_ref().map_or_else(SongEditorColors::default, |c| c.song_editor)
    }

    /// 12-bar-blues chord-function colors for the active theme, or
    /// [`TwelveBarColors::default`] if the theme's `theme.json` has no
    /// `"colors"` block at all.
    pub fn twelve_bar_colors(&self) -> TwelveBarColors {
        self.colors.as_ref().map_or_else(TwelveBarColors::default, |c| c.twelve_bar)
    }

    /// Circle-of-fifths diagram colors for the active theme, or
    /// [`CircleOfFifthsColors::default`] if the theme's `theme.json` has no
    /// `"colors"` block at all.
    pub fn circle_of_fifths_colors(&self) -> CircleOfFifthsColors {
        self.colors.as_ref().map_or_else(CircleOfFifthsColors::default, |c| c.circle_of_fifths)
    }

    /// Blow/draw note colors for the active theme, or [`NoteColors::default`]
    /// if the theme's `theme.json` has no `"colors"` block at all. Callers
    /// wanting the colorblind-aware choice should pass this through
    /// [`effective_note_colors`] rather than using it directly.
    pub fn note_colors(&self) -> NoteColors {
        self.colors.as_ref().map_or_else(NoteColors::default, |c| c.notes)
    }
}

#[derive(Error, Debug)]
enum ThemeLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Loads a `theme.json` into a [`ThemeJson`] asset. Matched by full filename
/// (`extensions()` returns the compound extension `"theme.json"`, not the
/// bare `"json"`) so registering it can't collide with some other, unrelated
/// `.json` asset gaining an `AssetLoader` of its own later.
#[derive(Default, TypePath)]
struct ThemeJsonLoader;

impl AssetLoader for ThemeJsonLoader {
    type Asset = ThemeJson;
    type Settings = ();
    type Error = ThemeLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<ThemeJson, ThemeLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["theme.json"]
    }
}

/// Resolves the `AssetSource` prefix for `theme_name`: empty when it exists
/// under the bundled `assets/themes/`, or `"external://"` when it only
/// exists under the `~/Harmonicon/themes/` drop folder. Bundled wins when a
/// name exists in both places, matching `load_theme`'s resolution — so a
/// theme's own preview image (loaded separately by the theme picker) comes
/// from the same place as the rest of its assets.
///
/// Native only: `Path::is_dir()` needs a real local filesystem, which wasm
/// doesn't have. `AvailableThemes` under wasm only ever lists bundled
/// themes anyway (see `assets_management`'s wasm-only `scan_ui_themes` —
/// there's no `~/Harmonicon` drop folder inside a browser), so the wasm
/// sibling below can just always answer "bundled".
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub fn theme_source_prefix(theme_name: &str) -> &'static str {
    let bundled = std::path::Path::new("assets/themes").join(theme_name);
    if bundled.is_dir() { "" } else { "external://" }
}

/// wasm sibling of the native `theme_source_prefix` above.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
pub fn theme_source_prefix(_theme_name: &str) -> &'static str {
    ""
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedTheme>()
            .init_asset::<ThemeJson>()
            .register_asset_loader(ThemeJsonLoader)
            // PreUpdate runs before StateTransition, so when the player
            // navigates back to a menu after changing themes the OnEnter
            // setup system will see the already-refreshed LoadedTheme.
            // resource_changed fires on the frame after SelectedTheme is
            // written — including the first frame after Startup, when
            // apply_loaded_settings restores the saved theme name.
            .add_systems(
                PreUpdate,
                (
                    request_theme_load.run_if(|s: Res<SelectedTheme>| s.is_changed()),
                    apply_theme_when_loaded,
                )
                    .chain(),
            );
    }
}

/// The in-flight `theme.json` load kicked off by [`request_theme_load`],
/// consumed once it resolves by [`apply_theme_when_loaded`]. Absent
/// whenever no theme.json load is outstanding.
#[derive(Resource)]
struct PendingTheme {
    handle: Handle<ThemeJson>,
    /// Same meaning as `theme_source_prefix`'s return value — reused here so
    /// `apply_theme_when_loaded` can build sibling image/sound asset paths
    /// without re-resolving it (and without ever disagreeing with the
    /// prefix the `theme.json` handle itself was loaded through).
    prefix: &'static str,
    name: String,
}

/// Clears the previous theme's data immediately (so nothing stale from the
/// old theme lingers while the new one loads) and kicks off an
/// [`AssetServer`] load of the new theme's `theme.json` — through the same
/// `AssetSource` prefix (bundled or `external://`) its sibling images/sounds
/// already load through, rather than reading the file directly.
fn request_theme_load(
    mut commands: Commands,
    mut theme: ResMut<LoadedTheme>,
    selected: Res<SelectedTheme>,
    asset_server: Res<AssetServer>,
) {
    *theme = LoadedTheme::default();

    let prefix = theme_source_prefix(&selected.0);
    let handle = asset_server.load(format!("{prefix}themes/{}/theme.json", selected.0));
    commands.insert_resource(PendingTheme { handle, prefix, name: selected.0.clone() });
}

/// Polls the load [`request_theme_load`] started, applying it to
/// [`LoadedTheme`] the instant it resolves (a no-op most frames — `Option<Res<PendingTheme>>`
/// is `None` whenever nothing is in flight).
fn apply_theme_when_loaded(
    mut commands: Commands,
    pending: Option<Res<PendingTheme>>,
    theme_jsons: Res<Assets<ThemeJson>>,
    asset_server: Res<AssetServer>,
    mut theme: ResMut<LoadedTheme>,
) {
    let Some(pending) = pending else {
        return;
    };

    match asset_server.load_state(&pending.handle) {
        LoadState::Loaded => {}
        LoadState::Failed(err) => {
            warn!(
                "Could not load theme.json for '{}' from {}themes/{}/: {err}",
                pending.name, pending.prefix, pending.name
            );
            commands.remove_resource::<PendingTheme>();
            return;
        }
        _ => return, // still loading
    }
    let Some(data) = theme_jsons.get(&pending.handle) else {
        return;
    };

    theme.colors = Some(data.colors.clone());

    let prefix = format!("{}themes/{}/", pending.prefix, pending.name);

    if !data.default_background.image.is_empty() {
        theme.default_background =
            Some(asset_server.load(format!("{prefix}{}", data.default_background.image)));
    }

    for (menu_id, menu) in &data.menus {
        if let Some(bg) = &menu.background_image {
            theme
                .menu_backgrounds
                .insert(menu_id.clone(), asset_server.load(format!("{prefix}{bg}")));
        }
    }

    if let Some(icon) = &data.default_menu_button.icon {
        theme.btn_icon = Some(asset_server.load(format!("{prefix}{}", icon.image_file)));
    }

    if let Some(sounds) = &data.default_menu_button.button_sounds {
        theme.btn_sound_hover = Some(asset_server.load(format!("{prefix}{}", sounds.hover)));
        theme.btn_sound_click = Some(asset_server.load(format!("{prefix}{}", sounds.click)));
    }

    theme.has_shaders = data.default_menu_button.button_shaders.is_some();

    info!("Loaded theme '{}' from {prefix}", data.name);
    commands.remove_resource::<PendingTheme>();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Song editor colors ──────────────────────────────────────────────────

    #[test]
    fn song_editor_colors_parse_from_hex_strings() {
        let json = r##"{
            "editor_bg": "#0F0F17",
            "accent": "#F2CC59FF"
        }"##;
        let colors: SongEditorColors = serde_json::from_str(json).unwrap();
        assert_eq!(colors.editor_bg, Color::srgb_u8(0x0F, 0x0F, 0x17));
        assert_eq!(colors.accent, Color::srgb_u8(0xF2, 0xCC, 0x59));
        // Fields not present in the JSON fall back to the hardcoded defaults.
        assert_eq!(colors.hole_box, SongEditorColors::default().hole_box);
    }

    // ── Note colors ───────────────────────────────────────────────────────

    #[test]
    fn note_colors_parse_from_hex_strings() {
        let json = r##"{ "blow": "#123456", "draw": "#ABCDEF" }"##;
        let colors: NoteColors = serde_json::from_str(json).unwrap();
        assert_eq!(colors.blow, Color::srgb_u8(0x12, 0x34, 0x56));
        assert_eq!(colors.draw, Color::srgb_u8(0xAB, 0xCD, 0xEF));
    }

    #[test]
    fn loaded_theme_falls_back_to_default_note_colors_without_a_colors_block() {
        let theme = LoadedTheme::default();
        assert_eq!(theme.note_colors(), NoteColors::default());
    }

    #[test]
    fn effective_note_colors_uses_the_theme_when_not_colorblind() {
        let theme = NoteColors::default();
        assert_eq!(effective_note_colors(theme, false), theme);
    }

    #[test]
    fn effective_note_colors_overrides_the_theme_when_colorblind() {
        let theme = NoteColors::default();
        assert_eq!(effective_note_colors(theme, true), COLORBLIND_NOTE_COLORS);
        assert_ne!(COLORBLIND_NOTE_COLORS, theme);
    }

    #[test]
    fn shipped_theme_jsons_with_colors_block_parse_end_to_end() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/themes");
        for name in ["default", "BluesNoir"] {
            let path = root.join(name).join("theme.json");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
            let data: ThemeJson = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{path:?} failed to parse: {e}"));
            // Both shipped themes ship an explicit song_editor colors block;
            // confirm it actually deserialized rather than silently defaulting.
            assert_ne!(
                data.colors.song_editor.editor_bg,
                Color::NONE,
                "{path:?}: editor_bg should be a real color",
            );
        }
    }

    // ── JSON parsing ──────────────────────────────────────────────────────

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
    fn theme_json_menu_without_background_leaves_it_none() {
        let json = r#"{
            "name": "T",
            "default_background": { "image": "" },
            "menus": { "Credits": {} }
        }"#;
        let d: ThemeJson = serde_json::from_str(json).unwrap();
        assert!(d.menus["Credits"].background_image.is_none());
    }

    #[test]
    fn theme_json_menu_with_background_reads_it() {
        let json = r#"{
            "name": "T",
            "default_background": { "image": "" },
            "menus": { "Credits": { "background_image": "bg.png" } }
        }"#;
        let d: ThemeJson = serde_json::from_str(json).unwrap();
        assert_eq!(d.menus["Credits"].background_image.as_deref(), Some("bg.png"));
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
        let theme =
            LoadedTheme { default_background: Some(Handle::default()), ..Default::default() };
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
            .add_systems(PreUpdate, count_reloads.run_if(|s: Res<SelectedTheme>| s.is_changed()));

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
            .add_systems(PreUpdate, count_reloads.run_if(|s: Res<SelectedTheme>| s.is_changed()));

        app.update(); // insert fires once
        for theme in ["dark", "light", "neon", "default"] {
            app.world_mut().resource_mut::<SelectedTheme>().0 = theme.into();
            app.update();
        }
        assert_eq!(app.world().resource::<ReloadCount>().0, 5);
    }
}
