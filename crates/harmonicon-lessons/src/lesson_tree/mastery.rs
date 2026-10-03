// SPDX-License-Identifier: MIT

//! The per-track mastery meter: how much of one track's training ladders
//! the player has cleared (`docs/training_tree_plan.md` §4).
//!
//! **Competence, not currency.** The meter is passed tiers over available
//! tiers, so it only moves when the player gets better at something — never
//! for time spent, which is what XP would reward. The per-node pip ring is
//! the same measure at lesson scale; this sums it across a track.
//!
//! Only tracks with at least one training get a meter. A track made of
//! instructional-only lessons has nothing a microphone can verify, and a
//! meter stuck at zero there would read as a failing the player can't fix.

use bevy::prelude::*;

use harmonicon_app::profile::PlayerProfile;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::lessons::LessonEntry;

use super::track_color;

const BAR_W_PX: f32 = 90.0;
const BAR_H_PX: f32 = 6.0;
const SWATCH_PX: f32 = 12.0;
const BAR_TRACK: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);

/// One track's meter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrackMastery {
    pub track: String,
    /// Passed tiers over all tiers in the track's ladders, 0..1.
    pub mastery: f32,
}

/// A meter for every track with trainings, in the order its first lesson
/// appears. `tiers` is the ladder length
/// (`harmonicon_core::training::Tier::ALL.len()`).
pub(crate) fn track_mastery(
    entries: &[LessonEntry],
    profile: &PlayerProfile,
    tiers: usize,
) -> Vec<TrackMastery> {
    let mut totals: Vec<(String, f32, usize)> = Vec::new();
    for entry in entries.iter().filter(|entry| entry.manifest.training.is_some()) {
        let track = entry.manifest.track();
        let mastered = profile.mastery(&entry.manifest.id, tiers);
        match totals.iter_mut().find(|(name, ..)| name == track) {
            Some((_, sum, count)) => {
                *sum += mastered;
                *count += 1;
            }
            None => totals.push((track.to_string(), mastered, 1)),
        }
    }
    totals
        .into_iter()
        .map(|(track, sum, count)| TrackMastery { track, mastery: sum / count as f32 })
        .collect()
}

/// The meters as a row of chips under the page header — the track's
/// colour, its name and percentage, and a bar. Nothing at all when no
/// track has trainings.
pub(crate) fn spawn_track_meters(
    commands: &mut Commands,
    parent: Entity,
    meters: &[TrackMastery],
    loc: &Localization,
) {
    if meters.is_empty() {
        return;
    }
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(20.0),
            row_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(8.0)),
            ..default()
        })
        .id();
    commands.entity(parent).add_child(row);

    for meter in meters {
        let color = track_color(&meter.track);
        let label = loc.msg_args(
            "lesson-tree-track-mastery",
            &[
                ("track", String::from(loc.msg(&format!("lesson-track-{}", meter.track)))),
                ("percent", ((meter.mastery * 100.0).round() as u32).to_string()),
            ],
        );
        commands.entity(row).with_children(|row| {
            row.spawn_empty().apply_scene(bsn! {
                Node {
                    flex_direction: {FlexDirection::Row},
                    align_items: {AlignItems::Center},
                    column_gap: {Val::Px(8.0)},
                }
                Children [
                    Node {
                        width: {Val::Px(SWATCH_PX)},
                        height: {Val::Px(SWATCH_PX)},
                        border_radius: {BorderRadius::MAX},
                    }
                    BackgroundColor({color})
                    --
                    Text({String::from(label)})
                    TextFont { font_size: {FontSize::Px(14.0)} }
                    TextColor({Color::srgb(0.86, 0.89, 0.95)})
                    --
                    Node {
                        width: {Val::Px(BAR_W_PX)},
                        height: {Val::Px(BAR_H_PX)},
                        border_radius: {BorderRadius::all(Val::Px(BAR_H_PX / 2.0))},
                    }
                    BackgroundColor({BAR_TRACK})
                    Children [
                        Node {
                            width: {Val::Percent(meter.mastery * 100.0)},
                            height: {Val::Percent(100.0)},
                            border_radius: {BorderRadius::all(Val::Px(BAR_H_PX / 2.0))},
                        }
                        BackgroundColor({color})
                    ]
                ]
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_app::profile::{record_training, training_key};
    use harmonicon_song::lessons::{LessonManifest, TrainingBlock};

    fn lesson(id: &str, track: &str, trainings: bool) -> LessonEntry {
        LessonEntry {
            manifest: LessonManifest {
                id: id.to_string(),
                unit: "u".to_string(),
                optional: false,
                track: Some(track.to_string()),
                title_key: format!("lesson-{id}-title"),
                body_key: format!("lesson-{id}-body"),
                chart: None,
                aural: false,
                prerequisites: Vec::new(),
                pass_criteria: None,
                training: trainings.then(|| TrainingBlock {
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

    fn pass(profile: &mut PlayerProfile, lesson: &str, tier: u8) {
        let record = profile.trainings.entry(training_key(lesson, tier)).or_default();
        record_training(record, true, 0.9, 100);
    }

    #[test]
    fn a_track_averages_its_ladders_and_skips_lessons_without_one() {
        let entries = [
            lesson("first-bend", "bend", true),
            lesson("deep-bends", "bend", true),
            lesson("theory", "bend", false),
            lesson("long-tones", "tone", false),
        ];
        let mut profile = PlayerProfile::default();
        // Two of five tiers on one ladder, none on the other: 2 of 10.
        pass(&mut profile, "first-bend", 1);
        pass(&mut profile, "first-bend", 2);

        let meters = track_mastery(&entries, &profile, 5);
        assert_eq!(
            meters,
            vec![TrackMastery { track: "bend".to_string(), mastery: 0.2 }],
            "a track with no trainings gets no meter"
        );
    }

    #[test]
    fn a_retried_failure_never_lowers_the_meter() {
        let entries = [lesson("first-bend", "bend", true)];
        let mut profile = PlayerProfile::default();
        pass(&mut profile, "first-bend", 1);
        let before = track_mastery(&entries, &profile, 5)[0].mastery;

        let record = profile.trainings.entry(training_key("first-bend", 1)).or_default();
        record_training(record, false, 0.1, 100);

        assert_eq!(track_mastery(&entries, &profile, 5)[0].mastery, before);
    }
}
