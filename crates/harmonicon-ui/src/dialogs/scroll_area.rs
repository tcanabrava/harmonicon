// SPDX-License-Identifier: MIT

//! A generic vertically-scrollable content area with a real, visible
//! scrollbar beside it — `bevy_ui_widgets::ScrollArea` alone gives wheel/
//! drag scrolling, but with nothing painted on screen there's no hint that
//! a page has more content than fits; pairing it with a
//! [`Scrollbar`]/[`ScrollbarThumb`] (drag/click-to-page already wired by
//! `UiWidgetsPlugins`) is what makes that visible. The scrollbar hides
//! itself entirely once its content already fits — see
//! [`update_scrollbar_visibility`]. Generic and theme-agnostic, shared by
//! `menu::scene::spawn_menu_root` (any page whose content can outgrow the
//! screen — a long artist/song/lesson/theme list) and the Song Editor.

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
            // The "min-height: auto" flexbox gotcha, one level up from the
            // `ScrollArea` column below: without this, this row (itself a
            // flex item of the menu's top-level column) refuses to shrink
            // below its content's natural height on resize, so the
            // `ScrollArea` inside never receives less room than it needs
            // and `update_scrollbar_visibility`'s overflow check never trips.
            min_height: Val::Px(0.0),
            ..default()
        })
        .with_children(|outer| {
            area = outer
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(16.0),
                        // Same "min-height: auto" gotcha: zeroing it lets
                        // this column force-shrink to whatever room is left
                        // under the title once content no longer fits,
                        // handing the rest to scrolling via `overflow`
                        // below instead of running past the edges.
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
                        // visibility`'s first run corrects it. `Display::
                        // None`, not just `Visibility::Hidden`: many menu
                        // pages rely on perfectly horizontal centering, and
                        // a merely-invisible-but-still-laid-out track would
                        // reserve its width, nudging content off-center.
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

/// Like [`spawn_scroll_area`], but scrollable on *both* axes, with a
/// scrollbar down the right and another along the bottom.
///
/// For content that is genuinely bigger than the window in both
/// directions rather than merely long — the lesson skill tree, whose
/// canvas is wider than any window once the curriculum is spread so no
/// column stacks more than a few nodes deep. An ordinary page should still
/// use [`spawn_scroll_area`]: a horizontal scrollbar under a page that
/// never needs one is noise.
///
/// Returns the scroll area's entity, as its sibling does.
pub fn spawn_scroll_area_xy(
    parent: &mut ChildSpawnerCommands,
    thumb_color: Color,
    track_color: Color,
) -> Entity {
    let mut area = Entity::PLACEHOLDER;
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            // Same "min-height: auto" gotcha `spawn_scroll_area` documents:
            // without this the column refuses to shrink below its content
            // and the overflow check never trips.
            min_height: Val::Px(0.0),
            min_width: Val::Px(0.0),
            flex_grow: 1.0,
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|column| {
            column
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    min_height: Val::Px(0.0),
                    min_width: Val::Px(0.0),
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|row| {
                    area = row
                        .spawn((
                            Node {
                                min_height: Val::Px(0.0),
                                min_width: Val::Px(0.0),
                                flex_grow: 1.0,
                                overflow: Overflow::scroll(),
                                ..default()
                            },
                            ScrollArea,
                        ))
                        .id();
                    row.spawn((
                        Scrollbar::new(area, ControlOrientation::Vertical, 24.0),
                        Node {
                            width: Val::Px(10.0),
                            flex_shrink: 0.0,
                            margin: UiRect::left(Val::Px(8.0)),
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
            column
                .spawn((
                    Scrollbar::new(area, ControlOrientation::Horizontal, 24.0),
                    Node {
                        height: Val::Px(10.0),
                        flex_shrink: 0.0,
                        margin: UiRect::top(Val::Px(8.0)),
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
/// update_grid_scrollbar` uses for its own horizontal one. Matches each
/// [`Scrollbar`] to its own `ScrollArea` via [`Scrollbar::target`], so this
/// is registered once for the whole app (see [`ScrollAreaPlugin`]) rather
/// than per caller.
///
/// Each bar is judged on **its own axis**: a two-axis area
/// ([`spawn_scroll_area_xy`]) routinely needs one bar and not the other,
/// and measuring both against height would show a horizontal bar for
/// content that is merely tall.
pub fn update_scrollbar_visibility(
    mut bars: Query<(&Scrollbar, &mut Visibility, &mut Node)>,
    areas: Query<&ComputedNode, With<ScrollArea>>,
) {
    for (bar, mut vis, mut node) in &mut bars {
        let Ok(area) = areas.get(bar.target) else {
            continue;
        };
        let needed = match bar.orientation {
            ControlOrientation::Vertical => area.content_size().y > area.size().y + 1.0,
            ControlOrientation::Horizontal => area.content_size().x > area.size().x + 1.0,
        };
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
