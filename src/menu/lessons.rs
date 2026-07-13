// SPDX-License-Identifier: MIT

//! The Lessons menu: a unit-grouped curriculum list, and the per-lesson
//! reader page (instructional body + Start button for chart-backed lessons,
//! Mark-as-Done for instructional-only ones). Discovery/unlock/pass logic
//! lives in `crate::lessons`; this module is only the menu surface.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::lessons::{AvailableLessons, LessonContext, LessonEntry, PassCriteria, group_by_unit, is_unlocked};
use crate::localization::LocalizationExt;
use crate::profile::{PlayerProfile, record_lesson, save_profile};
use crate::song::SongManifest;
use crate::theme::LoadedTheme;

use super::{
    AppState, ButtonMaterials, GameplayMode, MenuPage, SelectedSong, spawn_button,
    spawn_menu_root,
};

/// The lesson the reader page shows — set by the list page's buttons right
/// before switching to [`MenuPage::LessonReader`].
#[derive(Resource, Default)]
pub(super) struct SelectedLesson(pub Option<String>);

/// Looks a lesson up by id. The list page always sets [`SelectedLesson`]
/// before opening the reader, so a miss only happens if something desyncs —
/// the reader degrades to an empty page with a Back button rather than
/// panicking.
fn find_lesson<'a>(lessons: &'a AvailableLessons, id: &str) -> Option<&'a LessonEntry> {
    lessons.0.iter().find(|l| l.manifest.id == id)
}

/// One localized "Goal: ..." line for a lesson's pass criteria, or the
/// finish-to-pass wording when it has none but is still playable.
fn goal_line(loc: &Localization, entry: &LessonEntry) -> Option<String> {
    let pct = |t: f32| format!("{:.0}", t * 100.0);
    match &entry.manifest.pass_criteria {
        Some(PassCriteria::Accuracy { threshold }) => Some(
            loc.msg_args("lesson-goal-accuracy", &[("pct", pct(*threshold))])
                .into(),
        ),
        Some(PassCriteria::Technique {
            technique,
            threshold,
        }) => Some(
            loc.msg_args(
                "lesson-goal-technique",
                &[("pct", pct(*threshold)), ("technique", technique.clone())],
            )
            .into(),
        ),
        None if entry.chart_asset_path.is_some() => {
            Some(loc.msg("lesson-goal-finish").into())
        }
        None => None,
    }
}

// ── Lesson list page ──────────────────────────────────────────────────────────

pub(super) fn setup_lessons_menu(
    mut commands: Commands,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("menu-lessons"),
        None,
        &theme,
        "Lessons",
    );

    if lessons.0.is_empty() {
        let msg = commands
            .spawn((
                Text::new(String::from(loc.msg("no-lessons-found"))),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.4, 0.4)),
            ))
            .id();
        commands.entity(root).add_child(msg);
    }

    let passed = profile.passed_lesson_ids();
    for (unit, unit_lessons) in group_by_unit(&lessons.0) {
        // Unit heading — localized via the manifest-declared unit id.
        let heading = commands
            .spawn((
                Text::new(String::from(loc.msg(&format!("lesson-unit-{unit}")))),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.72, 0.35)),
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(root).add_child(heading);

        for entry in unit_lessons {
            let title = String::from(loc.msg(&entry.manifest.title_key));
            let lesson_passed = passed.contains(&entry.manifest.id.as_str());
            if is_unlocked(&entry.manifest, &passed) {
                // ✓ marks a passed lesson; both states stay clickable — a
                // passed lesson can always be replayed.
                let label = if lesson_passed {
                    format!("\u{2713} {title}")
                } else {
                    title
                };
                let id = entry.manifest.id.clone();
                spawn_button(
                    &mut commands,
                    root,
                    &label,
                    None,
                    &theme,
                    &btn_mats,
                    "Lessons",
                    move |_: On<Pointer<Click>>,
                          mut selected: ResMut<SelectedLesson>,
                          mut page: ResMut<NextState<MenuPage>>| {
                        selected.0 = Some(id.clone());
                        page.set(MenuPage::LessonReader);
                    },
                );
            } else {
                // Locked: a dimmed, non-clickable row naming what unlocks it.
                let row = commands
                    .spawn((
                        Text::new(format!(
                            "\u{1F512} {title} \u{2014} {}",
                            loc.msg("lesson-locked")
                        )),
                        TextFont {
                            font_size: FontSize::Px(17.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.42, 0.44, 0.50)),
                    ))
                    .id();
                commands.entity(root).add_child(row);
            }
        }
    }

    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        Some("BackToMain"),
        &theme,
        &btn_mats,
        "Lessons",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}

