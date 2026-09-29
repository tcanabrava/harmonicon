// SPDX-License-Identifier: MIT

//! First-launch steps for microphone setup, the guided tour, and lessons.
//! Routing consumes `FirstRun`; each completed step can return here.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::WelcomeFlow;
use harmonicon_app::profile::{PlayerProfile, save_profile};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

use super::tutorial;

pub(crate) fn setup_welcome_menu(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    welcome: Res<WelcomeFlow>,
) {
    let (root, _header, _page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("welcome-title"),
        None,
        &theme,
        "Welcome",
    );

    // The scrim keeps body text readable over theme backgrounds.
    let scrim = commands
        .spawn((
            Node {
                max_width: Val::Px(600.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.04, 0.07, 0.82)),
        ))
        .id();
    let body = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text({String::from(loc.msg("welcome-body"))})
            TextFont { font_size: {FontSize::Px(18.0)} }
            TextColor({Color::srgb(0.88, 0.88, 0.93)})
        })
        .id();
    commands.entity(scrim).add_child(body);
    commands.entity(root).add_child(scrim);

    // Each step's page comes back here when it's done (see `WelcomeFlow`),
    // and a step already taken this session is marked so the player can
    // see what's left.
    spawn_button(
        &mut commands,
        root,
        &step_label(&loc.msg("welcome-setup-mic"), welcome.mic_done),
        |_: On<Activate>,
         mut welcome: ResMut<WelcomeFlow>,
         mut page: ResMut<NextState<MenuPage>>| {
            welcome.return_to_welcome = true;
            page.set(MenuPage::Options);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &step_label(&loc.msg("welcome-tour"), welcome.tour_done),
        tutorial::start_tutorial_tour,
    );
    spawn_button(
        &mut commands,
        root,
        &step_label(&loc.msg("welcome-lessons"), welcome.lesson_done),
        |_: On<Activate>,
         mut welcome: ResMut<WelcomeFlow>,
         mut page: ResMut<NextState<MenuPage>>| {
            welcome.return_to_welcome = true;
            page.set(MenuPage::LessonTree);
        },
    );
    // No header Back button: there is nowhere "back" to on a first launch.
    // This is the deliberate way past the page, and Escape does the same via
    // `routing::menu_escape`.
    spawn_button(
        &mut commands,
        root,
        &loc.msg("welcome-skip"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}

/// A step's button label, with a check in front once the step has been
/// taken this session.
fn step_label(label: &str, done: bool) -> String {
    if done {
        format!("\u{2713} {label}")
    } else {
        label.to_string()
    }
}

/// Save on exit so the first-run greeting stays completed after a crash.
pub(crate) fn persist_profile_on_welcome_exit(profile: Res<PlayerProfile>) {
    save_profile(&profile);
}
