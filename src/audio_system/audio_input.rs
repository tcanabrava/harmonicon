// SPDX-License-Identifier: MIT

use bevy::log::{error, info, info_span};
use bevy::prelude::{Resource, World};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_channel::{Receiver, Sender, bounded};

use crate::settings::AudioSettings;

pub const CHUNK_SIZE: usize = 4096;

/// How many chunk buffers circulate between the real-time audio callback and
/// the consumer (`process_audio`). Comfortably more than one in flight at a
/// time — the consumer drains far faster than chunks arrive (one FFT per
/// ~46ms chunk) — so recycling normally never runs dry; if it ever does
/// (startup, or the consumer briefly falling behind), `push_chunks` falls
/// back to allocating a fresh buffer rather than dropping audio.
const POOL_SIZE: usize = 8;

// NonSend resource — keeps the cpal stream alive for the duration of the app.
#[allow(dead_code)]
pub struct AudioStream(pub cpal::Stream);

#[derive(Resource)]
pub struct AudioCapture {
    pub receiver: Receiver<Vec<f32>>,
    /// Hand a chunk buffer back here once you're done with it (e.g. when
    /// overwriting `AudioFrame::samples` with a newer chunk) so the
    /// real-time callback can reuse it instead of allocating — see
    /// `push_chunks`. Calling into the allocator from that callback risks
    /// blocking on a lock held by a lower-priority thread, causing an
    /// audible dropout ("xrun") on weaker machines.
    pub free_sender: Sender<Vec<f32>>,
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
    // Skip enumeration entirely for the common case (no preference set) — on
    // Linux, listing input devices makes cpal probe every ALSA/JACK backend,
    // which is noisy and pointless when we're just taking the default anyway.
    let device_name = if wanted.is_empty() {
        None
    } else {
        resolve_device_name(&input_device_names(), &wanted)
    };

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

    // Pre-warm the recycling pool so even the first few chunks don't need to
    // allocate — see `AudioCapture::free_sender` / `push_chunks`.
    let (free_tx, free_rx) = bounded::<Vec<f32>>(POOL_SIZE);
    for _ in 0..POOL_SIZE {
        let _ = free_tx.try_send(Vec::with_capacity(CHUNK_SIZE));
    }

    let stream = match sample_format {
        SampleFormat::F32 => build_stream_f32(&device, &stream_config, channels, tx, free_rx)?,
        SampleFormat::I16 => build_stream_i16(&device, &stream_config, channels, tx, free_rx)?,
        SampleFormat::I32 => build_stream_i32(&device, &stream_config, channels, tx, free_rx)?,
        fmt => return Err(format!("unsupported sample format: {fmt:?}").into()),
    };

    stream.play()?;

