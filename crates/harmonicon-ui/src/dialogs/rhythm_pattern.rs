// SPDX-License-Identifier: MIT

//! Reusable strip of rhythm subdivisions. Callers own selection and timing.

use bevy::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RhythmStep {
    pub label: String,
    pub rest: bool,
    pub accent: bool,
}

pub const ACTIVE_BG: Color = Color::srgba(0.82, 0.62, 0.10, 1.0);

pub const fn step_bg(step: &RhythmStep) -> Color {
    if step.rest {
        Color::srgba(0.16, 0.17, 0.22, 0.85)
    } else if step.accent {
        Color::srgba(0.34, 0.22, 0.42, 0.95)
    } else {
        Color::srgba(0.20, 0.25, 0.34, 0.95)
    }
}

pub fn spawn_rhythm_pattern(
    parent: &mut ChildSpawnerCommands,
    steps: &[RhythmStep],
) -> Vec<Entity> {
    let mut cells = Vec::with_capacity(steps.len());
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(5.0),
            row_gap: Val::Px(5.0),
            max_width: Val::Px(760.0),
            ..default()
        })
        .with_children(|row| {
            for (index, step) in steps.iter().enumerate() {
                let display = if step.rest {
                    format!("{}  —", step.label)
                } else if step.accent {
                    format!("> {}", step.label)
                } else {
                    step.label.clone()
                };
                let cell = row
                    .spawn((
                        Node {
                            min_width: Val::Px(58.0),
                            height: Val::Px(54.0),
                            padding: UiRect::horizontal(Val::Px(10.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(if index == 0 { ACTIVE_BG } else { step_bg(step) }),
                        BorderColor::all(Color::srgb(0.40, 0.40, 0.55)),
                    ))
                    .with_child((
                        Text::new(display),
                        TextFont {
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(if step.rest {
                            Color::srgb(0.62, 0.64, 0.70)
                        } else {
                            Color::WHITE
                        }),
                    ))
                    .id();
                cells.push(cell);
            }
        });
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(rest: bool, accent: bool) -> RhythmStep {
        RhythmStep {
            label: "1".into(),
            rest,
            accent,
        }
    }

    #[test]
    fn rests_accents_and_plain_steps_have_distinct_colours() {
        let plain = step_bg(&step(false, false));
        let rest = step_bg(&step(true, false));
        let accent = step_bg(&step(false, true));
        assert_ne!(plain, rest);
        assert_ne!(plain, accent);
        assert_ne!(rest, accent);
    }

    #[test]
    fn rest_takes_precedence_over_an_accent() {
        assert_eq!(step_bg(&step(true, true)), step_bg(&step(true, false)));
    }
}
