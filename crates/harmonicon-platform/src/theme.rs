// SPDX-License-Identifier: MIT

//! The two built-in palettes and shared UI colors.
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};

use crate::assets_management::SelectedTheme;

/// Chord-function colors for a 12-bar blues progression (I / IV / V).
/// Shared by the Jam Session bar grid (`gameplay::twelve_bar_blues_overlay`)
/// and the song editor's grid background, so both reflect the same theme.
#[derive(Clone, Copy, Debug)]

pub struct TwelveBarColors {
    /// I (tonic) — the "home" chord most bars sit on.
    pub tonic: Color,
    /// IV (subdominant).
    pub subdominant: Color,
    /// V (dominant).
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

/// Colors for the circle-of-fifths diagram.
#[derive(Clone, Copy, Debug)]

pub struct CircleOfFifthsColors {
    /// The 11 key points not currently highlighted.
    pub base: Color,
    /// The reference harp key's own point.
    pub harp_key: Color,
    /// A position's point (one per step clockwise from the harp key).
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

/// Blow/draw colors for scored note visuals (the falling-note highway in
/// Play 2D). Distinct from [`SongEditorColors`]'s own blow/draw
/// fields (`lane_a`/`lane_b`) — the Song Editor is an authoring tool, not
/// the thing a colorblind player stares at while trying to hit notes in
/// real time, so it's untouched by [`crate::settings::ColorblindPalette`]
/// (see [`effective_note_colors`]).
#[derive(Clone, Copy, Debug, PartialEq)]

pub struct NoteColors {
    pub blow: Color,

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
/// `gameplay_2d`'s note spawners/tinters can resolve it once per frame
/// instead of re-deriving the same branch in each of them.
pub fn effective_note_colors(theme_colors: NoteColors, colorblind: bool) -> NoteColors {
    if colorblind { COLORBLIND_NOTE_COLORS } else { theme_colors }
}

/// Shared background for the gameplay song-timeline HUD's panels —
/// `gameplay::song_progress_overlay::spawn_song_progress`'s bar and
/// `music_score::spawn_music_score`'s staff panel — so they read as one
/// panel rather than two unrelated widgets.
pub const HUD_PANEL_BG: Color = rgb(0x192630);

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

/// Colors for the editor grid, toolbars, and transport.
#[derive(Clone, Copy, Debug)]

pub struct SongEditorColors {
    pub editor_bg: Color,

    pub hole_box: Color,

    pub lane_a: Color,

    pub lane_b: Color,

    pub grid_line: Color,

    pub bar_line: Color,

    pub half_line: Color,

    pub quarter_line: Color,
    /// Triplet-subdivision gridlines (ticks 4/8 of a beat) — distinct in
    /// *hue* from `quarter_line`'s straight-16th positions (ticks 3/6/9) so
    /// which family the grid is currently drawing reads at a glance. Only
    /// one family is on screen at a time, chosen by the active
    /// `harmonicon_core::snap::SnapMode`, so this belongs at roughly
    /// `half_line`'s weight rather than brighter: in Shuffle and Triplet it
    /// *is* the beat's only sub-beat line, not an extra layered over one.
    pub triplet_line: Color,

    pub accent: Color,

    pub label: Color,

    pub panel_bg: Color,

    pub btn_bg: Color,

    pub btn_active: Color,

    pub field_bg: Color,

    pub field_bg_focus: Color,

    pub ghost_ok: Color,

    pub ghost_bad: Color,
    /// Distinct per-button backgrounds for the transport strip
    /// (`song_editor::panel_widgets::transport_button`'s callers) — unlike
    /// `btn_bg` (uniform across every mod/mode/timeline-tool button), these
    /// are colour-coded per action for quick visual recognition.
    pub transport_back: Color,

    pub transport_save: Color,

    pub transport_load: Color,

    pub transport_play: Color,

    pub transport_pause: Color,

    pub transport_stop: Color,

    pub transport_practice: Color,

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

// ── App palettes ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    Light,
    #[default]
    Dark,
}

impl Appearance {
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    /// Older settings used folder names, including default and BluesNoir.
    pub fn from_saved(id: &str) -> Self {
        if id.eq_ignore_ascii_case("light") { Self::Light } else { Self::Dark }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Light => "theme-light",
            Self::Dark => "theme-dark",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                canvas: rgb(0xEDF2F4),
                panel: rgb(0xFFFFFF),
                inset: rgb(0xE2EAEE),
                border: rgb(0xB7C7CE),
                text: rgb(0x19272E),
                muted: rgb(0x4A626F),
                accent: rgb(0x076554),
                control: rgb(0xDCE7EA),
                hover: rgb(0xCADCDF),
                selected: rgb(0xBDE8D9),
            },
            Self::Dark => Palette {
                canvas: rgb(0x111A22),
                panel: rgb(0x192630),
                inset: rgb(0x101921),
                border: rgb(0x394C57),
                text: rgb(0xEAF2F4),
                muted: rgb(0xAEC1C9),
                accent: rgb(0x78DBC2),
                control: rgb(0x253744),
                hover: rgb(0x364C59),
                selected: rgb(0x215448),
            },
        }
    }
}

const fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color,
    pub panel: Color,
    pub inset: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub control: Color,
    pub hover: Color,
    pub selected: Color,
}

