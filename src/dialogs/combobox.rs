// SPDX-License-Identifier: MIT

//! Reusable combobox: a labelled toggle button showing the current
//! selection, which opens an overlay list of choices above the rest of the
//! page (instead of pushing it down) when clicked. Dismissed by clicking
//! outside, pressing Escape, or picking an item — only picking an item
//! changes the selection. Extracted from the Options page's microphone
//! device picker so future pickers don't have to reinvent it.
//!
//! `spawn_combobox`'s `backdrop_parent` must be a full-screen-sized
//! container (e.g. the page root from `menu::spawn_menu_root`) — the
//! click-catching backdrop sizes itself to 100% of `backdrop_parent`, and is
//! despawned whenever `backdrop_parent` is (recursive despawn), so the
//! widget needs no lifecycle hooks of its own. `trigger_parent` is where the
//! visible label+toggle actually lives in-flow — usually the same entity as
//! `backdrop_parent` (a single-column page), but a page with its own nested
//! columns (see `gameplay::bending_trainer::setup`) can pass a narrower
//! column here while keeping the backdrop sized to the whole page, so a
//! click anywhere still dismisses the dropdown.
//!
//! Register [`ComboboxPlugin`] once per app. If some other Escape handler
//! (e.g. "go back a menu page") should only fire when no dropdown was open
//! to close, order it `.after(close_open_comboboxes_on_escape)`.
//!
//! The dropdown list's on-screen position is managed by `bevy::ui_widgets`'
//! [`Popover`] component, not a fixed `top`/`left` — it opens below the
//! toggle normally, above it instead if that would run past the bottom of
//! the window, and is clamped so it can't render off-screen either way.
//! `PopoverPlugin` runs this: it's part of `bevy_ui_widgets::UiWidgetsPlugins`,
//! which `DefaultPlugins` already includes whenever the `bevy_ui_widgets`
//! Cargo feature is on (as it is here) — nothing to register separately.

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};

use super::button;

const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.12, 0.98);
const PANEL_BORDER: Color = Color::srgb(0.30, 0.30, 0.40);
const TOGGLE_MIN_WIDTH: f32 = 220.0;
const LABEL_WIDTH: f32 = 110.0;
const LABEL_GAP: f32 = 14.0;

/// Triggered on a combobox's root entity (the `Entity` returned by
/// [`spawn_combobox`]) when the user picks an item from the list. The widget
/// always updates its own [`ComboboxValue`] regardless of whether anyone is
/// listening; this is how the caller finds out and reacts (persist a
/// setting, reconnect a device, ...).
#[derive(Clone, Debug, EntityEvent)]
pub struct ComboboxSelect {
    #[event_target]
    pub combobox: Entity,
    pub value: String,
}

/// The value currently shown by a combobox. The widget updates this itself
/// when the user picks an item; write to it directly to change the display
/// for reasons outside the widget (e.g. the underlying device falling back
/// to a different one on its own).
#[derive(Component, Clone, Debug, Default)]
pub struct ComboboxValue(pub String);

/// Links a combobox root to its overlay list and backdrop, so the toggle/
/// backdrop/item observers (which only know their own entity) can find the
/// rest of their own widget instance instead of relying on a global
/// singleton query — the reason multiple comboboxes can coexist on one page.
/// `pub(crate)` only because it appears in `close_open_comboboxes_on_escape`'s
/// signature, which other modules need to name for `.after(...)` ordering.
#[derive(Component, Clone, Copy)]
pub(crate) struct ComboboxLinks {
    list: Entity,
    backdrop: Entity,
}

/// Back-pointer from a toggle button or backdrop to its combobox root.
#[derive(Component, Clone, Copy)]
struct ComboboxRoot(Entity);

/// One item in a combobox's list.
#[derive(Component, Clone)]
struct ComboboxItemButton {
    root: Entity,
    value: String,
}

/// The toggle button's own label text, naming which combobox it belongs to
/// so [`sync_combobox_visuals`] can tell multiple comboboxes' labels apart.
#[derive(Component, Clone, Copy)]
struct ComboboxToggleLabel(Entity);

fn toggle_label(current: &str) -> String {
    format!("{current}  \u{25BE}")
}

