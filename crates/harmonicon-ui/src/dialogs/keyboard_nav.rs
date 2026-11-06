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

use bevy::input_focus::tab_navigation::{NavAction, TabGroup, TabNavigation, TabNavigationPlugin};
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};
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

/// Moves focus into a modal that is on screen while focus is outside it.
///
/// **A modal `TabGroup` confines focus only once focus is inside it.** With
/// focus anywhere else, Bevy's Tab cycles every *non*-modal group — the page
/// behind the dialog — and never reaches the dialog's own controls, so a
/// keyboard player could neither answer it nor leave it, and Enter re-pressed
/// whatever had opened it. Rather than every modal remembering to focus
/// something, this applies the rule to all of them, including those shown by
/// toggling `Display` or `Visibility` rather than spawned (the phrase
/// editor's popover).
///
/// A modal counts as on screen when it isn't `Display::None`, is visible,
/// and is laid out with a non-zero size. With several, the one painted on top (highest
/// `GlobalZIndex`) wins. Its first focusable descendant gets focus; a modal
/// that already placed focus itself (`AutoFocus`) is left alone, since focus
/// is then inside it.
fn focus_into_open_modal(
    modals: Query<(
        Entity,
        &TabGroup,
        &Node,
        &InheritedVisibility,
        &ComputedNode,
        Option<&GlobalZIndex>,
    )>,
    parents: Query<&ChildOf>,
    nav: TabNavigation,
    mut focus: ResMut<InputFocus>,
) {
    let mut open: Vec<(i32, Entity)> = modals
        .iter()
        .filter(|(_, group, node, visible, computed, _)| {
            group.modal
                && node.display != Display::None
                && visible.get()
                && computed.size() != Vec2::ZERO
        })
        .map(|(entity, _, _, _, _, z)| (z.map_or(0, |z| z.0), entity))
        .collect();
    if open.is_empty() {
        return;
    }
    if let Some(current) = focus.get()
        && open.iter().any(|&(_, modal)| {
            current == modal || parents.iter_ancestors(current).any(|a| a == modal)
        })
    {
        return;
    }
    open.sort_by_key(|&(z, _)| std::cmp::Reverse(z));
    for (_, modal) in open {
        if let Ok(first) = nav.initialize(modal, NavAction::First) {
            focus.set(first, FocusCause::Navigated);
            return;
        }
    }
}

pub struct KeyboardNavPlugin;

impl Plugin for KeyboardNavPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TabNavigationPlugin)
            .add_systems(Update, (focus_into_open_modal, update_focus_ring).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input_focus::tab_navigation::TabIndex;

    /// A page with one button, and a modal with one button over it.
    fn world_with_modal(display: Display) -> (World, Entity, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<InputFocus>();
        let laid_out = ComputedNode { size: Vec2::new(400.0, 300.0), ..default() };
        let page = world
            .spawn((TabGroup::new(0), Node::default(), InheritedVisibility::VISIBLE, laid_out))
            .id();
        let behind = world.spawn((TabIndex(0), ChildOf(page))).id();
        let modal = world
            .spawn((
                TabGroup::modal(),
                Node { display, ..default() },
                InheritedVisibility::VISIBLE,
                laid_out,
            ))
            .id();
        let inside = world.spawn((TabIndex(0), ChildOf(modal))).id();
        (world, behind, modal, inside)
    }

    #[test]
    fn focus_outside_an_open_modal_moves_into_it() {
        let (mut world, behind, _, inside) = world_with_modal(Display::Flex);
        world.insert_resource(InputFocus::from_entity(behind));
        world.run_system_once(focus_into_open_modal).unwrap();
        assert_eq!(world.resource::<InputFocus>().get(), Some(inside));
    }

    #[test]
    fn focus_already_inside_the_modal_is_left_where_it_is() {
        let (mut world, _, modal, inside) = world_with_modal(Display::Flex);
        let second = world.spawn((TabIndex(0), ChildOf(modal))).id();
        world.insert_resource(InputFocus::from_entity(second));
        world.run_system_once(focus_into_open_modal).unwrap();
        assert_eq!(world.resource::<InputFocus>().get(), Some(second));
        assert_ne!(Some(inside), Some(second));
    }

    #[test]
    fn a_closed_modal_takes_nothing() {
        // The phrase editor stays spawned and closes with Display::None.
        let (mut world, behind, _, _) = world_with_modal(Display::None);
        world.insert_resource(InputFocus::from_entity(behind));
        world.run_system_once(focus_into_open_modal).unwrap();
        assert_eq!(world.resource::<InputFocus>().get(), Some(behind));
    }
}
