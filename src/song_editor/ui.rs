// SPDX-License-Identifier: MIT

use bevy::audio::AudioSource;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;
use bevy::window::WindowResized;

use super::harpchart::safe_path_segment;
use super::interaction::apply_modifier;
use super::playback::{
    EditorAudio, EditorProgressFill, Playhead, PlayheadLine, start_playback, toggle_pause,
};
use super::practice::{PracticeState, start_practice, stop_practice};
use super::record::{RecordState, start_record, stop_record};
use super::state::{
    EditorState, FIELDS, Field, HARP_KEYS, HarmonicaKind, Mode, POSITIONS, Scroll, TimelineTool,
};
use super::{
    AppState, BEAT_W, HEADER_H, HOLE_COL_W, LOAD_PURPOSE, MIDI_PURPOSE, MUSIC_PURPOSE, NOTE_PAD,
    ROW_H, SAVE_PURPOSE, SILENCE_ROW_H, grid_height,
};
use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::dialogs::tooltip::Tooltip;
use crate::dialogs::confirm_dialog::OpenConfirmDialog;
use crate::song_editor::state::{TimelineDrag, normalize_range};
use crate::song_editor::timeline::request_confirm;
use crate::localization::{LocalizationExt, LocalizedStr};
use crate::settings::AudioSettings;
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
pub(super) struct MetaFieldBox(pub(super) Field);

#[derive(Component)]
pub(super) struct MetaFieldText(pub(super) Field);

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

fn spawn_hole_column(
    row: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    hole_count: u8,
    loc: &Localization,
) {
    row.spawn((
        HoleColumnContent,
        Node {
            width: Val::Px(HOLE_COL_W),
            height: Val::Px(grid_height(hole_count)),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..default()
        },
    ))
    .with_children(|col| {
        spawn_hole_column_rows(col, colors, hole_count, loc);
    });
}

/// Respawns the hole column's contents (called from [`setup`] initially, and
/// from [`sync_hole_column`] whenever the harmonica's hole count changes).
fn spawn_hole_column_rows(
    col: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    hole_count: u8,
    loc: &Localization,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Px(HEADER_H),
        ..default()
    });
    for hole in 1..=hole_count {
        col.spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(ROW_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new(format!("{hole:02}")),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(colors.label),
            ));
            r.spawn((
                Node {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(colors.hole_box),
                BorderColor::all(Color::srgb(0.45, 0.45, 0.55)),
            ));
        });
    }
    // Label for the silence track's background strip (spawned in
    // `grid::rebuild_grid`) — keeps this column's total height matching
    // `grid_height` so the hole rows on the right stay aligned with it.
    col.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(SILENCE_ROW_H),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Tooltip(String::from(loc.msg("editor-silence-track-tooltip"))),
    ))
    .with_children(|r| {
        r.spawn((
            Text::new(loc.msg("editor-silence-track-label").to_string()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(colors.label),
            Pickable::IGNORE,
        ));
    });
}

