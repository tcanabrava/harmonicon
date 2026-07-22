// SPDX-License-Identifier: MIT

//! The mod panel's two-strip assembly: a short, fixed global-transport strip
//! (Back / Edit / Perform / Lock / Save / Load — always the same regardless
//! of mode), then a `flex_wrap: Wrap` contextual tool strip below it (the
//! current mode's whole tool palette). See [`spawn_mod_panel`]'s doc comment
//! for why it's two stacked rows rather than one ever-growing row. Built
//! from the reusable button shapes in `super::panel_widgets`.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::AppState;
use super::panel_widgets::{
    mod_button, mode_button, panel_separator, timeline_tool_button, transport_button,
};
use super::playback::{EditorAudio, Playhead};
use super::practice::{PracticeState, stop_practice};
use super::record::{RecordState, stop_record};
use super::state::{EditorState, Mode, TimelineTool};
use super::transport::{spawn_file_buttons, spawn_playback_buttons, spawn_record_buttons};
use super::ui::{
    EditModeGroup, ModButton, ModeButton, PlayModeGroup, RecordModeGroup, TimelineToolButton,
};
use crate::audio_system::pitch_detect::{PitchAlgorithm, PitchRange};
use crate::dialogs::algo_picker::{algo_labels, on_algo_selected, spawn_algo_explanation};
use crate::dialogs::combobox;
use crate::localization::LocalizationExt;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

/// The mod panel: a short, fixed global-transport strip (Back / Edit /
/// Perform / Lock / Save / Load — always the same regardless of mode), then
/// a `flex_wrap: Wrap` contextual tool strip below it (the current mode's
/// whole tool palette — up to 13 buttons + 3 separators in Edit mode). Two
/// stacked rows rather than one ever-growing row, so a narrow/small window
/// wraps the tool strip onto a second line instead of rendering buttons past
/// the right edge with no way to reach them. The panel's own height is
/// therefore auto (driven by its two rows' content) rather than the fixed
/// `Val::Px(52.0)` a single non-wrapping row could get away with.
pub(super) fn spawn_mod_panel(
    root: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    mode: Mode,
    editor_root: Entity,
    algorithm: PitchAlgorithm,
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
                    colors,
                    |_: On<Pointer<Click>>,
                     mut state: ResMut<EditorState>,
                     playing: Query<Entity, With<EditorAudio>>,
                     mut practice: ResMut<PracticeState>,
                     mut record: ResMut<RecordState>,
                     mut playhead: ResMut<Playhead>,
                     mut pitch_range: ResMut<PitchRange>,
                     mut commands: Commands| {
                        state.mode = Mode::Edit;
                        stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
                        stop_record(
                            &mut state,
                            &playing,
                            &mut record,
                            &mut playhead,
                            &mut pitch_range,
                            &mut commands,
                        );
                    },
                );
                mode_button(
                    transport,
                    ModeButton::Record,
                    loc.msg("editor-mode-record"),
                    loc.msg("editor-mode-record-tooltip"),
                    colors,
                    |_: On<Pointer<Click>>,
                     mut state: ResMut<EditorState>,
                     playing: Query<Entity, With<EditorAudio>>,
                     mut practice: ResMut<PracticeState>,
                     mut playhead: ResMut<Playhead>,
                     mut commands: Commands| {
                        state.mode = Mode::Record;
                        // A recording can only have been started from this
                        // mode itself, so only Play-mode playback/practice
                        // needs stopping here.
                        stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
                    },
                );
                mode_button(
                    transport,
                    ModeButton::Play,
                    loc.msg("editor-mode-play"),
                    loc.msg("editor-mode-play-tooltip"),
                    colors,
                    |_: On<Pointer<Click>>,
                     mut state: ResMut<EditorState>,
                     playing: Query<Entity, With<EditorAudio>>,
                     mut record: ResMut<RecordState>,
                     mut playhead: ResMut<Playhead>,
                     mut pitch_range: ResMut<PitchRange>,
                     mut commands: Commands| {
                        state.mode = Mode::Play;
                        stop_record(
                            &mut state,
                            &playing,
                            &mut record,
                            &mut playhead,
                            &mut pitch_range,
                            &mut commands,
                        );
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
                timeline_tool_button(
                    g,
                    TimelineToolButton(TimelineTool::Tempo),
                    loc.msg("editor-tool-tempo"),
                    loc.msg("editor-tool-tempo-tooltip"),
                    colors,
                );
            });

        let mut record_group_ec = panel.spawn((
            RecordModeGroup,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
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
            spawn_record_buttons(g, loc, colors);

            // Detect algorithm: same shared combobox (and global
            // `AudioSettings::pitch_algorithm`) as Options/Bending Trainer —
            // picking one here takes effect immediately, including for a
            // take already in progress, since recording reads pitches off
            // the same continuously-running mic pipeline every other mode
            // does (see `record.rs`'s module docs).
            combobox::spawn_combobox(
                g.commands_mut(),
                record_group_id,
                editor_root,
                &loc.msg("editor-record-detect-label"),
                &algo_labels(),
                algorithm.label(),
                on_algo_selected,
            );
            spawn_algo_explanation(g.commands_mut(), record_group_id, 380.0, algorithm);

            // Dev-only debugging aid — see `debug_record`'s own module docs.
            #[cfg(feature = "dev")]
            super::debug_record::spawn_debug_recording_controls(g, loc, colors);
        });

        panel
            .spawn((
                PlayModeGroup,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
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
                spawn_playback_buttons(g, loc, colors);
            });
    });
}
