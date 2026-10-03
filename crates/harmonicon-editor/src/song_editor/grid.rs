// SPDX-License-Identifier: MIT

use bevy::picking::Pickable;
use bevy::picking::events::{PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::MaterialNode;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

use super::interaction::{ctrl_held, select_or_add, select_or_add_ctrl};
use super::material::EditorNoteMaterial;
use super::playback::note_freq;
use super::ranges::silence_gaps;
use super::snap::{
    GridlineKind, off_beat_labels, snap_absolute_tick, snap_tick_in_beat, sub_beat_gridlines,
};
use super::state::{
    DragKind, DragState, EditorState, Expr, GridNote, Mode, Pitch, enforce_direction, enforce_expr,
    move_target, note_rect, pitch_color, pitch_compatible, pitch_deny_key,
};
use super::ui::{GridContent, GridItem, NoteView};
use super::{
    BEAT_W, HEADER_H, ROW_H, SILENCE_ROW_H, TICK_W, TICKS_PER_BEAT, WAVEFORM_H, WAVEFORM_TOP,
    grid_height, silence_row_top,
};
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::midi::{freq_to_midi, midi_to_note};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::{LoadedTheme, SongEditorColors};
use harmonicon_ui::dialogs::twelve_bar_grid::bar_bg;
use std::collections::HashSet;

pub(super) fn visible_beats(win_w: f32) -> usize {
    (((win_w - super::HOLE_COL_W) / BEAT_W).ceil() as usize) + 1
}

/// How strongly a bar's 12-bar-blues chord-function tint (see [`bar_bg`])
/// shows through the lane's own alternating-row color. Low enough to keep
/// the checkerboard readable and not compete with note blocks. Only applies
/// when the tint is switched on — see [`EditorState::twelve_bar_tint`].
const BAR_TINT_MIX: f32 = 0.35;

/// The beat ruler's three text sizes: a bar number, a beat number within a
/// bar, and a counting syllable between two beats. The bar number is the
/// largest of the three because it and a beat number can read as the same
/// digit (beat 3 of bar 1 vs. beat 1 of bar 3) — size and `colors.accent`
/// are together what tell them apart.
const BAR_LABEL_FONT: f32 = 13.0;
const BEAT_LABEL_FONT: f32 = 11.5;
const OFF_BEAT_LABEL_FONT: f32 = 11.0;

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

/// A meter change's bar line — the full grid height, like a bar line,
/// but in its own colour so a change reads as structural rather than as
/// just another bar. Cooler than the tempo marker's orange so the two
/// kinds of "something changes here" don't read as one.
pub(super) const METER_MARKER_COLOR: Color = Color::srgb(0.45, 0.75, 0.95);

/// Whether `note`'s target pitch — bent/overblown/overdrawn, not just its
/// natural one (e.g. bending draw-3 down a step-and-a-half on a C harp is
/// how a blues player reaches the ♭7) — falls in `scale`. `None` (a
/// hole/direction the harp can't produce) counts as in-scale, so it isn't
/// flagged as "wrong" too.
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
    mut cache: ResMut<super::grid_cache::GridCache>,
    state: Res<EditorState>,
    waveform: Res<super::waveform::MusicWaveform>,
    content: Query<Entity, With<GridContent>>,
    old: Query<Entity, With<GridItem>>,
    windows: Query<&Window>,
    mut note_mats: ResMut<Assets<EditorNoteMaterial>>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    // A note drag owns picking-captured note entities a rebuild would
    // despawn — but *only* a note drag: the timeline Select drag's surface
    // is persistent (`ui::setup`), precisely so a mid-selection wheel pan
    // can rebuild the grid and spawn the notes it scrolls into view.
    if state.dragging.is_some() {
        return;
    }
    let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(1280.0);
    let cols = visible_beats(win_w);
    if !cache.bypass_change_detection().update(
        &state,
        cols,
        theme.is_changed() || waveform.is_changed(),
    ) {
        return;
    }
    cache.set_changed();
    let colors = theme.song_editor_colors();
    let bar_colors = theme.twelve_bar_colors();
    let scale = state.scale.classes(&state.key);
    let harp = state.effective_harp();
    let hole_count = state.hole_count();
    // Locked (user Lock toggle, or Perform mode): grid cells, notes, and
    // resize handles are all spawned non-interactive via `Pickable::IGNORE`,
    // so no click/drag observer below ever fires — a single gate at spawn
    // time rather than a check duplicated inside every observer.
    let locked = state.locked();
    // `Mode::ExpectedNotes` is "locked" too (`EditorState::locked`, unchanged)
    // — the *ordinary* notes layer shouldn't be editable there — but the
    // background cell itself must stay clickable so its own click observer
    // below can still reach `expected_notes::place_or_select_expected`.
    // Only this one spot needs the split: note visuals (and everything
    // else keyed off `locked`) stay exactly as locked as before.
    let cell_locked = state.user_locked || matches!(state.mode, Mode::Record | Mode::Play);
    let pickable = |locked: bool| {
        if locked { Pickable::IGNORE } else { Pickable::default() }
    };
    for e in &old {
        commands.entity(e).despawn();
    }
    let Ok(content) = content.single() else {
        return;
    };
    // Bar geometry is measured in ticks, not in whole quarter-note columns:
    // a 7/8 bar is 42 ticks — three and a half columns — so its bar line
    // genuinely falls *between* two of them. And it comes from the meter
    // *map*, not one meter: a mid-song change re-bars everything after it.
    let meter_map = state.meter_map();
    let first_tick = state.scroll_beat * TICKS_PER_BEAT;
    let last_tick = (state.scroll_beat + cols + 1) * TICKS_PER_BEAT;
    let mut items: Vec<Entity> = Vec::new();

    for col in 0..=cols {
        let beat = state.scroll_beat + col;
        let x = beat as f32 * BEAT_W;
        // Opt-in (`EditorState::twelve_bar_tint`): tiles the standard
        // 12-bar-blues form indefinitely as the user scrolls, so the grid
        // reads as harmonic function (I/IV/V) even for charts longer than
        // 12 bars. Off by default — a chart that isn't a 12-bar blues has
        // no such progression for the background to be describing. Keyed on
        // the bar containing the cell's *start*, since in a meter whose bar
        // isn't a whole number of columns one cell can straddle two bars.
        // A pickup belongs to no bar of the form, so it goes untinted.
        let bar_tint = state
            .twelve_bar_tint
            .then(|| {
                let bar = meter_map.position((beat * TICKS_PER_BEAT) as u64).bar;
                meter_map.bar_number(bar).map(|number| {
                    bar_bg(
                        (number - 1) % 12,
                        &state.key,
                        harmonicon_core::harmonica::Progression::Standard,
                        bar_colors,
                    )
                })
            })
            .flatten();

        for hole in 1..=hole_count {
            let y = HEADER_H + (hole as f32 - 1.0) * ROW_H;
            let lane = if hole % 2 == 0 { colors.lane_a } else { colors.lane_b };
            let lane = match bar_tint {
                Some(tint) => mix_srgba(lane, tint, BAR_TINT_MIX),
                None => lane,
            };
            let mut cell = commands.spawn((
                GridItem,
                WidgetButton,
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
                pickable(cell_locked),
            ));
            cell.observe(
                move |ev: On<Activate>,
                      rel: Query<&RelativeCursorPosition>,
                      mut state: ResMut<EditorState>,
                      keyboard: Res<ButtonInput<KeyCode>>| {
                    let frac = rel
                        .get(ev.entity)
                        .ok()
                        .and_then(|r| r.normalized)
                        .map_or(0.0, |n| n.x)
                        .clamp(0.0, 0.999);
                    let sub = snap_tick_in_beat(frac, state.snap_mode);
                    let tick = beat * TICKS_PER_BEAT + sub;
                    // Dev-only ("--features dev") benchmark-authoring mode —
                    // see `expected_notes`'s module docs. `cell_locked`
                    // (above) is what keeps this observer reachable at all
                    // while in that mode, despite the grid otherwise being
                    // `locked()` there like Record/Play.
                    #[cfg(feature = "dev")]
                    if state.mode == Mode::ExpectedNotes {
                        super::expected_notes::place_or_select_expected(&mut state, hole, tick);
                        return;
                    }
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
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(x)},
                        top: {Val::Px(silence_row_top(hole_count))},
                        width: {Val::Px(BEAT_W)},
                        height: {Val::Px(SILENCE_ROW_H)},
                    }
                    BackgroundColor({colors.panel_bg})
                    ~{Pickable::IGNORE}
                })
                .id(),
        );

        // Divider lines are spawned after the lane cells (not before) so they
        // render on top of them — otherwise the opaque lane backgrounds would
        // cover the lines everywhere except the header strip above the lanes,
        // making them look like they stop at the header instead of running
        // down through every hole's row. `Pickable::IGNORE` keeps them from
        // blocking clicks on the lane buttons underneath.
        //
        // Always the plain beat line: this one separates two lane *cells*,
        // so it belongs at a column boundary whatever the meter. Bar lines
        // are drawn afterwards, at their own tick positions, and land on
        // top of this where the two coincide (as they always do in 4/4).
        items.push(
            commands
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(x)},
                        top: {Val::Px(0.0)},
                        width: {Val::Px(1.0)},
                        height: {Val::Px(grid_height(hole_count))},
                    }
                    BackgroundColor({colors.grid_line})
                    ~{Pickable::IGNORE}
                })
                .id(),
        );

        // Only the positions the *active* snap mode can land a note on
        // (`sub_beat_gridlines`, which also picks each line's tier). Drawing
        // the straight-16th and triplet families together instead divides a
        // beat at ticks 3, 4, 6, 8 and 9 — two near-coincident pairs one
        // tick (5px) apart — which reads as neither a 2- nor a 3-way split,
        // and marks positions the active mode can't reach anyway.
        for (tick, kind) in sub_beat_gridlines(state.snap_mode) {
            items.push(
                commands
                    .spawn_empty()
                    .apply_scene(bsn! {
                        GridItem
                        Node {
                            position_type: {PositionType::Absolute},
                            left: {Val::Px(x + tick as f32 * TICK_W)},
                            top: {Val::Px(HEADER_H)},
                            width: {Val::Px(1.0)},
                            height: {Val::Px(grid_height(hole_count) - HEADER_H)},
                        }
                        BackgroundColor({match kind {
                            GridlineKind::Half => colors.half_line,
                            GridlineKind::Sixteenth => colors.quarter_line,
                            GridlineKind::Triplet => colors.triplet_line,
                        }})
                        ~{Pickable::IGNORE}
                    })
                    .id(),
            );
        }
    }

    // ── The beat ruler ───────────────────────────────────────────────────
    //
    // Driven by the meter map's own beat and bar starts rather than by the
    // column loop above, because a signature beat need not be a column
    // (an eighth in 6/8 is half of one), a bar boundary need not coincide
    // with a column at all (7/8), and after a meter change neither is where
    // the opening meter would put it. In 4/4 with no changes every position
    // below lands exactly on a column.
    let changes: std::collections::BTreeMap<u64, harmonicon_ui::music_score::MusicScoreMeter> =
        meter_map.changes().collect();
    let mut beat_ticks: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    for (tick, pos) in meter_map.beat_starts(first_tick as u64, last_tick as u64) {
        beat_ticks.insert(tick);
        let is_bar = pos.beat == 0;
        // A bar number on the downbeat, the beat's index otherwise — and,
        // where a meter change starts this bar, the new signature beside
        // the number, since a change always begins a bar. A pickup's beats
        // are its bar's last ones ("4" before bar 1 in 4/4), and its bar
        // has no downbeat on the grid to number.
        let bar_number = meter_map.bar_number(pos.bar).unwrap_or(0);
        let label = match (is_bar, changes.get(&tick)) {
            (true, Some(meter)) => {
                format!("{} \u{00B7} {}/{}", bar_number, meter.numerator, meter.denominator)
            }
            (true, None) => format!("{bar_number}"),
            (false, _) => format!("{}", pos.beat + 1),
        };
        items.push(
            commands
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(tick as f32 * TICK_W + 4.0)},
                        top: {Val::Px(6.0)},
                    }
                    Text({label})
                    TextFont { font_size: {FontSize::Px(if is_bar {
                        BAR_LABEL_FONT
                    } else {
                        BEAT_LABEL_FONT
                    })} }
                    TextColor({if is_bar { colors.accent } else { colors.label }})
                    ~{Pickable::IGNORE}
                })
                .id(),
        );
    }

    // Bar lines, at their true tick positions — heavier than the beat lines
    // the column loop drew, and drawn after them so they win where the two
    // coincide. A meter change's tick is a bar line too, and additionally
    // gets a `TEMPO_MARKER_COLOR`-style marker the full height of the grid,
    // since it restructures everything after it the way a tempo point
    // re-times everything after it.
    for (tick, _) in meter_map.bar_starts(first_tick as u64, last_tick as u64) {
        let is_change = changes.contains_key(&tick);
        items.push(
            commands
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(tick as f32 * TICK_W)},
                        top: {Val::Px(0.0)},
                        width: {Val::Px(2.0)},
                        height: {Val::Px(grid_height(hole_count))},
                    }
                    BackgroundColor({if is_change {
                        METER_MARKER_COLOR
                    } else {
                        colors.bar_line
                    }})
                    ~{Pickable::IGNORE}
                })
                .id(),
        );
    }

    super::repeat_marks::spawn(
        &mut commands,
        &mut items,
        &state.repeats,
        first_tick,
        last_tick,
        grid_height(hole_count),
    );

    // Counting syllables, on the ticks the *active* snap mode can land on —
    // a fixed "&" at half a beat would name an unreachable position in
    // Shuffle and Triplet. Skipped wherever one would collide with a
    // numbered signature beat, which is every one of them in a meter
    // counted in eighths: there the "&" position *is* a beat, and already
    // carries its own number.
    for col in 0..=cols {
        let beat = state.scroll_beat + col;
        for &(sub, key) in off_beat_labels(state.snap_mode) {
            let tick = beat * TICKS_PER_BEAT + sub;
            if beat_ticks.contains(&(tick as u64)) {
                continue;
            }
            items.push(
                commands
                    .spawn_empty()
                    .apply_scene(bsn! {
                        GridItem
                        Node {
                            position_type: {PositionType::Absolute},
                            left: {Val::Px(tick as f32 * TICK_W + 2.0)},
                            top: {Val::Px(6.0)},
                        }
                        Text({String::from(loc.msg(key))})
                        TextFont { font_size: {FontSize::Px(OFF_BEAT_LABEL_FONT)} }
                        TextColor({colors.label.with_alpha(0.55)})
                        ~{Pickable::IGNORE}
                    })
                    .id(),
            );
        }
    }

    super::annotation_lane::spawn(
        &mut commands,
        &mut items,
        &state,
        first_tick,
        last_tick,
        colors,
        &loc,
    );

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
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(x)},
                        top: {Val::Px(WAVEFORM_TOP + (WAVEFORM_H - h))},
                        width: {Val::Px(w.max(1.0) - 1.0)},
                        height: {Val::Px(h)},
                    }
                    BackgroundColor({colors.accent.with_alpha(0.35)})
                    ~{Pickable::IGNORE}
                })
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
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(x)},
                        top: {Val::Px(0.0)},
                        width: {Val::Px(2.0)},
                        height: {Val::Px(HEADER_H)},
                    }
                    BackgroundColor({TEMPO_MARKER_COLOR})
                    ~{Pickable::IGNORE}
                })
                .id(),
        );
        items.push(
            commands
                .spawn_empty()
                .apply_scene(bsn! {
                    GridItem
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(x + 4.0)},
                        top: {Val::Px(WAVEFORM_TOP - 14.0)},
                    }
                    Text({format!("\u{2669}={}", bpm.round() as i32)})
                    TextFont { font_size: {FontSize::Px(11.0)} }
                    TextColor({TEMPO_MARKER_COLOR})
                    ~{Pickable::IGNORE}
                })
                .id(),
        );
    }

    for (start, end) in silence_gaps(&state.notes) {
        if start < last_tick && end > first_tick {
            let duration_secs = harmonicon_core::chart::tick_to_seconds(
                end as u64,
                TICKS_PER_BEAT as u32,
                &tempo_map,
            ) - harmonicon_core::chart::tick_to_seconds(
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
        .spawn_empty()
        .apply_scene(bsn! {
            GridItem
            Node {
                position_type: {PositionType::Absolute},
                left: {Val::Px(left)},
                top: {Val::Px(silence_row_top(hole_count) + 2.0)},
                width: {Val::Px(width.max(0.0))},
                height: {Val::Px(SILENCE_ROW_H - 4.0)},
                align_items: {AlignItems::Center},
                justify_content: {JustifyContent::Center},
                overflow: {Overflow::clip()},
            }
            BackgroundColor({colors.label.with_alpha(0.20)})
            ~{Pickable::IGNORE}
            Children [
                Text({format!("{duration_secs:.1}s")})
                TextFont { font_size: {FontSize::Px(11.0)} }
                TextColor({colors.label})
                ~{Pickable::IGNORE}
            ]
        })
        .id()
}

/// Shifts every *other* selected note (`DragState::group`) by the exact
/// hole/tick delta the anchor moved by, so a multi-note drag moves the
/// whole group as one rigid shape — used alongside `state::move_target`,
/// which computes the anchor's own clamped target. Each member is
/// independently clamped the same way; at the grid's edges this can
/// compress the group's shape slightly rather than blocking the move
/// outright, an accepted rare edge case.
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

/// Whether every note in a multi-note move can legally land at its computed
/// target: its pitch technique still fits the hole (e.g. a note bent 1.5
/// semitones can't land on a hole with a smaller max bend), and it doesn't
/// overlap any note outside the group (members overlapping *each other* is
/// fine — they keep their original relative positions).
pub(super) fn group_move_valid(
    notes: &[GridNote],
    harp: &Harmonica,
    moving_ids: &[u32],
    targets: &[(u32, u8, usize, usize, Pitch)],
) -> bool {
    targets.iter().all(|&(_, hole, tick, len, pitch)| {
        pitch_compatible(pitch, harp, hole)
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
    let pick = if locked { Pickable::IGNORE } else { Pickable::default() };
    // Flag the exception (a note outside the song's blues scale), not the
    // common case: an in-scale note keeps its plain technique color; an
    // outside note gets a warm red warning blended in. Bend/overblow/overdraw
    // are accounted for by `note_in_scale` using the note's *target* pitch —
    // e.g. bending draw-3 down a step-and-a-half is how a blues player reaches
    // the ♭7, so that bent note reads as in-scale even though its natural
    // (unbent) pitch wouldn't.
    let note_color = |base: Color| {
        if in_scale { base } else { mix_srgba(base, OUT_OF_SCALE_TINT, OUT_OF_SCALE_MIX) }
    };

    let root = commands
        .spawn((
            GridItem,
            NoteView(id),
            WidgetButton,
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
            move |_: On<Activate>,
                  mut state: ResMut<EditorState>,
                  keyboard: Res<ButtonInput<KeyCode>>| {
                if ctrl_held(&keyboard) {
                    state.toggle_selected(id);
                } else {
                    state.select_only(id);
                }
            },
        )
        .observe(move |_: On<PointerDragStart>, mut state: ResMut<EditorState>| {
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
                state
                    .selected
                    .iter()
                    .filter(|&&gid| gid != id)
                    .filter_map(|&gid| state.note_by_id(gid).copied())
                    .collect()
            } else {
                state.select_only(id);
                Vec::new()
            };
            state.dragging = Some(DragState::new_group(id, &anchor, group));
        })
        .observe(
            move |ev: On<PointerDrag>,
                  mut state: ResMut<EditorState>,
                  loc: Res<Localization>,
                  ui_scale: Res<UiScale>| {
                let Some(drag) = state.dragging.as_ref() else {
                    return;
                };
                if drag.id != id || drag.kind != DragKind::Move {
                    return;
                }
                let hole_count = state.hole_count();
                // `PointerDrag::distance` is raw window-pixel motion, but
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
                // Snapped the same way a fresh note's placement already is
                // (`snap_tick_in_beat`) — group members below shift by the
                // delta *this* snapped tick produces, so the whole group
                // moves onto the grid together, not just the anchor.
                let tick = snap_absolute_tick(tick, state.snap_mode);
                let pitch = state
                    .notes
                    .iter()
                    .find(|n| n.id == id)
                    .map(|n| n.pitch)
                    .unwrap_or(Pitch::Normal);
                let harp = state.effective_harp();
                let pitch_ok = pitch_compatible(pitch, &harp, hole);
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
                targets.extend(group_move_targets(&drag.group, hole_delta, tick_delta, hole_count));
                let mut moving_ids: Vec<u32> = vec![id];
                moving_ids.extend(drag.group.iter().map(|n| n.id));
                let valid = group_move_valid(&state.notes, &harp, &moving_ids, &targets);
                state.drag_msg = if !pitch_ok {
                    loc.msg(pitch_deny_key(pitch, hole))
                } else if !valid {
                    loc.msg("drag-denied-overlap")
                } else {
                    harmonicon_platform::localization::LocalizedStr::default()
                };
                if let Some(d) = state.dragging.as_mut() {
                    d.target_hole = hole;
                    d.target_tick = tick;
                    d.valid = valid;
                }
            },
        )
        .observe(move |_: On<PointerDragEnd>, mut state: ResMut<EditorState>| {
            let Some(drag) = state.dragging.take() else {
                return;
            };
            state.drag_msg = harmonicon_platform::localization::LocalizedStr::default();
            if drag.kind == DragKind::Move && drag.valid {
                let hole_count = state.hole_count();
                let hole_delta = drag.target_hole as i32 - drag.start_hole as i32;
                let tick_delta = drag.target_tick as i32 - drag.start_tick as i32;
                let group_targets =
                    group_move_targets(&drag.group, hole_delta, tick_delta, hole_count);
                // One call for anchor and group together, so an onset
                // the whole group vacates is seen as vacated — moving
                // them one at a time would leave its annotation behind.
                let mut moves = vec![(id, drag.target_hole, drag.target_tick)];
                moves.extend(group_targets.iter().map(|&(gid, gh, gt, _, _)| (gid, gh, gt)));
                state.move_notes(&moves);
                enforce_direction(&mut state, id);
                enforce_expr(&mut state, id);
                for &(gid, _, _, _, _) in &group_targets {
                    enforce_direction(&mut state, gid);
                    enforce_expr(&mut state, gid);
                }
            }
        })
        .id();

    match note.expr {
        Expr::None => {
            commands.entity(root).insert(BackgroundColor(note_color(pitch_color(note.pitch))));
        }
        Expr::Wah(_) | Expr::Vibrato(_) => {
            let mode = if matches!(note.expr, Expr::Vibrato(_)) { 0.0 } else { 1.0 };
            let mat = note_mats.add(EditorNoteMaterial {
                color: note_color(pitch_color(note.pitch)).to_linear(),
                params: Vec4::new(mode, width, 0.0, 0.0),
            });
            commands.entity(root).insert(MaterialNode(mat));
        }
    }

    commands.entity(root).with_children(|r| {
        r.spawn_empty().apply_scene(bsn! {
            Text({note.dir.arrow()})
            TextFont { font_size: {FontSize::Px(15.0)} }
            TextColor({Color::WHITE})
            ~{Pickable::IGNORE}
        });
    });

    root
}

/// Selection only changes borders; keep note entities and observers alive.
pub(super) fn update_selection(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut notes: Query<(&NoteView, &mut Node, &mut BorderColor)>,
) {
    for (view, mut node, mut border) in &mut notes {
        let selected = state.is_selected(view.0);
        let width = UiRect::all(Val::Px(if selected { 2.0 } else { 0.0 }));
        if node.border != width {
            node.border = width;
        }
        let color = BorderColor::all(if selected {
            theme.song_editor_colors().accent
        } else {
            Color::NONE
        });
        if *border != color {
            *border = color;
        }
    }
}
