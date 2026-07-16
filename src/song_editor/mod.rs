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

use crate::menu::AppState;
use crate::menu::tutorial::tour_active;
use crate::theme::LoadedTheme;

mod grid;
mod harpchart;
mod interaction;
mod material;
mod midi_import;
mod panel;
// `pub(crate)`, not private like its neighbours: `gameplay::call_response`
// shares this module's synth (`PhraseNote`/`render_pcm`/`encode_wav`) for
// the call-and-response lesson feature's audio cue.
pub(crate) mod playback;
mod practice;
mod state;
mod timeline;
mod ui;

// ── Dialog purposes ───────────────────────────────────────────────────────────

use crate::dialogs::file_dialog::DialogId;

const SAVE_PURPOSE: DialogId = DialogId("song_editor_2_save");
const LOAD_PURPOSE: DialogId = DialogId("song_editor_2_load");
const MUSIC_PURPOSE: DialogId = DialogId("song_editor_2_music");
const MIDI_PURPOSE: DialogId = DialogId("song_editor_2_midi");

// ── Geometry ──────────────────────────────────────────────────────────────────

const HOLE_COL_W: f32 = 78.0;
const HEADER_H: f32 = 30.0;
const ROW_H: f32 = 34.0;
const BEAT_W: f32 = 60.0;
const BEATS_PER_BAR: usize = 4;
const NOTE_PAD: f32 = 4.0;
const HANDLE_W: f32 = 8.0;
// `pub(crate)`, not private like its neighbours here: `gameplay::
// call_response` needs the same tick grid to convert chart-time call
// phrases into the ticks `playback::render_pcm` expects.
pub(crate) const TICKS_PER_BEAT: usize = 4;
const TICK_W: f32 = BEAT_W / TICKS_PER_BEAT as f32;

