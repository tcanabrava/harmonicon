// SPDX-License-Identifier: MIT

//! Song authoring tool #2: a DAW-style note grid (`AppState::SongEditor2`).
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

use crate::app::AppState;
use crate::menu::tutorial::tour_active;
use crate::theme::LoadedTheme;

mod grid;
mod harpchart;
mod interaction;
mod lesson_form;
mod material;
mod meta_form;
mod midi_import;
mod mod_panel;
mod panel;
mod panel_widgets;
mod pitch_map;
mod ranges;
mod scroll;
// `pub(crate)`, not private like its neighbours: `gameplay::call_response`
// shares this module's synth (`PhraseNote`/`render_pcm`/`encode_wav`) for
// the call-and-response lesson feature's audio cue.
pub(crate) mod playback;
mod practice;
mod record;
mod state;
mod timeline;
mod timeline_overlay;
mod transport;
mod ui;
mod waveform;

// ── Dialog purposes ───────────────────────────────────────────────────────────

use crate::dialogs::file_dialog::DialogId;

const SAVE_PURPOSE: DialogId = DialogId("song_editor_2_save");
const LOAD_PURPOSE: DialogId = DialogId("song_editor_2_load");
const MUSIC_PURPOSE: DialogId = DialogId("song_editor_2_music");
const MIDI_PURPOSE: DialogId = DialogId("song_editor_2_midi");

// ── Geometry ──────────────────────────────────────────────────────────────────

const HOLE_COL_W: f32 = 78.0;
// Beat/bar number labels occupy the top of the header (unchanged, `top: 6.0`
// in `grid.rs`); the waveform reference strip (`waveform::MusicWaveform`)
// occupies the rest of it, so growing `HEADER_H` alone is all that's needed
// to make room for it — every other module that reads `HEADER_H` (as "where
// hole row 1 starts") adjusts automatically.
const WAVEFORM_TOP: f32 = 20.0;
const WAVEFORM_H: f32 = 36.0;
const HEADER_H: f32 = WAVEFORM_TOP + WAVEFORM_H + 4.0;
const ROW_H: f32 = 34.0;
const BEAT_W: f32 = 60.0;
const BEATS_PER_BAR: usize = 4;
const NOTE_PAD: f32 = 4.0;
const HANDLE_W: f32 = 8.0;
// Defined in `audio_system::synth` (shared tick-grid vocabulary: the same
// resolution `gameplay::call_response` uses to convert chart-time call
// phrases into the ticks `render_pcm` expects); re-exported here under its
// established name for this module's own grid/UI math.
pub(crate) use crate::audio_system::synth::TICKS_PER_BEAT;
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
// The editor's palette lives in the active theme (`crate::theme::LoadedTheme`,
// `theme.song_editor_colors()`) rather than as consts here, so a theme's
// `theme.json` can override it under `"colors": { "song_editor": { ... } }`.
// See `crate::theme::SongEditorColors` for the fields and their defaults.

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct SongEditor2Plugin;

impl Plugin for SongEditor2Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(material::EditorNoteMaterialPlugin)
            .add_systems(
                OnEnter(AppState::SongEditor2),
                (ui::init_state, ui::setup, ui::force_grid_rebuild).chain(),
            )
            .add_systems(OnExit(AppState::SongEditor2), ui::cleanup)
            .init_resource::<state::Scroll>()
            .init_resource::<practice::PracticeState>()
            .init_resource::<record::RecordState>()
            .init_resource::<state::TimelineSelection>()
            .init_resource::<playback::PendingMusicSeek>()
            .init_resource::<waveform::MusicWaveform>()
            .add_systems(
                Update,
                (
                    (
                        playback::advance_playhead,
                        interaction::auto_scroll,
                        interaction::pan_keys,
                        interaction::pan_wheel,
                        interaction::apply_scroll,
                        interaction::update_grid_scrollbar,
                        interaction::update_scrollbar_markers
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
                        waveform::sync_music_waveform
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        grid::rebuild_grid
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                    )
                        .chain(),
                    playback::apply_pending_music_seek,
                    playback::update_playhead_view.after(playback::advance_playhead),
                    playback::update_progress_bar.after(playback::advance_playhead),
                    // Practice/record ticks run after the playhead advances so `elapsed` is current.
                    practice::practice_tick.after(playback::advance_playhead),
                    record::record_tick.after(playback::advance_playhead),
                    // Suspended while the guided tour is showing this
                    // screen — Esc/Delete shouldn't act on it out from
                    // under the tour (see `menu::tutorial`).
                    interaction::grid_keys.run_if(not(tour_active)),
                    interaction::type_into_field,
                    (
                        interaction::live_resize,
                        interaction::update_move_ghost,
                        interaction::update_group_move_ghosts,
                    ),
                    panel::update_mod_panel.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<LoadedTheme>),
                    ),
                    panel::update_mode_buttons.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<LoadedTheme>),
                    ),
                    panel::update_mode_visibility
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    panel::update_technique_button_visibility
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    panel::update_meta_fields.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<LoadedTheme>),
                    ),
                    panel::update_harmonica_kind_text
                        .run_if(resource_exists_and_changed::<state::EditorState>),
                    panel::update_content_kind_text
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
                            .or_else(resource_changed::<record::RecordState>),
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
                    (
                        midi_import::handle_midi_chosen,
                        midi_import::rebuild_midi_track_combobox,
                    )
                        .chain(),
                    meta_form::spawn_scale_combobox,
                    meta_form::sync_scale_combobox_value
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
