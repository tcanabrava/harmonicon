// SPDX-License-Identifier: MIT

//! A guided auto-tour of Harmonicon's top-level screens: starting it drives
//! `NextState<MenuPage>`/`NextState<AppState>` through a fixed sequence on a
//! timer, with a click-blocking overlay on top naming the current screen and
//! briefly explaining what it's for, then returns to whichever page the tour
//! was started from. Alongside the no-selection-required menu pages, a few
//! steps actually enter live gameplay for a look — [`TourTarget::Playing`]
//! (2D and Jam Session, both against the bundled `DEMO_SONG_PATH`),
//! [`TourTarget::BendingTrainer`], and [`TourTarget::SongEditor`] — using
//! the exact same `AppState` transitions their normal entry points do, so
//! nothing about the tour needs those screens' own systems to know a tour
//! is even happening. Not covered: `ArtistList`/`SongList`/`LessonReader`,
//! which need an artist/song/lesson already picked.

use bevy::asset::AssetServer;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::dialogs::button;
use crate::localization::LocalizationExt;
use crate::song::SongManifest;

use crate::app::{AppState, GameplayMode, SelectedSong};

use super::MenuPage;

/// The bundled song a live-gameplay tour step plays a few seconds of. Long
/// enough (well over two minutes) that no tour step could ever run it to
/// completion and trigger a real `AppState::Results` — the tour always cuts
/// away first.
const DEMO_SONG_PATH: &str = "songs/Example Artist/Example Song/song/chart.harpchart";

/// How long a plain menu-page step stays on screen before auto-advancing.
const PAGE_STEP_SECONDS: f32 = 3.5;
/// Live-gameplay steps need the 3s countdown (`gameplay::COUNTDOWN`) to run
/// first before there's anything to actually see, so they get longer.
const PLAYING_STEP_SECONDS: f32 = 7.0;
/// The Bending Trainer and Song Editor have no countdown, but still want a
/// beat longer than a plain caption-only page to actually look around.
const LIVE_SCREEN_STEP_SECONDS: f32 = 5.0;

/// What a tour step actually does when it becomes current.
#[derive(Clone)]
enum TourTarget {
    /// Show a `MenuPage`, no `AppState` change (or a `Menu`-page landing
    /// after leaving a non-`Menu` step — see [`enter_tour_target`]).
    Page(MenuPage),
    /// Load [`DEMO_SONG_PATH`] and play it in the given mode for a look.
    Playing(GameplayMode),
    BendingTrainer,
    SongEditor,
}

/// The tour's fixed sequence: what to show, its title/explanation keys, and
/// how long to stay there.
const TOUR_STEPS: &[(TourTarget, &str, &str, f32)] = &[
    (
        TourTarget::Page(MenuPage::Main),
        "tutorial-title-main",
        "tutorial-body-main",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::Play),
        "tutorial-title-play",
        "tutorial-body-play",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::ModeSelect),
        "tutorial-title-mode-select",
        "tutorial-body-mode-select",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::Playing(GameplayMode::Play2D),
        "tutorial-title-gameplay",
        "tutorial-body-gameplay",
        PLAYING_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::JamSessionMenu),
        "tutorial-title-jam-session-menu",
        "tutorial-body-jam-session-menu",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::Playing(GameplayMode::JamSession),
        "tutorial-title-jam-session",
        "tutorial-body-jam-session",
        PLAYING_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::JamGenerate),
        "tutorial-title-jam-generate",
        "tutorial-body-jam-generate",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::BendingTrainer,
        "tutorial-title-bending-trainer",
        "tutorial-body-bending-trainer",
        LIVE_SCREEN_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::Lessons),
        "tutorial-title-lessons",
        "tutorial-body-lessons",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::SongEditor,
        "tutorial-title-song-editor",
        "tutorial-body-song-editor",
        LIVE_SCREEN_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::Options),
        "tutorial-title-options",
        "tutorial-body-options",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::Theme),
        "tutorial-title-theme",
        "tutorial-body-theme",
        PAGE_STEP_SECONDS,
    ),
    (
        TourTarget::Page(MenuPage::HelpAbout),
        "tutorial-title-help-about",
        "tutorial-body-help-about",
        PAGE_STEP_SECONDS,
    ),
];

/// Present while the tour is running. `return_to` is the `MenuPage` it was
/// started from — captured once, at the start, so the tour always lands
/// back where the player actually was regardless of which step it's on.
/// `step == TOUR_STEPS.len()` is the one-frame "ending" sentinel between
/// the last step finishing and `route_menu_entry` (which needs to see the
/// tour still present to route correctly) actually removing this resource
/// — see [`end_tutorial_tour`].
///
/// `pub(crate)` because [`tour_active`] — a `run_if` condition other
/// modules (gameplay's pause menu, the Bending Trainer, the Song Editor)
/// use to suspend their own Escape handling during a tour — takes
/// `Option<Res<TutorialTour>>`, which must be nameable at their call sites;
/// its fields stay private, so only this module can construct or read one.
#[derive(Resource)]
pub(crate) struct TutorialTour {
    step: usize,
    timer: Timer,
    return_to: MenuPage,
}

