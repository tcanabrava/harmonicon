// SPDX-License-Identifier: MIT

use bevy::log::{error, info};
use bevy::prelude::{Resource, World};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_channel::{Receiver, Sender, bounded};

use crate::settings::AudioSettings;

pub const CHUNK_SIZE: usize = 4096;

// NonSend resource — keeps the cpal stream alive for the duration of the app.
#[allow(dead_code)]
pub struct AudioStream(pub cpal::Stream);

#[derive(Resource)]
pub struct AudioCapture {
    pub receiver: Receiver<Vec<f32>>,
    pub sample_rate: u32,
    /// The actually-connected device's name — may differ from the requested
    /// one if it wasn't found and capture fell back to the system default.
    pub device_name: String,
}

/// Whether the microphone capture stream is currently up. Set by
/// [`start_capture`] (startup and manual retry) so menu UI can show a visible
/// warning instead of the game silently running "deaf" — see TODO.md.
#[derive(Resource, Clone, PartialEq, Debug)]
pub enum MicStatus {
    Connected { device_name: String },
    Failed { reason: String },
}

/// Names of every input device the current host reports, in host-listed
/// order. Empty (rather than an error) if enumeration itself fails — callers
/// treat "no devices" and "enumeration failed" the same way.
pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Which device name to actually look for, given the user's configured
/// preference (`""` means "use the system default"). Returns `None` when
/// `wanted` is empty or doesn't match anything currently plugged in, so the
/// caller falls back to the default device instead of erroring — a saved
/// preference for a since-unplugged device shouldn't brick capture.
fn resolve_device_name(available: &[String], wanted: &str) -> Option<String> {
    if wanted.is_empty() {
        return None;
    }
    available.iter().find(|n| n.as_str() == wanted).cloned()
}

/// (Re)starts the microphone capture stream using `AudioSettings::input_device`
/// (falling back to the system default if that device is empty/unavailable),
/// and records the outcome in [`MicStatus`]. Only needs `&mut World`, so both
/// the startup system and the Options page's "Retry" button / device picker
/// can trigger it directly (the latter via `Commands::queue`).
pub fn start_capture(world: &mut World) {
    let wanted = world.resource::<AudioSettings>().input_device.clone();
    let device_name = resolve_device_name(&input_device_names(), &wanted);

    match create_audio_capture(device_name.as_deref()) {
        Ok((stream, capture)) => {
            info!(
                "Audio capture started at {} Hz on \"{}\"",
                capture.sample_rate, capture.device_name
            );
            world.insert_resource(MicStatus::Connected {
                device_name: capture.device_name.clone(),
            });
            world.insert_non_send(stream);
            world.insert_resource(capture);
        }
        Err(e) => {
            error!("Failed to start audio capture: {e}");
            world.insert_resource(MicStatus::Failed {
                reason: e.to_string(),
            });
        }
    }
}

/// Opens capture on `device_name` (falling back to the system default if
/// `None` or not found among the current input devices).
pub fn create_audio_capture(
    device_name: Option<&str>,
) -> Result<(AudioStream, AudioCapture), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = device_name
        .and_then(|name| {
            host.input_devices()
                .ok()?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        })
        .or_else(|| host.default_input_device())
        .ok_or("no input device available")?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".to_string());

    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();
    let stream_config: StreamConfig = config.into();

    println!("Input device : {device_name}");
    println!(
        "Sample rate  : {} Hz  |  channels: {}  |  format: {:?}",
        sample_rate, channels, sample_format
    );

    let (tx, rx) = bounded::<Vec<f32>>(64);

    let stream = match sample_format {
        SampleFormat::F32 => build_stream_f32(&device, &stream_config, channels, tx)?,
        SampleFormat::I16 => build_stream_i16(&device, &stream_config, channels, tx)?,
        SampleFormat::I32 => build_stream_i32(&device, &stream_config, channels, tx)?,
        fmt => return Err(format!("unsupported sample format: {fmt:?}").into()),
    };

    stream.play()?;

    Ok((
        AudioStream(stream),
        AudioCapture {
            receiver: rx,
            sample_rate,
            device_name,
        },
    ))
}

// ---------------------------------------------------------------------------
// Per-format stream builders — identical logic, only the sample type differs.
// ---------------------------------------------------------------------------

fn build_stream_f32(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut buf: Vec<f32> = Vec::new();
    device.build_input_stream(
        config,
        move |data: &[f32], _| push_chunks(&mut buf, data, channels, &tx),
        |e| eprintln!("audio stream error: {e}"),
        None,
    )
}

fn build_stream_i16(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut buf: Vec<f32> = Vec::new();
    device.build_input_stream(
        config,
        move |data: &[i16], _| {
            let f: Vec<f32> = data.iter().map(|&s| s as f32 / 32_768.0).collect();
            push_chunks(&mut buf, &f, channels, &tx);
        },
        |e| eprintln!("audio stream error: {e}"),
        None,
    )
}

fn build_stream_i32(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut buf: Vec<f32> = Vec::new();
    device.build_input_stream(
        config,
        move |data: &[i32], _| {
            let f: Vec<f32> = data.iter().map(|&s| s as f32 / 2_147_483_648.0).collect();
            push_chunks(&mut buf, &f, channels, &tx);
        },
        |e| eprintln!("audio stream error: {e}"),
        None,
    )
}

// Downmix multichannel interleaved frames to mono, accumulate in `buf`, and
// emit CHUNK_SIZE blocks with 50 % overlap for better time resolution.
fn push_chunks(buf: &mut Vec<f32>, data: &[f32], channels: usize, tx: &Sender<Vec<f32>>) {
    let mono: Vec<f32> = if channels == 1 {
        data.to_vec()
    } else {
        data.chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    buf.extend(mono);
    while buf.len() >= CHUNK_SIZE {
        let _ = tx.try_send(buf[..CHUNK_SIZE].to_vec());
        buf.drain(..CHUNK_SIZE / 2);
    }
}
