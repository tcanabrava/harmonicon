// SPDX-License-Identifier: MIT

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Drag, DragEnd, DragStart, Pointer};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::MaterialNode;

use super::interaction::{ctrl_held, select_or_add, select_or_add_ctrl};
use super::material::EditorNoteMaterial;
use super::playback::{build_harp, note_freq};
use super::ranges::silence_gaps;
use super::state::{
    DragKind, DragState, Edge, EditorState, Expr, GridNote, Pitch, enforce_direction,
    enforce_expr, move_target, note_rect, pitch_color, pitch_compatible, pitch_deny_key,
};
use super::ui::{GridContent, GridItem, NoteView};
use super::{
    BEAT_W, BEATS_PER_BAR, HANDLE_W, HEADER_H, ROW_H, SILENCE_ROW_H, TICK_W, TICKS_PER_BEAT,
    WAVEFORM_H, WAVEFORM_TOP, grid_height, silence_row_top,
};
use crate::audio_system::midi::{freq_to_midi, midi_to_note};
use crate::gameplay::twelve_bar_blues_overlay::bar_bg;
use crate::localization::LocalizationExt;
use crate::song::harmonica::Harmonica;
use crate::theme::{LoadedTheme, SongEditorColors};
use bevy_fluent::prelude::Localization;
use std::collections::HashSet;

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

/// How strongly the "outside the blues scale" warning tint shows through a
/// note's own technique color. Subtle — this flags the exception (an outside
/// note), not the common case, so in-scale notes are left untouched.
pub(super) const OUT_OF_SCALE_MIX: f32 = 0.45;
pub(super) const OUT_OF_SCALE_TINT: Color = Color::srgb(0.95, 0.25, 0.20);

/// A tempo-change point's marker line/label in the grid header — distinct
/// from the waveform's own accent color and the beat/bar gridlines so it
/// reads as its own kind of thing.
pub(super) const TEMPO_MARKER_COLOR: Color = Color::srgb(0.95, 0.55, 0.15);

/// Whether `note`'s target pitch — its bent/overblown/overdrawn pitch, not
/// just its natural one, since e.g. bending draw-3 down a step-and-a-half on a
/// C harp is exactly how a blues player reaches the ♭7 — falls in `scale`.
/// `None` (holes/directions the harp can't produce) counts as in-scale, so a
/// note that can't be resolved to a pitch isn't flagged as "wrong" too.
pub(super) fn note_in_scale(note: &GridNote, harp: &Harmonica, scale: &HashSet<String>) -> bool {
    let Some(freq) = note_freq(note, harp) else {
        return true;
    };
    let Some(midi) = freq_to_midi(freq) else {
        return true;
    };
    let name = midi_to_note(midi);
    let class = name.trim_end_matches(|c: char| c.is_ascii_digit());
    scale.contains(class)
}

