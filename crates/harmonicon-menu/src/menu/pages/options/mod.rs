// SPDX-License-Identifier: MIT

//! The Options page: audio volume sliders, the microphone and pitch-detection
//! pickers, and display toggles. Owns its page lifecycle via
//! [`OptionsPlugin`]; the menu shell only routes to it.

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::Button as WidgetButton;
use bevy::ui_widgets::{
    Activate, Slider, SliderRange, SliderStep, SliderValue, TrackClick, ValueChange,
    slider_self_update,
};

const TRACK_BG: Color = Color::srgb(0.14, 0.14, 0.22);

use harmonicon_audio::AudioSettings;
use harmonicon_audio::audio_input::{self, MicStatus};
use harmonicon_platform::assets_management::ShowNoteNumbers;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{cleanup_menu, spawn_back_button, spawn_button, spawn_menu_root};
use harmonicon_app::app::AppState;

use harmonicon_ui::dialogs::algo_picker::{algo_labels, attach_algo_tooltip, on_algo_selected};
use harmonicon_ui::dialogs::button;
use harmonicon_ui::dialogs::checkbox;
use harmonicon_ui::dialogs::combobox;
use harmonicon_ui::dialogs::tooltip::Tooltip;

mod microphone;
mod sliders;
#[cfg(test)]
mod tests;
mod zoom;

use microphone::*;
use sliders::*;
use zoom::*;

/// Owns the Options page: builds it on entry, tears it down on exit, and runs
/// the slider interaction systems while it's open.
pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuPage::Options), setup_options_menu)
            .add_systems(OnExit(MenuPage::Options), cleanup_menu)
            // Keep each slider's own SliderValue in sync as it's dragged or
            // stepped, so keyboard adjustment works from the current value.
            .add_observer(slider_self_update)
            // Sliders carry their own change behaviour as inline on(...)
            // observers; these systems only mirror settings onto the visuals.
            .add_systems(
                Update,
                (
                    update_sliders,
                    update_latency_slider,
                    update_mic_banner,
                    sync_mic_combobox,
                    update_zoom_slider_visuals,
                    sync_zoom_slider_from_ui_scale,
                )
                    .run_if(in_state(MenuPage::Options)),
            );
    }
}

// ── Components ──────────────────────────────────────────────────────────────

/// Which audio level a slider controls.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
enum VolumeSlider {
    #[default]
    Music,
    Metronome,
}

/// The growing fill of a slider track; its width mirrors the bound level.
#[derive(Component, Default, Clone)]
struct SliderFill(VolumeSlider);

/// The "NN%" readout beside a slider.
#[derive(Component)]
struct SliderValueLabel(VolumeSlider);

/// The "no microphone" warning banner, hidden only while
/// [`MicStatus::Connected`] — see [`mic_banner_visible`]. See TODO.md: "No
/// microphone = silent failure."
#[derive(Component)]
struct MicBanner;

/// The failure-reason text inside [`MicBanner`].
#[derive(Component, Default, Clone)]
struct MicBannerText;

/// Marks the drag track of the input-latency slider.
#[derive(Component, Default, Clone)]
struct LatencySlider;

/// The fill bar inside the latency slider track.
#[derive(Component, Default, Clone)]
struct LatencySliderFill;

/// The "Xms" readout beside the latency slider.
#[derive(Component)]
struct LatencySliderLabel;

/// The "Zoom: N%" readout beside the zoom slider.
#[derive(Component)]
struct ZoomLabel;

/// Current level for a given slider kind.
fn audio_level(settings: &AudioSettings, kind: VolumeSlider) -> f32 {
    match kind {
        VolumeSlider::Music => settings.music_volume,
        VolumeSlider::Metronome => settings.metronome_volume,
    }
}

/// Width of the label column beside a slider or picker. Sized for the
/// longest translation on the page ("Detecção de tom", "Atraso de
/// entrada"), not the English, so a label never wraps onto two lines and
/// pushes its control down. Mirrors `dialogs::combobox::LABEL_WIDTH`.
const OPTIONS_LABEL_WIDTH: f32 = 190.0;

// ── Page setup ────────────────────────────────────────────────────────────────

