// SPDX-License-Identifier: MIT

//! Reusable tab bar: a horizontal row of mutually-exclusive tabs, one
//! always active. Built on `bevy_ui_widgets`' [`RadioGroup`]/[`RadioButton`]
//! — the bar itself carries `RadioGroup` (and is the sole focusable/
//! Tab-reachable entity per WAI-ARIA convention; individual tabs are
//! reached via the group's own arrow-key handling, not further Tab
//! presses), each tab a `RadioButton`. Clicking an inactive tab (or
//! arrowing to it while the bar is focused) retints the row and triggers
//! [`TabSelect`] on the bar's root entity; clicking the already-active tab
//! is a no-op at the widget level (`RadioButton`'s own click handler skips
//! an already-checked button). The caller owns what "switching tabs"
//! *means* — typically repopulating a content area from the selected
//! index — the widget only owns the selection state and its visuals.
//!
//! [`TabBarSelected`] mirrors the current index for read access (e.g. the
//! Lessons unit-tab caller doesn't otherwise track it); unlike before this
//! module rode on `RadioGroup`, it's no longer meant to be written
//! directly to switch tabs from outside — nothing needs that today, and
//! `RadioGroup`'s own external-state-management model (see its doc
//! comment) doesn't naturally support both directions without real
//! complexity, so it isn't attempted.

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::picking::events::{PointerOut, PointerOver};
use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{RadioButton, RadioGroup, ValueChange, radio_self_update};

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

/// The active tab's index, on the bar's root entity — kept in sync by
/// [`on_radio_group_value_change`] for read access; see the module doc
/// comment for why writing it directly no longer switches tabs.
#[derive(Component, Clone, Debug, Default)]
pub struct TabBarSelected(pub usize);

/// One tab button's back-pointer to its bar + position.
#[derive(Component, Clone, Copy)]
struct TabButton {
    bar: Entity,
    index: usize,
}

fn tab_color(active: bool) -> Color {
    if active { button::CHOICE_SELECTED } else { button::color_default() }
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
    on_select: impl IntoObserverSystem<TabSelect, M>,
) -> Entity {
    let bar = commands
        .spawn_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Row},
                column_gap: {Val::Px(4.0)},
            }
            RadioGroup
            TabIndex(0)
            TabBarSelected({selected})
        })
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
        if index == selected {
            commands.entity(tab).insert(Checked);
        }
        commands.entity(bar).add_child(tab);
    }

    commands.entity(parent).add_child(bar);
    bar
}

