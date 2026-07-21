// SPDX-License-Identifier: MIT

//! The song editor's vertically-scrollable form area: the meta form, the
//! lesson form, and the status bar — everything *below* the fixed grid/mod-
//! panel chrome `ui::setup` spawns directly. Wrapped in a
//! [`bevy::ui_widgets::ScrollArea`] with a real, visible [`Scrollbar`]/
//! [`ScrollbarThumb`] pair beside it (see [`spawn_editor_scrollbar`]), since
//! without one there's no hint that the lesson-details panel runs below the
//! window. Deliberately scoped to just the form fields, not the whole
//! editor: the grid already has its own horizontal scroll, and an earlier
//! version of this that wrapped the grid too meant scrolling either one
//! moved both at once. Split out of `ui.rs` once adding this pushed that
//! file over the physical-design line budget.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

use super::lesson_form::spawn_lesson_form;
use super::meta_form::spawn_meta_form;
use super::ui::StatusMsg;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

/// Hides the editor's vertical scrollbar entirely once the current content
/// fits without scrolling — same "don't show a scrollbar with nothing to
/// scroll to" convention `interaction::update_grid_scrollbar` already uses
/// for the grid's own horizontal one.
pub(super) fn update_editor_scrollbar_visibility(
    areas: Query<&ComputedNode, With<ScrollArea>>,
    mut bars: Query<&mut Visibility, With<Scrollbar>>,
) {
    let (Ok(area), Ok(mut vis)) = (areas.single(), bars.single_mut()) else {
        return;
    };
    let needed = area.content_size().y > area.size().y + 1.0;
    *vis = if needed {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

/// A visible vertical scrollbar for `target` (the editor's [`ScrollArea`]),
/// via `bevy_ui_widgets`' headless [`Scrollbar`]/[`ScrollbarThumb`] widgets
/// (drag/click-to-page already wired by `UiWidgetsPlugins`, no hand-rolled
/// interaction needed — unlike the grid's own horizontal scrollbar, which
/// predates this and tracks a plain `f32` instead of a real
/// `ScrollPosition`). Hidden entirely once everything fits — see
/// [`update_editor_scrollbar_visibility`].
pub(super) fn spawn_editor_scrollbar(
    outer: &mut ChildSpawnerCommands,
    target: Entity,
    colors: SongEditorColors,
) {
    outer
        .spawn((
            Scrollbar::new(target, ControlOrientation::Vertical, 24.0),
            Node {
                width: Val::Px(10.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            // Starts hidden, same as `GridScrollTrack` — avoids a one-frame
            // flash of a full-height thumb before `update_editor_
            // scrollbar_visibility`'s first run corrects it.
            Visibility::Hidden,
        ))
        .with_children(|track| {
            track.spawn((
                ScrollbarThumb {
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    border: UiRect::ZERO,
                },
                BackgroundColor(colors.accent.with_alpha(0.65)),
            ));
        });
}

/// The form fields the vertical `ScrollArea` scrolls: the meta form, the
/// lesson form, and the status bar. The grid and mod panel are deliberately
/// *not* in here — see this module's own doc comment.
pub(super) fn spawn_form_scroll_content(
    scroll: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    spawn_meta_form(scroll, loc, colors);
    spawn_lesson_form(scroll, loc, colors);

    scroll.spawn((
        StatusMsg,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.40, 0.15)),
        Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
            ..default()
        },
    ));
}
