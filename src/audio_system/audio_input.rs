// SPDX-License-Identifier: MIT

use bevy::prelude::Resource;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_channel::{Receiver, Sender, bounded};

pub const CHUNK_SIZE: usize = 4096;

// NonSend resource — keeps the cpal stream alive for the duration of the app.
#[allow(dead_code)]
pub struct AudioStream(pub cpal::Stream);

#[derive(Resource)]
pub struct AudioCapture {
    pub receiver: Receiver<Vec<f32>>,
    pub sample_rate: u32,
}

pub fn create_audio_capture() -> Result<(AudioStream, AudioCapture), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("no input device available")?;

    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();
    let stream_config: StreamConfig = config.into();

    println!(
        "Input device : {}",
        device.name().unwrap_or_else(|_| "unknown".into())
    );
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
