// SPDX-License-Identifier: MIT

//! Editor file, playback, practice, and recording controls, built with
//! `panel_widgets`' shared button helpers.

use bevy::audio::AudioSource;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::harpchart::safe_path_segment;
use super::panel_widgets::transport_button;
use super::playback::{EditorAudio, Playhead, start_playback, toggle_pause};
use super::practice::{PracticeState, start_practice, stop_practice};
use super::record::{RecordState, pause_record, stop_record};
use super::state::{ContentKind, EditorState};
use super::{LOAD_PURPOSE, SAVE_PURPOSE};
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::PitchRange;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::ActionButtonStyle;
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::file_dialog::{DialogMode, OpenFileDialog};

/// Where a file dialog for authored `kind` content (`"songs"`, `"lessons"`)
/// opens: the `~/Harmonicon/<kind>` drop folder, which the game watches
/// live, so a chart saved there is playable at once. The home directory when
/// that folder doesn't exist; the game ships no songs or lessons of its own
/// to start from.
fn drop_folder(kind: &str) -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let folder = home.join("Harmonicon").join(kind);
    Some(if folder.is_dir() { folder } else { home })
}

/// Chart file I/O — always visible, in both Edit and Perform mode. Save/Load
/// both branch on `state.content_kind` for the dialog's title/extension/
/// default name/start dir (`.harpchart` under `~/Harmonicon/songs`, vs.
/// `.json` under `~/Harmonicon/lessons`) — which actual file gets written/read from the
/// chosen path is decided separately, by whichever of `harpchart::
/// handle_save_chosen`/`lesson_form::handle_save_lesson_chosen` (and their
/// load siblings) matches that same `content_kind`.
pub(super) fn spawn_file_buttons(
    panel: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    style: ActionButtonStyle,
) {
    transport_button(
        panel,
        loc.msg("editor-save"),
        loc.msg("editor-save-tooltip"),
        "\u{1F4BE}",
        style,
        colors.transport_save,
        |_: On<Activate>,
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
                        start_dir: drop_folder("songs"),
                        mode: DialogMode::Save { default_name },
                    }
                }
                ContentKind::Lesson => OpenFileDialog {
                    purpose: SAVE_PURPOSE,
                    title: String::from(loc.msg("dialog-save-lesson")),
                    extensions: vec!["json".into()],
                    start_dir: drop_folder("lessons"),
                    mode: DialogMode::Save { default_name: "lesson.json".into() },
                },
            });
        },
    );
    transport_button(
        panel,
        loc.msg("editor-load"),
        loc.msg("editor-load-tooltip"),
        "\u{1F4C2}",
        style,
        colors.transport_load,
        |_: On<Activate>,
         state: Res<EditorState>,
         loc: Res<Localization>,
         mut open: MessageWriter<OpenFileDialog>| {
            open.write(match state.content_kind {
                ContentKind::Song => OpenFileDialog {
                    purpose: LOAD_PURPOSE,
                    title: String::from(loc.msg("dialog-load-chart")),
                    extensions: vec!["harpchart".into()],
                    start_dir: drop_folder("songs"),
                    mode: DialogMode::Open,
                },
                ContentKind::Lesson => OpenFileDialog {
                    purpose: LOAD_PURPOSE,
                    title: String::from(loc.msg("dialog-load-lesson")),
                    extensions: vec!["json".into()],
                    start_dir: drop_folder("lessons"),
                    mode: DialogMode::Open,
                },
            });
        },
    );
}

