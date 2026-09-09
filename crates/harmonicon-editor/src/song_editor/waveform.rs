// SPDX-License-Identifier: MIT

//! Displays the chart's referenced music file (`EditorState::music`) as a
//! peak-amplitude waveform in the grid header, aiding alignment of notes
//! and tempo to the actual audio. Reuses `audio_system::waveform`'s
//! existing decoders — the same ones a shipped song's music is analyzed
//! with at asset-load time — rather than duplicating decode logic.
//!
//! Placed against `EditorState`'s variable tempo map (`state::
//! build_tempo_map`) via `song::chart::seconds_to_tick`/`tick_to_seconds`,
//! so a tempo-change point shifts the waveform's alignment exactly like it
//! shifts note ticks on save/load.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};

use super::TICKS_PER_BEAT;
use super::state::EditorState;
use harmonicon_audio::waveform::{WAVEFORM_BUCKETS, analyze_ogg_waveform, analyze_wav_waveform};
use harmonicon_core::chart::{TempoPoint, seconds_to_tick, tick_to_seconds};

/// The chart's music file, decoded into a peak-amplitude waveform — empty
/// (`duration_secs == 0.0`) until a music file is set or if decoding
/// failed. `path` caches the `EditorState::music` value last decoded, so
/// [`sync_music_waveform`] can tell whether a re-decode is needed without
/// depending on `Changed<EditorState>`, which fires far more often.
#[derive(Resource, Default)]
pub(super) struct MusicWaveform {
    path: String,
    pending: Option<Task<(String, Vec<f32>, f64)>>,
    pub(super) buckets: Vec<f32>,
    pub(super) duration_secs: f64,
}

/// Decodes `path`'s audio into a peak-amplitude waveform, dispatching by
/// extension the same way `song::loader` does for a shipped song. Degrades
/// to an all-zero, zero-duration waveform (never errors) for an unreadable
/// file or unsupported extension, same convention as `analyze_ogg_
/// waveform`/`analyze_wav_waveform` themselves.
fn decode_music_waveform(path: &std::path::Path) -> (Vec<f32>, f64) {
    let Ok(bytes) = std::fs::read(path) else {
        return (vec![0.0; WAVEFORM_BUCKETS], 0.0);
    };
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("ogg") => analyze_ogg_waveform(&bytes, WAVEFORM_BUCKETS),
        Some("wav") => analyze_wav_waveform(&bytes, WAVEFORM_BUCKETS),
        _ => (vec![0.0; WAVEFORM_BUCKETS], 0.0),
    }
}

/// Start decoding on a worker and poll without blocking the ECS schedule.
pub(super) fn sync_music_waveform(state: Res<EditorState>, mut waveform: ResMut<MusicWaveform>) {
    if state.music != waveform.path {
        waveform.path = state.music.clone();
        waveform.pending = None;
        waveform.buckets.clear();
        waveform.duration_secs = 0.0;
        if !state.music.trim().is_empty() {
            let path = state.music.clone();
            waveform.pending = Some(AsyncComputeTaskPool::get().spawn(async move {
                let (buckets, duration) = decode_music_waveform(std::path::Path::new(&path));
                (path, buckets, duration)
            }));
        }
    }
    let result = waveform
        .bypass_change_detection()
        .pending
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)));
    if let Some((path, buckets, duration)) = result {
        waveform.pending = None;
        waveform.apply_result(&path, buckets, duration);
    }
}

impl MusicWaveform {
    fn apply_result(&mut self, path: &str, buckets: Vec<f32>, duration: f64) {
        if path == self.path {
            self.buckets = buckets;
            self.duration_secs = duration;
        }
    }
}

/// The grid-space pixel x/width for waveform bucket `i` of `bucket_count`
/// (evenly spanning `duration_secs`), against `tempo_map` — pure so
/// "waveform time" to "grid pixel space" is directly testable without a
/// loaded chart. Each bucket's start/end time is placed via `song::chart::
/// seconds_to_tick`, so a tempo-change point shifts alignment exactly
/// like it shifts note ticks.
pub(super) fn waveform_bar_geometry(
    i: usize,
    bucket_count: usize,
    duration_secs: f64,
    tempo_map: &[TempoPoint],
) -> (f32, f32) {
    if bucket_count == 0 || duration_secs <= 0.0 {
        return (0.0, 0.0);
    }
    let bucket_secs = duration_secs / bucket_count as f64;
    let start_tick = seconds_to_tick(i as f64 * bucket_secs, TICKS_PER_BEAT as u32, tempo_map);
    let end_tick = seconds_to_tick(
        (i + 1) as f64 * bucket_secs,
        TICKS_PER_BEAT as u32,
        tempo_map,
    );
    let x = start_tick as f32 * super::TICK_W;
    let w = end_tick.saturating_sub(start_tick) as f32 * super::TICK_W;
    (x, w)
}