/// The mod panel: a short, fixed global-transport strip (Back / Edit /
/// Perform / Lock / Save / Load — always the same regardless of mode), then
/// a `flex_wrap: Wrap` contextual tool strip below it (the current mode's
/// whole tool palette — up to 13 buttons + 3 separators in Edit mode). Two
/// stacked rows rather than one ever-growing row, so a narrow/small window
/// wraps the tool strip onto a second line instead of rendering buttons past
/// the right edge with no way to reach them. The panel's own height is
/// therefore auto (driven by its two rows' content) rather than the fixed
/// `Val::Px(52.0)` a single non-wrapping row could get away with.
fn spawn_mod_panel(
    root: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    mode: Mode,
) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(colors.panel_bg),
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|transport| {
                transport_button(
                    transport,
                    loc.msg("back"),
                    loc.msg("editor-back-tooltip"),
                    colors.transport_back,
                    |_: On<Pointer<Click>>,
                     mut next: ResMut<NextState<AppState>>,
                     mut ret_play: ResMut<crate::app::ReturnToPlay>| {
                        ret_play.0 = true;
                        next.set(AppState::Menu);
                    },
                );
                panel_separator(transport);

                // Edit/Perform/Lock: always visible, regardless of which
                // mode-group below is currently shown.
                mode_button(
                    transport,
                    ModeButton::Edit,
                    loc.msg("editor-mode-edit"),
                    loc.msg("editor-mode-edit-tooltip"),
                    colors,
                    |_: On<Pointer<Click>>,
                     mut state: ResMut<EditorState>,
                     playing: Query<Entity, With<EditorAudio>>,
                     mut practice: ResMut<PracticeState>,
                     mut record: ResMut<RecordState>,
                     mut playhead: ResMut<Playhead>,
                     mut commands: Commands| {
                        state.mode = Mode::Edit;
                        // Leaving Perform mode hides Play/Pause/Stop/Practice/
                        // Record, so nothing would be left to stop anything
                        // that's running.
                        stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
                        stop_record(&mut state, &playing, &mut record, &mut playhead, &mut commands);
                    },
                );
                mode_button(
                    transport,
                    ModeButton::Perform,
                    loc.msg("editor-mode-perform"),
                    loc.msg("editor-mode-perform-tooltip"),
                    colors,
                    |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                        state.mode = Mode::Perform;
                    },
                );
                mode_button(
                    transport,
                    ModeButton::Lock,
                    loc.msg("editor-lock"),
                    loc.msg("editor-lock-tooltip"),
                    colors,
                    |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                        state.user_locked = !state.user_locked;
                    },
                );
                panel_separator(transport);

                spawn_file_buttons(transport, loc, colors);
            });

        panel
            .spawn((
                EditModeGroup,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(6.0),
                    // `Display::None`, not `Visibility::Hidden` — Visibility
                    // only skips rendering, it still reserves this group's
                    // full layout width, which pushed the other group off to
                    // the right instead of freeing its place.
                    display: if mode == Mode::Edit {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
            ))
            .with_children(|g| {
                mod_button(
                    g,
                    ModButton::Blow,
                    loc.msg("mod-blow"),
                    loc.msg("mod-blow-tooltip"),
                    colors,
                );
                mod_button(
                    g,
                    ModButton::Draw,
                    loc.msg("mod-draw"),
                    loc.msg("mod-draw-tooltip"),
                    colors,
                );
                panel_separator(g);
                mod_button(
                    g,
                    ModButton::Bend,
                    loc.msg("mod-bend"),
                    loc.msg("mod-bend-tooltip"),
                    colors,
                );
                mod_button(
                    g,
                    ModButton::Overblow,
                    loc.msg("mod-overblow"),
                    loc.msg("mod-overblow-tooltip"),
                    colors,
                );
                mod_button(
                    g,
                    ModButton::Overdraw,
                    loc.msg("mod-overdraw"),
                    loc.msg("mod-overdraw-tooltip"),
                    colors,
                );
                mod_button(
                    g,
                    ModButton::Slide,
                    loc.msg("mod-slide"),
                    loc.msg("mod-slide-tooltip"),
                    colors,
                );
                mod_button(
                    g,
                    ModButton::Wah,
                    loc.msg("mod-wah"),
                    loc.msg("mod-wah-tooltip"),
                    colors,
                );
                mod_button(
                    g,
                    ModButton::Vibrato,
                    loc.msg("mod-vibrato"),
                    loc.msg("mod-vibrato-tooltip"),
                    colors,
                );
                g.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                mod_button(
                    g,
                    ModButton::Delete,
                    loc.msg("mod-delete"),
                    loc.msg("mod-delete-tooltip"),
                    colors,
                );
                panel_separator(g);
                timeline_tool_button(
                    g,
                    TimelineToolButton(TimelineTool::Select),
                    loc.msg("editor-tool-select"),
                    loc.msg("editor-tool-select-tooltip"),
                    colors,
                );
                timeline_tool_button(
                    g,
                    TimelineToolButton(TimelineTool::Erase),
                    loc.msg("editor-tool-erase"),
                    loc.msg("editor-tool-erase-tooltip"),
                    colors,
                );
                timeline_tool_button(
                    g,
                    TimelineToolButton(TimelineTool::Remove),
                    loc.msg("editor-tool-remove"),
                    loc.msg("editor-tool-remove-tooltip"),
                    colors,
                );
            });

        panel
            .spawn((
                PerformModeGroup,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(6.0),
                    display: if mode == Mode::Perform {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
            ))
            .with_children(|g| {
                spawn_playback_buttons(g, loc, colors);
            });
    });
}

