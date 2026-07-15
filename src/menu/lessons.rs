// SPDX-License-Identifier: MIT

//! The Lessons menu: a tab bar of curriculum units over a vertical
//! scrollbox of that unit's lessons, and the per-lesson reader page
//! (instructional body + Start button for chart-backed lessons,
//! Mark-as-Done for instructional-only ones). Discovery/unlock/pass logic
//! lives in `crate::lessons`; this module is only the menu surface.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;
use bevy_fluent::Localization;

use crate::dialogs::button;
use crate::dialogs::tab_bar::{TabSelect, spawn_tab_bar};
use crate::lessons::{AvailableLessons, LessonContext, LessonEntry, PassCriteria, group_by_unit, is_unlocked};
use crate::localization::LocalizationExt;
use crate::profile::{PlayerProfile, record_lesson, save_profile};
use crate::song::SongManifest;
use crate::song::harmonica::Progression;
use crate::theme::LoadedTheme;

use super::{
    AppState, ButtonMaterials, GameplayMode, JamProgression, MenuPage, SelectedSong, spawn_button,
    spawn_menu_root,
};

/// The lesson the reader page shows — set by the list page's buttons right
/// before switching to [`MenuPage::LessonReader`].
#[derive(Resource, Default)]
pub(super) struct SelectedLesson(pub Option<String>);

/// The unit tab currently shown on the list page — an index into
/// [`group_by_unit`]'s order. Persists across visits (returning from a
/// lesson lands back on the same tab); clamped on read so a shrunk lesson
/// set can't leave it dangling.
#[derive(Resource, Default)]
pub(super) struct SelectedUnitIx(pub usize);

/// Fired by the tab bar's `on_select` observer when the user actually
/// switches units — [`repopulate_lesson_list`] reacts to this instead of
/// `resource_changed::<SelectedUnitIx>`, which fires spuriously on the
/// list page's very first frame (the resource's initial value looks
/// "changed" to a run condition that has never evaluated it before) and
/// would otherwise re-populate the scrollbox in the same frame
/// `setup_lessons_menu` already did — despawning rows whose freshly
/// inserted `Text` `dialogs::font_fallback::apply_font_fallback` may not
/// have finished processing yet, which panics ("Entity despawned") when
/// its deferred command applies against the now-recycled entity index.
#[derive(Message)]
pub(super) struct LessonUnitChanged;

/// The scrollbox holding the selected unit's lesson rows, so
/// [`repopulate_lesson_list`] can swap its children when the tab changes.
/// `pub(super)` only because it appears in that system's signature, which
/// `menu::MenuPlugin` names when registering it.
#[derive(Component)]
pub(super) struct LessonListBox;

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
        Some(PassCriteria::ScaleAdherence { threshold }) => Some(
            loc.msg_args("lesson-goal-scale-adherence", &[("pct", pct(*threshold))])
                .into(),
        ),
        Some(PassCriteria::ChordToneAdherence { threshold }) => Some(
            loc.msg_args("lesson-goal-chord-tone-adherence", &[("pct", pct(*threshold))])
                .into(),
        ),
        Some(PassCriteria::PhraseDiscipline { threshold }) => Some(
            loc.msg_args("lesson-goal-phrase-discipline", &[("pct", pct(*threshold))])
                .into(),
        ),
        None if entry.chart_asset_path.is_some() => {
            Some(loc.msg("lesson-goal-finish").into())
        }
        None => None,
    }
}

/// Whether `criteria` routes a lesson into an open jam (`GameplayMode::
/// JamSession`) instead of the ordinary chart pipeline — every criterion
/// judged from `jam_session::ImprovStats` rather than a chart run, not just
/// `ScaleAdherence`. Pure so it's directly unit-testable.
fn is_jam_criteria(criteria: Option<&PassCriteria>) -> bool {
    matches!(
        criteria,
        Some(PassCriteria::ScaleAdherence { .. })
            | Some(PassCriteria::ChordToneAdherence { .. })
            | Some(PassCriteria::PhraseDiscipline { .. })
    )
}

/// Parses a lesson manifest's `progression` field (schema-enforced to
/// `"standard"`/`"quick-change"`/`"minor"` when present) into the
/// `Progression` it names. Absent or unrecognized both fall back to
/// `Standard` — the same "don't let a stale pick linger" default the
/// real-song Jam Session button applies.
pub(super) fn parse_progression(s: Option<&str>) -> Progression {
    match s {
        Some("quick-change") => Progression::QuickChange,
        Some("minor") => Progression::Minor,
        _ => Progression::Standard,
    }
}

// ── Lesson list page ──────────────────────────────────────────────────────────

