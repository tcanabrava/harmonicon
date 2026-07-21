// SPDX-License-Identifier: MIT

use bevy::asset::io::{AssetSource, AssetSourceBuilder};
use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;

/// Reverse-DNS app id. On Wayland the icon comes from a matching desktop file
/// (`<APP_ID>.desktop`); this sets the window's app_id so the compositor can find
/// it. On X11/Windows/macOS the pixel icon set in `set_window_icon` is used.
const APP_ID: &str = "io.github.tcanabrava.Harmonicon";

use harmonicon::app::AppState;
use harmonicon::assets_management::AssetsManagementPlugin;
use harmonicon::audio_system::pitch_detect::{AudioFrame, PitchEvent, PitchRange};
use harmonicon::audio_system::{audio_input, pipeline};
use harmonicon::dialogs::ui_scale::change_scaling;
use harmonicon::gameplay::GameplayPlugin;
use harmonicon::lessons::LessonsPlugin;
use harmonicon::localization::LocalizationPlugin;
use harmonicon::menu::MenuPlugin;
use harmonicon::profile::ProfilePlugin;
use harmonicon::settings::SettingsPlugin;
use harmonicon::song::SongPlugin;
use harmonicon::spectrogram::SpectrogramPlugin;
use harmonicon::theme::ThemePlugin;

fn main() {
    let mut app = App::new();

    // Extra, optional asset root: songs the user drops into ~/Harmonicon are
    // discovered alongside (not instead of) the bundled `assets/` tree, via
    // this "external" source (e.g. `external://songs/Artist/Song/...`). Must
    // be registered before `DefaultPlugins` — `AssetPlugin` builds registered
    // sources when it's added, not after.
    if let Some(home) = dirs::home_dir() {
        let external_root = home.join("Harmonicon");
        app.register_asset_source(
            "external",
            AssetSourceBuilder::new(AssetSource::get_default_reader(
                external_root.to_string_lossy().into_owned(),
            )),
        );
    }

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Harmonicon".into(),
                    // Wayland app_id / X11 WM_CLASS, so the desktop file's icon is matched.
                    name: Some(APP_ID.into()),
                    ..default()
                }),
                ..default()
            })
            // bevy_render warns about its own internal shadow-view cameras in 0.19 RC
            .set(bevy::log::LogPlugin {
                filter: "warn,bevy_render::camera=error".into(),
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
        SpectrogramPlugin,
        SettingsPlugin,
        ProfilePlugin,
    ))
    .add_plugins((
        harmonicon::dialogs::algo_picker::AlgoPickerPlugin,
        harmonicon::dialogs::combobox::ComboboxPlugin,
        harmonicon::dialogs::confirm_dialog::ConfirmDialogPlugin,
        harmonicon::dialogs::file_dialog::FileDialogsPlugin,
        harmonicon::dialogs::font_fallback::FontFallbackPlugin,
        harmonicon::dialogs::scroll_area::ScrollAreaPlugin,
        harmonicon::dialogs::tab_bar::TabBarPlugin,
        harmonicon::dialogs::tooltip::TooltipPlugin,
    ));

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
                audio_input::start_capture.after(harmonicon::settings::apply_loaded_settings),
            ),
        )
        // Hold on the Startup state until the locale folder has loaded, so the
        // menu's first frame shows translated labels rather than raw Fluent keys.
        .add_systems(
            Update,
            enter_menu_when_localized
                .run_if(in_state(AppState::Startup))
                .run_if(harmonicon::localization::localization_ready),
        )
        .add_systems(Update, pipeline::process_audio)
        .add_systems(
            Update,
            pipeline::log_pitches.run_if(in_state(AppState::Playing)),
        )
        .add_systems(Update, change_scaling)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Camera2d (main)")));
}

fn enter_menu_when_localized(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Menu);
}
