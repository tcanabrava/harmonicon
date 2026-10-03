// SPDX-License-Identifier: MIT

//! Dev-only ("--features dev") benchmark-authoring workflow — never wired
//! up outside it, see `mod.rs`'s conditional `mod expected_notes;`.
//!
//! `song_editor::debug_record` records raw mic audio plus whatever the live
//! detector actually produced (`EditorState::notes` — mistakes, phantom
//! notes and all; that's expected, not a problem to avoid). This module is
//! how you correct the record *afterward*: a "Draw correct notes" mode
//! button enters [`Mode::ExpectedNotes`], where clicking the grid
//! places/selects notes on a second, independent vector
//! ([`EditorState::expected_notes`]) instead of the ordinary one — purely
//! hand-placed ground truth, never recorded from sound. On save,
//! `debug_record::write_debug_recording_on_save` writes both vectors out as
//! separate charts (`recorded.harpchart`/`expected.harpchart`), so
//! `note_bench` can compare a detector's output against ground truth never
//! itself derived from any detector — solving the tempo-precision problem a
//! "play along to a pre-authored chart" workflow would otherwise have (see
//! `note_bench::DEFAULT_TIMING_TOLERANCE_SECS`'s own doc comment).
//!
//! Deliberately simpler than ordinary Edit-mode note editing
//! (`interaction::select_or_add`/`apply_modifier`): no collision/overlap
//! checks (annotating routinely means marking right on top of a
//! wrong/phantom recorded note), no auto-length trimming, no
//! chord-direction enforcement. Just place, select, set technique, delete.
//! `place_or_select_expected`/`apply_expected_modifier` reuse the same
//! `sticky_dir`/`sticky_pitch`/`sticky_expr` fields ordinary editing does.
//!
//! Rendered as a colored, unfilled outline overlay
//! (`rebuild_expected_notes_overlay`) on top of the ordinary grid in every
//! mode (so you can review annotations from Edit mode too), but only
//! selectable in `Mode::ExpectedNotes` — via the grid's own background-cell
//! click observer (`grid.rs`), never the overlay visuals directly (always
//! `Pickable::IGNORE`, so a click always reaches the background cell, which
//! resolves hole/tick and dispatches by current mode — no z-order tie-break
//! needed between the overlay and the ordinary note visuals underneath).
//! Unwindowed (one visual per `expected_notes` note regardless of scroll
//! position) — a deliberate simplification for the short clips this
//! targets (single notes, bends, chords, short phrases), unlike the
//! ordinary note grid, which must window for arbitrarily long real songs.

