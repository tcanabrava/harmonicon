mod audio_input;
mod gameplay;
mod menu;
mod pitch_detect;
mod song;

use bevy::prelude::*;
use gameplay::GameplayPlugin;
use menu::{AppState, MenuPlugin};
use pitch_detect::PitchEvent;
use song::SongPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Harmonicon".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((SongPlugin, MenuPlugin, GameplayPlugin))
        .add_message::<PitchEvent>()
        .add_systems(Startup, spawn_camera)
        .add_systems(OnEnter(AppState::Playing), setup_audio)
        .add_systems(
            Update,
            (process_audio, print_pitches)
                .chain()
                .run_if(in_state(AppState::Playing)),
        )
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup_audio(world: &mut World) {
    match audio_input::create_audio_capture() {
        Ok((stream, capture)) => {
            info!("Audio capture started at {} Hz", capture.sample_rate);
            world.insert_non_send_resource(stream);
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
    mut fft: Local<pitch_detect::FftState>,
) {
    let Some(capture) = capture else { return };
    while let Ok(samples) = capture.receiver.try_recv() {
        let pitches = pitch_detect::detect_pitches(&samples, capture.sample_rate, &mut fft);
        writer.write(PitchEvent(pitches));
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
