// SPDX-License-Identifier: MIT

//! Song authoring in a note grid (`AppState::SongEditor2`).
//!
//! Layout, left to right:
//!   * a fixed column of the ten harmonica holes (number + hole box), and
//!   * an infinite, horizontally-scrollable beat grid to its right.
//!
//! ```text
//!     _  |1 &|2 &|3 &|4 &|1 &|2 &|...
//!  01 |□| ____________________________
//!  ..
//!  10 |□| ____________________________
//! ```

use bevy::prelude::*;

use harmonicon_app::app::AppState;
use harmonicon_app::app::tour_active;
use harmonicon_platform::theme::LoadedTheme;

mod annotation_lane;
mod audition;
mod clipboard;
// Dev-only ("--features dev") debugging aid — see its own module docs.
// Compiled only under that feature, same as `Cargo.toml`'s `dev` feature
// gates dynamic linking and asset hot-reload elsewhere in the app.
#[cfg(feature = "dev")]
mod debug_record;
mod details_fields;
// Dev-only ("--features dev") benchmark-authoring workflow — see its own
// module docs. Same gating as `debug_record` above.
#[cfg(feature = "dev")]
mod expected_notes;
mod grid;
mod grid_cache;
mod harpchart;
mod interaction;
mod legend;
mod lesson_form;
mod loop_settings;
mod material;
mod meta_form;
mod metadata_sync;
mod metronome;
mod midi_import;
mod mod_panel;
mod music_score_bridge;
mod note_model;
mod panel;
mod panel_widgets;
mod phrase_editor;
mod pitch_map;
mod ranges;
mod repeat_marks;
mod save_feedback;
mod scoring_settings;
mod scroll;
mod selected_metadata;
// Shared synth primitives now live in `harmonicon_core::synth`; playback
// remains visible within this crate for editor modules.
pub(crate) mod playback;
mod practice;
mod record;
mod state;
mod timeline;
mod timeline_overlay;
mod transport;
mod transpose;
mod ui;
mod undo;
mod view_scroll;
mod waveform;

// ── Dialog purposes ───────────────────────────────────────────────────────────

use harmonicon_ui::dialogs::file_dialog::DialogId;

const SAVE_PURPOSE: DialogId = DialogId("song_editor_2_save");
const LOAD_PURPOSE: DialogId = DialogId("song_editor_2_load");
const MUSIC_PURPOSE: DialogId = DialogId("song_editor_2_music");
const MIDI_PURPOSE: DialogId = DialogId("song_editor_2_midi");

// ── Geometry ──────────────────────────────────────────────────────────────────

const HOLE_COL_W: f32 = 78.0;
// Beat/bar numbers occupy the first 20 px. Phrase markers get their own lane
// below them, then the waveform occupies the rest.
const ANNOTATION_TOP: f32 = 20.0;
const ANNOTATION_H: f32 = 18.0;
const WAVEFORM_TOP: f32 = ANNOTATION_TOP + ANNOTATION_H + 4.0;
const WAVEFORM_H: f32 = 36.0;
const HEADER_H: f32 = WAVEFORM_TOP + WAVEFORM_H + 4.0;
const ROW_H: f32 = 34.0;
const BEAT_W: f32 = 60.0;
const NOTE_PAD: f32 = 4.0;
// Diameter of the selected note's two resize grips (`ui::ResizeGrip`). They
// sit *outside* the note rather than inside it, so this is a free choice —
// it doesn't have to fit within the shortest note the grid can draw, and a
// 16th note (13px) couldn't have hosted two grab targets and a strip to
// drag the note by.
const GRIP_D: f32 = 12.0;
// Shared tick-grid vocabulary for this module's grid and UI math.
pub(crate) use harmonicon_core::snap;
pub(crate) use harmonicon_core::synth::TICKS_PER_BEAT;
const TICK_W: f32 = BEAT_W / TICKS_PER_BEAT as f32;
// The silence track: a summary row below the hole lanes showing the gap, in
// seconds, between consecutive notes — see `ranges::silence_gaps`. Shorter
// than an ordinary hole lane since it's read-only display, not an editable
// row of its own.
const SILENCE_ROW_H: f32 = 24.0;

