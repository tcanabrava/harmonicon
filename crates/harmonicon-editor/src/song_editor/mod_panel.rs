// SPDX-License-Identifier: MIT

//! The editor's scrollable tool sidebar, built from button helpers in
//! `super::panel_widgets`.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::AppState;
use super::panel_widgets::{
    mod_button, mode_button, panel_separator, timeline_tool_button, transport_button,
};
use super::state::{EditorState, Mode, TimelineTool};
use super::transport::{
    Transport, spawn_file_buttons, spawn_playback_buttons, spawn_record_buttons,
};
use super::ui::{
    EditModeGroup, EditorToolbar, EditorToolbarContent, ModButton, ModeButton, NoteColumn,
    PlayModeGroup, RecordModeGroup, TimelineToolButton,
};
use harmonicon_audio::pitch_detect::PitchAlgorithm;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::ActionButtonStyle;
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::algo_picker::{algo_labels, attach_algo_tooltip, on_algo_selected};
use harmonicon_ui::dialogs::combobox;

/// Width of one toolbar column in logical pixels.
pub(super) fn toolbar_width(style: ActionButtonStyle) -> f32 {
    match style {
        ActionButtonStyle::IconOnly => 56.0,
        ActionButtonStyle::TextBesideIcon | ActionButtonStyle::TextOnly => 168.0,
    }
}

/// Whether the note column sits *beside* the document column (icon-only:
/// two 56 px columns, and the toolbar scrolls half as far) or *below* it
/// (text styles: two 168 px columns would be a third of a small screen,
/// so they stack into the one column the toolbar always had).
pub(super) fn two_columns(style: ActionButtonStyle) -> bool {
    style == ActionButtonStyle::IconOnly
}

/// The toolbar's width with the note column shown (Edit mode) — see
/// [`two_columns`].
pub(super) fn toolbar_width_with_note_column(style: ActionButtonStyle) -> f32 {
    if two_columns(style) { 2.0 * toolbar_width(style) } else { toolbar_width(style) }
}