pub(super) fn rebuild_grid(
    mut commands: Commands,
    state: Res<EditorState>,
    waveform: Res<super::waveform::MusicWaveform>,
    content: Query<Entity, With<GridContent>>,
    old: Query<Entity, With<GridItem>>,
    windows: Query<&Window>,
    mut note_mats: ResMut<Assets<EditorNoteMaterial>>,
    theme: Res<LoadedTheme>,
) {
    // A note drag owns picking-captured note entities a rebuild would
    // despawn — but *only* a note drag: the timeline Select drag's surface
    // is persistent (`ui::setup`), precisely so a mid-selection wheel pan
    // can rebuild the grid and spawn the notes it scrolls into view.
    if state.dragging.is_some() {
        return;
    }
    let colors = theme.song_editor_colors();
    let bar_colors = theme.twelve_bar_colors();
    let scale = state.scale.classes(&state.key);
    let harp = build_harp(&state.key, state.harmonica_kind);
    let hole_count = state.hole_count();
    // Locked (user Lock toggle, or Perform mode): grid cells, notes, and
    // resize handles are all spawned non-interactive via `Pickable::IGNORE`,
    // so no click/drag observer below ever fires — a single gate at spawn
    // time rather than a check duplicated inside every observer.
    let locked = state.locked();
    let pickable = |locked: bool| {
        if locked {
            Pickable::IGNORE
        } else {
            Pickable::default()
        }
    };
    for e in &old {
        commands.entity(e).despawn();
    }
    let Ok(content) = content.single() else {
        return;
    };
    let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);
    let cols = visible_beats(win_w);
    let mut items: Vec<Entity> = Vec::new();

    for col in 0..=cols {
        let beat = state.scroll_beat + col;
        let x = beat as f32 * BEAT_W;
        let is_bar = beat.is_multiple_of(BEATS_PER_BAR);
        // Tiles the standard 12-bar-blues form indefinitely as the user
        // scrolls, so the grid reads as harmonic function (I/IV/V) even for
        // charts longer than 12 bars.
        let bar_index = (beat / BEATS_PER_BAR) % 12;
        let bar_tint = bar_bg(
            bar_index,
            &state.key,
            crate::song::harmonica::Progression::Standard,
            bar_colors,
        );

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
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
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
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.45, 0.45, 0.55)),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        for hole in 1..=hole_count {
            let y = HEADER_H + (hole as f32 - 1.0) * ROW_H;
            let lane = if hole % 2 == 0 {
                colors.lane_a
            } else {
                colors.lane_b
            };
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
                pickable(locked),
            ));
            cell.observe(
                move |ev: On<Pointer<Click>>,
                      rel: Query<&RelativeCursorPosition>,
                      mut state: ResMut<EditorState>,
                      keyboard: Res<ButtonInput<KeyCode>>| {
                    let frac = rel
                        .get(ev.entity)
                        .ok()
                        .and_then(|r| r.normalized)
                        .map_or(0.0, |n| n.x)
                        .clamp(0.0, 0.999);
                    let sub = (frac * TICKS_PER_BEAT as f32).floor() as usize;
                    let tick = beat * TICKS_PER_BEAT + sub;
                    if ctrl_held(&keyboard) {
                        select_or_add_ctrl(&mut state, hole, tick);
                    } else {
                        select_or_add(&mut state, hole, tick);
                    }
                },
            );
            items.push(cell.id());
        }

        // The silence track's background strip, below the hole lanes — pure
        // display, `Pickable::IGNORE` throughout (see `silence_gaps`'s
        // callers below for the actual gap blocks drawn on top of it).
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(silence_row_top(hole_count)),
                        width: Val::Px(BEAT_W),
                        height: Val::Px(SILENCE_ROW_H),
                        ..default()
                    },
                    BackgroundColor(colors.panel_bg),
                    Pickable::IGNORE,
                ))
                .id(),
        );

        // Divider lines are spawned after the lane cells (not before) so they
        // render on top of them — otherwise the opaque lane backgrounds would
        // cover the lines everywhere except the header strip above the lanes,
        // making them look like they stop at the header instead of running
        // down through every hole's row. `Pickable::IGNORE` keeps them from
        // blocking clicks on the lane buttons underneath.
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(0.0),
                        width: Val::Px(if is_bar { 2.0 } else { 1.0 }),
                        height: Val::Px(grid_height(hole_count)),
                        ..default()
                    },
                    BackgroundColor(if is_bar {
                        colors.bar_line
                    } else {
                        colors.grid_line
                    }),
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
                            height: Val::Px(grid_height(hole_count) - HEADER_H),
                            ..default()
                        },
                        BackgroundColor(if is_half {
                            colors.half_line
                        } else {
                            colors.quarter_line
                        }),
                        Pickable::IGNORE,
                    ))
                    .id(),
            );
        }
    }

    let first_tick = state.scroll_beat * TICKS_PER_BEAT;
    let last_tick = (state.scroll_beat + cols + 1) * TICKS_PER_BEAT;
    for note in &state.notes {
        if note.tick < last_tick && note.tick + note.len > first_tick {
            let selected = state.is_selected(note.id);
            let in_scale = note_in_scale(note, &harp, &scale);
            items.push(spawn_note(
                &mut commands,
                *note,
                selected,
                &mut note_mats,
                colors,
                locked,
                in_scale,
            ));
        }
    }

    let tempo_map = state.tempo_map();

    let bucket_count = waveform.buckets.len();
    let visible = super::waveform::visible_waveform_buckets(
        state.scroll_beat,
        cols,
        bucket_count,
        waveform.duration_secs,
        &tempo_map,
    );
    for i in visible {
        let (x, w) = super::waveform::waveform_bar_geometry(
            i,
            bucket_count,
            waveform.duration_secs,
            &tempo_map,
        );
        let amplitude = waveform.buckets[i].clamp(0.0, 1.0);
        let h = (amplitude * WAVEFORM_H).max(1.0);
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(WAVEFORM_TOP + (WAVEFORM_H - h)),
                        width: Val::Px(w.max(1.0) - 1.0),
                        height: Val::Px(h),
                        ..default()
                    },
                    BackgroundColor(colors.accent.with_alpha(0.35)),
                    Pickable::IGNORE,
                ))
                .id(),
        );
    }

    // Tempo-change points (placed via the Tempo timeline tool — see
    // `state::toggle_tempo_point`), windowed to the currently-visible ticks
    // like everything else in this function.
    for &(tick, bpm) in &state.tempo_changes {
        if tick >= last_tick || tick < first_tick {
            continue;
        }
        let x = tick as f32 * TICK_W;
        items.push(
            commands
                .spawn((
                    GridItem,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(0.0),
                        width: Val::Px(2.0),
                        height: Val::Px(HEADER_H),
                        ..default()
                    },
                    BackgroundColor(TEMPO_MARKER_COLOR),
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
                        left: Val::Px(x + 4.0),
                        top: Val::Px(WAVEFORM_TOP - 14.0),
                        ..default()
                    },
                    Text::new(format!("\u{2669}={}", bpm.round() as i32)),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(TEMPO_MARKER_COLOR),
                    Pickable::IGNORE,
                ))
                .id(),
        );
    }

    for (start, end) in silence_gaps(&state.notes) {
        if start < last_tick && end > first_tick {
            let duration_secs =
                crate::song::chart::tick_to_seconds(end as u64, TICKS_PER_BEAT as u32, &tempo_map)
                    - crate::song::chart::tick_to_seconds(
                        start as u64,
                        TICKS_PER_BEAT as u32,
                        &tempo_map,
                    );
            items.push(spawn_silence_gap(
                &mut commands,
                start,
                end,
                duration_secs as f32,
                hole_count,
                colors,
            ));
        }
    }

    commands.entity(content).add_children(&items);
}

