// SPDX-License-Identifier: MIT

//! A lesson's training ladder on the reader page: pick a tier, read what it
//! asks and what counts as passing, then start it.
//!
//! **The goal is stated before the tier starts**, as a specific target —
//! "70% accuracy on bend notes, at 96 BPM for 4 bars" — rather than a bare
//! tier number. That is the point of the panel (`docs/training_tree_plan.md`
//! §4): a specific, moderately hard goal is what makes practice
//! purposeful, and every number in it is the one the run is judged on —
//! [`training_criteria`] for the threshold, the ladder for tempo and length.
//!
//! The tiers are a radio group rather than five start buttons, so choosing
//! one and starting it are separate steps: the goal can be read first, on
//! touch as well as with a mouse, where a tooltip would need hover.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{AppState, GameplayMode, GeneratedSong, SelectedSong};
use harmonicon_app::profile::{PlayerProfile, training_key};
use harmonicon_core::pitch_map::{HarpKind, harp_for_key};
use harmonicon_core::training::{Tier, drill_chart};
use harmonicon_menu::menu::scene::spawn_button;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::lessons::{LessonContext, LessonEntry, LessonManifest, training_criteria};
use harmonicon_song::song::{SongManifest, training_manifest};
use harmonicon_ui::dialogs::tab_bar::{TabBarSelected, TabSelect, spawn_tab_bar};

use super::{criteria_goal, spawn_reader_line};

const LINE_COLOR: Color = Color::srgb(0.80, 0.82, 0.90);

/// The line saying what the selected tier asks of the player.
#[derive(Component)]
struct TierAbout;

/// The line stating the selected tier's goal.
#[derive(Component)]
struct TierGoal;

/// Locale key of a tier's name.
pub(crate) fn tier_name_key(tier: Tier) -> &'static str {
    match tier {
        Tier::Isolate => "lesson-training-tier-isolate",
        Tier::Consolidate => "lesson-training-tier-consolidate",
        Tier::Vary => "lesson-training-tier-vary",
        Tier::InContext => "lesson-training-tier-in-context",
        Tier::Interleave => "lesson-training-tier-interleave",
    }
}

/// Locale key of what a tier asks of the player.
pub(super) fn tier_about_key(tier: Tier) -> &'static str {
    match tier {
        Tier::Isolate => "lesson-training-tier-isolate-about",
        Tier::Consolidate => "lesson-training-tier-consolidate-about",
        Tier::Vary => "lesson-training-tier-vary-about",
        Tier::InContext => "lesson-training-tier-in-context-about",
        Tier::Interleave => "lesson-training-tier-interleave-about",
    }
}

/// The tier the panel opens on: the first one not yet passed, or the top
/// tier once every one is — the next thing worth practising either way.
pub(super) fn first_open_tier(passed: impl Fn(Tier) -> bool) -> Tier {
    Tier::ALL
        .into_iter()
        .find(|tier| !passed(*tier))
        .unwrap_or(Tier::Interleave)
}

/// What `tier` asks, and the goal it is judged on, as two localized lines.
fn tier_lines(loc: &Localization, entry: &LessonEntry, tier: Tier) -> (String, String) {
    let criteria = training_criteria(entry.manifest.pass_criteria.as_ref(), tier);
    let goal = loc.msg_args(
        "lesson-training-goal",
        &[
            ("goal", criteria_goal(loc, &criteria)),
            ("bpm", format!("{:.0}", tier.bpm())),
            ("bars", tier.bars().to_string()),
        ],
    );
    (loc.msg(tier_about_key(tier)).into(), goal.into())
}

fn spawn_line(commands: &mut Commands, root: Entity, text: String, marker: impl Component) {
    let line = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text({text})
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({LINE_COLOR})
        })
        .insert(marker)
        .id();
    commands.entity(root).add_child(line);
}

