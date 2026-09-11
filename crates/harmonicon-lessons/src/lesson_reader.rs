// SPDX-License-Identifier: MIT

//! One lesson's page: the instructional body, a Start button for
//! chart-backed lessons, Mark-as-Done for instructional-only ones, and the
//! row of five training tiers under it. Reached from a node of
//! `lesson_tree`, which is the only way in — the curriculum used to
//! have a flat list page here as well, and now has one home per lesson.
//!
//! Discovery/unlock/pass logic lives in `harmonicon_song::lessons`; this
//! module is only the menu surface.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_fluent::Localization;

use harmonicon_app::profile::{PlayerProfile, record_lesson, save_profile, training_key};
use harmonicon_core::chart::Scale;
use harmonicon_core::harmonica::{Position, Progression};
use harmonicon_core::pitch_map::{HarpKind, harp_for_key};
use harmonicon_core::training::{Tier, drill_chart};
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::lessons::training_criteria;
use harmonicon_song::lessons::{AvailableLessons, LessonContext, LessonEntry, PassCriteria};
use harmonicon_song::song::{SongManifest, training_manifest};
use harmonicon_ui::dialogs::circle_of_fifths::spawn_circle_of_fifths;

use harmonicon_app::app::{
    AppState, GameplayMode, GeneratedSong, JamPositionCycle, JamProgression, JamScale, SelectedSong,
};
use harmonicon_menu::menu::MenuPage;
use harmonicon_menu::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

/// The lesson this page shows — set by a skill-tree node right before it
/// switches to [`MenuPage::LessonReader`].
#[derive(Resource, Default)]
pub(crate) struct SelectedLesson(pub Option<String>);

/// Looks a lesson up by id. The tree always sets [`SelectedLesson`] before
/// opening this page, so a miss only happens if something desyncs — the
/// reader degrades to an empty page with a Back button rather than
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
            loc.msg_args(
                "lesson-goal-chord-tone-adherence",
                &[("pct", pct(*threshold))],
            )
            .into(),
        ),
        Some(PassCriteria::PhraseDiscipline { threshold }) => Some(
            loc.msg_args("lesson-goal-phrase-discipline", &[("pct", pct(*threshold))])
                .into(),
        ),
        None if entry.chart_asset_path.is_some() => Some(loc.msg("lesson-goal-finish").into()),
        None => None,
    }
}

/// Whether `criteria` routes a lesson into an open jam (`GameplayMode::
/// JamSession`) instead of the ordinary chart pipeline — every criterion
/// judged from `jam::improv::ImprovStats` rather than a chart run, not just
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
/// `"standard"`/`"quick-change"`/`"minor"`/`"jazz-blues"` when present) into
/// the `Progression` it names. Absent or unrecognized both fall back to
/// `Standard` — the same "don't let a stale pick linger" default the
/// real-song Jam Session button applies.
pub(crate) fn parse_progression(s: Option<&str>) -> Progression {
    match s {
        Some("quick-change") => Progression::QuickChange,
        Some("minor") => Progression::Minor,
        Some("jazz-blues") => Progression::JazzBlues,
        _ => Progression::Standard,
    }
}

/// Parses a lesson manifest's `scale` field (schema-enforced to
/// `"first-position"`/`"second-position"`/`"third-position"`/`"major"`/
/// `"minor-pentatonic"`/`"country"` when present) into the `Scale` it
/// names. Absent or unrecognized both fall back to `FirstPosition` (the
/// blues hexatonic) — same "don't let a stale pick linger" reasoning as
/// [`parse_progression`].
pub(crate) fn parse_scale(s: Option<&str>) -> Scale {
    match s {
        Some("second-position") => Scale::SecondPosition,
        Some("third-position") => Scale::ThirdPosition,
        Some("major") => Scale::Major,
        Some("minor-pentatonic") => Scale::MinorPentatonic,
        Some("country") => Scale::Country,
        _ => Scale::FirstPosition,
    }
}

