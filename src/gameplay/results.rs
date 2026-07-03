// SPDX-License-Identifier: MIT

//! Post-song results screen: the hit breakdown and a letter grade.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::menu::{AppState, ReturnToSongList};
use crate::dialogs::button;
use crate::settings::AudioSettings;

use super::{Score, SongStats, TechniqueStats};

#[derive(Component)]
pub(super) struct ResultsRoot;

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
    score: Res<Score>,
    stats: Res<SongStats>,
    audio: Res<AudioSettings>,
) {
    let acc = accuracy(&stats);
    let g = grade(acc);
    let hits = stats.perfect + stats.good + stats.delayed;
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
                                        ..default()
                },
                TextColor(Color::srgb(0.80, 0.82, 0.90)),
            ));
            // Big grade.
            root.spawn((
                Text::new(g),
                TextFont {
                    font_size: FontSize::Px(120.0),
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
                spawn_stat_row(root, label, value, color);
            }

            // Per-technique accuracy — only techniques the song actually used,
            // so a simple song without bends doesn't show a clutter of "n/a"
            // rows. This is the diagnostic a self-taught player needs: not
            // just "you scored 82%" but "your bends are solid, your overblows
            // need work".
            let technique_rows: Vec<(&str, TechniqueStats)> = [
                ("Normal notes", stats.normal),
                ("Bends", stats.bend),
                ("Vibrato", stats.vibrato),
                ("Wah", stats.wah),
                ("Overblow", stats.overblow),
                ("Overdraw", stats.overdraw),
            ]
            .into_iter()
            .filter(|(_, s)| s.total() > 0)
            .collect();

            if !technique_rows.is_empty() {
                root.spawn((
                    Text::new("By technique"),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(Color::srgb(0.55, 0.58, 0.65)),
                    Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
                ));
                for (label, s) in technique_rows {
                    spawn_technique_row(root, label, s);
                }
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
                row.spawn_empty()
                    .apply_scene(button::default("Retry", on_retry));
                row.spawn_empty()
                    .apply_scene(button::default("Continue", on_continue));
            });
        });
}

fn spawn_text_row(
    parent: &mut ChildSpawnerCommands,

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
                                        ..default()
                },
                TextColor(Color::srgb(0.65, 0.68, 0.75)),
            ));
            row.spawn((
                Text::new(value.to_string()),
                TextFont {
                    font_size: FontSize::Px(18.0),
                                        ..default()
                },
                TextColor(color),
            ));
        });
}

fn spawn_stat_row(
    parent: &mut ChildSpawnerCommands,

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
                                        ..default()
                },
                TextColor(Color::srgb(0.65, 0.68, 0.75)),
            ));
            row.spawn((
                Text::new(format!("{value}")),
                TextFont {
                    font_size: FontSize::Px(18.0),
                                        ..default()
                },
                TextColor(color),
            ));
        });
}

/// One "Bends  18/20  90%" row, color-coded by accuracy: green ≥ 80%,
/// amber ≥ 50%, red below.
fn spawn_technique_row(parent: &mut ChildSpawnerCommands, label: &str, s: TechniqueStats) {
    let Some(acc) = s.accuracy() else { return };
    let color = if acc >= 0.80 {
        Color::srgb(0.45, 1.00, 0.45)
    } else if acc >= 0.50 {
        Color::srgb(0.95, 0.75, 0.30)
    } else {
        Color::srgb(0.95, 0.40, 0.35)
    };
    let value = format!("{}/{}  \u{00B7}  {:.0}%", s.hits, s.total(), acc * 100.0);
    spawn_text_row(parent, label, &value, color);
}

pub(super) fn cleanup(mut commands: Commands, roots: Query<Entity, With<ResultsRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

// ── Dedicated button callbacks ────────────────────────────────────────────────

// Re-enter via SongLoading so the song restarts fresh (asset already loaded →
// resumes immediately).
fn on_retry(_: On<Pointer<Click>>, mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::SongLoading);
}

fn on_continue(
    _: On<Pointer<Click>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    return_to_song_list.0 = true;
    next_state.set(AppState::Menu);
}

/// Escape does the same as Continue: return to the song list.
pub(super) fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    return_to_song_list.0 = true;
    next_state.set(AppState::Menu);
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
            ..Default::default()
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
            ..Default::default()
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