use bevy::picking::Pickable;
use bevy::picking::events::{PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

use super::TICKS_PER_BEAT;
use super::interaction::{
    cycle_sticky_bend, cycle_sticky_pitch, next_expr, next_pitch, pitch_fits,
};
use super::panel::mod_button_active;
use super::state::{
    Dir, DragKind, DragState, Edge, EditorState, GridNote, Mode, Pitch, apply_resize, move_target,
    note_rect, pitch_color, pitch_compatible, pitch_forced_dir,
};
use super::ui::{ExpectedNotesGroup, GridContent, ModButton, ModeButton};
use harmonicon_app::app::AppState;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::ActionButtonStyle;
use harmonicon_platform::theme::{LoadedTheme, SongEditorColors};

// ── EditorState accessors ────────────────────────────────────────────────────
//
// A second `impl EditorState` block, separate from `state.rs`'s own — purely
// a file-size trim (`docs/physical_design_plan.md`'s ~1000-line budget) now
// that this dev-only module exists to hold it; Rust allows an inherent impl
// to be split across files freely, and everything here is only ever called
// from this same file anyway.

impl EditorState {
    fn expected_note_by_id(&self, id: u32) -> Option<&GridNote> {
        self.expected_notes.iter().find(|n| n.id == id)
    }

    fn expected_selected_note(&self) -> Option<&GridNote> {
        self.expected_selected.and_then(|id| self.expected_note_by_id(id))
    }

    fn expected_selected_note_mut(&mut self) -> Option<&mut GridNote> {
        let id = self.expected_selected?;
        self.expected_notes.iter_mut().find(|n| n.id == id)
    }
}

// ── Interaction ───────────────────────────────────────────────────────────────

/// The grid background cell's click handler while in [`Mode::ExpectedNotes`]
/// (see `grid.rs`'s own call site) — the sibling of `interaction::
/// select_or_add`, but for [`EditorState::expected_notes`]: selects an
/// existing expected note at `hole`/`tick` if there is one, otherwise places
/// a fresh one there (default length, current sticky dir/pitch/expr — no
/// collision check against anything, ever, see the module docs).
pub(super) fn place_or_select_expected(state: &mut EditorState, hole: u8, tick: usize) {
    if let Some(existing) = state
        .expected_notes
        .iter()
        .find(|n| n.hole == hole && n.tick <= tick && tick < n.tick + n.len)
    {
        state.expected_selected = Some(existing.id);
        return;
    }

    let dir = state.sticky_dir;
    let pitch = if pitch_compatible(state.sticky_pitch, &state.effective_harp(), hole) {
        state.sticky_pitch
    } else {
        Pitch::Normal
    };
    let dir = pitch_forced_dir(pitch).unwrap_or(dir);
    let expr = state.sticky_expr;

    let id = state.expected_next_id;
    state.expected_next_id += 1;
    state.expected_notes.push(GridNote { id, hole, tick, len: TICKS_PER_BEAT, dir, pitch, expr });
    state.expected_selected = Some(id);
}

fn delete_expected_selected(state: &mut EditorState) {
    let Some(id) = state.expected_selected.take() else {
        return;
    };
    state.expected_notes.retain(|n| n.id != id);
}

/// The [`ExpectedNotesGroup`] mod-button row's click handler — the sibling
/// of `interaction::apply_modifier`, operating on `expected_selected`/
/// `expected_notes` instead of `selected`/`notes`, and without that
/// function's chord-direction enforcement (`enforce_direction`/
/// `enforce_expr`): this layer's notes are independent annotations, not a
/// chart that needs internally-consistent simultaneous-note chords.
pub(super) fn apply_expected_modifier(state: &mut EditorState, kind: ModButton) {
    if kind == ModButton::Delete {
        delete_expected_selected(state);
        return;
    }
    // Expected notes carry no depth, belong to no phrase, and are placed
    // by hand as ground truth — nothing for these to act on in this layer.
    if matches!(
        kind,
        ModButton::Depth
            | ModButton::Call
            | ModButton::Split
            | ModButton::Phrase
            | ModButton::TransposeUp
            | ModButton::TransposeDown
    ) {
        return;
    }
    if matches!(kind, ModButton::Blow | ModButton::Draw) {
        let dir = if kind == ModButton::Blow { Dir::Blow } else { Dir::Draw };
        state.sticky_dir = dir;
        if pitch_forced_dir(state.sticky_pitch).is_some_and(|d| d != dir) {
            state.sticky_pitch = Pitch::Normal;
        }
        if let Some(note) = state.expected_selected_note_mut() {
            note.dir = dir;
            if pitch_forced_dir(note.pitch).is_some_and(|d| d != dir) {
                note.pitch = Pitch::Normal;
            }
        }
        return;
    }

    let harp = state.effective_harp();
    let Some(note) = state.expected_selected_note_mut() else {
        match kind {
            ModButton::Bend => cycle_sticky_bend(state),
            ModButton::Overblow => cycle_sticky_pitch(state, Pitch::Overblow),
            ModButton::Overdraw => cycle_sticky_pitch(state, Pitch::Overdraw),
            ModButton::Slide => cycle_sticky_pitch(state, Pitch::Slide),
            ModButton::Wah | ModButton::Vibrato => {
                state.sticky_expr = next_expr(kind, state.sticky_expr);
            }
            _ => {}
        }
        return;
    };
    if matches!(kind, ModButton::Wah | ModButton::Vibrato) {
        note.expr = next_expr(kind, note.expr);
        return;
    }
    // The same cycle Edit mode runs, for a selection of exactly this note.
    let Some(target) = next_pitch(kind, note, std::slice::from_ref(note), &harp) else {
        return;
    };
    if target == Pitch::Normal || pitch_fits(target, note.hole, &harp) {
        note.pitch = target;
        if let Some(dir) = pitch_forced_dir(target) {
            note.dir = dir;
        }
    }
}

// ── UI: mode button + mod-button row ─────────────────────────────────────────

/// The "Draw correct notes" mode button — spawned alongside Edit/Record/
/// Play/Lock in the mod panel's always-visible top strip (`mod_panel.rs`).
pub(super) fn spawn_expected_notes_mode_button(
    transport: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    style: ActionButtonStyle,
) {
    super::panel_widgets::mode_button(
        transport,
        ModeButton::ExpectedNotes,
        loc.msg("editor-mode-expected"),
        loc.msg("editor-mode-expected-tooltip"),
        "\u{2713}",
        style,
        colors,
        |_: On<Activate>, mut t: super::transport::Transport| {
            t.state.mode = Mode::ExpectedNotes;
            t.stop_all();
        },
    );
}

/// A single button in the [`ExpectedNotesGroup`] row — same visual shape as
/// `panel_widgets::mod_button`, but wired to [`apply_expected_modifier`]
/// instead of `interaction::apply_modifier`, and marked with
/// [`ExpectedModButton`] instead of a bare [`ModButton`] so this row's
/// coloring/visibility stays independent of the ordinary `EditModeGroup`
/// row's: `panel::update_mod_panel`/`update_technique_button_visibility`
/// query `ModButton` globally (unscoped by group), so reusing it directly
/// here would recolor/hide these buttons against `state.notes`/
/// `state.selected` instead of `state.expected_notes`/`expected_selected`.
#[derive(Component, Clone, Copy)]
struct ExpectedModButton(ModButton);

fn spawn_expected_mod_button(
    panel: &mut ChildSpawnerCommands,
    kind: ModButton,
    label: harmonicon_platform::localization::LocalizedStr,
    tooltip: harmonicon_platform::localization::LocalizedStr,
    icon: &str,
    style: ActionButtonStyle,
    colors: SongEditorColors,
) {
    let text = super::panel_widgets::button_content_text(style, icon, &label);
    let mut ec = super::panel_widgets::spawn_shell(panel, colors.btn_bg, tooltip);
    ec.insert(ExpectedModButton(kind))
        .observe(move |_: On<Activate>, mut state: ResMut<EditorState>| {
            apply_expected_modifier(&mut state, kind);
        })
        .with_children(|b| {
            super::panel_widgets::spawn_label(b, text);
        });
}

/// Spawned once into the mod panel (`mod_panel.rs`), as its own
/// [`ExpectedNotesGroup`]-wrapped row alongside `EditModeGroup`/
/// `RecordModeGroup`/`PlayModeGroup` — shown only in
/// [`Mode::ExpectedNotes`] (`panel::update_mode_visibility` already handles
/// this group like the other three).
pub(super) fn spawn_expected_notes_group(
    panel: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    mode: Mode,
    style: ActionButtonStyle,
) {
    panel
        .spawn((
            ExpectedNotesGroup,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(6.0),
                display: if mode == Mode::ExpectedNotes { Display::Flex } else { Display::None },
                ..default()
            },
        ))
        .with_children(|g| {
            for (kind, key, tooltip, icon) in [
                (ModButton::Blow, "mod-blow", "mod-blow-tooltip", "\u{2191}"),
                (ModButton::Draw, "mod-draw", "mod-draw-tooltip", "\u{2193}"),
                (ModButton::Bend, "mod-bend", "mod-bend-tooltip", "\u{007E}"),
                (ModButton::Overblow, "mod-overblow", "mod-overblow-tooltip", "\u{21C8}"),
                (ModButton::Overdraw, "mod-overdraw", "mod-overdraw-tooltip", "\u{21CA}"),
                (ModButton::Slide, "mod-slide", "mod-slide-tooltip", "\u{2194}"),
                (ModButton::Delete, "mod-delete", "mod-delete-tooltip", "\u{25CB}"),
            ] {
                spawn_expected_mod_button(
                    g,
                    kind,
                    loc.msg(key),
                    loc.msg(tooltip),
                    icon,
                    style,
                    colors,
                );
            }
        });
}

