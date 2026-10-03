// SPDX-License-Identifier: MIT

//! The "Warm-up" review queue at the top of the skill tree: a few passed
//! trainings due for another go (`docs/training_tree_plan.md` §4).
//!
//! **Spacing, not repetition.** Each training tier carries a review date
//! (`profile::TrainingRecord::review_due`) that moves further out every
//! time a review holds — see `profile::record_training` for the schedule.
//! A skill practised at widening intervals is retained far better than one
//! drilled in a single sitting, and this queue is what brings each one
//! back at the right moment.
//!
//! **One review per lesson, at its highest passed tier.** Reviewing the
//! first tier of a ladder the player has climbed to the top says nothing
//! about whether the skill held; the hardest tier they have passed does.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{AppState, GameplayMode};
use harmonicon_app::profile::{PlayerProfile, training_key};
use harmonicon_core::training::Tier;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::lessons::LessonEntry;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::button;

use super::layout::PlacedNode;
use super::placement::PartKind;
use super::{BADGE_PX, ClusterMember, LayoutOwner};
use crate::lesson_reader::training::{start_training, tier_name_key};

/// The warm-up row's gold, so a marked node reads as one of its reviews.
const DUE_COLOR: Color = Color::srgb(0.95, 0.82, 0.45);

/// How many reviews the queue shows at once. More than a handful stops
/// being a warm-up and becomes a to-do list.
pub(crate) const WARMUP_LIMIT: usize = 3;

/// One review due today or earlier.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Warmup {
    /// Index into the lesson catalogue.
    pub entry: usize,
    pub tier: Tier,
    /// The day it fell due; earlier means more overdue.
    pub due: u32,
}

/// The warm-up queue: the most overdue [`WARMUP_LIMIT`] of the reviews
/// [`due_reviews`] found.
pub(crate) fn warmups(due: &[Warmup]) -> &[Warmup] {
    &due[..due.len().min(WARMUP_LIMIT)]
}

/// Every review due on day `today`: each lesson's highest passed tier whose
/// review date has come, most overdue first, then in catalogue order. The
/// warm-up row shows the first few; every one of them marks its node.
pub(crate) fn due_reviews(
    entries: &[LessonEntry],
    profile: &PlayerProfile,
    today: u32,
) -> Vec<Warmup> {
    let mut due: Vec<Warmup> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.manifest.training.is_some())
        .filter_map(|(index, entry)| {
            let (tier, record) = Tier::ALL.into_iter().rev().find_map(|tier| {
                let record =
                    profile.trainings.get(&training_key(&entry.manifest.id, tier.number()))?;
                record.passed.then_some((tier, record))
            })?;
            let day = record.review_due_day()?;
            (day <= today).then_some(Warmup { entry: index, tier, due: day })
        })
        .collect();
    // Stable, so equally overdue reviews keep catalogue order.
    due.sort_by_key(|warmup| warmup.due);
    due
}

/// The mark on a node whose review is due. The mastery ring never goes
/// down — a cleared tier stays cleared — so this is where a lapsed skill
/// shows, beside the node rather than taken off its meter.
pub(super) fn spawn_due_badge(
    commands: &mut Commands,
    canvas: Entity,
    node: &PlacedNode,
    centre: Vec2,
) {
    let at = PartKind::DueBadge.top_left(centre);
    let badge = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                left: {Val::Px(at.x)},
                top: {Val::Px(at.y)},
            }
            Text("\u{21BB}")
            TextFont { font_size: {FontSize::Px(BADGE_PX)} }
            TextColor({DUE_COLOR})
        })
        .insert((
            PartKind::DueBadge.part(&node.id),
            ClusterMember(node.unit_id.clone()),
            LayoutOwner(node.unit_id.clone()),
        ))
        .id();
    commands.entity(canvas).add_child(badge);
}

/// The practice streak, as one line under the page header — only from two
/// days up. A single day is not a streak yet, and a lapsed one reads as 0
/// (`PracticeStreak::current`), so nothing is ever shown as lost.
pub(crate) fn spawn_streak(commands: &mut Commands, parent: Entity, days: u32, loc: &Localization) {
    if days < 2 {
        return;
    }
    let line = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text({String::from(
                loc.msg_args("lesson-tree-streak", &[("days", days.to_string())]),
            )})
            TextFont { font_size: {FontSize::Px(15.0)} }
            TextColor({Color::srgb(0.95, 0.82, 0.45)})
            Node { margin: {UiRect::bottom(Val::Px(6.0))} }
        })
        .id();
    commands.entity(parent).add_child(line);
}

