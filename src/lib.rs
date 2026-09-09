// SPDX-License-Identifier: MIT

//! The composition root: every plugin the game is assembled from, and the
//! `run()` that starts it.
//!
//! This lives in a library rather than in `main.rs` because Android has no
//! `main` — the platform loads a shared library and calls `android_main`
//! (`crates/harmonicon-android`). Both entry points are thin wrappers around
//! `run()`, so there is still exactly one place the app is assembled.

/// Dev-only remote control: screenshots, video capture and live state
/// inspection over the Bevy Remote Protocol. Compiled out entirely without
/// `--features dev`, since it serves unauthenticated RPC.
#[cfg(feature = "dev")]
mod dev_capture;

use bevy::asset::AssetPlugin;
use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;

/// Reverse-DNS app id. On Wayland the icon comes from a matching desktop file
/// (`<APP_ID>.desktop`); this sets the window's app_id so the compositor can find
/// it. On X11/Windows/macOS the pixel icon set in `set_window_icon` is used.
const APP_ID: &str = "io.github.tcanabrava.Harmonicon";

use harmonicon_app::app::AppState;
use harmonicon_app::profile::ProfilePlugin;
use harmonicon_audio::pitch_detect::{AudioFrame, PitchEvent, PitchRange};
use harmonicon_audio::{audio_input, pipeline};
use harmonicon_gameplay::gameplay::GameplayPlugin;
use harmonicon_jam::jam::JamPlugin;
use harmonicon_menu::menu::MenuPlugin;
use harmonicon_platform::assets_management::AssetsManagementPlugin;
use harmonicon_platform::localization::LocalizationPlugin;
use harmonicon_platform::responsive::ResponsivePlugin;
use harmonicon_platform::settings::SettingsPlugin;
use harmonicon_platform::theme::ThemePlugin;
use harmonicon_song::lessons::LessonsPlugin;
use harmonicon_song::song::SongPlugin;
use harmonicon_ui::music_score::MusicScorePlugin;
use harmonicon_ui::spectrogram::SpectrogramPlugin;

/// A raw debug binary lives under `target/debug`, while the assets remain in
/// the checkout root. Cargo supplies `CARGO_MANIFEST_DIR` to `cargo run`, but
/// it is absent when that binary is launched directly, so Bevy otherwise
/// falls back to looking beside the executable (`target/debug/assets`).
///
/// The packaged macOS app does not need this: its launcher changes into the
/// bundle directory and the bundle contains an adjacent `assets` directory.
#[cfg(all(target_os = "macos", debug_assertions))]
fn configured_asset_plugin() -> AssetPlugin {
    let root = std::env::var_os("BEVY_ASSET_ROOT")
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let root = root.canonicalize().unwrap_or_else(|err| {
        panic!(
            "could not resolve macOS debug asset root '{}': {err}",
            root.display()
        )
    });

    std::env::set_current_dir(&root).unwrap_or_else(|err| {
        panic!(
            "could not switch to macOS debug asset root '{}': {err}",
            root.display()
        )
    });

    AssetPlugin {
        file_path: root.join("assets").to_string_lossy().into_owned(),
        ..default()
    }
}

#[cfg(not(all(target_os = "macos", debug_assertions)))]
fn configured_asset_plugin() -> AssetPlugin {
    AssetPlugin::default()
}

/// Extra, optional asset root: songs the user drops into `~/Harmonicon` are
/// discovered alongside (not instead of) the bundled `assets/` tree, via this
/// "external" source (e.g. `external://songs/Artist/Song/...`). Must be
/// registered before `DefaultPlugins` — `AssetPlugin` builds registered
/// sources when it's added, not after.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn register_external_asset_source(app: &mut App) {
    use bevy::asset::io::{AssetSource, AssetSourceBuilder};

    if let Some(home) = dirs::home_dir() {
        let external_root = home.join("Harmonicon");
        app.register_asset_source(
            "external",
            AssetSourceBuilder::new(AssetSource::get_default_reader(
                external_root.to_string_lossy().into_owned(),
            )),
        );
    }
}

/// No drop folder exists on these targets — a browser has no home directory,
/// and an Android app can only reach its own sandbox — so the scan functions
/// there are manifest-backed and never produce an `external://` path.
///
/// Registering one anyway would be worse than useless on Android
/// specifically: `AssetSource::get_default_reader` yields the *APK* asset
/// reader there, not a filesystem one, so the source would silently resolve
/// against the bundle rather than the path it was handed.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn register_external_asset_source(_app: &mut App) {}

