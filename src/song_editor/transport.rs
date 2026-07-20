// SPDX-License-Identifier: MIT

//! The mod panel's transport clusters: chart file I/O (Save/Load/Import,
//! always visible), the Play-mode playback/practice buttons, and the
//! Record-mode recording transport — split out of `mod_panel` (which
//! assembles the panel and owns the Edit-mode tool strip) purely along the
//! "button cluster" seam. Built from `panel_widgets`' shared shapes.

use bevy::audio::AudioSource;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::harpchart::safe_path_segment;
use super::panel_widgets::transport_button;
use super::playback::{EditorAudio, Playhead, start_playback, toggle_pause};
use super::practice::{PracticeState, start_practice, stop_practice};
use super::record::{RecordState, start_record, stop_record};
use super::state::{ContentKind, EditorState};
use super::{LOAD_PURPOSE, SAVE_PURPOSE};
use crate::audio_system::pitch_detect::PitchRange;
use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::localization::LocalizationExt;
use crate::settings::AudioSettings;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

/// Chart file I/O — always visible, in both Edit and Perform mode. Save/Load
/// both branch on `state.content_kind` for the dialog's title/extension/
/// default name/start dir (`.harpchart` under `assets/songs`, vs. `.json`
/// under `assets/lessons`) — which actual file gets written/read from the
/// chosen path is decided separately, by whichever of `harpchart::
/// handle_save_chosen`/`lesson_form::handle_save_lesson_chosen` (and their
/// load siblings) matches that same `content_kind`.
pub(super) fn spawn_file_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
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

/// Play/Pause/Stop/Practice — only shown in [`Mode::Play`] (wrapped in
/// [`PlayModeGroup`] by the caller).
pub(super) fn spawn_playback_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
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
}

/// Play/Pause/Stop/Finish — the recording transport, only shown in
/// [`Mode::Record`] (wrapped in [`RecordModeGroup`] by the caller). Play
/// starts a take (or resumes a paused one); Pause freezes it in place;
/// Stop ends the take leaving the playhead where it stopped; Finish ends
/// it and rewinds to the beginning.
pub(super) fn spawn_record_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    transport_button(
        panel,
        loc.msg("editor-play"),
        loc.msg("editor-record-play-tooltip"),
        colors.transport_record,
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         sinks: Query<&AudioSink, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut commands: Commands| {
            if record.active {
                // Paused, not stopped: resume in place rather than
                // restarting the take.
                if playhead.paused {
                    toggle_pause(&mut playhead, &sinks);
                }
                return;
            }
            practice.reset();
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
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        loc.msg("editor-pause-tooltip"),
        colors.transport_pause,
        |_: On<Pointer<Click>>,
         record: Res<RecordState>,
         mut playhead: ResMut<Playhead>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            if record.active {
                toggle_pause(&mut playhead, &sinks);
            }
        },
    );
    transport_button(
        panel,
        loc.msg("editor-stop"),
        loc.msg("editor-record-stop-tooltip"),
        colors.transport_stop,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<EditorAudio>>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut commands: Commands| {
            stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-finish"),
        loc.msg("editor-finish-tooltip"),
        colors.transport_stop,
        |_: On<Pointer<Click>>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<EditorAudio>>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut commands: Commands| {
            stop_record(&mut state, &playing, &mut record, &mut playhead, &mut pitch_range, &mut commands);
            playhead.elapsed = 0.0;
        },
    );
}
