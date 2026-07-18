// SPDX-License-Identifier: MIT

//! Displays the chart's referenced music file (`EditorState::music`) as a
//! peak-amplitude waveform in the grid header — a visual placement aid for
//! aligning notes and tempo to the actual audio, the foundation
//! `ROADMAP.md`'s "tempo-map editing against an imported audio track" 0.5
//! item builds on. Reuses `audio_system::waveform`'s existing decoders (the
//! same ones a shipped song's own music gets analyzed with at asset-load
//! time) rather than duplicating any audio-decoding logic.
//!
//! Still assumes one constant tempo throughout, same as the rest of the
//! editor's tick grid today — so the waveform-to-pixel mapping is a single
//! multiplication (see [`waveform_bar_geometry`]), not yet a real variable
//! tempo map. That's the larger remaining part of the 0.5 item.

use bevy::prelude::*;

use super::state::EditorState;
use crate::audio_system::waveform::{WAVEFORM_BUCKETS, analyze_ogg_waveform, analyze_wav_waveform};

/// The chart's music file, decoded into a peak-amplitude waveform — empty
/// (`duration_secs == 0.0`) until a music file is set, or if decoding it
/// failed. `path` is `MusicWaveform`'s own cache of the `EditorState::music`
/// value it was last decoded from, so [`sync_music_waveform`] can tell
/// whether a re-decode is needed without depending on `Changed<EditorState>`
/// (which fires far more often than the music field actually changes).
#[derive(Resource, Default)]
pub(super) struct MusicWaveform {
    path: String,
    pub(super) buckets: Vec<f32>,
    pub(super) duration_secs: f64,
}

/// Decodes `path`'s audio into a peak-amplitude waveform, dispatching by
/// file extension the same way `song::loader` does for a shipped song's own
/// music. Degrades to an all-zero, zero-duration waveform (never errors)
/// for an unreadable file or an extension neither decoder handles — the
/// same "never fails the caller" convention `analyze_ogg_waveform`/
/// `analyze_wav_waveform` themselves already follow.
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

/// Keeps [`MusicWaveform`] in step with `EditorState::music` — re-decodes
/// only when the path actually changed since last frame. A synchronous
/// decode on the main thread, same as `midi_import`'s own file-picker
/// handling; a brief hitch on picking a long file is an accepted trade-off
/// for not needing an async asset-loading path just for this preview.
pub(super) fn sync_music_waveform(state: Res<EditorState>, mut waveform: ResMut<MusicWaveform>) {
    if state.music == waveform.path {
        return;
    }
    waveform.path = state.music.clone();
    if state.music.trim().is_empty() {
        waveform.buckets = Vec::new();
        waveform.duration_secs = 0.0;
        return;
    }
    let (buckets, duration_secs) = decode_music_waveform(std::path::Path::new(&state.music));
    waveform.buckets = buckets;
    waveform.duration_secs = duration_secs;
}

/// The grid-space pixel x/width for waveform bucket `i` of `bucket_count`
/// (evenly spanning `duration_secs`), at the chart's `bpm` — pure so the
/// mapping from "waveform time" to "grid pixel space" is directly testable
/// without a loaded chart. Assumes a constant tempo throughout, same
/// simplification the rest of the editor's tick grid makes today.
pub(super) fn waveform_bar_geometry(
    i: usize,
    bucket_count: usize,
    duration_secs: f64,
    bpm: f32,
) -> (f32, f32) {
    if bucket_count == 0 || duration_secs <= 0.0 {
        return (0.0, 0.0);
    }
    let secs_per_beat = 60.0 / bpm.max(1.0) as f64;
    let bucket_secs = duration_secs / bucket_count as f64;
    let x = (i as f64 * bucket_secs / secs_per_beat) as f32 * super::BEAT_W;
    let w = (bucket_secs / secs_per_beat) as f32 * super::BEAT_W;
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
    bpm: f32,
) -> std::ops::Range<usize> {
    if bucket_count == 0 || duration_secs <= 0.0 {
        return 0..0;
    }
    let secs_per_beat = 60.0 / bpm.max(1.0) as f64;
    let bucket_secs = duration_secs / bucket_count as f64;
    let start_secs = scroll_beat as f64 * secs_per_beat;
    let end_secs = (scroll_beat + cols) as f64 * secs_per_beat;
    let start = (start_secs / bucket_secs).floor().clamp(0.0, bucket_count as f64) as usize;
    let end = (end_secs / bucket_secs).ceil().clamp(0.0, bucket_count as f64) as usize;
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── waveform_bar_geometry ────────────────────────────────────────────────

    #[test]
    fn first_bucket_starts_at_the_left_edge() {
        let (x, _) = waveform_bar_geometry(0, 10, 20.0, 120.0);
        assert_eq!(x, 0.0);
    }

    #[test]
    fn bucket_width_matches_its_time_span_in_beats() {
        // 120 bpm -> 0.5s/beat. 10 buckets over 20s -> 2s/bucket -> 4 beats
        // per bucket -> 4 * BEAT_W px wide.
        let (_, w) = waveform_bar_geometry(0, 10, 20.0, 120.0);
        assert!((w - 4.0 * super::super::BEAT_W).abs() < 0.01);
    }

    #[test]
    fn later_buckets_are_offset_by_their_start_time() {
        let (x0, w0) = waveform_bar_geometry(0, 10, 20.0, 120.0);
        let (x1, _) = waveform_bar_geometry(1, 10, 20.0, 120.0);
        assert!((x1 - (x0 + w0)).abs() < 0.01);
    }

    #[test]
    fn zero_buckets_or_duration_yields_a_degenerate_bar() {
        assert_eq!(waveform_bar_geometry(0, 0, 20.0, 120.0), (0.0, 0.0));
        assert_eq!(waveform_bar_geometry(0, 10, 0.0, 120.0), (0.0, 0.0));
    }

    // ── visible_waveform_buckets ─────────────────────────────────────────────

    #[test]
    fn no_buckets_or_duration_yields_an_empty_range() {
        assert_eq!(visible_waveform_buckets(0, 8, 0, 20.0, 120.0), 0..0);
        assert_eq!(visible_waveform_buckets(0, 8, 10, 0.0, 120.0), 0..0);
    }

    #[test]
    fn range_covers_the_visible_beats_worth_of_buckets() {
        // 120 bpm, 10 buckets over 20s (2s = 4 beats per bucket). Viewing
        // beats 0..8 (two bucket-widths) should cover buckets 0 and 1.
        let range = visible_waveform_buckets(0, 8, 10, 20.0, 120.0);
        assert_eq!(range, 0..2);
    }

    #[test]
    fn range_shifts_with_scroll_position() {
        // 120 bpm: 0.5s/beat, so 20 beats in is 10s (bucket 5, since each
        // bucket is 2s); the following 8-beat (4s) window reaches bucket 7.
        let range = visible_waveform_buckets(20, 8, 10, 20.0, 120.0);
        assert_eq!(range, 5..7);
    }

    #[test]
    fn range_never_exceeds_the_bucket_count() {
        let range = visible_waveform_buckets(1000, 8, 10, 20.0, 120.0);
        assert_eq!(range, 10..10);
    }
}