/// The lesson's five training tiers, as a row of buttons under Start.
///
/// A lesson with no `training` block gets nothing — that is the honest
/// answer for anything instructional-only, where the microphone cannot
/// verify the technique and five drills would only pretend to check it.
///
/// Tiers are **not gated on each other**. Practising tier 4 before tier 2
/// is the player's business, and locking them would remove the one choice
/// the ladder offers; the label carries the tick so progress is still
/// visible.
fn spawn_training_row(
    commands: &mut Commands,
    root: Entity,
    entry: &LessonEntry,
    profile: &PlayerProfile,
    loc: &Localization,
) {
    if entry.manifest.training.is_none() {
        return;
    }
    let lesson_id = entry.manifest.id.clone();
    spawn_reader_line(
        commands,
        root,
        String::from(loc.msg_args(
            "lesson-training-heading",
            &[(
                "percent",
                ((profile.mastery(&lesson_id, Tier::ALL.len()) * 100.0).round() as u32).to_string(),
            )],
        )),
        Color::srgb(0.80, 0.82, 0.90),
    );

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(row);

    for tier in Tier::ALL {
        let done = profile
            .trainings
            .get(&training_key(&lesson_id, tier.number()))
            .is_some_and(|r| r.passed);
        let label = if done {
            format!("\u{2713} {}", tier.number())
        } else {
            tier.number().to_string()
        };
        let spec = entry.manifest.drill_spec(tier);
        let criteria = entry.manifest.pass_criteria.clone();
        let id = lesson_id.clone();
        spawn_button(
            commands,
            row,
            &label,
            move |_: On<Activate>,
                  mut manifests: ResMut<Assets<SongManifest>>,
                  mut mode: ResMut<GameplayMode>,
                  mut state: ResMut<NextState<AppState>>,
                  mut commands: Commands| {
                let Some(spec) = spec.clone() else { return };
                let harp = harp_for_key("C", HarpKind::Diatonic);
                let Some(chart) = drill_chart(&spec, &harp, &id, "") else {
                    // No hole in the lesson can do the technique on this
                    // harp. Nothing to play, so stay put rather than open a
                    // drill of plain notes that trains nothing.
                    return;
                };
                commands.insert_resource(SelectedSong(manifests.add(training_manifest(chart))));
                commands.insert_resource(LessonContext {
                    lesson_id: id.clone(),
                    pass_criteria: Some(training_criteria(criteria.as_ref(), tier)),
                    aural: false,
                    tier: Some(tier.number()),
                });
                // Built by `Assets::add`, so it has no `LoadState` and
                // `SongLoading` would wait on it forever — see
                // `app::GeneratedSong`.
                commands.insert_resource(GeneratedSong);
                *mode = GameplayMode::Play2D;
                state.set(AppState::Playing);
            },
        );
    }
}

/// One plain text line appended directly to `root` (no card/box around
/// it) — the shared shape the lesson reader's goal-progress line and its
/// "Passed" badge both use, differing only in text/color.
fn spawn_reader_line(commands: &mut Commands, root: Entity, text: String, color: Color) {
    let line = commands
        .spawn((
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(color),
        ))
        .id();
    commands.entity(root).add_child(line);
}

// ── Lesson reader page ────────────────────────────────────────────────────────

