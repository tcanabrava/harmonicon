// SPDX-License-Identifier: MIT

//! Documentation link, About, Tutorial, and Credits, plus the static
//! "what is this app" About page reached from here.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::AppState;
use crate::dialogs::button_material::ButtonMaterials;
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

use super::tutorial;

/// Marks the status line on the Help/About page that reports why the
/// Documentation button couldn't open (missing local build, or the OS
/// couldn't launch a handler for it).
#[derive(Component)]
struct DocsStatusLabel;

/// Finds a locally-built mdBook index, checking both the packaged layout
/// (next to the executable) and the dev-tree layout (`docs/book/book/`,
/// gitignored — built on demand via `mdbook build`). Returns `None` if
/// neither exists; the docs aren't bundled into builds yet (see CLAUDE.md).
fn locate_docs_index() -> Option<std::path::PathBuf> {
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("docs/book/book/index.html"))),
        Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/book/book/index.html")),
    ];
    candidates.into_iter().flatten().find(|p| p.exists())
}

/// Hands a local file path to the OS's default handler. Best-effort: the
/// caller reports failure via `DocsStatusLabel` rather than treating it as
/// fatal.
fn open_in_default_app(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()?;
    }
    Ok(())
}

pub(crate) fn setup_help_about_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("help-about-title"),
        None,
        &theme,
        "HelpAbout",
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("help-documentation"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>,
         mut status: Query<&mut Text, With<DocsStatusLabel>>,
         loc: Res<Localization>| {
            let message = match locate_docs_index() {
                Some(path) => open_in_default_app(&path).err().map(|err| err.to_string()),
                None => Some(String::from(loc.msg("help-docs-not-found"))),
            };
            if let Some(message) = message
                && let Ok(mut text) = status.single_mut()
            {
                text.0 = message;
            }
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-about"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::About),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-tutorial"),
        &theme,
        &btn_mats,
        tutorial::start_tutorial_tour,
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-credits"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::Credits)
        },
    );
    let status = commands
        .spawn((
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::srgb(0.8, 0.4, 0.4)),
            DocsStatusLabel,
        ))
        .id();
    commands.entity(root).add_child(status);
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}

pub(crate) fn setup_about_page(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("about-title"),
        None,
        &theme,
        "About",
    );
    let body = commands
        .spawn((
            Text::new(loc.msg("about-body")),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.85, 0.85, 0.9)),
            Node {
                max_width: Val::Px(560.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(body);
    let version = commands
        .spawn((
            Text::new(String::from(loc.msg_args(
                "about-version",
                &[("version", String::from(env!("CARGO_PKG_VERSION")))],
            ))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.7)),
        ))
        .id();
    commands.entity(root).add_child(version);
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::HelpAbout)
        },
    );
}
