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

use super::AppState;

mod grid;
mod harpchart;
mod interaction;
mod material;
mod panel;
mod playback;
mod practice;
mod state;
mod ui;

// ── Dialog purposes ───────────────────────────────────────────────────────────

use crate::dialogs::file_dialog::DialogId;

const SAVE_PURPOSE:  DialogId = DialogId("song_editor_2_save");
const LOAD_PURPOSE:  DialogId = DialogId("song_editor_2_load");
const MUSIC_PURPOSE: DialogId = DialogId("song_editor_2_music");

// ── Geometry ──────────────────────────────────────────────────────────────────

const HOLE_COL_W:    f32    = 78.0;
const HEADER_H:      f32    = 30.0;
const ROW_H:         f32    = 34.0;
const BEAT_W:        f32    = 60.0;
const ROWS:          u8     = 10;
const BEATS_PER_BAR: usize  = 4;
const NOTE_PAD:      f32    = 4.0;
const HANDLE_W:      f32    = 8.0;
const TICKS_PER_BEAT: usize = 4;
const TICK_W:        f32    = BEAT_W / TICKS_PER_BEAT as f32;

fn grid_height() -> f32 {
    HEADER_H + ROW_H * ROWS as f32
}

// ── Colours ───────────────────────────────────────────────────────────────────
//
// The editor's palette lives in the active theme (`crate::theme::LoadedTheme`,
// `theme.song_editor_colors()`) rather than as consts here, so a theme's
// `theme.json` can override it under `"colors": { "song_editor": { ... } }`.
// See `crate::theme::SongEditorColors` for the fields and their defaults —
// the same values this module used to hardcode.

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
                        grid::rebuild_grid
                            .run_if(resource_exists_and_changed::<state::EditorState>),
                    )
                        .chain(),
                    playback::update_playhead_view.after(playback::advance_playhead),
                    playback::update_progress_bar.after(playback::advance_playhead),
                    // Practice tick runs after the playhead advances so `elapsed` is current.
                    practice::practice_tick.after(playback::advance_playhead),
                    interaction::grid_keys,
                    interaction::type_into_field,
                    interaction::live_resize,
                    interaction::update_move_ghost,
                    panel::update_mod_panel,
                    panel::update_meta_fields,
                    panel::update_status_bar,
                    harpchart::handle_save_chosen,
                    harpchart::handle_load_chosen,
                    harpchart::handle_music_chosen,

                )
                    .run_if(in_state(AppState::SongEditor2)),
            );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::state::{
        apply_resize, can_place, enforce_direction, enforce_expr, move_target, Dir, EditorState,
        Edge, Expr, GridNote, Pitch,
    };
    use super::interaction::{apply_modifier, select_or_add};
    use super::ui::ModButton;
    use super::playback::{encode_wav, key_offset, note_freq, render_pcm, SAMPLE_RATE};
    use super::harpchart::serialize_harpchart;
    use super::grid::mix_srgba;
    use super::{BEAT_W, ROW_H, TICK_W, TICKS_PER_BEAT};

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
        assert_eq!(s.notes.iter().find(|n| n.hole == 5).unwrap().pitch, Pitch::Bend(0.5));
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes.iter().find(|n| n.hole == 5).unwrap().pitch, Pitch::Normal);
    }

    #[test]
    fn pitch_and_expression_stack() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Bend);
        apply_modifier(&mut s, ModButton::Vibrato);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
        assert_eq!(s.notes[0].expr, Expr::Vibrato);
        apply_modifier(&mut s, ModButton::Wah);
        assert_eq!(s.notes[0].expr, Expr::Wah);
        assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
    }

    #[test]
    fn overblow_only_on_low_holes() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 8, 0);
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes[0].pitch, Pitch::Normal);
        select_or_add(&mut s, 3, 0);
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes.iter().find(|n| n.hole == 3).unwrap().pitch, Pitch::Overblow);
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
        let mut s = EditorState::default();
        s.notes = vec![
            GridNote { id: 0, hole: 1, tick: 0, len: 3, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
            GridNote { id: 1, hole: 2, tick: 2, len: 3, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
            GridNote { id: 2, hole: 3, tick: 10, len: 1, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
        ];
        s.next_id = 3;
        enforce_direction(&mut s, 0);
        assert_eq!(s.note_by_id(1).unwrap().dir, Dir::Blow);
        assert_eq!(s.note_by_id(2).unwrap().dir, Dir::Draw);
    }

    // Wah (hand cupping) and vibrato (breath vibrato) are whole-player
    // techniques: every hole sounding at the same instant must share the
    // same one, mirroring how Blow/Draw is already unified above.
    #[test]
    fn enforce_expr_unifies_overlap_chain_but_not_independent_notes() {
        let mut s = EditorState::default();
        s.notes = vec![
            GridNote { id: 0, hole: 1, tick: 0, len: 3, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::Vibrato },
            GridNote { id: 1, hole: 2, tick: 2, len: 3, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
            GridNote { id: 2, hole: 3, tick: 10, len: 1, dir: Dir::Draw, pitch: Pitch::Normal, expr: Expr::None },
        ];
        s.next_id = 3;
        enforce_expr(&mut s, 0);
        assert_eq!(s.note_by_id(1).unwrap().expr, Expr::Vibrato, "overlapping note shares the vibrato");
        assert_eq!(s.note_by_id(2).unwrap().expr, Expr::None, "independent note is untouched");
    }

    #[test]
    fn clicking_wah_propagates_to_overlapping_notes_via_apply_modifier() {
        let mut s = EditorState::default();
        select_or_add(&mut s, 2, 0);
        select_or_add(&mut s, 5, 2); // overlaps the first note (tick 0..4 vs 2..6)
        select_or_add(&mut s, 7, 10); // independent
        s.selected = Some(s.note_at(2, 0).unwrap().id);
        apply_modifier(&mut s, ModButton::Wah);
        assert_eq!(s.note_at(2, 0).unwrap().expr, Expr::Wah);
        assert_eq!(s.note_at(5, 2).unwrap().expr, Expr::Wah, "overlapping note picks up the wah too");
        assert_eq!(s.note_at(7, 10).unwrap().expr, Expr::None, "independent note keeps its own expression");
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
        GridNote { id: 0, hole, tick: 0, len: 4, dir, pitch, expr: Expr::None }
    }

    #[test]
    fn note_freq_maps_holes_bends_and_key() {
        let c4 = note_freq(&note(1, Dir::Blow, Pitch::Normal), 0).unwrap();
        assert!((c4 - 261.63).abs() < 0.5, "got {c4}");
        let bent = note_freq(&note(1, Dir::Blow, Pitch::Bend(1.0)), 0).unwrap();
        assert!(bent < c4, "bend should drop pitch: {bent} !< {c4}");
        let g = note_freq(&note(1, Dir::Blow, Pitch::Normal), key_offset("G")).unwrap();
        assert!((g / c4 - 2f32.powf(7.0 / 12.0)).abs() < 0.001, "G harp is a fifth up");
        assert_eq!(key_offset("C"), 0);
        assert!(note_freq(&note(11, Dir::Blow, Pitch::Normal), 0).is_none());
    }

    #[test]
    fn render_and_wav_have_expected_size() {
        let notes = vec![note(4, Dir::Draw, Pitch::Normal)];
        let pcm = render_pcm(&notes, 120.0, 0);
        let expected = ((0.5 + 0.25) * SAMPLE_RATE as f32).ceil() as usize;
        assert_eq!(pcm.len(), expected);
        assert!(pcm.iter().any(|&s| s.abs() > 0.01), "note should be audible");
        let wav = encode_wav(&pcm, SAMPLE_RATE);
        assert_eq!(wav.len(), 44 + pcm.len() * 2);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn move_target_snaps_and_clamps() {
        assert_eq!(move_target(5, 4, 0.0, 0.0), (5, 4));
        assert_eq!(move_target(5, 4, TICK_W, 2.0 * ROW_H), (7, 5));
        assert_eq!(move_target(5, 4, BEAT_W, 0.0), (5, 4 + TICKS_PER_BEAT));
        assert_eq!(move_target(1, 0, -5.0 * BEAT_W, -5.0 * ROW_H), (1, 0));
        assert_eq!(move_target(10, 2, 0.0, 5.0 * ROW_H), (10, 2));
    }

    #[test]
    fn move_is_blocked_where_a_note_already_sits() {
        let notes = vec![
            GridNote { id: 0, hole: 3, tick: 0, len: 2, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
            GridNote { id: 1, hole: 3, tick: 5, len: 1, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None },
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
        let mut s = EditorState::default();
        s.name = "Test Song".into();
        s.author = "Test Artist".into();
        s.tempo = "120".into();
        s.key = "G".into();
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

        let single = &track[0];
        assert_eq!(single["events"][0]["note"], "B4");
    }

    #[test]
    fn mix_srgba_interpolates_and_keeps_base_alpha() {
        let base = bevy::prelude::Color::srgba(0.0, 0.0, 0.0, 0.5);
        let tint = bevy::prelude::Color::srgba(1.0, 1.0, 1.0, 1.0);

        let none = mix_srgba(base, tint, 0.0).to_srgba();
        assert_eq!((none.red, none.green, none.blue), (0.0, 0.0, 0.0));
        assert_eq!(none.alpha, 0.5, "base's own alpha is preserved, not blended");

        let full = mix_srgba(base, tint, 1.0).to_srgba();
        assert_eq!((full.red, full.green, full.blue), (1.0, 1.0, 1.0));
        assert_eq!(full.alpha, 0.5);

        let half = mix_srgba(base, tint, 0.5).to_srgba();
        assert!((half.red - 0.5).abs() < 1e-6);
    }
}
