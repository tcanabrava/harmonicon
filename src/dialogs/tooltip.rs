// SPDX-License-Identifier: MIT

//! A hover tooltip. Attach [`Tooltip`] to any pickable entity (a `Button`,
//! typically) and a small floating label follows the cursor near it while
//! the pointer stays over that entity, showing the tooltip's text.
//!
//! ```ignore
//! use crate::dialogs::tooltip::Tooltip;
//! parent.spawn((Button, /* ... */, Tooltip(String::from(loc.msg("some-key")))));
//! ```

use bevy::picking::Pickable;
use bevy::picking::events::{Out, Over, Pointer};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Text to show in a floating tooltip when this entity is hovered. Expected
/// to already be localized (`loc.msg(...)`) by the caller — see
/// `crate::localization`.
#[derive(Component, Clone)]
pub struct Tooltip(pub String);

/// The floating tooltip panel — one instance, repositioned and re-labelled
/// as the hovered entity changes rather than spawned per-widget.
#[derive(Component)]
struct TooltipRoot;

#[derive(Component)]
struct TooltipText;

/// The entity currently under the pointer that carries a [`Tooltip`], if
/// any. Tracked as a resource (rather than derived purely from the
/// Over/Out events) so [`update_tooltip`] can keep following the cursor
/// every frame for as long as it stays hovered.
#[derive(Resource, Default)]
struct HoveredTooltip(Option<Entity>);

const TOOLTIP_BG: Color = Color::srgba(0.05, 0.05, 0.08, 0.95);
const TOOLTIP_BORDER: Color = Color::srgb(0.35, 0.35, 0.45);
/// Offset (logical px) from the cursor tip so the tooltip doesn't sit
/// directly under — and immediately re-trigger Out/Over on — the pointer.
const CURSOR_OFFSET: f32 = 16.0;

fn spawn_tooltip_root(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                max_width: Val::Px(320.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(TOOLTIP_BG),
            BorderColor::all(TOOLTIP_BORDER),
            GlobalZIndex(1000),
            Visibility::Hidden,
            Pickable::IGNORE,
            TooltipRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TooltipText,
                Pickable::IGNORE,
            ));
        });
}

fn on_hover_start(
    ev: On<Pointer<Over>>,
    tooltips: Query<&Tooltip>,
    mut hovered: ResMut<HoveredTooltip>,
) {
    if tooltips.contains(ev.entity) {
        hovered.0 = Some(ev.entity);
    }
}

fn on_hover_end(ev: On<Pointer<Out>>, mut hovered: ResMut<HoveredTooltip>) {
    if hovered.0 == Some(ev.entity) {
        hovered.0 = None;
    }
}

/// Keeps the tooltip panel's visibility, text, and position in step with
/// [`HoveredTooltip`] — written every frame while visible so it tracks the
/// cursor, not just once on hover start.
fn update_tooltip(
    hovered: Res<HoveredTooltip>,
    tooltips: Query<&Tooltip>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut roots: Query<(&mut Node, &mut Visibility), With<TooltipRoot>>,
    mut texts: Query<&mut Text, With<TooltipText>>,
) {
    let Ok((mut node, mut vis)) = roots.single_mut() else {
        return;
    };
    let tooltip = hovered.0.and_then(|e| tooltips.get(e).ok());
    let Some(tooltip) = tooltip else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };

    if *vis != Visibility::Visible {
        *vis = Visibility::Visible;
    }
    // `cursor_position()` is in logical window pixels; `Val::Px` is further
    // scaled by `UiScale` at layout time (see the note-label fix in
    // `gameplay_3d.rs`), so divide it back out here to land at the cursor.
    node.left = Val::Px(cursor.x / ui_scale.0 + CURSOR_OFFSET);
    node.top = Val::Px(cursor.y / ui_scale.0 + CURSOR_OFFSET);

    for mut text in &mut texts {
        if text.0 != tooltip.0 {
            *text = Text::new(tooltip.0.clone());
        }
    }
}

pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredTooltip>()
            .add_systems(Startup, spawn_tooltip_root)
            .add_observer(on_hover_start)
            .add_observer(on_hover_end)
            .add_systems(Update, update_tooltip);
    }
}