/// Builds and runs the game. Blocks until the window closes.
pub fn run() {
    let mut app = App::new();

    register_external_asset_source(&mut app);

    app.add_plugins(
        DefaultPlugins
            .set(configured_asset_plugin())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Harmonicon".into(),
                    // Wayland app_id / X11 WM_CLASS, so the desktop file's icon is matched.
                    name: Some(APP_ID.into()),
                    // Web-only fields (see `index.html`); each is a no-op
                    // on native, so no `#[cfg]` is needed to set them here.
                    canvas: Some("#bevy-canvas".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin {
                // bevy_render warns about its own internal shadow-view cameras
                // in 0.19 RC. A plain `cargo run` keeps the console quiet
                // below `warn`; a `trace_tracy` build needs the default
                // filter level to stay at `info` instead — Bevy's ECS/render
                // spans (and this crate's own manual ones, see
                // `audio_system::audio_input`/`pitch_detect`) are emitted at
                // `info`, and a span below the configured level is dropped
                // before it reaches Tracy or any other backend at all
                // (contributing/src/profiling.md), no matter how the trace feature is
                // wired up.
                #[cfg(not(feature = "trace_tracy"))]
                filter: "warn,bevy_render::camera=error".into(),
                #[cfg(feature = "trace_tracy")]
                filter: "bevy_render::camera=error,wgpu=error,naga=warn".into(),
                ..default()
            })
            // Linear filtering on all three stages (mag, min, mipmap) so that
            // assets scaled down from their source resolution stay sharp instead
            // of aliasing or blurring without mip interpolation.
            .set(ImagePlugin {
                default_sampler: ImageSamplerDescriptor::linear(),
            }),
    )
    .add_plugins((
        AssetsManagementPlugin,
        ThemePlugin,
        LessonsPlugin,
        LocalizationPlugin,
        SongPlugin,
        MenuPlugin,
        GameplayPlugin,
        JamPlugin,
        SpectrogramPlugin,
        SettingsPlugin,
        ProfilePlugin,
        MusicScorePlugin,
        ResponsivePlugin,
    ))
    .add_plugins((
        harmonicon_ui::dialogs::algo_picker::AlgoPickerPlugin,
        harmonicon_ui::dialogs::button::ButtonVisualsPlugin,
        harmonicon_ui::dialogs::checkbox::CheckboxPlugin,
        harmonicon_ui::dialogs::combobox::ComboboxPlugin,
        harmonicon_ui::dialogs::confirm_dialog::ConfirmDialogPlugin,
        harmonicon_ui::dialogs::file_dialog::FileDialogsPlugin,
        harmonicon_ui::dialogs::font_fallback::FontFallbackPlugin,
        harmonicon_ui::dialogs::keyboard_nav::KeyboardNavPlugin,
        harmonicon_ui::dialogs::scroll_area::ScrollAreaPlugin,
        harmonicon_ui::dialogs::tab_bar::TabBarPlugin,
        harmonicon_ui::dialogs::text_input::TextInputPlugin,
        harmonicon_ui::dialogs::tooltip::TooltipPlugin,
    ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_capture::DevCapturePlugin);

    app.add_message::<PitchEvent>()
        .init_resource::<AudioFrame>()
        .init_resource::<PitchRange>()
        .add_systems(
            Startup,
            (
                spawn_camera,
                // Must run after settings are loaded from disk, or the mic
                // would always start on the default device, ignoring a saved
                // `input_device` preference.
                audio_input::start_capture
                    .after(harmonicon_platform::settings::apply_loaded_settings),
            ),
        )
        // Hold on the Startup state until the locale folder has loaded, so the
        // menu's first frame shows translated labels rather than raw Fluent keys.
        .add_systems(
            Update,
            enter_menu_when_localized
                .run_if(in_state(AppState::Startup))
                .run_if(harmonicon_platform::localization::localization_ready),
        )
        .add_plugins(harmonicon_audio::AudioPipelinePlugin)
        .add_systems(
            Update,
            pipeline::log_pitches.run_if(in_state(AppState::Playing)),
        )
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Camera2d (main)")));
}

fn enter_menu_when_localized(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Menu);
}