/// Spawns a combobox as a child of `trigger_parent`, with its click-catching
/// backdrop sized to `backdrop_parent` instead (see the module docs — the
/// two are almost always the same entity): `label` beside a toggle button
/// showing `current`, opening an overlay list of `options` below it when
/// clicked. Returns the combobox's root entity, which carries
/// [`ComboboxValue`] and is where [`ComboboxSelect`] is triggered — pass
/// `on_select` as an observer system reacting to it, the same way
/// `spawn_volume_slider` elsewhere takes an `on_change` observer for
/// `ValueChange<f32>`.
pub fn spawn_combobox<M: 'static>(
    commands: &mut Commands,
    trigger_parent: Entity,
    backdrop_parent: Entity,
    label: &str,
    options: &[String],
    current: &str,
    on_select: impl IntoObserverSystem<ComboboxSelect, (), M>,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            ComboboxValue(current.to_string()),
        ))
        .id();
    commands.entity(root).observe(on_select);

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(LABEL_GAP),
            ..default()
        })
        .id();
    commands.entity(root).add_child(row);

    let label_entity = commands
        .spawn((
            Node {
                width: Val::Px(LABEL_WIDTH),
                ..default()
            },
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(row).add_child(label_entity);

    // `ComboboxRoot`/`ComboboxToggleLabel` wrap a bare `Entity`, which
    // has no meaningful `Default`, so `bsn!`'s inline component-value
    // syntax (which needs `Default + Clone`) can't embed them —
    // attach them imperatively instead. Spawned at the top level (not
    // nested in a `with_children` closure) so its `Entity` id is available
    // below, for the dropdown list to parent itself to directly — see the
    // `list` comment for why.
    let toggle = commands
        .spawn_empty()
        .apply_scene(toggle_scene())
        .insert(ComboboxRoot(root))
        .id();
    commands.entity(row).add_child(toggle);
    commands.entity(toggle).with_children(|t| {
        t.spawn((
            Text::new(toggle_label(current)),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
            ComboboxToggleLabel(root),
        ));
    });

    // Absolutely positioned (out of flow) so opening it overlays the rest of
    // the page instead of pushing it down. A child of `toggle` (not `root`,
    // which spans the whole label+toggle row) so `Popover`'s `Start`
    // alignment lands it flush with the toggle's own left edge, same as the
    // old hand-authored `left: Px(LABEL_WIDTH + LABEL_GAP)` offset it
    // replaces. `Popover` (`bevy::ui_widgets`) repositions it every frame to
    // whichever of its candidate placements fits the window best — opening
    // below the toggle normally, above it if that would run past the bottom
    // edge — instead of a fixed `top: 100%` that has no idea where the
    // window's edges are. `GlobalZIndex` puts it above both normal page
    // content and the backdrop below.
    let list = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor::all(PANEL_BORDER),
            GlobalZIndex(250),
            Popover {
                positions: vec![
                    PopoverPlacement {
                        side: PopoverSide::Bottom,
                        align: PopoverAlign::Start,
                        gap: 4.0,
                    },
                    PopoverPlacement {
                        side: PopoverSide::Top,
                        align: PopoverAlign::Start,
                        gap: 4.0,
                    },
                ],
                window_margin: 8.0,
            },
        ))
        .id();
    commands.entity(list).with_children(|l| {
        for value in options {
            let is_selected = value == current;
            l.spawn_empty()
                .apply_scene(item_scene(value.clone(), is_selected))
                .insert(ComboboxItemButton {
                    root,
                    value: value.clone(),
                });
        }
    });
    commands.entity(toggle).add_child(list);

    // Full-screen invisible click-catcher, a *direct* child of
    // `backdrop_parent` (not nested under `root`) so its 100% size resolves
    // against the page's own full-screen box rather than this widget's
    // small one — same technique as `dialogs::file_dialog`'s overlay.
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(240),
            ComboboxRoot(root),
        ))
        .observe(backdrop_click)
        .id();
    commands.entity(backdrop_parent).add_child(backdrop);

    commands
        .entity(root)
        .insert(ComboboxLinks { list, backdrop });
    commands.entity(trigger_parent).add_child(root);
    root
}

/// The toggle button's visuals + click behaviour: shows the current value +
/// a chevron, and opens/closes the dropdown (and its backdrop) on click. Its
/// `ComboboxRoot` back-pointer and its label child (with
/// `ComboboxToggleLabel`) are attached imperatively by the caller — see the
/// comment at its only call site.
fn toggle_scene() -> impl Scene {
    bsn! {
        Button
        Node {
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(8.0))},
            min_width: {Val::Px(TOGGLE_MIN_WIDTH)},
        }
        BackgroundColor({button::color_default()})
        on(toggle_click)
    }
}

