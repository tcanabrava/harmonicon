// SPDX-License-Identifier: MIT

//! Dev-only ("--features dev") benchmark-authoring workflow — never wired
//! up outside it, see `mod.rs`'s conditional `mod expected_notes;`.
//!
//! `song_editor::debug_record` records raw mic audio plus whatever the live
//! detector actually produced (`EditorState::notes` — mistakes, phantom
//! notes, and all; that's expected, not a problem to avoid). This module is
//! how you correct the record *afterward*, at your own pace: a "Draw
//! correct notes" mode button (next to Edit/Record/Play/Lock) enters
//! [`Mode::ExpectedNotes`], where clicking the same grid places/selects
//! notes on a second, independent vector — [`EditorState::expected_notes`]
//! — instead of the ordinary one. Nothing here is ever recorded from sound;
//! it's purely hand-placed ground truth, marking down what should actually
//! have been played. On save, `debug_record::write_debug_recording_on_save`
//! writes both vectors out as separate charts (`recorded.harpchart` /
//! `expected.harpchart`), so `note_bench` can compare a detector's output
//! against ground truth that was never itself derived from any detector —
//! solving the tempo-precision problem a "play along to a pre-authored
//! chart" workflow would otherwise have (see `note_bench::
//! DEFAULT_TIMING_TOLERANCE_SECS`'s own doc comment).
//!
//! Deliberately simpler than ordinary Edit-mode note editing
//! (`interaction::select_or_add`/`apply_modifier`): no collision/overlap
//! checks at all (annotating "what should have sounded" routinely means
//! marking it right on top of a wrong/phantom recorded note, or another
//! expected note), no auto-length trimming against a neighboring note, no
//! chord-direction enforcement across simultaneous notes. Just place,
//! select, set its technique, delete. `place_or_select_expected`/
//! `apply_expected_modifier` reuse the same `sticky_dir`/`sticky_pitch`/
//! `sticky_expr` "currently armed technique" fields ordinary editing does —
//! one shared "what would a newly placed note get" concept regardless of
//! which layer you're placing into.
//!
//! Rendered as a colored, unfilled outline overlay (`rebuild_expected_notes_
//! overlay`) on top of the ordinary grid, in every mode (so you can review
//! your annotations while just looking at Edit mode too) — but only
//! selectable/clickable in `Mode::ExpectedNotes` itself, via the grid's own
//! background-cell click observer (`grid.rs`), never via the overlay
//! visuals directly (they're `Pickable::IGNORE` unconditionally, so a click
//! always reaches the background cell underneath, which resolves hole/tick
//! and decides what to do with it based on the current mode — no z-order/
//! picking-priority tie-break to get wrong between the overlay and the
//! ordinary note visuals it's drawn over). Unwindowed (one visual per
//! `expected_notes` note, regardless of scroll position) — a deliberate
//! simplification appropriate for the short clips this feature targets
//! (single notes, bends, chords, short phrases — see the roadmap's own
//! dataset list), unlike the ordinary note grid, which does need to window
//! for arbitrarily long real songs.

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::panel::mod_button_active;
use super::state::{
    Dir, EditorState, Expr, GridNote, HarmonicaKind, Mode, Pitch, VIBRATO_HZ_MAX, VIBRATO_HZ_MIN,
    VIBRATO_HZ_STEP, WAH_HZ_MAX, WAH_HZ_MIN, WAH_HZ_STEP, max_bend, note_rect, overblow_ok,
    overdraw_ok, pitch_color, pitch_compatible, pitch_forced_dir,
};
use super::ui::{ExpectedNotesGroup, GridContent, ModButton, ModeButton};
use super::TICKS_PER_BEAT;
use crate::app::AppState;
use crate::dialogs::tooltip::Tooltip;
use crate::localization::LocalizationExt;
use crate::theme::{LoadedTheme, SongEditorColors};
use bevy_fluent::prelude::Localization;

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
        self.expected_selected
            .and_then(|id| self.expected_note_by_id(id))
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
    let pitch = if pitch_compatible(state.sticky_pitch, hole) {
        state.sticky_pitch
    } else {
        Pitch::Normal
    };
    let dir = pitch_forced_dir(pitch).unwrap_or(dir);
    let expr = state.sticky_expr;

    let id = state.expected_next_id;
    state.expected_next_id += 1;
    state.expected_notes.push(GridNote {
        id,
        hole,
        tick,
        len: TICKS_PER_BEAT,
        dir,
        pitch,
        expr,
    });
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
    if matches!(kind, ModButton::Blow | ModButton::Draw) {
        let dir = if kind == ModButton::Blow {
            Dir::Blow
        } else {
            Dir::Draw
        };
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

    let Some(note) = state.expected_selected_note_mut() else {
        match kind {
            ModButton::Bend => super::interaction::cycle_sticky_bend(state),
            ModButton::Overblow => {
                super::interaction::cycle_sticky_pitch(state, Pitch::Overblow)
            }
            ModButton::Overdraw => {
                super::interaction::cycle_sticky_pitch(state, Pitch::Overdraw)
            }
            ModButton::Slide => super::interaction::cycle_sticky_pitch(state, Pitch::Slide),
            ModButton::Wah => super::interaction::cycle_sticky_wah(state),
            ModButton::Vibrato => super::interaction::cycle_sticky_vibrato(state),
            _ => {}
        }
        return;
    };
    match kind {
        ModButton::Blow | ModButton::Draw => unreachable!(),
        ModButton::Bend => {
            let max = max_bend(note.hole);
            if max <= 0.0 {
                return;
            }
            let next = note.bend() + 0.5;
            note.pitch = if next > max + f32::EPSILON {
                Pitch::Normal
            } else {
                Pitch::Bend(next)
            };
        }
        ModButton::Overblow => {
            if overblow_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overblow {
                    Pitch::Normal
                } else {
                    Pitch::Overblow
                };
                if note.pitch == Pitch::Overblow {
                    note.dir = Dir::Blow;
                }
            }
        }
        ModButton::Overdraw => {
            if overdraw_ok(note.hole) {
                note.pitch = if note.pitch == Pitch::Overdraw {
                    Pitch::Normal
                } else {
                    Pitch::Overdraw
                };
                if note.pitch == Pitch::Overdraw {
                    note.dir = Dir::Draw;
                }
            }
        }
        ModButton::Slide => {
            note.pitch = if note.pitch == Pitch::Slide {
                Pitch::Normal
            } else {
                Pitch::Slide
            };
        }
        ModButton::Wah => {
            let next = match note.expr {
                Expr::Wah(hz) => hz + WAH_HZ_STEP,
                _ => WAH_HZ_MIN,
            };
            note.expr = if next > WAH_HZ_MAX + f32::EPSILON {
                Expr::None
            } else {
                Expr::Wah(next)
            };
        }
        ModButton::Vibrato => {
            let next = match note.expr {
                Expr::Vibrato(hz) => hz + VIBRATO_HZ_STEP,
                _ => VIBRATO_HZ_MIN,
            };
            note.expr = if next > VIBRATO_HZ_MAX + f32::EPSILON {
                Expr::None
            } else {
                Expr::Vibrato(next)
            };
        }
        ModButton::Delete => unreachable!(),
    }
}

