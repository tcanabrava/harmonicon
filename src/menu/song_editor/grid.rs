// SPDX-License-Identifier: MIT

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::MaterialNode;

use super::{
    grid_height, BEAT_W, BEATS_PER_BAR,
    HEADER_H, ROW_H, ROWS,
    TICK_W, TICKS_PER_BEAT, HANDLE_W,
};
use super::material::EditorNoteMaterial;
use crate::gameplay::twelve_bar_blues_overlay::bar_bg;
use crate::localization::LocalizationExt;
use crate::theme::{LoadedTheme, SongEditorColors};
use bevy_fluent::prelude::Localization;
use super::state::{
    can_place, enforce_direction, note_rect, pitch_color, pitch_compatible, pitch_deny_key,
    move_target, DragKind, DragState, Edge, EditorState, Expr, GridNote, Pitch,
};
use super::ui::{GridContent, GridItem, NoteView};
use super::interaction::select_or_add;

pub(super) fn visible_beats(win_w: f32) -> usize {
    (((win_w - super::HOLE_COL_W) / BEAT_W).ceil() as usize) + 1
}

/// How strongly a bar's 12-bar-blues chord-function tint (see [`bar_bg`])
/// shows through the lane's own alternating-row color. Low enough to keep
/// the checkerboard readable and not compete with note blocks.
const BAR_TINT_MIX: f32 = 0.35;

/// Blends `tint` into `base` by `t` (0 = pure `base`, 1 = pure `tint`),
/// keeping `base`'s own alpha so lane cells stay fully opaque.
pub(super) fn mix_srgba(base: Color, tint: Color, t: f32) -> Color {
    let b = base.to_srgba();
    let c = tint.to_srgba();
    Color::srgba(
        b.red + (c.red - b.red) * t,
        b.green + (c.green - b.green) * t,
        b.blue + (c.blue - b.blue) * t,
        b.alpha,
    )
}

pub(super) fn rebuild_grid(
    mut commands: Commands,
    state: Res<EditorState>,
    content: Query<Entity, With<GridContent>>,
    old: Query<Entity, With<GridItem>>,
    windows: Query<&Window>,
    mut note_mats: ResMut<Assets<EditorNoteMaterial>>,
    theme: Res<LoadedTheme>,
) {
    if state.dragging.is_some() {
        return;
    }
    let colors = theme.song_editor_colors();
    let bar_colors = theme.twelve_bar_colors();
    for e in &old {
        commands.entity(e).despawn();
    }
    let Ok(content) = content.single() else { return };
    let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);
    let cols = visible_beats(win_w);
    let mut items: Vec<Entity> = Vec::new();

    for col in 0..=cols {
        let beat = state.scroll_beat + col;
        let x = beat as f32 * BEAT_W;
        let is_bar = beat % BEATS_PER_BAR == 0;
        // Tiles the standard 12-bar-blues form indefinitely as the user
        // scrolls, so the grid reads as harmonic function (I/IV/V) even for
        // charts longer than 12 bars.
        let bar_index = (beat / BEATS_PER_BAR) % 12;
        let bar_tint = bar_bg(bar_index, &state.key, bar_colors);

        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(0.0),
                        width: Val::Px(if is_bar { 2.0 } else { 1.0 }),
                        height: Val::Px(grid_height()),
                        ..default()
                    },
                    BackgroundColor(if is_bar { colors.bar_line } else { colors.grid_line }),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        for s in 1..TICKS_PER_BEAT {
            let is_half = s * 2 == TICKS_PER_BEAT;
            items.push(
                commands
                    .spawn((
                        GridItem,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x + s as f32 * TICK_W),
                            top: Val::Px(HEADER_H),
                            width: Val::Px(1.0),
                            height: Val::Px(grid_height() - HEADER_H),
                            ..default()
                        },
                        BackgroundColor(if is_half { colors.half_line } else { colors.quarter_line }),
                        Pickable::IGNORE,
                    ))
                    .id(),
            );
        }

        let in_bar = beat % BEATS_PER_BAR + 1;
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + 4.0),
                        top: Val::Px(6.0),
                        ..default()
                    },
                    Text::new(format!("{in_bar}")),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(if is_bar { colors.accent } else { colors.label }),
                    Pickable::IGNORE,
                ))
                .id(),
        );
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + BEAT_W * 0.5 + 2.0),
                        top: Val::Px(6.0),
                        ..default()
                    },
                    Text::new("&"),
                    TextFont { font_size: FontSize::Px(11.0), ..default() },
                    TextColor(Color::srgb(0.45, 0.45, 0.55)),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        for hole in 1..=ROWS {
            let y = HEADER_H + (hole as f32 - 1.0) * ROW_H;
            let lane = if hole % 2 == 0 { colors.lane_a } else { colors.lane_b };
            let lane = mix_srgba(lane, bar_tint, BAR_TINT_MIX);
            let mut cell = commands.spawn((
                GridItem,
                Button,
                RelativeCursorPosition::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(y),
                    width: Val::Px(BEAT_W),
                    height: Val::Px(ROW_H),
                    ..default()
                },
                BackgroundColor(lane),
            ));
            cell.observe(
                move |ev: On<Pointer<Click>>,
                      rel: Query<&RelativeCursorPosition>,
                      mut state: ResMut<EditorState>| {
                    let frac = rel
                        .get(ev.entity)
                        .ok()
                        .and_then(|r| r.normalized)
                        .map_or(0.0, |n| n.x)
                        .clamp(0.0, 0.999);
                    let sub = (frac * TICKS_PER_BEAT as f32).floor() as usize;
                    select_or_add(&mut state, hole, beat * TICKS_PER_BEAT + sub);
                },
            );
            items.push(cell.id());
        }
    }

    let first_tick = state.scroll_beat * TICKS_PER_BEAT;
    let last_tick = (state.scroll_beat + cols + 1) * TICKS_PER_BEAT;
    for note in &state.notes {
        if note.tick < last_tick && note.tick + note.len > first_tick {
            let selected = state.selected == Some(note.id);
            items.push(spawn_note(&mut commands, *note, selected, &mut note_mats, colors));
        }
    }

    commands.entity(content).add_children(&items);
}