/// Builds the scrollable document and note-control columns beside the grid.
pub(super) fn spawn_mod_panel(
    root: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    mode: Mode,
    editor_root: Entity,
    algorithm: PitchAlgorithm,
    style: ActionButtonStyle,
) {
    let width_without = toolbar_width(style);
    let width_with_note_column = toolbar_width_with_note_column(style);
    root.spawn((
        EditorToolbar { width_with_note_column, width_without },
        // Read by `view_scroll::wheel_toolbar`, so a wheel gesture over the
        // toolbar scrolls it instead of panning the grid sideways.
        bevy::picking::hover::Hovered::default(),
        Node {
            width: Val::Px(if mode == Mode::Edit { width_with_note_column } else { width_without }),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            // Never let the grid squeeze the toolbar: a flex row hands out
            // leftover space by shrinking, and this is the one column that
            // must keep its buttons hittable.
            flex_shrink: 0.0,
            // Taller than the screen is the normal case on a phone — the
            // inner content column is offset by `apply_toolbar_scroll`
            // instead of a `ScrollArea`, so a drag anywhere on the toolbar
            // scrolls it (see `drag_toolbar`).
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(colors.panel_bg),
    ))
    .observe(super::view_scroll::drag_toolbar)
    .with_children(|outer| {
        outer
            .spawn((
                EditorToolbarContent,
                Node {
                    width: Val::Percent(100.0),
                    // Two columns side by side (icon-only), or stacked into
                    // one (text styles) — see `two_columns`.
                    flex_direction: if two_columns(style) {
                        FlexDirection::Row
                    } else {
                        FlexDirection::Column
                    },
                    align_items: AlignItems::FlexStart,
                    row_gap: Val::Px(6.0),
                    column_gap: Val::Px(0.0),
                    ..default()
                },
            ))
            .with_children(|columns| {
                // ── Left column: the document and the tools ──────────────
                columns
                    .spawn(Node {
                        width: Val::Px(width_without),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|panel| {
                        panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Stretch,
                                row_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|transport| {
                                transport_button(
                    transport,
                    loc.msg("editor-back-label"),
                    loc.msg("editor-back-tooltip"),
                    "\u{2190}",
                    style,
                    colors.transport_back,
                    |_: On<Activate>,
                     mut next: ResMut<NextState<AppState>>,
                     mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>| {
                        ret_play.0 = true;
                        next.set(AppState::Menu);
                    },
                );
                                panel_separator(transport);

                                // Edit/Record/Play/Lock: always visible, regardless of
                                // which mode-group below is currently shown. Every mode
                                // switch stops whatever the departed mode had running —
                                // its transport is about to disappear, so nothing would be
                                // left to stop it.
                                mode_button(
                                    transport,
                                    ModeButton::Edit,
                                    loc.msg("editor-mode-edit"),
                                    loc.msg("editor-mode-edit-tooltip"),
                                    "\u{270E}",
                                    style,
                                    colors,
                                    |_: On<Activate>, mut t: Transport| {
                                        t.state.mode = Mode::Edit;
                                        t.stop_all();
                                    },
                                );
                                mode_button(
                                    transport,
                                    ModeButton::Record,
                                    loc.msg("editor-mode-record"),
                                    loc.msg("editor-mode-record-tooltip"),
                                    "\u{23FA}",
                                    style,
                                    colors,
                                    |_: On<Activate>, mut t: Transport| {
                                        t.state.mode = Mode::Record;
                                        // A recording can only have been started from this
                                        // mode itself, so only Play-mode playback/practice
                                        // needs stopping here.
                                        t.stop_practice();
                                    },
                                );
                                mode_button(
                                    transport,
                                    ModeButton::Play,
                                    loc.msg("editor-mode-play"),
                                    loc.msg("editor-mode-play-tooltip"),
                                    "\u{1F3B5}",
                                    style,
                                    colors,
                                    |_: On<Activate>, mut t: Transport| {
                                        t.state.mode = Mode::Play;
                                        t.stop_record();
                                    },
                                );
                                mode_button(
                                    transport,
                                    ModeButton::Lock,
                                    loc.msg("editor-lock"),
                                    loc.msg("editor-lock-tooltip"),
                                    "\u{1F512}",
                                    style,
                                    colors,
                                    |_: On<Activate>, mut state: ResMut<EditorState>| {
                                        state.user_locked = !state.user_locked;
                                    },
                                );

                                panel_separator(transport);

                                // Undo/redo are plain click actions (not a mode toggle),
                                // so `transport_button` rather than `mode_button` — and a
                                // no-op rather than visually disabled when the relevant
                                // stack is empty, the same "clicking does nothing" shape
                                // `UndoHistory::undo`/`redo` already have. See
                                // `undo::UndoHistory`'s doc comment for exactly what
                                // counts as an undoable edit.
                                transport_button(
                    transport,
                    loc.msg("editor-undo"),
                    loc.msg("editor-undo-tooltip"),
                    "\u{21B6}",
                    style,
                    colors.btn_bg,
                    |_: On<Activate>,
                     mut state: ResMut<EditorState>,
                     mut history: ResMut<super::undo::UndoHistory>| {
                        history.undo(&mut state);
                    },
                )
                .insert(super::ui::UndoRedoButton::Undo);
                                transport_button(
                    transport,
                    loc.msg("editor-redo"),
                    loc.msg("editor-redo-tooltip"),
                    "\u{21B7}",
                    style,
                    colors.btn_bg,
                    |_: On<Activate>,
                     mut state: ResMut<EditorState>,
                     mut history: ResMut<super::undo::UndoHistory>| {
                        history.redo(&mut state);
                    },
                )
                .insert(super::ui::UndoRedoButton::Redo);

                                // On-screen equivalents of Ctrl+C/Ctrl+V
                                // (`interaction::handle_copy_paste`) — the *only* way to
                                // copy/paste on a touch-only device with no keyboard.
                                // Kept together here even though Copy needs a selection:
                                // splitting the pair across columns would cost more than
                                // a Copy that no-ops with nothing selected. Paste has no
                                // cursor position to anchor on without a mouse, so it
                                // lands at the start of the current view
                                // (`state.scroll_beat`) instead of "wherever the mouse
                                // is," unlike the keyboard shortcut. Delete lives in the
                                // note column — it acts on the note.
                                transport_button(
                    transport,
                    loc.msg("editor-copy"),
                    loc.msg("editor-copy-tooltip"),
                    "\u{25C8}",
                    style,
                    colors.btn_bg,
                    |_: On<Activate>,
                     state: Res<EditorState>,
                     mut clipboard: ResMut<super::clipboard::NoteClipboard>| {
                        if !state.selected.is_empty() {
                            *clipboard = state.copy_selection();
                        }
                    },
                );
                                transport_button(
                    transport,
                    loc.msg("editor-paste"),
                    loc.msg("editor-paste-tooltip"),
                    "\u{21B4}",
                    style,
                    colors.btn_bg,
                    |_: On<Activate>,
                     mut state: ResMut<EditorState>,
                     clipboard: Res<super::clipboard::NoteClipboard>| {
                        if clipboard.is_empty() {
                            return;
                        }
                        let tick = state.scroll_beat * super::TICKS_PER_BEAT;
                        state.paste(&clipboard, tick);
                    },
                );

                                // Edit-mode tools that act on the *timeline*, not a note —
                                // the note's own buttons are the right-hand column below.
                                transport
                                    .spawn((
                                        EditModeGroup,
                                        Node {
                                            width: Val::Percent(100.0),
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::Stretch,
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
                                        timeline_tool_button(
                                            g,
                                            TimelineToolButton(TimelineTool::Select),
                                            loc.msg("editor-tool-select"),
                                            loc.msg("editor-tool-select-tooltip"),
                                            "\u{25FB}",
                                            style,
                                            colors,
                                        );
                                        timeline_tool_button(
                                            g,
                                            TimelineToolButton(TimelineTool::Erase),
                                            loc.msg("editor-tool-erase"),
                                            loc.msg("editor-tool-erase-tooltip"),
                                            "\u{25AD}",
                                            style,
                                            colors,
                                        );
                                        timeline_tool_button(
                                            g,
                                            TimelineToolButton(TimelineTool::Remove),
                                            loc.msg("editor-tool-remove"),
                                            loc.msg("editor-tool-remove-tooltip"),
                                            "\u{25FC}",
                                            style,
                                            colors,
                                        );
                                        timeline_tool_button(
                                            g,
                                            TimelineToolButton(TimelineTool::Tempo),
                                            loc.msg("editor-tool-tempo"),
                                            loc.msg("editor-tool-tempo-tooltip"),
                                            "\u{2669}",
                                            style,
                                            colors,
                                        );
                                        timeline_tool_button(
                                            g,
                                            TimelineToolButton(TimelineTool::Meter),
                                            loc.msg("editor-tool-meter"),
                                            loc.msg("editor-tool-meter-tooltip"),
                                            "\u{2016}",
                                            style,
                                            colors,
                                        );
                                        // Actions on the Select tool's span, not tools
                                        // of their own: the selection stays, so a
                                        // second press counts another pass.
                                        transport_button(
                                            g,
                                            loc.msg("editor-tool-repeat"),
                                            loc.msg("editor-tool-repeat-tooltip"),
                                            "\u{2016}:",
                                            style,
                                            colors.btn_bg,
                                            super::timeline::on_repeat_button,
                                        );
                                        transport_button(
                                            g,
                                            loc.msg("editor-tool-ending"),
                                            loc.msg("editor-tool-ending-tooltip"),
                                            "1.",
                                            style,
                                            colors.btn_bg,
                                            super::timeline::on_ending_button,
                                        );
                                    });

                                // The metronome click, shared with gameplay/the Bending
                                // Trainer via the same `MetronomeMuted` global (see
                                // `metronome`'s module doc) — clicks during Record/Play/
                                // Practice, dimmed here while muted rather than a
                                // live-swapped label, same visual language as Undo/Redo.
                                transport_button(
                            transport,
                            loc.msg("editor-metronome"),
                            loc.msg("editor-metronome-tooltip"),
                            "\u{1F514}",
                            style,
                            colors.btn_bg,
                            |_: On<Activate>,
                             mut muted: ResMut<
                                harmonicon_gameplay::gameplay::metronome_overlay::MetronomeMuted,
                            >| {
                                muted.0 = !muted.0;
                            },
                        )
                        .insert(super::ui::MetronomeToggleButton);

                                // Toggles the meta form's third (color-legend) column — see
                                // `meta_form::update_legend_visibility`.
                                transport_button(
                                    transport,
                                    loc.msg("editor-legend-toggle"),
                                    loc.msg("editor-legend-toggle-tooltip"),
                                    "\u{2139}",
                                    style,
                                    colors.btn_bg,
                                    |_: On<Activate>, mut state: ResMut<EditorState>| {
                                        state.legend_visible = !state.legend_visible;
                                    },
                                );

                                // Dev-only ("--features dev") benchmark ground-truth mode —
                                // see `expected_notes`'s own module docs.
                                #[cfg(feature = "dev")]
                                super::expected_notes::spawn_expected_notes_mode_button(
                                    transport, loc, colors, style,
                                );

                                panel_separator(transport);

                                spawn_file_buttons(transport, loc, colors, style);

                                // Dev-only debugging aid — see `debug_record`'s own module
                                // docs. Deliberately in this always-visible strip, not a
                                // mode-specific group: it needs to stay checkable (and its
                                // one shared checkbox/status-label entity needs to exist
                                // exactly once) regardless of whether the mic tap it arms
                                // ends up gated on `RecordState::active` or
                                // `PracticeState::active` — see `sync_raw_capture`.
                                #[cfg(feature = "dev")]
                                super::debug_record::spawn_debug_recording_controls(
                                    transport, loc, colors, style,
                                );
                            });

                        let mut record_group_ec = panel.spawn((
                            RecordModeGroup,
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Stretch,
                                row_gap: Val::Px(6.0),
                                display: if mode == Mode::Record {
                                    Display::Flex
                                } else {
                                    Display::None
                                },
                                ..default()
                            },
                        ));
                        // Captured so the combobox below can use it as its own trigger
                        // parent — `combobox::spawn_combobox` needs a concrete `Entity` up
                        // front, and this row (unlike `EditorRoot`) is spawned fresh right
                        // here, so there's nothing to query for.
                        let record_group_id = record_group_ec.id();
                        record_group_ec.with_children(|g| {
                            spawn_record_buttons(g, loc, colors, style);

                            // Detect algorithm: same shared combobox (and global
                            // `AudioSettings::pitch_algorithm`) as Options/Bending Trainer —
                            // picking one here takes effect immediately, including for a
                            // take already in progress, since recording reads pitches off
                            // the same continuously-running mic pipeline every other mode
                            // does (see `record.rs`'s module docs).
                            let algo_combo = combobox::spawn_combobox(
                                g.commands_mut(),
                                record_group_id,
                                editor_root,
                                &loc.msg("editor-record-detect-label"),
                                &algo_labels(loc),
                                algorithm.label(),
                                on_algo_selected,
                            );
                            attach_algo_tooltip(g.commands_mut(), algo_combo, algorithm);
                        });

                        panel
                            .spawn((
                                PlayModeGroup,
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Stretch,
                                    row_gap: Val::Px(6.0),
                                    display: if mode == Mode::Play {
                                        Display::Flex
                                    } else {
                                        Display::None
                                    },
                                    ..default()
                                },
                            ))
                            .with_children(|g| {
                                spawn_playback_buttons(g, loc, colors, style);
                            });

                        // Dev-only ("--features dev") benchmark ground-truth mode — see
                        // `expected_notes`'s own module docs.
                        #[cfg(feature = "dev")]
                        super::expected_notes::spawn_expected_notes_group(
                            panel, loc, colors, mode, style,
                        );
                    });

                // ── Right column: the note ───────────────────────────────
                columns
                    .spawn((
                        NoteColumn,
                        Node {
                            width: Val::Px(width_without),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(6.0)),
                            display: if mode == Mode::Edit { Display::Flex } else { Display::None },
                            ..default()
                        },
                    ))
                    .with_children(|g| {
                        spawn_note_column_buttons(g, loc, colors, style);
                    });
            });
    });
}

