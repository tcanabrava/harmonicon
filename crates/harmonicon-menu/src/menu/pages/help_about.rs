// SPDX-License-Identifier: MIT

//! Documentation link, About, Tutorial, and Credits, plus the static
//! "what is this app" About page reached from here.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::AppState;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

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
        std::env::current_exe().ok().and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join("docs/book/book/index.html"))
        }),
        // `CARGO_MANIFEST_DIR` is *this crate's* directory, not the workspace
        // root — hence the `../..`. The book lives at the repo root alongside
        // `assets/`, which the root package owns (see `CLAUDE.md`, "Paths
        // reaching `assets/` from a crate need `../../`").
        Some(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/book/book/index.html"),
        ),
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

    // Anywhere else — wasm, and Android, which is deliberately not covered by
    // the `target_os = "linux"` arm above — there is no process to spawn: say
    // so instead of reporting a success that never happened. (Opening a file
    // on Android means handing an Intent to the system through JNI, which is
    // a different mechanism entirely, not a missing `Command`.)
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("no default application handler for {}", path.display()),
    ));

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    Ok(())
}

pub(crate) fn setup_help_about_menu(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, header, _page_root) = spawn_menu_root(
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
        |_: On<Activate>,
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
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::About),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-tutorial"),
        tutorial::start_tutorial_tour,
    );
    // The first-run welcome, re-runnable: the same three steps a fresh
    // install offers, reachable without deleting the profile.
    spawn_button(
        &mut commands,
        root,
        &loc.msg("help-first-run"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Welcome),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-credits"),
        |_: On<Activate>, mut state: ResMut<NextState<AppState>>| state.set(AppState::Credits),
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
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}

pub(crate) fn setup_about_page(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, header, _page_root) = spawn_menu_root(
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
        .spawn_empty()
        .apply_scene(bsn! {
            Text({String::from(loc.msg_args(
                "about-version",
                &[("version", String::from(env!("CARGO_PKG_VERSION")))],
            ))})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({Color::srgb(0.6, 0.6, 0.7)})
        })
        .id();
    commands.entity(root).add_child(version);
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::HelpAbout),
    );
}
