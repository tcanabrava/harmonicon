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

use bevy::picking::events::{Drag, DragStart, Pointer};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, Pressed, ScrollPosition};
use bevy::ui_widgets::{
    Button as WidgetButton, ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb,
};

/// Scroll offset captured when a pointer drag begins.
#[derive(Component, Default)]
struct DragScrollStart(Vec2);

/// Makes a freshly spawned scroll area pannable by dragging its content,
/// and returns its entity.
///
/// **Every scroll area goes through here**, which is the point: touch has
/// no wheel, so an area without this can only be scrolled by dragging a
/// 10px scrollbar thumb. Wiring it at each spawn site instead let the two
/// of them disagree, and for a while only the lesson tree could be panned.
///
/// Both axes are handled by one implementation because
/// [`bounded_scroll_position`] reads the area's own [`Overflow`]: a
/// vertical-only area stays pinned at x with nothing extra said here.
fn drag_to_pan(mut area: EntityCommands) -> Entity {
    area.insert(DragScrollStart::default())
        .observe(begin_drag_scroll)
        .observe(drag_scroll)
        .id()
}

fn begin_drag_scroll(
    drag: On<Pointer<DragStart>>,
    mut commands: Commands,
    mut areas: Query<(&ComputedNode, &mut DragScrollStart), With<ScrollArea>>,
    buttons: Query<(), With<WidgetButton>>,
) {
    let Ok((computed, mut start)) = areas.get_mut(drag.entity) else {
        return;
    };
    start.0 = computed.scroll_position * computed.inverse_scale_factor;

    // Bevy dispatches Click before DragEnd on release, and Button activates
    // that click while Pressed is still present. Without cancelling it here,
    // a swipe that begins and ends over the same lesson opens the lesson after
    // moving the map. Controls such as sliders consume DragStart themselves,
    // so their gesture never bubbles here and their pressed state is untouched.
    let origin = drag.original_event_target();
    if buttons.contains(origin) {
        commands.entity(origin).remove::<Pressed>();
    }
}

fn drag_scroll(
    drag: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    mut areas: Query<
        (&Node, &ComputedNode, &DragScrollStart, &mut ScrollPosition),
        With<ScrollArea>,
    >,
) {
    let Ok((node, computed, start, mut position)) = areas.get_mut(drag.entity) else {
        return;
    };

    let visible_size = computed.size() * computed.inverse_scale_factor;
    let content_size = computed.content_size() * computed.inverse_scale_factor;
    let requested = start.0 - drag.distance / ui_scale.0;
    position.0 = bounded_scroll_position(node.overflow, requested, content_size - visible_size);
}

fn bounded_scroll_position(overflow: Overflow, requested: Vec2, available_range: Vec2) -> Vec2 {
    let max = available_range.max(Vec2::ZERO);
    Vec2::new(
        if overflow.x == OverflowAxis::Scroll {
            requested.x.clamp(0.0, max.x)
        } else {
            0.0
        },
        if overflow.y == OverflowAxis::Scroll {
            requested.y.clamp(0.0, max.y)
        } else {
            0.0
        },
    )
}

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
            area = drag_to_pan(outer.spawn((
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
            )));
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
                    area = drag_to_pan(row.spawn((
                        Node {
                            min_height: Val::Px(0.0),
                            min_width: Val::Px(0.0),
                            flex_grow: 1.0,
                            overflow: Overflow::scroll(),
                            ..default()
                        },
                        ScrollArea,
                    )));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spawner_wires_up_drag_panning() {
        // The one invariant `drag_to_pan` exists to hold. Touch has no
        // wheel, so a scroll area without it can only be moved by dragging
        // a 10px scrollbar thumb — and an area missing it looks perfectly
        // fine on a desktop, where the wheel hides the omission entirely.
        // `DragScrollStart` stands in for the observers here: it is
        // inserted in the same place they are attached, and nothing else
        // inserts it.
        let mut world = World::new();
        let mut areas = Vec::new();
        {
            let mut commands = world.commands();
            commands.spawn_empty().with_children(|parent| {
                areas.push(spawn_scroll_area(parent, Color::WHITE, Color::BLACK));
                areas.push(spawn_scroll_area_xy(parent, Color::WHITE, Color::BLACK));
            });
        }
        world.flush();

        assert_eq!(areas.len(), 2, "a spawner was added without a case here");
        for area in areas {
            assert!(
                world.get::<ScrollArea>(area).is_some(),
                "{area} is not the scroll area itself"
            );
            assert!(
                world.get::<DragScrollStart>(area).is_some(),
                "{area} cannot be panned by dragging"
            );
        }
    }

    #[test]
    fn drag_scroll_is_bounded_on_both_axes() {
        let overflow = Overflow::scroll();

        assert_eq!(
            bounded_scroll_position(overflow, Vec2::new(240.0, -20.0), Vec2::new(200.0, 300.0),),
            Vec2::new(200.0, 0.0),
        );
        assert_eq!(
            bounded_scroll_position(overflow, Vec2::new(80.0, 120.0), Vec2::new(200.0, 300.0),),
            Vec2::new(80.0, 120.0),
        );
    }

    #[test]
    fn drag_scroll_leaves_disabled_axes_at_origin() {
        assert_eq!(
            bounded_scroll_position(
                Overflow::scroll_y(),
                Vec2::new(80.0, 120.0),
                Vec2::new(200.0, 300.0),
            ),
            Vec2::new(0.0, 120.0),
        );
    }
}