/// Which waveform bucket indices fall within the beats currently visible
/// (`scroll_beat..scroll_beat+cols`) — so `grid::rebuild_grid` only spawns
/// bars for the portion of a (possibly much longer) song actually on
/// screen, the same windowing principle as the note grid's own column loop.
pub(super) fn visible_waveform_buckets(
    scroll_beat: usize,
    cols: usize,
    bucket_count: usize,
    duration_secs: f64,
    tempo_map: &[TempoPoint],
) -> std::ops::Range<usize> {
    if bucket_count == 0 || duration_secs <= 0.0 {
        return 0..0;
    }
    let bucket_secs = duration_secs / bucket_count as f64;
    let start_tick = (scroll_beat * TICKS_PER_BEAT) as u64;
    let end_tick = ((scroll_beat + cols) * TICKS_PER_BEAT) as u64;
    let start_secs = tick_to_seconds(start_tick, TICKS_PER_BEAT as u32, tempo_map);
    let end_secs = tick_to_seconds(end_tick, TICKS_PER_BEAT as u32, tempo_map);
    let start = (start_secs / bucket_secs)
        .floor()
        .clamp(0.0, bucket_count as f64) as usize;
    let end = (end_secs / bucket_secs)
        .ceil()
        .clamp(0.0, bucket_count as f64) as usize;
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_background_result_cannot_replace_new_selection() {
        let mut waveform = MusicWaveform {
            path: "new.wav".into(),
            ..default()
        };
        waveform.apply_result("old.wav", vec![1.0], 60.0);
        assert!(waveform.buckets.is_empty());
        waveform.apply_result("new.wav", vec![0.5], 30.0);
        assert_eq!(waveform.buckets, vec![0.5]);
        assert_eq!(waveform.duration_secs, 30.0);
    }

    fn flat_120() -> Vec<TempoPoint> {
        vec![TempoPoint {
            tick: 0,
            bpm: 120.0,
        }]
    }

    // ── waveform_bar_geometry ────────────────────────────────────────────────

    #[test]
    fn first_bucket_starts_at_the_left_edge() {
        let (x, _) = waveform_bar_geometry(0, 10, 20.0, &flat_120());
        assert_eq!(x, 0.0);
    }

    #[test]
    fn bucket_width_matches_its_time_span_in_beats() {
        // 120 bpm -> 0.5s/beat. 10 buckets over 20s -> 2s/bucket -> 4 beats
        // per bucket -> 4 * BEAT_W px wide.
        let (_, w) = waveform_bar_geometry(0, 10, 20.0, &flat_120());
        assert!((w - 4.0 * super::super::BEAT_W).abs() < 0.01);
    }

    #[test]
    fn later_buckets_are_offset_by_their_start_time() {
        let (x0, w0) = waveform_bar_geometry(0, 10, 20.0, &flat_120());
        let (x1, _) = waveform_bar_geometry(1, 10, 20.0, &flat_120());
        assert!((x1 - (x0 + w0)).abs() < 0.01);
    }

    #[test]
    fn zero_buckets_or_duration_yields_a_degenerate_bar() {
        assert_eq!(waveform_bar_geometry(0, 0, 20.0, &flat_120()), (0.0, 0.0));
        assert_eq!(waveform_bar_geometry(0, 10, 0.0, &flat_120()), (0.0, 0.0));
    }

    #[test]
    fn a_tempo_change_shifts_later_bucket_positions() {
        // Doubling the tempo partway through means more beats (so more grid
        // pixels) pass per second of real time from then on — a later
        // bucket should land further right than the flat-tempo case, not
        // at the same spot.
        let flat = flat_120();
        let changed = vec![
            TempoPoint {
                tick: 0,
                bpm: 120.0,
            },
            TempoPoint {
                tick: TICKS_PER_BEAT as u64,
                bpm: 240.0,
            }, // one beat in (0.5s @ 120bpm), doubles
        ];
        let (x_flat, _) = waveform_bar_geometry(5, 10, 20.0, &flat);
        let (x_changed, _) = waveform_bar_geometry(5, 10, 20.0, &changed);
        assert!(
            x_changed > x_flat,
            "{x_changed} should be later than {x_flat}"
        );
    }

    // ── visible_waveform_buckets ─────────────────────────────────────────────

    #[test]
    fn no_buckets_or_duration_yields_an_empty_range() {
        assert_eq!(visible_waveform_buckets(0, 8, 0, 20.0, &flat_120()), 0..0);
        assert_eq!(visible_waveform_buckets(0, 8, 10, 0.0, &flat_120()), 0..0);
    }

    #[test]
    fn range_covers_the_visible_beats_worth_of_buckets() {
        // 120 bpm, 10 buckets over 20s (2s = 4 beats per bucket). Viewing
        // beats 0..8 (two bucket-widths) should cover buckets 0 and 1.
        let range = visible_waveform_buckets(0, 8, 10, 20.0, &flat_120());
        assert_eq!(range, 0..2);
    }

    #[test]
    fn range_shifts_with_scroll_position() {
        // 120 bpm: 0.5s/beat, so 20 beats in is 10s (bucket 5, since each
        // bucket is 2s); the following 8-beat (4s) window reaches bucket 7.
        let range = visible_waveform_buckets(20, 8, 10, 20.0, &flat_120());
        assert_eq!(range, 5..7);
    }

    #[test]
    fn range_never_exceeds_the_bucket_count() {
        let range = visible_waveform_buckets(1000, 8, 10, 20.0, &flat_120());
        assert_eq!(range, 10..10);
    }
}
