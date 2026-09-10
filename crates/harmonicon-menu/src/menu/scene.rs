// SPDX-License-Identifier: MIT

//! Shared menu scene helpers: the full-screen root container every page
//! spawns into (with its background image), the themed/plain button
//! widget, and the `MenuRoot` marker `cleanup_menu` despawns on page exit.
//!
//! Every page follows the same top-level shape:
//! ```text
//! Root (Column, 100% x 100%)
//!   header (Row, width 100%, fixed height)
//!     title/subtitle column (flex_grow: 1.0)
//!     back button (icon-only, omitted on pages with no back target)
//!   body (Column, flex_grow: 1.0, fills the rest of the screen)
//!     scrollable content area — every page's own buttons/rows go here
//! ```
//! [`spawn_menu_root`] builds `header`/`body` and returns `(content,
//! header, page_root)` — `content` is the scrollable area inside `body`,
//! `header` is where [`spawn_back_button`] attaches a page's own Back
//! control, and `page_root` is the true full-screen (`100% x 100%`) outer
//! entity: pass *that*, not `content`, as `combobox::spawn_combobox`'s
//! `backdrop_parent` for any combobox a page nests inside its own narrower
//! sub-layout (see `menu::pages::options`) — `content`/`body` size to their
//! own content and get centered, so they're usually narrower than the
//! screen, which is exactly the bug `spawn_combobox`'s own doc comment
//! warns a too-small `backdrop_parent` causes (a click-outside-to-close
//! that only works along one axis).

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabGroup;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_ui::dialogs::button;
use harmonicon_ui::dialogs::page_chrome::{header_scene, heading_scene, title_column_scene};
// Re-exported so this module stays the one place a *menu page* reaches for
// page chrome; the definitions live in `dialogs` because non-menu screens
// need them too (see `dialogs::page_chrome`).
use harmonicon_platform::theme::LoadedTheme;
pub use harmonicon_ui::dialogs::page_chrome::spawn_back_button;
use harmonicon_ui::dialogs::scroll_area::spawn_scroll_area;

/// Scrollbar track/thumb colors for every menu page's content area — plain
/// constants, not theme-driven, matching this file's existing
/// `menu_bg()`-style "no per-theme menu chrome color" convention.
const SCROLLBAR_TRACK_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);
const SCROLLBAR_THUMB_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);

/// Marks every entity that belongs to a menu screen so `cleanup_menu` can
/// remove it in one sweep when the page changes. Shared with the `options` page.
#[derive(Component, Default, Clone)]
pub struct MenuRoot;

fn menu_bg() -> Color {
    Color::srgb(0.05, 0.05, 0.08)
}

/// The menu root container as a `bsn!` [`Scene`]: a plain full-screen
/// column — `header`/`body` (see [`spawn_menu_root`]) own their own
/// alignment, so the root itself doesn't need to center anything.
fn menu_root_scene() -> impl Scene {
    bsn! {
        Node {
            width: {Val::Percent(100.0)},
            height: {Val::Percent(100.0)},
            flex_direction: {FlexDirection::Column},
        }
        BackgroundColor({menu_bg()})
        MenuRoot
    }
}

fn title_column_with_subtitle_scene(title: String, subtitle: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: {FlexDirection::Column},
            flex_grow: {1.0_f32},
            row_gap: {Val::Px(6.0)},
        }
        Children [
            heading_scene(title, 52.0, Color::WHITE),
            heading_scene(subtitle, 20.0, Color::srgb(0.6, 0.6, 0.7)),
        ]
    }
}

/// The body column: fills whatever height `header` leaves behind
/// (`flex_grow: 1.0`), spans the full width, and centers its content as a
/// whole — same "short content stays centered, long content scrolls"
/// behaviour the root used to provide directly. `min_height: Val::Px(0.0)`
/// is the flexbox "min-height:auto" shrink trick (see `dialogs::
/// scroll_area::spawn_scroll_area`'s own use of it one level down) that
/// lets this force-shrink below its content's natural height once the
/// window is short, instead of pushing past the screen's bottom edge.
fn body_scene() -> impl Scene {
    bsn! {
        Node {
            width: {Val::Percent(100.0)},
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
            flex_grow: {1.0_f32},
            min_height: {Val::Px(0.0)},
            row_gap: {Val::Px(16.0)},
        }
    }
}