pub(crate) fn setup_lesson_reader(
    mut commands: Commands,
    selected: Res<SelectedLesson>,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let entry = selected
        .0
        .as_deref()
        .and_then(|id| find_lesson(&lessons, id));

    let title = entry
        .map(|e| String::from(loc.msg(&e.manifest.title_key)))
        .unwrap_or_default();
    let (root, header, _page_root) =
        spawn_menu_root(&mut commands, &title, None, &theme, "Lessons");

    let Some(entry) = entry else {
        spawn_back_to_tree(&mut commands, header, &loc);
        return;
    };

    // Instructional body — width-capped so long text wraps like a page
    // rather than spanning the whole window, on a dark translucent card so
    // it stays readable over a busy background image.
    let body = commands
        .spawn((
            Node {
                max_width: Val::Px(760.0),
                margin: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.95)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(String::from(loc.msg(&entry.manifest.body_key))),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.82, 0.84, 0.90)),
            ));
        })
        .id();
    commands.entity(root).add_child(body);

    // Embedded reference diagram, if this lesson declares one — spawned
    // right under the body text, above the goal/pass lines.
    if entry.manifest.diagram.as_deref() == Some("circle-of-fifths") {
        commands.entity(root).with_children(|parent| {
            spawn_circle_of_fifths(
                parent,
                "C",
                Position::all(),
                theme.circle_of_fifths_colors(),
            );
        });
    }

    if let Some(goal) = goal_line(&loc, entry) {
        spawn_reader_line(&mut commands, root, goal, Color::srgb(0.85, 0.72, 0.35));
    }

    let record = profile.lessons.get(&entry.manifest.id);
    if record.is_some_and(|r| r.passed) {
        spawn_reader_line(
            &mut commands,
            root,
            format!("\u{2713} {}", loc.msg("lesson-passed")),
            Color::srgb(0.45, 0.95, 0.50),
        );
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
            let scale = entry.manifest.scale.clone();
            let position_cycle = entry.manifest.position_cycle;
            let aural = entry.manifest.aural;
            spawn_button(
                &mut commands,
                root,
                &loc.msg("lesson-start"),
                move |_: On<Activate>,
                      asset_server: Res<AssetServer>,
                      mut mode: ResMut<GameplayMode>,
                      mut jam_progression: ResMut<JamProgression>,
                      mut jam_scale: ResMut<JamScale>,
                      mut jam_position_cycle: ResMut<JamPositionCycle>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(chart_path.clone()),
                    ));
                    commands.insert_resource(LessonContext {
                        lesson_id: lesson_id.clone(),
                        pass_criteria: criteria.clone(),
                        aural,
                        tier: None,
                    });
                    // A jam-based lesson (scale-adherence/chord-tone-
                    // adherence/phrase-discipline) is an open jam, not a
                    // chart to play through — see `is_jam_criteria`.
                    if is_jam_criteria(criteria.as_ref()) {
                        *mode = GameplayMode::JamSession;
                        jam_progression.0 = parse_progression(progression.as_deref());
                        jam_scale.0 = parse_scale(scale.as_deref());
                        jam_position_cycle.0 = position_cycle;
                    } else {
                        *mode = GameplayMode::Play2D;
                    }
                    state.set(AppState::SongLoading);
                },
            );
            spawn_training_row(&mut commands, root, entry, &profile, &loc);
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
                move |_: On<Activate>,
                      mut profile: ResMut<PlayerProfile>,
                      mut page: ResMut<NextState<MenuPage>>| {
                    let record = profile.lessons.entry(lesson_id.clone()).or_default();
                    record_lesson(record, true, 0.0);
                    save_profile(&profile);
                    // Back to the tree, which re-spawns with this node
                    // passed and anything it unlocked now lit.
                    page.set(MenuPage::LessonTree);
                },
            );
        }
        None => {}
    }

    spawn_back_to_tree(&mut commands, header, &loc);
}

fn spawn_back_to_tree(commands: &mut Commands, header: Entity, loc: &Localization) {
    spawn_back_button(
        commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::LessonTree),
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
        assert_eq!(
            parse_progression(Some("jazz-blues")),
            Progression::JazzBlues
        );
    }

    #[test]
    fn parse_progression_defaults_to_standard_when_absent_or_unknown() {
        assert_eq!(parse_progression(None), Progression::Standard);
        assert_eq!(parse_progression(Some("jazz")), Progression::Standard);
    }

    // ── parse_scale ────────────────────────────────────────────────────────────

    #[test]
    fn parse_scale_reads_each_known_value() {
        assert_eq!(parse_scale(Some("first-position")), Scale::FirstPosition);
        assert_eq!(parse_scale(Some("second-position")), Scale::SecondPosition);
        assert_eq!(parse_scale(Some("third-position")), Scale::ThirdPosition);
        assert_eq!(parse_scale(Some("major")), Scale::Major);
        assert_eq!(
            parse_scale(Some("minor-pentatonic")),
            Scale::MinorPentatonic
        );
        assert_eq!(parse_scale(Some("country")), Scale::Country);
    }

    #[test]
    fn parse_scale_defaults_to_first_position_when_absent_or_unknown() {
        assert_eq!(parse_scale(None), Scale::FirstPosition);
        assert_eq!(parse_scale(Some("dorian")), Scale::FirstPosition);
    }
}
