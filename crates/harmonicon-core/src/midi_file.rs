// SPDX-License-Identifier: MIT

//! Pure MIDI-file parsing used by `song_editor::midi_import`'s track
//! listing/pitch mapping/note import: reading track names/note counts,
//! tempo maps, and note on/off pairs out of a parsed `midly::Smf` — no
//! pitch-to-harp resolution or chart-building happens here, that's the
//! caller's own concern. Lives here rather than in `song_editor` since it's
//! low-level shared vocabulary, not editor-specific.

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use std::collections::HashMap;

use crate::midi::midi_to_freq_hz;
use crate::synth::{Expr, PhraseNote, TICKS_PER_BEAT, render_pcm};

pub const DEFAULT_TEMPO_US: u32 = 500_000; // 120 BPM if the file specifies none

pub fn ticks_per_quarter(smf: &Smf) -> Result<u32, String> {
    match smf.header.timing {
        Timing::Metrical(tpq) => Ok(tpq.as_int() as u32),
        Timing::Timecode(..) => {
            Err("timecode-based MIDI timing is not supported (need metrical)".to_string())
        }
    }
}

/// The first time signature declared anywhere in a MIDI file. MIDI encodes
/// the denominator as a power of two (`3` means `/8`).
pub fn time_signature_of(smf: &Smf) -> Option<(u8, u8)> {
    for track in &smf.tracks {
        for event in track {
            if let TrackEventKind::Meta(MetaMessage::TimeSignature(
                numerator,
                denominator_pow2,
                _,
                _,
            )) = event.kind
            {
                return Some((numerator, 1u8.checked_shl(denominator_pow2 as u32)?));
            }
        }
    }
    None
}

/// All time-signature changes across the file as `(absolute_tick, numerator,
/// denominator)`, sorted, with an implicit 4/4 entry at tick 0 when none is
/// declared there. Invalid denominator exponents are ignored.
pub fn collect_time_signature_map(smf: &Smf) -> Vec<(u64, u8, u8)> {
    let mut changes = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u64;
        for event in track {
            tick += event.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::TimeSignature(
                numerator,
                denominator_pow2,
                _,
                _,
            )) = event.kind
                && let Some(denominator) = 1u8.checked_shl(denominator_pow2 as u32)
            {
                changes.push((tick, numerator, denominator));
            }
        }
    }
    changes.sort_by_key(|&(tick, _, _)| tick);
    if changes.first().map(|&(tick, _, _)| tick) != Some(0) {
        changes.insert(0, (0, 4, 4));
    }
    changes
}

pub fn track_name_of(track: &[midly::TrackEvent]) -> Option<String> {
    track.iter().find_map(|ev| match ev.kind {
        TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
            let name = String::from_utf8_lossy(bytes).trim().to_string();
            (!name.is_empty()).then_some(name)
        }
        _ => None,
    })
}

pub fn note_on_count(track: &[midly::TrackEvent]) -> usize {
    track
        .iter()
        .filter(|ev| {
            matches!(
                ev.kind,
                TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0
            )
        })
        .count()
}

/// All tempo changes across the file as `(absolute_tick, microseconds_per_quarter)`,
/// sorted, with an implicit 120 BPM entry at tick 0 if none is given there.
pub fn collect_tempo_map(smf: &Smf) -> Vec<(u64, u32)> {
    let mut changes: Vec<(u64, u32)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = ev.kind {
                changes.push((tick, us.as_int()));
            }
        }
    }
    changes.sort_by_key(|&(t, _)| t);
    if changes.first().map(|&(t, _)| t) != Some(0) {
        changes.insert(0, (0, DEFAULT_TEMPO_US));
    }
    changes
}

/// Absolute time in seconds of a tick, integrating across tempo changes.
pub fn tick_to_seconds(tick: u64, tpq: u32, tempo: &[(u64, u32)]) -> f64 {
    let mut seconds = 0.0;
    let mut prev_tick = 0u64;
    let mut us = tempo.first().map(|&(_, u)| u).unwrap_or(DEFAULT_TEMPO_US);
    for &(change_tick, change_us) in tempo {
        if change_tick >= tick {
            break;
        }
        let span = change_tick - prev_tick;
        seconds += span as f64 * us as f64 / tpq as f64 / 1_000_000.0;
        prev_tick = change_tick;
        us = change_us;
    }
    seconds += (tick - prev_tick) as f64 * us as f64 / tpq as f64 / 1_000_000.0;
    seconds
}

