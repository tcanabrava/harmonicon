// SPDX-License-Identifier: MIT

//! The mod panel's two-strip assembly: a short, fixed global-transport strip
//! (Back / Edit / Perform / Lock / Save / Load — always the same regardless
//! of mode), then a `flex_wrap: Wrap` contextual tool strip below it (the
//! current mode's whole tool palette). See [`spawn_mod_panel`]'s doc comment
//! for why it's two stacked rows rather than one ever-growing row. Built
//! from the reusable button shapes in `super::panel_widgets`.

use bevy::audio::AudioSource;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::harpchart::safe_path_segment;
use super::panel_widgets::{
    mod_button, mode_button, panel_separator, spawn_record_button, timeline_tool_button,
    transport_button,
};
use super::playback::{EditorAudio, Playhead, start_playback, toggle_pause};
use super::practice::{PracticeState, start_practice, stop_practice};
use super::record::{RecordState, start_record, stop_record};
use super::state::{ContentKind, EditorState, Mode, TimelineTool};
use super::ui::{EditModeGroup, ModButton, ModeButton, PerformModeGroup, TimelineToolButton};
use super::{AppState, LOAD_PURPOSE, SAVE_PURPOSE};
use crate::audio_system::pitch_detect::PitchRange;
use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::localization::LocalizationExt;
use crate::settings::AudioSettings;
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
                     mut pitch_range: ResMut<PitchRange>,
                     mut commands: Commands| {
                        state.mode = Mode::Edit;
                        // Leaving Perform mode hides Play/Pause/Stop/Practice/
                        // Record, so nothing would be left to stop anything
                        // that's running.
                        stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
                        stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
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
                timeline_tool_button(
                    g,
                    TimelineToolButton(TimelineTool::Tempo),
                    loc.msg("editor-tool-tempo"),
                    loc.msg("editor-tool-tempo-tooltip"),
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

/// Chart file I/O — always visible, in both Edit and Perform mode. Save/Load
/// both branch on `state.content_kind` for the dialog's title/extension/
/// default name/start dir (`.harpchart` under `assets/songs`, vs. `.json`
/// under `assets/lessons`) — which actual file gets written/read from the
/// chosen path is decided separately, by whichever of `harpchart::
/// handle_save_chosen`/`lesson_form::handle_save_lesson_chosen` (and their
/// load siblings) matches that same `content_kind`.
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
            open.write(match state.content_kind {
                ContentKind::Song => {
                    let default_name = format!(
                        "{}.harpchart",
                        safe_path_segment(if state.name.is_empty() {
                            "chart"
                        } else {
                            &state.name
                        })
                    );
                    OpenFileDialog {
                        purpose: SAVE_PURPOSE,
                        title: String::from(loc.msg("dialog-save-chart")),
                        extensions: vec!["harpchart".into()],
                        start_dir: Some(std::path::PathBuf::from("assets/songs")),
                        mode: DialogMode::Save { default_name },
                    }
                }
                ContentKind::Lesson => OpenFileDialog {
                    purpose: SAVE_PURPOSE,
                    title: String::from(loc.msg("dialog-save-lesson")),
                    extensions: vec!["json".into()],
                    start_dir: Some(std::path::PathBuf::from("assets/lessons")),
                    mode: DialogMode::Save {
                        default_name: "lesson.json".into(),
                    },
                },
            });
        },
    );
    transport_button(
        panel,
        loc.msg("editor-load"),
        loc.msg("editor-load-tooltip"),
        colors.transport_load,
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         loc: Res<Localization>,
         mut open: MessageWriter<OpenFileDialog>| {
            open.write(match state.content_kind {
                ContentKind::Song => OpenFileDialog {
                    purpose: LOAD_PURPOSE,
                    title: String::from(loc.msg("dialog-load-chart")),
                    extensions: vec!["harpchart".into()],
                    start_dir: Some(std::path::PathBuf::from("assets/songs")),
                    mode: DialogMode::Open,
                },
                ContentKind::Lesson => OpenFileDialog {
                    purpose: LOAD_PURPOSE,
                    title: String::from(loc.msg("dialog-load-lesson")),
                    extensions: vec!["json".into()],
                    start_dir: Some(std::path::PathBuf::from("assets/lessons")),
                    mode: DialogMode::Open,
                },
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
         mut pitch_range: ResMut<PitchRange>,
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
            stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
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
         mut pitch_range: ResMut<PitchRange>,
         mut commands: Commands| {
            stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
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
         mut pitch_range: ResMut<PitchRange>,
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
                stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
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
         mut pitch_range: ResMut<PitchRange>,
         mut commands: Commands| {
            if record.active {
                stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
            } else {
                practice.reset(); // exit practice mode before recording, same as Play does
                start_record(
                    &state,
                    &mut sources,
                    &settings,
                    &playing,
                    &mut record,
                    &mut playhead,
                    &mut pitch_range,
                    &mut commands,
                );
            }
        },
    );
}