pub(super) fn mode_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    kind: ModeButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    colors: SongEditorColors,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    panel
        .spawn((
            Button,
            kind,
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
        .observe(on_click)
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

/// An Erase/Remove timeline-tool toggle button — see [`TimelineToolButton`].
fn timeline_tool_button(
    panel: &mut ChildSpawnerCommands,
    kind: TimelineToolButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    colors: SongEditorColors,
) {
    panel
        .spawn((
            Button,
            kind,
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
        .observe(
            move |_: On<Pointer<Click>>,
                loc: Res<Localization>,
                mut state: ResMut<EditorState>,
                mut open: MessageWriter<OpenConfirmDialog>| {
                if let Some(TimelineDrag { start, end }) = state.timeline_drag {
                    let (s, e) = normalize_range(start, end);
                    if kind == TimelineToolButton(TimelineTool::Erase) {
                        state.timeline_tool = TimelineTool::Erase;
                        request_confirm(&mut state, &loc, &mut open, s, e);
                    } else if kind == TimelineToolButton(TimelineTool::Remove) {
                        state.timeline_tool = TimelineTool::Remove;
                        request_confirm(&mut state, &loc, &mut open, s, e);
                    }
                };

                state.timeline_tool = if state.timeline_tool == kind.0 {
                    TimelineTool::None
                } else {
                    kind.0
                };
                state.timeline_drag = None;
                state.timeline_split = None;
            },
        )
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

pub(super) fn mod_button(
    panel: &mut ChildSpawnerCommands,
    kind: ModButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    colors: SongEditorColors,
) {
    panel
        .spawn((
            Button,
            kind,
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
        .observe(
            move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                apply_modifier(&mut state, kind);
            },
        )
        .with_children(|b| {
            let base = String::from(label);
            let mut text = b.spawn((
                Text::new(base.clone()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
            if matches!(kind, ModButton::Wah | ModButton::Vibrato) {
                text.insert(ModButtonLabel { kind, base });
            }
            if kind == ModButton::Bend {
                b.spawn((
                    BendDot,
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        margin: UiRect::left(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.90, 0.20, 0.20)),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ));
            }
        });
}

fn panel_separator(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Node {
            width: Val::Px(1.0),
            height: Val::Px(28.0),
            margin: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.30, 0.30, 0.40)),
    ));
}

/// Chart file I/O — always visible, in both Edit and Perform mode.
fn spawn_file_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    transport_button(
        panel,
        loc.msg("editor-save"),
        loc.msg("editor-save-tooltip"),
        colors.transport_save,
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         loc: Res<Localization>,
         mut open: MessageWriter<OpenFileDialog>| {
            let default_name = format!(
                "{}.harpchart",
                safe_path_segment(if state.name.is_empty() {
                    "chart"
                } else {
                    &state.name
                })
            );
            open.write(OpenFileDialog {
                purpose: SAVE_PURPOSE,
                title: String::from(loc.msg("dialog-save-chart")),
                extensions: vec!["harpchart".into()],
                start_dir: Some(std::path::PathBuf::from("assets/songs")),
                mode: DialogMode::Save { default_name },
            });
        },
    );
    transport_button(
        panel,
        loc.msg("editor-load"),
        loc.msg("editor-load-tooltip"),
        colors.transport_load,
        |_: On<Pointer<Click>>, loc: Res<Localization>, mut open: MessageWriter<OpenFileDialog>| {
            open.write(OpenFileDialog {
                purpose: LOAD_PURPOSE,
                title: String::from(loc.msg("dialog-load-chart")),
                extensions: vec!["harpchart".into()],
                start_dir: Some(std::path::PathBuf::from("assets/songs")),
                mode: DialogMode::Open,
            });
        },
    );
}

/// Play/Pause/Stop/Practice — only shown in [`Mode::Perform`] (wrapped in
/// [`PerformModeGroup`] by the caller).
fn spawn_playback_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    transport_button(
        panel,
        loc.msg("editor-play"),
        loc.msg("editor-play-tooltip"),
        colors.transport_play,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         sinks: Query<&AudioSink, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            // Paused, not stopped: resume in place rather than restarting.
            if playhead.playing && playhead.paused {
                toggle_pause(&mut playhead, &sinks);
                return;
            }
            practice.reset(); // exit practice mode before starting preview playback
            // A recording in progress owns the shared `Playhead` clock —
            // close it out (rather than letting `start_playback` below
            // silently repurpose it out from under `record.open`) before
            // taking over.
            stop_record(&mut state, &playing, &mut record, &mut playhead, &mut commands);
            start_playback(
                &state,
                &mut sources,
                &settings,
                &playing,
                &mut playhead,
                &mut commands,
            );
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        loc.msg("editor-pause-tooltip"),
        colors.transport_pause,
        |_: On<Pointer<Click>>,
         mut playhead: ResMut<Playhead>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            toggle_pause(&mut playhead, &sinks);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-stop"),
        loc.msg("editor-stop-tooltip"),
        colors.transport_stop,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            stop_record(&mut state, &playing, &mut record, &mut playhead, &mut commands);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-practice"),
        loc.msg("editor-practice-tooltip"),
        colors.transport_practice,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands,
         loc: Res<Localization>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            // Paused, not stopped: resume in place rather than stopping.
            if practice.active && playhead.paused {
                toggle_pause(&mut playhead, &sinks);
                return;
            }
            if practice.active {
                stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            } else {
                // A recording in progress owns the shared `Playhead` clock —
                // close it out before `start_practice` below repurposes it.
                stop_record(&mut state, &playing, &mut record, &mut playhead, &mut commands);
                start_practice(
                    &state,
                    &mut sources,
                    &settings,
                    &playing,
                    &mut practice,
                    &mut playhead,
                    &mut commands,
                    &loc,
                );
            }
        },
    );
    spawn_record_button(
        panel,
        loc.msg("editor-record"),
        loc.msg("editor-record-stop"),
        loc.msg("editor-record-tooltip"),
        colors.transport_record,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            if record.active {
                stop_record(&mut state, &playing, &mut record, &mut playhead, &mut commands);
            } else {
                practice.reset(); // exit practice mode before recording, same as Play does
                start_record(
                    &state,
                    &mut sources,
                    &settings,
                    &playing,
                    &mut record,
                    &mut playhead,
                    &mut commands,
                );
            }
        },
    );
}

pub(super) fn transport_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    bg: Color,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    panel
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            Tooltip(String::from(tooltip)),
        ))
        .observe(on_click)
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