// ── UI: mode button + mod-button row ─────────────────────────────────────────

/// The "Draw correct notes" mode button — spawned alongside Edit/Record/
/// Play/Lock in the mod panel's always-visible top strip (`mod_panel.rs`).
pub(super) fn spawn_expected_notes_mode_button(
    transport: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    super::panel_widgets::mode_button(
        transport,
        ModeButton::ExpectedNotes,
        loc.msg("editor-mode-expected"),
        loc.msg("editor-mode-expected-tooltip"),
        colors,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<super::playback::EditorAudio>>,
         mut practice: ResMut<super::practice::PracticeState>,
         mut record: ResMut<super::record::RecordState>,
         mut playhead: ResMut<super::playback::Playhead>,
         mut pitch_range: ResMut<crate::audio_system::pitch_detect::PitchRange>,
         mut commands: Commands| {
            state.mode = Mode::ExpectedNotes;
            super::practice::stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            super::record::stop_record(
                &mut state,
                &playing,
                &mut record,
                &mut playhead,
                &mut pitch_range,
                &mut commands,
            );
        },
    );
}

/// A single button in the [`ExpectedNotesGroup`] row — same visual shape as
/// `panel_widgets::mod_button`, but wired to [`apply_expected_modifier`]
/// instead of `interaction::apply_modifier`, and marked with
/// [`ExpectedModButton`] instead of a bare [`ModButton`] so this row's
/// coloring/visibility stay independent of the ordinary `EditModeGroup`
/// row's — both `panel::update_mod_panel` and `panel::
/// update_technique_button_visibility` query `ModButton` globally
/// (unscoped by group), so reusing that component directly here would have
/// them recolor/hide these buttons against `state.notes`/`state.selected`
/// instead of `state.expected_notes`/`expected_selected`.
#[derive(Component, Clone, Copy)]
struct ExpectedModButton(ModButton);