pub(super) fn spawn_note(
    commands: &mut Commands,
    note: GridNote,
    selected: bool,
    note_mats: &mut Assets<EditorNoteMaterial>,
    colors: SongEditorColors,
) -> Entity {
    let (left, top, width, height) = note_rect(&note);
    let border = if selected { 2.0 } else { 0.0 };
    let border_color = if selected { colors.accent } else { Color::NONE };
    let id = note.id;

    let root = commands
        .spawn((
            GridItem,
            NoteView(id),
            Button,
            ZIndex(1),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(width),
                height: Val::Px(height),
                border: UiRect::all(Val::Px(border)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(border_color),
        ))
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            state.selected = Some(id);
        })
        .observe(move |_: On<Pointer<DragStart>>, mut state: ResMut<EditorState>| {
            if state.dragging.is_some() { return; }
            if let Some(n) = state.note_by_id(id).copied() {
                state.selected = Some(id);
                state.dragging = Some(DragState::new(id, DragKind::Move, &n));
            }
        })
        .observe(move |ev: On<Pointer<Drag>>, mut state: ResMut<EditorState>, loc: Res<Localization>| {
            let Some(drag) = state.dragging else { return };
            if drag.id != id || drag.kind != DragKind::Move { return; }
            let (hole, tick) = move_target(drag.start_hole, drag.start_tick, ev.distance.x, ev.distance.y);
            let pitch = state.notes.iter().find(|n| n.id == id).map(|n| n.pitch).unwrap_or(Pitch::Normal);
            let place_ok = can_place(&state.notes, id, hole, tick, drag.start_len);
            let pitch_ok = pitch_compatible(pitch, hole);
            let valid = place_ok && pitch_ok;
            state.drag_msg = if !pitch_ok {
                loc.msg(pitch_deny_key(pitch, hole))
            } else if !place_ok {
                loc.msg("drag-denied-overlap")
            } else {
                crate::localization::LocalizedStr::default()
            };
            if let Some(d) = state.dragging.as_mut() {
                d.target_hole = hole;
                d.target_tick = tick;
                d.valid = valid;
            }
        })
        .observe(move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging.take() else { return };
            state.drag_msg = crate::localization::LocalizedStr::default();
            if drag.kind == DragKind::Move && drag.valid {
                if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                    n.hole = drag.target_hole;
                    n.tick = drag.target_tick;
                }
                enforce_direction(&mut state, id);
            }
        })
        .id();

    match note.expr {
        Expr::None => {
            commands.entity(root).insert(BackgroundColor(pitch_color(note.pitch)));
        }
        Expr::Wah | Expr::Vibrato => {
            let mode = if note.expr == Expr::Vibrato { 0.0 } else { 1.0 };
            let mat = note_mats.add(EditorNoteMaterial {
                color: pitch_color(note.pitch).to_linear(),
                params: Vec4::new(mode, 0.0, 0.0, 0.0),
            });
            commands.entity(root).insert(MaterialNode(mat));
        }
    }

    commands.entity(root).with_children(|r| {
        r.spawn((
            Text::new(note.dir.arrow()),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ));
        spawn_resize_handle(r, id, Edge::Left);
        spawn_resize_handle(r, id, Edge::Right);
    });

    root
}

fn spawn_resize_handle(parent: &mut ChildSpawnerCommands, id: u32, edge: Edge) {
    use super::state::apply_resize;
    let mut node = Node {
        position_type: PositionType::Absolute,
        top: Val::Px(0.0),
        bottom: Val::Px(0.0),
        width: Val::Px(HANDLE_W),
        ..default()
    };
    match edge {
        Edge::Left  => node.left  = Val::Px(0.0),
        Edge::Right => node.right = Val::Px(0.0),
    }
    parent
        .spawn((node, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25))))
        .observe(move |_: On<Pointer<DragStart>>, mut state: ResMut<EditorState>| {
            if let Some(n) = state.note_by_id(id).copied() {
                state.selected = Some(id);
                state.dragging = Some(DragState::new(id, DragKind::Resize(edge), &n));
            }
        })
        .observe(move |ev: On<Pointer<Drag>>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging else { return };
            if drag.id != id || drag.kind != DragKind::Resize(edge) { return; }
            let hole = drag.start_hole;
            let mut left_bound = 0usize;
            let mut right_bound: Option<usize> = None;
            for n in &state.notes {
                if n.id == id || n.hole != hole { continue; }
                if n.tick < drag.start_tick {
                    left_bound = left_bound.max(n.tick + n.len);
                } else {
                    right_bound = Some(right_bound.map_or(n.tick, |r| r.min(n.tick)));
                }
            }
            let steps = (ev.distance.x / TICK_W).round() as i32;
            let (tick, len) =
                apply_resize(drag.start_tick, drag.start_len, edge, steps, left_bound, right_bound);
            if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                n.tick = tick;
                n.len = len;
            }
        })
        .observe(move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
            if matches!(state.dragging, Some(d) if d.id == id && matches!(d.kind, DragKind::Resize(_))) {
                state.dragging = None;
                enforce_direction(&mut state, id);
            }
        });
}