/// The overlay's root — not `MenuRoot`/`GameplayRoot`, so none of the
/// screens the tour itself drives through despawn it as part of their own
/// teardown; only the tour's own end logic does.
#[derive(Component)]
pub(super) struct TutorialOverlayRoot;

/// True while a tour is running — the gate every screen the tour can drive
/// through (gameplay's pause menu, the Bending Trainer, the Song Editor,
/// and `menu::mod`'s own page-back handler) uses to suspend its own
/// Escape/pause handling for the duration, so the tour's click-blocking
/// overlay isn't the only thing standing between the player and
/// accidentally steering the tour off course. "Skip Tutorial" (in the
/// overlay itself) is the one deliberate way out.
pub(crate) fn tour_active(tour: Option<Res<TutorialTour>>) -> bool {
    tour.is_some()
}

/// The `MenuPage` `route_menu_entry` should land on for the tour's current
/// step, or `None` if the current step isn't a `Page` step (shouldn't be
/// possible to observe from `route_menu_entry`, which only runs on
/// entering `AppState::Menu` — a `Playing`/`BendingTrainer`/`SongEditor`
/// step never targets `Menu` directly, see [`enter_tour_target`] — but a
/// missing entry falls back to `Main` rather than panicking regardless).
pub(super) fn tour_menu_landing(tour: &TutorialTour) -> MenuPage {
    if tour.step >= TOUR_STEPS.len() {
        return tour.return_to.clone();
    }
    match TOUR_STEPS.get(tour.step) {
        Some((TourTarget::Page(page), ..)) => page.clone(),
        _ => tour.return_to.clone(),
    }
}

/// Whether the tour resource should be dropped once `route_menu_entry` has
/// used it to route this `OnEnter(AppState::Menu)` — true once it's past
/// its last real step (the "ending" sentinel described on [`TutorialTour`]).
pub(super) fn tour_finished(tour: &TutorialTour) -> bool {
    tour.step >= TOUR_STEPS.len()
}

/// The "Tutorial" button on the Main menu: starts the tour from whatever
/// page is active when it's clicked (always `Main` today, but this doesn't
/// assume that). Always starts on a `Page` step, and we're already in
/// `AppState::Menu` to click it from, so this can set `NextState<MenuPage>`
/// directly rather than going through `route_menu_entry` — see
/// [`enter_tour_target`]'s doc comment for why later steps can't.
pub(super) fn start_tutorial_tour(
    _: On<Pointer<Click>>,
    page: Res<State<MenuPage>>,
    mut commands: Commands,
    mut next_page: ResMut<NextState<MenuPage>>,
) {
    commands.insert_resource(TutorialTour {
        step: 0,
        timer: Timer::from_seconds(step_seconds(0), TimerMode::Once),
        return_to: page.get().clone(),
    });
    if let Some((TourTarget::Page(first_page), ..)) = TOUR_STEPS.first() {
        next_page.set(first_page.clone());
    }
}

fn step_seconds(step: usize) -> f32 {
    TOUR_STEPS.get(step).map_or(PAGE_STEP_SECONDS, |&(.., s)| s)
}

/// Applies `target`: for a `Page`, just queues `AppState::Menu` —
/// `route_menu_entry` (via [`tour_menu_landing`]) is what actually picks
/// the right page, because directly setting `NextState<MenuPage>` in the
/// same tick as `NextState<AppState>` isn't reliable once `AppState` is
/// actually changing (the substate machinery resets it to its own default
/// first) — the same reason `ReturnToSongList`/`GeneratedJamSession`/
/// `LessonContext` all exist as flags `route_menu_entry` reads instead of
/// setting `NextState<MenuPage>` directly from wherever they're raised.
/// For the live-screen targets, this is exactly what each screen's own
/// normal entry point does (loading `DEMO_SONG_PATH` the same way picking
/// a song from the song list does, for `Playing`).
fn enter_tour_target(
    target: &TourTarget,
    commands: &mut Commands,
    next_app_state: &mut NextState<AppState>,
    mode: &mut GameplayMode,
    asset_server: &AssetServer,
) {
    match target {
        TourTarget::Page(_) => next_app_state.set(AppState::Menu),
        TourTarget::Playing(gameplay_mode) => {
            *mode = gameplay_mode.clone();
            commands.insert_resource(SelectedSong(
                asset_server.load::<SongManifest>(DEMO_SONG_PATH),
            ));
            next_app_state.set(AppState::SongLoading);
        }
        TourTarget::BendingTrainer => next_app_state.set(AppState::BendingTrainer),
        TourTarget::SongEditor => next_app_state.set(AppState::SongEditor2),
    }
}