/// The queue as a row of buttons under the page header, each starting its
/// review directly. Nothing at all when nothing is due — an empty queue is
/// the reward for keeping up, not a gap to fill.
pub(crate) fn spawn_warmups(
    commands: &mut Commands,
    parent: Entity,
    entries: &[LessonEntry],
    warmups: &[Warmup],
    loc: &Localization,
) {
    if warmups.is_empty() {
        return;
    }
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            row_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(8.0)),
            ..default()
        })
        .id();
    commands.entity(parent).add_child(row);
    commands.entity(row).with_children(|row| {
        row.spawn_empty().apply_scene(bsn! {
            Text({String::from(loc.msg("lesson-tree-warmup"))})
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::srgb(0.95, 0.82, 0.45)})
        });
    });

    for warmup in warmups {
        let manifest = entries[warmup.entry].manifest.clone();
        let tier = warmup.tier;
        let label = loc.msg_args(
            "lesson-tree-warmup-item",
            &[
                ("lesson", String::from(loc.msg(&manifest.title_key))),
                ("tier", String::from(loc.msg(tier_name_key(tier)))),
            ],
        );
        let item = commands
            .spawn_scene(button::small(
                &String::from(label),
                move |_: On<Activate>,
                      mut manifests: ResMut<Assets<SongManifest>>,
                      mut mode: ResMut<GameplayMode>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    start_training(
                        &manifest,
                        tier,
                        &mut manifests,
                        &mut mode,
                        &mut state,
                        &mut commands,
                    );
                },
            ))
            .id();
        commands.entity(row).add_child(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_app::profile::record_training;
    use harmonicon_song::lessons::{LessonManifest, TrainingBlock};

    fn lesson(id: &str) -> LessonEntry {
        LessonEntry {
            manifest: LessonManifest {
                id: id.to_string(),
                unit: "u".to_string(),
                optional: false,
                track: Some("bend".to_string()),
                title_key: format!("lesson-{id}-title"),
                body_key: format!("lesson-{id}-body"),
                chart: None,
                aural: false,
                prerequisites: Vec::new(),
                pass_criteria: None,
                training: Some(TrainingBlock {
                    technique: "bend".to_string(),
                    holes: vec![4],
                    seed: None,
                }),
                progression: None,
                scale: None,
                diagram: None,
                widgets: Vec::new(),
                position_cycle: false,
            },
            chart_asset_path: None,
        }
    }

    fn pass(profile: &mut PlayerProfile, lesson: &str, tier: u8, day: u32) {
        let record = profile.trainings.entry(training_key(lesson, tier)).or_default();
        record_training(record, true, 0.9, day);
    }

    #[test]
    fn a_review_comes_due_the_day_after_a_first_pass() {
        let entries = [lesson("a")];
        let mut profile = PlayerProfile::default();
        pass(&mut profile, "a", 1, 100);
        assert!(warmups(&due_reviews(&entries, &profile, 100)).to_vec().is_empty());
        assert_eq!(
            warmups(&due_reviews(&entries, &profile, 101)).to_vec(),
            vec![Warmup { entry: 0, tier: Tier::Isolate, due: 101 }]
        );
    }

    #[test]
    fn each_lesson_is_reviewed_at_its_highest_passed_tier() {
        let entries = [lesson("a")];
        let mut profile = PlayerProfile::default();
        pass(&mut profile, "a", 1, 90);
        pass(&mut profile, "a", 3, 100);
        let due = warmups(&due_reviews(&entries, &profile, 120)).to_vec();
        assert_eq!(due.len(), 1, "one review per lesson");
        assert_eq!(due[0].tier, Tier::Vary);
    }

    #[test]
    fn the_most_overdue_come_first_and_the_queue_is_capped() {
        let entries = [lesson("a"), lesson("b"), lesson("c"), lesson("d")];
        let mut profile = PlayerProfile::default();
        pass(&mut profile, "a", 1, 50);
        pass(&mut profile, "b", 1, 10);
        pass(&mut profile, "c", 1, 30);
        pass(&mut profile, "d", 1, 40);
        let order: Vec<usize> = warmups(&due_reviews(&entries, &profile, 100))
            .to_vec()
            .iter()
            .map(|warmup| warmup.entry)
            .collect();
        assert_eq!(order, vec![1, 2, 3], "b, c, d; a is least overdue and cut");
        assert_eq!(
            due_reviews(&entries, &profile, 100).len(),
            4,
            "the cap is the warm-up row's; every due node is still marked"
        );
    }

    #[test]
    fn nothing_unpassed_or_untrained_is_ever_due() {
        let mut untrained = lesson("plain");
        untrained.manifest.training = None;
        let entries = [lesson("a"), untrained];
        let mut profile = PlayerProfile::default();
        let record = profile.trainings.entry(training_key("a", 1)).or_default();
        record_training(record, false, 0.2, 10);
        assert!(warmups(&due_reviews(&entries, &profile, 1_000)).to_vec().is_empty());
    }
}
