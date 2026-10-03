// SPDX-License-Identifier: MIT

//! Post-song results screen, read as coaching: accuracy and the one thing
//! worth working on lead, then the grade and score, then the evidence —
//! the hit tally, timing as an early/on-time/late split, and the technique
//! rows ranked by where practice would pay off. Every judgment shown comes
//! from `coaching`'s pure functions; this file only lays them out.

use bevy::input_focus::tab_navigation::TabGroup;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{AppState, ReturnToSongList, SelectedSong};
use harmonicon_app::profile::{
    PlayerProfile, record_lesson, record_play, record_training, save_profile, training_key,
};
use harmonicon_audio::AudioSettings;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::lessons::{LessonContext, PassCriteria, lesson_passed};
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::button;

use super::adaptive_difficulty::{
    AdaptiveDifficulty, bump_learned_sections, learned_vec_from_map, write_learned_into_map,
};
use super::coaching::{
    Observation, latency_suggestion, lesson_progress, mean_offset_ms, missed_range, observation,
    ranked_techniques, technique_buckets,
};
use super::state::{PracticeRange, PracticeRequest};
use super::{Score, ScoringConfig, SongNotes, SongStats, TechniqueStats};

/// Bars of the song "Practice missed section" loops around the densest
/// cluster of misses.
const PRACTICE_WINDOW_BARS: f64 = 2.0;
/// Beats of lead-in before the first missed note, so the loop doesn't
/// start on the attack itself.
const PRACTICE_LEAD_IN_BEATS: f64 = 1.0;

#[derive(Component)]
pub(super) struct ResultsRoot;

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

/// Localization key for a technique bucket's display name, from the
/// profile-vocabulary name `coaching::technique_buckets` uses.
fn technique_key(name: &str) -> &'static str {
    match name {
        "bend" => "results-technique-bend",
        "vibrato" => "results-technique-vibrato",
        "wah-wah" => "results-technique-wah",
        "overblow" => "results-technique-overblow",
        "overdraw" => "results-technique-overdraw",
        "slide" => "results-technique-slide",
        "clean-attack" => "results-technique-clean-attack",
        _ => "results-technique-normal",
    }
}

/// The observation as one localized sentence.
fn observation_text(loc: &Localization, obs: Observation) -> String {
    let pct = |share: f32| format!("{:.0}", share * 100.0);
    match obs {
        Observation::Technique { technique, hits, total } => loc
            .msg_args(
                "results-observation-technique",
                &[
                    ("technique", loc.msg(technique_key(technique)).to_string()),
                    ("hits", hits.to_string()),
                    ("total", total.to_string()),
                ],
            )
            .into(),
        Observation::MissedNotes { misses, total } => loc
            .msg_args(
                "results-observation-missed",
                &[("misses", misses.to_string()), ("total", total.to_string())],
            )
            .into(),
        Observation::Timing { late: true, share } => {
            loc.msg_args("results-observation-late", &[("pct", pct(share))]).into()
        }
        Observation::Timing { late: false, share } => {
            loc.msg_args("results-observation-early", &[("pct", pct(share))]).into()
        }
        Observation::LeakyAttacks { clean, total } => loc
            .msg_args(
                "results-observation-leaky",
                &[("clean", clean.to_string()), ("total", total.to_string())],
            )
            .into(),
        Observation::Solid => loc.msg("results-observation-solid").into(),
    }
}

/// The lesson's goal and how far this run got, as two localized lines —
/// the goal wording is the lesson reader's own, so the two screens agree.
fn lesson_goal_lines(
    loc: &Localization,
    criteria: Option<&PassCriteria>,
    acc: f32,
    technique_accuracy: &[(&str, f32)],
) -> Option<(String, String)> {
    let (reached, goal) = lesson_progress(criteria, acc, technique_accuracy)?;
    let pct = |t: f32| format!("{:.0}", t * 100.0);
    let goal_line = match criteria? {
        PassCriteria::Technique { technique, .. } => loc.msg_args(
            "lesson-goal-technique",
            &[("pct", pct(goal)), ("technique", technique.clone())],
        ),
        _ => loc.msg_args("lesson-goal-accuracy", &[("pct", pct(goal))]),
    };
    let reached_line = loc.msg_args("results-lesson-reached", &[("pct", pct(reached))]);
    Some((goal_line.into(), reached_line.into()))
}

