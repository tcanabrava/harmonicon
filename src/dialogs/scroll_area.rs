// SPDX-License-Identifier: MIT

//! A generic vertically-scrollable content area with a real, visible
//! scrollbar beside it — `bevy_ui_widgets::ScrollArea` alone gives wheel/
//! drag scrolling, but with nothing painted on screen there's no hint that
//! a page has more content than fits; pairing it with a
//! [`Scrollbar`]/[`ScrollbarThumb`] (drag/click-to-page already wired by
//! `UiWidgetsPlugins`, no hand-rolled interaction needed) is what makes
//! that visible. The scrollbar hides itself entirely once its content
//! already fits without scrolling — see [`update_scrollbar_visibility`].
//!
//! Originally the Song Editor's own `song_editor::scroll::
//! spawn_editor_scrollbar`; pulled out here, generic and theme-agnostic,
//! once `menu::scene::spawn_menu_root` needed the exact same thing for
//! every menu page — any page whose content can outgrow the screen (a long
//! artist/song/lesson/theme list) used to just silently overflow past the
//! edges with no way to reach the rest, since `spawn_menu_root`'s content
//! column had no `overflow`/size constraint of its own.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

/// Spawns a full "scrollable content area + visible scrollbar" unit as a
/// child of `parent`: an outer row holding the scrollable column (sized to
/// its own content, but force-shrinkable down to whatever room is actually
/// available — see the `min_height: Val::Px(0.0)` comment below) beside a
/// slim vertical scrollbar. Returns the scroll area's entity — what the
/// caller should add its actual page content children to; everything it
/// contains scrolls together once it no longer fits.
pub fn spawn_scroll_area(
    parent: &mut ChildSpawnerCommands,
    thumb_color: Color,
    track_color: Color,
) -> Entity {
    let mut area = Entity::PLACEHOLDER;
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|outer| {
            area = outer
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(16.0),
                        // The flexbox "min-height: auto" gotcha: by default
                        // a flex item won't shrink below its own content's
                        // natural size, so a tall page would just overflow
                        // the screen instead of activating `overflow`
                        // below. Explicitly zeroing it is what lets this
                        // column get force-shrunk to whatever room is left
                        // under the title once content no longer fits,
                        // handing the rest off to scrolling instead of
                        // silently running past the edges — the same fix
                        // `song_editor::ui::setup` already applies for its
                        // own scrollable form area.
                        min_height: Val::Px(0.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollArea,
                ))
                .id();
            outer
                .spawn((
                    Scrollbar::new(area, ControlOrientation::Vertical, 24.0),
                    Node {
                        width: Val::Px(10.0),
                        flex_shrink: 0.0,
                        margin: UiRect::left(Val::Px(8.0)),
                        // Starts collapsed — avoids a one-frame flash of a
                        // full-height thumb before `update_scrollbar_
                        // visibility`'s first run corrects it; also the
                        // right state for the common case of content that
                        // fits without scrolling. `Display::None`, not
                        // just `Visibility::Hidden`: many menu pages rely
                        // on their content being perfectly horizontally
                        // centered, and a merely-invisible-but-still-
                        // laid-out scrollbar track would reserve its own
                        // width, nudging that content slightly off-center
                        // even when there's nothing to scroll to.
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(track_color),
                    Visibility::Hidden,
                ))
                .with_children(|track| {
                    track.spawn((
                        ScrollbarThumb {
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            border: UiRect::ZERO,
                        },
                        BackgroundColor(thumb_color),
                    ));
                });
        });
    area
}

/// Hides a scrollbar entirely once its paired [`ScrollArea`]'s content
/// already fits without scrolling — same "don't show a scrollbar with
/// nothing to scroll to" convention `song_editor::interaction::
/// update_grid_scrollbar` uses for its own (differently-implemented,
/// predating this widget) horizontal one. Matches each [`Scrollbar`] to
/// its own `ScrollArea` via [`Scrollbar::target`], so this is registered
/// once for the whole app (see [`ScrollAreaPlugin`]) rather than per
/// caller, even though several unrelated scroll areas exist across
/// different screens (never literally at the same time today — only one
/// menu page or the Song Editor is ever on screen — but nothing here
/// assumes that).
pub fn update_scrollbar_visibility(
    mut bars: Query<(&Scrollbar, &mut Visibility, &mut Node)>,
    areas: Query<&ComputedNode, With<ScrollArea>>,
) {
    for (bar, mut vis, mut node) in &mut bars {
        let Ok(area) = areas.get(bar.target) else {
            continue;
        };
        let needed = area.content_size().y > area.size().y + 1.0;
        *vis = if needed {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if needed { Display::Flex } else { Display::None };
    }
}

/// Registers [`update_scrollbar_visibility`]. Add once per app.
pub struct ScrollAreaPlugin;

impl Plugin for ScrollAreaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_scrollbar_visibility);
    }
}