/// Ends the tour: marks it finished (see [`TutorialTour`]'s doc comment)
/// and queues a return to `AppState::Menu` — `route_menu_entry` reads
/// [`tour_menu_landing`]/[`tour_finished`] to land on `return_to` and
/// actually remove the resource, the same indirection every other step
/// uses to land on a specific page (see [`enter_tour_target`]).
fn end_tutorial_tour(tour: &mut TutorialTour, next_app_state: &mut NextState<AppState>) {
    tour.step = TOUR_STEPS.len();
    next_app_state.set(AppState::Menu);
}

/// The overlay's own "Skip Tutorial" button.
fn skip_tutorial_tour(
    _: On<Pointer<Click>>,
    tour: Option<ResMut<TutorialTour>>,
    mut next_app_state: ResMut<NextState<AppState>>,
) {
    if let Some(mut tour) = tour {
        end_tutorial_tour(&mut tour, &mut next_app_state);
    }
}

/// Ticks the active step's timer and, once it finishes, either advances to
/// the next step or ends the tour.
pub(super) fn advance_tutorial_tour(
    time: Res<Time>,
    tour: Option<ResMut<TutorialTour>>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut mode: ResMut<GameplayMode>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let Some(mut tour) = tour else { return };
    // Once the tour has ended (the sentinel step), stop ticking — the
    // resource is about to be removed by `route_menu_entry` and nothing
    // further should happen on its way out.
    if tour.step >= TOUR_STEPS.len() {
        return;
    }
    tour.timer.tick(time.delta());
    if !tour.timer.is_finished() {
        return;
    }
    let next_step = tour.step + 1;
    match TOUR_STEPS.get(next_step) {
        Some((target, .., seconds)) => {
            let target = target.clone();
            tour.step = next_step;
            tour.timer = Timer::from_seconds(*seconds, TimerMode::Once);
            enter_tour_target(
                &target,
                &mut commands,
                &mut next_app_state,
                &mut mode,
                &asset_server,
            );
        }
        None => end_tutorial_tour(&mut tour, &mut next_app_state),
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
    let Some((_, title_key, body_key, _)) = TOUR_STEPS.get(tour.step) else {
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
    fn tour_has_at_least_one_step() {
        assert!(!TOUR_STEPS.is_empty());
    }

    #[test]
    fn every_page_step_targets_a_distinct_page() {
        let mut seen = std::collections::HashSet::new();
        for (target, ..) in TOUR_STEPS {
            if let TourTarget::Page(page) = target {
                assert!(
                    seen.insert(format!("{page:?}")),
                    "page {page:?} appears more than once in TOUR_STEPS"
                );
            }
        }
    }

    #[test]
    fn the_first_step_is_a_page_step() {
        // `start_tutorial_tour` only handles a `Page` first step (it can
        // set `NextState<MenuPage>` directly, since it's already in
        // `AppState::Menu` — see its doc comment).
        assert!(matches!(TOUR_STEPS.first(), Some((TourTarget::Page(_), ..))));
    }

    #[test]
    fn every_step_has_a_positive_duration() {
        for (.., seconds) in TOUR_STEPS {
            assert!(*seconds > 0.0);
        }
    }

    #[test]
    fn tour_menu_landing_uses_the_current_pages_step() {
        let tour = TutorialTour {
            step: 1,
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            return_to: MenuPage::Main,
        };
        assert_eq!(tour_menu_landing(&tour), MenuPage::Play);
    }

    #[test]
    fn tour_menu_landing_falls_back_to_return_to_past_the_last_step() {
        let tour = TutorialTour {
            step: TOUR_STEPS.len(),
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            return_to: MenuPage::Options,
        };
        assert_eq!(tour_menu_landing(&tour), MenuPage::Options);
        assert!(tour_finished(&tour));
    }

    #[test]
    fn tour_menu_landing_falls_back_to_return_to_on_a_non_page_step() {
        let step = TOUR_STEPS
            .iter()
            .position(|(t, ..)| !matches!(t, TourTarget::Page(_)))
            .expect("at least one non-Page step exists");
        let tour = TutorialTour {
            step,
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            return_to: MenuPage::Lessons,
        };
        assert_eq!(tour_menu_landing(&tour), MenuPage::Lessons);
        assert!(!tour_finished(&tour));
    }
}