/// One block of the silence track, spanning `[start, end)` ticks — labeled
/// with its duration so the gap's length reads at a glance without having to
/// count grid squares.
fn spawn_silence_gap(
    commands: &mut Commands,
    start: usize,
    end: usize,
    duration_secs: f32,
    hole_count: u8,
    colors: SongEditorColors,
) -> Entity {
    let left = start as f32 * TICK_W + 1.0;
    let width = (end - start) as f32 * TICK_W - 2.0;
    commands
        .spawn((
            GridItem,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(silence_row_top(hole_count) + 2.0),
                width: Val::Px(width.max(0.0)),
                height: Val::Px(SILENCE_ROW_H - 4.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(colors.label.with_alpha(0.20)),
            Pickable::IGNORE,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(format!("{duration_secs:.1}s")),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(colors.label),
                Pickable::IGNORE,
            ));
        })
        .id()
}

/// Shifts every *other* selected note (`DragState::group`) by the exact
/// hole/tick delta the anchor moved by (`anchor_target - anchor_start`),
/// so a multi-note drag moves the whole group as one rigid shape — used
/// alongside `state::move_target`, which already computes the anchor's own
/// clamped target. Each member is independently clamped to the harp's hole
/// range and non-negative ticks, the same rule `move_target` applies to
/// the anchor; at the extreme edges of the grid this can compress the
/// group's shape slightly (some members clamp, others don't) rather than
/// blocking the whole move outright — an accepted, rare edge case, same
/// spirit as a single note's own drag clamping instead of refusing to move.
pub(super) fn group_move_targets(
    others: &[GridNote],
    hole_delta: i32,
    tick_delta: i32,
    hole_count: u8,
) -> Vec<(u32, u8, usize, usize, Pitch)> {
    others
        .iter()
        .map(|n| {
            let hole = (n.hole as i32 + hole_delta).clamp(1, hole_count as i32) as u8;
            let tick = (n.tick as i32 + tick_delta).max(0) as usize;
            (n.id, hole, tick, n.len, n.pitch)
        })
        .collect()
}

/// Whether every note in a multi-note move — the anchor plus every other
/// member of its group, each as `(id, hole, tick, len, pitch)` — can
/// legally land at its computed target: its pitch technique still fits the
/// hole it would land on (e.g. a note bent 1.5 semitones can't land on a
/// hole whose own max bend is smaller), and it doesn't overlap any note
/// that ISN'T part of the group (group members overlapping *each other* is
/// fine — they keep their original relative positions, so if they didn't
/// collide before the drag they won't after it either).
pub(super) fn group_move_valid(
    notes: &[GridNote],
    moving_ids: &[u32],
    targets: &[(u32, u8, usize, usize, Pitch)],
) -> bool {
    targets.iter().all(|&(_, hole, tick, len, pitch)| {
        pitch_compatible(pitch, hole)
            && !notes.iter().any(|n| {
                !moving_ids.contains(&n.id)
                    && n.hole == hole
                    && n.tick < tick + len
                    && tick < n.tick + n.len
            })
    })
}

