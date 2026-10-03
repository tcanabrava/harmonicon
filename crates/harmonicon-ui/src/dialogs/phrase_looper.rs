// SPDX-License-Identifier: MIT

//! Reusable A/B phrase strip and its caller-driven loop clock.

use bevy::prelude::*;

pub const ACTIVE_BG: Color = Color::srgba(0.82, 0.62, 0.10, 1.0);
pub const CELL_BG: Color = Color::srgba(0.20, 0.25, 0.34, 0.95);

fn phrase_strip_node() -> Node {
    Node {
        width: Val::Percent(100.0),
        max_width: Val::Px(760.0),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(5.0),
        row_gap: Val::Px(5.0),
        ..default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhraseLoopClock {
    elapsed: f64,
    current: usize,
    pub running: bool,
}

impl PhraseLoopClock {
    pub const fn current(&self) -> usize {
        self.current
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.current = 0;
    }

    /// Advances a phrase whose cells each last `beats_per_step` beats.
    /// Returns the selected cell only when a boundary is crossed.
    pub fn advance(
        &mut self,
        delta_seconds: f64,
        bpm: f64,
        beats_per_step: f64,
        step_count: usize,
    ) -> Option<usize> {
        if !self.running || step_count == 0 || bpm <= 0.0 || beats_per_step <= 0.0 {
            return None;
        }
        let seconds_per_step = 60.0 * beats_per_step / bpm;
        self.elapsed += delta_seconds.max(0.0);
        if self.elapsed < seconds_per_step {
            return None;
        }
        let crossed = (self.elapsed / seconds_per_step).floor() as usize;
        self.elapsed %= seconds_per_step;
        self.current = (self.current + crossed) % step_count;
        Some(self.current)
    }

    pub fn step(&mut self, delta: i32, step_count: usize) -> usize {
        if step_count == 0 {
            self.current = 0;
        } else {
            self.current = (self.current as i32 + delta).rem_euclid(step_count as i32) as usize;
        }
        self.elapsed = 0.0;
        self.current
    }
}

pub fn spawn_phrase_looper(parent: &mut ChildSpawnerCommands, steps: &[String]) -> Vec<Entity> {
    let mut cells = Vec::with_capacity(steps.len());
    parent.spawn(phrase_strip_node()).with_children(|row| {
        for (index, label) in steps.iter().enumerate() {
            let boundary = match (index, index + 1 == steps.len()) {
                (0, true) => "A/B",
                (0, false) => "A",
                (_, true) => "B",
                _ => "",
            };
            let cell = row
                .spawn_empty()
                .apply_scene(bsn! {
                    Node {
                        min_width: {Val::Px(82.0)},
                        height: {Val::Px(58.0)},
                        padding: {UiRect::horizontal(Val::Px(10.0))},
                        flex_direction: {FlexDirection::Column},
                        align_items: {AlignItems::Center},
                        justify_content: {JustifyContent::Center},
                        border: {UiRect::all(Val::Px(1.0))},
                    }
                    BackgroundColor({if index == 0 { ACTIVE_BG } else { CELL_BG }})
                    ~{BorderColor::all(Color::srgb(0.40, 0.40, 0.55))}
                    Children [
                        Text({label.clone()})
                        TextFont { font_size: {FontSize::Px(19.0)} }
                        TextColor({Color::WHITE})
                        --
                        Text({boundary})
                        TextFont { font_size: {FontSize::Px(12.0)} }
                        TextColor({Color::srgb(0.68, 0.70, 0.76)})
                    ]
                })
                .id();
            cells.push(cell);
        }
    });
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_wraps_from_b_to_a() {
        let mut clock = PhraseLoopClock { running: true, ..default() };
        assert_eq!(clock.advance(0.5, 120.0, 1.0, 3), Some(1));
        assert_eq!(clock.advance(0.5, 120.0, 1.0, 3), Some(2));
        assert_eq!(clock.advance(0.5, 120.0, 1.0, 3), Some(0));
    }

    #[test]
    fn stopped_clock_does_not_advance() {
        let mut clock = PhraseLoopClock::default();
        assert_eq!(clock.advance(4.0, 60.0, 1.0, 4), None);
        assert_eq!(clock.current(), 0);
    }

    #[test]
    fn manual_step_wraps_and_resets_fractional_time() {
        let mut clock = PhraseLoopClock { running: true, ..default() };
        assert_eq!(clock.advance(0.25, 60.0, 1.0, 4), None);
        assert_eq!(clock.step(-1, 4), 3);
        assert_eq!(clock.advance(0.75, 60.0, 1.0, 4), None);
    }

    #[test]
    fn phrase_strip_uses_available_width_and_wraps() {
        let node = phrase_strip_node();
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.max_width, Val::Px(760.0));
        assert_eq!(node.flex_wrap, FlexWrap::Wrap);
    }
}