    Ok((
        AudioStream(stream),
        AudioCapture {
            receiver: rx,
            free_sender: free_tx,
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
    free_rx: Receiver<Vec<f32>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut buf: Vec<f32> = Vec::with_capacity(CHUNK_SIZE);
    let mut mono: Vec<f32> = Vec::with_capacity(CHUNK_SIZE / 2);
    device.build_input_stream(
        config,
        move |data: &[f32], _| push_chunks(&mut buf, &mut mono, data, channels, &tx, &free_rx),
        |e| eprintln!("audio stream error: {e}"),
        None,
    )
}

fn build_stream_i16(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    tx: Sender<Vec<f32>>,
    free_rx: Receiver<Vec<f32>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut buf: Vec<f32> = Vec::with_capacity(CHUNK_SIZE);
    let mut mono: Vec<f32> = Vec::with_capacity(CHUNK_SIZE / 2);
    let mut converted: Vec<f32> = Vec::with_capacity(CHUNK_SIZE / 2);
    device.build_input_stream(
        config,
        move |data: &[i16], _| {
            converted.clear();
            converted.extend(data.iter().map(|&s| s as f32 / 32_768.0));
            push_chunks(&mut buf, &mut mono, &converted, channels, &tx, &free_rx);
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
    free_rx: Receiver<Vec<f32>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut buf: Vec<f32> = Vec::with_capacity(CHUNK_SIZE);
    let mut mono: Vec<f32> = Vec::with_capacity(CHUNK_SIZE / 2);
    let mut converted: Vec<f32> = Vec::with_capacity(CHUNK_SIZE / 2);
    device.build_input_stream(
        config,
        move |data: &[i32], _| {
            converted.clear();
            converted.extend(data.iter().map(|&s| s as f32 / 2_147_483_648.0));
            push_chunks(&mut buf, &mut mono, &converted, channels, &tx, &free_rx);
        },
        |e| eprintln!("audio stream error: {e}"),
        None,
    )
}

/// Downmixes multichannel interleaved frames to mono into the reusable
/// `mono` scratch buffer, accumulates into `buf`, and emits CHUNK_SIZE
/// blocks with 50% overlap. Every buffer here (`buf`, `mono`, and the chunk
/// handed to `tx`, drawn from `free_rx`) is reused across calls rather than
/// freshly allocated, since this runs on the real-time audio callback
/// thread — calling into the allocator there risks blocking on a lock held
/// by a lower-priority thread and causing an audible dropout.
fn push_chunks(
    buf: &mut Vec<f32>,
    mono: &mut Vec<f32>,
    data: &[f32],
    channels: usize,
    tx: &Sender<Vec<f32>>,
    free_rx: &Receiver<Vec<f32>>,
) {
    // This runs on cpal's real-time callback thread, invisible to Bevy's own
    // per-system spans (those only wrap systems the ECS schedule calls) — a
    // manual span here is the only way Tracy shows this thread's activity at
    // all, which matters since it's the one place an allocator stall would
    // cause an audible dropout rather than just a dropped frame.
    let _span = info_span!("push_chunks", frames = data.len()).entered();
    mono.clear();
    if channels == 1 {
        mono.extend_from_slice(data);
    } else {
        mono.extend(
            data.chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
    }
    buf.extend_from_slice(mono);
    while buf.len() >= CHUNK_SIZE {
        // Reuse a buffer the consumer already handed back if one's
        // available; only allocate as a last resort (pool momentarily
        // empty), so steady-state operation never touches the allocator.
        let mut chunk = free_rx
            .try_recv()
            .unwrap_or_else(|_| Vec::with_capacity(CHUNK_SIZE));
        chunk.clear();
        chunk.extend_from_slice(&buf[..CHUNK_SIZE]);
        let _ = tx.try_send(chunk);
        buf.drain(..CHUNK_SIZE / 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_device_name ──────────────────────────────────────────────────

    #[test]
    fn empty_preference_means_use_the_default() {
        assert_eq!(resolve_device_name(&["Mic A".to_string()], ""), None);
    }

    #[test]
    fn finds_a_currently_available_match() {
        let available = vec!["Mic A".to_string(), "Mic B".to_string()];
        assert_eq!(
            resolve_device_name(&available, "Mic B"),
            Some("Mic B".to_string())
        );
    }

    #[test]
    fn falls_back_to_default_when_the_saved_device_is_unplugged() {
        let available = vec!["Mic A".to_string()];
        assert_eq!(resolve_device_name(&available, "USB Mic (unplugged)"), None);
    }

    // ── push_chunks ──────────────────────────────────────────────────────────

    #[test]
    fn emits_a_full_chunk_and_keeps_the_overlap_tail() {
        let (tx, rx) = bounded::<Vec<f32>>(4);
        let (_free_tx, free_rx) = bounded::<Vec<f32>>(4); // empty pool: falls back to alloc
        let mut buf = Vec::new();
        let mut mono = Vec::new();
        let data: Vec<f32> = (0..CHUNK_SIZE).map(|i| i as f32).collect();

        push_chunks(&mut buf, &mut mono, &data, 1, &tx, &free_rx);

        let chunk = rx.try_recv().expect("one chunk should have been emitted");
        assert_eq!(chunk, data);
        // 50% overlap: the back half stays buffered for the next call.
        assert_eq!(buf, &data[CHUNK_SIZE / 2..]);
        assert!(rx.try_recv().is_err(), "only one chunk should have emitted");
    }

    #[test]
    fn downmixes_multichannel_frames_by_averaging() {
        let (tx, rx) = bounded::<Vec<f32>>(4);
        let (_free_tx, free_rx) = bounded::<Vec<f32>>(4);
        let mut buf = Vec::new();
        let mut mono = Vec::new();
        // Two channels interleaved: (1,3) -> 2.0, (2,4) -> 3.0.
        let data = vec![1.0, 3.0, 2.0, 4.0];

        push_chunks(&mut buf, &mut mono, &data, 2, &tx, &free_rx);

        assert_eq!(buf, vec![2.0, 3.0]);
        assert!(
            rx.try_recv().is_err(),
            "not enough samples yet for a full chunk"
        );
    }

    #[test]
    fn reuses_a_recycled_buffer_instead_of_allocating() {
        let (tx, rx) = bounded::<Vec<f32>>(4);
        let (free_tx, free_rx) = bounded::<Vec<f32>>(4);

        let recycled: Vec<f32> = Vec::with_capacity(CHUNK_SIZE);
        let recycled_ptr = recycled.as_ptr();
        free_tx.try_send(recycled).unwrap();

        let mut buf = Vec::new();
        let mut mono = Vec::new();
        let data = vec![0.0f32; CHUNK_SIZE];

        push_chunks(&mut buf, &mut mono, &data, 1, &tx, &free_rx);

        let chunk = rx.try_recv().expect("chunk emitted");
        assert_eq!(
            chunk.as_ptr(),
            recycled_ptr,
            "should reuse the pooled allocation instead of a fresh one"
        );
    }
}