/// The note column's buttons — see [`NoteColumn`] for what it is and why
/// it's a column of its own. Order follows what a player reaches for most:
/// breath, then pitch technique, then expression and its depth, then the
/// phrase, then delete.
fn spawn_note_column_buttons(
    g: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    style: ActionButtonStyle,
) {
    mod_button(
        g,
        ModButton::Blow,
        loc.msg("mod-blow"),
        loc.msg("mod-blow-tooltip"),
        "\u{2191}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Draw,
        loc.msg("mod-draw"),
        loc.msg("mod-draw-tooltip"),
        "\u{2193}",
        style,
        colors,
    );
    panel_separator(g);
    mod_button(
        g,
        ModButton::Bend,
        loc.msg("mod-bend"),
        loc.msg("mod-bend-tooltip"),
        "\u{007E}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Overblow,
        loc.msg("mod-overblow"),
        loc.msg("mod-overblow-tooltip"),
        "\u{21C8}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Overdraw,
        loc.msg("mod-overdraw"),
        loc.msg("mod-overdraw-tooltip"),
        "\u{21CA}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Slide,
        loc.msg("mod-slide"),
        loc.msg("mod-slide-tooltip"),
        "\u{2194}",
        style,
        colors,
    );
    panel_separator(g);
    mod_button(
        g,
        ModButton::Wah,
        loc.msg("mod-wah"),
        loc.msg("mod-wah-tooltip"),
        "\u{2248}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Vibrato,
        loc.msg("mod-vibrato"),
        loc.msg("mod-vibrato-tooltip"),
        "\u{2195}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Depth,
        loc.msg("mod-depth"),
        loc.msg("mod-depth-tooltip"),
        "\u{25D0}",
        style,
        colors,
    );
    panel_separator(g);
    mod_button(
        g,
        ModButton::Call,
        loc.msg("mod-call"),
        loc.msg("mod-call-tooltip"),
        "\u{21A9}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Split,
        loc.msg("mod-split"),
        loc.msg("mod-split-tooltip"),
        "TB",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::Phrase,
        loc.msg("mod-phrase"),
        loc.msg("mod-phrase-tooltip"),
        "\u{00A7}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::TransposeUp,
        loc.msg("mod-transpose-up"),
        loc.msg("mod-transpose-up-tooltip"),
        "\u{266F}",
        style,
        colors,
    );
    mod_button(
        g,
        ModButton::TransposeDown,
        loc.msg("mod-transpose-down"),
        loc.msg("mod-transpose-down-tooltip"),
        "\u{266D}",
        style,
        colors,
    );
    g.spawn(Node { flex_grow: 1.0, ..default() });
    mod_button(
        g,
        ModButton::Delete,
        loc.msg("mod-delete"),
        loc.msg("mod-delete-tooltip"),
        "\u{25CB}",
        style,
        colors,
    );
}
