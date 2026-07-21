// SPDX-License-Identifier: MIT

use bevy::picking::Pickable;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;
use bevy::window::WindowResized;

use super::interaction::drag_grid_scrollbar;
use super::meta_form::{spawn_hole_column, spawn_hole_column_rows};
use super::mod_panel::spawn_mod_panel;
use super::playback::{EditorAudio, EditorProgressFill, Playhead, PlayheadLine};
use super::state::{EditorState, Mode, Scroll, TimelineTool};
use super::{BEAT_W, HOLE_COL_W, NOTE_PAD, ROW_H, grid_height};
use crate::theme::{LoadedTheme, SongEditorColors};
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

/// The horizontal scrollbar's track, spanning only the grid area's own
/// width (not the hole column) below it — hidden entirely when the song's
/// notes all fit within the visible width, since there's nothing to scroll
/// to. See `interaction::update_grid_scrollbar`.
#[derive(Component)]
pub(super) struct GridScrollTrack;

/// One note's tiny rectangle on the scrollbar track — together they sketch
/// the whole song in miniature (horizontal = time, vertical = hole lane),
/// so the scrollbar doubles as a minimap of where the notes are. Rebuilt by
/// `interaction::update_scrollbar_markers` whenever the notes change.
#[derive(Component)]
pub(super) struct GridScrollMarker;

/// The scrollbar's thumb, sized/positioned each frame from [`Scroll`] vs.
/// the notes' total span vs. the track's own width.
#[derive(Component)]
pub(super) struct GridScrollThumb;

/// The fixed-width hole column's container (number + box per hole). Its
/// per-hole rows are (re)spawned by `grid::rebuild_grid` alongside the grid
/// lanes, since both depend on the current harmonica's hole count.
#[derive(Component)]
pub(super) struct HoleColumnContent;

/// The label showing the current [`super::state::HarmonicaKind`] ("Diatonic"
/// / "Chromatic") next to its toggle button.
#[derive(Component)]
pub(super) struct HarmonicaKindText;

/// The label showing the current [`super::state::ContentKind`] ("Record
/// Song" / "Record Lesson") next to its toggle button.
#[derive(Component)]
pub(super) struct ContentKindText;

// `LessonFormGroup`/`LessonDetailsBody`/`LessonDetailsToggleLabel`/
// `LessonConditionalRow` — the lesson-fields panel's own components — live
// in `lesson_form.rs` itself, not here, since nothing outside that module
// uses them (same "components live with their one feature" precedent
// `playback.rs`/`timeline.rs` already set).

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
    Record,
    Play,
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
/// shown only in [`Mode::Play`]. See `update_mode_visibility`.
#[derive(Component)]
pub(super) struct PlayModeGroup;

/// Wraps the recording transport cluster (Play, Pause, Stop, Finish),
/// shown only in [`Mode::Record`]. See `update_mode_visibility`.
#[derive(Component)]
pub(super) struct RecordModeGroup;

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

/// Empty container [`super::meta_form::spawn_scale_combobox`] spawns the
/// scale-picker combobox under, once — unlike [`MidiTrackComboboxSlot`],
/// its option list is fixed at compile time, so this only ever spawns once
/// rather than rebuilding on some external event.
#[derive(Component)]
pub(super) struct ScaleComboboxSlot;

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
            With<super::timeline_overlay::TimelineSplitLine>,
            With<super::timeline_overlay::TimelineHighlight>,
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

            // Fixed chrome: the grid row (own horizontal scroll) + mod
            // panel — kept out of the form `ScrollArea` below, since sharing
            // one scrollable area between the grid and the form fields let
            // scrolling either one move both (a horizontal-scrollbar drag on
            // the grid would also drag the page vertically on a small window).
            spawn_fixed_chrome(root, &loc, colors, mode, hole_count);

            // The form fields (meta form, lesson form, status bar), in their
            // own scrollable column — a fully expanded lesson-details panel
            // routinely exceeds a laptop window's height. Same
            // `Overflow::scroll_y()` + `ScrollArea` pattern `menu::pages::
            // lessons`/`dialogs::file_dialog` use; `min_height: Val::Px(0.0)`
            // lets this flex item shrink below its content size (the
            // flexbox "min-height: auto" gotcha). The sibling `Scrollbar`
            // (`scroll::spawn_editor_scrollbar`) is what makes the fact that
            // this scrolls at all visible to the player.
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..default()
            })
            .with_children(|outer| {
                let scroll_area = outer
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollArea,
                    ))
                    .with_children(|scroll| {
                        super::scroll::spawn_form_scroll_content(scroll, &loc, colors);
                    })
                    .id();
                super::scroll::spawn_editor_scrollbar(outer, scroll_area, colors);
            });
        });
}

/// The editor's always-visible chrome, above the scrollable form area: the
/// grid row (hole column + grid + its own horizontal scrollbar) and the mod
/// panel. Kept out of the `ScrollArea` — see [`setup`]'s own comment for why.
fn spawn_fixed_chrome(
    root: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    mode: Mode,
    hole_count: u8,
) {
    root.spawn((
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
        spawn_hole_column(row, colors, hole_count, loc);
        row.spawn((
            GridArea,
            // So `interaction::pan_wheel` only pans horizontally
            // while the pointer is actually over the grid.
            Hovered::default(),
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
                super::timeline_overlay::spawn_persistent_entities(content, hole_count);
            });
        });
    });

    // Horizontal scrollbar for the grid, spanning only the grid area's own
    // width (the leading spacer matches `HOLE_COL_W`) — hidden by
    // `update_grid_scrollbar` whenever the song fits the visible width.
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            width: Val::Px(HOLE_COL_W),
            flex_shrink: 0.0,
            ..default()
        });
        row.spawn((
            GridScrollTrack,
            Node {
                flex_grow: 1.0,
                height: Val::Px(10.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            Visibility::Hidden,
        ))
        .with_children(|track| {
            // ZIndex above the note markers, which are spawned later (as
            // fresh children) and would otherwise paint over the thumb.
            track
                .spawn((
                    GridScrollThumb,
                    ZIndex(1),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        left: Val::Px(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(colors.accent.with_alpha(0.65)),
                ))
                .observe(drag_grid_scrollbar);
        });
    });

    spawn_mod_panel(root, loc, colors, mode);
}