/// The current palette, resolved synchronously so newly opened pages can
/// immediately read the selected appearance.
#[derive(Resource, Default)]
pub struct LoadedTheme(pub Appearance);

impl LoadedTheme {
    pub fn palette(&self) -> Palette {
        self.0.palette()
    }

    pub fn song_editor_colors(&self) -> SongEditorColors {
        let p = self.palette();
        let mut colors = SongEditorColors {
            editor_bg: p.canvas,
            panel_bg: p.panel,
            hole_box: p.control,
            lane_a: p.panel,
            lane_b: p.inset,
            label: p.muted,
            accent: p.accent,
            btn_bg: p.control,
            field_bg: p.inset,
            field_bg_focus: p.control,
            ..default()
        };
        if self.0 == Appearance::Light {
            colors.grid_line = rgb(0xB7C7CE);
            colors.bar_line = rgb(0x839AA5);
            colors.half_line = rgb(0xC4D2D9);
            colors.quarter_line = rgb(0xD6E0E4);
            colors.triplet_line = rgb(0xD9CFBC);
            colors.ghost_ok = p.accent;
        } else {
            colors.grid_line = p.border;
            colors.bar_line = rgb(0x617985);
            colors.half_line = rgb(0x2E404B);
            colors.quarter_line = rgb(0x24343F);
        }
        colors
    }

    pub fn twelve_bar_colors(&self) -> TwelveBarColors {
        match self.0 {
            Appearance::Dark => TwelveBarColors::default(),
            Appearance::Light => TwelveBarColors {
                tonic: rgb(0xDAE9F5),
                subdominant: rgb(0xD8EBE2),
                dominant: rgb(0xF0E2D5),
            },
        }
    }

    pub fn circle_of_fifths_colors(&self) -> CircleOfFifthsColors {
        CircleOfFifthsColors::default()
    }

    pub fn note_colors(&self) -> NoteColors {
        NoteColors::default()
    }
}

/// Surfaces that should follow the current theme, including while the theme
/// picker is already open.
#[derive(Component, Clone, Copy)]
#[require(BackgroundColor)]
pub enum ThemeSurface {
    Canvas,
    Panel,
    Inset,
    Control,
    Hover,
    Selected,
    Divider,
    HitLine,
    HitZone,
}

fn apply_appearance(
    selected: Res<SelectedTheme>,
    mut theme: ResMut<LoadedTheme>,
    mut clear: ResMut<ClearColor>,
) {
    theme.0 = selected.0;
    clear.0 = theme.palette().canvas;
}

fn paint_surfaces(
    theme: Res<LoadedTheme>,
    mut surfaces: Query<(&ThemeSurface, &mut BackgroundColor)>,
) {
    let p = theme.palette();
    for (role, mut color) in &mut surfaces {
        let wanted = match role {
            ThemeSurface::Canvas => p.canvas,
            ThemeSurface::Panel => p.panel,
            ThemeSurface::Inset => p.inset,
            ThemeSurface::Control => p.control,
            ThemeSurface::Hover => p.hover,
            ThemeSurface::Selected => p.selected,
            ThemeSurface::Divider => p.border,
            ThemeSurface::HitLine => p.accent,
            ThemeSurface::HitZone => p.selected.with_alpha(0.45),
        };
        if color.0 != wanted {
            color.0 = wanted;
        }
    }
}

/// Keep authored ink on instrument diagrams and colored controls whenever it
/// is legible. On solid UI surfaces, resolve low-contrast labels against the
/// composed background. Track only colors that need adapting so a later
/// state change or theme switch can restore the authored color.
fn ensure_text_contrast(
    theme: Res<LoadedTheme>,
    mut texts: Query<
        (Entity, &mut TextColor, Option<&mut TextCursorStyle>),
        Or<(With<Text>, With<TextSpan>, With<EditableText>)>,
    >,
    surfaces: Query<(Option<&BackgroundColor>, Option<&ChildOf>, Has<ImageNode>), With<Node>>,
    parents: Query<&ChildOf>,
    mut adjusted: Local<bevy::ecs::entity::EntityHashMap<(Color, Color)>>,
) {
    let palette = theme.palette();
    adjusted.retain(|entity, _| texts.contains(*entity));
    for (entity, mut text, cursor) in &mut texts {
        let source = adjusted
            .get(&entity)
            .filter(|(_, applied)| *applied == text.0)
            .map_or(text.0, |(original, _)| *original);
        let mut ancestor = Some(entity);
        let mut premultiplied = Vec3::ZERO;
        let mut remaining = 1.0;
        let mut image = false;
        while let Some(node) = ancestor {
            if let Ok((background, parent, has_image)) = surfaces.get(node) {
                if has_image {
                    image = true;
                    break;
                }
                if let Some(background) = background {
                    let linear = background.0.to_linear();
                    premultiplied += Vec3::new(linear.red, linear.green, linear.blue)
                        * (linear.alpha * remaining);
                    remaining *= 1.0 - linear.alpha;
                    if remaining <= 0.001 {
                        break;
                    }
                }
                ancestor = parent.map(ChildOf::parent);
            } else {
                ancestor = parents.get(node).ok().map(ChildOf::parent);
            }
        }
        let canvas = palette.canvas.to_linear();
        let background =
            premultiplied + Vec3::new(canvas.red, canvas.green, canvas.blue) * remaining;
        let resolved = if image { source } else { readable_ink(source, background) };
        if source != resolved {
            adjusted.insert(entity, (source, resolved));
        } else {
            adjusted.remove(&entity);
        }
        if text.0 != resolved {
            text.0 = resolved;
        }
        if let Some(mut cursor) = cursor
            && cursor.color != resolved
        {
            cursor.color = resolved;
        }
    }
}

