// SPDX-License-Identifier: MIT

use bevy::log::{error, info};
use bevy::prelude::{Res, ResMut, Resource, World};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_channel::{Receiver, Sender, bounded};

use crate::AudioSettings;

// The analysis window itself belongs with the analysis: re-exported from
// `harmonicon-dsp` so an offline run (`harmonicon-bench`) chunks audio
// exactly as the live mic path does, without depending on cpal or Bevy to
// learn the numbers.
pub use harmonicon_dsp::{CHUNK_SIZE, HOP_SIZE};

/// Fixed buffer inventory shared by capture and its consumer. Exhaustion
/// drops audio; callback code never grows the inventory.
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
    /// `ChunkWriter`. Calling into the allocator from that callback risks
    /// blocking on a lock held by a lower-priority thread, causing an
    /// audible dropout ("xrun") on weaker machines.
    pub free_sender: Sender<Vec<f32>>,
    pub sample_rate: u32,
    /// The actually-connected device's name — may differ from the requested
    /// one if it wasn't found and capture fell back to the system default.
    pub device_name: String,
    /// Stream errors reported by cpal *after* the stream opened successfully
    /// — above all, the device being unplugged mid-session.
    ///
    /// cpal delivers these on its own thread, so they can't touch the `World`
    /// directly; [`detect_stream_failure`] drains this every frame and turns
    /// the first one into [`MicStatus::Failed`]. Before this channel existed
    /// the callback only did an `eprintln!`, which meant a mic unplugged
    /// during a song left `MicStatus` reading `Connected` forever: the
    /// Options banner stayed hidden, the in-play warning never appeared, and
    /// the game just silently stopped scoring.
    pub errors: Receiver<String>,
}

/// Whether the microphone capture stream is currently up.
///
/// Written from two places, which between them cover both ways a mic can be
/// unusable: [`start_capture`] (startup, and the Options Retry button) for a
/// stream that won't open, and [`detect_stream_failure`] for one that opened
/// and later died. Read by the Options banner and the in-play warning
/// overlay, so neither has to guess whether the game can hear anything.
#[derive(Resource, Clone, PartialEq, Debug)]
pub enum MicStatus {
    Connected {
        device_name: String,
    },
    Failed {
        reason: String,
    },
    /// Android's `RECORD_AUDIO` and iOS's `NSMicrophoneUsageDescription`
    /// both require an explicit runtime permission prompt before capture
    /// can succeed — this is somewhere for that state to land distinct
    /// from a hard failure, so the Options page can show "waiting on
    /// permission" rather than a generic error. Set by `start_capture` when
    /// `permission::microphone_granted()` says no — which off Android is
    /// never, since there the check always answers "granted".
    AwaitingPermission,
}

impl MicStatus {
    /// Whether capture is actually running. Anything else means the game is
    /// deaf, and every screen that cares says so — the Options banner and
    /// the in-play warning overlay both branch on exactly this, so "what
    /// counts as working" has one definition rather than one per screen.
    pub fn is_connected(&self) -> bool {
        matches!(self, MicStatus::Connected { .. })
    }
}

/// Names of every input device the current host reports, in host-listed
/// order. Empty (rather than an error) if enumeration itself fails — callers
/// treat "no devices" and "enumeration failed" the same way.
pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices
            .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
            .collect(),
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
    // On a platform with a runtime permission model, opening the stream
    // before the user has granted it fails in a way indistinguishable from a
    // broken device. Ask first, and park in `AwaitingPermission` until the
    // answer comes back (see `retry_capture_when_permission_granted`).
    if !crate::permission::microphone_granted() {
        crate::permission::request_microphone();
        world.insert_resource(MicStatus::AwaitingPermission);
        return;
    }

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

/// Turns a cpal stream error into [`MicStatus::Failed`], so a microphone
/// unplugged *mid-session* is reported the same way one that never opened is.
///
/// Startup failure was always handled — `start_capture` sets `Failed` when
/// the stream won't open. A device that dies *after* opening is a different
/// path entirely: cpal reports it through the stream's error callback on its
/// own thread, and that callback used to only `eprintln!`. So `MicStatus`
/// stayed `Connected`, the Options banner stayed hidden, the in-play warning
/// never fired, and the player just watched their notes stop scoring.
///
/// Deliberately does **not** try to reopen the stream. An automatic retry
/// would need a backoff (cpal errors arrive in bursts), and on a machine with
/// no working input at all it would probe the audio backend forever. Recovery
/// stays the explicit Retry button on the Options page, which already exists
/// and already re-runs `start_capture`.
pub fn detect_stream_failure(
    capture: Option<Res<AudioCapture>>,
    status: Option<ResMut<MicStatus>>,
) {
    let (Some(capture), Some(mut status)) = (capture, status) else {
        return;
    };
    // Drain rather than take one: a dying device reports repeatedly, and
    // anything left queued would re-trigger this on later frames.
    let mut first: Option<String> = None;
    while let Ok(reason) = capture.errors.try_recv() {
        first.get_or_insert(reason);
    }
    let Some(reason) = first else {
        return;
    };
    // Only downgrade from `Connected`. A stream error while parked in
    // `AwaitingPermission` is the *denial* showing up as an I/O failure on
    // Android — "grant the permission" stays the more useful thing to say —
    // and overwriting an existing `Failed` would just churn its reason.
    if status.is_connected() {
        error!("Audio stream failed: {reason}");
        *status = MicStatus::Failed { reason };
    }
}