/// The loop range "Practice missed section" would enter, from the run's
/// missed notes: two bars around the densest cluster, one beat of lead-in.
fn practice_range(notes: &SongNotes, config: &ScoringConfig, bpm: f64) -> Option<PracticeRange> {
    let misses: Vec<(f64, f64)> =
        notes.notes.iter().filter(|n| n.missed).map(|n| (n.time, n.time + n.duration)).collect();
    let window = config.meter.bar_secs(bpm) * PRACTICE_WINDOW_BARS;
    let lead_in = config.meter.beat_secs(bpm) * PRACTICE_LEAD_IN_BEATS;
    missed_range(&misses, window, lead_in)
        .map(|(start_time, end_time)| PracticeRange { start_time, end_time })
}

pub(super) fn setup(
    mut commands: Commands,
    score: Res<Score>,
    stats: Res<SongStats>,
    audio: Res<AudioSettings>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut profile: ResMut<PlayerProfile>,
    song_notes: Res<SongNotes>,
    config: Res<ScoringConfig>,
    adaptive: Res<AdaptiveDifficulty>,
    lesson: Option<Res<LessonContext>>,
    loc: Res<Localization>,
) {
    let acc = accuracy(&stats);
    let g = grade(acc);
    // Any finished run is a practice day, pass or fail; saved with the
    // run's own record below.
    let today = harmonicon_platform::calendar::today();
    profile.practice.record(today);
    let technique_accuracy: Vec<(&str, f32)> = technique_buckets(&stats)
        .into_iter()
        .filter_map(|(name, s)| s.accuracy().map(|a| (name, a)))
        .collect();

    // A lesson run is judged against its pass criteria and recorded under
    // the lesson's own id — it deliberately does *not* touch the per-song
    // best/adaptive records below (a lesson chart isn't a song the best-
    // scores screen should list, and its learned-fraction is meaningless).
    let lesson_result = lesson.as_ref().map(|ctx| {
        // A chart-backed run through Results never accumulates jam scale
        // data — only the jam pause menu's "Finish Lesson" button does
        // (see `PassCriteria::ScaleAdherence`).
        let passed = lesson_passed(ctx.pass_criteria.as_ref(), acc, &technique_accuracy, None);
        match ctx.tier {
            // A training is practice, not evidence the lesson was learned:
            // recording it under `lessons` would satisfy prerequisites and
            // silently unlock everything downstream.
            Some(tier) => {
                let key = training_key(&ctx.lesson_id, tier);
                let record = profile.trainings.entry(key).or_default();
                record_training(record, passed, acc, today);
            }
            None => {
                let record = profile.lessons.entry(ctx.lesson_id.clone()).or_default();
                record_lesson(record, passed, acc);
            }
        }
        save_profile(&profile);
        let goal = lesson_goal_lines(&loc, ctx.pass_criteria.as_ref(), acc, &technique_accuracy);
        (passed, goal)
    });

    // Record this play against the song's persisted best — keyed by the
    // manifest's own path (stable across restarts, unlike the `Handle` in
    // `SelectedSong`), so repeated plays only ever improve what's shown here,
    // never regress it because of one worse run. Saved immediately (not
    // debounced) so quitting right after still keeps the new best.
    let manifest = manifests.get(&selected.0);
    let new_best = if lesson.is_some() {
        None
    } else {
        manifest.map(|manifest| {
            let key = manifest.path.display().to_string();
            let record = profile.songs.entry(key).or_default();
            let improved = record_play(record, score.points, acc, &technique_accuracy);
            let mut learned = learned_vec_from_map(&adaptive.sections, &record.phrase_learned);
            bump_learned_sections(&song_notes.notes, adaptive.sections.len(), &mut learned);
            write_learned_into_map(&adaptive.sections, &learned, &mut record.phrase_learned);
            let best_score = record.best_score;
            save_profile(&profile);
            (improved, best_score)
        })
    };

    let practice = manifest
        .and_then(|m| practice_range(&song_notes, &config, f64::from(m.chart.song.tempo_bpm)));
    let obs = observation(&stats);
    let latency_fix = latency_suggestion(&stats);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.04, 0.07)),
            GlobalZIndex(300),
            TabGroup::default(),
            ResultsRoot,
        ))
        .with_children(|root| {
            root.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("results-song-complete"))})
                TextFont { font_size: {FontSize::Px(28.0)} }
                TextColor({Color::srgb(0.80, 0.82, 0.90)})
            });

            // A lesson's verdict and its goal come first: they're what the
            // run was for, and the song score below is incidental to them.
            if let Some((passed, goal)) = &lesson_result {
                let (key, color) = if *passed {
                    ("lesson-complete-banner", Color::srgb(0.45, 0.95, 0.50))
                } else {
                    ("lesson-failed-banner", Color::srgb(0.95, 0.62, 0.30))
                };
                root.spawn_empty().apply_scene(bsn! {
                    Text({String::from(loc.msg(key))})
                    TextFont { font_size: {FontSize::Px(22.0)} }
                    TextColor({color})
                });
                if let Some((goal_line, reached_line)) = goal {
                    spawn_caption(root, goal_line, Color::srgb(0.75, 0.78, 0.85));
                    spawn_caption(root, reached_line, color);
                }
            }

            // Accuracy leads, with the observation right under it — the two
            // things a player should take away before the grade catches
            // the eye.
            root.spawn_empty().apply_scene(bsn! {
                Text({format!("{:.0}%", acc * 100.0)})
                TextFont { font_size: {FontSize::Px(72.0)} }
                TextColor({grade_color(g)})
                Node { margin: {UiRect::top(Val::Px(6.0))} }
            });
            spawn_caption(
                root,
                &loc.msg("results-accuracy-caption"),
                Color::srgb(0.55, 0.58, 0.65),
            );
            if let Some(obs) = obs {
                root.spawn_empty().apply_scene(bsn! {
                    Text({observation_text(&loc, obs)})
                    TextFont { font_size: {FontSize::Px(19.0)} }
                    TextColor({Color::srgb(0.95, 0.85, 0.45)})
                    ~{TextLayout::justify(Justify::Center)}
                    Node {
                        max_width: {Val::Px(620.0)},
                        margin: {UiRect::vertical(Val::Px(4.0))},
                    }
                });
            }

            // Grade and score together, one line.
            root.spawn_empty().apply_scene(bsn! {
                Node {
                    flex_direction: {FlexDirection::Row},
                    align_items: {AlignItems::Baseline},
                    column_gap: {Val::Px(18.0)},
                    margin: {UiRect::vertical(Val::Px(4.0))},
                }
                Children [
                    Text({g})
                    TextFont { font_size: {FontSize::Px(48.0)} }
                    TextColor({grade_color(g)})
                    --
                    Text({String::from(loc.msg_args(
                        "results-score",
                        &[("points", score.points.to_string())],
                    ))})
                    TextFont { font_size: {FontSize::Px(22.0)} }
                    TextColor({Color::WHITE})
                ]
            });

            // Persisted best for this song — always shown once known, with a
            // callout when this run just raised it.
            if let Some((improved, best_score)) = new_best {
                if improved {
                    root.spawn_empty().apply_scene(bsn! {
                        Text({String::from(loc.msg("results-new-best"))})
                        TextFont { font_size: {FontSize::Px(18.0)} }
                        TextColor({Color::srgb(0.95, 0.85, 0.20)})
                    });
                } else {
                    spawn_stat_row(
                        root,
                        &loc.msg("results-best-score"),
                        best_score,
                        Color::srgb(0.70, 0.72, 0.80),
                    );
                }
            }

            // The tally. No "Hits" row: it's the sum of the three hit kinds
            // and reads as a fourth category beside them.
            let rows = [
                ("results-biggest-combo", score.max_combo, Color::srgb(0.90, 0.72, 0.20)),
                ("results-perfect-hits", stats.perfect, Color::srgb(1.00, 0.85, 0.20)),
                ("results-good-hits", stats.good, Color::srgb(0.45, 1.00, 0.45)),
                ("results-delayed-hits", stats.delayed, Color::srgb(0.95, 0.62, 0.30)),
                ("results-misses", stats.miss, Color::srgb(0.95, 0.35, 0.35)),
            ];
            for (key, value, color) in rows {
                spawn_stat_row(root, &loc.msg(key), value, color);
            }

            // Timing as a distribution: the early / on-time / late split
            // drawn as a bar, the mean as a caption. The Input-lag fix is
            // offered only when the distribution is actually lopsided
            // (`latency_suggestion`), not merely off-centre on average.
            if let Some(ms) = mean_offset_ms(&stats) {
                let sign = if ms >= 0.0 { "+" } else { "" };
                spawn_section_heading(
                    root,
                    &loc.msg_args("results-timing", &[("ms", format!("{sign}{ms:.0}"))]),
                );
                spawn_timing_bar(root, &loc, &stats);
                if let Some(adjustment) = latency_fix {
                    let new_latency = (audio.input_latency_ms + adjustment).max(0);
                    let key = if adjustment > 0 {
                        "results-increase-latency"
                    } else {
                        "results-decrease-latency"
                    };
                    let label = loc.msg_args(key, &[("ms", new_latency.to_string())]);
                    root.spawn_empty().apply_scene(button::small(
                        &label,
                        move |_: On<Activate>, mut audio: ResMut<AudioSettings>| {
                            audio.input_latency_ms = new_latency;
                        },
                    ));
                }
            }

            // Per-technique accuracy, most practice-worthy first, only for
            // techniques the song actually used — a simple song without
            // bends doesn't show a clutter of "n/a" rows. Sample counts stay
            // on every row so a 0/1 can't read as a trend.
            let technique_rows = ranked_techniques(&stats);
            if !technique_rows.is_empty() {
                spawn_section_heading(root, &loc.msg("results-by-technique"));
                for (name, s) in technique_rows {
                    spawn_technique_row(root, &loc.msg(technique_key(name)), s);
                }
            }

            // Retry / Practice missed section / Continue.
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                margin: UiRect::top(Val::Px(14.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn_empty().apply_scene(button::default(&loc.msg("results-retry"), on_retry));
                if let Some(range) = practice {
                    row.spawn_empty().apply_scene(button::default(
                        &loc.msg("results-practice-missed"),
                        move |_: On<Activate>,
                              mut practice: ResMut<PracticeRequest>,
                              mut next_state: ResMut<NextState<AppState>>| {
                            practice.0 = Some(range);
                            next_state.set(AppState::SongLoading);
                        },
                    ));
                }
                row.spawn_empty()
                    .apply_scene(button::default(&loc.msg("results-continue"), on_continue));
            });
        });
}