fn update_expected_technique_button_visibility(
    state: Res<EditorState>,
    mut buttons: Query<(&ExpectedModButton, &mut Node)>,
) {
    let diatonic_only = state.harmonica_kind.is_diatonic();
    for (ExpectedModButton(kind), mut node) in &mut buttons {
        let visible = match kind {
            ModButton::Bend | ModButton::Overblow | ModButton::Overdraw => diatonic_only,
            ModButton::Slide => !diatonic_only,
            _ => continue,
        };
        let display = if visible { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

fn update_expected_mod_panel(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ExpectedModButton, &mut BackgroundColor)>,
) {
    let colors = theme.song_editor_colors();
    let selected = state.expected_selected_note();
    let (dir, pitch, expr) = match selected {
        Some(n) => (n.dir, n.pitch, n.expr),
        None => (state.sticky_dir, state.sticky_pitch, state.sticky_expr),
    };
    for (ExpectedModButton(kind), mut bg) in &mut buttons {
        let active = mod_button_active(*kind, dir, pitch, expr);
        let color = if active { colors.btn_active } else { colors.btn_bg };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

// ── Rendering + drag/resize ───────────────────────────────────────────────────

/// One expected-note's overlay visual, carrying its own id so the drag/
/// resize observers below (and nothing else — this is *not* reused as a
/// generic lookup key the way `grid::NoteView` is) can find their own
/// entity again without needing to search by position.
#[derive(Component)]
struct ExpectedNoteVisual(u32);

/// Rebuilds the whole overlay from scratch whenever `EditorState` changes —
/// simple despawn-all/respawn-all rather than diffing, same trade-off
/// `grid::rebuild_grid` itself makes, and cheap here since this is
/// unwindowed (see the module docs) over what's meant to stay a short clip.
/// Skips entirely while a drag/resize on this layer is in flight
/// (`expected_dragging`), same reason `grid::rebuild_grid` skips during an
/// ordinary note drag: rebuilding would despawn the very entity picking has
/// captured the gesture on.
fn rebuild_expected_notes_overlay(
    mut commands: Commands,
    state: Res<EditorState>,
    content: Query<Entity, With<GridContent>>,
    old: Query<Entity, With<ExpectedNoteVisual>>,
) {
    if state.expected_dragging.is_some() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let Ok(content) = content.single() else {
        return;
    };
    // Interactive (clickable/draggable) only in `Mode::ExpectedNotes` — in
    // any other mode this is a pure review overlay, and must not steal
    // clicks meant for the ordinary grid underneath it.
    let interactive = state.mode == Mode::ExpectedNotes;
    let pick = if interactive { Pickable::default() } else { Pickable::IGNORE };
    commands.entity(content).with_children(|c| {
        for note in &state.expected_notes {
            let (left, top, width, height) = note_rect(note);
            let selected = state.expected_selected == Some(note.id);
            let color = pitch_color(note.pitch);
            let id = note.id;
            let mut ec = c.spawn((
                ExpectedNoteVisual(id),
                WidgetButton,
                ZIndex(4),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(top),
                    width: Val::Px(width),
                    height: Val::Px(height),
                    border: UiRect::all(Val::Px(if selected { 3.0 } else { 2.0 })),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(color),
                pick,
            ));
            ec.observe(move |_: On<Activate>, mut state: ResMut<EditorState>| {
                state.expected_selected = Some(id);
            })
            .observe(move |_: On<PointerDragStart>, mut state: ResMut<EditorState>| {
                if state.expected_dragging.is_some() {
                    return;
                }
                let Some(note) = state.expected_note_by_id(id).copied() else {
                    return;
                };
                state.expected_selected = Some(id);
                state.expected_dragging = Some(DragState::new(id, DragKind::Move, &note));
            })
            .observe(
                move |ev: On<PointerDrag>,
                      mut state: ResMut<EditorState>,
                      ui_scale: Res<UiScale>,
                      mut nodes: Query<&mut Node, With<ExpectedNoteVisual>>| {
                    let Some(drag) = state.expected_dragging.as_ref() else {
                        return;
                    };
                    if drag.kind != DragKind::Move {
                        return;
                    }
                    let hole_count = state.hole_count();
                    let (hole, tick) = move_target(
                        drag.start_hole,
                        drag.start_tick,
                        ev.distance.x / ui_scale.0,
                        ev.distance.y / ui_scale.0,
                        hole_count,
                    );
                    let Some(n) = state.expected_notes.iter_mut().find(|n| n.id == id) else {
                        return;
                    };
                    n.hole = hole;
                    n.tick = tick;
                    let (left, top, _, _) = note_rect(n);
                    if let Ok(mut node) = nodes.get_mut(ev.entity) {
                        node.left = Val::Px(left);
                        node.top = Val::Px(top);
                    }
                },
            )
            .observe(move |_: On<PointerDragEnd>, mut state: ResMut<EditorState>| {
                if matches!(&state.expected_dragging, Some(d) if d.kind == DragKind::Move) {
                    state.expected_dragging = None;
                }
            });
            ec.with_children(|n| {
                n.spawn_empty().apply_scene(bsn! {
                    Text({note.dir.arrow()})
                    TextFont { font_size: {FontSize::Px(15.0)} }
                    TextColor({color})
                    ~{Pickable::IGNORE}
                });
                spawn_expected_resize_handle(n, id, Edge::Left, pick);
                spawn_expected_resize_handle(n, id, Edge::Right, pick);
            });
        }
    });
}

/// Width of this layer's in-note resize handles. Local to the module
/// because it's the only user: the ordinary note grid puts its grips
/// *outside* the note instead (`ui::ResizeGrip`), since in-note handles
/// cover a short note whole. This layer keeps them because it's a
/// dev-only benchmark-authoring surface whose expected notes are written
/// at whole-beat lengths, not 16ths.
const EXPECTED_HANDLE_W: f32 = 8.0;

/// The move-drag sibling for resizing: writes into
/// `expected_notes`/`expected_dragging` and has no neighboring-note bound
/// to respect (`apply_resize`'s `left_bound: 0, right_bound: None` — this
/// layer never collision-checks, see the module docs), so it needs no
/// per-hole scan of other notes before resizing.
fn spawn_expected_resize_handle(
    parent: &mut ChildSpawnerCommands,
    id: u32,
    edge: Edge,
    pick: Pickable,
) {
    let mut node = Node {
        position_type: PositionType::Absolute,
        top: Val::Px(0.0),
        bottom: Val::Px(0.0),
        width: Val::Px(EXPECTED_HANDLE_W),
        ..default()
    };
    match edge {
        Edge::Left => node.left = Val::Px(0.0),
        Edge::Right => node.right = Val::Px(0.0),
    }
    parent
        .spawn((node, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.35)), pick))
        .observe(move |_: On<PointerDragStart>, mut state: ResMut<EditorState>| {
            if state.expected_dragging.is_some() {
                return;
            }
            let Some(note) = state.expected_note_by_id(id).copied() else {
                return;
            };
            state.expected_selected = Some(id);
            state.expected_dragging = Some(DragState::new(id, DragKind::Resize(edge), &note));
        })
        .observe(
            move |ev: On<PointerDrag>,
                  mut state: ResMut<EditorState>,
                  ui_scale: Res<UiScale>,
                  mut boxes: Query<(&ExpectedNoteVisual, &mut Node)>| {
                let Some(drag) = state.expected_dragging.as_ref() else {
                    return;
                };
                if drag.kind != DragKind::Resize(edge) {
                    return;
                }
                let steps = ((ev.distance.x / ui_scale.0) / super::TICK_W).round() as i32;
                let (tick, len) =
                    apply_resize(drag.start_tick, drag.start_len, edge, steps, 0, None);
                let Some(n) = state.expected_notes.iter_mut().find(|n| n.id == id) else {
                    return;
                };
                n.tick = tick;
                n.len = len;
                let (left, _, width, _) = note_rect(n);
                // The handle is a child of the note box, so `ev.entity`
                // (the handle) isn't what needs its `Node` updated — find
                // the box by its own `ExpectedNoteVisual` id instead.
                if let Some((_, mut node)) = boxes.iter_mut().find(|(v, _)| v.0 == id) {
                    node.left = Val::Px(left);
                    node.width = Val::Px(width);
                }
            },
        )
        .observe(move |_: On<PointerDragEnd>, mut state: ResMut<EditorState>| {
            if matches!(&state.expected_dragging, Some(d) if d.kind == DragKind::Resize(edge)) {
                state.expected_dragging = None;
            }
        });
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub(super) struct ExpectedNotesPlugin;

impl Plugin for ExpectedNotesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rebuild_expected_notes_overlay.run_if(resource_exists_and_changed::<EditorState>),
                update_expected_technique_button_visibility,
                update_expected_mod_panel,
            )
                .run_if(in_state(AppState::SongEditor2)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::{Expr, WAH_HZ_MIN, max_bend};
    use super::*;

    fn with_note(hole: u8) -> EditorState {
        let mut state = EditorState::default();
        place_or_select_expected(&mut state, hole, 0);
        state
    }

    fn note(state: &EditorState) -> &GridNote {
        state.expected_selected_note().unwrap()
    }

    #[test]
    fn overblow_toggles_on_a_hole_that_has_one_and_forces_blow() {
        let mut state = with_note(4);
        state.expected_notes[0].dir = Dir::Draw;
        apply_expected_modifier(&mut state, ModButton::Overblow);
        assert_eq!((note(&state).pitch, note(&state).dir), (Pitch::Overblow, Dir::Blow));
        apply_expected_modifier(&mut state, ModButton::Overblow);
        assert_eq!(note(&state).pitch, Pitch::Normal);
    }

    #[test]
    fn overblow_does_nothing_on_a_hole_without_one() {
        let mut state = with_note(2);
        apply_expected_modifier(&mut state, ModButton::Overblow);
        assert_eq!(note(&state).pitch, Pitch::Normal);
    }

    #[test]
    fn bend_deepens_to_the_holes_cap_then_switches_off() {
        let mut state = with_note(1);
        let cap = max_bend(&state.effective_harp(), 1);
        let mut depths = Vec::new();
        for _ in 0..8 {
            apply_expected_modifier(&mut state, ModButton::Bend);
            depths.push(note(&state).bend());
            if note(&state).pitch == Pitch::Normal {
                break;
            }
        }
        assert_eq!(depths.iter().copied().fold(0.0, f32::max), cap);
        assert_eq!(depths.last(), Some(&0.0));
    }

    #[test]
    fn wah_steps_its_rate_on_the_selected_note_only() {
        let mut state = with_note(4);
        apply_expected_modifier(&mut state, ModButton::Wah);
        assert_eq!(note(&state).expr, Expr::Wah(WAH_HZ_MIN));
        assert_eq!(state.sticky_expr, Expr::None);
    }
}