fn grid_height(hole_count: u8) -> f32 {
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
            .add_systems(
                Update,
                (
                    (
                        playback::advance_playhead,
                        interaction::auto_scroll,
                        interaction::pan_keys,
                        interaction::pan_wheel,
                        interaction::apply_scroll,
                        ui::rebuild_grid_on_resize,
                        ui::sync_chrome_height
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        ui::sync_hole_column
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                        grid::rebuild_grid
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                    )
                        .chain(),
                    playback::update_playhead_view.after(playback::advance_playhead),
                    playback::update_progress_bar.after(playback::advance_playhead),
                    // Practice tick runs after the playhead advances so `elapsed` is current.
                    practice::practice_tick.after(playback::advance_playhead),
                    // Suspended while the guided tour is showing this
                    // screen — Esc/Delete shouldn't act on it out from
                    // under the tour (see `menu::tutorial`).
                    interaction::grid_keys.run_if(not(tour_active)),
                    interaction::type_into_field,
                    interaction::live_resize,
                    interaction::update_move_ghost,
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
                    panel::update_status_bar.run_if(
                        resource_exists_and_changed::<state::EditorState>
                            .or_else(resource_changed::<practice::PracticeState>),
                    ),
                    harpchart::handle_save_chosen,
                    harpchart::handle_load_chosen,
                    harpchart::handle_music_chosen,
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
                    timeline::update_timeline_overlays,
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
mod tests {
    use super::grid::{mix_srgba, note_in_scale, visible_beats};
    use super::harpchart::{
        load_harpchart, parse_pitch_expr, safe_path_segment, serialize_harpchart,
    };
    use super::interaction::{apply_modifier, select_or_add};
    use super::timeline::{TimelineSurfaceGeometry, drag_end_tick};
    use super::playback::{
        PhraseNote, SAMPLE_RATE, build_harp, encode_wav, envelope, note_freq, render_pcm,
    };
    use super::state::Scroll;
    use super::state::{
        Dir, Edge, EditorState, Expr, GridNote, HarmonicaKind, Pitch, Side, TimelineTool,
        apply_resize, can_place, enforce_direction, enforce_expr, erase_range, move_target,
        normalize_range, note_rect, remove_range, song_end_tick, split_side_range,
    };
    use super::ui::ModButton;
    use super::{BEAT_W, HEADER_H, HOLE_COL_W, NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};
    use crate::song::harmonica::blues_scale_classes;

    #[test]
    fn click_adds_then_selects_without_duplicating() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
        let added = s.notes[0];
        assert_eq!(s.selected, Some(added.id));
        assert_eq!((added.hole, added.tick, added.len), (4, 2, TICKS_PER_BEAT));
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.selected, Some(added.id));
    }

    #[test]
    fn bend_cycles_and_caps_at_hole_max() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 1, 0);
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(1.0));
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes[0].pitch, Pitch::Normal);
    }

    #[test]
    fn unbendable_hole_ignores_bend() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 5, 0);
        let hole5 = s.notes[0].id;
        select_or_add(&mut s, 7, 0);
        s.selected = Some(hole5);
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(
            s.notes.iter().find(|n| n.hole == 5).unwrap().pitch,
            Pitch::Bend(0.5)
        );
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(
            s.notes.iter().find(|n| n.hole == 5).unwrap().pitch,
            Pitch::Normal
        );
    }

    #[test]
    fn pitch_and_expression_stack() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Bend);
        apply_modifier(&mut s, ModButton::Vibrato);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        assert_eq!(
            s.notes[0].expr,
            Expr::Vibrato(3.0),
            "first click lands on the min rate"
        );
        apply_modifier(&mut s, ModButton::Wah);
        assert_eq!(
            s.notes[0].expr,
            Expr::Wah(2.0),
            "first click lands on the min rate"
        );
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
    }

    #[test]
    fn vibrato_cycles_through_rates_and_caps_at_none() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 1, 0);
        for expected in [3.0, 4.0, 5.0, 6.0, 7.0] {
            apply_modifier(&mut s, ModButton::Vibrato);
            assert_eq!(s.notes[0].expr, Expr::Vibrato(expected));
        }
        apply_modifier(&mut s, ModButton::Vibrato);
        assert_eq!(
            s.notes[0].expr,
            Expr::None,
            "cycling past the max rate deselects"
        );
    }

    #[test]
    fn wah_cycles_through_rates_and_caps_at_none() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 1, 0);
        for expected in [2.0, 3.0, 4.0, 5.0] {
            apply_modifier(&mut s, ModButton::Wah);
            assert_eq!(s.notes[0].expr, Expr::Wah(expected));
        }
        apply_modifier(&mut s, ModButton::Wah);
        assert_eq!(
            s.notes[0].expr,
            Expr::None,
            "cycling past the max rate deselects"
        );
    }

    #[test]
    fn overblow_only_on_low_holes() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 8, 0);
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes[0].pitch, Pitch::Normal);
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(
            s.notes.iter().find(|n| n.hole == 3).unwrap().pitch,
            Pitch::Overblow
        );
    }

    #[test]
    fn slide_cycles_on_and_off_on_any_hole() {
        let mut s = EditorState {
            harmonica_kind: HarmonicaKind::Chromatic,
            ..Default::default()
        };
        select_or_add(&mut s, 11, 0); // valid on a 12-hole chromatic harp
        apply_modifier(&mut s, ModButton::Slide);
        assert_eq!(s.notes[0].pitch, Pitch::Slide);
        apply_modifier(&mut s, ModButton::Slide);
        assert_eq!(s.notes[0].pitch, Pitch::Normal);
    }

    // ── HarmonicaKind switching ──────────────────────────────────────────────

    #[test]
    fn hole_count_matches_the_harmonica_kind() {
        let mut s = EditorState::default();
        assert_eq!(s.hole_count(), 10);
        s.set_harmonica_kind(HarmonicaKind::Chromatic);
        assert_eq!(s.hole_count(), 12);
    }

    #[test]
    fn switching_to_diatonic_drops_notes_beyond_hole_ten_and_clears_slide() {
        let mut s = EditorState {
            harmonica_kind: HarmonicaKind::Chromatic,
            ..Default::default()
        };
        select_or_add(&mut s, 11, 0);
        apply_modifier(&mut s, ModButton::Slide);
        select_or_add(&mut s, 3, 4);
        apply_modifier(&mut s, ModButton::Slide);

        s.set_harmonica_kind(HarmonicaKind::Diatonic);

        assert_eq!(s.notes.len(), 1, "the hole-11 note doesn't fit anymore");
        assert_eq!(
            s.notes[0].pitch,
            Pitch::Normal,
            "slide isn't a valid diatonic technique"
        );
    }

    #[test]
    fn switching_to_chromatic_clears_diatonic_only_techniques() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Overblow);

        s.set_harmonica_kind(HarmonicaKind::Chromatic);

        assert_eq!(s.notes[0].pitch, Pitch::Normal);
    }

    #[test]
    fn switching_kind_deselects_a_note_that_got_dropped() {
        let mut s = EditorState {
            harmonica_kind: HarmonicaKind::Chromatic,
            ..Default::default()
        };
        select_or_add(&mut s, 11, 0);
        assert!(s.selected.is_some());

        s.set_harmonica_kind(HarmonicaKind::Diatonic);

        assert_eq!(s.selected, None);
    }

    #[test]
    fn blow_draw_toggles_independently_of_techniques() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        assert_eq!(s.notes[0].dir, Dir::Blow);
        apply_modifier(&mut s, ModButton::Bend);
        apply_modifier(&mut s, ModButton::Draw);
        assert_eq!(s.notes[0].dir, Dir::Draw);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        apply_modifier(&mut s, ModButton::Blow);
        assert_eq!(s.notes[0].dir, Dir::Blow);
    }

    #[test]
    fn delete_removes_selected() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 1);
        apply_modifier(&mut s, ModButton::Delete);
        assert!(s.notes.is_empty());
        assert_eq!(s.selected, None);
    }

    #[test]
    fn clicking_a_covered_beat_selects_rather_than_stacks() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 4, 0);
        let id = s.notes[0].id;
        s.notes[0].len = 3;
        select_or_add(&mut s, 4, 2);
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.selected, Some(id));
    }

    #[test]
    fn new_note_adopts_direction_sounding_at_that_beat() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        apply_modifier(&mut s, ModButton::Draw);
        select_or_add(&mut s, 5, 0);
        assert_eq!(s.note_at(5, 0).unwrap().dir, Dir::Draw);
    }

    #[test]
    fn setting_direction_propagates_to_simultaneous_notes() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        select_or_add(&mut s, 5, 0);
        s.selected = Some(s.note_at(2, 0).unwrap().id);
        apply_modifier(&mut s, ModButton::Draw);
        assert_eq!(s.note_at(2, 0).unwrap().dir, Dir::Draw);
        assert_eq!(s.note_at(5, 0).unwrap().dir, Dir::Draw);
    }

    #[test]
    fn enforce_unifies_overlap_chain_but_not_independent_notes() {
        let mut s = EditorState {
            notes: vec![
                GridNote {
                    id: 0,
                    hole: 1,
                    tick: 0,
                    len: 3,
                    dir: Dir::Blow,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
                GridNote {
                    id: 1,
                    hole: 2,
                    tick: 2,
                    len: 3,
                    dir: Dir::Draw,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
                GridNote {
                    id: 2,
                    hole: 3,
                    tick: 10,
                    len: 1,
                    dir: Dir::Draw,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
            ],
            next_id: 3,
            ..Default::default()
        };
        enforce_direction(&mut s, 0);
        assert_eq!(s.note_by_id(1).unwrap().dir, Dir::Blow);
        assert_eq!(s.note_by_id(2).unwrap().dir, Dir::Draw);
    }

    // Wah (hand cupping) and vibrato (breath vibrato) are whole-player
    // techniques: every hole sounding at the same instant must share the
    // same one, mirroring how Blow/Draw is already unified above.
    #[test]
    fn enforce_expr_unifies_overlap_chain_but_not_independent_notes() {
        let mut s = EditorState {
            notes: vec![
                GridNote {
                    id: 0,
                    hole: 1,
                    tick: 0,
                    len: 3,
                    dir: Dir::Blow,
                    pitch: Pitch::Normal,
                    expr: Expr::Vibrato(5.0),
                },
                GridNote {
                    id: 1,
                    hole: 2,
                    tick: 2,
                    len: 3,
                    dir: Dir::Draw,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
                GridNote {
                    id: 2,
                    hole: 3,
                    tick: 10,
                    len: 1,
                    dir: Dir::Draw,
                    pitch: Pitch::Normal,
                    expr: Expr::None,
                },
            ],
            next_id: 3,
            ..Default::default()
        };
        enforce_expr(&mut s, 0);
        assert_eq!(
            s.note_by_id(1).unwrap().expr,
            Expr::Vibrato(5.0),
            "overlapping note shares the vibrato (rate included)"
        );
        assert_eq!(
            s.note_by_id(2).unwrap().expr,
            Expr::None,
            "independent note is untouched"
        );
    }

    #[test]
    fn clicking_wah_propagates_to_overlapping_notes_via_apply_modifier() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        select_or_add(&mut s, 5, 2); // overlaps the first note (tick 0..4 vs 2..6)
        select_or_add(&mut s, 7, 10); // independent
        s.selected = Some(s.note_at(2, 0).unwrap().id);
        apply_modifier(&mut s, ModButton::Wah);
        assert_eq!(s.note_at(2, 0).unwrap().expr, Expr::Wah(2.0));
        assert_eq!(
            s.note_at(5, 2).unwrap().expr,
            Expr::Wah(2.0),
            "overlapping note picks up the wah too"
        );
        assert_eq!(
            s.note_at(7, 10).unwrap().expr,
            Expr::None,
            "independent note keeps its own expression"
        );
    }

    #[test]
    fn separate_times_keep_independent_directions() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        select_or_add(&mut s, 2, 4);
        s.selected = Some(s.note_at(2, 4).unwrap().id);
        apply_modifier(&mut s, ModButton::Draw);
        assert_eq!(s.note_at(2, 0).unwrap().dir, Dir::Blow);
        assert_eq!(s.note_at(2, 4).unwrap().dir, Dir::Draw);
    }

    #[test]
    fn right_edge_resizes_length_and_clamps_to_one() {
        assert_eq!(apply_resize(4, 1, Edge::Right, 2, 0, None), (4, 3));
        assert_eq!(apply_resize(4, 3, Edge::Right, -1, 0, None), (4, 2));
        assert_eq!(apply_resize(4, 2, Edge::Right, -5, 0, None), (4, 1));
    }

    #[test]
    fn left_edge_moves_start_and_resizes_inversely() {
        assert_eq!(apply_resize(4, 3, Edge::Left, 1, 0, None), (5, 2));
        assert_eq!(apply_resize(4, 2, Edge::Left, -2, 0, None), (2, 4));
        assert_eq!(apply_resize(4, 2, Edge::Left, 9, 0, None), (5, 1));
        assert_eq!(apply_resize(1, 2, Edge::Left, -9, 0, None), (0, 3));
    }

    fn note(hole: u8, dir: Dir, pitch: Pitch) -> GridNote {
        GridNote {
            id: 0,
            hole,
            tick: 0,
            len: 4,
            dir,
            pitch,
            expr: Expr::None,
        }
    }

    #[test]
    fn note_freq_maps_holes_bends_and_key() {
        let c_harp = build_harp("C", HarmonicaKind::Diatonic);
        let c4 = note_freq(&note(1, Dir::Blow, Pitch::Normal), &c_harp).unwrap();
        assert!((c4 - 261.63).abs() < 0.5, "got {c4}");
        let bent = note_freq(&note(1, Dir::Blow, Pitch::Bend(1.0)), &c_harp).unwrap();
        assert!(bent < c4, "bend should drop pitch: {bent} !< {c4}");
        // G sits 7 semitones above C, but a real G Richter harp is a "low"
        // harp — its hole-1 blow is pitched *down* to G3 (a fourth below C4),
        // not up to G4 (a fifth above), so the octave-folded key offset is
        // -5, not +7. See `song::harmonica::key_offset`.
        let g_harp = build_harp("G", HarmonicaKind::Diatonic);
        let g = note_freq(&note(1, Dir::Blow, Pitch::Normal), &g_harp).unwrap();
        assert!(
            (g / c4 - 2f32.powf(-5.0 / 12.0)).abs() < 0.001,
            "G harp is the low harp — a fourth down, not a fifth up"
        );
        assert!(note_freq(&note(11, Dir::Blow, Pitch::Normal), &c_harp).is_none());
    }

    #[test]
    fn note_freq_resolves_overblow_and_overdraw_from_the_correct_reed() {
        // Regression: this used to take whichever table `note.dir` picked
        // (whatever the player happened to set the note's Blow/Draw arrow
        // to) and add a flat +1 semitone, rather than deriving the reed the
        // technique actually sounds from — wrong for the very common case of
        // an Overblow note left at its default `Dir::Blow`. Overblow (holes
        // 1/4/5/6) always sounds a semitone above the *draw* reed, and
        // Overdraw (holes 7-10) a semitone above the *blow* reed, regardless
        // of the note's own `dir` — see `song::harmonica::hole_notes`.
        let harp = build_harp("C", HarmonicaKind::Diatonic);

        // Hole 1: blow C4, draw D4 → overblow is D#4 (draw reed + 1), not
        // C#4 (blow reed + 1), even though the note is tagged `Dir::Blow`.
        let overblow = note_freq(&note(1, Dir::Blow, Pitch::Overblow), &harp).unwrap();
        let draw_reed = note_freq(&note(1, Dir::Draw, Pitch::Normal), &harp).unwrap();
        let semitone = 2f32.powf(1.0 / 12.0);
        assert!(
            (overblow / draw_reed - semitone).abs() < 0.001,
            "overblow should be a semitone above the draw reed"
        );

        // Hole 10: blow C7, draw A6 → overdraw is C#7 (blow reed + 1), even
        // though the note is tagged `Dir::Draw`.
        let overdraw = note_freq(&note(10, Dir::Draw, Pitch::Overdraw), &harp).unwrap();
        let blow_reed = note_freq(&note(10, Dir::Blow, Pitch::Normal), &harp).unwrap();
        assert!(
            (overdraw / blow_reed - semitone).abs() < 0.001,
            "overdraw should be a semitone above the blow reed"
        );
    }

    #[test]
    fn note_freq_reads_the_chromatic_layout_and_slide_table() {
        let harp = build_harp("C", HarmonicaKind::Chromatic);
        let c4 = note_freq(&note(1, Dir::Blow, Pitch::Normal), &harp).unwrap();
        assert!((c4 - 261.63).abs() < 0.5, "hole 1 blow is C4, got {c4}");
        let slid = note_freq(&note(1, Dir::Blow, Pitch::Slide), &harp).unwrap();
        assert!(slid > c4, "slide should raise pitch: {slid} !> {c4}");
        // Chromatic goes up to hole 12; hole 11 is out of range for diatonic
        // but valid here.
        assert!(note_freq(&note(11, Dir::Blow, Pitch::Normal), &harp).is_some());
    }

    #[test]
    fn render_and_wav_have_expected_size() {
        let notes = vec![note(4, Dir::Draw, Pitch::Normal)];
        let harp = build_harp("C", HarmonicaKind::Diatonic);
        let phrase: Vec<PhraseNote> = notes
            .iter()
            .map(|n| PhraseNote {
                tick: n.tick,
                len: n.len,
                freq: note_freq(n, &harp),
                expr: n.expr,
            })
            .collect();
        let pcm = render_pcm(&phrase, 120.0);
        let expected = ((0.5 + 0.25) * SAMPLE_RATE as f32).ceil() as usize;
        assert_eq!(pcm.len(), expected);
        assert!(
            pcm.iter().any(|&s| s.abs() > 0.01),
            "note should be audible"
        );
        let wav = encode_wav(&pcm, SAMPLE_RATE);
        assert_eq!(wav.len(), 44 + pcm.len() * 2);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn move_target_snaps_and_clamps() {
        assert_eq!(move_target(5, 4, 0.0, 0.0, 10), (5, 4));
        assert_eq!(move_target(5, 4, TICK_W, 2.0 * ROW_H, 10), (7, 5));
        assert_eq!(move_target(5, 4, BEAT_W, 0.0, 10), (5, 4 + TICKS_PER_BEAT));
        assert_eq!(move_target(1, 0, -5.0 * BEAT_W, -5.0 * ROW_H, 10), (1, 0));
        assert_eq!(move_target(10, 2, 0.0, 5.0 * ROW_H, 10), (10, 2));
    }

    #[test]
    fn move_target_clamps_to_a_chromatic_hole_count() {
        // A chromatic chart's 12 holes should let a note move past hole 10,
        // where a diatonic chart would clamp.
        assert_eq!(move_target(10, 0, 0.0, 2.0 * ROW_H, 12), (12, 0));
        assert_eq!(move_target(10, 0, 0.0, 5.0 * ROW_H, 12), (12, 0));
    }

    #[test]
    fn move_is_blocked_where_a_note_already_sits() {
        let notes = vec![
            GridNote {
                id: 0,
                hole: 3,
                tick: 0,
                len: 2,
                dir: Dir::Blow,
                pitch: Pitch::Normal,
                expr: Expr::None,
            },
            GridNote {
                id: 1,
                hole: 3,
                tick: 5,
                len: 1,
                dir: Dir::Blow,
                pitch: Pitch::Normal,
                expr: Expr::None,
            },
        ];
        assert!(!can_place(&notes, 1, 3, 1, 1));
        assert!(can_place(&notes, 1, 3, 2, 1));
        assert!(can_place(&notes, 1, 4, 0, 1));
    }

    #[test]
    fn resize_stops_at_neighbour_on_same_hole() {
        assert_eq!(apply_resize(0, 1, Edge::Right, 10, 0, Some(3)), (0, 3));
        assert_eq!(apply_resize(4, 2, Edge::Left, -10, 2, None), (2, 4));
    }

    #[test]
    fn serialize_harpchart_is_valid_json_with_required_fields() {
        let mut s = EditorState {
            name: "Test Song".into(),
            author: "Test Artist".into(),
            tempo: "120".into(),
            key: "G".into(),
            ..Default::default()
        };
        select_or_add(&mut s, 2, 0);
        select_or_add(&mut s, 4, 4);
        select_or_add(&mut s, 5, 4);
        apply_modifier(&mut s, ModButton::Vibrato);

        let json_str = serialize_harpchart(&s);
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");

        assert_eq!(v["song"]["title"], "Test Song");
        assert_eq!(v["song"]["artist"], "Test Artist");
        assert_eq!(v["timing"]["resolution"], TICKS_PER_BEAT as i64);

        let track = v["track"].as_array().expect("track array");
        assert_eq!(track.len(), 2, "one single + one chord phrase");

        let chord = track.iter().find(|p| p["tick"] == 4).expect("chord phrase");
        assert_eq!(chord["play_mode"], "chord");
        assert_eq!(chord["events"].as_array().unwrap().len(), 2);

        // Hole-2 blow is E4 on a C harp; key "G" is a low harp (see
        // `song::harmonica::key_offset`), transposing it down a fourth to B3.
        let single = &track[0];
        assert_eq!(single["events"][0]["note"], "B3");
    }

    #[test]
    fn serialize_harpchart_omits_audio_file_when_no_music_is_picked() {
        let mut s = EditorState {
            name: "Test Song".into(),
            key: "G".into(),
            ..Default::default()
        };
        select_or_add(&mut s, 2, 0);

        let v: serde_json::Value =
            serde_json::from_str(&serialize_harpchart(&s)).expect("valid JSON");
        assert!(
            v["metadata"].get("audio_file").is_none(),
            "an empty/never-picked audio file shouldn't be written at all, \
             not even as an empty string — it's optional in the schema"
        );
    }

    #[test]
    fn serialize_harpchart_writes_audio_file_once_music_is_picked() {
        let mut s = EditorState {
            name: "Test Song".into(),
            key: "G".into(),
            music: " music.ogg ".into(),
            ..Default::default()
        };
        select_or_add(&mut s, 2, 0);

        let v: serde_json::Value =
            serde_json::from_str(&serialize_harpchart(&s)).expect("valid JSON");
        assert_eq!(v["metadata"]["audio_file"], "music.ogg");
    }

    /// A chart the Song Editor writes must pass the exact schema
    /// `song::loader::SongChartLoader` validates against at load time — a
    /// field the editor writes but the schema doesn't declare (with
    /// `additionalProperties: false` at every level, an *undeclared* field
    /// fails validation outright, not just gets ignored) makes every song
    /// saved by the editor unplayable. Caught `metadata.audio_file` missing
    /// from the schema this way.
    #[test]
    fn serialize_harpchart_validates_against_the_song_schema() {
        let mut s = EditorState {
            name: "Test Song".into(),
            author: "Test Artist".into(),
            tempo: "120".into(),
            key: "G".into(),
            music: "music.ogg".into(),
            ..Default::default()
        };
        select_or_add(&mut s, 2, 0);

        let json_str = serialize_harpchart(&s);
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");

        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/song_schema.dtd.json"))
                .expect("schema is valid JSON");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| format!("  - {e} (at /{path})", path = e.instance_path))
            .collect();
        assert!(
            errors.is_empty(),
            "chart saved by the Song Editor must pass its own schema:\n{}",
            errors.join("\n")
        );
    }

    #[test]
    fn serialize_harpchart_writes_the_notes_own_oscillation_hz() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        apply_modifier(&mut s, ModButton::Vibrato); // -> 3.0
        apply_modifier(&mut s, ModButton::Vibrato); // -> 4.0
        apply_modifier(&mut s, ModButton::Vibrato); // -> 5.0

        let json_str = serialize_harpchart(&s);
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        let modifiers = v["track"][0]["events"][0]["modifiers"]
            .as_array()
            .expect("modifiers array");
        let vibrato = modifiers
            .iter()
            .find(|m| m["type"] == "vibrato")
            .expect("vibrato modifier");
        assert_eq!(vibrato["oscillation_hz"], 5.0);
    }

    #[test]
    fn oscillation_hz_round_trips_through_save_and_load() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Wah); // -> 2.0
        apply_modifier(&mut s, ModButton::Wah); // -> 3.0

        let json_str = serialize_harpchart(&s);
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");

        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&v, &mut loaded, &mut scroll);
        assert_eq!(loaded.notes[0].expr, Expr::Wah(3.0));
    }

    #[test]
    fn chromatic_chart_round_trips_kind_hole_count_and_slide() {
        let mut s = EditorState {
            harmonica_kind: HarmonicaKind::Chromatic,
            ..Default::default()
        };
        select_or_add(&mut s, 11, 0); // only valid on a chromatic (12-hole) harp
        apply_modifier(&mut s, ModButton::Slide);

        let json_str = serialize_harpchart(&s);
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(v["harmonica"]["type"], "chromatic");
        assert_eq!(v["harmonica"]["holes"], 12);
        assert_eq!(
            v["track"][0]["events"][0]["modifiers"][0]["type"],
            "slide"
        );

        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&v, &mut loaded, &mut scroll);
        assert_eq!(loaded.harmonica_kind, HarmonicaKind::Chromatic);
        assert_eq!(loaded.notes[0].hole, 11);
        assert_eq!(loaded.notes[0].pitch, Pitch::Slide);
    }

    #[test]
    fn loading_a_diatonic_chart_drops_holes_beyond_ten() {
        // A hand-edited or malformed chart claiming diatonic with an
        // out-of-range hole shouldn't produce an invalid GridNote.
        let v: serde_json::Value = serde_json::json!({
            "harmonica": { "type": "diatonic" },
            "track": [{
                "tick": 0,
                "duration": 0.5,
                "events": [{ "hole": 11, "action": "blow" }]
            }]
        });
        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&v, &mut loaded, &mut scroll);
        assert!(loaded.notes.is_empty());
    }

    #[test]
    fn saved_position_round_trips_through_load() {
        let s = EditorState {
            position: "3rd".into(),
            ..Default::default()
        };

        let json_str = serialize_harpchart(&s);
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(v["harmonica"]["position"], "3rd");

        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&v, &mut loaded, &mut scroll);
        assert_eq!(loaded.position, "3rd");
    }

    #[test]
    fn loading_an_unknown_position_keeps_the_default() {
        let v: serde_json::Value = serde_json::json!({
            "harmonica": { "position": "9th" }
        });
        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&v, &mut loaded, &mut scroll);
        assert_eq!(loaded.position, "2nd");
    }

    #[test]
    fn mix_srgba_interpolates_and_keeps_base_alpha() {
        let base = bevy::prelude::Color::srgba(0.0, 0.0, 0.0, 0.5);
        let tint = bevy::prelude::Color::srgba(1.0, 1.0, 1.0, 1.0);

        let none = mix_srgba(base, tint, 0.0).to_srgba();
        assert_eq!((none.red, none.green, none.blue), (0.0, 0.0, 0.0));
        assert_eq!(
            none.alpha, 0.5,
            "base's own alpha is preserved, not blended"
        );

        let full = mix_srgba(base, tint, 1.0).to_srgba();
        assert_eq!((full.red, full.green, full.blue), (1.0, 1.0, 1.0));
        assert_eq!(full.alpha, 0.5);

        let half = mix_srgba(base, tint, 0.5).to_srgba();
        assert!((half.red - 0.5).abs() < 1e-6);
    }

    #[test]
    fn note_in_scale_uses_the_bent_target_pitch_not_the_natural_one() {
        let scale = blues_scale_classes("C");
        let harp = build_harp("C", HarmonicaKind::Diatonic);

        // Draw-3 unbent is B4 (the major 7th) — outside the C blues scale.
        let natural = GridNote {
            id: 0,
            hole: 3,
            tick: 0,
            len: 1,
            dir: Dir::Draw,
            pitch: Pitch::Normal,
            expr: Expr::None,
        };
        assert!(
            !note_in_scale(&natural, &harp, &scale),
            "unbent B (major 7th) is outside the blues scale"
        );

        // Bending draw-3 down a step-and-a-half reaches Bb (the ♭7) — exactly
        // how a blues player accesses that blue note. Should read as in-scale.
        let bent = GridNote {
            id: 0,
            hole: 3,
            tick: 0,
            len: 1,
            dir: Dir::Draw,
            pitch: Pitch::Bend(1.5),
            expr: Expr::None,
        };
        assert!(
            note_in_scale(&bent, &harp, &scale),
            "bending down 1.5 steps reaches Bb, the b7 — in scale"
        );
    }

    // ── safe_path_segment ────────────────────────────────────────────────────────

    #[test]
    fn safe_path_segment_keeps_alphanumerics_and_hyphens() {
        assert_eq!(safe_path_segment("Windy-City Swing2"), "Windy-City_Swing2");
    }

    #[test]
    fn safe_path_segment_strips_traversal_and_separators() {
        // Every path separator/traversal character becomes an underscore, and
        // runs of them collapse rather than leaving "..", "/", or "\" intact.
        assert_eq!(safe_path_segment("../../etc/passwd"), "etc_passwd");
        assert_eq!(safe_path_segment("a/b\\c"), "a_b_c");
    }

    #[test]
    fn safe_path_segment_trims_and_collapses_whitespace_punctuation() {
        assert_eq!(safe_path_segment("  My Song!!  "), "My_Song");
    }

    #[test]
    fn safe_path_segment_of_all_punctuation_is_empty() {
        assert_eq!(safe_path_segment("###"), "");
        assert_eq!(safe_path_segment(""), "");
    }

    // ── parse_pitch_expr ──────────────────────────────────────────────────────────

    #[test]
    fn parse_pitch_expr_reads_bend_semitones_as_negative() {
        let mods = vec![serde_json::json!({ "type": "bend", "semitones": -1.5 })];
        let (pitch, expr) = parse_pitch_expr(&mods);
        assert_eq!(pitch, Pitch::Bend(1.5));
        assert_eq!(expr, Expr::None);
    }

    #[test]
    fn parse_pitch_expr_reads_overblow_overdraw_vibrato_wah() {
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "overblow" })]).0,
            Pitch::Overblow
        );
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "overdraw" })]).0,
            Pitch::Overdraw
        );
        // No `oscillation_hz` in the JSON (e.g. a chart saved before it was
        // per-note) falls back to the default rate.
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "vibrato" })]).1,
            Expr::Vibrato(5.5)
        );
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "wah-wah" })]).1,
            Expr::Wah(4.0)
        );
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "slide" })]).0,
            Pitch::Slide
        );
    }

    #[test]
    fn parse_pitch_expr_reads_custom_oscillation_hz() {
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "vibrato", "oscillation_hz": 6.0 })]).1,
            Expr::Vibrato(6.0)
        );
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "wah-wah", "oscillation_hz": 2.5 })]).1,
            Expr::Wah(2.5)
        );
    }

    #[test]
    fn parse_pitch_expr_clamps_a_nonpositive_oscillation_hz() {
        assert_eq!(
            parse_pitch_expr(&[serde_json::json!({ "type": "vibrato", "oscillation_hz": 0.0 })]).1,
            Expr::Vibrato(0.5)
        );
    }

    #[test]
    fn parse_pitch_expr_defaults_for_empty_or_unknown_modifiers() {
        assert_eq!(parse_pitch_expr(&[]), (Pitch::Normal, Expr::None));
        let unknown = vec![serde_json::json!({ "type": "flutter" })];
        assert_eq!(parse_pitch_expr(&unknown), (Pitch::Normal, Expr::None));
    }

    // ── note_rect ─────────────────────────────────────────────────────────────────

    #[test]
    fn note_rect_places_hole_one_tick_zero_at_the_grid_origin() {
        let note = GridNote {
            id: 0,
            hole: 1,
            tick: 0,
            len: 1,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        };
        let (left, top, width, height) = note_rect(&note);
        assert_eq!(left, 1.0);
        assert_eq!(top, HEADER_H + NOTE_PAD);
        assert_eq!(width, TICK_W - 2.0);
        assert_eq!(height, ROW_H - 2.0 * NOTE_PAD);
    }

    #[test]
    fn note_rect_advances_one_row_per_hole_and_scales_width_with_len() {
        let a = GridNote {
            id: 0,
            hole: 1,
            tick: 0,
            len: 3,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        };
        let b = GridNote {
            id: 1,
            hole: 2,
            tick: 0,
            len: 3,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        };
        let (_, top_a, width_a, _) = note_rect(&a);
        let (_, top_b, width_b, _) = note_rect(&b);
        assert_eq!(
            top_b - top_a,
            ROW_H,
            "hole 2 sits exactly one row below hole 1"
        );
        assert_eq!(width_a, width_b);
        assert_eq!(width_a, 3.0 * TICK_W - 2.0);
    }

    // ── visible_beats ─────────────────────────────────────────────────────────────

    #[test]
    fn visible_beats_covers_the_window_with_one_extra_partial_beat() {
        // Window exactly wide enough for 5 beats past the hole column still
        // gets a +1 so a partially-scrolled beat at the edge still renders.
        let win_w = HOLE_COL_W + 5.0 * BEAT_W;
        assert_eq!(visible_beats(win_w), 6);
    }

    #[test]
    fn visible_beats_rounds_up_a_partial_beat() {
        let win_w = HOLE_COL_W + 5.5 * BEAT_W;
        assert_eq!(visible_beats(win_w), 7);
    }

    #[test]
    fn visible_beats_never_goes_negative_for_a_narrow_window() {
        // Window narrower than the hole column alone: ceil() of a negative
        // fraction still produces a small, non-panicking usize.
        assert_eq!(visible_beats(HOLE_COL_W), 1);
    }

    // ── envelope ──────────────────────────────────────────────────────────────────

    #[test]
    fn envelope_starts_at_zero_and_stays_in_unit_range() {
        let dur = SAMPLE_RATE as usize; // 1 second, comfortably longer than attack+release
        for i in [0, 100, dur / 2, dur - 100, dur - 1] {
            let e = envelope(i, dur);
            assert!(
                (0.0..=1.0).contains(&e),
                "envelope({i}, {dur}) = {e} out of range"
            );
        }
        assert_eq!(envelope(0, dur), 0.0);
    }

    #[test]
    fn envelope_reaches_full_sustain_between_attack_and_release() {
        let dur = SAMPLE_RATE as usize;
        assert_eq!(envelope(dur / 2, dur), 1.0);
    }

    #[test]
    fn envelope_ramps_down_toward_the_note_end() {
        let dur = SAMPLE_RATE as usize;
        let near_end = envelope(dur - 10, dur);
        let mid = envelope(dur / 2, dur);
        assert!(
            near_end < mid,
            "release should pull the tail down from full sustain"
        );
    }

    #[test]
    fn envelope_of_a_very_short_note_never_panics_or_exceeds_unity() {
        // Duration shorter than the release window entirely: `dur > release`
        // is false, so only the attack ramp applies — this must not panic
        // on the `dur - i` subtraction inside the (skipped) release branch.
        for dur in [0usize, 1, 10, 100] {
            for i in 0..dur {
                let e = envelope(i, dur);
                assert!((0.0..=1.0).contains(&e));
            }
        }
    }

    // ── Timeline erase/remove ────────────────────────────────────────────────────

    fn timeline_note(id: u32, hole: u8, tick: usize, len: usize) -> GridNote {
        GridNote {
            id,
            hole,
            tick,
            len,
            dir: Dir::Blow,
            pitch: Pitch::Normal,
            expr: Expr::None,
        }
    }

    #[test]
    fn song_end_tick_is_the_last_notes_end() {
        let notes = vec![
            timeline_note(0, 1, 0, 4),
            timeline_note(1, 2, 10, 2),
            timeline_note(2, 3, 4, 4),
        ];
        assert_eq!(song_end_tick(&notes), 12);
    }

    #[test]
    fn song_end_tick_of_an_empty_song_is_zero() {
        assert_eq!(song_end_tick(&[]), 0);
    }

    #[test]
    fn normalize_range_orders_a_backwards_span() {
        assert_eq!(normalize_range(10, 4), (4, 10));
        assert_eq!(normalize_range(4, 10), (4, 10));
        assert_eq!(normalize_range(5, 5), (5, 5));
    }

    #[test]
    fn split_side_range_left_is_song_start_to_the_split() {
        let notes = vec![timeline_note(0, 1, 0, 20)];
        assert_eq!(split_side_range(8, Side::Left, &notes), (0, 8));
    }

    #[test]
    fn split_side_range_right_is_the_split_to_song_end() {
        let notes = vec![timeline_note(0, 1, 0, 20)];
        assert_eq!(split_side_range(8, Side::Right, &notes), (8, 20));
    }

    #[test]
    fn split_side_range_right_never_ends_before_the_split_on_an_empty_song() {
        assert_eq!(split_side_range(8, Side::Right, &[]), (8, 8));
    }

    #[test]
    fn erase_range_deletes_only_overlapping_notes_and_shifts_nothing() {
        let notes = vec![
            timeline_note(0, 1, 0, 4),  // 0..4, fully before the range
            timeline_note(1, 2, 4, 4),  // 4..8, inside the range
            timeline_note(2, 3, 6, 4),  // 6..10, partially overlaps
            timeline_note(3, 4, 12, 4), // 12..16, fully after the range
        ];
        let out = erase_range(&notes, 4, 10);
        let ids: Vec<u32> = out.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 3]);
        // Untouched notes keep their original position.
        assert_eq!(out.iter().find(|n| n.id == 3).unwrap().tick, 12);
    }

    #[test]
    fn remove_range_deletes_overlapping_notes_and_shifts_the_rest_earlier() {
        let notes = vec![
            timeline_note(0, 1, 0, 4),  // 0..4, before the range — untouched
            timeline_note(1, 2, 4, 4),  // 4..8, inside the range — deleted
            timeline_note(2, 3, 10, 4), // 10..14, after the range — shifts left by 6
        ];
        let out = remove_range(&notes, 4, 10);
        let ids: Vec<u32> = out.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 2]);
        assert_eq!(out.iter().find(|n| n.id == 0).unwrap().tick, 0);
        assert_eq!(out.iter().find(|n| n.id == 2).unwrap().tick, 4);
    }

    #[test]
    fn remove_range_closes_the_gap_exactly_the_removed_length() {
        let notes = vec![timeline_note(0, 1, 20, 4)];
        let out = remove_range(&notes, 5, 8); // remove a 3-tick span before it
        assert_eq!(out[0].tick, 17);
    }

    #[test]
    fn erase_and_remove_on_a_zero_length_range_are_no_ops() {
        let notes = vec![timeline_note(0, 1, 0, 4), timeline_note(1, 2, 8, 4)];
        assert_eq!(erase_range(&notes, 6, 6), notes);
        assert_eq!(remove_range(&notes, 6, 6), notes);
    }

    #[test]
    fn timeline_tool_is_active_is_false_only_for_none() {
        assert!(!TimelineTool::None.is_active());
        assert!(TimelineTool::Erase.is_active());
        assert!(TimelineTool::Remove.is_active());
    }

    // ── drag_end_tick ─────────────────────────────────────────────────────────

    #[test]
    fn drag_end_tick_advances_by_whole_ticks_moved_right() {
        assert_eq!(drag_end_tick(4, TICK_W, 1.0), 5);
        assert_eq!(drag_end_tick(4, 3.0 * TICK_W, 1.0), 7);
    }

    #[test]
    fn drag_end_tick_moves_back_left_and_clamps_at_zero() {
        assert_eq!(drag_end_tick(4, -TICK_W, 1.0), 3);
        assert_eq!(drag_end_tick(4, -10.0 * TICK_W, 1.0), 0);
    }

    #[test]
    fn drag_end_tick_divides_out_the_ui_scale_before_converting() {
        // At 2x UI zoom, the same visual tick of motion is twice as many
        // raw window pixels — dividing by `ui_scale` first is what keeps
        // the drag tracking the pointer 1:1 regardless of zoom level, the
        // same correction `grid.rs`'s note-move drag already applies.
        assert_eq!(drag_end_tick(4, 2.0 * TICK_W, 2.0), 5);
    }

    // ── TimelineSurfaceGeometry::tick_at ─────────────────────────────────────────

    #[test]
    fn tick_at_recenters_the_minus_half_to_half_normalized_range() {
        // `RelativeCursorPosition::normalized` is -0.5..0.5 across the
        // surface's own width, not 0..1 — a click at the surface's left
        // edge (-0.5) must resolve to tick 0, not get clamped away.
        let geom = TimelineSurfaceGeometry {
            scroll_beat: 0,
            width_px: 20.0 * TICK_W,
        };
        assert_eq!(geom.tick_at(-0.5), 0);
        assert_eq!(geom.tick_at(0.0), 10);
        assert_eq!(geom.tick_at(0.5), 20);
    }

    #[test]
    fn tick_at_offsets_by_the_surfaces_own_scroll_beat() {
        let geom = TimelineSurfaceGeometry {
            scroll_beat: 4,
            width_px: 20.0 * TICK_W,
        };
        // scroll_beat=4 beats = 16 ticks (TICKS_PER_BEAT=4) added on top of
        // the in-surface position.
        assert_eq!(geom.tick_at(-0.5), 16);
    }

    #[test]
    fn tick_at_clamps_outside_the_surfaces_own_bounds() {
        let geom = TimelineSurfaceGeometry {
            scroll_beat: 0,
            width_px: 20.0 * TICK_W,
        };
        assert_eq!(geom.tick_at(-5.0), 0);
        assert_eq!(geom.tick_at(5.0), 20);
    }
}