pub(super) fn spawn_note(
    commands: &mut Commands,
    note: GridNote,
    selected: bool,
    note_mats: &mut Assets<EditorNoteMaterial>,
    colors: SongEditorColors,
    locked: bool,
    in_scale: bool,
) -> Entity {
    let (left, top, width, height) = note_rect(&note);
    let border = if selected { 2.0 } else { 0.0 };
    let border_color = if selected { colors.accent } else { Color::NONE };
    let id = note.id;
    let pick = if locked {
        Pickable::IGNORE
    } else {
        Pickable::default()
    };
    // Flag the exception (a note outside the song's blues scale), not the
    // common case: an in-scale note keeps its plain technique color; an
    // outside note gets a warm red warning blended in. Bend/overblow/overdraw
    // are accounted for by `note_in_scale` using the note's *target* pitch —
    // e.g. bending draw-3 down a step-and-a-half is how a blues player reaches
    // the ♭7, so that bent note reads as in-scale even though its natural
    // (unbent) pitch wouldn't.
    let note_color = |base: Color| {
        if in_scale {
            base
        } else {
            mix_srgba(base, OUT_OF_SCALE_TINT, OUT_OF_SCALE_MIX)
        }
    };

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
            pick,
        ))
        .observe(
            move |_: On<Pointer<Click>>,
                  mut state: ResMut<EditorState>,
                  keyboard: Res<ButtonInput<KeyCode>>| {
                if ctrl_held(&keyboard) {
                    state.toggle_selected(id);
                } else {
                    state.select_only(id);
                }
            },
        )
        .observe(
            move |_: On<Pointer<DragStart>>, mut state: ResMut<EditorState>| {
                if state.dragging.is_some() {
                    return;
                }
                let Some(anchor) = state.note_by_id(id).copied() else {
                    return;
                };
                // Dragging a note that's part of the current multi-selection
                // (more than one note selected, this one among them) moves
                // the whole group together; otherwise a drag behaves like
                // before — it exclusively selects just the note being
                // dragged.
                let group: Vec<GridNote> = if state.selected.len() > 1 && state.is_selected(id) {
                    let ids = state.selected.clone();
                    ids.iter()
                        .filter(|&&gid| gid != id)
                        .filter_map(|&gid| state.note_by_id(gid).copied())
                        .collect()
                } else {
                    state.select_only(id);
                    Vec::new()
                };
                state.dragging = Some(DragState::new_group(id, &anchor, group));
            },
        )
        .observe(
            move |ev: On<Pointer<Drag>>,
                  mut state: ResMut<EditorState>,
                  loc: Res<Localization>,
                  ui_scale: Res<UiScale>| {
                let Some(drag) = state.dragging.clone() else {
                    return;
                };
                if drag.id != id || drag.kind != DragKind::Move {
                    return;
                }
                let hole_count = state.hole_count();
                // `Pointer<Drag>::distance` is raw window-pixel motion, but
                // `TICK_W`/`ROW_H` are logical sizes that `UiScale` (the
                // arrow-key UI zoom, `dialogs::ui_scale`) multiplies up for
                // display — without dividing it back out here, dragging a
                // note moves it by the wrong number of ticks/holes at any
                // zoom level other than 1x. Same fix as
                // `gameplay_3d::note_label_position`.
                let (hole, tick) = move_target(
                    drag.start_hole,
                    drag.start_tick,
                    ev.distance.x / ui_scale.0,
                    ev.distance.y / ui_scale.0,
                    hole_count,
                );
                let pitch = state
                    .notes
                    .iter()
                    .find(|n| n.id == id)
                    .map(|n| n.pitch)
                    .unwrap_or(Pitch::Normal);
                let pitch_ok = pitch_compatible(pitch, hole);
                // The anchor and the rest of the group (if any) are checked
                // together, in one `group_move_valid` call, rather than the
                // anchor via `can_place` and the group separately: since
                // every member shifts by the same delta, a `can_place`-style
                // check comparing the anchor's *new* spot against the other
                // members' *stale, not-yet-moved* positions would wrongly
                // flag a collision whenever the group's own shape has two
                // members swap-adjacent (e.g. dragging two same-hole notes
                // right by exactly one note's length) — they're moving out
                // of each other's way together, not colliding.
                let hole_delta = hole as i32 - drag.start_hole as i32;
                let tick_delta = tick as i32 - drag.start_tick as i32;
                let mut targets = vec![(id, hole, tick, drag.start_len, pitch)];
                targets.extend(group_move_targets(
                    &drag.group,
                    hole_delta,
                    tick_delta,
                    hole_count,
                ));
                let mut moving_ids: Vec<u32> = vec![id];
                moving_ids.extend(drag.group.iter().map(|n| n.id));
                let valid = group_move_valid(&state.notes, &moving_ids, &targets);
                state.drag_msg = if !pitch_ok {
                    loc.msg(pitch_deny_key(pitch, hole))
                } else if !valid {
                    loc.msg("drag-denied-overlap")
                } else {
                    crate::localization::LocalizedStr::default()
                };
                if let Some(d) = state.dragging.as_mut() {
                    d.target_hole = hole;
                    d.target_tick = tick;
                    d.valid = valid;
                }
            },
        )
        .observe(
            move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
                let Some(drag) = state.dragging.take() else {
                    return;
                };
                state.drag_msg = crate::localization::LocalizedStr::default();
                if drag.kind == DragKind::Move && drag.valid {
                    let hole_count = state.hole_count();
                    let hole_delta = drag.target_hole as i32 - drag.start_hole as i32;
                    let tick_delta = drag.target_tick as i32 - drag.start_tick as i32;
                    let group_targets =
                        group_move_targets(&drag.group, hole_delta, tick_delta, hole_count);
                    if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                        n.hole = drag.target_hole;
                        n.tick = drag.target_tick;
                    }
                    for &(gid, gh, gt, _, _) in &group_targets {
                        if let Some(n) = state.notes.iter_mut().find(|n| n.id == gid) {
                            n.hole = gh;
                            n.tick = gt;
                        }
                    }
                    enforce_direction(&mut state, id);
                    enforce_expr(&mut state, id);
                    for &(gid, _, _, _, _) in &group_targets {
                        enforce_direction(&mut state, gid);
                        enforce_expr(&mut state, gid);
                    }
                }
            },
        )
        .id();

    match note.expr {
        Expr::None => {
            commands
                .entity(root)
                .insert(BackgroundColor(note_color(pitch_color(note.pitch))));
        }
        Expr::Wah(_) | Expr::Vibrato(_) => {
            let mode = if matches!(note.expr, Expr::Vibrato(_)) {
                0.0
            } else {
                1.0
            };
            let mat = note_mats.add(EditorNoteMaterial {
                color: note_color(pitch_color(note.pitch)).to_linear(),
                params: Vec4::new(mode, width, 0.0, 0.0),
            });
            commands.entity(root).insert(MaterialNode(mat));
        }
    }

    commands.entity(root).with_children(|r| {
        r.spawn((
            Text::new(note.dir.arrow()),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ));
        spawn_resize_handle(r, id, Edge::Left, locked);
        spawn_resize_handle(r, id, Edge::Right, locked);
    });

    root
}

