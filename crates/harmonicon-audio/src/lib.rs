// SPDX-License-Identifier: MIT

//! Microphone capture and real-time pitch detection.
//!
//! cpal callback -> mono downmix -> overlapped chunks ([`audio_input`]) ->
//! one FFT per chunk ([`pitch_detect`]), plus the offline waveform analysis
//! songs are summarised with ([`waveform`]). Depends only on
//! `harmonicon-core` for the pitch/MIDI maths; it knows nothing about
//! songs, scoring or UI.

pub mod audio_input;
pub mod config;
pub mod permission;
pub mod pipeline;
pub mod pitch_detect;
pub mod waveform;

pub use config::AudioSettings;

/// Capture errors and analysis finish before any Update consumers run.
/// Consumers in PreUpdate can explicitly order themselves after this set.
#[derive(bevy::prelude::SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioPipelineSet;

pub struct AudioPipelinePlugin;

impl bevy::prelude::Plugin for AudioPipelinePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        use bevy::prelude::*;
        app.init_resource::<AudioSettings>()
            .init_resource::<pitch_detect::PitchRange>()
            .init_resource::<pitch_detect::AudioFrame>()
            .add_message::<pitch_detect::PitchEvent>()
            .add_systems(
                PreUpdate,
                (
                    audio_input::retry_capture_when_permission_granted,
                    audio_input::detect_stream_failure,
                    pipeline::process_audio,
                )
                    .chain()
                    .in_set(AudioPipelineSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[derive(Resource, Default)]
    struct Observed {
        messages: usize,
        samples: usize,
    }

    fn consume(
        mut messages: MessageReader<pitch_detect::PitchEvent>,
        frame: Res<pitch_detect::AudioFrame>,
        mut seen: ResMut<Observed>,
    ) {
        seen.messages = messages.read().count();
        seen.samples = frame.samples.len();
    }

    #[test]
    fn update_consumers_see_current_audio_regardless_of_registration_order() {
        for consumer_first in [false, true] {
            let mut app = App::new();
            app.init_resource::<Time<Real>>()
                .init_resource::<Observed>();
            if consumer_first {
                app.add_systems(Update, consume);
            }
            app.add_plugins(AudioPipelinePlugin);
            if !consumer_first {
                app.add_systems(Update, consume);
            }
            app.world_mut()
                .resource_mut::<pitch_detect::AudioFrame>()
                .samples = vec![1.0];
            app.update();
            let seen = app.world().resource::<Observed>();
            assert_eq!(seen.messages, 1);
            assert_eq!(seen.samples, 0);
        }
    }
}