fn spawn_caption(parent: &mut ChildSpawnerCommands, text: &str, color: Color) {
    parent.spawn_empty().apply_scene(bsn! {
        Text({text.to_string()})
        TextFont { font_size: {FontSize::Px(16.0)} }
        TextColor({color})
    });
}

fn spawn_section_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn_empty().apply_scene(bsn! {
        Text({text.to_string()})
        TextFont { font_size: {FontSize::Px(15.0)} }
        TextColor({Color::srgb(0.55, 0.58, 0.65)})
        Node { margin: {UiRect::top(Val::Px(6.0))} }
    });
}

fn spawn_text_row(parent: &mut ChildSpawnerCommands, label: &str, value: &str, color: Color) {
    parent.spawn_empty().apply_scene(bsn! {
        Node {
            width: {Val::Px(320.0)},
            flex_direction: {FlexDirection::Row},
            justify_content: {JustifyContent::SpaceBetween},
        }
        Children [
            Text({label.to_string()})
            TextFont { font_size: {FontSize::Px(18.0)} }
            TextColor({Color::srgb(0.65, 0.68, 0.75)})
            --
            Text({value.to_string()})
            TextFont { font_size: {FontSize::Px(18.0)} }
            TextColor({color})
        ]
    });
}

fn spawn_stat_row(parent: &mut ChildSpawnerCommands, label: &str, value: u32, color: Color) {
    spawn_text_row(parent, label, &format!("{value}"), color);
}