/// Polls for the permission dialog being answered, then starts capture for
/// real.
///
/// Only does anything while [`MicStatus::AwaitingPermission`] is the current
/// status, which off Android is never — [`permission::microphone_granted`]
/// answers `true` there, so `start_capture` never parks and this system
/// returns on its first line forever.
///
/// A poll rather than a callback because the permission result is delivered
/// to the Java activity, not to us; see [`permission::request_microphone`].
/// If the user *denies* it, this simply keeps polling and the status stays
/// `AwaitingPermission` — which is the truth, and what the Options page
/// already renders a banner for.
pub fn retry_capture_when_permission_granted(world: &mut World) {
    if !matches!(
        world.get_resource::<MicStatus>(),
        Some(MicStatus::AwaitingPermission)
    ) {
        return;
    }
    if !crate::permission::microphone_granted() {
        return;
    }
    start_capture(world);
}

/// Opens capture on `device_name` (falling back to the system default if
/// `None` or not found among the current input devices).
pub fn create_audio_capture(
    device_name: Option<&str>,
) -> Result<(AudioStream, AudioCapture), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = device_name
        .and_then(|name| {
            host.input_devices().ok()?.find(|d| {
                d.description()
                    .map(|desc| desc.name() == name)
                    .unwrap_or(false)
            })
        })
        .or_else(|| host.default_input_device())
        .ok_or("no input device available")?;
    let device_name = device
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate();
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();
    let stream_config: StreamConfig = config.into();

    println!("Input device : {device_name}");
    println!(
        "Sample rate  : {} Hz  |  channels: {}  |  format: {:?}",
        sample_rate, channels, sample_format
    );

    let (tx, rx) = bounded::<Vec<f32>>(64);

    // Small and bounded: only the first error actually matters (they arrive
    // in bursts once a device dies), and a full channel must never block
    // cpal's callback thread — `try_send` drops the surplus.
    let (err_tx, err_rx) = bounded::<String>(4);

    // Pre-warm the recycling pool so even the first few chunks don't need to
    // allocate — see `AudioCapture::free_sender` / `ChunkWriter`.
    let (free_tx, free_rx) = bounded::<Vec<f32>>(POOL_SIZE);
    for _ in 0..POOL_SIZE {
        let _ = free_tx.try_send(Vec::with_capacity(CHUNK_SIZE));
    }

    let stream = match sample_format {
        SampleFormat::F32 => {
            build_stream::<f32>(&device, &stream_config, channels, tx, free_rx, err_tx)?
        }
        SampleFormat::I16 => {
            build_stream::<i16>(&device, &stream_config, channels, tx, free_rx, err_tx)?
        }
        SampleFormat::I32 => {
            build_stream::<i32>(&device, &stream_config, channels, tx, free_rx, err_tx)?
        }
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
            errors: err_rx,
        },
    ))
}

