// SPDX-License-Identifier: MIT
//! Where the staff's bar lines fall, and how a note is tied across them.

use super::{MeterMap, NotationNote};
use bevy::prelude::*;

/// Bar grid for the same score as [`MusicScoreNotes`](super::MusicScoreNotes). Both callers provide
/// their chart's meter map; gameplay uses the expanded performance chart.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct MusicScoreBarMap {
    pub meter: MeterMap,
    pub quarter_ticks: u32,
}

impl Default for MusicScoreBarMap {
    fn default() -> Self {
        Self { meter: MeterMap::constant("4/4", 12), quarter_ticks: 12 }
    }
}

impl MusicScoreBarMap {
    /// The bar boundaries in staff quarter-note beats. The first segment's
    /// phase puts a pickup at the end of its otherwise silent first bar.
    pub fn beats(&self, from: f64, to: f64) -> Vec<f64> {
        if self.quarter_ticks == 0 || to < from {
            return Vec::new();
        }
        let phase = self.meter.segments()[0].phase_ticks as f64;
        let q = self.quarter_ticks as f64;
        let start = ((from * q - phase).ceil().max(0.0)) as u64;
        let end = ((to * q - phase).floor().max(0.0)) as u64;
        self.meter
            .bar_starts(start, end.saturating_add(1))
            .into_iter()
            .filter(|&(tick, _)| tick > 0)
            .map(|(tick, _)| (tick as f64 + phase) / q)
            .collect()
    }

    /// Turns a tick-based note into staff segments, tying it at every bar
    /// boundary, including a meter change or a repeated passage boundary.
    pub fn split_note(
        &self,
        start: u64,
        end: u64,
        midi: u8,
        highlighted: bool,
    ) -> Vec<NotationNote> {
        let end = end.max(start.saturating_add(1));
        let phase = self.meter.segments()[0].phase_ticks as f64;
        let q = self.quarter_ticks.max(1) as f64;
        let mut boundaries = self.meter.bar_starts(start.saturating_add(1), end);
        boundaries.push((end, 0));
        let mut previous = start;
        boundaries
            .into_iter()
            .enumerate()
            .map(|(index, (tick, _))| {
                let note = NotationNote {
                    spelling: None,
                    start_beat: (previous as f64 + phase) / q,
                    duration_beats: (tick - previous) as f64 / q,
                    midi,
                    tied_from_previous: index > 0,
                    highlighted,
                };
                previous = tick;
                note
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn bar_lines_follow_meter_changes_and_do_not_duplicate_the_opening() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::new([(0, "4/4"), (48, "3/4")], 12),
            quarter_ticks: 12,
        };
        assert_eq!(bars.beats(0.0, 11.0), vec![4.0, 7.0, 10.0]);
    }

    #[test]
    fn pickup_and_window_edges_keep_the_bar_line_at_the_right_beat() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::with_pickup([(0, "4/4")], 12, 12),
            quarter_ticks: 12,
        };
        assert_eq!(bars.beats(0.0, 8.0), vec![4.0, 8.0]);
        assert_eq!(bars.beats(4.0, 4.0), vec![4.0]);
    }

    #[test]
    fn a_mid_bar_meter_change_is_a_line_and_a_tie_boundary() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::new([(0, "4/4"), (42, "3/4")], 12),
            quarter_ticks: 12,
        };
        assert_eq!(bars.beats(0.0, 7.0), vec![3.5, 6.5]);
        let parts = bars.split_note(36, 54, 60, false);
        assert_eq!(parts.iter().map(|n| n.start_beat).collect::<Vec<_>>(), vec![3.0, 3.5]);
        assert!(parts[1].tied_from_previous);
    }

    #[test]
    fn a_short_pause_widens_the_staff_until_its_rest_fits() {
        let bars = MusicScoreBarMap::default();
        let notes = vec![
            NotationNote {
                spelling: None,
                start_beat: 0.0,
                duration_beats: 1.0,
                midi: 60,
                tied_from_previous: false,
                highlighted: false,
            },
            NotationNote {
                spelling: None,
                start_beat: 1.25,
                duration_beats: 1.0,
                midi: 62,
                tied_from_previous: false,
                highlighted: true,
            },
        ];
        let rests = rests_between_notes(&notes, &bars);
        assert_eq!(rests.len(), 1);
        let scale = score_pixels_per_beat(&notes, &bars);
        assert!(scale > 68.0);
        assert!(rests[0].fits_slot(scale, STAFF_LINE_SPACING));
    }
}
