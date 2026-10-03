// SPDX-License-Identifier: MIT

//! Reusable combobox: a labelled toggle button showing the current
//! selection, which opens an overlay list of choices above the rest of the
//! page when clicked. Dismissed by clicking outside, pressing Escape, or
//! picking an item — only picking an item changes the selection.
//!
//! `spawn_combobox`'s `backdrop_parent` must be a full-screen-sized
//! container (e.g. the page root from `menu::spawn_menu_root`) — the
//! click-catching backdrop sizes itself to 100% of `backdrop_parent`, and is
//! despawned whenever `backdrop_parent` is, so the widget needs no
//! lifecycle hooks of its own. `trigger_parent` is where the visible
//! label+toggle lives in-flow — usually the same entity as
//! `backdrop_parent`, but a page with its own nested columns (see
//! `gameplay::bending_trainer::setup`) can pass a narrower column here
//! while keeping the backdrop sized to the whole page.
//!
//! Register [`ComboboxPlugin`] once per app. If some other Escape handler
//! should only fire when no dropdown was open to close, order it
//! `.after(ComboboxEscapeSet)`.
//!
//! The dropdown list's on-screen position is managed by `bevy::ui_widgets`'
//! [`Popover`] component, not a fixed `top`/`left` — it opens below the
//! toggle normally, above it if that would run past the window's bottom
//! edge, and is clamped either way. `PopoverPlugin` runs this: it's part of
//! `bevy_ui_widgets::UiWidgetsPlugins`, which `DefaultPlugins` already
//! includes with the `bevy_ui_widgets` Cargo feature on.

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

use super::button;

const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.12, 0.98);
const PANEL_BORDER: Color = Color::srgb(0.30, 0.30, 0.40);
const TOGGLE_MIN_WIDTH: f32 = 220.0;
/// Sized for the longest translated label on the Options page ("Detecção
/// de tom"), so a label never wraps beside its dropdown; matches
/// `options::OPTIONS_LABEL_WIDTH`.
const LABEL_WIDTH: f32 = 190.0;
const LABEL_GAP: f32 = 14.0;

// The dropdown list caps itself to `MAX_VISIBLE_ITEMS` rows and scrolls
// past that (see the `list`/`items_area` split in `spawn_combobox`) instead
// of growing to fit however many options were passed in — an unbounded
// panel can overflow whatever dialog/page it's opened from. The pixel
// height is derived from `item_scene`'s own padding/font metrics rather
// than a guessed constant, so it stays right if those change.
const MAX_VISIBLE_ITEMS: f32 = 5.0;
const ITEM_ROW_GAP: f32 = 4.0;
const ITEM_VERTICAL_PADDING: f32 = 8.0;
const ITEM_FONT_SIZE: f32 = 16.0;
const ITEM_LINE_HEIGHT_FACTOR: f32 = 1.25;
const ITEM_HEIGHT_PX: f32 = ITEM_VERTICAL_PADDING * 2.0 + ITEM_FONT_SIZE * ITEM_LINE_HEIGHT_FACTOR;
const LIST_MAX_HEIGHT_PX: f32 =
    ITEM_HEIGHT_PX * MAX_VISIBLE_ITEMS + ITEM_ROW_GAP * (MAX_VISIBLE_ITEMS - 1.0);
const LIST_SCROLLBAR_THUMB: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);
const LIST_SCROLLBAR_TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);

/// Triggered on a combobox's root entity (the `Entity` returned by
/// [`spawn_combobox`]) when the user picks an item from the list. The widget
/// always updates its own [`ComboboxValue`] regardless of whether anyone is
/// listening; this is how the caller finds out and reacts (persist a
/// setting, reconnect a device, ...).
#[derive(Clone, Debug, EntityEvent)]
pub struct ComboboxSelect {
    #[event_target]
    pub combobox: Entity,
    /// The chosen option's text, as displayed.
    pub value: String,
    /// The chosen option's position in the `options` slice the combobox was
    /// spawned with. A page whose options are *localized* labels of an enum
    /// maps back through this (`Progression::all()[ev.index]`) rather than
    /// by parsing the text, so the label can be any language.
    pub index: usize,
}

/// The value currently shown by a combobox. The widget updates this itself
/// when the user picks an item; write to it directly to change the display
/// for reasons outside the widget (e.g. the underlying device falling back
/// to a different one on its own).
#[derive(Component, Clone, Debug, Default)]
pub struct ComboboxValue(pub String);