fn setup_options_menu(
    mut commands: Commands,
    loc: Res<Localization>,
    settings: Res<AudioSettings>,
    mic_status: Res<MicStatus>,
    theme: Res<LoadedTheme>,
    show_numbers: Res<ShowNoteNumbers>,
    adaptive_difficulty: Res<harmonicon_platform::settings::AdaptiveDifficultyEnabled>,
    fullscreen: Res<harmonicon_platform::settings::FullscreenEnabled>,
    colorblind_palette: Res<harmonicon_platform::settings::ColorblindPalette>,
    reduced_motion: Res<harmonicon_platform::settings::ReducedMotion>,
    action_button_style: Res<harmonicon_platform::settings::ActionButtonStyle>,
    ui_scale: Res<UiScale>,
) {
    let (root, header, page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("options-title"),
        Some(&loc.msg("options-subtitle-audio")),
        &theme,
        "Options",
    );

    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("options-back-tooltip"),
        |_: On<Activate>,
         mut welcome: ResMut<harmonicon_app::app::WelcomeFlow>,
         mut page: ResMut<NextState<MenuPage>>| {
            // Back to Welcome when this is the first-run "set up your
            // microphone" step, else to Main as usual.
            page.set(welcome.back_target(MenuPage::Main, MenuPage::Welcome, |w| {
                w.mic_done = true;
            }));
        },
    );

    // Two columns, sized to their own content (like every other menu page's
    // body) rather than a fixed screen percentage — `root` here is the
    // shared scrollable body area (`menu::scene::spawn_menu_root`), which
    // itself sizes to *its* content and gets centered as a whole, so a
    // percentage width/height on `main_layout` would resolve against that
    // auto-sized ancestor instead of the actual screen, producing an
    // arbitrary, off-center result. Left undefined, `main_layout` and its
    // two columns naturally size to their own content and are centered by
    // `root`'s own centering — same mechanism every single-column page
    // already relies on.
    let main_layout = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(20.0), ..default() })
        .id();

    let left_layout = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            // `row_gap`, not `column_gap` — this stacks rows vertically, so
            // the gap that matters is between rows, along the main axis.
            row_gap: Val::Px(20.0),
            ..default()
        })
        .id();

    let right_layout = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(20.0), ..default() })
        .id();

    commands.entity(root).add_child(main_layout);
    commands.entity(main_layout).add_child(left_layout);
    commands.entity(main_layout).add_child(right_layout);

    spawn_left_column(
        &mut commands,
        left_layout,
        page_root,
        mic_status,
        settings,
        &loc,
        show_numbers,
        adaptive_difficulty,
        fullscreen,
        colorblind_palette,
        reduced_motion,
        *action_button_style,
        ui_scale.0,
    );
    spawn_right_column(&mut commands, right_layout, &loc);
}

fn spawn_left_column(
    commands: &mut Commands,
    parent: Entity,
    page_root: Entity,
    mic_status: Res<MicStatus>,
    settings: Res<AudioSettings>,
    loc: &Localization,
    show_numbers: Res<ShowNoteNumbers>,
    adaptive_difficulty: Res<harmonicon_platform::settings::AdaptiveDifficultyEnabled>,
    fullscreen: Res<harmonicon_platform::settings::FullscreenEnabled>,
    colorblind_palette: Res<harmonicon_platform::settings::ColorblindPalette>,
    reduced_motion: Res<harmonicon_platform::settings::ReducedMotion>,
    action_button_style: harmonicon_platform::settings::ActionButtonStyle,
    ui_scale: f32,
) {
    spawn_mic_banner(commands, parent, &mic_status, loc);
    spawn_volume_slider(
        commands,
        parent,
        &loc.msg("options-music"),
        &loc.msg("options-music-volume-tooltip"),
        VolumeSlider::Music,
        settings.music_volume,
        set_music_volume,
    );
    spawn_volume_slider(
        commands,
        parent,
        &loc.msg("options-metronome"),
        &loc.msg("options-metronome-volume-tooltip"),
        VolumeSlider::Metronome,
        settings.metronome_volume,
        set_metronome_volume,
    );
    spawn_latency_slider(commands, parent, settings.input_latency_ms, loc);
    spawn_mic_combobox(
        commands,
        parent,
        page_root,
        loc,
        &audio_input::input_device_names(),
        connected_device_name(&mic_status),
    );

    let algo_combo = combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-pitch-detect"),
        &algo_labels(loc),
        settings.pitch_algorithm.label(),
        on_algo_selected,
    );
    attach_algo_tooltip(commands, algo_combo, settings.pitch_algorithm);

    spawn_note_numbers_toggle(commands, parent, loc, show_numbers.0);
    spawn_adaptive_difficulty_toggle(commands, parent, adaptive_difficulty.0, loc);
    spawn_fullscreen_toggle(commands, parent, fullscreen.0, loc);
    spawn_colorblind_palette_toggle(commands, parent, colorblind_palette.0, loc);
    spawn_reduced_motion_toggle(commands, parent, reduced_motion.0, loc);
    spawn_zoom_slider(commands, parent, ui_scale, loc);
    spawn_action_button_style_combobox(commands, parent, page_root, loc, action_button_style);
}