pub struct RawNote {
    pub start_tick: u64,
    pub dur_ticks: u64,
    pub key: u8,
}

/// Pairs NoteOn/NoteOff into [`RawNote`]s, ordered by start tick.
pub fn extract_notes(track: &[midly::TrackEvent]) -> Vec<RawNote> {
    let mut open: HashMap<u8, u64> = HashMap::new();
    let mut notes: Vec<RawNote> = Vec::new();
    let mut tick: u64 = 0;
    for ev in track {
        tick += ev.delta.as_int() as u64;
        if let TrackEventKind::Midi { message, .. } = ev.kind {
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    open.insert(key.as_int(), tick);
                }
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    if let Some(start) = open.remove(&key.as_int()) {
                        notes.push(RawNote {
                            start_tick: start,
                            dur_ticks: tick.saturating_sub(start),
                            key: key.as_int(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    notes.sort_by_key(|n| (n.start_tick, n.key));
    notes
}

/// Converts extracted MIDI notes onto a synth-renderable tick grid
/// (`secs_per_tick` seconds per tick) — shared by [`render_track_pcm`]
/// below and `song_editor::midi_import::render_backing_pcm`, so both
/// convert raw MIDI timing into synth phrase notes exactly the same way.
pub fn notes_to_phrase(
    notes: &[RawNote],
    tpq: u32,
    tempo: &[(u64, u32)],
    secs_per_tick: f64,
) -> Vec<PhraseNote> {
    notes
        .iter()
        .map(|n| {
            let start_secs = tick_to_seconds(n.start_tick, tpq, tempo);
            let end_secs = tick_to_seconds(n.start_tick + n.dur_ticks, tpq, tempo);
            let tick = (start_secs / secs_per_tick).round() as usize;
            let len = (((end_secs - start_secs) / secs_per_tick).round() as usize).max(1);
            PhraseNote { tick, len, freq: Some(midi_to_freq_hz(n.key as f32)), expr: Expr::None }
        })
        .collect()
}

/// Renders one track's own notes (only, not the other tracks) to a PCM
/// buffer via the shared additive harmonica-voice synth
/// (`audio_system::synth::render_pcm`) — used when a song ships each MIDI
/// track as its own separately-playable backing stem (`song::loader`'s
/// multi-track loading) rather than one pre-mixed file. `tpq`/`tempo` are
/// the file's own (`ticks_per_quarter`/`collect_tempo_map`), computed once
/// by the caller and shared across every track's render rather than
/// re-derived per call. `None` for a track with no notes — nothing to
/// render, and an all-silent stem would be pointless to play/mute.
pub fn render_track_pcm(
    track: &[midly::TrackEvent],
    tpq: u32,
    tempo: &[(u64, u32)],
) -> Option<Vec<f32>> {
    let notes = extract_notes(track);
    if notes.is_empty() {
        return None;
    }
    let initial_bpm = (60_000_000.0 / tempo[0].1 as f64).clamp(20.0, 300.0) as f32;
    let secs_per_tick = 60.0 / initial_bpm.max(1.0) as f64 / TICKS_PER_BEAT as f64;
    let phrase = notes_to_phrase(&notes, tpq, tempo, secs_per_tick);
    Some(render_pcm(&phrase, secs_per_tick as f32))
}

// ── Shared test fixtures ─────────────────────────────────────────────────────
//
// Also used by `song_editor::midi_import`'s own tests (constructing the
// same fake MIDI byte streams), hence `pub` and module-level rather than
// nested in `mod tests` below.

#[cfg(any(test, feature = "test-support"))]
pub fn meta(delta: u32, kind: MetaMessage<'static>) -> midly::TrackEvent<'static> {
    midly::TrackEvent { delta: midly::num::u28::from(delta), kind: TrackEventKind::Meta(kind) }
}

#[cfg(any(test, feature = "test-support"))]
pub fn note_on(delta: u32, key: u8, vel: u8) -> midly::TrackEvent<'static> {
    midly::TrackEvent {
        delta: midly::num::u28::from(delta),
        kind: TrackEventKind::Midi {
            channel: midly::num::u4::from(0),
            message: MidiMessage::NoteOn {
                key: midly::num::u7::from(key),
                vel: midly::num::u7::from(vel),
            },
        },
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn note_off(delta: u32, key: u8) -> midly::TrackEvent<'static> {
    midly::TrackEvent {
        delta: midly::num::u28::from(delta),
        kind: TrackEventKind::Midi {
            channel: midly::num::u4::from(0),
            message: MidiMessage::NoteOff {
                key: midly::num::u7::from(key),
                vel: midly::num::u7::from(0),
            },
        },
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn smf_bytes(tracks: Vec<Vec<midly::TrackEvent<'static>>>) -> Vec<u8> {
    let smf = Smf {
        header: midly::Header {
            format: if tracks.len() > 1 {
                midly::Format::Parallel
            } else {
                midly::Format::SingleTrack
            },
            timing: Timing::Metrical(midly::num::u15::from(480)),
        },
        tracks,
    };
    let mut out = Vec::new();
    smf.write_std(&mut out).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── track_name_of / note_on_count ────────────────────────────────────────

    #[test]
    fn track_name_of_reads_the_first_track_name_event() {
        let track = vec![meta(0, MetaMessage::TrackName(b"Bass")), note_on(0, 60, 100)];
        assert_eq!(track_name_of(&track).as_deref(), Some("Bass"));
    }

    #[test]
    fn track_name_of_is_none_without_a_name_event() {
        assert_eq!(track_name_of(&[note_on(0, 60, 100)]), None);
    }

    #[test]
    fn track_name_of_is_none_for_a_blank_name() {
        let track = vec![meta(0, MetaMessage::TrackName(b"   "))];
        assert_eq!(track_name_of(&track), None);
    }

    #[test]
    fn note_on_count_ignores_zero_velocity_note_ons() {
        let track =
            vec![note_on(0, 60, 100), note_on(10, 62, 0), note_off(10, 60), note_on(0, 64, 80)];
        assert_eq!(note_on_count(&track), 2);
    }

    // ── collect_tempo_map / tick_to_seconds ─────────────────────────────────────

    #[test]
    fn collect_tempo_map_defaults_to_120bpm_at_tick_zero_when_unset() {
        let bytes = smf_bytes(vec![vec![note_on(0, 60, 100)]]);
        let smf = Smf::parse(&bytes).unwrap();
        assert_eq!(collect_tempo_map(&smf), vec![(0, DEFAULT_TEMPO_US)]);
    }

    #[test]
    fn time_signature_converts_midis_power_of_two_denominator() {
        let bytes = smf_bytes(vec![vec![meta(0, MetaMessage::TimeSignature(6, 3, 24, 8))]]);
        let smf = Smf::parse(&bytes).unwrap();
        assert_eq!(time_signature_of(&smf), Some((6, 8)));
    }

    #[test]
    fn collect_time_signature_map_sorts_changes_across_tracks() {
        let bytes = smf_bytes(vec![
            vec![meta(480, MetaMessage::TimeSignature(3, 2, 24, 8))],
            vec![meta(0, MetaMessage::TimeSignature(6, 3, 24, 8))],
        ]);
        let smf = Smf::parse(&bytes).unwrap();
        assert_eq!(collect_time_signature_map(&smf), vec![(0, 6, 8), (480, 3, 4)]);
    }

    #[test]
    fn collect_time_signature_map_supplies_opening_four_four() {
        let bytes = smf_bytes(vec![vec![meta(480, MetaMessage::TimeSignature(7, 3, 24, 8))]]);
        let smf = Smf::parse(&bytes).unwrap();
        assert_eq!(collect_time_signature_map(&smf), vec![(0, 4, 4), (480, 7, 8)]);
    }

    #[test]
    fn collect_tempo_map_collects_and_sorts_changes_across_tracks() {
        let bytes = smf_bytes(vec![
            vec![meta(100, MetaMessage::Tempo(midly::num::u24::from(300_000)))],
            vec![meta(0, MetaMessage::Tempo(midly::num::u24::from(500_000)))],
        ]);
        let smf = Smf::parse(&bytes).unwrap();
        assert_eq!(collect_tempo_map(&smf), vec![(0, 500_000), (100, 300_000)]);
    }

    #[test]
    fn tick_to_seconds_integrates_across_a_tempo_change() {
        let tpq = 480;
        let tempo = vec![(0u64, 500_000u32), (480, 250_000)];
        assert!((tick_to_seconds(0, tpq, &tempo) - 0.0).abs() < 1e-9);
        assert!((tick_to_seconds(480, tpq, &tempo) - 0.5).abs() < 1e-9);
        assert!((tick_to_seconds(960, tpq, &tempo) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn tick_to_seconds_uses_default_tempo_with_an_empty_map() {
        assert!((tick_to_seconds(480, 480, &[]) - 0.5).abs() < 1e-9);
    }

    // ── extract_notes ─────────────────────────────────────────────────────────────

    #[test]
    fn extract_notes_pairs_note_on_with_note_off() {
        let track = vec![note_on(0, 60, 100), note_off(240, 60)];
        let notes = extract_notes(&track);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].start_tick, 0);
        assert_eq!(notes[0].dur_ticks, 240);
        assert_eq!(notes[0].key, 60);
    }

    #[test]
    fn extract_notes_treats_zero_velocity_note_on_as_note_off() {
        let track = vec![note_on(0, 60, 100), note_on(120, 60, 0)];
        let notes = extract_notes(&track);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].dur_ticks, 120);
    }

    #[test]
    fn extract_notes_orders_overlapping_notes_by_start_then_key() {
        let track =
            vec![note_on(0, 64, 100), note_on(0, 60, 100), note_off(100, 64), note_off(0, 60)];
        let notes = extract_notes(&track);
        let keys: Vec<u8> = notes.iter().map(|n| n.key).collect();
        assert_eq!(keys, vec![60, 64]);
    }

    #[test]
    fn extract_notes_ignores_an_unmatched_note_off() {
        let track = vec![note_off(0, 60)];
        assert!(extract_notes(&track).is_empty());
    }

    // ── render_track_pcm ─────────────────────────────────────────────────────

    #[test]
    fn render_track_pcm_is_none_for_a_track_with_no_notes() {
        let track = [meta(0, MetaMessage::TrackName(b"Empty"))];
        assert!(render_track_pcm(&track, 480, &[(0, DEFAULT_TEMPO_US)]).is_none());
    }

    #[test]
    fn render_track_pcm_renders_nonempty_audio_for_a_track_with_notes() {
        let track = [note_on(0, 60, 100), note_off(480, 60)];
        let pcm = render_track_pcm(&track, 480, &[(0, DEFAULT_TEMPO_US)])
            .expect("a track with notes should render something");
        assert!(!pcm.is_empty());
        // 120 BPM (the default), one beat (480 ticks at tpq 480) long, plus
        // the synth's fixed tail — comfortably longer than a zero-length hum.
        assert!(pcm.len() > 44_100 / 4);
    }

    #[test]
    fn notes_to_phrase_converts_tick_timing_into_the_synth_grid() {
        let notes = vec![RawNote { start_tick: 0, dur_ticks: 240, key: 60 }];
        // tpq 480, 120 BPM (500_000 us/quarter, so one quarter = 0.5s) ->
        // 240 ticks (a quarter of that quarter) is 0.25s; at
        // secs_per_tick = 1.0 / TICKS_PER_BEAT (one beat per second), that's
        // exactly a quarter of TICKS_PER_BEAT synth ticks.
        let secs_per_tick = 1.0 / TICKS_PER_BEAT as f64;
        let phrase = notes_to_phrase(&notes, 480, &[(0, DEFAULT_TEMPO_US)], secs_per_tick);
        assert_eq!(phrase.len(), 1);
        assert_eq!(phrase[0].tick, 0);
        assert_eq!(phrase[0].len, TICKS_PER_BEAT / 4);
        assert_eq!(phrase[0].freq, Some(midi_to_freq_hz(60.0)));
    }
}
