// SPDX-License-Identifier: MIT

//! Rests for silent spans of the shared staff. Work from the union of
//! sounding intervals so chords and overlapping voices do not create rests.

use super::{MusicScoreBarMap, NotationNote};

/// One conventional rest duration in quarter-note beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NotationRest {
    pub start_beat: f64,
    pub duration_beats: f64,
    pub dots: u8,
}

impl NotationRest {
    pub(super) fn glyph(self) -> &'static str {
        match self.duration_beats {
            d if d >= 4.0 => "\u{E4E3}", // restWhole
            d if d >= 2.0 => "\u{E4E4}", // restHalf
            d if d >= 1.0 => "\u{E4E5}", // restQuarter
            d if d >= 0.5 => "\u{E4E6}", // rest8th
            _ => "\u{E4E7}",             // rest16th
        }
    }

    /// The whole rest hangs from the fourth line; the other rest glyphs
    /// use the middle line as their SMuFL origin.
    pub(super) fn staff_step(self) -> i32 {
        if self.duration_beats >= 4.0 { 6 } else { 4 }
    }

    /// A rest must fit before the next rhythmic position. Bravura's
    /// `glyphBBoxes` give the rightmost points as 282/282/270/247/320
    /// units for whole through sixteenth rests (1000 units per em).
    /// A dotted rest extends farther right than its main glyph.
    pub(super) fn required_pixels_per_beat(self, staff_line_spacing: f32) -> f32 {
        let right_units = match self.duration_beats {
            d if d >= 4.0 => 282.0,
            d if d >= 2.0 => 282.0,
            d if d >= 1.0 => 270.0,
            d if d >= 0.5 => 247.0,
            _ => 320.0,
        };
        let glyph_right = right_units / 1000.0 * (4.0 * staff_line_spacing);
        let right =
            if self.dots > 0 { glyph_right.max(1.85 * staff_line_spacing) } else { glyph_right };
        (right + staff_line_spacing) / self.duration_beats as f32
    }

    pub(super) fn fits_slot(self, pixels_per_beat: f32, staff_line_spacing: f32) -> bool {
        pixels_per_beat >= self.required_pixels_per_beat(staff_line_spacing)
    }
}

const VALUES: &[(f64, u8)] =
    &[(4.0, 0), (3.0, 1), (2.0, 0), (1.5, 1), (1.0, 0), (0.75, 1), (0.5, 0), (0.25, 0)];

/// Rests before and between notes, ending at the final sounded note.
/// If a gap crosses a bar line, leave the earlier bar's tail blank and
/// begin rests at the next bar's start. An entirely silent intermediate
/// bar still gets its own rest. Gaps shorter than a sixteenth are below
/// the staff's engraving resolution and are left unmarked.
pub fn rests_between_notes(notes: &[NotationNote], bars: &MusicScoreBarMap) -> Vec<NotationRest> {
    let mut intervals: Vec<(f64, f64)> = notes
        .iter()
        .filter_map(|note| {
            let end = note.start_beat + note.duration_beats;
            (note.start_beat.is_finite() && end.is_finite() && end > note.start_beat)
                .then_some((note.start_beat, end))
        })
        .collect();
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let Some(_) = intervals.first() else {
        return Vec::new();
    };
    let phase = bars.meter.segments()[0].phase_ticks as f64 / bars.quarter_ticks.max(1) as f64;
    let mut cursor = phase;
    let mut rests = Vec::new();
    for (start, end) in intervals {
        if start > cursor {
            let mut at = cursor;
            let mut boundaries = bars
                .beats(cursor, start)
                .into_iter()
                .filter(|&boundary| boundary > cursor && boundary <= start);
            if let Some(first_boundary) = boundaries.next() {
                // The note in the previous bar ended before its line. Do
                // not fill that bar's trailing silence with rest symbols.
                at = first_boundary;
                for boundary in boundaries {
                    spell_gap(at, boundary, &mut rests);
                    at = boundary;
                }
            }
            spell_gap(at, start, &mut rests);
        }
        cursor = cursor.max(end);
    }
    rests
}

