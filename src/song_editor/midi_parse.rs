// SPDX-License-Identifier: MIT

//! The Song Editor's own MIDI-tempo-map conversion — turning a raw MIDI
//! tempo map (`crate::song::midi::collect_tempo_map`'s `(tick, µs/quarter)`
//! pairs, in the file's own `tpq` resolution) into the editor's own tempo
//! map (`TICKS_PER_BEAT`-resolution ticks, `bpm` instead of µs). The actual
//! MIDI-file parsing this builds on (`ticks_per_quarter`/`track_name_of`/
//! `note_on_count`/`collect_tempo_map`/`extract_notes`) lives in
//! `crate::song::midi` — shared with `bin/midi_to_chart`, which used to
//! keep its own duplicate copies of all of it.

use super::TICKS_PER_BEAT;
use crate::song::chart::{TempoPoint, seconds_to_tick};
use crate::song::midi::tick_to_seconds;

/// Converts a MIDI tempo map (`(tick, microseconds_per_quarter)`, in the
/// file's own `tpq` resolution) into the editor's own tempo map (ticks in
/// `TICKS_PER_BEAT` units, `bpm` instead of microseconds) — each point's
/// *real time* position is preserved (via `tick_to_seconds`/
/// `seconds_to_tick`), not its raw tick number, since a MIDI file's `tpq`
/// has no fixed ratio to the editor's own resolution the way two charts
/// both declaring `resolution: TICKS_PER_BEAT` would (see
/// `harpchart::load_harpchart`'s simpler constant-ratio rescaling for that
/// case). Built incrementally: each new point is placed by `seconds_to_tick`
/// against the *already-converted* prefix of the map, which is exactly the
/// segment it's the end of.
pub(super) fn editor_tempo_map(midi_tempo: &[(u64, u32)], tpq: u32) -> Vec<TempoPoint> {
    let mut editor_map: Vec<TempoPoint> = Vec::with_capacity(midi_tempo.len());
    for &(tick, us) in midi_tempo {
        let bpm = (60_000_000.0 / us as f64).clamp(20.0, 300.0) as f32;
        let editor_tick = if editor_map.is_empty() {
            0
        } else {
            let secs = tick_to_seconds(tick, tpq, midi_tempo);
            seconds_to_tick(secs, TICKS_PER_BEAT as u32, &editor_map)
        };
        editor_map.push(TempoPoint {
            tick: editor_tick,
            bpm,
        });
    }
    editor_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_tempo_point_lands_at_tick_zero() {
        let map = editor_tempo_map(&[(0, 500_000)], 480);
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].tick, 0);
        assert!((map[0].bpm - 120.0).abs() < 0.01);
    }

    #[test]
    fn a_later_tempo_change_is_placed_by_real_time_not_raw_tick() {
        // 480 tpq at 120 BPM: tick 480 is exactly one beat (0.5s) in, which
        // is exactly TICKS_PER_BEAT in the editor's own resolution.
        let map = editor_tempo_map(&[(0, 500_000), (480, 250_000)], 480);
        assert_eq!(map.len(), 2);
        assert_eq!(map[1].tick, TICKS_PER_BEAT as u64);
        assert!((map[1].bpm - 240.0).abs() < 0.01);
    }

    #[test]
    fn bpm_is_clamped_to_a_sane_range() {
        let map = editor_tempo_map(&[(0, 20_000_000)], 480); // 3 BPM, absurdly slow
        assert!((map[0].bpm - 20.0).abs() < 0.01);
    }
}
