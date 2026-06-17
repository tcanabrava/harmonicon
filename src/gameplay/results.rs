// SPDX-License-Identifier: MIT

//! Post-song results screen: the hit breakdown and a letter grade.

use bevy::prelude::*;

use crate::assets_management::GlobalFonts;
use crate::menu::{AppState, ReturnToSongList};

use crate::settings::AudioSettings;

use super::{Score, SongStats};

#[derive(Component)]
pub(super) struct ResultsRoot;

#[derive(Component, Clone, Copy)]
pub(super) enum ResultsButton {
    Retry,
    Continue,
}

/// Mean timing offset in milliseconds over all hits.
/// Positive = player sounds notes after the target even with current compensation;
/// increase `input_latency_ms` by this value to re-centre the window.
/// Returns `None` when there are no hits to average.
pub fn mean_offset_ms(stats: &SongStats) -> Option<f64> {
    let hits = stats.perfect + stats.good + stats.delayed;
    if hits == 0 {
        return None;
    }
    Some(stats.offset_sum / hits as f64 * 1000.0)
}

/// Weighted accuracy in 0..1 from the hit tally (perfect counts full, good less,
/// a late "delayed" hit least). Empty songs grade as 0.
pub fn accuracy(stats: &SongStats) -> f32 {
    let total = stats.perfect + stats.good + stats.delayed + stats.miss;
    if total == 0 {
        return 0.0;
    }
    let weighted = stats.perfect as f32 + stats.good as f32 * 0.7 + stats.delayed as f32 * 0.45;
    weighted / total as f32
}

/// Letter grade for a 0..1 accuracy, A+ down to F.
pub fn grade(accuracy: f32) -> &'static str {
    match accuracy {
        a if a >= 0.95 => "A+",
        a if a >= 0.88 => "A",
        a if a >= 0.78 => "B",
        a if a >= 0.65 => "C",
        a if a >= 0.50 => "D",
        _ => "F",
    }
}

fn grade_color(grade: &str) -> Color {
    match grade {
        "A+" | "A" => Color::srgb(0.35, 0.95, 0.45),
        "B" => Color::srgb(0.40, 0.85, 0.95),
        "C" => Color::srgb(0.95, 0.85, 0.30),
        "D" => Color::srgb(0.95, 0.55, 0.25),
        _ => Color::srgb(0.95, 0.30, 0.30),
    }
}

pub(super) fn setup(
    mut commands: Commands,
    fonts: Res<GlobalFonts>,
    score: Res<Score>,
    stats: Res<SongStats>,
    audio: Res<AudioSettings>,
) {
    let acc = accuracy(&stats);
    let g = grade(acc);
    let hits = stats.perfect + stats.good + stats.delayed;
    let font = fonts.gameplay.clone();
    let mean_ms = mean_offset_ms(&stats);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.04, 0.07)),
            GlobalZIndex(300),
            ResultsRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("SONG COMPLETE"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.80, 0.82, 0.90)),
            ));
            // Big grade.
            root.spawn((
                Text::new(g),
                TextFont {
                    font_size: FontSize::Px(120.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(grade_color(g)),
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
            ));

            // Stat lines.
            let rows = [
                (
                    "Biggest combo",
                    score.max_combo,
                    Color::srgb(0.90, 0.72, 0.20),
                ),
                ("Perfect hits", stats.perfect, Color::srgb(1.00, 0.85, 0.20)),
                ("Good hits", stats.good, Color::srgb(0.45, 1.00, 0.45)),
                ("Hits", hits, Color::srgb(0.75, 0.85, 0.95)),
                ("Delayed hits", stats.delayed, Color::srgb(0.95, 0.62, 0.30)),
                ("Misses", stats.miss, Color::srgb(0.95, 0.35, 0.35)),
            ];
            for (label, value, color) in rows {
                spawn_stat_row(root, &font, label, value, color);
            }

            // Timing offset row + calibration hint.
            if let Some(ms) = mean_ms {
                let sign = if ms >= 0.0 { "+" } else { "" };
                let offset_color = if ms.abs() < 10.0 {
                    Color::srgb(0.45, 1.00, 0.45) // green: well calibrated
                } else {
                    Color::srgb(0.95, 0.62, 0.30) // orange: needs adjustment
                };
                spawn_text_row(
                    root,
                    &font,
                    "Avg timing offset",
                    &format!("{sign}{ms:.0}ms"),
                    offset_color,
                );

                let adjustment = ms.round() as i32;
                let new_latency = (audio.input_latency_ms + adjustment).max(0);
                if adjustment.abs() >= 5 {
                    let hint = if adjustment > 0 {
                        format!("→ Increase Input lag to {}ms in Options", new_latency)
                    } else {
                        format!("→ Decrease Input lag to {}ms in Options", new_latency)
                    };
                    root.spawn((
                        Text::new(hint),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            font: font.clone(),
                            ..default()
                        },
                        TextColor(Color::srgb(0.60, 0.65, 0.75)),
                    ));
                }
            }

            // Final score.
            root.spawn((
                Text::new(format!("Score: {}", score.points)),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
            ));

            // Retry / Continue buttons.
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                margin: UiRect::top(Val::Px(18.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_results_button(row, &font, "Retry", ResultsButton::Retry);
                spawn_results_button(row, &font, "Continue", ResultsButton::Continue);
            });
        });
}

