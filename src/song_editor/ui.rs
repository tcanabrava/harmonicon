// SPDX-License-Identifier: MIT

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;
use bevy::window::WindowResized;

use super::meta_form::{spawn_hole_column, spawn_hole_column_rows, spawn_meta_form};
use super::mod_panel::spawn_mod_panel;
use super::playback::{EditorAudio, EditorProgressFill, Playhead, PlayheadLine};
use super::state::{EditorState, Scroll, TimelineTool};
use super::{BEAT_W, HOLE_COL_W, NOTE_PAD, ROW_H, grid_height};
use crate::theme::LoadedTheme;
use bevy_fluent::prelude::Localization;

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(super) struct EditorRoot;

#[derive(Component)]
pub(super) struct GridArea;

#[derive(Component)]
pub(super) struct GridContent;

#[derive(Component)]
pub(super) struct GridItem;

/// The row wrapping the hole column and the grid area, sized to
/// [`grid_height`] — resized when the harmonica's hole count changes.
#[derive(Component)]
pub(super) struct GridRowContainer;

/// The fixed-width hole column's container (number + box per hole). Its
/// per-hole rows are (re)spawned by `grid::rebuild_grid` alongside the grid
/// lanes, since both depend on the current harmonica's hole count.
#[derive(Component)]
pub(super) struct HoleColumnContent;

/// The label showing the current [`super::state::HarmonicaKind`] ("Diatonic"
/// / "Chromatic") next to its toggle button.
#[derive(Component)]
pub(super) struct HarmonicaKindText;

#[derive(Component)]
pub(super) struct NoteView(pub(super) u32);

#[derive(Component)]
pub(super) struct MoveGhost;

#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum ModButton {
    Blow,
    Draw,
    Bend,
    Overblow,
    Overdraw,
    /// Chromatic-only: the slide button, a half-step raise. Hidden for
    /// diatonic charts, shown in place of Bend/Overblow/Overdraw.
    Slide,
    Wah,
    Vibrato,
    Delete,
}

/// The always-visible Edit/Perform mode switch and Lock toggle. Unlike
/// [`ModButton`], these don't go through `apply_modifier` — they change
/// `EditorState::mode`/`user_locked` directly, and switching to Edit also
/// needs to stop any running playback/practice, which needs more than just
/// `&mut EditorState` gives `apply_modifier`.
#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum ModeButton {
    Edit,
    Perform,
    Lock,
}

/// The Erase/Remove timeline-tool toggle buttons — see `timeline`'s module
/// docs. Picking one sets `EditorState::timeline_tool`; picking the
/// already-active one deselects it back to `TimelineTool::None`, same
/// deselect-by-reclicking convention as the mod-panel's other toggles.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(super) struct TimelineToolButton(pub(super) TimelineTool);

/// Wraps the note-editing button cluster (Blow, Draw, Bend, ...), shown only
/// in [`Mode::Edit`]. See `update_mode_visibility`.
#[derive(Component)]
pub(super) struct EditModeGroup;

/// Wraps the playback/practice button cluster (Play, Pause, Stop, Practice),
/// shown only in [`Mode::Perform`]. See `update_mode_visibility`.
#[derive(Component)]
pub(super) struct PerformModeGroup;

#[derive(Component)]
pub(super) struct BendDot;

/// Marks a mod button's label text so [`super::panel::update_mod_panel`] can
/// append the selected note's configured rate (e.g. "Vibrato 5Hz"). `base` is
/// the localized label cached at spawn time, since the per-frame update only
/// has the note's numeric state, not the `Localization` resource.
#[derive(Component)]
pub(super) struct ModButtonLabel {
    pub(super) kind: ModButton,
    pub(super) base: String,
}

/// Marks the Record button's label text so
/// [`super::panel::update_record_button_label`] can swap it between its
/// idle and actively-recording text — cached at spawn time, same reasoning
/// as [`ModButtonLabel::base`].
#[derive(Component)]
pub(super) struct RecordButtonLabel {
    pub(super) idle: String,
    pub(super) active: String,
}

#[derive(Component)]
pub(super) struct MetaFieldBox(pub(super) super::state::Field);

#[derive(Component)]
pub(super) struct MetaFieldText(pub(super) super::state::Field);

#[derive(Component)]
pub(super) struct StatusMsg;

/// Empty container [`super::midi_import::rebuild_midi_track_combobox`]
/// (re)spawns the MIDI track-picker combobox under, once a MIDI file has
/// been imported — empty until then, since the track list isn't known
/// before that.
#[derive(Component)]
pub(super) struct MidiTrackComboboxSlot;

// ── Lifecycle systems ─────────────────────────────────────────────────────────

pub(super) fn init_state(mut commands: Commands, existing: Option<Res<EditorState>>) {
    if existing.is_none() {
        commands.insert_resource(EditorState::default());
    }
    commands.insert_resource(Playhead::default());
    commands.insert_resource(Scroll::default());
}

pub(super) fn force_grid_rebuild(mut state: ResMut<EditorState>) {
    state.set_changed();
}

/// `rebuild_grid` only runs when `EditorState` changes, but the number of
/// visible columns depends on the window width too — without this, resizing
/// the window would leave the grid showing its old column count until some
/// unrelated edit happens to touch `EditorState` and trigger a rebuild.
pub(super) fn rebuild_grid_on_resize(
    mut resized: MessageReader<WindowResized>,
    mut state: ResMut<EditorState>,
) {
    if resized.read().next().is_some() {
        state.set_changed();
    }
}