fn spawn_expected_mod_button(
    panel: &mut ChildSpawnerCommands,
    kind: ModButton,
    label: crate::localization::LocalizedStr,
    tooltip: crate::localization::LocalizedStr,
    colors: SongEditorColors,
) {
    panel
        .spawn((
            Button,
            ExpectedModButton(kind),
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(colors.btn_bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            Tooltip(String::from(tooltip)),
        ))
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            apply_expected_modifier(&mut state, kind);
        })
        .with_children(|b| {
            b.spawn((
                Text::new(String::from(label)),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
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
                display: if mode == Mode::ExpectedNotes {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
        ))
        .with_children(|g| {
            spawn_expected_mod_button(
                g,
                ModButton::Blow,
                loc.msg("mod-blow"),
                loc.msg("mod-blow-tooltip"),
                colors,
            );
            spawn_expected_mod_button(
                g,
                ModButton::Draw,
                loc.msg("mod-draw"),
                loc.msg("mod-draw-tooltip"),
                colors,
            );
            spawn_expected_mod_button(
                g,
                ModButton::Bend,
                loc.msg("mod-bend"),
                loc.msg("mod-bend-tooltip"),
                colors,
            );
            spawn_expected_mod_button(
                g,
                ModButton::Overblow,
                loc.msg("mod-overblow"),
                loc.msg("mod-overblow-tooltip"),
                colors,
            );
            spawn_expected_mod_button(
                g,
                ModButton::Overdraw,
                loc.msg("mod-overdraw"),
                loc.msg("mod-overdraw-tooltip"),
                colors,
            );
            spawn_expected_mod_button(
                g,
                ModButton::Slide,
                loc.msg("mod-slide"),
                loc.msg("mod-slide-tooltip"),
                colors,
            );
            spawn_expected_mod_button(
                g,
                ModButton::Delete,
                loc.msg("mod-delete"),
                loc.msg("mod-delete-tooltip"),
                colors,
            );
        });
}

fn update_expected_technique_button_visibility(
    state: Res<EditorState>,
    mut buttons: Query<(&ExpectedModButton, &mut Node)>,
) {
    let diatonic_only = matches!(state.harmonica_kind, HarmonicaKind::Diatonic);
    for (ExpectedModButton(kind), mut node) in &mut buttons {
        let visible = match kind {
            ModButton::Bend | ModButton::Overblow | ModButton::Overdraw => diatonic_only,
            ModButton::Slide => !diatonic_only,
            _ => continue,
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn update_expected_mod_panel(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ExpectedModButton, &mut BackgroundColor)>,
) {
    let colors = theme.song_editor_colors();
    let selected = state.expected_selected_note().copied();
    let (dir, pitch, expr) = match selected {
        Some(n) => (n.dir, n.pitch, n.expr),
        None => (state.sticky_dir, state.sticky_pitch, state.sticky_expr),
    };
    for (ExpectedModButton(kind), mut bg) in &mut buttons {
        let active = mod_button_active(*kind, dir, pitch, expr);
        bg.0 = if active {
            colors.btn_active
        } else {
            colors.btn_bg
        };
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// One expected-note's overlay visual — see the module docs for why this is
/// a separate, always-`Pickable::IGNORE` entity rather than reusing
/// `grid::spawn_note`/`NoteView`.
#[derive(Component)]
struct ExpectedNoteVisual;

/// Rebuilds the whole overlay from scratch whenever `EditorState` changes —
/// simple despawn-all/respawn-all rather than diffing, same trade-off
/// `grid::rebuild_grid` itself makes, and cheap here since this is
/// unwindowed (see the module docs) over what's meant to stay a short clip.
fn rebuild_expected_notes_overlay(
    mut commands: Commands,
    state: Res<EditorState>,
    content: Query<Entity, With<GridContent>>,
    old: Query<Entity, With<ExpectedNoteVisual>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let Ok(content) = content.single() else {
        return;
    };
    commands.entity(content).with_children(|c| {
        for note in &state.expected_notes {
            let (left, top, width, height) = note_rect(note);
            let selected = state.expected_selected == Some(note.id);
            let color = pitch_color(note.pitch);
            c.spawn((
                ExpectedNoteVisual,
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
                Pickable::IGNORE,
            ))
            .with_children(|n| {
                n.spawn((
                    Text::new(note.dir.arrow()),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(color),
                    Pickable::IGNORE,
                ));
            });
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
