// SPDX-License-Identifier: MIT

use bevy::asset::io::{AssetSource, AssetSourceBuilder};
use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;

/// Reverse-DNS app id. On Wayland the icon comes from a matching desktop file
/// (`<APP_ID>.desktop`); this sets the window's app_id so the compositor can find
/// it. On X11/Windows/macOS the pixel icon set in `set_window_icon` is used.
const APP_ID: &str = "io.github.tcanabrava.Harmonicon";

use harmonicon::assets_management::AssetsManagementPlugin;
use harmonicon::audio_system::pitch_detect::{AudioFrame, PitchRange};
use harmonicon::audio_system::{audio_input, pitch_detect, pitch_detect::PitchEvent};
use harmonicon::dialogs::ui_scale::change_scaling;
use harmonicon::gameplay::GameplayPlugin;
use harmonicon::localization::LocalizationPlugin;
use harmonicon::menu::{AppState, MenuPlugin};
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
        LocalizationPlugin,
        SongPlugin,
        MenuPlugin,
        GameplayPlugin,
        SpectrogramPlugin,
        SettingsPlugin,
        harmonicon::dialogs::algo_picker::AlgoPickerPlugin,
        harmonicon::dialogs::combobox::ComboboxPlugin,
        harmonicon::dialogs::file_dialog::FileDialogsPlugin,
        harmonicon::dialogs::font_fallback::FontFallbackPlugin,
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
        .add_systems(Update, process_audio)
        .add_systems(Update, print_pitches.run_if(in_state(AppState::Playing)))
        .add_systems(Update, change_scaling)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Camera2d (main)")));
}

fn enter_menu_when_localized(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Menu);
}

fn process_audio(
    capture: Option<Res<audio_input::AudioCapture>>,
    settings: Res<harmonicon::settings::AudioSettings>,
    range: Res<PitchRange>,
    mut writer: MessageWriter<PitchEvent>,
    mut frame: ResMut<AudioFrame>,
    mut fft: Local<pitch_detect::FftState>,
) {
    let Some(capture) = capture else { return };
    while let Ok(samples) = capture.receiver.try_recv() {
        // One FFT per chunk for the spectrum; pitches use the chosen algorithm.
        let analysis = pitch_detect::analyze(
            &samples,
            capture.sample_rate,
            &mut fft,
            settings.pitch_algorithm,
            *range,
        );
        writer.write(PitchEvent(analysis.pitches));
        // Publish the frame so visualizers reuse this FFT (freq) or the raw
        // waveform (time) without re-analysing.
        frame.magnitudes = analysis.magnitudes;
        frame.freq_res = analysis.freq_res;
        frame.samples = samples;
    }
}

fn print_pitches(mut reader: MessageReader<PitchEvent>, mut last: Local<Vec<String>>) {
    for event in reader.read() {
        let current: Vec<String> = event
            .0
            .iter()
            .map(|p| format!("{}{} ({:.1}Hz)", p.note, p.octave, p.frequency))
            .collect();

        if current == *last {
            continue;
        }

        if current.is_empty() {
            println!("---");
        } else {
            println!("Pitches: {}", current.join("  |  "));
        }
        *last = current;
    }
}