/// Shared scaffolding between [`spawn_menu_root`] and
/// [`spawn_menu_root_plain`] — the full-screen root, header (title/
/// subtitle, plus whatever back button a caller adds via
/// [`spawn_back_button`]), background image, and `TabGroup`. Callers attach
/// their own `body` child of the returned `page_root` immediately after —
/// see [`body_scene`] for the flex shape a body should follow to fill the
/// remaining space correctly. Returns `(page_root, header)`.
fn spawn_menu_chrome(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    theme: &LoadedTheme,
    menu_id: &str,
) -> (Entity, Entity) {
    let root = commands.spawn_scene(menu_root_scene()).id();

    let title_column = if let Some(sub) = subtitle {
        commands
            .spawn_scene(title_column_with_subtitle_scene(
                title.to_string(),
                sub.to_string(),
            ))
            .id()
    } else {
        commands
            .spawn_scene(title_column_scene(title.to_string()))
            .id()
    };
    let header = commands.spawn_scene(header_scene()).id();
    commands.entity(header).add_child(title_column);
    commands.entity(root).add_child(header);

    // Background image behind all other content. Inserted at index 0 so it stays
    // the lowest layer regardless of when this command is applied.
    if let Some(bg) = theme.background_for(menu_id) {
        let bg_layer = commands
            .spawn((
                ImageNode::new(bg.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
            ))
            .id();
        commands.entity(root).insert_children(0, &[bg_layer]);
    }

    // Scopes Tab/Shift+Tab cycling to this page's header (its Back button)
    // and body content together — on `root`, not just `content`, since the
    // back button lives in a sibling of `content` now.
    commands.entity(root).insert(TabGroup::default());

    (root, header)
}

/// Spawns the full-screen root, its header, and a scrollable body content
/// area — returning `(content, header, page_root)`: `content` is where
/// every caller's buttons/rows go (auto-scrolling, with a real visible
/// scrollbar — `dialogs::scroll_area::spawn_scroll_area` — once a page's
/// content, e.g. a long artist/song/lesson/theme list, no longer fits);
/// `header` is where a page attaches its own Back control; `page_root` is
/// the true `100% x 100%` outer container (see this module's own doc
/// comment for why a caller would need it over `content`).
///
/// For a short, bounded-height page that will never realistically need to
/// scroll (a handful of form rows + a button), use
/// [`spawn_menu_root_plain`] instead — nesting a combobox inside this
/// `ScrollArea` clips its open dropdown to the *content's own* height, not
/// the full available body height (`content`/`body` size to their content,
/// see this module's own doc comment), so a combobox near the bottom of a
/// page whose content happens to be shorter than the window gets its
/// dropdown cut off with nothing to scroll to — not a hypothetical, this is
/// exactly the bug `jam_generate`'s Genre combobox hit.
pub fn spawn_menu_root(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    theme: &LoadedTheme,
    menu_id: &str,
) -> (Entity, Entity, Entity) {
    let (root, header) = spawn_menu_chrome(commands, title, subtitle, theme, menu_id);

    let body = commands.spawn_scene(body_scene()).id();
    commands.entity(root).add_child(body);

    let mut content = Entity::PLACEHOLDER;
    commands.entity(body).with_children(|children| {
        content = spawn_scroll_area(children, SCROLLBAR_THUMB_COLOR, SCROLLBAR_TRACK_COLOR);
    });

    (content, header, root)
}

/// Like [`spawn_menu_root`], but `content` is a plain, non-clipping column
/// instead of a `ScrollArea` — for a page whose content is short enough to
/// never need scrolling and contains a combobox, where `spawn_menu_root`'s
/// `ScrollArea` would risk clipping an open dropdown (see that function's
/// own doc comment for why). Not a drop-in general replacement: a page
/// whose content genuinely can overflow (a long list) still needs
/// `spawn_menu_root`'s real scrolling.
pub fn spawn_menu_root_plain(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    theme: &LoadedTheme,
    menu_id: &str,
) -> (Entity, Entity, Entity) {
    let (root, header) = spawn_menu_chrome(commands, title, subtitle, theme, menu_id);

    let body = commands.spawn_scene(body_scene()).id();
    commands.entity(root).add_child(body);

    let mut content = Entity::PLACEHOLDER;
    commands.entity(body).with_children(|children| {
        content = children
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            })
            .id();
    });

    (content, header, root)
}

/// Spawn a single button as a child of `parent`, in the normal flex flow —
/// themes control appearance only, never layout. When the theme has
/// shaders the button also gets a smoke background layer, an optional
/// icon, and audio on hover/click. `on_click` is wired inline as the
/// `on(...)` callback (plain buttons) or via `observe` (themed buttons).
pub fn spawn_button<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    on_click: impl IntoObserverSystem<Activate, (), M> + Clone + Sync + 'static,
) -> Entity {
    let node = Node {
        min_width: Val::Px(260.0),
        padding: UiRect::axes(Val::Px(32.0), Val::Px(14.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };

    // Plain button: authored declaratively; click + hover ride along as
    // inline on(...)
    let e = commands
        .spawn_scene(button::default(label, on_click))
        .insert(node)
        .id();
    commands.entity(parent).add_child(e);
    e
}

pub fn cleanup_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
