// SPDX-License-Identifier: MIT

//! Pure MIDI-file parsing shared by `midi_import`'s track listing, pitch
//! mapping, and note import — reading track names/note counts, tempo maps,
//! and note on/off pairs out of a parsed `midly::Smf`. No pitch-to-harp
//! resolution or `EditorState` mutation happens here; see `midi_import` for
//! that.

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use std::collections::HashMap;

pub(super) const DEFAULT_TEMPO_US: u32 = 500_000; // 120 BPM if the file specifies none

pub(super) fn ticks_per_quarter(smf: &Smf) -> Result<u32, String> {
    match smf.header.timing {
        Timing::Metrical(tpq) => Ok(tpq.as_int() as u32),
        Timing::Timecode(..) => {
            Err("timecode-based MIDI timing is not supported (need metrical)".to_string())
        }
    }
}

pub(super) fn track_name_of(track: &[midly::TrackEvent]) -> Option<String> {
    track.iter().find_map(|ev| match ev.kind {
        TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
            let name = String::from_utf8_lossy(bytes).trim().to_string();
            (!name.is_empty()).then_some(name)
        }
        _ => None,
    })
}

pub(super) fn note_on_count(track: &[midly::TrackEvent]) -> usize {
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
pub(super) fn collect_tempo_map(smf: &Smf) -> Vec<(u64, u32)> {
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
pub(super) fn tick_to_seconds(tick: u64, tpq: u32, tempo: &[(u64, u32)]) -> f64 {
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

pub(super) struct RawNote {
    pub(super) start_tick: u64,
    pub(super) dur_ticks: u64,
    pub(super) key: u8,
}

/// Pairs NoteOn/NoteOff into [`RawNote`]s, ordered by start tick.
pub(super) fn extract_notes(track: &[midly::TrackEvent]) -> Vec<RawNote> {
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

// ── Shared test fixtures ─────────────────────────────────────────────────────
//
// Also used by `midi_import`'s own tests (constructing the same fake MIDI
// byte streams for `map_pitch`/`import_track_notes`/etc.), hence `pub(super)`
// and module-level rather than nested in `mod tests` below.

#[cfg(test)]
pub(super) fn meta(delta: u32, kind: MetaMessage<'static>) -> midly::TrackEvent<'static> {
    midly::TrackEvent {
        delta: midly::num::u28::from(delta),
        kind: TrackEventKind::Meta(kind),
    }
}

#[cfg(test)]
pub(super) fn note_on(delta: u32, key: u8, vel: u8) -> midly::TrackEvent<'static> {
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

#[cfg(test)]
pub(super) fn note_off(delta: u32, key: u8) -> midly::TrackEvent<'static> {
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

#[cfg(test)]
pub(super) fn smf_bytes(tracks: Vec<Vec<midly::TrackEvent<'static>>>) -> Vec<u8> {
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
        let track = vec![
            note_on(0, 60, 100),
            note_on(10, 62, 0),
            note_off(10, 60),
            note_on(0, 64, 80),
        ];
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
    fn tick_to_seconds_integrates_across_a_tempo_change() {
        let tpq = 480;
        let tempo = vec![(0u64, 500_000u32), (480, 250_000)];
        assert!((tick_to_seconds(0, tpq, &tempo) - 0.0).abs() < 1e-9);
        assert!((tick_to_seconds(480, tpq, &tempo) - 0.5).abs() < 1e-9);
        assert!((tick_to_seconds(960, tpq, &tempo) - 0.75).abs() < 1e-9);
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
}
