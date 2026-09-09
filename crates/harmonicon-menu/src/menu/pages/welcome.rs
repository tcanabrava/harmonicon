// SPDX-License-Identifier: MIT

//! The first-launch greeting.
//!
//! Harmonicon does nothing useful without a working microphone, and until
//! this page existed a new player was shown four buttons — Play, Options,
//! Help / About, Quit — none of which says so. Microphone setup was reachable
//! only by opening Options and reading it, and the guided tour only from
//! inside Help / About. This page puts all three first steps in front of the
//! player once, on the launch where they're actually needed.
//!
//! Reached only via `harmonicon_app::profile::FirstRun`, which
//! `menu::routing::route_menu_entry` consumes; no other page navigates here.
//!
//! **"Set up your microphone" goes to Options, not straight to latency
//! calibration.** The first-run question is "does the game hear me at all",
//! which Options answers — it holds the input-device picker and the
//! `MicStatus` banner, and offers calibration from there for the player who
//! wants it. Latency calibration is a refinement, and it also always returns
//! to Options anyway (`ReturnToOptions`), so routing through it first would
//! just be a detour.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_fluent::Localization;

use harmonicon_app::profile::{PlayerProfile, save_profile};
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

use super::tutorial;

pub(crate) fn setup_welcome_menu(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, _header, _page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("welcome-title"),
        None,
        &theme,
        "Welcome",
    );

    // The body sits on a scrim rather than straight on the theme background.
    // Every bundled theme uses a photographic backdrop (the default is a lit
    // neon sign), and light body text laid directly over one is genuinely
    // hard to read — which matters more here than anywhere else, since this
    // paragraph is the only place the game explains that it needs to hear a
    // real harmonica. Buttons don't need it: they carry their own fill.
    let scrim = commands
        .spawn((
            Node {
                max_width: Val::Px(600.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
                // A `Node` field in Bevy 0.19, not a component of its own.
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.04, 0.07, 0.82)),
        ))
        .id();
    let body = commands
        .spawn((
            Text::new(loc.msg("welcome-body")),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.88, 0.88, 0.93)),
        ))
        .id();
    commands.entity(scrim).add_child(body);
    commands.entity(root).add_child(scrim);

    spawn_button(
        &mut commands,
        root,
        &loc.msg("welcome-setup-mic"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("welcome-tour"),
        tutorial::start_tutorial_tour,
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("welcome-lessons"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::LessonTree),
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

/// Writes `profile.json` as soon as the player leaves the welcome page, so
/// the greeting is once-only even if the session later crashes.
///
/// `FirstRun` alone would not survive that: it's consumed in memory, but the
/// *next* launch re-derives it from whether the file exists, and the only
/// other thing that writes one is the `AppExit` flush — which a crash skips.
/// Saving here costs one write of an almost-empty profile and makes the
/// question "have they been greeted?" durable at the moment it's answered.
pub(crate) fn persist_profile_on_welcome_exit(profile: Res<PlayerProfile>) {
    save_profile(&profile);
}