fn spawn_results_button(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
    label: &str,
    kind: ResultsButton,
) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(180.0),
                padding: UiRect::axes(Val::Px(28.0), Val::Px(12.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            kind,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_text_row(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
    label: &str,
    value: &str,
    color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Px(320.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.68, 0.75)),
            ));
            row.spawn((
                Text::new(value.to_string()),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(color),
            ));
        });
}

fn spawn_stat_row(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
    label: &str,
    value: u32,
    color: Color,
) {
    parent
        .spawn(Node {
            width: Val::Px(320.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.68, 0.75)),
            ));
            row.spawn((
                Text::new(format!("{value}")),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(color),
            ));
        });
}

pub(super) fn cleanup(mut commands: Commands, roots: Query<Entity, With<ResultsRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

pub(super) fn handle_buttons(
    buttons: Query<(&Interaction, &ResultsButton), Changed<Interaction>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            // Re-enter via SongLoading so the song restarts fresh (asset already
            // loaded → resumes immediately).
            ResultsButton::Retry => next_state.set(AppState::SongLoading),
            ResultsButton::Continue => {
                return_to_song_list.0 = true;
                next_state.set(AppState::Menu);
            }
        }
    }
}

pub(super) fn button_hover(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ResultsButton>),
    >,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = BackgroundColor(match interaction {
            Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
            Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
            Interaction::None => Color::srgb(0.14, 0.14, 0.22),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(perfect: u32, good: u32, delayed: u32, miss: u32) -> SongStats {
        SongStats {
            perfect,
            good,
            delayed,
            miss,
            offset_sum: 0.0,
        }
    }

    fn stats_with_offset(
        perfect: u32,
        good: u32,
        delayed: u32,
        miss: u32,
        offset_sum: f64,
    ) -> SongStats {
        SongStats {
            perfect,
            good,
            delayed,
            miss,
            offset_sum,
        }
    }

    #[test]
    fn all_perfect_is_a_plus() {
        let acc = accuracy(&stats(20, 0, 0, 0));
        assert!((acc - 1.0).abs() < 1e-6);
        assert_eq!(grade(acc), "A+");
    }

    #[test]
    fn all_misses_is_f() {
        let acc = accuracy(&stats(0, 0, 0, 20));
        assert_eq!(acc, 0.0);
        assert_eq!(grade(acc), "F");
    }

    #[test]
    fn empty_song_does_not_panic_and_grades_f() {
        let acc = accuracy(&stats(0, 0, 0, 0));
        assert_eq!(acc, 0.0);
        assert_eq!(grade(acc), "F");
    }

    #[test]
    fn grade_thresholds() {
        assert_eq!(grade(0.96), "A+");
        assert_eq!(grade(0.90), "A");
        assert_eq!(grade(0.80), "B");
        assert_eq!(grade(0.70), "C");
        assert_eq!(grade(0.55), "D");
        assert_eq!(grade(0.40), "F");
    }

    #[test]
    fn delayed_hits_count_less_than_good() {
        let good = accuracy(&stats(0, 10, 0, 0));
        let delayed = accuracy(&stats(0, 0, 10, 0));
        assert!(good > delayed);
    }

    // ── mean_offset_ms ────────────────────────────────────────────────────────

    #[test]
    fn no_hits_yields_none() {
        assert_eq!(mean_offset_ms(&stats(0, 0, 0, 5)), None);
    }

    #[test]
    fn perfectly_centred_hits_give_zero_offset() {
        // 10 hits, offset_sum = 0.0 → mean = 0 ms
        let s = stats_with_offset(10, 0, 0, 0, 0.0);
        let ms = mean_offset_ms(&s).unwrap();
        assert!(ms.abs() < 1e-6, "expected ~0, got {ms}");
    }

    #[test]
    fn positive_offset_sum_reports_late_mean() {
        // 5 hits at +50 ms each (offset_sum = 5 * 0.05 = 0.25 s)
        let s = stats_with_offset(5, 0, 0, 0, 0.25);
        let ms = mean_offset_ms(&s).unwrap();
        assert!((ms - 50.0).abs() < 1e-6, "expected +50, got {ms}");
    }

    #[test]
    fn misses_do_not_dilute_the_mean() {
        // 4 hits at +40 ms each, 6 misses
        let s = stats_with_offset(4, 0, 0, 6, 0.16);
        let ms = mean_offset_ms(&s).unwrap();
        assert!((ms - 40.0).abs() < 1e-4, "expected +40, got {ms}");
    }
}
