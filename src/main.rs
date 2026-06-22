// SPDX-License-Identifier: MIT

use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;

/// Reverse-DNS app id. On Wayland the icon comes from a matching desktop file
/// (`<APP_ID>.desktop`); this sets the window's app_id so the compositor can find
/// it. On X11/Windows/macOS the pixel icon set in `set_window_icon` is used.
const APP_ID: &str = "io.github.tcanabrava.Harmonicon";

use harmonicon::assets_management::AssetsManagementPlugin;
use harmonicon::audio_system::pitch_detect::AudioFrame;
use harmonicon::audio_system::{audio_input, pitch_detect, pitch_detect::PitchEvent};
use harmonicon::gameplay::GameplayPlugin;
use harmonicon::menu::{AppState, MenuPlugin};
use harmonicon::settings::SettingsPlugin;
use harmonicon::song::SongPlugin;
use harmonicon::spectrogram::SpectrogramPlugin;
use harmonicon::theme::ThemePlugin;

fn main() {
    let mut app = App::new();
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
        SongPlugin,
        MenuPlugin,
        GameplayPlugin,
        SpectrogramPlugin,
        SettingsPlugin,
        harmonicon::dialogs::DialogsPlugin,
    ));

    #[cfg(feature = "inspector")]
    app.add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::new());

    app.add_message::<PitchEvent>()
        .init_resource::<AudioFrame>()
        .add_systems(Startup, (spawn_camera, initialize_game, setup_audio))
        .add_systems(Update, process_audio)
        .add_systems(
            Update,
            print_pitches.run_if(in_state(AppState::Playing)),
        )
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Camera2d (main)")));
}

fn initialize_game(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Menu);
}

fn setup_audio(world: &mut World) {
    match audio_input::create_audio_capture() {
        Ok((stream, capture)) => {
            info!("Audio capture started at {} Hz", capture.sample_rate);
            world.insert_non_send(stream);
            world.insert_resource(capture);
        }
        Err(e) => {
            error!("Failed to start audio capture: {e}");
        }
    }
}

fn process_audio(
    capture: Option<Res<audio_input::AudioCapture>>,
    mut writer: MessageWriter<PitchEvent>,
    mut frame: ResMut<AudioFrame>,
    mut fft: Local<pitch_detect::FftState>,
) {
    let Some(capture) = capture else { return };
    while let Ok(samples) = capture.receiver.try_recv() {
        // One FFT per chunk: pitches and the magnitude spectrum come out together.
        let analysis = pitch_detect::analyze(&samples, capture.sample_rate, &mut fft);
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