pub(super) fn setup_lessons_menu(
    mut commands: Commands,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    selected_unit: Res<SelectedUnitIx>,
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

    let units = group_by_unit(&lessons.0);
    if units.is_empty() {
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
        spawn_back_to_play(&mut commands, root, &theme, &btn_mats, &loc);
        return;
    }

    // Unit tabs — switching one only updates `SelectedUnitIx`;
    // `repopulate_lesson_list` reacts and swaps the scrollbox contents.
    let unit_labels: Vec<String> = units
        .iter()
        .map(|(unit, _)| String::from(loc.msg(&format!("lesson-unit-{unit}"))))
        .collect();
    let ix = selected_unit.0.min(units.len() - 1);
    spawn_tab_bar(
        &mut commands,
        root,
        &unit_labels,
        ix,
        |ev: On<TabSelect>,
         mut selected: ResMut<SelectedUnitIx>,
         mut changed: MessageWriter<LessonUnitChanged>| {
            selected.0 = ev.index;
            changed.write(LessonUnitChanged);
        },
    );

    // The selected unit's lessons, in a vertical scrollbox (`ScrollArea`
    // gives wheel scrolling — same pattern as `dialogs::file_dialog`).
    let list = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                width: Val::Px(520.0),
                max_height: Val::Percent(48.0),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.85)),
            LessonListBox,
            ScrollArea,
        ))
        .id();
    commands.entity(root).add_child(list);
    populate_lesson_rows(
        &mut commands,
        list,
        units[ix].1.as_slice(),
        &profile,
        &loc,
    );

    spawn_back_to_play(&mut commands, root, &theme, &btn_mats, &loc);
}

/// Swaps the scrollbox's rows for the newly selected unit's. Runs only
/// while the list page is open and only when a [`LessonUnitChanged`]
/// message says the tab actually changed (see the registration in
/// `menu::MenuPlugin`) — not on resource change-detection, which can't
/// distinguish "the user just switched tabs" from "this run condition has
/// never observed this resource before" (see the doc comment on
/// [`LessonUnitChanged`]).
pub(super) fn repopulate_lesson_list(
    mut changed: MessageReader<LessonUnitChanged>,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    selected_unit: Res<SelectedUnitIx>,
    loc: Res<Localization>,
    list: Query<Entity, With<LessonListBox>>,
    mut commands: Commands,
) {
    if changed.is_empty() {
        return;
    }
    changed.clear();
    let Ok(list) = list.single() else {
        return;
    };
    let units = group_by_unit(&lessons.0);
    if units.is_empty() {
        return;
    }
    let ix = selected_unit.0.min(units.len() - 1);
    commands.entity(list).despawn_related::<Children>();
    populate_lesson_rows(&mut commands, list, units[ix].1.as_slice(), &profile, &loc);
}

/// Fixed width every lesson-list row (locked or not) is spawned at, so a
/// long title never makes its row wider than a short one — matches the
/// list container's own width (`setup_lessons_menu`) minus its padding.
const LESSON_ROW_WIDTH: f32 = 500.0;

/// Font size for a lesson-list row's label (already decorated with any
/// 🔒/✓ prefix or " — locked" suffix — the caller passes the final display
/// string). Bevy doesn't expose glyph metrics before layout, so this is a
/// coarse character-count curve rather than a measured fit — chosen
/// conservatively against the longest label any shipped locale actually
/// produces (a long title plus the locked decoration, in the most verbose
/// locale) so labels stay on one line at `LESSON_ROW_WIDTH` in practice.
const fn lesson_button_font_size(char_count: usize) -> f32 {
    match char_count {
        0..=25 => 17.0,
        26..=38 => 14.5,
        39..=50 => 12.5,
        _ => 11.0,
    }
}

/// One row per lesson of the shown unit: a clickable button opening the
/// reader (✓-prefixed once passed — a passed lesson stays replayable), or a
/// dimmed 🔒 row while its prerequisites aren't met. Under `--features dev`
/// every lesson is treated as unlocked regardless of prerequisites — a dev
/// convenience for jumping straight to any lesson while iterating, not a
/// change to `is_unlocked` itself (which stays a plain prerequisite check,
/// still fully exercised by its own unit tests).
fn populate_lesson_rows(
    commands: &mut Commands,
    list: Entity,
    unit_lessons: &[&LessonEntry],
    profile: &PlayerProfile,
    loc: &Localization,
) {
    let passed = profile.passed_lesson_ids();
    for entry in unit_lessons {
        let unlocked = cfg!(feature = "dev") || is_unlocked(&entry.manifest, &passed);
        let title = String::from(loc.msg(&entry.manifest.title_key));
        let label = if !unlocked {
            format!("\u{1F512} {} \u{2014} {}", title, loc.msg("lesson-locked"))
        } else if passed.contains(&entry.manifest.id.as_str()) {
            format!("\u{2713} {}", title)
        } else {
            title
        };
        let font_size = lesson_button_font_size(label.chars().count());
        if unlocked {
            let id = entry.manifest.id.clone();
            commands.entity(list).with_children(|row| {
                row.spawn_empty().apply_scene(button::sized(
                    &label,
                    LESSON_ROW_WIDTH,
                    font_size,
                    move |_: On<Pointer<Click>>,
                          mut selected: ResMut<SelectedLesson>,
                          mut page: ResMut<NextState<MenuPage>>| {
                        selected.0 = Some(id.clone());
                        page.set(MenuPage::LessonReader);
                    },
                ));
            });
        } else {
            let row = commands
                .spawn((
                    Node {
                        width: Val::Px(LESSON_ROW_WIDTH),
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.08, 0.11, 0.6)),
                ))
                .with_children(|cell| {
                    cell.spawn((
                        Text::new(&label),
                        TextFont {
                            font_size: FontSize::Px(font_size),
                            ..default()
                        },
                        TextColor(Color::srgb(0.42, 0.44, 0.50)),
                        TextLayout {
                            justify: Justify::Center,
                            ..default()
                        },
                    ));
                })
                .id();
            commands.entity(list).add_child(row);
        }
    }
}