/// Like [`transport_button`], except its label swaps between `idle_label`
/// and `active_label` at runtime — [`super::panel::update_record_button_label`]
/// picks one based on [`RecordState::active`]. Kept separate from
/// `transport_button` rather than adding an optional param there, since
/// every other transport button has a fixed label.
fn spawn_record_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    idle_label: LocalizedStr,
    active_label: LocalizedStr,
    tooltip: LocalizedStr,
    bg: Color,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    panel
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            Tooltip(String::from(tooltip)),
        ))
        .observe(on_click)
        .with_children(|b| {
            b.spawn((
                Text::new(String::from(idle_label.clone())),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
                RecordButtonLabel {
                    idle: String::from(idle_label),
                    active: String::from(active_label),
                },
            ));
        });
}

fn spawn_meta_form(root: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        form.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|line| {
            line.spawn((
                Node {
                    width: Val::Px(150.0),
                    ..default()
                },
                Text::new(format!("{}:", loc.msg("editor-field-harmonica"))),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(colors.label),
            ));
            line.spawn((
                Button,
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(26.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(colors.field_bg),
                BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                Tooltip(String::from(loc.msg("editor-harmonica-toggle-tooltip"))),
            ))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                let next = match state.harmonica_kind {
                    HarmonicaKind::Diatonic => HarmonicaKind::Chromatic,
                    HarmonicaKind::Chromatic => HarmonicaKind::Diatonic,
                };
                state.set_harmonica_kind(next);
            })
            .with_children(|b| {
                b.spawn((
                    HarmonicaKindText,
                    Text::new(String::new()),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                ));
            });
        });

        for (field, label) in FIELDS {
            form.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    Node {
                        width: Val::Px(150.0),
                        ..default()
                    },
                    Text::new(format!("{}:", loc.msg(label))),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(colors.label),
                ));

                let mut btn = line.spawn((
                    Button,
                    MetaFieldBox(field),
                    Node {
                        width: Val::Px(240.0),
                        height: Val::Px(26.0),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(colors.field_bg),
                    BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                ));

                if field == Field::Key {
                    btn.insert(Tooltip(String::from(loc.msg("editor-field-key-tooltip"))))
                        .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                            let idx = HARP_KEYS
                                .iter()
                                .position(|&k| k == state.key.as_str())
                                .unwrap_or(0);
                            state.key = HARP_KEYS[(idx + 1) % HARP_KEYS.len()].into();
                        });
                } else if field == Field::Position {
                    btn.insert(Tooltip(String::from(
                        loc.msg("editor-field-position-tooltip"),
                    )))
                    .observe(
                        |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                            let idx = POSITIONS
                                .iter()
                                .position(|&p| p == state.position.as_str())
                                .unwrap_or(0);
                            state.position = POSITIONS[(idx + 1) % POSITIONS.len()].into();
                        },
                    );
                } else {
                    btn.observe(
                        move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                            state.focus = Some(field);
                        },
                    );
                }

                btn.with_children(|b| {
                    b.spawn((
                        MetaFieldText(field),
                        Text::new(String::new()),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                    ));
                });

                if field == Field::Music {
                    line.spawn((
                        Button,
                        Node {
                            height: Val::Px(26.0),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.24, 0.36)),
                        BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                        Tooltip(String::from(loc.msg("editor-browse-tooltip"))),
                    ))
                    .observe(
                        |_: On<Pointer<Click>>,
                         loc: Res<Localization>,
                         mut open: MessageWriter<OpenFileDialog>| {
                            open.write(OpenFileDialog {
                                purpose: MUSIC_PURPOSE,
                                title: String::from(loc.msg("dialog-select-music")),
                                extensions: vec!["ogg".into()],
                                start_dir: dirs::home_dir(),
                                mode: DialogMode::Open,
                            });
                        },
                    )
                    .with_children(|b| {
                        b.spawn((
                            Text::new(String::from(loc.msg("editor-browse"))),
                            TextFont {
                                font_size: FontSize::Px(13.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        ));
                    });
                }
            });
        }

        form.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|line| {
            line.spawn((
                Node {
                    width: Val::Px(150.0),
                    ..default()
                },
                Text::new(format!("{}:", loc.msg("editor-field-midi-track"))),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(colors.label),
            ));
            line.spawn((
                Button,
                Node {
                    height: Val::Px(26.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.24, 0.30, 0.20)),
                BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                Tooltip(String::from(loc.msg("editor-import-midi-tooltip"))),
            ))
            .observe(
                |_: On<Pointer<Click>>,
                 loc: Res<Localization>,
                 mut open: MessageWriter<OpenFileDialog>| {
                    open.write(OpenFileDialog {
                        purpose: MIDI_PURPOSE,
                        title: String::from(loc.msg("dialog-select-midi")),
                        extensions: vec!["mid".into(), "midi".into()],
                        start_dir: dirs::home_dir(),
                        mode: DialogMode::Open,
                    });
                },
            )
            .with_children(|b| {
                b.spawn((
                    Text::new(String::from(loc.msg("editor-import-midi"))),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                ));
            });
            line.spawn((
                MidiTrackComboboxSlot,
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
            ));
        });
    });
}
