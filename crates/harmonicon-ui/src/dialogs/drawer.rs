// SPDX-License-Identifier: MIT

//! A collapsible panel whose open/closed state lives on the panel itself.
//!
//! [`Drawer::open`] is the single source of truth. [`sync_drawers`] mirrors
//! it onto the node's `Display` and — the part that is easy to get wrong —
//! keeps a closed drawer's contents out of the Tab order.
//! `TabNavigation::gather_focusable` walks the tree by `TabIndex`/`Children`
//! alone, with no `Display` or `Visibility` check, so a hidden drawer's
//! buttons stay Tab stops unless something suppresses them.
//!
//! Suppression is *reversible*, not a blanket rewrite: closing records each
//! focusable descendant's own `TabIndex` before pushing it negative, and
//! opening restores exactly that value. A blanket "open ⇒ 0" would re-expose
//! things that were deliberately unreachable to begin with — a combobox's
//! closed dropdown items sit at `-1` for the same reason (see
//! `combobox::set_combobox_open`), and must stay there when the drawer
//! holding that combobox opens.
//!
//! The drawer owns visibility and focus only. Where it sits, what toggles
//! it, and whether its state persists are the caller's: a drawer meant to
//! float over content rather than reflow it is just an absolutely
//! positioned node carrying this component.
//!
//! Descendants are processed when `Drawer` *changes* (including when it is
//! first added), so content spawned into an already-closed drawer later on
//! keeps its own `TabIndex` until the drawer next toggles. Nothing does that
//! today — every caller builds a drawer's contents with the drawer itself.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;

/// A collapsible panel. Toggle [`open`](Self::open); [`sync_drawers`] does
/// the rest.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drawer {
    pub open: bool,
}

/// The `TabIndex` a descendant had before its drawer closed over it.
/// Present only while suppressed.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuppressedTabIndex(pub i32);

/// What closing or opening a drawer does to one descendant's `TabIndex`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabChange {
    /// Leave it alone.
    Keep,
    /// Remember `original`, then make it unreachable.
    Suppress { original: i32 },
    /// Put back what it was before the drawer closed.
    Restore { original: i32 },
}

/// The rule, stated once. A descendant that was already unreachable
/// (negative) when the drawer closed is not recorded, so opening leaves it
/// unreachable too.
#[must_use]
pub fn tab_change(open: bool, current: i32, suppressed: Option<i32>) -> TabChange {
    match (open, suppressed) {
        (true, Some(original)) => TabChange::Restore { original },
        (false, None) if current >= 0 => TabChange::Suppress { original: current },
        _ => TabChange::Keep,
    }
}

/// Mirrors each changed [`Drawer`] onto its node and its descendants'
/// focusability.
pub fn sync_drawers(
    mut drawers: Query<(Entity, &Drawer, &mut Node), Changed<Drawer>>,
    children: Query<&Children>,
    mut tabs: Query<(&mut TabIndex, Option<&SuppressedTabIndex>)>,
    mut commands: Commands,
) {
    for (entity, drawer, mut node) in &mut drawers {
        node.display = if drawer.open { Display::Flex } else { Display::None };
        for descendant in children.iter_descendants(entity) {
            let Ok((mut tab_index, suppressed)) = tabs.get_mut(descendant) else {
                continue;
            };
            match tab_change(drawer.open, tab_index.0, suppressed.map(|s| s.0)) {
                TabChange::Keep => {}
                TabChange::Suppress { original } => {
                    tab_index.0 = -1;
                    commands.entity(descendant).insert(SuppressedTabIndex(original));
                }
                TabChange::Restore { original } => {
                    tab_index.0 = original;
                    commands.entity(descendant).remove::<SuppressedTabIndex>();
                }
            }
        }
    }
}

pub struct DrawerPlugin;

impl Plugin for DrawerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_drawers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_records_a_reachable_index_and_opening_restores_it() {
        assert_eq!(tab_change(false, 0, None), TabChange::Suppress { original: 0 });
        assert_eq!(tab_change(false, 3, None), TabChange::Suppress { original: 3 });
        assert_eq!(tab_change(true, -1, Some(3)), TabChange::Restore { original: 3 });
    }

    #[test]
    fn an_already_unreachable_index_is_left_unreachable_both_ways() {
        // A closed combobox's items: -1 before, -1 after, whatever the drawer
        // around them does.
        assert_eq!(tab_change(false, -1, None), TabChange::Keep);
        assert_eq!(tab_change(true, -1, None), TabChange::Keep);
    }

    #[test]
    fn repeated_syncs_are_idempotent() {
        // Closed twice: the second pass must not record `-1` as the original.
        assert_eq!(tab_change(false, -1, Some(0)), TabChange::Keep);
        // Open twice: nothing left to restore.
        assert_eq!(tab_change(true, 0, None), TabChange::Keep);
    }

    fn world_with_drawer(open: bool) -> (World, Schedule, Entity, Entity, Entity) {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_drawers);
        let reachable = world.spawn(TabIndex(0)).id();
        let unreachable = world.spawn(TabIndex(-1)).id();
        let drawer = world
            .spawn((Node::default(), Drawer { open }))
            .add_children(&[reachable, unreachable])
            .id();
        (world, schedule, drawer, reachable, unreachable)
    }

    #[test]
    fn a_closed_drawer_hides_itself_and_its_tab_stops() {
        let (mut world, mut schedule, drawer, reachable, unreachable) = world_with_drawer(false);
        schedule.run(&mut world);
        assert_eq!(world.get::<Node>(drawer).unwrap().display, Display::None);
        assert_eq!(world.get::<TabIndex>(reachable).unwrap().0, -1);
        assert_eq!(world.get::<TabIndex>(unreachable).unwrap().0, -1);
    }

    #[test]
    fn reopening_restores_exactly_what_was_there() {
        let (mut world, mut schedule, drawer, reachable, unreachable) = world_with_drawer(false);
        schedule.run(&mut world);
        world.get_mut::<Drawer>(drawer).unwrap().open = true;
        schedule.run(&mut world);
        assert_eq!(world.get::<Node>(drawer).unwrap().display, Display::Flex);
        assert_eq!(world.get::<TabIndex>(reachable).unwrap().0, 0);
        assert_eq!(
            world.get::<TabIndex>(unreachable).unwrap().0,
            -1,
            "something unreachable before the drawer closed stays unreachable"
        );
        assert!(world.get::<SuppressedTabIndex>(reachable).is_none());
    }
}
