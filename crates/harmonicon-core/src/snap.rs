// SPDX-License-Identifier: MIT

//! Which within-beat tick positions a click on the note grid can land a new
//! note on — split out of `state.rs` purely to stay under its line budget
//! (`docs/physical_design_plan.md`), not because this is a separate feature
//! area; `EditorState::snap_mode` is still state.rs's own field.

use crate::synth::TICKS_PER_BEAT;

/// `TICKS_PER_BEAT` (12) is the lowest resolution divisible by both 4
/// (straight 16ths) and 3 (triplets), so every mode's positions below are
/// exact integer ticks. See [`SnapMode::grid_points`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SnapMode {
    /// Straight 16th notes — ticks 0, 3, 6, 9.
    #[default]
    Sixteenth,
    /// Swung ("shuffle") 8th notes — a 2:1 long-short pair, ticks 0 and 8.
    /// This is the classic blues shuffle feel: play the first and third
    /// notes of an 8th-note triplet, skip the middle one.
    Shuffle,
    /// Straight 8th-note triplets — three equal subdivisions, ticks 0, 4, 8.
    Triplet,
}

impl SnapMode {
    /// The tick offsets (0..`TICKS_PER_BEAT`) this mode allows landing a new
    /// note on, within a single beat.
    pub fn grid_points(self) -> &'static [usize] {
        match self {
            SnapMode::Sixteenth => &[0, 3, 6, 9],
            SnapMode::Shuffle => &[0, 8],
            SnapMode::Triplet => &[0, 4, 8],
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            SnapMode::Sixteenth => "editor-snap-mode-sixteenth",
            SnapMode::Shuffle => "editor-snap-mode-shuffle",
            SnapMode::Triplet => "editor-snap-mode-triplet",
        }
    }

    pub fn next(self) -> SnapMode {
        match self {
            SnapMode::Sixteenth => SnapMode::Shuffle,
            SnapMode::Shuffle => SnapMode::Triplet,
            SnapMode::Triplet => SnapMode::Sixteenth,
        }
    }
}

/// Which visual tier a sub-beat gridline is drawn in — a colour choice
/// (`SongEditorColors`'s `half_line`/`quarter_line`/`triplet_line`), not a
/// distinction the snapping itself makes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridlineKind {
    /// The mid-beat line (tick 6), the strongest sub-beat division.
    Half,
    /// A straight-16th position (ticks 3 and 9).
    Sixteenth,
    /// A triplet position (ticks 4 and 8).
    Triplet,
}

/// The sub-beat gridlines to draw for `mode`: exactly the within-beat ticks
/// it can snap a note to, minus tick 0 — already drawn as the beat/bar line.
///
/// Keyed off the *active* mode rather than drawing every subdivision the
/// resolution can express. The union of the straight-16th and triplet
/// families puts lines at ticks 3, 4, 6, 8 and 9, which leaves two of the
/// six gaps in a beat one tick wide and the rest three ticks wide — a
/// pattern that reads as neither a 2- nor a 3-way division — and half those
/// lines mark positions the active mode cannot land a note on anyway.
pub fn sub_beat_gridlines(mode: SnapMode) -> Vec<(usize, GridlineKind)> {
    mode.grid_points()
        .iter()
        .copied()
        .filter(|&tick| tick != 0)
        .map(|tick| {
            let kind = if tick * 2 == TICKS_PER_BEAT {
                GridlineKind::Half
            } else if tick.is_multiple_of(TICKS_PER_BEAT / 4) {
                GridlineKind::Sixteenth
            } else {
                GridlineKind::Triplet
            };
            (tick, kind)
        })
        .collect()
}

/// The counting syllables the beat ruler prints between one beat number and
/// the next, as (within-beat tick, localization key) pairs.
///
/// "&" and "a" name the same two ticks in every mode, which is what makes a
/// shuffle legible as what it actually is: ticks 0 and 8 of an 8th-note
/// triplet — the "1 … a" of "1 & a", with the "&" left out. Straight 16ths
/// get a syllable at the halfway point only; naming ticks 3 and 9 too would
/// need four labels inside one beat cell.
pub fn off_beat_labels(mode: SnapMode) -> &'static [(usize, &'static str)] {
    match mode {
        SnapMode::Sixteenth => &[(6, "editor-beat-count-and")],
        SnapMode::Shuffle => &[(8, "editor-beat-count-a")],
        SnapMode::Triplet => &[(4, "editor-beat-count-and"), (8, "editor-beat-count-a")],
    }
}