/// The lesson's training ladder: its mastery heading, the five tiers as a
/// radio group, the selected tier's description and goal, and a Start
/// button for it.
///
/// A lesson with no `training` block gets nothing — that is the honest
/// answer for anything instructional-only, where the microphone cannot
/// verify the technique and five drills would only pretend to check it.
///
/// Tiers are **not gated on each other**. Practising tier 4 before tier 2
/// is the player's business, and locking them would remove the one choice
/// the ladder offers; a passed tier's label carries a tick so progress is
/// still visible.
pub(super) fn spawn_training_panel(
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
        LINE_COLOR,
    );

    let passed = |tier: Tier| {
        profile
            .trainings
            .get(&training_key(&lesson_id, tier.number()))
            .is_some_and(|record| record.passed)
    };
    let labels: Vec<String> = Tier::ALL
        .into_iter()
        .map(|tier| {
            let tick = if passed(tier) { "\u{2713} " } else { "" };
            format!("{tick}{} {}", tier.number(), loc.msg(tier_name_key(tier)))
        })
        .collect();
    let lines: Vec<(String, String)> = Tier::ALL
        .into_iter()
        .map(|tier| tier_lines(loc, entry, tier))
        .collect();
    let opening = first_open_tier(passed);
    let (about, goal) = lines[usize::from(opening.number() - 1)].clone();

    let bar = spawn_tab_bar(
        commands,
        root,
        &labels,
        usize::from(opening.number() - 1),
        move |ev: On<TabSelect>,
              mut about: Query<&mut Text, (With<TierAbout>, Without<TierGoal>)>,
              mut goal: Query<&mut Text, With<TierGoal>>| {
            let Some((about_text, goal_text)) = lines.get(ev.index) else {
                return;
            };
            for mut text in &mut about {
                text.0.clone_from(about_text);
            }
            for mut text in &mut goal {
                text.0.clone_from(goal_text);
            }
        },
    );
    spawn_line(commands, root, about, TierAbout);
    spawn_line(commands, root, goal, TierGoal);

    let manifest = entry.manifest.clone();
    spawn_button(
        commands,
        root,
        &loc.msg("lesson-training-start"),
        move |_: On<Activate>,
              bars: Query<&TabBarSelected>,
              mut manifests: ResMut<Assets<SongManifest>>,
              mut mode: ResMut<GameplayMode>,
              mut state: ResMut<NextState<AppState>>,
              mut commands: Commands| {
            let Some(tier) = bars
                .get(bar)
                .ok()
                .and_then(|selected| Tier::ALL.get(selected.0).copied())
            else {
                return;
            };
            start_training(
                &manifest,
                tier,
                &mut manifests,
                &mut mode,
                &mut state,
                &mut commands,
            );
        },
    );
}

/// Starts one tier of `manifest`'s training ladder: generates the drill and
/// enters scored play with the tier's criteria in the `LessonContext`.
/// Shared by the reader's Start Training button and the skill tree's
/// warm-up reviews, so both run the same drill against the same goal.
pub(crate) fn start_training(
    manifest: &LessonManifest,
    tier: Tier,
    manifests: &mut Assets<SongManifest>,
    mode: &mut GameplayMode,
    state: &mut NextState<AppState>,
    commands: &mut Commands,
) {
    let Some(spec) = manifest.drill_spec(tier) else {
        return;
    };
    let harp = harp_for_key("C", HarpKind::Diatonic);
    let Some(chart) = drill_chart(&spec, &harp, &manifest.id, "") else {
        // No hole in the lesson can do the technique on this harp. Nothing
        // to play, so stay put rather than open a drill of plain notes that
        // trains nothing.
        return;
    };
    commands.insert_resource(SelectedSong(manifests.add(training_manifest(chart))));
    commands.insert_resource(LessonContext {
        lesson_id: manifest.id.clone(),
        pass_criteria: Some(training_criteria(manifest.pass_criteria.as_ref(), tier)),
        aural: false,
        tier: Some(tier.number()),
    });
    // Built by `Assets::add`, so it has no `LoadState` and `SongLoading`
    // would wait on it forever — see `app::GeneratedSong`.
    commands.insert_resource(GeneratedSong);
    *mode = GameplayMode::Play2D;
    state.set(AppState::Playing);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_opens_on_the_first_tier_not_yet_passed() {
        assert_eq!(first_open_tier(|_| false), Tier::Isolate);
        assert_eq!(
            first_open_tier(|tier| tier.number() <= 2),
            Tier::Vary,
            "tiers 1 and 2 passed"
        );
        // Tiers aren't gated, so a gap below a passed tier is found first.
        assert_eq!(
            first_open_tier(|tier| tier == Tier::Isolate || tier == Tier::Vary),
            Tier::Consolidate
        );
        assert_eq!(first_open_tier(|_| true), Tier::Interleave);
    }

    #[test]
    fn every_tier_has_a_name_and_a_description() {
        // `locales_define_the_same_keys` (harmonicon-platform) then carries
        // each key to the other locales.
        let ftl = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/locales/en-US/main/ui.ftl"),
        )
        .unwrap();
        let has = |key: &str| ftl.lines().any(|l| l.starts_with(&format!("{key} =")));
        let missing: Vec<&str> = Tier::ALL
            .into_iter()
            .flat_map(|tier| [tier_name_key(tier), tier_about_key(tier)])
            .chain(["lesson-training-goal", "lesson-training-start"])
            .filter(|key| !has(key))
            .collect();
        assert!(missing.is_empty(), "missing locale keys: {missing:?}");
    }
}