fn spawn_right_column(commands: &mut Commands, parent: Entity, loc: &Localization) {
    let theme_btn = spawn_button(
        commands,
        parent,
        &loc.msg("options-theme"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Theme),
    );
    commands.entity(theme_btn).insert(Tooltip(String::from(loc.msg("options-theme-tooltip"))));

    // Downloaded content: wasm's is bundled at build time, so it has no
    // repositories to manage.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let content_btn = spawn_button(
            commands,
            parent,
            &loc.msg("options-content"),
            |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| {
                page.set(MenuPage::ContentSources)
            },
        );
        commands
            .entity(content_btn)
            .insert(Tooltip(String::from(loc.msg("options-content-tooltip"))));
    }

    let calibrate_btn = spawn_button(
        commands,
        parent,
        &loc.msg("options-calibrate-input-lag"),
        |_: On<Activate>, mut state: ResMut<NextState<AppState>>| state.set(AppState::Calibration),
    );
    commands
        .entity(calibrate_btn)
        .insert(Tooltip(String::from(loc.msg("options-calibrate-input-lag-tooltip"))));
}

/// Flips whether falling notes show their hole number instead of the
/// blow/draw arrow (`gameplay_2d`/`gameplay_3d`'s note spawners read this).
fn set_note_numbers(ev: On<ValueChange<bool>>, mut show: ResMut<ShowNoteNumbers>) {
    show.0 = ev.value;
}

/// A checkbox bound to [`ShowNoteNumbers`], with a tooltip explaining the
/// two display modes it switches between.
fn spawn_note_numbers_toggle(
    commands: &mut Commands,
    parent: Entity,
    loc: &Localization,
    show_numbers: bool,
) {
    let row = checkbox::spawn_checkbox(
        commands,
        parent,
        &loc.msg("options-note-labels"),
        show_numbers,
        set_note_numbers,
    );
    commands.entity(row).insert(Tooltip(String::from(loc.msg("options-note-labels-tooltip"))));
}

/// Flips the single global adaptive-difficulty setting — not per-song, see
/// `settings::AdaptiveDifficultyEnabled`'s doc comment. Persisted
/// automatically by `settings`'s debounced-save machinery, same as every
/// other Options-page toggle; doesn't touch the live per-session
/// `gameplay::adaptive_difficulty::AdaptiveDifficulty` cache — that only
/// gets (re)seeded from this setting at the next song's start, or flipped
/// directly by the pause menu's own toggle for an immediate mid-song effect.
fn set_adaptive_difficulty(
    ev: On<ValueChange<bool>>,
    mut enabled: ResMut<harmonicon_platform::settings::AdaptiveDifficultyEnabled>,
) {
    enabled.0 = ev.value;
}