/// Snaps a fractional position within a beat (`0.0..1.0`, e.g. a click's
/// normalized offset across a beat cell) to the nearest tick `mode` allows.
/// Pure so it's unit-testable without spinning up a grid click.
pub fn snap_tick_in_beat(frac: f32, mode: SnapMode) -> usize {
    let raw = frac.clamp(0.0, 0.999) * TICKS_PER_BEAT as f32;
    mode.grid_points()
        .iter()
        .copied()
        .min_by(|&a, &b| {
            (raw - a as f32)
                .abs()
                .partial_cmp(&(raw - b as f32).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0)
}

/// Snaps an *absolute* tick position (unlike [`snap_tick_in_beat`]'s
/// fractional position within a single beat) to the nearest tick `mode`
/// allows, across beat boundaries — used by drag-to-move/-resize so an
/// existing note snaps onto a shuffle/triplet position the same way a new
/// one can. `grid_points()` always includes 0, so the current beat's own
/// points plus the *next* beat's tick 0 are the only candidates that
/// matter.
pub fn snap_absolute_tick(tick: usize, mode: SnapMode) -> usize {
    let beat = tick / TICKS_PER_BEAT;
    let mut best = beat * TICKS_PER_BEAT;
    let mut best_dist = tick - best;
    for &p in mode.grid_points() {
        let candidate = beat * TICKS_PER_BEAT + p;
        let dist = tick.abs_diff(candidate);
        if dist < best_dist {
            best = candidate;
            best_dist = dist;
        }
    }
    let next_beat_start = (beat + 1) * TICKS_PER_BEAT;
    if next_beat_start - tick < best_dist {
        best = next_beat_start;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteenth_mode_reproduces_the_old_any_tick_grid() {
        assert_eq!(snap_tick_in_beat(0.0, SnapMode::Sixteenth), 0);
        assert_eq!(snap_tick_in_beat(0.26, SnapMode::Sixteenth), 3);
        assert_eq!(snap_tick_in_beat(0.5, SnapMode::Sixteenth), 6);
        assert_eq!(snap_tick_in_beat(0.76, SnapMode::Sixteenth), 9);
    }

    #[test]
    fn shuffle_mode_only_lands_on_the_long_short_pair() {
        // grid_points = [0, 8]; the midpoint (raw tick 4, frac 1/3) is where
        // the nearest point flips from 0 to 8.
        assert_eq!(snap_tick_in_beat(0.0, SnapMode::Shuffle), 0);
        assert_eq!(snap_tick_in_beat(0.3, SnapMode::Shuffle), 0);
        assert_eq!(snap_tick_in_beat(0.4, SnapMode::Shuffle), 8);
        assert_eq!(snap_tick_in_beat(0.7, SnapMode::Shuffle), 8);
        assert_eq!(snap_tick_in_beat(0.99, SnapMode::Shuffle), 8);
    }

    #[test]
    fn triplet_mode_lands_on_three_equal_subdivisions() {
        // grid_points = [0, 4, 8]; midpoints at raw ticks 2 and 6.
        assert_eq!(snap_tick_in_beat(0.0, SnapMode::Triplet), 0);
        assert_eq!(snap_tick_in_beat(0.3, SnapMode::Triplet), 4);
        assert_eq!(snap_tick_in_beat(0.5, SnapMode::Triplet), 4);
        assert_eq!(snap_tick_in_beat(0.7, SnapMode::Triplet), 8);
    }

    #[test]
    fn snap_absolute_tick_snaps_within_the_current_beat() {
        // Sixteenth: [0, 3, 6, 9] within beat 0 (ticks 0..12).
        assert_eq!(snap_absolute_tick(0, SnapMode::Sixteenth), 0);
        assert_eq!(snap_absolute_tick(4, SnapMode::Sixteenth), 3);
        assert_eq!(snap_absolute_tick(5, SnapMode::Sixteenth), 6);
        assert_eq!(snap_absolute_tick(10, SnapMode::Sixteenth), 9);
    }

    #[test]
    fn snap_absolute_tick_can_wrap_forward_into_the_next_beat() {
        // Tick 11 (beat 0) is one away from beat 1's own tick 0 (12), but
        // two away from beat 0's own last Sixteenth point (9) -> snaps
        // forward across the beat boundary rather than staying in beat 0.
        assert_eq!(snap_absolute_tick(11, SnapMode::Sixteenth), 12);
        // Sanity: one tick earlier still resolves within beat 0.
        assert_eq!(snap_absolute_tick(10, SnapMode::Sixteenth), 9);
    }

    #[test]
    fn snap_absolute_tick_never_snaps_backward_past_the_beat_it_started_in() {
        // A tick just after a beat boundary is always closer to that beat's
        // own 0 than to the previous beat's last point, for every mode —
        // e.g. tick 13 (beat 1, one tick in) must resolve to 12, not to
        // beat 0's own last Shuffle point (8).
        assert_eq!(snap_absolute_tick(13, SnapMode::Shuffle), 12);
        assert_eq!(snap_absolute_tick(13, SnapMode::Triplet), 12);
    }

    #[test]
    fn snap_absolute_tick_works_in_a_later_beat() {
        // Beat 2 starts at tick 24; Triplet's points there are 24, 28, 32.
        assert_eq!(snap_absolute_tick(29, SnapMode::Triplet), 28);
        assert_eq!(snap_absolute_tick(31, SnapMode::Triplet), 32);
    }

    #[test]
    fn gridlines_are_exactly_the_modes_own_snap_points() {
        // Never the union of both families: a straight-16th chart gets no
        // triplet lines, and a triplet chart gets no 16th lines.
        assert_eq!(
            sub_beat_gridlines(SnapMode::Sixteenth),
            vec![
                (3, GridlineKind::Sixteenth),
                (6, GridlineKind::Half),
                (9, GridlineKind::Sixteenth),
            ]
        );
        assert_eq!(sub_beat_gridlines(SnapMode::Shuffle), vec![(8, GridlineKind::Triplet)]);
        assert_eq!(
            sub_beat_gridlines(SnapMode::Triplet),
            vec![(4, GridlineKind::Triplet), (8, GridlineKind::Triplet)]
        );
    }

    #[test]
    fn no_gridline_marks_a_position_its_mode_cannot_snap_to() {
        for mode in [SnapMode::Sixteenth, SnapMode::Shuffle, SnapMode::Triplet] {
            for (tick, _) in sub_beat_gridlines(mode) {
                assert!(
                    mode.grid_points().contains(&tick),
                    "{mode:?} draws a line at tick {tick}, which it cannot snap to"
                );
                assert_ne!(tick, 0, "tick 0 is the beat line, not a sub-beat line");
            }
        }
    }

    #[test]
    fn a_counting_syllable_always_names_the_same_tick() {
        // The shuffle's single off-beat is the triplet's *third* partial,
        // tick 8 — the same "a" the triplet mode labels there, not an "&".
        let tick_of =
            |mode, key| off_beat_labels(mode).iter().find(|(_, k)| *k == key).map(|(t, _)| *t);
        assert_eq!(tick_of(SnapMode::Sixteenth, "editor-beat-count-and"), Some(6));
        assert_eq!(tick_of(SnapMode::Triplet, "editor-beat-count-and"), Some(4));
        assert_eq!(tick_of(SnapMode::Shuffle, "editor-beat-count-a"), Some(8));
        assert_eq!(tick_of(SnapMode::Triplet, "editor-beat-count-a"), Some(8));
        assert_eq!(tick_of(SnapMode::Shuffle, "editor-beat-count-and"), None);
    }

    #[test]
    fn every_counting_syllable_sits_on_a_snap_point() {
        for mode in [SnapMode::Sixteenth, SnapMode::Shuffle, SnapMode::Triplet] {
            for &(tick, _) in off_beat_labels(mode) {
                assert!(
                    mode.grid_points().contains(&tick),
                    "{mode:?} labels tick {tick}, which it cannot snap to"
                );
            }
        }
    }

    #[test]
    fn cycling_snap_mode_visits_all_three_and_wraps() {
        assert_eq!(SnapMode::Sixteenth.next(), SnapMode::Shuffle);
        assert_eq!(SnapMode::Shuffle.next(), SnapMode::Triplet);
        assert_eq!(SnapMode::Triplet.next(), SnapMode::Sixteenth);
    }
}
