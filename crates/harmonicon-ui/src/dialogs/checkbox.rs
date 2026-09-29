// SPDX-License-Identifier: MIT

//! Reusable labelled checkbox: a small box (checkmark shown while checked)
//! plus a label, wired to `bevy_ui_widgets`' headless [`Checkbox`]. One
//! static label instead of a toggle button whose own text has to double as
//! the on/off readout — see `menu::pages::options`'s four Options-page
//! toggles for the motivating case.
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use bevy::ui_widgets::ValueChange;
//! # use harmonicon_ui::dialogs::checkbox;
//! # fn example(commands: &mut Commands, parent: Entity, enabled: bool) {
//! checkbox::spawn_checkbox(commands, parent, "Fullscreen", enabled, on_change);
//! # }
//! fn on_change(ev: On<ValueChange<bool>>, mut commands: Commands) {
//!     commands.entity(ev.source).insert(bevy::ui::Checked);
//! }
//! ```
//!
//! Register [`CheckboxPlugin`] once per app.

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{Checkbox, ValueChange, checkbox_self_update};

const BOX_SIZE: f32 = 18.0;
const BOX_BG: Color = Color::srgb(0.10, 0.10, 0.16);
const BOX_BORDER: Color = Color::srgb(0.45, 0.45, 0.55);
const CHECK_COLOR: Color = Color::srgb(0.35, 0.85, 0.45);

/// The checkmark glyph inside a checkbox box; shown only while its sibling
/// [`Checkbox`] entity carries [`Checked`] — see [`sync_checkbox_visuals`].
#[derive(Component, Default, Clone)]
struct CheckMark;

/// Spawns a `<box> <label>` row as a child of `parent`. `on_change` fires on
/// every click/key toggle (`ValueChange<bool>`) — the same shape as
/// `combobox::spawn_combobox`'s `on_select`. The widget's own visible check
/// state is driven by [`Checked`], added/removed by `bevy_ui_widgets`' own
/// [`checkbox_self_update`] (registered once in [`CheckboxPlugin`]), so the
/// caller's `on_change` only needs to persist the value, not manage the box.
pub fn spawn_checkbox<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    checked: bool,
    on_change: impl IntoObserverSystem<ValueChange<bool>, M> + Clone + Sync + 'static,
) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(row).with_children(|r| {
        let mut box_ec = r.spawn_empty();
        box_ec.apply_scene(bsn! {
            Checkbox
            TabIndex(0)
            Node {
                width: {Val::Px(BOX_SIZE)},
                height: {Val::Px(BOX_SIZE)},
                border: {UiRect::all(Val::Px(2.0))},
                align_items: {AlignItems::Center},
                justify_content: {JustifyContent::Center},
            }
            BackgroundColor({BOX_BG})
            ~{BorderColor::all(BOX_BORDER)}
            Children [
                Text("\u{2713}")
                TextFont { font_size: {FontSize::Px(14.0)} }
                TextColor({CHECK_COLOR})
                CheckMark
                ~{Visibility::Hidden}
            ]
        });
        if checked {
            box_ec.insert(Checked);
        }
        box_ec.observe(on_change);
        r.spawn_empty().apply_scene(bsn! {
            Text({label.to_string()})
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::WHITE})
        });
    });
    commands.entity(parent).add_child(row);
    row
}

/// Shows/hides each checkbox's own checkmark glyph to match whether it
/// currently carries [`Checked`]. Runs unconditionally (no `is_changed`
/// guard) since a handful of menu checkboxes cost nothing to re-sync every
/// frame, and `Checked` is added/removed via commands with no single
/// component-mutation event this could key off of instead.
fn sync_checkbox_visuals(
    boxes: Query<(&Children, Has<Checked>), With<Checkbox>>,
    mut marks: Query<&mut Visibility, With<CheckMark>>,
) {
    for (children, checked) in &boxes {
        let visible = if checked {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        for child in children {
            if let Ok(mut vis) = marks.get_mut(*child)
                && *vis != visible
            {
                *vis = visible;
            }
        }
    }
}

pub struct CheckboxPlugin;

impl Plugin for CheckboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(checkbox_self_update)
            .add_systems(Update, sync_checkbox_visuals);
    }
}
