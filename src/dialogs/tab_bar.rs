// SPDX-License-Identifier: MIT

//! Reusable tab bar: a horizontal row of mutually-exclusive tabs, one
//! always active. Clicking an inactive tab retints the row and triggers
//! [`TabSelect`] on the bar's root entity; clicking the already-active tab
//! does nothing (no event, no retint). The caller owns what "switching
//! tabs" *means* — typically repopulating a content area from the selected
//! index — the widget only owns the selection state and its visuals.
//!
//! Built the same way as `dialogs::combobox`: spawned with a plain helper,
//! caller reacts via an [`EntityEvent`] observer, visuals re-sync through a
//! `Changed<TabBarSelected>`-gated system registered once by
//! [`TabBarPlugin`] — so external code may also *write* `TabBarSelected`
//! directly to switch tabs programmatically and the row keeps up.

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;

use super::button;

/// Triggered on a tab bar's root entity (the `Entity` returned by
/// [`spawn_tab_bar`]) when the user switches to a different tab. Not
/// triggered for clicks on the already-active tab.
#[derive(Clone, Debug, EntityEvent)]
pub struct TabSelect {
    #[event_target]
    pub tab_bar: Entity,
    /// Index into the `labels` slice the bar was spawned with.
    pub index: usize,
}

/// The active tab's index, on the bar's root entity. The widget updates it
/// on click; write to it directly to switch tabs from code (the visuals
/// follow either way).
#[derive(Component, Clone, Debug, Default)]
pub struct TabBarSelected(pub usize);

/// One tab button's back-pointer to its bar + position.
#[derive(Component, Clone, Copy)]
struct TabButton {
    bar: Entity,
    index: usize,
}

fn tab_color(active: bool) -> Color {
    if active {
        button::CHOICE_SELECTED
    } else {
        button::color_default()
    }
}

/// Spawns a tab bar as a child of `parent`, one tab per label, with
/// `selected` active. Returns the bar's root entity, which carries
/// [`TabBarSelected`] and is where [`TabSelect`] is triggered — pass
/// `on_select` as an observer system reacting to it, the same shape as
/// `spawn_combobox`'s `on_select`.
pub fn spawn_tab_bar<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    labels: &[String],
    selected: usize,
    on_select: impl IntoObserverSystem<TabSelect, (), M>,
) -> Entity {
    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            },
            TabBarSelected(selected),
        ))
        .id();
    commands.entity(bar).observe(on_select);

    for (index, label) in labels.iter().enumerate() {
        // `TabButton` wraps a bare `Entity` (no meaningful `Default`), so it
        // can't ride along inside `bsn!` — attach it imperatively, same as
        // the combobox's back-pointer components.
        let tab = commands
            .spawn_empty()
            .apply_scene(tab_scene(label.clone(), index == selected))
            .insert(TabButton { bar, index })
            .id();
        commands.entity(bar).add_child(tab);
    }

    commands.entity(parent).add_child(bar);
    bar
}

/// One tab: a button whose colour marks the active state (kept in sync by
/// [`sync_tab_visuals`] after spawn).
fn tab_scene(label: String, active: bool) -> impl Scene {
    bsn! {
        Button
        Node {
            padding: {UiRect::axes(Val::Px(18.0), Val::Px(8.0))},
        }
        BackgroundColor({tab_color(active)})
        on(tab_click)
        on(tab_over)
        on(tab_out)
        Children [
            (
                Text({label})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

fn tab_click(
    ev: On<Pointer<Click>>,
    tabs: Query<&TabButton>,
    mut bars: Query<&mut TabBarSelected>,
    mut commands: Commands,
) {
    let Ok(tab) = tabs.get(ev.entity) else {
        return;
    };
    let Ok(mut selected) = bars.get_mut(tab.bar) else {
        return;
    };
    if selected.0 == tab.index {
        return; // already active — not a switch
    }
    selected.0 = tab.index;
    commands.trigger(TabSelect {
        tab_bar: tab.bar,
        index: tab.index,
    });
}

fn tab_over(
    ev: On<Pointer<Over>>,
    tabs: Query<&TabButton>,
    bars: Query<&TabBarSelected>,
    mut colors: Query<&mut BackgroundColor>,
) {
    let Ok(tab) = tabs.get(ev.entity) else {
        return;
    };
    let active = bars.get(tab.bar).is_ok_and(|s| s.0 == tab.index);
    if !active && let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(button::CHOICE_HOVER);
    }
}

fn tab_out(
    ev: On<Pointer<Out>>,
    tabs: Query<&TabButton>,
    bars: Query<&TabBarSelected>,
    mut colors: Query<&mut BackgroundColor>,
) {
    let Ok(tab) = tabs.get(ev.entity) else {
        return;
    };
    let active = bars.get(tab.bar).is_ok_and(|s| s.0 == tab.index);
    if !active && let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(button::color_default());
    }
}

/// Retint a bar's tabs whenever its `TabBarSelected` changes — from a click
/// or from external code writing the component directly.
fn sync_tab_visuals(
    changed: Query<(Entity, &TabBarSelected), Changed<TabBarSelected>>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor)>,
) {
    for (bar, selected) in &changed {
        for (tab, mut bg) in &mut tabs {
            if tab.bar != bar {
                continue;
            }
            bg.0 = tab_color(tab.index == selected.0);
        }
    }
}

/// Registers the visual-sync system every tab bar needs. Add once per app.
pub struct TabBarPlugin;

impl Plugin for TabBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_tab_visuals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(TabBarPlugin);
        app
    }

    fn bar_with_tabs(app: &mut App, selected: usize, count: usize) -> (Entity, Vec<Entity>) {
        let bar = app.world_mut().spawn(TabBarSelected(selected)).id();
        let tabs = (0..count)
            .map(|index| {
                app.world_mut()
                    .spawn((
                        TabButton { bar, index },
                        BackgroundColor(tab_color(index == selected)),
                    ))
                    .id()
            })
            .collect();
        (bar, tabs)
    }

    #[test]
    fn writing_selected_retints_every_tab_of_that_bar() {
        let mut app = app();
        let (bar, tabs) = bar_with_tabs(&mut app, 0, 3);
        app.update();

        app.world_mut().get_mut::<TabBarSelected>(bar).unwrap().0 = 2;
        app.update();

        let color_of = |app: &App, e: Entity| app.world().get::<BackgroundColor>(e).unwrap().0;
        assert_eq!(color_of(&app, tabs[0]), tab_color(false));
        assert_eq!(color_of(&app, tabs[1]), tab_color(false));
        assert_eq!(color_of(&app, tabs[2]), tab_color(true));
    }

    #[test]
    fn other_bars_tabs_are_left_alone() {
        let mut app = app();
        let (bar_a, _tabs_a) = bar_with_tabs(&mut app, 0, 2);
        let (_bar_b, tabs_b) = bar_with_tabs(&mut app, 0, 2);
        app.update();

        // Deliberately wrong tint on the other bar's tab: the sync for
        // bar_a must not "fix" it (it only walks its own bar's tabs).
        app.world_mut()
            .get_mut::<BackgroundColor>(tabs_b[1])
            .unwrap()
            .0 = Color::srgb(1.0, 0.0, 0.0);
        app.world_mut().get_mut::<TabBarSelected>(bar_a).unwrap().0 = 1;
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(tabs_b[1]).unwrap().0,
            Color::srgb(1.0, 0.0, 0.0),
        );
    }
}
