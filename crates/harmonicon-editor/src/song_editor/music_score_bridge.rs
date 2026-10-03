// SPDX-License-Identifier: MIT

//! Converts editor notes and playhead ticks for the shared music score.

use bevy::prelude::*;

use harmonicon_ui::music_score::{
    MeterMap, MusicScoreNotes, MusicScorePlayhead, NotationNote, parse_time_signature,
};
use harmonicon_ui::music_score::{MusicScoreBarMap, MusicScoreMeter};

use super::TICKS_PER_BEAT;
use super::metronome::MeterClockCache;
use super::playback::{Playhead, note_midi};
use super::state::EditorState;

/// Where `tick` sits on the staff, in quarter-note beats. The staff draws
/// bar lines at whole multiples of a bar from beat 0, so a pickup is moved
/// along by the part of its bar the music skips — its first segment's phase
/// — which puts bar 1 exactly on a bar line.
fn staff_beat(tick: f64, meter_map: &MeterMap) -> f64 {
    (tick + meter_map.segments()[0].phase_ticks as f64) / TICKS_PER_BEAT as f64
}

/// `note` as staff notes, split and tied at every bar line it crosses.
/// `selected` highlights every segment, so a long selected note reads as
/// selected wherever the staff has scrolled to.
fn notation_segments(
    note: &super::state::GridNote,
    midi: u8,
    meter_map: &MeterMap,
    selected: bool,
) -> Vec<NotationNote> {
    let start = note.tick as u64;
    let end = (note.tick + note.len.max(1)) as u64;
    MusicScoreBarMap { meter: meter_map.clone(), quarter_ticks: TICKS_PER_BEAT as u32 }
        .split_note(start, end, midi, selected)
}

/// Rebuilds [`MusicScoreNotes`] from `EditorState::notes` whenever the
/// editor state changes — same `resource_exists_and_changed::<EditorState>`
/// gate every other EditorState-derived rebuild in `song_editor::mod` uses.
/// A note whose hole/technique the current harp can't resolve is skipped,
/// same as gameplay's bridge. The editor's meter map supplies every bar
/// boundary, so notes crossing either an ordinary bar line or a meter change
/// become tied segments instead of one oversized notehead. Selected notes
/// are highlighted, so the staff shows where in the song the grid's
/// selection is; a selection change is an `EditorState` change, so it
/// rebuilds here like any edit.
pub(super) fn sync_music_score(
    state: Res<EditorState>,
    mut notes: ResMut<MusicScoreNotes>,
    mut meter: ResMut<MusicScoreMeter>,
    mut bar_map: ResMut<MusicScoreBarMap>,
) {
    let editor_meter = parse_time_signature(&state.time_signature);
    if *meter != editor_meter {
        *meter = editor_meter;
    }
    let harp = state.effective_harp();
    let meter_map = state.meter_map();
    let next_bar_map =
        MusicScoreBarMap { meter: meter_map.clone(), quarter_ticks: TICKS_PER_BEAT as u32 };
    if *bar_map != next_bar_map {
        *bar_map = next_bar_map;
    }
    let mut staff: Vec<NotationNote> = state
        .notes
        .iter()
        .filter_map(|n| {
            let midi = note_midi(n, &harp)?;
            Some(notation_segments(n, midi, &meter_map, state.is_selected(n.id)))
        })
        .flatten()
        .collect();
    // The grid keeps notes in the order they were placed; the staff reads
    // them in time order (accidentals carry through a bar, beams join
    // neighbours). Stable, so a note's own tied segments stay in order.
    staff.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    notes.0 = staff;
}