fn spell_gap(mut at: f64, end: f64, out: &mut Vec<NotationRest>) {
    while end - at >= 0.25 - 1e-6 {
        let Some(&(duration, dots)) = VALUES.iter().find(|(value, _)| *value <= end - at + 1e-6)
        else {
            break;
        };
        out.push(NotationRest { start_beat: at, duration_beats: duration, dots });
        at += duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_score::MeterMap;

    fn note(start: f64, duration: f64) -> NotationNote {
        NotationNote {
            start_beat: start,
            duration_beats: duration,
            midi: 60,
            tied_from_previous: false,
            highlighted: false,
        }
    }

    #[test]
    fn leading_and_internal_silence_get_rests() {
        let bars = MusicScoreBarMap::default();
        let rests = rests_between_notes(&[note(1.0, 1.0), note(3.0, 1.0)], &bars);
        assert_eq!(
            rests.iter().map(|r| (r.start_beat, r.duration_beats)).collect::<Vec<_>>(),
            vec![(0.0, 1.0), (2.0, 1.0)]
        );
    }

    #[test]
    fn chords_and_overlapping_notes_leave_no_false_rest() {
        let bars = MusicScoreBarMap::default();
        let rests = rests_between_notes(
            &[note(0.0, 2.0), note(0.0, 1.0), note(1.5, 1.0), note(3.0, 1.0)],
            &bars,
        );
        assert_eq!(rests, vec![NotationRest { start_beat: 2.5, duration_beats: 0.5, dots: 0 }]);
    }

    #[test]
    fn a_gap_across_bars_skips_the_first_bars_tail() {
        let bars = MusicScoreBarMap { meter: MeterMap::constant("4/4", 12), quarter_ticks: 12 };
        let rests = rests_between_notes(&[note(0.0, 1.0), note(6.0, 1.0)], &bars);
        assert_eq!(
            rests.iter().map(|r| (r.start_beat, r.duration_beats)).collect::<Vec<_>>(),
            vec![(4.0, 2.0)]
        );
    }

    #[test]
    fn a_note_on_the_next_downbeat_needs_no_trailing_rest() {
        let bars = MusicScoreBarMap::default();
        assert!(rests_between_notes(&[note(0.0, 1.0), note(4.0, 1.0)], &bars).is_empty());
    }

    #[test]
    fn an_entire_silent_bar_still_has_a_whole_rest() {
        let bars = MusicScoreBarMap::default();
        let rests = rests_between_notes(&[note(0.0, 1.0), note(9.0, 1.0)], &bars);
        assert_eq!(
            rests.iter().map(|r| (r.start_beat, r.duration_beats)).collect::<Vec<_>>(),
            vec![(4.0, 4.0), (8.0, 1.0)]
        );
    }

    #[test]
    fn pickup_starts_at_its_actual_beat() {
        let bars = MusicScoreBarMap {
            meter: MeterMap::with_pickup([(0, "4/4")], 12, 12),
            quarter_ticks: 12,
        };
        let rests = rests_between_notes(&[note(3.5, 0.5)], &bars);
        assert_eq!(rests, vec![NotationRest { start_beat: 3.0, duration_beats: 0.5, dots: 0 }]);
    }

    #[test]
    fn tied_continuations_are_continuous_and_no_tail_is_invented() {
        let bars = MusicScoreBarMap::default();
        assert!(rests_between_notes(&[note(0.0, 4.0), note(4.0, 1.0)], &bars).is_empty());
    }

    #[test]
    fn a_sixteenth_rest_is_hidden_when_its_glyph_would_reach_the_next_note() {
        let rest = NotationRest { start_beat: 2.0, duration_beats: 0.25, dots: 0 };
        assert!(!rest.fits_slot(34.0, 9.0));
        assert!(!rest.fits_slot(68.0, 9.0));
        assert!(rest.fits_slot(rest.required_pixels_per_beat(9.0).ceil(), 9.0));
        let quarter = NotationRest { duration_beats: 1.0, ..rest };
        assert!(quarter.fits_slot(34.0, 9.0));
    }
}