fn spawn_resize_handle(parent: &mut ChildSpawnerCommands, id: u32, edge: Edge, locked: bool) {
    use super::state::apply_resize;
    let mut node = Node {
        position_type: PositionType::Absolute,
        top: Val::Px(0.0),
        bottom: Val::Px(0.0),
        width: Val::Px(HANDLE_W),
        ..default()
    };
    match edge {
        Edge::Left => node.left = Val::Px(0.0),
        Edge::Right => node.right = Val::Px(0.0),
    }
    let pick = if locked {
        Pickable::IGNORE
    } else {
        Pickable::default()
    };
    parent
        .spawn((node, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.25)), pick))
        .observe(move |_: On<Pointer<DragStart>>, mut state: ResMut<EditorState>| {
            if let Some(n) = state.note_by_id(id).copied() {
                state.select_only(id);
                state.dragging = Some(DragState::new(id, DragKind::Resize(edge), &n));
            }
        })
        .observe(move |ev: On<Pointer<Drag>>, mut state: ResMut<EditorState>, ui_scale: Res<UiScale>| {
            let Some(drag) = state.dragging.clone() else { return };
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
            // Same `UiScale` correction as the move-drag observer above —
            // `ev.distance` is raw window pixels, `TICK_W` is a logical
            // size `UiScale` multiplies up for display.
            let steps = ((ev.distance.x / ui_scale.0) / TICK_W).round() as i32;
            let (tick, len) =
                apply_resize(drag.start_tick, drag.start_len, edge, steps, left_bound, right_bound);
            if let Some(n) = state.notes.iter_mut().find(|n| n.id == id) {
                n.tick = tick;
                n.len = len;
            }
        })
        .observe(move |_: On<Pointer<DragEnd>>, mut state: ResMut<EditorState>| {
            if matches!(&state.dragging, Some(d) if d.id == id && matches!(d.kind, DragKind::Resize(_))) {
                state.dragging = None;
                enforce_direction(&mut state, id);
                enforce_expr(&mut state, id);
            }
        });
}
