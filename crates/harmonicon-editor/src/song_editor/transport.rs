// SPDX-License-Identifier: MIT

//! Editor file, playback, practice, and recording controls, built with
//! `panel_widgets`' shared button helpers.

use bevy::audio::AudioSource;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::harpchart::safe_path_segment;
use super::metronome::{CountIn, begin_count_in};
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
        |_: On<Activate>, mut t: Transport| {
            // Paused, not stopped: resume in place rather than restarting.
            if t.playhead.playing && t.playhead.paused {
                t.toggle_pause();
                return;
            }
            t.practice.reset(); // exit practice mode before starting preview playback
            // A recording in progress owns the shared `Playhead` clock —
            // close it out (rather than letting `start_playback` below
            // silently repurpose it out from under `record.open`) before
            // taking over.
            t.stop_record();
            t.start_playback();
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        loc.msg("editor-pause-tooltip"),
        "\u{23F8}",
        style,
        colors.transport_pause,
        |_: On<Activate>, mut t: Transport| t.toggle_pause(),
    );
    transport_button(
        panel,
        loc.msg("editor-stop"),
        loc.msg("editor-stop-tooltip"),
        "\u{25A0}",
        style,
        colors.transport_stop,
        |_: On<Activate>, mut t: Transport| t.stop_all(),
    );
    transport_button(
        panel,
        loc.msg("editor-practice"),
        loc.msg("editor-practice-tooltip"),
        "\u{1F3A4}",
        style,
        colors.transport_practice,
        |_: On<Activate>, mut t: Transport| {
            // Paused, not stopped: resume in place rather than stopping.
            if t.practice.active && t.playhead.paused {
                t.toggle_pause();
            } else if t.practice.active {
                t.stop_practice();
            } else {
                // A recording in progress owns the shared `Playhead` clock —
                // close it out before `start_practice` below repurposes it.
                t.stop_record();
                t.start_practice();
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
        |_: On<Activate>, mut t: Transport| {
            if t.count_in.active() {
                // Already counting in — a second click shouldn't restart it.
                return;
            }
            if t.record.active {
                // Paused, not stopped: resume in place rather than
                // restarting the take (no count-in — the player is picking
                // straight back up, not starting fresh).
                if t.playhead.paused {
                    t.toggle_pause();
                }
                return;
            }
            t.practice.reset();
            begin_count_in(&t.state, t.playhead.elapsed, &mut t.count_in);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        loc.msg("editor-pause-tooltip"),
        "\u{23F8}",
        style,
        colors.transport_pause,
        |_: On<Activate>, mut t: Transport| {
            if t.count_in.active() {
                // Nothing has actually started yet — cancel outright rather
                // than "pausing" a take that was never recording anything.
                t.count_in.stop();
            } else if t.record.active && t.playhead.paused {
                // Toggle back out of pause — same resume the Play button does.
                t.toggle_pause();
            } else if t.record.active {
                t.pause_record();
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
        |_: On<Activate>, mut t: Transport| t.stop_record(),
    );
    transport_button(
        panel,
        loc.msg("editor-finish"),
        loc.msg("editor-finish-tooltip"),
        "\u{23F9}",
        style,
        colors.transport_stop,
        |_: On<Activate>, mut t: Transport| {
            t.stop_record();
            t.playhead.elapsed = 0.0;
        },
    );
}

/// Everything the editor's start/stop/pause helpers touch, so a transport
/// or mode button takes one parameter instead of a dozen. Each method is a
/// thin call into the owning module's free function, which stays the unit
/// under test.
#[derive(SystemParam)]
pub(super) struct Transport<'w, 's> {
    pub state: ResMut<'w, EditorState>,
    pub practice: ResMut<'w, PracticeState>,
    pub record: ResMut<'w, RecordState>,
    pub playhead: ResMut<'w, Playhead>,
    pub count_in: ResMut<'w, CountIn>,
    pitch_range: ResMut<'w, PitchRange>,
    sources: ResMut<'w, Assets<AudioSource>>,
    settings: Res<'w, AudioSettings>,
    loc: Res<'w, Localization>,
    playing: Query<'w, 's, Entity, With<EditorAudio>>,
    sinks: Query<'w, 's, &'static AudioSink, With<EditorAudio>>,
    commands: Commands<'w, 's>,
}

impl Transport<'_, '_> {
    pub fn toggle_pause(&mut self) {
        toggle_pause(&mut self.playhead, &self.sinks);
    }

    pub fn start_playback(&mut self) {
        start_playback(
            &self.state,
            &mut self.sources,
            &self.settings,
            &self.playing,
            &mut self.playhead,
            &mut self.commands,
        );
    }

    pub fn start_practice(&mut self) {
        start_practice(
            &self.state,
            &mut self.sources,
            &self.settings,
            &self.playing,
            &mut self.practice,
            &mut self.playhead,
            &mut self.commands,
            &self.loc,
        );
    }

    pub fn stop_practice(&mut self) {
        stop_practice(&self.playing, &mut self.practice, &mut self.playhead, &mut self.commands);
    }

    pub fn pause_record(&mut self) {
        pause_record(&mut self.state, &mut self.record, &mut self.playhead, &self.sinks);
    }

    pub fn stop_record(&mut self) {
        stop_record(
            &mut self.state,
            &self.playing,
            &mut self.record,
            &mut self.playhead,
            &mut self.pitch_range,
            &mut self.count_in,
            &mut self.commands,
        );
    }

    /// Stops whatever is running — practice, a take, or a pending count-in.
    pub fn stop_all(&mut self) {
        self.stop_practice();
        self.stop_record();
    }
}
