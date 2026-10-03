// SPDX-License-Identifier: MIT

//! Keyboard focus navigation: registers `bevy_input_focus`'s Tab/Shift+Tab
//! cycling (not part of `DefaultPlugins`, unlike `InputFocusPlugin`/
//! `InputDispatchPlugin`, which already are) and paints a visible focus
//! ring on whichever entity currently has keyboard focus.
//!
//! Keyboard activation itself needs no bridge here: every button-shaped
//! widget in the app is spawned as a real `bevy_ui_widgets::Button`
//! (`use bevy::ui_widgets::Button as WidgetButton` — plain
//! `bevy::prelude::*` resolves the bare `Button` name to `bevy_ui`'s
//! *legacy*, pre-headless-widgets marker instead, which has no keyboard
//! support at all), and every click handler in this codebase is written
//! as `on(...): On<Activate>` rather than `On<PointerClick>` — `Activate`
//! is `bevy_ui_widgets::Button`'s own unified "this was activated" event,
//! fired for a real click *and* a focused Enter/Space alike (see
//! `button_on_pointer_click`/`button_on_key_event` in the vendored crate).
//! An earlier version of this module instead re-triggered a synthetic
//! `PointerClick` from `Activate` to avoid retyping every click handler
//! — that bridge could recurse into itself through `bevy_ui_widgets`' own
//! `button_on_pointer_click` (which reacts to the synthetic click and,
//! seeing a real click's still-`Pressed` component, re-emits `Activate`)
//! and overflow the stack on a genuine mouse click. Writing every handler
//! directly against `Activate` removes the recursive path entirely rather
//! than trying to out-guard it.

use bevy::input_focus::tab_navigation::TabNavigationPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::prelude::*;

/// Outline color for whichever control currently has keyboard focus —
/// distinct from this UI's existing hover (`dialogs::button::CHOICE_HOVER`,
/// a dark blue-grey) and selected (`dialogs::button::CHOICE_SELECTED`, a
/// green) tints, so it reads as its own kind of feedback.
const FOCUS_OUTLINE_COLOR: Color = Color::srgb(0.95, 0.80, 0.20);
const FOCUS_OUTLINE_WIDTH: f32 = 2.0;
const FOCUS_OUTLINE_OFFSET: f32 = 2.0;

/// Paints a visible outline on whichever entity currently has keyboard
/// focus, following [`InputFocus`]/[`InputFocusVisible`] (the latter is
/// kept in sync automatically by `TabNavigationPlugin` itself — hidden on
/// a mouse click, shown again on Tab — nothing to hand-roll here). Uses
/// `Outline`, not `BorderColor`: it draws outside the box with no layout
/// impact, so focusable widgets don't need to reserve border space up
/// front just to support this. Only touches the previous/current focused
/// entity, not a whole-UI scan; `try_insert` since either can have
/// despawned since last frame (e.g. a page navigation).
fn update_focus_ring(
    focus: Res<InputFocus>,
    visible: Res<InputFocusVisible>,
    mut last_painted: Local<Option<Entity>>,
    mut commands: Commands,
) {
    if !focus.is_changed() && !visible.is_changed() {
        return;
    }
    if let Some(prev) = last_painted.take()
        && Some(prev) != focus.get()
    {
        commands.entity(prev).try_insert(Outline {
            width: Val::ZERO,
            offset: Val::ZERO,
            color: Color::NONE,
        });
    }
    let Some(current) = focus.get().filter(|_| visible.0) else {
        return;
    };
    commands.entity(current).try_insert(Outline {
        width: Val::Px(FOCUS_OUTLINE_WIDTH),
        offset: Val::Px(FOCUS_OUTLINE_OFFSET),
        color: FOCUS_OUTLINE_COLOR,
    });
    *last_painted = Some(current);
}

pub struct KeyboardNavPlugin;

impl Plugin for KeyboardNavPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TabNavigationPlugin).add_systems(Update, update_focus_ring);
    }
}
