// SPDX-License-Identifier: MIT

//! Clock-independent metronome state shared by gameplay, editors, and lesson
//! widgets. A caller supplies elapsed seconds; this module supplies musical
//! tick and accent semantics.

use bevy::prelude::Resource;

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MetronomeFeel {
    Straight,
    #[default]
    Shuffle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetronomeClock {
    pub elapsed: f64,
    pub running: bool,
    pub last_tick: Option<i64>,
}

impl Default for MetronomeClock {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            running: false,
            last_tick: None,
        }
    }
}

impl MetronomeClock {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn advance(&mut self, delta_seconds: f64, bpm: f64, feel: MetronomeFeel) -> Option<i64> {
        if !self.running {
            return None;
        }
        self.elapsed += delta_seconds.max(0.0);
        let tick = tick_index(self.elapsed, bpm, feel)?;
        if self.last_tick == Some(tick) {
            return None;
        }
        self.last_tick = Some(tick);
        Some(tick)
    }
}

pub const fn is_downbeat(beat: i64, beats_per_bar: f64) -> bool {
    let beats = (beats_per_bar.max(1.0)) as i64;
    beat.rem_euclid(beats) == 0
}

pub const fn tick_index(clock: f64, bpm: f64, feel: MetronomeFeel) -> Option<i64> {
    if clock < 0.0 || bpm <= 0.0 {
        return None;
    }
    let beat_duration = 60.0 / bpm;
    let subdivision = match feel {
        MetronomeFeel::Straight => beat_duration,
        MetronomeFeel::Shuffle => beat_duration / 3.0,
    };
    Some((clock / subdivision).floor() as i64)
}

pub const fn click_for_tick(
    tick: i64,
    beats_per_bar: f64,
    feel: MetronomeFeel,
) -> Option<(bool, f32)> {
    match feel {
        MetronomeFeel::Straight => Some((is_downbeat(tick, beats_per_bar), 1.0)),
        MetronomeFeel::Shuffle => match tick.rem_euclid(3) {
            0 => Some((is_downbeat(tick.div_euclid(3), beats_per_bar), 1.0)),
            2 => Some((false, 0.55)),
            _ => None,
        },
    }
}

/// Zero-based 12-bar position for a metronome subdivision tick.
pub const fn twelve_bar_for_tick(tick: i64, beats_per_bar: usize, feel: MetronomeFeel) -> usize {
    let beat = match feel {
        MetronomeFeel::Straight => tick,
        MetronomeFeel::Shuffle => tick.div_euclid(3),
    };
    let beats_per_bar = if beats_per_bar == 0 { 1 } else { beats_per_bar };
    beat.div_euclid(beats_per_bar as i64).rem_euclid(12) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_clock_emits_each_tick_once_and_pauses() {
        let mut clock = MetronomeClock {
            running: true,
            ..Default::default()
        };
        assert_eq!(clock.advance(0.0, 60.0, MetronomeFeel::Straight), Some(0));
        assert_eq!(clock.advance(0.5, 60.0, MetronomeFeel::Straight), None);
        assert_eq!(clock.advance(0.5, 60.0, MetronomeFeel::Straight), Some(1));
        clock.running = false;
        assert_eq!(clock.advance(2.0, 60.0, MetronomeFeel::Straight), None);
        assert_eq!(clock.elapsed, 1.0);
    }

    #[test]
    fn shuffle_clicks_on_the_beat_and_swung_and() {
        assert_eq!(
            click_for_tick(0, 4.0, MetronomeFeel::Shuffle),
            Some((true, 1.0))
        );
        assert_eq!(click_for_tick(1, 4.0, MetronomeFeel::Shuffle), None);
        assert_eq!(
            click_for_tick(2, 4.0, MetronomeFeel::Shuffle),
            Some((false, 0.55))
        );
    }

    #[test]
    fn twelve_bar_position_advances_and_wraps() {
        assert_eq!(twelve_bar_for_tick(0, 4, MetronomeFeel::Straight), 0);
        assert_eq!(twelve_bar_for_tick(4, 4, MetronomeFeel::Straight), 1);
        assert_eq!(twelve_bar_for_tick(47, 4, MetronomeFeel::Straight), 11);
        assert_eq!(twelve_bar_for_tick(48, 4, MetronomeFeel::Straight), 0);
        assert_eq!(twelve_bar_for_tick(12, 4, MetronomeFeel::Shuffle), 1);
    }
}
