// SPDX-License-Identifier: MIT

//! A guided auto-tour of the top-level menu screens: starting it drives
//! `NextState<MenuPage>` through a fixed sequence on a timer, with a
//! click-blocking overlay on top naming the current screen and briefly
//! explaining what it's for, then returns to whichever page the tour was
//! started from. Only the screens reachable with no prior selection are
//! covered (not `ArtistList`/`SongList`/`LessonReader`, which need an
//! artist/song/lesson already picked, and not the separate `AppState`s like
//! the Bending Trainer or Song Editor).

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::dialogs::button;
use crate::localization::LocalizationExt;

use super::MenuPage;

/// How long each step stays on screen before auto-advancing.
const TOUR_STEP_SECONDS: f32 = 3.5;

/// The tour's fixed sequence: page, its title key, and its explanation key.
const TOUR_STEPS: &[(MenuPage, &str, &str)] = &[
    (MenuPage::Main, "tutorial-title-main", "tutorial-body-main"),
    (MenuPage::Play, "tutorial-title-play", "tutorial-body-play"),
    (
        MenuPage::ModeSelect,
        "tutorial-title-mode-select",
        "tutorial-body-mode-select",
    ),
    (
        MenuPage::Options,
        "tutorial-title-options",
        "tutorial-body-options",
    ),
    (
        MenuPage::Theme,
        "tutorial-title-theme",
        "tutorial-body-theme",
    ),
    (
        MenuPage::Lessons,
        "tutorial-title-lessons",
        "tutorial-body-lessons",
    ),
    (
        MenuPage::JamGenerate,
        "tutorial-title-jam-generate",
        "tutorial-body-jam-generate",
    ),
];

/// Present while the tour is running. `return_to` is the page it was
/// started from — captured once, at the start, so the tour always lands
/// back where the player actually was regardless of which step it's on.
/// `pub(super)` only so `menu::mod`'s `handle_menu_escape` can gate on its
/// presence — Escape shouldn't navigate a page out from under a running tour.
#[derive(Resource)]
pub(super) struct TutorialTour {
    step: usize,
    timer: Timer,
    return_to: MenuPage,
}

/// The overlay's root — not `MenuRoot`, so `cleanup_menu`'s per-page
/// teardown (which runs on every `OnExit(MenuPage::_)` the tour itself
/// triggers) never touches it; only the tour's own end logic despawns it.
#[derive(Component)]
pub(super) struct TutorialOverlayRoot;

/// The "Tutorial" button on the Main menu: starts the tour from whatever
/// page is active when it's clicked (always `Main` today, but this doesn't
/// assume that).
pub(super) fn start_tutorial_tour(
    _: On<Pointer<Click>>,
    page: Res<State<MenuPage>>,
    mut commands: Commands,
    mut next_page: ResMut<NextState<MenuPage>>,
) {
    commands.insert_resource(TutorialTour {
        step: 0,
        timer: Timer::from_seconds(TOUR_STEP_SECONDS, TimerMode::Once),
        return_to: page.get().clone(),
    });
    if let Some((first_page, ..)) = TOUR_STEPS.first() {
        next_page.set(first_page.clone());
    }
}

/// Ends the tour immediately: removes the driving resource (which
/// `sync_tutorial_overlay` reacts to by despawning the overlay) and returns
/// to whichever page the tour started from.
fn end_tutorial_tour(
    tour: &TutorialTour,
    next_page: &mut NextState<MenuPage>,
    commands: &mut Commands,
) {
    commands.remove_resource::<TutorialTour>();
    next_page.set(tour.return_to.clone());
}

/// The overlay's own "Skip Tutorial" button.
fn skip_tutorial_tour(
    _: On<Pointer<Click>>,
    tour: Option<Res<TutorialTour>>,
    mut next_page: ResMut<NextState<MenuPage>>,
    mut commands: Commands,
) {
    if let Some(tour) = tour {
        end_tutorial_tour(&tour, &mut next_page, &mut commands);
    }
}

/// Ticks the active step's timer and, once it finishes, either advances to
/// the next step (driving `NextState<MenuPage>` there) or ends the tour.
pub(super) fn advance_tutorial_tour(
    time: Res<Time>,
    tour: Option<ResMut<TutorialTour>>,
    mut next_page: ResMut<NextState<MenuPage>>,
    mut commands: Commands,
) {
    let Some(mut tour) = tour else { return };
    tour.timer.tick(time.delta());
    if !tour.timer.is_finished() {
        return;
    }
    let next_step = tour.step + 1;
    match TOUR_STEPS.get(next_step) {
        Some((page, ..)) => {
            let page = page.clone();
            tour.step = next_step;
            tour.timer.reset();
            next_page.set(page);
        }
        None => end_tutorial_tour(&tour, &mut next_page, &mut commands),
    }
}

/// Keeps the overlay in step with the tour: spawns it fresh (title/body
/// text for the current step) whenever the tour resource changes — inserted
/// (tour just started) or its `step` just advanced — and despawns it the
/// instant the tour resource is gone (skipped, or ran out of steps).
pub(super) fn sync_tutorial_overlay(
    tour: Option<Res<TutorialTour>>,
    existing: Query<Entity, With<TutorialOverlayRoot>>,
    mut commands: Commands,
    loc: Res<Localization>,
) {
    let Some(tour) = tour else {
        for e in &existing {
            commands.entity(e).despawn();
        }
        return;
    };
    if !tour.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    let Some(&(_, title_key, body_key)) = TOUR_STEPS.get(tour.step) else {
        return;
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::bottom(Val::Px(64.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            GlobalZIndex(500),
            TutorialOverlayRoot,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(10.0),
                        max_width: Val::Px(560.0),
                        padding: UiRect::axes(Val::Px(28.0), Val::Px(20.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.96)),
                    BorderColor::all(Color::srgb(0.35, 0.35, 0.48)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(String::from(loc.msg(title_key))),
                        TextFont {
                            font_size: FontSize::Px(24.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    panel.spawn((
                        Text::new(String::from(loc.msg(body_key))),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.80, 0.80, 0.88)),
                        TextLayout {
                            justify: Justify::Center,
                            ..default()
                        },
                    ));
                    panel.spawn((
                        Text::new(String::from(loc.msg_args(
                            "tutorial-step",
                            &[
                                ("n", (tour.step + 1).to_string()),
                                ("total", TOUR_STEPS.len().to_string()),
                            ],
                        ))),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.55, 0.55, 0.65)),
                    ));
                    panel.spawn_empty().apply_scene(button::small(
                        &String::from(loc.msg("tutorial-skip")),
                        skip_tutorial_tour,
                    ));
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_has_a_distinct_page() {
        let mut seen = std::collections::HashSet::new();
        for (page, ..) in TOUR_STEPS {
            assert!(
                seen.insert(format!("{page:?}")),
                "page {page:?} appears more than once in TOUR_STEPS"
            );
        }
    }

    #[test]
    fn tour_has_at_least_one_step() {
        assert!(!TOUR_STEPS.is_empty());
    }
}