/// All formats share the same fixed-storage downmix and overlap handling.
fn build_stream<T: cpal::SizedSample>(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    tx: Sender<Vec<f32>>,
    free_rx: Receiver<Vec<f32>>,
    errors: Sender<String>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    f32: cpal::FromSample<T>,
{
    let mut chunks = ChunkWriter::new();
    device.build_input_stream(
        config,
        move |data: &[T], _| chunks.push(data, channels, &tx, &free_rx),
        move |e| {
            let _ = errors.try_send(e.to_string());
        },
        None,
    )
}

/// Allocated before capture starts. A full channel retains its rejected
/// buffer for the next hop; pool exhaustion drops a hop without allocating.
/// Scratch storage is independent of the backend's callback block size.
struct ChunkWriter {
    samples: Box<[f32; CHUNK_SIZE]>,
    len: usize,
    pending: Option<Vec<f32>>,
}

impl ChunkWriter {
    fn new() -> Self {
        Self {
            samples: Box::new([0.0; CHUNK_SIZE]),
            len: 0,
            pending: None,
        }
    }

    fn push<T: cpal::Sample>(
        &mut self,
        data: &[T],
        channels: usize,
        tx: &Sender<Vec<f32>>,
        free: &Receiver<Vec<f32>>,
    ) where
        f32: cpal::FromSample<T>,
    {
        if channels == 0 {
            return;
        }
        for frame in data.chunks_exact(channels) {
            self.samples[self.len] =
                frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / channels as f32;
            self.len += 1;
            if self.len != CHUNK_SIZE {
                continue;
            }
            if let Some(mut chunk) = self.pending.take().or_else(|| free.try_recv().ok()) {
                // Every production buffer comes from the preallocated pool;
                // the consumer only recycles buffers with this capacity.
                debug_assert!(chunk.capacity() >= CHUNK_SIZE);
                chunk.clear();
                chunk.extend_from_slice(&self.samples[..]);
                if let Err(error) = tx.try_send(chunk) {
                    self.pending = Some(error.into_inner());
                }
            }
            self.samples.copy_within(HOP_SIZE.., 0);
            self.len = CHUNK_SIZE - HOP_SIZE;
        }
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

    struct CountingAllocator;
    thread_local! {
        static ALLOCATIONS: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
    }
    // Only this crate's unit-test binary installs the wrapper. Production
    // capture has no instrumentation or custom allocator.
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn count_allocation(deallocation: bool) {
        let _ = ALLOCATIONS.try_with(|counter| {
            if let Some((alloc, free)) = counter.get() {
                counter.set(Some((
                    alloc + usize::from(!deallocation),
                    free + usize::from(deallocation),
                )));
            }
        });
    }

    // SAFETY: every operation forwards the original pointer/layout unchanged
    // to System; thread-local counters neither allocate nor access that memory.
    unsafe impl std::alloc::GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            count_allocation(false);
            unsafe { std::alloc::System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
            count_allocation(false);
            unsafe { std::alloc::System.alloc_zeroed(layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
            count_allocation(false);
            unsafe { std::alloc::System.realloc(ptr, layout, size) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
            count_allocation(true);
            unsafe { std::alloc::System.dealloc(ptr, layout) }
        }
    }

    #[test]
    fn capture_sample_callback_neither_allocates_nor_frees_under_load() {
        let (tx, _rx) = bounded(1);
        let (free_tx, free_rx) = bounded(2);
        for _ in 0..2 {
            free_tx.send(Vec::with_capacity(CHUNK_SIZE)).unwrap();
        }
        let mut writer = ChunkWriter::new();
        let data = vec![0i16; CHUNK_SIZE * 40];
        ALLOCATIONS.with(|counter| counter.set(Some((0, 0))));
        writer.push(&data, 2, &tx, &free_rx);
        writer.push(&data, 2, &tx, &free_rx); // output full, reuse rejected buffer
        let counts = ALLOCATIONS.with(|counter| counter.replace(None).unwrap());
        assert_eq!(counts, (0, 0));
        let (empty_tx, empty_rx) = bounded(1);
        ALLOCATIONS.with(|counter| counter.set(Some((0, 0))));
        // No pool buffers and no rejected buffer: exhaust the inventory.
        writer.pending.as_mut().unwrap().clear();
        // Keep pending owned until outside the measured interval.
        let retained = writer.pending.take();
        writer.push(&data, 2, &tx, &empty_rx);
        let counts = ALLOCATIONS.with(|counter| counter.replace(None).unwrap());
        assert_eq!(counts, (0, 0));
        drop((retained, empty_tx));
    }

    #[test]
    fn chunks_downmix_and_preserve_overlap_across_callback_sizes() {
        let (tx, rx) = bounded(8);
        let (free_tx, free_rx) = bounded(8);
        for _ in 0..8 {
            free_tx.send(Vec::with_capacity(CHUNK_SIZE)).unwrap();
        }
        let mut writer = ChunkWriter::new();
        let data: Vec<f32> = (0..CHUNK_SIZE + HOP_SIZE)
            .flat_map(|i| [i as f32, i as f32 + 2.0])
            .collect();
        for part in data.chunks(254) {
            writer.push(part, 2, &tx, &free_rx);
        }
        let first = rx.try_recv().unwrap();
        let second = rx.try_recv().unwrap();
        assert_eq!(
            first,
            (0..CHUNK_SIZE).map(|i| i as f32 + 1.0).collect::<Vec<_>>()
        );
        assert_eq!(
            second,
            (HOP_SIZE..CHUNK_SIZE + HOP_SIZE)
                .map(|i| i as f32 + 1.0)
                .collect::<Vec<_>>()
        );
        assert!(rx.is_empty());
    }

    #[test]
    fn full_channel_retains_buffer_and_empty_pool_drops_without_allocation() {
        let (tx, rx) = bounded(1);
        let (free_tx, free_rx) = bounded(2);
        let buffer = Vec::with_capacity(CHUNK_SIZE);
        let pointer = buffer.as_ptr();
        free_tx.send(buffer).unwrap();
        tx.send(Vec::with_capacity(CHUNK_SIZE)).unwrap();
        let mut writer = ChunkWriter::new();
        let data = vec![0.5; CHUNK_SIZE * 8];
        writer.push(&data, 1, &tx, &free_rx);
        assert_eq!(writer.pending.as_ref().unwrap().as_ptr(), pointer);
        rx.try_recv().unwrap();
        writer.push(&data[..HOP_SIZE], 1, &tx, &free_rx);
        let received = rx.try_recv().unwrap();
        assert_eq!(received.as_ptr(), pointer);
        assert!(writer.pending.is_none());
        writer.push(&data, 1, &tx, &free_rx);
        assert!(rx.is_empty()); // all buffers in flight: no fallback allocation
        free_tx.send(received).unwrap();
        writer.push(&data[..HOP_SIZE], 1, &tx, &free_rx);
        assert_eq!(rx.try_recv().unwrap().as_ptr(), pointer);
    }

    #[test]
    fn integer_capture_uses_the_same_normalized_pipeline() {
        let (tx, rx) = bounded(1);
        let (free_tx, free_rx) = bounded(1);
        free_tx.send(Vec::with_capacity(CHUNK_SIZE)).unwrap();
        ChunkWriter::new().push(&vec![i16::MIN; CHUNK_SIZE], 1, &tx, &free_rx);
        assert_eq!(rx.try_recv().unwrap(), vec![-1.0; CHUNK_SIZE]);
        free_tx.send(Vec::with_capacity(CHUNK_SIZE)).unwrap();
        ChunkWriter::new().push(&vec![i32::MIN; CHUNK_SIZE], 1, &tx, &free_rx);
        assert_eq!(rx.try_recv().unwrap(), vec![-1.0; CHUNK_SIZE]);
    }

    /// An `AudioCapture` whose only live wire is the error channel — the
    /// sample-path fields are real but unused here, since
    /// `detect_stream_failure` never touches them.
    fn capture_reporting(errors: Receiver<String>) -> AudioCapture {
        let (tx, rx) = bounded::<Vec<f32>>(1);
        AudioCapture {
            receiver: rx,
            free_sender: tx,
            sample_rate: 44_100,
            device_name: "Test Device".into(),
            errors,
        }
    }

    fn app_with(status: MicStatus, errors: Receiver<String>) -> bevy::prelude::App {
        let mut app = bevy::prelude::App::new();
        app.insert_resource(capture_reporting(errors))
            .insert_resource(status)
            .add_systems(bevy::prelude::Update, detect_stream_failure);
        app
    }

    #[test]
    fn a_device_dying_mid_session_downgrades_connected_to_failed() {
        // The whole point: cpal reports this *after* the stream opened, on
        // its own thread. Before the error channel existed the status stayed
        // Connected and the player just watched their notes stop scoring.
        let (tx, rx) = bounded::<String>(4);
        tx.try_send("device disconnected".into()).unwrap();
        let mut app = app_with(
            MicStatus::Connected {
                device_name: "Test Device".into(),
            },
            rx,
        );
        app.update();
        assert_eq!(
            *app.world().resource::<MicStatus>(),
            MicStatus::Failed {
                reason: "device disconnected".into()
            }
        );
    }

    #[test]
    fn a_healthy_stream_leaves_the_status_alone() {
        let (_tx, rx) = bounded::<String>(4);
        let connected = MicStatus::Connected {
            device_name: "Test Device".into(),
        };
        let mut app = app_with(connected.clone(), rx);
        app.update();
        assert_eq!(*app.world().resource::<MicStatus>(), connected);
    }

    #[test]
    fn a_stream_error_does_not_overwrite_awaiting_permission() {
        // On Android a denied RECORD_AUDIO surfaces as an I/O-shaped stream
        // error; "grant the permission" is the more useful thing to keep
        // saying than a raw device message.
        let (tx, rx) = bounded::<String>(4);
        tx.try_send("permission denied".into()).unwrap();
        let mut app = app_with(MicStatus::AwaitingPermission, rx);
        app.update();
        assert_eq!(
            *app.world().resource::<MicStatus>(),
            MicStatus::AwaitingPermission
        );
    }

    #[test]
    fn a_burst_of_errors_is_drained_so_it_only_reports_once() {
        // A dying device reports repeatedly. Anything left queued would
        // re-trigger on later frames and churn the reason string.
        let (tx, rx) = bounded::<String>(4);
        for _ in 0..3 {
            tx.try_send("device disconnected".into()).unwrap();
        }
        let mut app = app_with(
            MicStatus::Connected {
                device_name: "Test Device".into(),
            },
            rx,
        );
        app.update();
        assert!(tx.is_empty(), "every queued error should have been drained");
    }
}