/// Keeps [`MusicScorePlayhead`] following the same tick position
/// `playback::update_playhead_view`'s moving line derives from [`Playhead`]
/// while a take is playing (or paused mid-take) — ordered `.after(playback
/// ::advance_playhead)` so it reads the same frame's `elapsed`. Otherwise
/// (just editing, nothing running) it follows `EditorState::scroll_beat`
/// instead, so a note far from the song's start can still scroll into the
/// panel's fixed-width visible window rather than the score staying pinned
/// at beat 0.
pub(super) fn sync_music_score_playhead(
    playhead: Res<Playhead>,
    state: Res<EditorState>,
    mut score_playhead: ResMut<MusicScorePlayhead>,
    mut meter: ResMut<MusicScoreMeter>,
    mut meter_cache: Local<Option<MeterClockCache>>,
) {
    let cur_tick = if playhead.playing && playhead.secs_per_tick > 0.0 {
        f64::from(playhead.elapsed / playhead.secs_per_tick)
    } else {
        (state.scroll_beat * TICKS_PER_BEAT) as f64
    };
    let map = MeterClockCache::map_for(&mut meter_cache, &state);
    // The same offset the notes carry, so the playhead reads against them.
    let beat = staff_beat(cur_tick, map);
    if score_playhead.0 != beat {
        score_playhead.0 = beat;
    }
    let active_meter = map.meter_at(cur_tick.max(0.0).round() as u64);
    if *meter != active_meter {
        *meter = active_meter;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_editor::state::{Dir, Expr, GridNote, Pitch};
    use harmonicon_ui::music_score::rests_between_notes;

    fn note(tick: usize, len: usize) -> GridNote {
        GridNote {
            id: 1,
            hole: 4,
            tick,
            len,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        }
    }

    #[test]
    fn notation_splits_and_ties_across_meter_map_bars() {
        let map = MeterMap::new([(0, "4/4"), (48, "3/4")], TICKS_PER_BEAT as u32);

        let segments = notation_segments(&note(36, 60), 60, &map, false);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].duration_beats, 1.0);
        assert_eq!(segments[1].duration_beats, 3.0);
        assert_eq!(segments[2].duration_beats, 1.0);
        assert!(!segments[0].tied_from_previous);
        assert!(segments[1].tied_from_previous);
        assert!(segments[2].tied_from_previous);
    }

    #[test]
    fn a_pickup_sits_at_the_end_of_the_staffs_first_bar() {
        // One-beat pickup in 4/4: it is drawn as beat 4 of bar 0, so a note
        // on bar 1's downbeat lands on the staff's first bar line.
        let map = MeterMap::with_pickup([(0, "4/4")], TICKS_PER_BEAT as u32, 12);
        let pickup = notation_segments(&note(0, 12), 60, &map, false);
        let downbeat = notation_segments(&note(12, 12), 60, &map, false);
        assert_eq!(pickup[0].start_beat, 3.0);
        assert_eq!(downbeat[0].start_beat, 4.0, "a whole bar from beat 0");
    }

    #[test]
    fn a_selected_note_is_highlighted_in_every_segment() {
        let map = MeterMap::new([(0, "4/4")], TICKS_PER_BEAT as u32);
        // Crosses the bar line at 48: two tied segments, both highlighted.
        let selected = notation_segments(&note(36, 24), 60, &map, true);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|segment| segment.highlighted));
        let other = notation_segments(&note(36, 24), 60, &map, false);
        assert!(other.iter().all(|segment| !segment.highlighted));
    }

    #[test]
    fn notation_treats_an_off_bar_meter_change_as_a_boundary() {
        let map = MeterMap::new([(0, "4/4"), (42, "3/4")], TICKS_PER_BEAT as u32);

        let segments = notation_segments(&note(36, 54), 60, &map, false);

        let starts: Vec<_> = segments.iter().map(|segment| segment.start_beat).collect();
        assert_eq!(starts, vec![3.0, 3.5, 6.5]);
        assert!(segments[1..].iter().all(|segment| segment.tied_from_previous));
    }

    #[test]
    fn staff_rests_match_short_silence_lane_gaps() {
        // At 120 BPM these are 0.125 s and 0.375 s, displayed as 0.1 s
        // and 0.4 s in the editor's Silence lane.
        let map = MeterMap::constant("4/4", TICKS_PER_BEAT as u32);
        let staff = [note(0, 12), note(15, 12), note(36, 12)]
            .iter()
            .flat_map(|n| notation_segments(n, 60, &map, false))
            .collect::<Vec<_>>();
        let bars = MusicScoreBarMap { meter: map, quarter_ticks: TICKS_PER_BEAT as u32 };
        let rests = rests_between_notes(&staff, &bars);
        assert_eq!(
            rests.iter().map(|r| (r.start_beat, r.duration_beats)).collect::<Vec<_>>(),
            vec![(1.0, 0.25), (2.25, 0.75)]
        );
    }
}