fn grid_height(hole_count: u8) -> f32 {
    HEADER_H + ROW_H * hole_count as f32 + SILENCE_ROW_H
}

/// Top of the silence track, inside `GridContent`'s coordinate space —
/// directly below the last hole lane.
fn silence_row_top(hole_count: u8) -> f32 {
    HEADER_H + ROW_H * hole_count as f32
}

// ── Colours ───────────────────────────────────────────────────────────────────
//
// The editor's palette lives in the active theme (`harmonicon_platform::theme::LoadedTheme`,
// `theme.song_editor_colors()`) rather than as consts here, so a theme's
// `theme.json` can override it under `"colors": { "song_editor": { ... } }`.
// See `harmonicon_platform::theme::SongEditorColors` for the fields and their defaults.

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct SongEditor2Plugin;

impl Plugin for SongEditor2Plugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "dev")]
        app.add_plugins((debug_record::DebugRecordPlugin, expected_notes::ExpectedNotesPlugin));

        app.add_plugins(material::EditorNoteMaterialPlugin)
            .add_systems(
                OnEnter(AppState::SongEditor2),
                (ui::init_state, ui::setup, ui::force_grid_rebuild).chain(),
            )
            .add_systems(OnExit(AppState::SongEditor2), ui::cleanup)
            .init_resource::<state::Scroll>()
            .init_resource::<state::ToolbarScroll>()
            .init_resource::<practice::PracticeState>()
            .init_resource::<record::RecordState>()
            .init_resource::<state::TimelineSelection>()
            .init_resource::<playback::PendingMusicSeek>()
            .init_resource::<waveform::MusicWaveform>()
            .init_resource::<grid_cache::GridCache>()
            .init_resource::<clipboard::NoteClipboard>()
            .init_resource::<metronome::CountIn>()
            .init_resource::<metronome::EditorLastClickedTick>()
            .init_resource::<audition::LastAuditioned>()
            .init_resource::<save_feedback::SaveFeedback>()
            .add_systems(
                Update,
                (
                    (
                        playback::finish_pending_playback,
                        playback::advance_playhead,
                        view_scroll::auto_scroll,
                        view_scroll::pan_keys,
                        view_scroll::pan_wheel,
                        view_scroll::pan_touch,
                        view_scroll::wheel_toolbar,
                        view_scroll::apply_scroll,
                        view_scroll::apply_toolbar_scroll,
                        view_scroll::update_grid_scrollbar,
                        view_scroll::update_scrollbar_markers
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        scroll::update_editor_scrollbar_visibility,
                        ui::rebuild_grid_on_resize,
                        ui::sync_chrome_height
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        ui::sync_hole_column
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        // Must run before `rebuild_grid` so a music-path
                        // change (which also mutates `EditorState`, e.g. via
                        // the meta form's Browse button) is decoded and
                        // ready the same frame the grid rebuilds around it,
                        // not one frame late.
                        waveform::sync_music_waveform,
                        grid::rebuild_grid.run_if(
                            resource_exists_and_changed::<state::EditorState>
                                .or_else(resource_changed::<waveform::MusicWaveform>)
                                .or_else(resource_changed::<LoadedTheme>)
                                .or_else(resource_changed::<grid_cache::GridCache>),
                        ),
                    )
                        .chain(),
                    (
                        grid::update_selection.after(grid::rebuild_grid).run_if(
                            resource_exists_and_changed::<state::EditorState>
                                .or_else(resource_changed::<LoadedTheme>)
                                .or_else(resource_changed::<grid_cache::GridCache>),
                        ),
                        playback::apply_pending_music_seek,
                    ),
                    playback::update_playhead_view.after(playback::advance_playhead),
                    playback::update_progress_bar.after(playback::advance_playhead),
                    music_score_bridge::sync_music_score
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    music_score_bridge::sync_music_score_playhead.after(playback::advance_playhead),
                    // Practice/record/metronome ticks run after the playhead
                    // advances so `elapsed` is current.
                    (
                        practice::practice_tick.after(playback::advance_playhead),
                        record::record_tick.after(playback::advance_playhead),
                        metronome::sync_tempo
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        metronome::click_metronome.after(playback::advance_playhead),
                        metronome::tick_count_in.after(playback::advance_playhead),
                        metronome::finish_count_in.after(metronome::tick_count_in),
                        audition::audition_on_select
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        save_feedback::tick_save_feedback,
                        transpose::report_transpose,
                        interaction::report_technique_skips,
                    ),
                    // Suspended while the guided tour is showing this
                    // screen — Esc/Delete/Ctrl+C/Ctrl+V shouldn't act on it
                    // out from under the tour (see `menu::tutorial`).
                    (
                        interaction::grid_keys.run_if(not(tour_active)),
                        interaction::handle_copy_paste.run_if(not(tour_active)),
                        interaction::handle_undo_redo.run_if(not(tour_active)),
                        undo::track_changes,
                    ),
                    (
                        interaction::live_resize,
                        interaction::update_move_ghost,
                        interaction::update_group_move_ghosts,
                        interaction::update_resize_grips,
                    ),
                    panel::update_mod_panel.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<LoadedTheme>),
                    ),
                    (
                        panel::update_mode_buttons.run_if(
                            resource_exists_and_changed::<state::EditorState>
                                .or_else(resource_changed::<LoadedTheme>),
                        ),
                        panel::update_undo_redo_buttons.run_if(
                            resource_exists_and_changed::<undo::UndoHistory>
                                .or_else(resource_changed::<LoadedTheme>),
                        ),
                        panel::update_metronome_toggle_button.run_if(
                            resource_changed::<
                                harmonicon_gameplay::gameplay::metronome_overlay::MetronomeMuted,
                            >
                                .or_else(resource_changed::<LoadedTheme>),
                        ),
                    ),
                    (panel::update_mode_visibility, panel::update_note_column)
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    legend::update_legend_visibility
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    panel::update_technique_button_visibility
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    panel::update_meta_fields.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<LoadedTheme>),
                    ),
                    panel::sync_meta_field_text
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    (
                        panel::update_harmonica_kind_text,
                        panel::update_content_kind_text,
                        panel::update_snap_mode_text,
                    )
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    (
                        lesson_form::update_lesson_form_visibility,
                        lesson_form::update_lesson_details_visibility,
                        lesson_form::update_lesson_conditional_rows,
                    )
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    panel::update_status_bar.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<practice::PracticeState>)
                            .or_else(resource_changed::<record::RecordState>)
                            .or_else(resource_changed::<metronome::CountIn>)
                            .or_else(resource_changed::<save_feedback::SaveFeedback>),
                    ),
                    (
                        harpchart::handle_save_chosen,
                        harpchart::handle_load_chosen,
                        harpchart::handle_music_chosen,
                        lesson_form::handle_save_lesson_chosen,
                        lesson_form::handle_load_lesson_chosen,
                    ),
                )
                    .run_if(in_state(AppState::SongEditor2)),
            )
            .add_systems(
                Update,
                (
                    (midi_import::handle_midi_chosen, midi_import::rebuild_midi_track_combobox)
                        .chain(),
                    meta_form::spawn_scale_combobox,
                    meta_form::spawn_time_signature_combobox,
                    phrase_editor::populate_phrase_editor,
                    phrase_editor::update_phrase_editor,
                    meta_form::sync_scale_combobox_value,
                    meta_form::sync_time_signature_combobox_value
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    timeline::sync_timeline_surface,
                    timeline::sync_selection_with_scroll
                        .before(timeline_overlay::update_timeline_overlays),
                    timeline_overlay::update_timeline_overlays,
                    timeline::handle_timeline_confirm,
                    panel::update_timeline_tool_buttons
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                )
                    .run_if(in_state(AppState::SongEditor2)),
            )
            .add_message::<midi_import::MidiFileLoaded>();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