pub(super) fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<EditorRoot>>,
    audio: Query<Entity, With<EditorAudio>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    for e in &audio {
        commands.entity(e).despawn();
    }
}

/// Resizes the fixed chrome around the note grid — the row wrapping the hole
/// column and the grid area, the grid area itself, its scrollable content,
/// and the playhead line — to fit the harmonica's current hole count. Runs
/// alongside [`sync_hole_column`] and `grid::rebuild_grid`, gated the same
/// way, since hole count only changes when the harmonica kind is switched.
pub(super) fn sync_chrome_height(
    state: Res<EditorState>,
    mut rows: Query<
        &mut Node,
        Or<(
            With<GridRowContainer>,
            With<GridArea>,
            With<GridContent>,
            With<PlayheadLine>,
            With<super::timeline::TimelineSplitLine>,
            With<super::timeline::TimelineHighlight>,
        )>,
    >,
) {
    let h = grid_height(state.hole_count());
    for mut node in &mut rows {
        node.height = Val::Px(h);
    }
}

/// Respawns the hole column's per-hole rows for the current harmonica's hole
/// count. The rows carry no interaction state (unlike the note grid), so a
/// full despawn/respawn on every `EditorState` change is simple and cheap.
pub(super) fn sync_hole_column(
    mut commands: Commands,
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    col: Query<(Entity, Option<&Children>), With<HoleColumnContent>>,
) {
    let Ok((entity, children)) = col.single() else {
        return;
    };
    if let Some(children) = children {
        for &c in children {
            commands.entity(c).despawn();
        }
    }
    let hole_count = state.hole_count();
    let colors = theme.song_editor_colors();
    commands.entity(entity).insert(Node {
        width: Val::Px(HOLE_COL_W),
        height: Val::Px(grid_height(hole_count)),
        flex_direction: FlexDirection::Column,
        flex_shrink: 0.0,
        ..default()
    });
    commands.entity(entity).with_children(|col| {
        spawn_hole_column_rows(col, colors, hole_count, &loc);
    });
}

// ── Setup ─────────────────────────────────────────────────────────────────────

pub(super) fn setup(
    mut commands: Commands,
    loc: Res<Localization>,
    theme: Res<LoadedTheme>,
    state: Res<EditorState>,
) {
    let colors = theme.song_editor_colors();
    let mode = state.mode;
    let hole_count = state.hole_count();
    commands
        .spawn((
            EditorRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(colors.editor_bg),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(5.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    EditorProgressFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                ));
            });

            // Everything below the progress bar, in a scrollable column —
            // total content height (grid + mod panel + meta form + status
            // bar) routinely exceeds a laptop window's height, and without
            // this whatever's last in the tree (the meta form's MIDI-track
            // combobox) is simply pushed off-screen with no way to reach it.
            // Same `Overflow::scroll_y()` + `ScrollArea` pattern
            // `menu::pages::lessons`/`dialogs::file_dialog` already
            // establish; `min_height: Val::Px(0.0)` lets this flex item
            // actually shrink below its content size instead of refusing to
            // clip (the standard flexbox "min-height: auto" gotcha).
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollArea,
            ))
            .with_children(|scroll| {
                scroll
                    .spawn((
                        GridRowContainer,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(grid_height(hole_count)),
                            flex_direction: FlexDirection::Row,
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        spawn_hole_column(row, colors, hole_count, &loc);
                        row.spawn((
                            GridArea,
                            Node {
                                flex_grow: 1.0,
                                height: Val::Px(grid_height(hole_count)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ))
                        .with_children(|ga| {
                            ga.spawn((
                                GridContent,
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(0.0),
                                    height: Val::Px(grid_height(hole_count)),
                                    ..default()
                                },
                            ))
                            .with_children(|content| {
                                content.spawn((
                                    MoveGhost,
                                    ZIndex(2),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        width: Val::Px(BEAT_W - 2.0),
                                        height: Val::Px(ROW_H - 2.0 * NOTE_PAD),
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(colors.ghost_ok.with_alpha(0.30)),
                                    BorderColor::all(colors.ghost_ok),
                                    Visibility::Hidden,
                                    Pickable::IGNORE,
                                ));
                                content.spawn((
                                    PlayheadLine,
                                    ZIndex(3),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(0.0),
                                        width: Val::Px(2.0),
                                        height: Val::Px(grid_height(hole_count)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.95, 0.30, 0.30)),
                                    Visibility::Hidden,
                                    Pickable::IGNORE,
                                ));
                                // Erase/Remove tool overlays — see `timeline`'s
                                // module docs. Both hidden until a tool picks a
                                // split point or drag span; `update_timeline_
                                // overlays` (unconditional, like the playhead/move
                                // ghost above) repositions and shows/hides them
                                // every frame.
                                content.spawn((
                                    super::timeline::TimelineSplitLine,
                                    ZIndex(3),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(0.0),
                                        width: Val::Px(2.0),
                                        height: Val::Px(grid_height(hole_count)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.95, 0.75, 0.20)),
                                    Visibility::Hidden,
                                    Pickable::IGNORE,
                                ));
                                content.spawn((
                                    super::timeline::TimelineHighlight,
                                    ZIndex(1),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(0.0),
                                        height: Val::Px(grid_height(hole_count)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.95, 0.30, 0.20, 0.22)),
                                    Visibility::Hidden,
                                    Pickable::IGNORE,
                                ));
                            });
                        });
                    });

                spawn_mod_panel(scroll, &loc, colors, mode);
                spawn_meta_form(scroll, &loc, colors);

                scroll.spawn((
                    StatusMsg,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.40, 0.15)),
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        ..default()
                    },
                ));
            });
        });
}