fn readable_ink(source: Color, background: Vec3) -> Color {
    let luminance = |rgb: Vec3| rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let linear = source.to_linear();
    let ink = Vec3::new(linear.red, linear.green, linear.blue);
    let bg = luminance(background);
    let contrast = |fg: f32| (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05);
    if linear.alpha <= 0.01 || contrast(luminance(ink)) >= 4.5 {
        return source;
    }

    if bg > 0.40 && ink.min_element() >= 0.70 {
        return Appearance::Light.palette().text.with_alpha(linear.alpha);
    }

    // Preserve the hue by mixing toward the readable end of the palette.
    let target =
        if bg > 0.179 { Appearance::Light.palette().text } else { Appearance::Dark.palette().text };
    let target = target.to_linear();
    let target = Vec3::new(target.red, target.green, target.blue);
    let target = if contrast(luminance(target)) >= 4.5 {
        target
    } else if bg > 0.179 {
        Vec3::ZERO
    } else {
        Vec3::ONE
    };
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..7 {
        let amount = (low + high) / 2.0;
        if contrast(luminance(ink.lerp(target, amount))) >= 4.5 {
            high = amount;
        } else {
            low = amount;
        }
    }
    let rgb = ink.lerp(target, high);
    Color::linear_rgba(rgb.x, rgb.y, rgb.z, linear.alpha)
}

#[derive(Default)]
pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedTheme>()
            .init_resource::<LoadedTheme>()
            .init_resource::<ClearColor>()
            .add_systems(PreUpdate, apply_appearance.run_if(resource_changed::<SelectedTheme>))
            .add_systems(
                PostUpdate,
                (paint_surfaces, ensure_text_contrast).chain().before(bevy::ui::UiSystems::Content),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_select_dark_and_light_round_trips() {
        for old in ["default", "BluesNoir", "my-external-theme"] {
            assert_eq!(Appearance::from_saved(old), Appearance::Dark);
        }
        for theme in Appearance::ALL {
            assert_eq!(Appearance::from_saved(theme.id()), theme);
        }
    }

    #[test]
    fn contrasting_ink_is_preserved_and_white_ink_adapts_on_light() {
        let light = Appearance::Light.palette();
        let light_bg = light.panel.to_linear();
        let bg = Vec3::new(light_bg.red, light_bg.green, light_bg.blue);
        assert_eq!(readable_ink(light.text, bg), light.text);
        assert!(readable_ink(Color::WHITE, bg).to_linear().red < 0.19);
        assert_eq!(readable_ink(Color::WHITE, Vec3::ZERO), Color::WHITE);
    }

    #[test]
    fn switching_appearance_updates_existing_surfaces_and_restores_authored_ink() {
        let mut app = App::new();
        app.add_plugins(ThemePlugin);
        let root = app.world_mut().spawn((Node::default(), ThemeSurface::Canvas)).id();
        let title = app
            .world_mut()
            .spawn((Text::new("Title"), TextColor(Color::WHITE), ChildOf(root)))
            .id();

        app.world_mut().resource_mut::<SelectedTheme>().0 = Appearance::Light;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(root).unwrap().0,
            Appearance::Light.palette().canvas
        );
        assert_eq!(
            app.world().get::<TextColor>(title).unwrap().0,
            Appearance::Light.palette().text
        );

        app.world_mut().resource_mut::<SelectedTheme>().0 = Appearance::Dark;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(root).unwrap().0,
            Appearance::Dark.palette().canvas
        );
        assert_eq!(app.world().get::<TextColor>(title).unwrap().0, Color::WHITE);
    }

    #[test]
    fn colorblind_colors_override_both_palettes() {
        for appearance in Appearance::ALL {
            let notes = LoadedTheme(appearance).note_colors();
            assert_eq!(effective_note_colors(notes, false), notes);
            assert_eq!(effective_note_colors(notes, true), COLORBLIND_NOTE_COLORS);
        }
    }
}