/// One choice in the list: its label + hover, recoloured to
/// `button::CHOICE_SELECTED` by [`sync_combobox_visuals`] once selected.
fn item_scene(value: String, is_selected: bool) -> impl Scene {
    let color = if is_selected {
        button::CHOICE_SELECTED
    } else {
        button::color_default()
    };
    bsn! {
        Button
        Node { padding: {UiRect::axes(Val::Px(14.0), Val::Px(8.0))} }
        BackgroundColor({color})
        on(item_click)
        on(item_over)
        on(item_out)
        Children [
            (
                Text({value})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

fn set_combobox_open(
    root: Entity,
    open: bool,
    links: &Query<&ComboboxLinks>,
    nodes: &mut Query<&mut Node>,
) {
    let Ok(links) = links.get(root) else { return };
    if let Ok(mut node) = nodes.get_mut(links.list) {
        node.display = if open { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = nodes.get_mut(links.backdrop) {
        node.display = if open { Display::Flex } else { Display::None };
    }
}

fn is_combobox_open(root: Entity, links: &Query<&ComboboxLinks>, nodes: &Query<&mut Node>) -> bool {
    links
        .get(root)
        .ok()
        .and_then(|l| nodes.get(l.list).ok())
        .is_some_and(|n| n.display != Display::None)
}

fn toggle_click(
    ev: On<Pointer<Click>>,
    toggles: Query<&ComboboxRoot>,
    links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
) {
    let Ok(&ComboboxRoot(root)) = toggles.get(ev.entity) else {
        return;
    };
    let opening = !is_combobox_open(root, &links, &nodes);
    set_combobox_open(root, opening, &links, &mut nodes);
}

/// Clicking the backdrop means clicking outside the dropdown — close it
/// without touching `ComboboxValue`.
fn backdrop_click(
    ev: On<Pointer<Click>>,
    backdrops: Query<&ComboboxRoot>,
    links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
) {
    let Ok(&ComboboxRoot(root)) = backdrops.get(ev.entity) else {
        return;
    };
    set_combobox_open(root, false, &links, &mut nodes);
}

fn item_click(
    ev: On<Pointer<Click>>,
    items: Query<&ComboboxItemButton>,
    links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
    mut values: Query<&mut ComboboxValue>,
    mut commands: Commands,
) {
    let Ok(item) = items.get(ev.entity) else {
        return;
    };
    let (root, value) = (item.root, item.value.clone());
    if let Ok(mut v) = values.get_mut(root) {
        v.0 = value.clone();
    }
    set_combobox_open(root, false, &links, &mut nodes);
    commands.trigger(ComboboxSelect {
        combobox: root,
        value,
    });
}

fn item_over(
    ev: On<Pointer<Over>>,
    items: Query<&ComboboxItemButton>,
    values: Query<&ComboboxValue>,
    mut colors: Query<&mut BackgroundColor>,
) {
    let Ok(item) = items.get(ev.entity) else {
        return;
    };
    let is_selected = values.get(item.root).is_ok_and(|v| v.0 == item.value);
    if !is_selected && let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(button::CHOICE_HOVER);
    }
}

fn item_out(
    ev: On<Pointer<Out>>,
    items: Query<&ComboboxItemButton>,
    values: Query<&ComboboxValue>,
    mut colors: Query<&mut BackgroundColor>,
) {
    let Ok(item) = items.get(ev.entity) else {
        return;
    };
    let is_selected = values.get(item.root).is_ok_and(|v| v.0 == item.value);
    if !is_selected && let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(button::color_default());
    }
}

/// Recolour a combobox's items and refresh its toggle label whenever its
/// `ComboboxValue` changes — whether from a user pick or from external code
/// writing to it directly (e.g. the mic falling back to a different device).
fn sync_combobox_visuals(
    changed: Query<(Entity, &ComboboxValue), Changed<ComboboxValue>>,
    mut items: Query<(&ComboboxItemButton, &mut BackgroundColor)>,
    mut toggle_labels: Query<(&ComboboxToggleLabel, &mut Text)>,
) {
    for (root, value) in &changed {
        for (item, mut bg) in &mut items {
            if item.root != root {
                continue;
            }
            bg.0 = if item.value == value.0 {
                button::CHOICE_SELECTED
            } else {
                button::color_default()
            };
        }
        for (label, mut text) in &mut toggle_labels {
            if label.0 != root {
                continue;
            }
            **text = toggle_label(&value.0);
        }
    }
}

/// Closes every open combobox on Escape, consuming the keypress (so a
/// separately-registered "go back" Escape handler ordered `.after` this one
/// doesn't also fire on the same press) — but only when a dropdown was
/// actually open, so Escape still reaches that other handler otherwise.
/// `pub(crate)` since only same-crate code orders against it directly
/// (external callers just add [`ComboboxPlugin`], which registers it).
pub(crate) fn close_open_comboboxes_on_escape(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    all_links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    let any_open = all_links.iter().any(|links| {
        nodes
            .get(links.list)
            .is_ok_and(|n| n.display != Display::None)
    });
    if !any_open {
        return;
    }
    for links in &all_links {
        if let Ok(mut node) = nodes.get_mut(links.list) {
            node.display = Display::None;
        }
        if let Ok(mut node) = nodes.get_mut(links.backdrop) {
            node.display = Display::None;
        }
    }
    keyboard.clear_just_pressed(KeyCode::Escape);
}

/// Registers the always-on reactive systems every combobox needs
/// (visual sync + Escape-to-close). Add once per app.
pub struct ComboboxPlugin;

impl Plugin for ComboboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_combobox_visuals, close_open_comboboxes_on_escape),
        );
    }
}