fn spawn_back_to_play(
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
        theme,
        btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
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
            let progression = entry.manifest.progression.clone();
            spawn_button(
                &mut commands,
                root,
                &loc.msg("lesson-start"),
                &theme,
                &btn_mats,
                move |_: On<Pointer<Click>>,
                      asset_server: Res<AssetServer>,
                      mut mode: ResMut<GameplayMode>,
                      mut jam_progression: ResMut<JamProgression>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(chart_path.clone()),
                    ));
                    commands.insert_resource(LessonContext {
                        lesson_id: lesson_id.clone(),
                        pass_criteria: criteria.clone(),
                    });
                    // A jam-based lesson (scale-adherence/chord-tone-
                    // adherence/phrase-discipline) is an open jam, not a
                    // chart to play through — see `is_jam_criteria`.
                    if is_jam_criteria(criteria.as_ref()) {
                        *mode = GameplayMode::JamSession;
                        jam_progression.0 = parse_progression(progression.as_deref());
                    } else {
                        *mode = GameplayMode::Play2D;
                    }
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
                &theme,
                &btn_mats,
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
        theme,
        btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::Lessons)
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_jam_criteria ───────────────────────────────────────────────────────

    #[test]
    fn every_jam_based_criterion_routes_into_jam_session() {
        for c in [
            PassCriteria::ScaleAdherence { threshold: 0.1 },
            PassCriteria::ChordToneAdherence { threshold: 0.1 },
            PassCriteria::PhraseDiscipline { threshold: 0.1 },
        ] {
            assert!(is_jam_criteria(Some(&c)));
        }
    }

    #[test]
    fn chart_based_criteria_and_none_stay_on_the_ordinary_pipeline() {
        assert!(!is_jam_criteria(None));
        assert!(!is_jam_criteria(Some(&PassCriteria::Accuracy {
            threshold: 0.5
        })));
        assert!(!is_jam_criteria(Some(&PassCriteria::Technique {
            technique: "bend".into(),
            threshold: 0.5
        })));
    }

    // ── parse_progression ─────────────────────────────────────────────────────

    #[test]
    fn parse_progression_reads_each_known_value() {
        assert_eq!(parse_progression(Some("standard")), Progression::Standard);
        assert_eq!(
            parse_progression(Some("quick-change")),
            Progression::QuickChange
        );
        assert_eq!(parse_progression(Some("minor")), Progression::Minor);
    }

    #[test]
    fn parse_progression_defaults_to_standard_when_absent_or_unknown() {
        assert_eq!(parse_progression(None), Progression::Standard);
        assert_eq!(parse_progression(Some("jazz")), Progression::Standard);
    }

    #[test]
    fn short_titles_use_the_largest_size() {
        assert_eq!(lesson_button_font_size(0), 17.0);
        assert_eq!(lesson_button_font_size(25), 17.0);
    }

    #[test]
    fn size_shrinks_in_steps_as_length_grows() {
        assert_eq!(lesson_button_font_size(26), 14.5);
        assert_eq!(lesson_button_font_size(38), 14.5);
        assert_eq!(lesson_button_font_size(39), 12.5);
        assert_eq!(lesson_button_font_size(50), 12.5);
        assert_eq!(lesson_button_font_size(51), 11.0);
    }

    #[test]
    fn the_longest_shipped_locked_label_still_fits_the_smallest_size() {
        // "🔒 Leer la Rejilla del Blues de 12 Compases — bloqueada" (es-ES,
        // the longest lesson title, decorated the way a locked row actually
        // renders it) — the curve must not fall through to something even
        // this doesn't have a tier for.
        let longest = "\u{1F512} Leer la Rejilla del Blues de 12 Compases \u{2014} bloqueada";
        assert!(lesson_button_font_size(longest.chars().count()) >= 11.0);
    }
}