/// A checkbox bound to the global adaptive-difficulty setting — see
/// `settings::AdaptiveDifficultyEnabled`'s doc comment for what it does and
/// how it interacts with the pause menu's own live toggle.
fn spawn_adaptive_difficulty_toggle(
    commands: &mut Commands,
    parent: Entity,
    enabled: bool,
    loc: &Localization,
) {
    let row = checkbox::spawn_checkbox(
        commands,
        parent,
        &loc.msg("options-adaptive-difficulty"),
        enabled,
        set_adaptive_difficulty,
    );
    commands
        .entity(row)
        .insert(Tooltip(String::from(loc.msg("options-adaptive-difficulty-tooltip"))));
}

/// Flips the fullscreen preference; `settings::apply_fullscreen` mirrors the
/// resulting `FullscreenEnabled` onto the primary window's `WindowMode`.
fn set_fullscreen(
    ev: On<ValueChange<bool>>,
    mut fullscreen: ResMut<harmonicon_platform::settings::FullscreenEnabled>,
) {
    fullscreen.0 = ev.value;
}

/// A checkbox bound to the fullscreen setting.
fn spawn_fullscreen_toggle(
    commands: &mut Commands,
    parent: Entity,
    enabled: bool,
    loc: &Localization,
) {
    let row = checkbox::spawn_checkbox(
        commands,
        parent,
        &loc.msg("options-fullscreen"),
        enabled,
        set_fullscreen,
    );
    commands.entity(row).insert(Tooltip(String::from(loc.msg("options-fullscreen-tooltip"))));
}

/// A combobox picking `settings::ActionButtonStyle` — how the Song Editor's
/// action buttons render (icon only, icon + text, or text only).
fn spawn_action_button_style_combobox(
    commands: &mut Commands,
    parent: Entity,
    page_root: Entity,
    loc: &Localization,
    current: harmonicon_platform::settings::ActionButtonStyle,
) {
    let options: Vec<String> = harmonicon_platform::settings::ActionButtonStyle::all()
        .iter()
        .map(|s| String::from(loc.msg(s.loc_key())))
        .collect();
    let combo = combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-button-style"),
        &options,
        &loc.msg(current.loc_key()),
        on_action_button_style_selected,
    );
    commands.entity(combo).insert(Tooltip(String::from(loc.msg("options-button-style-tooltip"))));
}

fn on_action_button_style_selected(
    ev: On<combobox::ComboboxSelect>,
    loc: Res<Localization>,
    mut style: ResMut<harmonicon_platform::settings::ActionButtonStyle>,
) {
    if let Some(picked) =
        harmonicon_platform::settings::ActionButtonStyle::from_localized_label(&loc, &ev.value)
    {
        *style = picked;
    }
}

/// Flips whether scored notes use the fixed colorblind-safe blow/draw pair
/// instead of the active theme's own note colors — see
/// `settings::ColorblindPalette`'s doc comment.
fn set_colorblind_palette(
    ev: On<ValueChange<bool>>,
    mut enabled: ResMut<harmonicon_platform::settings::ColorblindPalette>,
) {
    enabled.0 = ev.value;
}

/// A checkbox bound to the colorblind-palette setting.
fn spawn_colorblind_palette_toggle(
    commands: &mut Commands,
    parent: Entity,
    enabled: bool,
    loc: &Localization,
) {
    let row = checkbox::spawn_checkbox(
        commands,
        parent,
        &loc.msg("options-colorblind-palette"),
        enabled,
        set_colorblind_palette,
    );
    commands
        .entity(row)
        .insert(Tooltip(String::from(loc.msg("options-colorblind-palette-tooltip"))));
}

/// Stills the highway's decorative motion — see
/// `settings::ReducedMotion`'s doc comment.
fn set_reduced_motion(
    ev: On<ValueChange<bool>>,
    mut enabled: ResMut<harmonicon_platform::settings::ReducedMotion>,
) {
    enabled.0 = ev.value;
}

/// A checkbox bound to the reduced-motion setting.
fn spawn_reduced_motion_toggle(
    commands: &mut Commands,
    parent: Entity,
    enabled: bool,
    loc: &Localization,
) {
    let row = checkbox::spawn_checkbox(
        commands,
        parent,
        &loc.msg("options-reduced-motion"),
        enabled,
        set_reduced_motion,
    );
    commands.entity(row).insert(Tooltip(String::from(loc.msg("options-reduced-motion-tooltip"))));
}