/// One tab: a radio button whose colour marks the active state (kept in
/// sync by [`on_radio_group_value_change`] after spawn). No `TabIndex` —
/// per `RadioButton`'s own doc comment, individual radio buttons aren't
/// Tab stops; the enclosing `RadioGroup` is.
fn tab_scene(label: String, active: bool) -> impl Scene {
    bsn! {
        RadioButton
        Node {
            padding: {UiRect::axes(Val::Px(18.0), Val::Px(8.0))},
        }
        BackgroundColor({tab_color(active)})
        on(tab_over)
        on(tab_out)
        Children [
            Text({label})
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::WHITE})
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

fn tab_over(
    ev: On<PointerOver>,
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
    ev: On<PointerOut>,
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

/// Reacts to `RadioGroup`'s own [`ValueChange<Entity>`] (fired by
/// `RadioButton`'s click/keyboard handling — see `bevy_ui_widgets::radio`
/// — only on a genuine change, since the widget itself already skips an
/// already-checked button): updates [`TabBarSelected`], retints every tab
/// of that bar, and fires [`TabSelect`]. Guards on `ev.source` actually
/// carrying `TabBarSelected` so this can't misfire for some unrelated
/// `RadioGroup` elsewhere in the app. Deliberately keys the retint off
/// `TabButton::index == clicked_tab.index`, not `Checked` — the latter is
/// updated by [`radio_self_update`] via a *separate* observer on the same
/// event, whose ordering relative to this one isn't guaranteed.
fn on_radio_group_value_change(
    ev: On<ValueChange<Entity>>,
    tabs: Query<&TabButton>,
    mut bars: Query<&mut TabBarSelected>,
    mut colors: Query<&mut BackgroundColor>,
    children: Query<&Children>,
    mut commands: Commands,
) {
    let Ok(mut selected) = bars.get_mut(ev.source) else {
        return;
    };
    let Ok(clicked_tab) = tabs.get(ev.value) else {
        return;
    };
    selected.0 = clicked_tab.index;
    if let Ok(kids) = children.get(ev.source) {
        for &child in kids {
            if let (Ok(tab), Ok(mut bg)) = (tabs.get(child), colors.get_mut(child)) {
                bg.0 = tab_color(tab.index == clicked_tab.index);
            }
        }
    }
    commands.trigger(TabSelect { tab_bar: ev.source, index: clicked_tab.index });
}

/// Registers the observers every tab bar needs. Add once per app.
pub struct TabBarPlugin;

impl Plugin for TabBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(radio_self_update).add_observer(on_radio_group_value_change);
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
        let tabs: Vec<Entity> = (0..count)
            .map(|index| {
                app.world_mut()
                    .spawn((
                        TabButton { bar, index },
                        BackgroundColor(tab_color(index == selected)),
                    ))
                    .id()
            })
            .collect();
        app.world_mut().entity_mut(bar).add_children(&tabs);
        (bar, tabs)
    }

    /// Simulates what `RadioButton`'s own click handling would trigger on
    /// the group once a real click resolves — the actual entry point
    /// `on_radio_group_value_change` reacts to.
    fn click_tab(app: &mut App, bar: Entity, tab: Entity) {
        app.world_mut().trigger(ValueChange::<Entity> { source: bar, value: tab, is_final: true });
    }

    #[test]
    fn clicking_a_tab_retints_every_tab_of_that_bar_and_updates_selected() {
        let mut app = app();
        let (bar, tabs) = bar_with_tabs(&mut app, 0, 3);
        app.update();

        click_tab(&mut app, bar, tabs[2]);
        app.update();

        assert_eq!(app.world().get::<TabBarSelected>(bar).unwrap().0, 2);
        let color_of = |app: &App, e: Entity| app.world().get::<BackgroundColor>(e).unwrap().0;
        assert_eq!(color_of(&app, tabs[0]), tab_color(false));
        assert_eq!(color_of(&app, tabs[1]), tab_color(false));
        assert_eq!(color_of(&app, tabs[2]), tab_color(true));
    }

    #[test]
    fn clicking_a_tab_fires_tab_select_with_its_index() {
        let mut app = app();
        let (bar, tabs) = bar_with_tabs(&mut app, 0, 3);
        app.update();

        #[derive(Resource, Default)]
        struct LastSelect(Option<usize>);
        app.init_resource::<LastSelect>();
        app.world_mut().entity_mut(bar).observe(
            |ev: On<TabSelect>, mut last: ResMut<LastSelect>| {
                last.0 = Some(ev.index);
            },
        );

        click_tab(&mut app, bar, tabs[1]);
        app.update();

        assert_eq!(app.world().resource::<LastSelect>().0, Some(1));
    }

    #[test]
    fn other_bars_tabs_are_left_alone() {
        let mut app = app();
        let (bar_a, tabs_a) = bar_with_tabs(&mut app, 0, 2);
        let (_bar_b, tabs_b) = bar_with_tabs(&mut app, 0, 2);
        app.update();

        // Deliberately wrong tint on the other bar's tab: switching bar_a
        // must not "fix" it (it only walks its own bar's tabs).
        app.world_mut().get_mut::<BackgroundColor>(tabs_b[1]).unwrap().0 =
            Color::srgb(1.0, 0.0, 0.0);

        click_tab(&mut app, bar_a, tabs_a[1]);
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(tabs_b[1]).unwrap().0,
            Color::srgb(1.0, 0.0, 0.0),
        );
    }

    #[test]
    fn a_value_change_from_an_unrelated_radio_group_is_ignored() {
        let mut app = app();
        let (_bar, tabs) = bar_with_tabs(&mut app, 0, 2);
        app.update();

        let stray_group = app.world_mut().spawn_empty().id();
        // No `TabBarSelected` on `stray_group` — must not panic or repaint.
        app.world_mut().trigger(ValueChange::<Entity> {
            source: stray_group,
            value: tabs[1],
            is_final: true,
        });
        app.update();

        assert_eq!(app.world().get::<BackgroundColor>(tabs[0]).unwrap().0, tab_color(true));
    }
}