const EARLY_COLOR: Color = Color::srgb(0.40, 0.70, 0.95);
const ON_TIME_COLOR: Color = Color::srgb(0.45, 1.00, 0.45);
const LATE_COLOR: Color = Color::srgb(0.95, 0.62, 0.30);

/// The early / on-time / late split as one 320 px bar — each segment's
/// width is its share of hits — with the three counts labelled beneath in
/// the same colours.
fn spawn_timing_bar(parent: &mut ChildSpawnerCommands, loc: &Localization, stats: &SongStats) {
    let counts = [
        ("results-timing-early", stats.timing.early(), EARLY_COLOR),
        ("results-timing-on-time", stats.timing.on_time(), ON_TIME_COLOR),
        ("results-timing-late", stats.timing.late(), LATE_COLOR),
    ];
    parent
        .spawn(Node {
            width: Val::Px(320.0),
            height: Val::Px(10.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|bar| {
            for (_, count, color) in counts {
                if count == 0 {
                    continue;
                }
                bar.spawn_empty().apply_scene(bsn! {
                    Node {
                        flex_grow: {count as f32},
                        height: {Val::Percent(100.0)},
                    }
                    BackgroundColor({color})
                });
            }
        });
    parent
        .spawn(Node {
            width: Val::Px(320.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            for (key, count, color) in counts {
                row.spawn_empty().apply_scene(bsn! {
                    Text({String::from(loc.msg_args(key, &[("n", count.to_string())]))})
                    TextFont { font_size: {FontSize::Px(15.0)} }
                    TextColor({color})
                });
            }
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
fn on_retry(_: On<Activate>, mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::SongLoading);
}

fn on_continue(
    _: On<Activate>,
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
        SongStats { perfect, good, delayed, miss, ..Default::default() }
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

    #[test]
    fn every_technique_bucket_has_a_display_key() {
        for (name, _) in technique_buckets(&SongStats::default()) {
            assert!(technique_key(name).starts_with("results-technique-"));
        }
        assert_ne!(technique_key("bend"), technique_key("normal"));
    }
}