/// Play/Pause/Stop/Practice — only shown in [`Mode::Play`] (wrapped in
/// [`PlayModeGroup`] by the caller).
pub(super) fn spawn_playback_buttons(
    panel: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    style: ActionButtonStyle,
) {
    transport_button(
        panel,
        loc.msg("editor-play"),
        loc.msg("editor-play-tooltip"),
        "\u{25B6}",
        style,
        colors.transport_play,
        |_: On<Activate>,
         mut state: ResMut<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         sinks: Query<&AudioSink, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut count_in: ResMut<super::metronome::CountIn>,
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
            stop_record(
                &mut state,
                &playing,
                &mut record,
                &mut playhead,
                &mut pitch_range,
                &mut count_in,
                &mut commands,
            );
            start_playback(&state, &mut sources, &settings, &playing, &mut playhead, &mut commands);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        loc.msg("editor-pause-tooltip"),
        "\u{23F8}",
        style,
        colors.transport_pause,
        |_: On<Activate>,
         mut playhead: ResMut<Playhead>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            toggle_pause(&mut playhead, &sinks);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-stop"),
        loc.msg("editor-stop-tooltip"),
        "\u{25A0}",
        style,
        colors.transport_stop,
        |_: On<Activate>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut count_in: ResMut<super::metronome::CountIn>,
         mut commands: Commands| {
            stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            stop_record(
                &mut state,
                &playing,
                &mut record,
                &mut playhead,
                &mut pitch_range,
                &mut count_in,
                &mut commands,
            );
        },
    );
    transport_button(
        panel,
        loc.msg("editor-practice"),
        loc.msg("editor-practice-tooltip"),
        "\u{1F3A4}",
        style,
        colors.transport_practice,
        |_: On<Activate>,
         mut state: ResMut<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut count_in: ResMut<super::metronome::CountIn>,
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
                stop_record(
                    &mut state,
                    &playing,
                    &mut record,
                    &mut playhead,
                    &mut pitch_range,
                    &mut count_in,
                    &mut commands,
                );
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
pub(super) fn spawn_record_buttons(
    panel: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    style: ActionButtonStyle,
) {
    transport_button(
        panel,
        loc.msg("editor-play"),
        loc.msg("editor-record-play-tooltip"),
        "\u{25B6}",
        style,
        colors.transport_record,
        |_: On<Activate>,
         state: Res<EditorState>,
         mut practice: ResMut<PracticeState>,
         record: Res<RecordState>,
         mut playhead: ResMut<Playhead>,
         sinks: Query<&AudioSink, With<EditorAudio>>,
         mut count_in: ResMut<super::metronome::CountIn>| {
            if count_in.active() {
                // Already counting in — a second click shouldn't restart it.
                return;
            }
            if record.active {
                // Paused, not stopped: resume in place rather than
                // restarting the take (no count-in — the player is picking
                // straight back up, not starting fresh).
                if playhead.paused {
                    toggle_pause(&mut playhead, &sinks);
                }
                return;
            }
            practice.reset();
            super::metronome::begin_count_in(&state, playhead.elapsed, &mut count_in);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        loc.msg("editor-pause-tooltip"),
        "\u{23F8}",
        style,
        colors.transport_pause,
        |_: On<Activate>,
         mut state: ResMut<EditorState>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut count_in: ResMut<super::metronome::CountIn>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            if count_in.active() {
                // Nothing has actually started yet — cancel outright rather
                // than "pausing" a take that was never recording anything.
                count_in.stop();
                return;
            }
            if !record.active {
                return;
            }
            if playhead.paused {
                // Toggle back out of pause — same resume the Play button does.
                toggle_pause(&mut playhead, &sinks);
            } else {
                pause_record(&mut state, &mut record, &mut playhead, &sinks);
            }
        },
    );
    transport_button(
        panel,
        loc.msg("editor-stop"),
        loc.msg("editor-record-stop-tooltip"),
        "\u{25A0}",
        style,
        colors.transport_stop,
        |_: On<Activate>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<EditorAudio>>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut count_in: ResMut<super::metronome::CountIn>,
         mut commands: Commands| {
            stop_record(
                &mut state,
                &playing,
                &mut record,
                &mut playhead,
                &mut pitch_range,
                &mut count_in,
                &mut commands,
            );
        },
    );
    transport_button(
        panel,
        loc.msg("editor-finish"),
        loc.msg("editor-finish-tooltip"),
        "\u{23F9}",
        style,
        colors.transport_stop,
        |_: On<Activate>,
         mut state: ResMut<EditorState>,
         playing: Query<Entity, With<EditorAudio>>,
         mut record: ResMut<RecordState>,
         mut playhead: ResMut<Playhead>,
         mut pitch_range: ResMut<PitchRange>,
         mut count_in: ResMut<super::metronome::CountIn>,
         mut commands: Commands| {
            stop_record(
                &mut state,
                &playing,
                &mut record,
                &mut playhead,
                &mut pitch_range,
                &mut count_in,
                &mut commands,
            );
            playhead.elapsed = 0.0;
        },
    );
}