/// Links a combobox root to its overlay list and backdrop, so the toggle/
/// backdrop/item observers can find the rest of their own widget instance
/// instead of relying on a global singleton query — why multiple comboboxes
/// can coexist on one page. Private: callers order against
/// [`ComboboxEscapeSet`] rather than naming the system, so this stays an
/// ordering elsewhere.
///
/// `list` is the whole bordered popover panel (what gets shown/hidden);
/// `items_area` is the scrollable column inside it that actually parents the
/// `ComboboxItemButton`s, beside the panel's scrollbar (see
/// `spawn_combobox`).
#[derive(Component, Clone, Copy)]
pub(crate) struct ComboboxLinks {
    list: Entity,
    items_area: Entity,
    backdrop: Entity,
}

/// Back-pointer from a toggle button or backdrop to its combobox root.
#[derive(Component, Clone, Copy)]
struct ComboboxRoot(Entity);

/// One item in a combobox's list. `pub(crate)` only because it appears in
/// `close_open_comboboxes_on_escape`'s signature (see [`ComboboxLinks`] for
/// the same reasoning) — nothing outside this module otherwise touches it.
#[derive(Component, Clone)]
pub(crate) struct ComboboxItemButton {
    root: Entity,
    value: String,
    index: usize,
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
/// `on_select` as an observer system reacting to it, same as
/// `spawn_volume_slider`'s `on_change` for `ValueChange<f32>`.
pub fn spawn_combobox<M: 'static>(
    commands: &mut Commands,
    trigger_parent: Entity,
    backdrop_parent: Entity,
    label: &str,
    options: &[String],
    current: &str,
    on_select: impl IntoObserverSystem<ComboboxSelect, M>,
) -> Entity {
    let root = commands
        .spawn_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Column},
                row_gap: {Val::Px(6.0)},
            }
        })
        .insert(ComboboxValue(current.to_string()))
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
        .spawn_empty()
        .apply_scene(bsn! {
            Node {
                width: {Val::Px(LABEL_WIDTH)},
            }
            Text({label.to_string()})
            TextFont { font_size: {FontSize::Px(20.0)} }
            TextColor({Color::WHITE})
        })
        .id();
    commands.entity(row).add_child(label_entity);

    // `ComboboxRoot`/`ComboboxToggleLabel` wrap a bare `Entity`, which
    // has no meaningful `Default`, so `bsn!`'s inline component-value
    // syntax (which needs `Default + Clone`) can't embed them —
    // attach them imperatively instead. Spawned at the top level (not
    // nested in a `with_children` closure) so its `Entity` id is available
    // below, for the dropdown list to parent itself to directly — see the
    // `list` comment for why.
    let toggle = commands.spawn_empty().apply_scene(toggle_scene()).insert(ComboboxRoot(root)).id();
    commands.entity(row).add_child(toggle);
    commands.entity(toggle).with_children(|t| {
        t.spawn_empty()
            .apply_scene(bsn! {
                Text({toggle_label(current)})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::WHITE})
                ~{Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                }}
            })
            .insert(ComboboxToggleLabel(root));
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
    //
    // `list` itself stays the same plain absolutely-positioned frame Popover
    // has always measured/positioned — the new scroll-capped content lives
    // one level deeper, in a nested in-flow `inner_row` (a `Row` holding
    // `items_area` beside its own `Scrollbar`, the exact same composition
    // `dialogs::scroll_area::spawn_scroll_area` already uses elsewhere,
    // just inlined here so `items_area` can additionally cap itself to
    // `LIST_MAX_HEIGHT_PX`) instead of growing to fit however many
    // `options` there are. Keeping `list` itself a bare `Column` (no
    // `Row`/`Stretch` of its own) means `Popover`'s own sizing/positioning
    // pass — which needs `list`'s `ComputedNode` — only ever has to resolve
    // one ordinary in-flow child, not reason about stretch-vs-max-height on
    // the absolutely-positioned node itself.
    //
    // Marking `items_area` a `ScrollArea` also gives the dropdown its "acts
    // as its own modal panel" behaviour for wheel input: `bevy_ui_widgets`'
    // `PointerScroll` observer calls `propagate(false)` the moment it
    // reaches a `ScrollArea` ancestor of the hovered item, so a wheel
    // scroll over the open dropdown is consumed there and never reaches
    // whatever page-level `ScrollArea` the combobox happens to be nested
    // inside (see `dialogs::scroll_area`/`menu::scene::spawn_menu_root`) —
    // only one of the two scrolls, ever.
    let list = commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                flex_direction: {FlexDirection::Column},
                padding: {UiRect::all(Val::Px(6.0))},
                border: {UiRect::all(Val::Px(1.0))},
                display: {Display::None},
            }
            BackgroundColor({PANEL_BG})
            ~{BorderColor::all(PANEL_BORDER)}
            GlobalZIndex(250)
        })
        .insert(Popover {
            positions: vec![
                PopoverPlacement {
                    side: PopoverSide::Bottom,
                    align: PopoverAlign::Start,
                    gap: 4.0,
                },
                PopoverPlacement { side: PopoverSide::Top, align: PopoverAlign::Start, gap: 4.0 },
            ],
            window_margin: 8.0,
        })
        .id();
    let mut items_area = Entity::PLACEHOLDER;
    commands.entity(list).with_children(|l| {
        l.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|row| {
            items_area = row
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(ITEM_ROW_GAP),
                        max_height: Val::Px(LIST_MAX_HEIGHT_PX),
                        min_height: Val::Px(0.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollArea,
                ))
                .id();
            row.spawn((
                Scrollbar::new(items_area, ControlOrientation::Vertical, 24.0),
                Node {
                    width: Val::Px(8.0),
                    flex_shrink: 0.0,
                    margin: UiRect::left(Val::Px(6.0)),
                    // Same "starts collapsed" reasoning as `dialogs::
                    // scroll_area::spawn_scroll_area`'s own track — avoids a
                    // one-frame flash of a full-height thumb before
                    // `update_scrollbar_visibility` (registered app-wide by
                    // `dialogs::scroll_area::ScrollAreaPlugin`, which matches
                    // any `Scrollbar`/`ScrollArea` pair, not just its own
                    // callers' markup) first corrects it, and collapses
                    // again for any dropdown short enough not to need it.
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(LIST_SCROLLBAR_TRACK),
                Visibility::Hidden,
            ))
            .with_children(|track| {
                track.spawn((
                    ScrollbarThumb {
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        border: UiRect::ZERO,
                    },
                    BackgroundColor(LIST_SCROLLBAR_THUMB),
                ));
            });
        });
    });
    commands.entity(items_area).with_children(|l| {
        for (index, value) in options.iter().enumerate() {
            let is_selected = value == current;
            l.spawn_empty()
                .apply_scene(item_scene(value.clone(), is_selected))
                .insert(ComboboxItemButton { root, value: value.clone(), index });
        }
    });
    commands.entity(toggle).add_child(list);

    // Full-screen invisible click-catcher, a *direct* child of
    // `backdrop_parent` (not nested under `root`) so its 100% size resolves
    // against the page's own full-screen box rather than this widget's
    // small one — same technique as `dialogs::file_dialog`'s overlay.
    let backdrop = commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                left: {Val::Px(0.0)},
                top: {Val::Px(0.0)},
                width: {Val::Percent(100.0)},
                height: {Val::Percent(100.0)},
                display: {Display::None},
            }
            GlobalZIndex(240)
        })
        .insert(ComboboxRoot(root))
        .observe(backdrop_click)
        .id();
    commands.entity(backdrop_parent).add_child(backdrop);

    commands.entity(root).insert(ComboboxLinks { list, items_area, backdrop });
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
        WidgetButton
        TabIndex(0)
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
    let color = if is_selected { button::CHOICE_SELECTED } else { button::color_default() };
    bsn! {
        WidgetButton
        // Starts unreachable via Tab (negative index) since the dropdown
        // starts closed — set_combobox_open flips this to 0 while open.
        TabIndex(-1)
        Node { padding: {UiRect::axes(Val::Px(14.0), Val::Px(8.0))} }
        BackgroundColor({color})
        on(item_click)
        on(item_over)
        on(item_out)
        Children [
                Text({value})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

/// Opens/closes the dropdown, and — since `TabNavigation::gather_focusable`
/// walks the ECS tree by `TabIndex`/`Children` alone, with no visibility
/// check at all — also flips each item's own `TabIndex` sign in step with
/// `Display`, so Tab can't land on a closed dropdown's invisible options
/// (a negative index means "not reachable via sequential navigation", per
/// `bevy_input_focus::tab_navigation`'s own doc comment).
fn set_combobox_open(
    root: Entity,
    open: bool,
    links: &Query<&ComboboxLinks>,
    nodes: &mut Query<&mut Node>,
    list_children: &Query<&Children>,
    item_tab_indices: &mut Query<&mut TabIndex, With<ComboboxItemButton>>,
) {
    let Ok(links) = links.get(root) else { return };
    if let Ok(mut node) = nodes.get_mut(links.list) {
        node.display = if open { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = nodes.get_mut(links.backdrop) {
        node.display = if open { Display::Flex } else { Display::None };
    }
    if let Ok(children) = list_children.get(links.items_area) {
        for &child in children {
            if let Ok(mut tab_index) = item_tab_indices.get_mut(child) {
                tab_index.0 = if open { 0 } else { -1 };
            }
        }
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
    ev: On<Activate>,
    toggles: Query<&ComboboxRoot>,
    links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
    list_children: Query<&Children>,
    mut item_tab_indices: Query<&mut TabIndex, With<ComboboxItemButton>>,
) {
    let Ok(&ComboboxRoot(root)) = toggles.get(ev.entity) else {
        return;
    };
    let opening = !is_combobox_open(root, &links, &nodes);
    set_combobox_open(root, opening, &links, &mut nodes, &list_children, &mut item_tab_indices);
}

/// Clicking the backdrop means clicking outside the dropdown — close it
/// without touching `ComboboxValue`.
// not-a-widget-button: the backdrop is a bare full-screen Node whose only
// job is catching clicks outside the dropdown; it has no Button and is not
// keyboard-reachable by design (Escape closes the dropdown instead).
fn backdrop_click(
    mut ev: On<PointerClick>,
    backdrops: Query<&ComboboxRoot>,
    links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
    list_children: Query<&Children>,
    mut item_tab_indices: Query<&mut TabIndex, With<ComboboxItemButton>>,
) {
    ev.propagate(false);
    let Ok(&ComboboxRoot(root)) = backdrops.get(ev.entity) else {
        return;
    };
    set_combobox_open(root, false, &links, &mut nodes, &list_children, &mut item_tab_indices);
}

#[allow(clippy::too_many_arguments)]
fn item_click(
    ev: On<Activate>,
    items: Query<&ComboboxItemButton>,
    links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
    mut values: Query<&mut ComboboxValue>,
    list_children: Query<&Children>,
    mut item_tab_indices: Query<&mut TabIndex, With<ComboboxItemButton>>,
    mut commands: Commands,
) {
    let Ok(item) = items.get(ev.entity) else {
        return;
    };
    let (root, value, index) = (item.root, item.value.clone(), item.index);
    if let Ok(mut v) = values.get_mut(root) {
        v.0 = value.clone();
    }
    set_combobox_open(root, false, &links, &mut nodes, &list_children, &mut item_tab_indices);
    commands.trigger(ComboboxSelect { combobox: root, value, index });
}

fn item_over(
    ev: On<PointerOver>,
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
    ev: On<PointerOut>,
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
fn close_open_comboboxes_on_escape(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    all_links: Query<&ComboboxLinks>,
    mut nodes: Query<&mut Node>,
    list_children: Query<&Children>,
    mut item_tab_indices: Query<&mut TabIndex, With<ComboboxItemButton>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    let any_open = all_links
        .iter()
        .any(|links| nodes.get(links.list).is_ok_and(|n| n.display != Display::None));
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
        if let Ok(children) = list_children.get(links.items_area) {
            for &child in children {
                if let Ok(mut tab_index) = item_tab_indices.get_mut(child) {
                    tab_index.0 = -1;
                }
            }
        }
    }
    keyboard.clear_just_pressed(KeyCode::Escape);
}

/// Registers the always-on reactive systems every combobox needs
/// The Escape-to-close pass. Another Escape handler that should only fire
/// when no dropdown was open to close orders itself `.after` this set —
/// an ordering point the widget publishes deliberately, so its systems and
/// their parameter types stay private.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComboboxEscapeSet;

/// (visual sync + Escape-to-close). Add once per app.
pub struct ComboboxPlugin;

impl Plugin for ComboboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_combobox_visuals, close_open_comboboxes_on_escape.in_set(ComboboxEscapeSet)),
        );
    }
}