// ── Lesson reader page ────────────────────────────────────────────────────────

pub(super) fn setup_lesson_reader(
    mut commands: Commands,
    selected: Res<SelectedLesson>,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let entry = selected
        .0
        .as_deref()
        .and_then(|id| find_lesson(&lessons, id));

    let title = entry
        .map(|e| String::from(loc.msg(&e.manifest.title_key)))
        .unwrap_or_default();
    let root = spawn_menu_root(&mut commands, &title, None, &theme, "Lessons");

    let Some(entry) = entry else {
        spawn_back_to_lessons(&mut commands, root, &theme, &btn_mats, &loc);
        return;
    };

    // Instructional body — width-capped so long text wraps like a page
    // rather than spanning the whole window.
    let body = commands
        .spawn((
            Text::new(String::from(loc.msg(&entry.manifest.body_key))),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.82, 0.84, 0.90)),
            Node {
                max_width: Val::Px(760.0),
                margin: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(body);

    if let Some(goal) = goal_line(&loc, entry) {
        let goal_row = commands
            .spawn((
                Text::new(goal),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.72, 0.35)),
            ))
            .id();
        commands.entity(root).add_child(goal_row);
    }

    let record = profile.lessons.get(&entry.manifest.id);
    if record.is_some_and(|r| r.passed) {
        let done = commands
            .spawn((
                Text::new(format!("\u{2713} {}", loc.msg("lesson-passed"))),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.95, 0.50)),
            ))
            .id();
        commands.entity(root).add_child(done);
    }

    match &entry.chart_asset_path {
        // Chart-backed lesson: Start launches the chart through the normal
        // song pipeline, with a LessonContext so results judge the pass
        // criteria (and adaptive difficulty leaves every note unlocked).
        Some(chart_path) => {
            let chart_path = chart_path.clone();
            let lesson_id = entry.manifest.id.clone();
            let criteria = entry.manifest.pass_criteria.clone();
            spawn_button(
                &mut commands,
                root,
                &loc.msg("lesson-start"),
                None,
                &theme,
                &btn_mats,
                "Lessons",
                move |_: On<Pointer<Click>>,
                      asset_server: Res<AssetServer>,
                      mut mode: ResMut<GameplayMode>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(chart_path.clone()),
                    ));
                    commands.insert_resource(LessonContext {
                        lesson_id: lesson_id.clone(),
                        pass_criteria: criteria.clone(),
                    });
                    *mode = GameplayMode::Play2D;
                    state.set(AppState::SongLoading);
                },
            );
        }
        // Instructional-only lesson: nothing to score — reading it and
        // saying "done" is the pass (see docs/lessons_plan.md on what's
        // honestly verifiable). Hidden once passed.
        None if !record.is_some_and(|r| r.passed) => {
            let lesson_id = entry.manifest.id.clone();
            spawn_button(
                &mut commands,
                root,
                &loc.msg("lesson-mark-done"),
                None,
                &theme,
                &btn_mats,
                "Lessons",
                move |_: On<Pointer<Click>>,
                      mut profile: ResMut<PlayerProfile>,
                      mut page: ResMut<NextState<MenuPage>>| {
                    let record = profile.lessons.entry(lesson_id.clone()).or_default();
                    record_lesson(record, true, 0.0);
                    save_profile(&profile);
                    // Back to the list, which re-spawns with the new ✓ (and
                    // any newly unlocked lessons).
                    page.set(MenuPage::Lessons);
                },
            );
        }
        None => {}
    }

    spawn_back_to_lessons(&mut commands, root, &theme, &btn_mats, &loc);
}

fn spawn_back_to_lessons(
    commands: &mut Commands,
    root: Entity,
    theme: &LoadedTheme,
    btn_mats: &ButtonMaterials,
    loc: &Localization,
) {
    spawn_button(
        commands,
        root,
        &loc.msg("back"),
        Some("BackToLessons"),
        theme,
        btn_mats,
        "Lessons",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::Lessons)
        },
    );
}
