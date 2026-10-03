// SPDX-License-Identifier: MIT

use super::clipboard::paste_targets_with_sources;
use super::grid::{group_move_targets, group_move_valid, mix_srgba, note_in_scale, visible_beats};
use super::harpchart::{
    load_harpchart, parse_pitch_expr, safe_path_segment, serialize_harpchart, validated_harpchart,
};
use super::interaction::{
    apply_modifier, delete_selected, resize_grip_position, select_or_add, select_or_add_ctrl,
};
use super::lesson_form::{populate_from_lesson_manifest, serialize_lesson};
use super::playback::{
    EditorAudio, PendingPlayback, Playhead, build_harp, finish_pending_playback, note_freq,
    playhead_for, secs_per_tick, start_playback,
};
use super::ranges::{
    erase_range, normalize_range, remove_range, silence_gaps, song_end_tick, split_side_range,
};
use super::state::Scroll;
use super::state::{
    ContentKind, Dir, Edge, EditorState, Expr, Field, GridNote, HarmonicaKind, PhraseAnnotation,
    Pitch, Side, TimelineTool, apply_resize, build_tempo_map, cycle_next, enforce_direction,
    enforce_expr, move_target, note_rect, toggle_tempo_point,
};
use super::timeline::{TimelineSurfaceGeometry, cycle_meter_point, drag_end_tick};
use super::ui::ModButton;
use super::undo::{HISTORY_LIMIT, UndoHistory};
use super::{BEAT_W, GRIP_D, HEADER_H, HOLE_COL_W, NOTE_PAD, ROW_H, TICK_W, TICKS_PER_BEAT};
use harmonicon_core::chart::Scale;
use harmonicon_core::harmonica::blues_scale_classes;
use harmonicon_core::synth::{PhraseNote, SAMPLE_RATE, envelope, render_pcm};
use harmonicon_core::wav::encode_wav;
use harmonicon_song::lessons::{LessonManifest, PassCriteria};

#[test]
fn cycle_next_wraps_back_to_the_first_option() {
    let options = ["a", "b", "c"];
    assert_eq!(cycle_next(&options, "a"), "b");
    assert_eq!(cycle_next(&options, "c"), "a");
}

#[test]
fn cycle_next_treats_an_unknown_current_value_as_the_first_option() {
    let options = ["a", "b", "c"];
    assert_eq!(cycle_next(&options, "not-a-real-option"), "b");
}

// ── playback: secs_per_tick / playhead_for ───────────────────────────────

#[test]
fn secs_per_tick_reflects_the_songs_own_tempo() {
    let s = EditorState { tempo: "60".into(), ..EditorState::default() };
    // 60 BPM: one beat per second, TICKS_PER_BEAT ticks per beat.
    let spt = secs_per_tick(&s);
    assert!((spt - 1.0 / TICKS_PER_BEAT as f32).abs() < 1e-6, "got {spt}");
}

#[test]
fn secs_per_tick_falls_back_to_120_bpm_for_an_unparseable_tempo() {
    let s = EditorState { tempo: "not-a-number".into(), ..EditorState::default() };
    let spt = secs_per_tick(&s);
    let expected = 60.0 / 120.0 / TICKS_PER_BEAT as f32;
    assert!((spt - expected).abs() < 1e-6, "got {spt}");
}

#[test]
fn playhead_for_starts_playing_from_zero_with_the_right_total() {
    let ph = playhead_for(8, 0.25);
    assert!(ph.playing);
    assert!(!ph.paused);
    assert_eq!(ph.elapsed, 0.0);
    assert_eq!(ph.secs_per_tick, 0.25);
    assert_eq!(ph.total, 2.0);
}

// ── playback: rendering off the main thread ──────────────────────────────

/// An app with the task pools and audio assets Play needs, holding a
/// one-note chart, with Play already pressed.
fn app_after_play() -> bevy::app::App {
    use bevy::asset::AssetPlugin;
    use bevy::audio::AudioSource;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<AudioSource>()
        .insert_resource(EditorState {
            notes: vec![grid_note(0, TICKS_PER_BEAT)],
            ..EditorState::default()
        })
        .insert_resource(harmonicon_audio::AudioSettings::default())
        .init_resource::<Playhead>()
        .add_systems(Update, finish_pending_playback);
    app.world_mut()
        .run_system_once(
            |state: Res<EditorState>,
             mut sources: ResMut<Assets<AudioSource>>,
             settings: Res<harmonicon_audio::AudioSettings>,
             playing: Query<Entity, With<EditorAudio>>,
             mut playhead: ResMut<Playhead>,
             mut commands: Commands| {
                start_playback(
                    &state,
                    &mut sources,
                    &settings,
                    &playing,
                    &mut playhead,
                    &mut commands,
                );
            },
        )
        .unwrap();
    app
}

/// Runs frames until no render is pending, or gives up after two seconds.
fn update_until_rendered(app: &mut bevy::app::App) {
    for _ in 0..200 {
        app.update();
        let mut pending =
            app.world_mut().query_filtered::<(), bevy::prelude::With<PendingPlayback>>();
        if pending.iter(app.world()).next().is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the Play render never finished");
}

#[test]
fn play_starts_the_playhead_only_once_the_render_lands() {
    use bevy::audio::{AudioPlayer, AudioSource};

    let mut app = app_after_play();
    assert!(
        !app.world().resource::<Playhead>().playing,
        "the playhead must wait for the audio it tracks"
    );

    update_until_rendered(&mut app);

    let mut players = app.world_mut().query_filtered::<(), (
        bevy::prelude::With<EditorAudio>,
        bevy::prelude::With<AudioPlayer<AudioSource>>,
    )>();
    assert_eq!(players.iter(app.world()).count(), 1);
    let playhead = app.world().resource::<Playhead>();
    assert!(playhead.playing);
    let spt = secs_per_tick(&EditorState::default());
    assert!((playhead.total - TICKS_PER_BEAT as f32 * spt).abs() < 1e-6);
}

#[test]
fn stopping_before_the_render_lands_cancels_it() {
    use bevy::prelude::*;

    let mut app = app_after_play();
    // What every stop path does: despawn all editor audio.
    let audio: Vec<Entity> =
        app.world_mut().query_filtered::<Entity, With<EditorAudio>>().iter(app.world()).collect();
    assert_eq!(audio.len(), 1, "the pending render is editor audio");
    for entity in audio {
        app.world_mut().despawn(entity);
    }
    for _ in 0..20 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!app.world().resource::<Playhead>().playing);
}

// ── lesson_form ──────────────────────────────────────────────────────────

#[test]
fn serialize_lesson_omits_optional_fields_when_unset() {
    let s = EditorState {
        content_kind: ContentKind::Lesson,
        lesson_id: "my-lesson".into(),
        lesson_unit: "basics".into(),
        ..EditorState::default()
    };
    let (json, _warnings) = serialize_lesson(&s);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["id"], "my-lesson");
    assert_eq!(v["unit"], "basics");
    assert_eq!(v["title_key"], "lesson-my-lesson-title");
    assert_eq!(v["body_key"], "lesson-my-lesson-body");
    assert!(v.get("chart").is_none());
    assert!(v.get("prerequisites").is_none());
    assert!(v.get("pass_criteria").is_none());
    assert!(v.get("progression").is_none());
}

#[test]
fn serialize_lesson_has_no_warnings_when_id_and_unit_are_set() {
    let s = EditorState {
        content_kind: ContentKind::Lesson,
        lesson_id: "my-lesson".into(),
        lesson_unit: "basics".into(),
        ..EditorState::default()
    };
    let (_json, warnings) = serialize_lesson(&s);
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn serialize_lesson_warns_when_id_or_unit_is_empty() {
    let s = EditorState { content_kind: ContentKind::Lesson, ..EditorState::default() };
    let (_json, warnings) = serialize_lesson(&s);
    // An empty id/unit also fails the manifest's own schema (both are
    // required fields), so this expects at least the id/unit warning
    // itself, not necessarily only that one.
    assert!(
        warnings.iter().any(|w| w.contains("id/unit")),
        "expected an id/unit warning, got: {warnings:?}"
    );
}

#[test]
fn serialize_lesson_includes_chart_only_when_notes_exist() {
    let mut s = EditorState {
        lesson_id: "with-notes".into(),
        lesson_unit: "basics".into(),
        ..EditorState::default()
    };
    select_or_add(&mut s, 4, 0);
    let (json, _warnings) = serialize_lesson(&s);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["chart"], "song/chart.harpchart");
}

#[test]
fn serialize_lesson_writes_a_technique_pass_criterion() {
    let s = EditorState {
        lesson_id: "x".into(),
        lesson_unit: "u".into(),
        lesson_pass_criteria: "technique".into(),
        lesson_technique: "bend".into(),
        lesson_threshold: "0.6".into(),
        ..EditorState::default()
    };
    let (json, _warnings) = serialize_lesson(&s);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["pass_criteria"]["type"], "technique");
    assert_eq!(v["pass_criteria"]["technique"], "bend");
    assert_eq!(v["pass_criteria"]["threshold"].as_f64().unwrap(), 0.6_f32 as f64);
}

#[test]
fn serialize_lesson_writes_prerequisites_and_progression() {
    let s = EditorState {
        lesson_id: "x".into(),
        lesson_unit: "u".into(),
        lesson_prerequisites: "a, b ,c".into(),
        lesson_progression: "minor".into(),
        lesson_scale: "minor-pentatonic".into(),
        ..EditorState::default()
    };
    let (json, _warnings) = serialize_lesson(&s);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["prerequisites"], serde_json::json!(["a", "b", "c"]));
    assert_eq!(v["progression"], "minor");
    assert_eq!(v["scale"], "minor-pentatonic");
}

#[test]
fn serialize_lesson_writes_elective_only_when_selected() {
    let core =
        EditorState { lesson_id: "core".into(), lesson_unit: "u".into(), ..EditorState::default() };
    let (json, _) = serialize_lesson(&core);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("optional").is_none());

    let elective = EditorState { lesson_path: "elective".into(), ..core };
    let (json, _) = serialize_lesson(&elective);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["optional"], true);
}

#[test]
fn populate_from_lesson_manifest_round_trips_a_technique_criterion() {
    let manifest = LessonManifest {
        id: "hand-wah".into(),
        unit: "blowing".into(),
        optional: false,
        track: None,
        training: None,
        title_key: "t".into(),
        body_key: "b".into(),
        chart: None,
        aural: false,
        prerequisites: vec!["single-note".into()],
        pass_criteria: Some(PassCriteria::Technique {
            technique: "wah-wah".into(),
            threshold: 0.5,
        }),
        progression: None,
        scale: None,
        diagram: None,
        widgets: Vec::new(),
        position_cycle: false,
    };
    let mut s = EditorState::default();
    populate_from_lesson_manifest(&manifest, &mut s);
    assert_eq!(s.lesson_id, "hand-wah");
    assert_eq!(s.lesson_unit, "blowing");
    assert_eq!(s.lesson_path, "core");
    assert_eq!(s.lesson_prerequisites, "single-note");
    assert_eq!(s.lesson_pass_criteria, "technique");
    assert_eq!(s.lesson_technique, "wah-wah");
    assert_eq!(s.lesson_threshold, "0.5");
    assert_eq!(s.lesson_progression, "none");
    assert_eq!(s.lesson_scale, "none");
}

#[test]
fn populate_from_lesson_manifest_defaults_pass_criteria_to_none_when_absent() {
    let manifest = LessonManifest {
        id: "x".into(),
        unit: "u".into(),
        optional: false,
        track: None,
        training: None,
        title_key: "t".into(),
        body_key: "b".into(),
        chart: None,
        aural: false,
        prerequisites: Vec::new(),
        pass_criteria: None,
        progression: Some("standard".into()),
        scale: Some("major".into()),
        diagram: None,
        widgets: Vec::new(),
        position_cycle: false,
    };
    let mut s = EditorState::default();
    populate_from_lesson_manifest(&manifest, &mut s);
    assert_eq!(s.lesson_pass_criteria, "none");
    assert_eq!(s.lesson_progression, "standard");
    assert_eq!(s.lesson_scale, "major");
}

#[test]
fn click_adds_then_selects_without_duplicating() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 4, 2);
    assert_eq!(s.notes.len(), 1);
    let added = s.notes[0];
    assert_eq!(s.selected, vec![added.id]);
    assert_eq!((added.hole, added.tick, added.len), (4, 2, TICKS_PER_BEAT));
    select_or_add(&mut s, 4, 2);
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.selected, vec![added.id]);
}

// ── Multi-select ──────────────────────────────────────────────────────────

#[test]
fn ctrl_click_toggles_notes_into_and_out_of_the_selection() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 2, 0);
    let a = s.notes[0].id;
    select_or_add(&mut s, 5, 0);
    let b = s.notes[1].id;
    // A plain click on `b` above already replaced the selection with just
    // it — Ctrl+click `a` to add it back in alongside `b`.
    select_or_add_ctrl(&mut s, 2, 0);
    assert_eq!(s.selected, vec![b, a]);
    // Ctrl+click `b` again removes just it, leaving `a` selected.
    select_or_add_ctrl(&mut s, 5, 0);
    assert_eq!(s.selected, vec![a]);
}

#[test]
fn ctrl_click_on_empty_space_still_creates_and_selects_a_note() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 2, 0);
    select_or_add_ctrl(&mut s, 5, 0);
    assert_eq!(s.notes.len(), 2);
    // Extending onto a freshly-created note behaves like a plain click,
    // since there was nothing existing yet to add to the selection.
    assert_eq!(s.selected, vec![s.notes[1].id]);
}

#[test]
fn delete_selected_removes_every_note_in_a_multi_selection() {
    let mut s = EditorState::default();
    // Plain clicks to create three notes (each replaces the selection with
    // just itself), then Ctrl+click the first two back in alongside the
    // third to build a three-note multi-selection.
    select_or_add(&mut s, 2, 0);
    select_or_add(&mut s, 5, 0);
    select_or_add(&mut s, 7, 0);
    select_or_add_ctrl(&mut s, 2, 0);
    select_or_add_ctrl(&mut s, 5, 0);
    assert_eq!(s.notes.len(), 3);
    assert_eq!(s.selected.len(), 3);
    apply_modifier(&mut s, ModButton::Delete);
    assert!(s.notes.is_empty());
    assert!(s.selected.is_empty());
}

#[test]
fn group_move_targets_shifts_every_member_by_the_same_delta() {
    let make = |id: u32, hole: u8, tick: usize| GridNote::plain(id, hole, tick, 4);
    let others = vec![make(2, 3, 8), make(3, 5, 16)];
    let targets = group_move_targets(&others, 1, TICKS_PER_BEAT as i32, 10);
    assert_eq!(
        targets,
        vec![
            (2, 4, 8 + TICKS_PER_BEAT, 4, Pitch::Normal),
            (3, 6, 16 + TICKS_PER_BEAT, 4, Pitch::Normal),
        ]
    );
}

#[test]
fn group_move_targets_clamps_each_member_to_the_hole_range() {
    let note = GridNote::plain(1, 9, 0, 4);
    let targets = group_move_targets(&[note], 5, 0, 10);
    assert_eq!(targets[0].1, 10); // clamped at the top hole
}

#[test]
fn group_move_valid_rejects_a_target_overlapping_a_note_outside_the_group() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 3, 0); // an unrelated, unselected note
    let targets = vec![(99u32, 3, 0, 4, Pitch::Normal)];
    assert!(!group_move_valid(
        &s.notes,
        &build_harp("C", HarmonicaKind::Diatonic),
        &[99],
        &targets
    ));
    // Moving out of the blocker's way is fine again — the blocker's default
    // length is one full beat (`TICKS_PER_BEAT`), so its span ends there.
    let clear = vec![(99u32, 3, TICKS_PER_BEAT, 4, Pitch::Normal)];
    assert!(group_move_valid(&s.notes, &build_harp("C", HarmonicaKind::Diatonic), &[99], &clear));
}

#[test]
fn group_move_valid_ignores_overlap_among_the_groups_own_members() {
    // Two notes in the same group, already overlapping each other (e.g. a
    // chord) — that must not block the move.
    let notes = vec![GridNote::plain(1, 2, 0, 4), GridNote::plain(2, 5, 0, 4)];
    let targets = vec![(1u32, 2, 4, 4, Pitch::Normal), (2u32, 5, 4, 4, Pitch::Normal)];
    assert!(group_move_valid(&notes, &build_harp("C", HarmonicaKind::Diatonic), &[1, 2], &targets));
}

#[test]
fn group_move_valid_rejects_a_pitch_incompatible_with_its_target_hole() {
    // Bend(1.5) needs a hole with at least two semitones of bend, i.e.
    // holes 2/3/10 (see `max_bend`) — landing on hole 5
    // must fail even with nothing else in the way.
    let targets = vec![(1u32, 5, 0, 4, Pitch::Bend(1.5))];
    assert!(!group_move_valid(&[], &build_harp("C", HarmonicaKind::Diatonic), &[1], &targets));
}

// ── Copy/paste ────────────────────────────────────────────────────────────

/// [`paste_targets_with_sources`] minus the source pairing, for the
/// placement-rule tests below that only care where notes land.
fn paste_targets(
    clipboard: &[GridNote],
    target_tick: usize,
    hole_count: u8,
    existing: &[GridNote],
    next_id: u32,
) -> (Vec<GridNote>, u32) {
    let (pairs, next) =
        paste_targets_with_sources(clipboard, target_tick, hole_count, existing, next_id);
    (pairs.into_iter().map(|(_, p)| p).collect(), next)
}

#[test]
fn copy_selection_returns_only_the_selected_notes_verbatim() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 2, 0);
    select_or_add(&mut s, 5, 4);
    s.selected = vec![s.notes[0].id];
    let copied = s.copy_selection();
    assert_eq!(copied.notes, vec![s.notes[0]]);
}

#[test]
fn paste_targets_shifts_the_earliest_note_to_the_target_tick() {
    let clipboard = vec![GridNote::plain(1, 2, 4, 4), GridNote::plain(2, 5, 8, 4)];
    let (pasted, next_id) = paste_targets(&clipboard, 20, 10, &[], 100);
    // The earliest note (tick 4) lands at 20; the other keeps its +4 offset.
    assert_eq!(pasted.iter().map(|n| (n.hole, n.tick)).collect::<Vec<_>>(), vec![(2, 20), (5, 24)]);
    // Ids are freshly assigned starting at `next_id`, never reusing the
    // clipboard's own copied ids.
    assert_eq!(pasted.iter().map(|n| n.id).collect::<Vec<_>>(), vec![100, 101]);
    assert_eq!(next_id, 102);
}

#[test]
fn paste_targets_skips_a_note_landing_on_top_of_an_existing_one() {
    let clipboard = vec![GridNote::plain(1, 2, 0, 4)];
    let existing = vec![GridNote::plain(99, 2, 10, 4)];
    // Pasting right on top of the existing note is skipped...
    let (pasted, next_id) = paste_targets(&clipboard, 10, 10, &existing, 5);
    assert!(pasted.is_empty());
    assert_eq!(next_id, 5);
    // ...but pasting clear of it lands normally.
    let (pasted, next_id) = paste_targets(&clipboard, 20, 10, &existing, 5);
    assert_eq!(pasted.len(), 1);
    assert_eq!(next_id, 6);
}

#[test]
fn paste_targets_skips_a_note_beyond_the_current_harps_hole_count() {
    let clipboard = vec![GridNote::plain(1, 11, 0, 4)];
    // Fits a 12-hole chromatic harp...
    let (pasted, _) = paste_targets(&clipboard, 0, 12, &[], 0);
    assert_eq!(pasted.len(), 1);
    // ...but not a 10-hole diatonic one.
    let (pasted, next_id) = paste_targets(&clipboard, 0, 10, &[], 0);
    assert!(pasted.is_empty());
    assert_eq!(next_id, 0);
}

#[test]
fn paste_targets_of_an_empty_clipboard_is_a_no_op() {
    let (pasted, next_id) = paste_targets(&[], 10, 10, &[], 7);
    assert!(pasted.is_empty());
    assert_eq!(next_id, 7);
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
    // Hole 5's two reeds are a semitone apart (E and F on a C harp), so
    // there is no note in between for a bend to land on — `max_bend` is 0
    // and the button does nothing (see `pitch_map::max_bend`).
    let mut s = EditorState::default();
    select_or_add(&mut s, 5, 0);
    let hole5 = s.notes[0].id;
    select_or_add(&mut s, 7, 0);
    s.selected = vec![hole5];
    for _ in 0..2 {
        apply_modifier(&mut s, ModButton::Bend);
        assert_eq!(s.notes.iter().find(|n| n.hole == 5).unwrap().pitch, Pitch::Normal);
    }
}

// ── Sticky modifiers ──────────────────────────────────────────────────────

#[test]
fn clicking_a_mod_button_with_nothing_selected_arms_it_for_new_notes() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Draw);
    apply_modifier(&mut s, ModButton::Wah); // -> 2.0
    select_or_add(&mut s, 3, 0);
    let n = &s.notes[0];
    assert_eq!(n.dir, Dir::Draw);
    assert_eq!(n.expr, Expr::Wah(2.0));
}

#[test]
fn sticky_bend_arms_without_a_selection_and_applies_to_a_compatible_hole() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Bend); // -> 0.5
    select_or_add(&mut s, 2, 0); // hole 2: max_bend 2.0, compatible
    assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
}

#[test]
fn sticky_pitch_falls_back_to_normal_on_an_incompatible_hole_but_stays_armed() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Overblow);
    // Hole 8 can't overblow (only 1/4/5/6) — this one note falls back...
    select_or_add(&mut s, 8, 0);
    assert_eq!(s.notes[0].pitch, Pitch::Normal);
    // ...but the sticky arm itself wasn't cleared by that rejection.
    select_or_add(&mut s, 4, 4);
    assert_eq!(s.notes.iter().find(|n| n.hole == 4).unwrap().pitch, Pitch::Overblow);
}

#[test]
fn cycling_sticky_bend_past_the_richest_cap_turns_it_off() {
    // Counted off the harp's own deepest bend rather than a literal, so
    // this keeps testing the wrap and not a particular depth.
    let mut s = EditorState::default();
    let steps = (super::interaction::deepest_bend(&s.effective_harp()) / 0.5).round() as usize;
    for _ in 0..steps {
        apply_modifier(&mut s, ModButton::Bend); // 0.5 .. DEEPEST_BEND
    }
    apply_modifier(&mut s, ModButton::Bend); // past the cap -> Normal (off)
    select_or_add(&mut s, 2, 0);
    assert_eq!(s.notes[0].pitch, Pitch::Normal);
}

#[test]
fn selecting_an_existing_note_and_editing_it_also_arms_sticky() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 2, 0);
    apply_modifier(&mut s, ModButton::Bend); // edits the selected note...
    assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
    // ...and arms sticky the same way a nothing-selected click would.
    select_or_add(&mut s, 3, 4);
    assert_eq!(s.notes.iter().find(|n| n.hole == 3).unwrap().pitch, Pitch::Bend(0.5));
}

#[test]
fn armed_sticky_wah_propagates_to_a_simultaneous_chord_note() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 2, 0);
    apply_modifier(&mut s, ModButton::Wah); // arms sticky Wah, applies to hole 2
    select_or_add(&mut s, 5, 0); // same tick, different hole — a chord
    assert_eq!(
        s.notes.iter().find(|n| n.hole == 2).unwrap().expr,
        s.notes.iter().find(|n| n.hole == 5).unwrap().expr
    );
    assert!(matches!(s.notes.iter().find(|n| n.hole == 5).unwrap().expr, Expr::Wah(_)));
}

#[test]
fn switching_harmonica_kind_sanitizes_an_incompatible_sticky_pitch() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Overblow);
    assert_eq!(s.sticky_pitch, Pitch::Overblow);
    s.set_harmonica_kind(HarmonicaKind::Chromatic);
    assert_eq!(s.sticky_pitch, Pitch::Normal);
}

// ── Overblow/Overdraw direction pairing ──────────────────────────────────────
//
// Overblow only exists while blowing and Overdraw only while drawing
// (see `state::pitch_forced_dir`'s doc comment) — a note (or the sticky
// arm) must never end up with e.g. `pitch: Overblow, dir: Draw`, a
// physically impossible combination, however direction and pitch are
// armed.

#[test]
fn arming_overblow_then_draw_with_nothing_selected_clears_the_pitch() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Overblow);
    assert_eq!(s.sticky_pitch, Pitch::Overblow);
    assert_eq!(s.sticky_dir, Dir::Blow);
    apply_modifier(&mut s, ModButton::Draw);
    assert_eq!(s.sticky_dir, Dir::Draw);
    assert_eq!(s.sticky_pitch, Pitch::Normal, "overblow can't survive a switch to Draw");
}

#[test]
fn arming_overdraw_then_blow_with_nothing_selected_clears_the_pitch() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Overdraw);
    assert_eq!(s.sticky_pitch, Pitch::Overdraw);
    assert_eq!(s.sticky_dir, Dir::Draw);
    apply_modifier(&mut s, ModButton::Blow);
    assert_eq!(s.sticky_dir, Dir::Blow);
    assert_eq!(s.sticky_pitch, Pitch::Normal);
}

#[test]
fn a_new_note_placed_with_armed_overblow_is_never_tagged_draw() {
    let mut s = EditorState::default();
    // Arm Draw first, then Overblow — before the fix these were two
    // independent sticky fields, so an overblow-capable note placed here
    // would have landed as `pitch: Overblow, dir: Draw`.
    apply_modifier(&mut s, ModButton::Draw);
    apply_modifier(&mut s, ModButton::Overblow);
    select_or_add(&mut s, 4, 0);
    let n = &s.notes[0];
    assert_eq!(n.pitch, Pitch::Overblow);
    assert_eq!(n.dir, Dir::Blow);
}

#[test]
fn a_new_note_placed_with_armed_overdraw_is_never_tagged_blow() {
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Blow);
    apply_modifier(&mut s, ModButton::Overdraw);
    select_or_add(&mut s, 8, 0);
    let n = &s.notes[0];
    assert_eq!(n.pitch, Pitch::Overdraw);
    assert_eq!(n.dir, Dir::Draw);
}

#[test]
fn setting_overblow_on_a_selected_note_forces_its_direction_and_propagates() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 4, 0);
    apply_modifier(&mut s, ModButton::Draw); // starts as Draw
    select_or_add(&mut s, 5, 0); // a simultaneous chord note, also Draw
    s.selected = vec![s.note_at(4, 0).unwrap().id];
    apply_modifier(&mut s, ModButton::Overblow);
    let hole4 = s.notes.iter().find(|n| n.hole == 4).unwrap();
    assert_eq!(hole4.pitch, Pitch::Overblow);
    assert_eq!(hole4.dir, Dir::Blow);
    // The whole chord follows — direction is whole-player, not per-hole.
    assert_eq!(s.notes.iter().find(|n| n.hole == 5).unwrap().dir, Dir::Blow);
}

#[test]
fn clicking_draw_on_a_selected_overblow_note_clears_its_pitch() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 4, 0);
    apply_modifier(&mut s, ModButton::Overblow);
    assert_eq!(s.notes[0].pitch, Pitch::Overblow);
    apply_modifier(&mut s, ModButton::Draw);
    assert_eq!(s.notes[0].dir, Dir::Draw);
    assert_eq!(s.notes[0].pitch, Pitch::Normal);
}

#[test]
fn pitch_and_expression_stack() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 3, 0);
    apply_modifier(&mut s, ModButton::Bend);
    apply_modifier(&mut s, ModButton::Vibrato);
    assert_eq!(s.notes[0].pitch, Pitch::Bend(0.5));
    assert_eq!(s.notes[0].expr, Expr::Vibrato(3.0), "first click lands on the min rate");
    apply_modifier(&mut s, ModButton::Wah);
    assert_eq!(s.notes[0].expr, Expr::Wah(2.0), "first click lands on the min rate");
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
    assert_eq!(s.notes[0].expr, Expr::None, "cycling past the max rate deselects");
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
    assert_eq!(s.notes[0].expr, Expr::None, "cycling past the max rate deselects");
}

#[test]
fn overblow_only_on_holes_with_a_reed_to_overblow() {
    // `song::harmonica::hole_notes` only defines an overblow reed for
    // 1/4/5/6 — `state::overblow_ok` must agree exactly, or a note tagged
    // `Overblow` on some other hole (holes 2/3 included: this codebase's
    // harp model, unlike some looser "any of 1-6" conventions, doesn't
    // give them one) resolves to no pitch anywhere downstream (scoring,
    // playback, `music_score`'s notation) despite the editor having
    // accepted the click. Each hole gets its own fresh `EditorState` —
    // editing a selected note's pitch also syncs `sticky_pitch` to match,
    // so accumulating one shared state across holes would let an earlier
    // successful Overblow silently pre-apply to (and then, via the second
    // click, un-apply from) a later hole regardless of its own compatibility.
    for hole in [2, 3, 7, 8, 9, 10] {
        let mut s = EditorState::default();
        select_or_add(&mut s, hole, 0);
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes[0].pitch, Pitch::Normal, "hole {hole} has no overblow reed");
    }
    for hole in [1, 4, 5, 6] {
        let mut s = EditorState::default();
        select_or_add(&mut s, hole, 0);
        apply_modifier(&mut s, ModButton::Overblow);
        assert_eq!(s.notes[0].pitch, Pitch::Overblow, "hole {hole} does have one");
    }
}

#[test]
fn slide_cycles_on_and_off_on_any_hole() {
    let mut s = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
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
    s.set_harmonica_kind(HarmonicaKind::Chromatic16);
    assert_eq!(s.hole_count(), 16);
}

#[test]
fn switching_to_diatonic_drops_notes_beyond_hole_ten_and_clears_slide() {
    let mut s = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
    select_or_add(&mut s, 11, 0);
    apply_modifier(&mut s, ModButton::Slide);
    select_or_add(&mut s, 3, 4);
    apply_modifier(&mut s, ModButton::Slide);

    s.set_harmonica_kind(HarmonicaKind::Diatonic);

    assert_eq!(s.notes.len(), 1, "the hole-11 note doesn't fit anymore");
    assert_eq!(s.notes[0].pitch, Pitch::Normal, "slide isn't a valid diatonic technique");
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
    let mut s = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
    select_or_add(&mut s, 11, 0);
    assert!(!s.selected.is_empty());

    s.set_harmonica_kind(HarmonicaKind::Diatonic);

    assert!(s.selected.is_empty());
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
    assert!(s.selected.is_empty());
}

#[test]
fn clicking_a_covered_beat_selects_rather_than_stacks() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 4, 0);
    let id = s.notes[0].id;
    s.notes[0].len = 3;
    select_or_add(&mut s, 4, 2);
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.selected, vec![id]);
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
    s.selected = vec![s.note_at(2, 0).unwrap().id];
    apply_modifier(&mut s, ModButton::Draw);
    assert_eq!(s.note_at(2, 0).unwrap().dir, Dir::Draw);
    assert_eq!(s.note_at(5, 0).unwrap().dir, Dir::Draw);
}

#[test]
fn enforce_unifies_overlap_chain_but_not_independent_notes() {
    let mut s = EditorState {
        notes: vec![
            GridNote::plain(0, 1, 0, 3),
            GridNote { dir: Dir::Draw, ..GridNote::plain(1, 2, 2, 3) },
            GridNote { dir: Dir::Draw, ..GridNote::plain(2, 3, 10, 1) },
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
            GridNote { expr: Expr::Vibrato(5.0), ..GridNote::plain(0, 1, 0, 3) },
            GridNote { dir: Dir::Draw, ..GridNote::plain(1, 2, 2, 3) },
            GridNote { dir: Dir::Draw, ..GridNote::plain(2, 3, 10, 1) },
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
    assert_eq!(s.note_by_id(2).unwrap().expr, Expr::None, "independent note is untouched");
}

#[test]
fn clicking_wah_propagates_to_overlapping_notes_via_apply_modifier() {
    let mut s = EditorState::default();
    let half_beat = TICKS_PER_BEAT / 2;
    select_or_add(&mut s, 2, 0);
    // Overlaps the first note: its default length is one full beat
    // (`TICKS_PER_BEAT`), so a note starting mid-beat still falls inside it.
    select_or_add(&mut s, 5, half_beat);
    select_or_add(&mut s, 7, TICKS_PER_BEAT * 3); // well past it: independent
    s.selected = vec![s.note_at(2, 0).unwrap().id];
    apply_modifier(&mut s, ModButton::Wah);
    assert_eq!(s.note_at(2, 0).unwrap().expr, Expr::Wah(2.0));
    assert_eq!(
        s.note_at(5, half_beat).unwrap().expr,
        Expr::Wah(2.0),
        "overlapping note picks up the wah too"
    );
    assert_eq!(
        s.note_at(7, TICKS_PER_BEAT * 3).unwrap().expr,
        Expr::None,
        "independent note keeps its own expression"
    );
}

#[test]
fn separate_times_keep_independent_directions() {
    let mut s = EditorState::default();
    select_or_add(&mut s, 2, 0);
    // Starts exactly where the first note's default one-beat length ends,
    // so the two don't overlap.
    select_or_add(&mut s, 2, TICKS_PER_BEAT);
    s.selected = vec![s.note_at(2, TICKS_PER_BEAT).unwrap().id];
    apply_modifier(&mut s, ModButton::Draw);
    assert_eq!(s.note_at(2, 0).unwrap().dir, Dir::Blow);
    assert_eq!(s.note_at(2, TICKS_PER_BEAT).unwrap().dir, Dir::Draw);
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
    // The reed comes from the technique, not from `note.dir` plus a
    // semitone — which would be wrong for the very common case of an
    // Overblow note left at its default `Dir::Blow`. Overblow (holes
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
    // One full beat long (`note()`'s default `len: 4` is a third of one)
    // — `expected` below assumes exactly one beat of note (0.5s at
    // 120bpm) plus the synth's fixed tail.
    let notes = [GridNote { len: TICKS_PER_BEAT, ..note(4, Dir::Draw, Pitch::Normal) }];
    let harp = build_harp("C", HarmonicaKind::Diatonic);
    let phrase: Vec<PhraseNote> = notes
        .iter()
        .map(|n| PhraseNote { tick: n.tick, len: n.len, freq: note_freq(n, &harp), expr: n.expr })
        .collect();
    let secs_per_tick = 60.0 / 120.0 / TICKS_PER_BEAT as f32;
    let pcm = render_pcm(&phrase, secs_per_tick);
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
    let notes = vec![GridNote::plain(0, 3, 0, 2), GridNote::plain(1, 3, 5, 1)];
    let target = |hole, tick| vec![(1u32, hole, tick, 1, Pitch::Normal)];
    assert!(!group_move_valid(
        &notes,
        &build_harp("C", HarmonicaKind::Diatonic),
        &[1],
        &target(3, 1)
    ));
    assert!(group_move_valid(
        &notes,
        &build_harp("C", HarmonicaKind::Diatonic),
        &[1],
        &target(3, 2)
    ));
    assert!(group_move_valid(
        &notes,
        &build_harp("C", HarmonicaKind::Diatonic),
        &[1],
        &target(4, 0)
    ));
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
fn unequal_simultaneous_note_lengths_round_trip_without_being_extended() {
    let state = EditorState {
        notes: vec![
            GridNote::plain(0, 1, 0, TICKS_PER_BEAT),
            GridNote::plain(1, 2, 0, TICKS_PER_BEAT / 2),
        ],
        ..Default::default()
    };
    let value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    assert_eq!(value["track"].as_array().unwrap().len(), 2);

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&value, &mut loaded, &mut scroll);
    assert_eq!(loaded.note_at(1, 0).unwrap().len, TICKS_PER_BEAT);
    assert_eq!(loaded.note_at(2, 0).unwrap().len, TICKS_PER_BEAT / 2);
}

#[test]
fn serialize_harpchart_omits_audio_file_when_no_music_is_picked() {
    let mut s = EditorState { name: "Test Song".into(), key: "G".into(), ..Default::default() };
    select_or_add(&mut s, 2, 0);

    let v: serde_json::Value = serde_json::from_str(&serialize_harpchart(&s)).expect("valid JSON");
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

    let v: serde_json::Value = serde_json::from_str(&serialize_harpchart(&s)).expect("valid JSON");
    assert_eq!(v["metadata"]["audio_file"], "music.ogg");
}

/// A chart the Song Editor writes must pass the exact schema
/// `song::loader::SongChartLoader` validates against at load time — with
/// `additionalProperties: false` at every level, a field the editor writes
/// but the schema doesn't declare fails validation outright, making every
/// song saved by the editor unplayable.
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
        serde_json::from_str(include_str!("../../../../assets/song_schema.dtd.json"))
            .expect("schema is valid JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("  - {e} (at /{path})", path = e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "chart saved by the Song Editor must pass its own schema:\n{}",
        errors.join("\n")
    );
}

#[test]
fn editor_load_validation_rejects_a_structurally_invalid_chart() {
    let error = validated_harpchart("{}").expect_err("an empty object is not a chart");
    assert!(error.contains("Chart validation failed"));
    assert!(error.contains("song"));
}

#[test]
fn editor_load_validation_rejects_a_future_chart_version() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    let mut value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    value["metadata"]["format_version"] = serde_json::json!("999.0.0");

    let error = validated_harpchart(&value.to_string()).expect_err("future chart must fail");
    assert!(error.contains("999.0.0"));
    assert!(error.contains("can't load"));
}

#[test]
fn chart_schema_rejects_unknown_diatonic_profiles_before_editor_loading() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    let mut value: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    value["harmonica"]["bending_profile"] = serde_json::json!("future_tuning");

    let error = validated_harpchart(&value.to_string()).expect_err("unknown profile must fail");
    assert!(error.contains("Chart validation failed"));
    assert!(error.contains("bending_profile"));
}

#[test]
fn chart_schema_rejects_chromatics_above_sixteen_holes_before_editor_loading() {
    let mut state =
        EditorState { harmonica_kind: HarmonicaKind::Chromatic16, ..Default::default() };
    select_or_add(&mut state, 1, 0);
    let mut value: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    value["harmonica"]["holes"] = serde_json::json!(17);

    let error = validated_harpchart(&value.to_string()).expect_err("17 holes must fail");
    assert!(error.contains("Chart validation failed"));
    assert!(error.contains("holes"));
}

#[test]
fn editor_load_validation_applies_legacy_migrations() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    let mut value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    value["metadata"]["format_version"] = serde_json::json!("1.0.0");
    value["fx_mapping"] = serde_json::json!({ "bend": "pitch_bend" });

    let migrated = validated_harpchart(&value.to_string()).expect("legacy chart migrates");
    assert!(migrated.get("fx_mapping").is_none());
    assert_eq!(
        migrated["metadata"]["format_version"],
        harmonicon_core::chart::CURRENT_FORMAT_VERSION
    );
}

#[test]
fn editor_load_validation_accepts_its_own_expression_intensity() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    apply_modifier(&mut state, ModButton::Vibrato);
    validated_harpchart(&serialize_harpchart(&state))
        .expect("the editor must accept a chart it wrote itself");
}

/// Every real chart within reach: the fixture packs and — when they are
/// checked out beside this repository, as
/// `../harmonicon-lessons` and `../harmonicon-songs` — the official packs.
/// CI checks out only this repository and so covers the fixtures; a local
/// run with the packs beside it covers every chart a player can get.
fn content_charts() -> Vec<std::path::PathBuf> {
    fn charts_below(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue; // a checkout's .git
            }
            if path.is_dir() {
                charts_below(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "harpchart") {
                out.push(path);
            }
        }
    }

    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for dir in [
        repo.join("tests/fixtures"),
        repo.join("../harmonicon-lessons"),
        repo.join("../harmonicon-songs"),
    ] {
        charts_below(&dir, &mut paths);
    }
    assert!(!paths.is_empty(), "no charts found under {}", repo.display());
    paths
}

#[test]
fn every_bundled_chart_loads_and_resaves_as_a_valid_chart() {
    let paths = content_charts();

    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let value = validated_harpchart(&text)
            .unwrap_or_else(|error| panic!("{} cannot be edited: {error}", path.display()));
        let source_events: usize = value["track"]
            .as_array()
            .unwrap()
            .iter()
            .map(|phrase| phrase["events"].as_array().map_or(0, Vec::len))
            .sum();
        let mut state = EditorState::default();
        load_harpchart(&value, &mut state, &mut Scroll::default());
        assert_eq!(state.notes.len(), source_events, "{} lost notes", path.display());
        let saved = serialize_harpchart(&state);
        validated_harpchart(&saved)
            .unwrap_or_else(|error| panic!("{} resaved invalidly: {error}", path.display()));
    }
}

// ── semantic round-trip of every bundled chart ────────────────────────────────
//
// `every_bundled_chart_loads_and_resaves_as_a_valid_chart` above proves the
// resave is *valid* and keeps the note count. This proves it keeps the
// *meaning*: every event's onset, duration, hole, breath, pitch and
// modifiers, every phrase label, and every song/harmonica/scoring/loop
// field — allowing only the normalisation the editor does on purpose:
//
// - phrase `id`s are regenerated;
// - onsets are converted from `time` (seconds) or a foreign `tick`
//   resolution to the editor's own ticks, so both are compared in *beats*;
// - durations are seconds in both, re-derived from ticks on save, so they
//   are compared in beats at the grid's own precision;
// - numbers may change representation (`180` vs `180.0`);
// - phrases may be regrouped (the editor groups by onset *and* duration),
//   so the track is compared as a flat set of events, with each phrase's
//   own labels carried down onto its events;
// - `format_version` is bumped.
//
// Anything else that differs is data the editor silently rewrote, which is
// the one thing the audit set out to make impossible.

/// One event as it means to a player, in units that survive the editor's
/// representation changes.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
struct CanonicalEvent {
    onset_beats: f64,
    duration_beats: f64,
    hole: u64,
    action: String,
    modifiers: Vec<String>,
    section: Option<String>,
    chord: Option<String>,
    groove: Option<String>,
    call: bool,
    play_mode: String,
}

/// Rounds to the editor's grid (a twelfth of a beat) so a value that
/// survives quantisation compares equal, and one that doesn't shows up.
fn grid_beats(beats: f64) -> f64 {
    (beats * TICKS_PER_BEAT as f64).round() / TICKS_PER_BEAT as f64
}

fn canonical_events(chart: &serde_json::Value) -> Vec<CanonicalEvent> {
    use harmonicon_core::chart::{TempoPoint, seconds_to_tick, tick_to_seconds};
    let resolution = chart["timing"]["resolution"].as_u64().unwrap_or(480) as u32;
    let tempo_map: Vec<TempoPoint> =
        serde_json::from_value(chart["timing"]["tempo_map"].clone()).unwrap_or_default();
    let onset_ticks = |phrase: &serde_json::Value| -> u64 {
        if let Some(t) = phrase["tick"].as_u64() {
            t
        } else {
            seconds_to_tick(phrase["time"].as_f64().unwrap_or(0.0), resolution, &tempo_map)
        }
    };
    let mut out = Vec::new();
    for phrase in chart["track"].as_array().into_iter().flatten() {
        let tick = onset_ticks(phrase);
        let onset_secs = tick_to_seconds(tick, resolution, &tempo_map);
        let end_secs = onset_secs + phrase["duration"].as_f64().unwrap_or(0.0);
        let end_tick = seconds_to_tick(end_secs, resolution, &tempo_map);
        let onset_beats = grid_beats(tick as f64 / resolution as f64);
        let duration_beats = grid_beats(end_tick.saturating_sub(tick) as f64 / resolution as f64);
        for event in phrase["events"].as_array().into_iter().flatten() {
            let mut modifiers: Vec<String> = event["modifiers"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|m| {
                    // Numbers inside a modifier (bend semitones, hz,
                    // intensity) go through the same representation
                    // normalisation as everything else.
                    let mut modifier = m.clone();
                    if matches!(modifier["type"].as_str(), Some("vibrato" | "wah-wah"))
                        && modifier.get("intensity").is_none()
                    {
                        modifier["intensity"] = serde_json::json!(0.5);
                    }
                    normalize_numbers(&modifier).to_string()
                })
                .collect();
            modifiers.sort();
            out.push(CanonicalEvent {
                onset_beats,
                duration_beats,
                hole: event["hole"].as_u64().unwrap_or(0),
                action: event["action"].as_str().unwrap_or("").to_string(),
                modifiers,
                section: phrase["phrase"].as_str().map(str::to_owned),
                chord: phrase["chord"].as_str().map(str::to_owned),
                groove: phrase["groove"].as_str().map(str::to_owned),
                call: phrase["call"].as_bool() == Some(true),
                play_mode: phrase["play_mode"].as_str().unwrap_or("single").to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Every number as an `f64` rounded to six places, so `180` and `180.0`
/// (and `0.6667` re-derived as `0.666667`) compare equal.
fn normalize_numbers(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            serde_json::json!((f * 1e6).round() / 1e6)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize_numbers).collect()),
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), normalize_numbers(v))).collect())
        }
        other => other.clone(),
    }
}

/// The tempo/meter maps with their ticks in beats, since the two charts
/// are at different resolutions.
fn canonical_timing(chart: &serde_json::Value) -> serde_json::Value {
    let resolution = chart["timing"]["resolution"].as_f64().unwrap_or(480.0);
    let in_beats = |points: &serde_json::Value, value_key: &str| -> serde_json::Value {
        let mut v: Vec<(f64, serde_json::Value)> = points
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                (
                    grid_beats(p["tick"].as_f64().unwrap_or(0.0) / resolution),
                    normalize_numbers(&p[value_key]),
                )
            })
            .collect();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        serde_json::json!(v)
    };
    serde_json::json!({
        "tempo_map": in_beats(&chart["timing"]["tempo_map"], "bpm"),
        "time_signature_map": in_beats(&chart["timing"]["time_signature_map"], "time_signature"),
    })
}

/// Everything about a chart that isn't the track, minus what the editor
/// rewrites on purpose.
fn canonical_rest(chart: &serde_json::Value) -> serde_json::Value {
    let mut metadata = normalize_numbers(&chart["metadata"]);
    if let Some(m) = metadata.as_object_mut() {
        m.remove("format_version");
    }
    let mut harmonica = normalize_numbers(&chart["harmonica"]);
    if harmonica.get("scale").is_none() {
        harmonica["scale"] = serde_json::json!("first_position");
    }
    if harmonica.get("position").is_none() {
        harmonica["position"] = serde_json::json!("2nd");
    }
    let mut scoring = normalize_numbers(&chart["scoring"]);
    if scoring.get("combo").is_none() {
        scoring["combo"] = serde_json::json!({
            "enabled": true,
            "base_multiplier": 1.0,
            "step_multiplier": 0.1,
            "max_multiplier": 4.0,
            "decay_ms": 2000.0,
        });
    }
    let mut loop_settings = normalize_numbers(&chart["loop"]);
    if loop_settings.is_null() {
        loop_settings = serde_json::json!({"type": "full", "repeat": false});
    }
    if loop_settings["type"] == "full" && loop_settings["repeat"] == false {
        loop_settings.as_object_mut().unwrap().remove("start_index");
        loop_settings.as_object_mut().unwrap().remove("end_index");
    }
    serde_json::json!({
        "song": normalize_numbers(&chart["song"]),
        "harmonica": harmonica,
        "scoring": scoring,
        "loop": loop_settings,
        "metadata": metadata,
    })
}

/// Loads `source` into a fresh editor, saves it, and describes every way
/// the result differs in meaning from the source — empty when none.
fn round_trip_differences(source: &serde_json::Value) -> Vec<String> {
    let mut state = EditorState::default();
    load_harpchart(source, &mut state, &mut Scroll::default());
    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    let mut diffs = Vec::new();

    let before = canonical_events(source);
    let after = canonical_events(&saved);
    for e in before.iter().filter(|e| !after.contains(e)) {
        diffs.push(format!("event lost or changed on save: {e:?}"));
    }
    for e in after.iter().filter(|e| !before.contains(e)) {
        diffs.push(format!("event invented or changed on save: {e:?}"));
    }
    let (t0, t1) = (canonical_timing(source), canonical_timing(&saved));
    if t0 != t1 {
        diffs.push(format!("timing changed: {t0} -> {t1}"));
    }
    let (r0, r1) = (canonical_rest(source), canonical_rest(&saved));
    for key in ["song", "harmonica", "scoring", "loop", "metadata"] {
        if r0[key] != r1[key] {
            diffs.push(format!("{key} changed: {} -> {}", r0[key], r1[key]));
        }
    }
    diffs
}

#[test]
fn a_minor_song_key_survives_loading_and_saving() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    let mut source: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    source["song"]["key"] = serde_json::json!("Am");
    let differences = round_trip_differences(&source);
    assert!(differences.is_empty(), "{differences:?}");
}

#[test]
fn every_bundled_chart_means_the_same_after_a_round_trip() {
    let paths = content_charts();

    let mut report = String::new();
    for path in &paths {
        let text = std::fs::read_to_string(path).unwrap();
        let source = validated_harpchart(&text)
            .unwrap_or_else(|error| panic!("{} cannot be edited: {error}", path.display()));
        let diffs = round_trip_differences(&source);
        if !diffs.is_empty() {
            report.push_str(&format!("\n{}:\n", path.display()));
            for d in diffs {
                report.push_str(&format!("  - {d}\n"));
            }
        }
    }
    assert!(report.is_empty(), "charts whose meaning changed on a round trip:{report}");
}

#[test]
fn non_grid_chart_settings_survive_load_and_save() {
    let mut source = EditorState::default();
    select_or_add(&mut source, 1, 0);
    let mut value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&source)).expect("valid chart JSON");
    value["metadata"]["source"] = serde_json::json!("Traditional");
    value["metadata"]["license"] = serde_json::json!("CC-BY-4.0");
    value["metadata"]["description"] = serde_json::json!("Custom description");
    value["song"]["difficulty"] = serde_json::json!("expert");
    value["song"]["feel"] = serde_json::json!("shuffle");
    value["scoring"] = serde_json::json!({
        "perfect_window_ms": 90,
        "good_window_ms": 180,
        "miss_window_ms": 360
    });
    value["loop"] = serde_json::json!({
        "type": "verse", "repeat": true, "start_index": 0, "end_index": 0
    });

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&value, &mut loaded, &mut scroll);
    assert_eq!(loaded.difficulty, "expert");
    assert_eq!(loaded.song_feel, "shuffle");
    assert_eq!(loaded.source, "Traditional");
    assert_eq!(loaded.license, "CC-BY-4.0");
    assert_eq!(loaded.description, "Custom description");
    assert_eq!(loaded.perfect_window_ms, "90");
    assert_eq!(loaded.good_window_ms, "180");
    assert_eq!(loaded.miss_window_ms, "360");
    assert_eq!(loaded.loop_settings.kind, "verse");
    assert_eq!(loaded.loop_settings.repeat, "yes");
    loaded.difficulty = "advanced".into();
    loaded.song_feel = "straight".into();
    loaded.source = "Field recording".into();
    loaded.license = "CC0".into();
    loaded.description = "Revised description".into();
    loaded.perfect_window_ms = "70".into();
    loaded.good_window_ms = "140".into();
    loaded.miss_window_ms = "280".into();
    loaded.loop_settings.kind = "chorus".into();
    loaded.loop_settings.repeat = "no".into();
    let saved: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&loaded)).expect("saved chart JSON");

    assert_eq!(saved["metadata"]["source"], "Field recording");
    assert_eq!(saved["metadata"]["license"], "CC0");
    assert_eq!(saved["metadata"]["description"], "Revised description");
    assert_eq!(saved["song"]["difficulty"], "advanced");
    assert_eq!(saved["song"]["feel"], "straight");
    assert_eq!(saved["scoring"]["perfect_window_ms"], 70);
    assert_eq!(saved["scoring"]["good_window_ms"], 140);
    assert_eq!(saved["scoring"]["miss_window_ms"], 280);
    assert_eq!(saved["loop"]["type"], "chorus");
    assert_eq!(saved["loop"]["repeat"], false);
    validated_harpchart(&saved.to_string()).expect("preserved settings remain valid");
}

#[test]
fn chart_author_survives_when_it_differs_from_the_artist() {
    let mut source: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&EditorState::default())).unwrap();
    source["metadata"]["author"] = serde_json::json!("Chart Transcriber");
    source["song"]["artist"] = serde_json::json!("Performing Artist");

    let mut state = EditorState::default();
    load_harpchart(&source, &mut state, &mut Scroll::default());
    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();

    assert_eq!(saved["metadata"]["author"], "Chart Transcriber");
    assert_eq!(saved["song"]["artist"], "Performing Artist");
}

#[test]
fn loop_indices_are_clamped_to_the_current_phrase_range() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    state.loop_settings.start = "99".into();
    state.loop_settings.end = "before".into();

    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    assert_eq!(saved["loop"]["start_index"], 0);
    assert_eq!(saved["loop"]["end_index"], 0);
    validated_harpchart(&saved.to_string()).expect("clamped loop satisfies the schema");
}

#[test]
fn invalid_scoring_window_text_falls_back_to_schema_valid_defaults() {
    let mut state = EditorState {
        perfect_window_ms: "zero".into(),
        good_window_ms: "0".into(),
        miss_window_ms: "-1".into(),
        ..Default::default()
    };
    select_or_add(&mut state, 1, 0);
    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    assert_eq!(saved["scoring"]["perfect_window_ms"], 60);
    assert_eq!(saved["scoring"]["good_window_ms"], 120);
    assert_eq!(saved["scoring"]["miss_window_ms"], 220);
    validated_harpchart(&saved.to_string()).expect("fallback windows satisfy the schema");
}

#[test]
fn combo_settings_are_editable_while_style_bonuses_round_trip() {
    let mut source = EditorState::default();
    select_or_add(&mut source, 1, 0);
    let mut value: serde_json::Value = serde_json::from_str(&serialize_harpchart(&source)).unwrap();
    value["scoring"]["combo"] = serde_json::json!({
        "enabled": false,
        "base_multiplier": 1.5,
        "step_multiplier": 0.25,
        "max_multiplier": 6.0,
        "decay_ms": 3000
    });
    value["scoring"]["style_bonus"] = serde_json::json!({ "bend": 75 });

    let mut loaded = EditorState::default();
    load_harpchart(&value, &mut loaded, &mut Scroll::default());
    assert_eq!(loaded.combo.enabled, "disabled");
    assert_eq!(loaded.combo.base, "1.5");
    loaded.combo.enabled = "enabled".into();
    loaded.combo.max = "8".into();

    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&loaded)).unwrap();
    assert_eq!(saved["scoring"]["combo"]["enabled"], true);
    assert_eq!(saved["scoring"]["combo"]["max_multiplier"], 8.0);
    assert_eq!(saved["scoring"]["style_bonus"], value["scoring"]["style_bonus"]);
    validated_harpchart(&saved.to_string()).expect("edited combo satisfies the schema");
}

#[test]
fn default_song_feel_is_omitted_from_saved_charts() {
    let saved: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&EditorState::default())).unwrap();
    assert!(saved["song"].get("feel").is_none());
}

#[test]
fn editor_load_validation_lists_only_semantics_it_cannot_preserve() {
    let mut state = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
    select_or_add(&mut state, 1, 0);
    let mut value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    value["timing"]["time_signature_map"] =
        serde_json::json!([{ "tick": 0, "time_signature": "4/4" }]);
    value["track"][0]["events"][0]["modifiers"] = serde_json::json!([
        { "type": "bend", "semitones": -1.0 },
        { "type": "overblow" }
    ]);
    let mut expression_event = value["track"][0]["events"][0].clone();
    expression_event["modifiers"] = serde_json::json!([
        { "type": "vibrato", "oscillation_hz": 5.0 },
        { "type": "wah-wah", "oscillation_hz": 4.0 }
    ]);
    value["track"][0]["events"].as_array_mut().unwrap().push(expression_event);
    value["track"][0]["groove"] = serde_json::json!("laid-back");
    value["track"][0]["call"] = serde_json::json!(true);
    value["track"][0]["play_mode"] = serde_json::json!("split");

    let error = validated_harpchart(&value.to_string()).expect_err("unsupported chart must fail");
    assert!(error.contains("multiple mutually exclusive pitch techniques"), "{error}");
    assert!(error.contains("multiple mutually exclusive expressions"), "{error}");
    assert!(!error.contains("time-signature changes"));
    assert!(!error.contains("groove annotation"));
    assert!(!error.contains("call-and-response"));
    assert!(!error.contains("split play mode"));
}

#[test]
fn custom_expression_intensity_round_trips() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    apply_modifier(&mut state, ModButton::Vibrato);
    let mut value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    value["track"][0]["events"][0]["modifiers"][0]["intensity"] = serde_json::json!(0.9);

    validated_harpchart(&value.to_string()).expect("custom intensity is preserved");
    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&value, &mut loaded, &mut scroll);
    loaded.select_only(loaded.notes[0].id);
    assert_eq!(loaded.selected_expression_intensity(), "0.9");

    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&loaded)).unwrap();
    assert_eq!(saved["track"][0]["events"][0]["modifiers"][0]["intensity"], 0.9);
}

#[test]
fn expression_intensity_can_be_authored_only_on_an_expression_note() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    state.set_selected_expression_intensity("0.75".into());
    assert!(state.expression_intensities.is_empty());

    apply_modifier(&mut state, ModButton::Vibrato);
    state.set_selected_expression_intensity("0.75".into());
    assert_eq!(state.selected_expression_intensity(), "0.75");
    state.set_selected_expression_intensity("0.5".into());
    assert!(state.expression_intensities.is_empty());
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
    let modifiers = v["track"][0]["events"][0]["modifiers"].as_array().expect("modifiers array");
    let vibrato = modifiers.iter().find(|m| m["type"] == "vibrato").expect("vibrato modifier");
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
fn scale_round_trips_through_save_and_load() {
    let s = EditorState { scale: Scale::SecondPosition, ..Default::default() };

    let json_str = serialize_harpchart(&s);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    assert_eq!(v["harmonica"]["scale"], "second_position");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);
    assert_eq!(loaded.scale, Scale::SecondPosition);
}

#[test]
fn phrase_annotations_round_trip() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 2, TICKS_PER_BEAT);
    state.phrase_annotations.insert(
        TICKS_PER_BEAT,
        PhraseAnnotation {
            section: Some("Verse A".into()),
            chord: Some("G7alt".into()),
            groove: Some("laid-back shuffle".into()),
            lyric: Some("A-".into()),
            call: true,
            split: true,
        },
    );

    let text = serialize_harpchart(&state);
    validated_harpchart(&text).expect("annotations are supported editor semantics");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["track"][0]["phrase"], "Verse A");
    assert_eq!(value["track"][0]["chord"], "G7alt");
    assert_eq!(value["track"][0]["groove"], "laid-back shuffle");
    assert_eq!(value["track"][0]["lyric"], "A-");
    assert_eq!(value["track"][0]["call"], true);
    assert_eq!(value["track"][0]["play_mode"], "split");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&value, &mut loaded, &mut scroll);
    assert_eq!(loaded.phrase_annotations, state.phrase_annotations);
}

#[test]
fn a_typed_line_of_lyrics_lands_one_syllable_per_onset() {
    let mut state = EditorState::default();
    for beat in 0..6 {
        select_or_add(&mut state, 4, beat * TICKS_PER_BEAT);
    }
    // A second note on beat 1's onset: still one onset, one syllable.
    select_or_add(&mut state, 5, TICKS_PER_BEAT);
    state.set_annotation(4 * TICKS_PER_BEAT, Field::Lyric, "old".into());

    state.set_lyrics_from(TICKS_PER_BEAT, "A- maz- _ ing");
    let lyric = |state: &EditorState, beat: usize| {
        state.annotation_text(beat * TICKS_PER_BEAT, Field::Lyric).to_string()
    };
    assert_eq!(
        (0..6).map(|beat| lyric(&state, beat)).collect::<Vec<_>>(),
        vec!["", "A-", "maz-", "", "ing", ""],
        "the held beat is skipped, and what was there is replaced"
    );

    state.set_lyrics_from(TICKS_PER_BEAT, "  grace ");
    assert_eq!(lyric(&state, 1), "grace", "one syllable is just this onset");
    state.set_lyrics_from(TICKS_PER_BEAT, "");
    assert_eq!(lyric(&state, 1), "");
    assert_eq!(lyric(&state, 2), "maz-", "clearing touches only this onset");
}

#[test]
fn a_lyric_is_saved_once_when_its_onset_splits_into_several_items() {
    // Two notes starting together with different lengths save as two chart
    // items; only one of them may carry the syllable.
    let mut state = EditorState::default();
    select_or_add(&mut state, 4, 0);
    select_or_add(&mut state, 5, 0);
    state.notes[1].len *= 2;
    state.set_annotation(0, Field::Lyric, "grace".into());
    let value: serde_json::Value = serde_json::from_str(&serialize_harpchart(&state)).unwrap();
    let track = value["track"].as_array().unwrap();
    assert_eq!(track.len(), 2);
    let sung: Vec<_> = track.iter().filter(|item| item.get("lyric").is_some()).collect();
    assert_eq!(sung.len(), 1);
}

#[test]
fn phrase_annotation_text_follows_the_selected_notes_onset_and_clears_cleanly() {
    // `field_text` for the three phrase fields reads the *selected* note's
    // phrase — what the phrase editor's boxes showed through Details once,
    // and what `depth_for_button`-style previews still key on.
    let mut state = EditorState::default();
    select_or_add(&mut state, 2, TICKS_PER_BEAT);
    state.set_annotation(TICKS_PER_BEAT, Field::Section, "Verse".into());
    state.set_annotation(TICKS_PER_BEAT, Field::Chord, "G7".into());
    state.set_annotation(TICKS_PER_BEAT, Field::Groove, "behind the beat".into());
    assert_eq!(state.field_text(Field::Section), "Verse");
    assert_eq!(state.field_text(Field::Chord), "G7");
    assert_eq!(state.field_text(Field::Groove), "behind the beat");

    state.selected.clear();
    assert_eq!(state.field_text(Field::Section), "");
    state.set_annotation(TICKS_PER_BEAT, Field::Section, String::new());
    state.set_annotation(TICKS_PER_BEAT, Field::Chord, "  ".into());
    state.set_annotation(TICKS_PER_BEAT, Field::Groove, String::new());
    assert!(state.phrase_annotations.is_empty());
}

#[test]
fn annotation_commit_on_an_onset_with_no_note_is_ignored() {
    let mut state = EditorState::default();
    state.set_annotation(0, Field::Chord, "Cmaj7".into());
    assert!(state.phrase_annotations.is_empty());
}

#[test]
fn selected_call_annotation_follows_selection_and_clears_cleanly() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 2, TICKS_PER_BEAT);
    state.set_selected_call(true);
    assert!(state.selected_call());

    state.selected.clear();
    assert!(!state.selected_call());
    state.select_only(state.notes[0].id);
    state.set_selected_call(false);
    assert!(state.phrase_annotations.is_empty());
}

#[test]
fn selected_split_annotation_follows_selection_and_clears_cleanly() {
    let mut state = EditorState::default();
    select_or_add(&mut state, 1, 0);
    select_or_add(&mut state, 4, 0);
    state.set_selected_split(true);
    assert!(state.selected_split());

    state.selected.clear();
    assert!(!state.selected_split());
    state.select_only(state.notes[0].id);
    state.set_selected_split(false);
    assert!(state.phrase_annotations.is_empty());
}

#[test]
fn loading_a_chart_without_a_scale_field_leaves_the_current_scale_untouched() {
    // Matches `position`'s existing precedent: a missing field doesn't
    // reset the editor's current selection, since `load_harpchart` never
    // resets `EditorState` wholesale before applying fields piecemeal.
    let s = EditorState::default();
    let json_str = serialize_harpchart(&s);
    let mut v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    v["harmonica"].as_object_mut().unwrap().remove("scale");

    let mut loaded = EditorState { scale: Scale::Country, ..Default::default() };
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);
    assert_eq!(loaded.scale, Scale::Country);
}

#[test]
fn chromatic_chart_round_trips_kind_hole_count_and_slide() {
    let mut s = EditorState { harmonica_kind: HarmonicaKind::Chromatic, ..Default::default() };
    select_or_add(&mut s, 11, 0); // only valid on a chromatic (12-hole) harp
    apply_modifier(&mut s, ModButton::Slide);

    let json_str = serialize_harpchart(&s);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    assert_eq!(v["harmonica"]["type"], "chromatic");
    assert_eq!(v["harmonica"]["holes"], 12);
    assert_eq!(v["track"][0]["events"][0]["modifiers"][0]["type"], "slide");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);
    assert_eq!(loaded.harmonica_kind, HarmonicaKind::Chromatic);
    assert_eq!(loaded.notes[0].hole, 11);
    assert_eq!(loaded.notes[0].pitch, Pitch::Slide);
}

#[test]
fn sixteen_hole_chromatic_round_trips_high_holes_and_slide() {
    let mut state =
        EditorState { harmonica_kind: HarmonicaKind::Chromatic16, ..Default::default() };
    select_or_add(&mut state, 16, 0);
    apply_modifier(&mut state, ModButton::Slide);

    let text = serialize_harpchart(&state);
    validated_harpchart(&text).expect("the editor must accept its 16-hole chart");
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(value["harmonica"]["holes"], 16);
    assert_eq!(value["harmonica"]["layout"]["blow"].as_array().unwrap().len(), 16);

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&value, &mut loaded, &mut scroll);
    assert_eq!(loaded.harmonica_kind, HarmonicaKind::Chromatic16);
    assert_eq!(loaded.notes[0].hole, 16);
    assert_eq!(loaded.notes[0].pitch, Pitch::Slide);
}

#[test]
fn alternate_diatonic_tunings_round_trip_profile_and_layout() {
    for (kind, profile, hole_three_blow, hole_five_draw) in [
        (HarmonicaKind::PaddyRichter, "paddy_richter", "A4", "F5"),
        (HarmonicaKind::CountryTuned, "country_tuned", "G4", "F#5"),
        (HarmonicaKind::NaturalMinor, "natural_minor", "G4", "F5"),
    ] {
        let mut state = EditorState { harmonica_kind: kind, ..Default::default() };
        select_or_add(&mut state, 1, 0);
        let text = serialize_harpchart(&state);
        validated_harpchart(&text).expect("the editor must accept its alternate tuning");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["harmonica"]["bending_profile"], profile);
        assert_eq!(value["harmonica"]["layout"]["blow"][2], hole_three_blow);
        assert_eq!(value["harmonica"]["layout"]["draw"][4], hole_five_draw);

        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&value, &mut loaded, &mut scroll);
        assert_eq!(loaded.harmonica_kind, kind);
    }
}

/// A C diatonic chart whose blow-1 reed has been re-tuned — the smallest
/// possible custom layout, enough to tell the loaded reeds from the preset.
fn state_with_custom_layout() -> EditorState {
    let mut source = EditorState::default();
    select_or_add(&mut source, 1, 0);
    let mut value: serde_json::Value = serde_json::from_str(&serialize_harpchart(&source)).unwrap();
    value["harmonica"]["layout"]["blow"][0] = serde_json::json!("F#3");
    let mut loaded = EditorState::default();
    load_harpchart(&value, &mut loaded, &mut Scroll::default());
    assert!(loaded.loaded_harmonica.is_some(), "precondition: layout retained");
    loaded
}

fn blow_one_of(state: &EditorState) -> String {
    state.effective_harp().wind_direction_label(1, &harmonicon_core::chart::Action::Blow)
}

#[test]
fn the_loaded_layout_survives_reselecting_the_same_key_and_kind() {
    // `set_key`/`set_harmonica_kind` are *intentional* instrument changes;
    // re-picking what is already selected is not one, and must not throw
    // the chart's reeds away.
    let mut s = state_with_custom_layout();
    let key = s.key.clone();
    let kind = s.harmonica_kind;
    s.set_key(key);
    s.set_harmonica_kind(kind);
    assert_eq!(blow_one_of(&s), "F#3");
}

#[test]
fn changing_the_key_drops_the_loaded_layout_for_the_preset() {
    let mut s = state_with_custom_layout();
    s.set_key("D".into());
    assert!(s.loaded_harmonica.is_none());
    // The preset D harp's blow 1 — not the custom reed, and not C's.
    assert_eq!(blow_one_of(&s), "D4");
}

#[test]
fn changing_the_kind_drops_the_loaded_layout_for_the_preset() {
    let mut s = state_with_custom_layout();
    s.set_harmonica_kind(HarmonicaKind::PaddyRichter);
    assert!(s.loaded_harmonica.is_none());
    assert_ne!(blow_one_of(&s), "F#3");
}

#[test]
fn a_layout_loaded_for_one_key_is_ignored_if_the_key_no_longer_matches() {
    // Belt and braces for `effective_harp`'s own guard: even if something
    // writes `key` directly rather than through `set_key`, reeds loaded
    // for C must not be used as if they were an A harp's.
    let mut s = state_with_custom_layout();
    s.key = "A".into();
    assert_eq!(blow_one_of(&s), "A3");
}

#[test]
fn a_midi_import_drops_the_loaded_layout_even_when_the_key_matches() {
    // Import resolves every pitch against the *preset* for the suggested
    // key (`import_track_notes` builds that harp itself). If the suggested
    // key happens to equal the chart's, `set_key` alone would keep the
    // custom reeds — and the imported holes would then be read by a layout
    // they weren't placed with.
    let mut s = state_with_custom_layout();
    s.meter_changes.push((24, "7/8".into()));
    let key = s.key.clone();
    let imported = super::midi_import::ImportedTrack {
        initial_bpm: 100.0,
        time_signature: "4/4".into(),
        meter_changes: Vec::new(),
        tempo_changes: Vec::new(),
        notes: Vec::new(),
        diagnostics: Default::default(),
    };
    super::midi_import::apply_imported_track(&mut s, imported, &key);
    assert!(s.loaded_harmonica.is_none());
    assert!(s.meter_changes.is_empty());
    assert_eq!(blow_one_of(&s), "C4");
}

#[test]
fn custom_layouts_round_trip_for_every_named_harmonica() {
    for kind in [
        HarmonicaKind::Diatonic,
        HarmonicaKind::PaddyRichter,
        HarmonicaKind::CountryTuned,
        HarmonicaKind::NaturalMinor,
        HarmonicaKind::Chromatic,
        HarmonicaKind::Chromatic16,
    ] {
        let mut source = EditorState { harmonica_kind: kind, ..Default::default() };
        select_or_add(&mut source, 1, 0);
        let mut value: serde_json::Value =
            serde_json::from_str(&serialize_harpchart(&source)).unwrap();
        value["harmonica"]["layout"]["blow"][0] = serde_json::json!("F#3");
        let expected = value["harmonica"]["layout"].clone();

        let mut loaded = EditorState::default();
        let mut scroll = Scroll::default();
        load_harpchart(&value, &mut loaded, &mut scroll);
        let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&loaded)).unwrap();

        assert_eq!(saved["harmonica"]["layout"], expected, "{kind:?}");
        assert_eq!(
            loaded.effective_harp().wind_direction_label(1, &harmonicon_core::chart::Action::Blow),
            "F#3",
            "{kind:?}"
        );
    }
}

#[test]
fn country_tuned_editor_harp_raises_draw_five() {
    let harp = build_harp("C", HarmonicaKind::CountryTuned);
    assert_eq!(harp.wind_direction_label(5, &harmonicon_core::chart::Action::Draw), "F#5");
}

#[test]
fn natural_minor_editor_harp_uses_minor_reeds() {
    let harp = build_harp("C", HarmonicaKind::NaturalMinor);
    assert_eq!(harp.wind_direction_label(2, &harmonicon_core::chart::Action::Blow), "D#4");
    assert_eq!(harp.wind_direction_label(3, &harmonicon_core::chart::Action::Draw), "A#4");
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
    let s = EditorState { position: "3rd".into(), ..Default::default() };

    let json_str = serialize_harpchart(&s);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    assert_eq!(v["harmonica"]["position"], "3rd");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);
    assert_eq!(loaded.position, "3rd");
}

#[test]
fn serialize_harpchart_writes_every_tempo_change_point() {
    let s = EditorState {
        tempo: "120".into(),
        tempo_changes: vec![(960, 180.0)],
        ..Default::default()
    };
    let json_str = serialize_harpchart(&s);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    let map = v["timing"]["tempo_map"].as_array().expect("tempo_map array");
    assert_eq!(map.len(), 2);
    assert_eq!(map[0]["tick"], 0);
    assert_eq!(map[0]["bpm"], 120.0);
    assert_eq!(map[1]["tick"], 960);
    assert_eq!(map[1]["bpm"], 180.0);
}

#[test]
fn a_multi_point_tempo_map_round_trips_through_save_and_load() {
    let s = EditorState {
        tempo: "120".into(),
        tempo_changes: vec![(960, 180.0)],
        ..Default::default()
    };
    let json_str = serialize_harpchart(&s);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);

    assert_eq!(loaded.tempo, "120");
    assert_eq!(loaded.tempo_changes, vec![(960, 180.0)]);
}

#[test]
fn a_meter_map_round_trips_and_rescales_from_a_foreign_resolution() {
    let mut source = EditorState::default();
    select_or_add(&mut source, 1, 0);
    let mut value: serde_json::Value = serde_json::from_str(&serialize_harpchart(&source)).unwrap();
    value["timing"]["resolution"] = serde_json::json!(960);
    value["timing"]["time_signature_map"] = serde_json::json!([
        { "tick": 0, "time_signature": "6/8" },
        { "tick": 3360, "time_signature": "7/8" }
    ]);
    validated_harpchart(&value.to_string()).expect("meter maps are editable");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&value, &mut loaded, &mut scroll);
    assert_eq!(loaded.time_signature, "6/8");
    assert_eq!(loaded.meter_changes, vec![(42, "7/8".into())]);

    let saved: serde_json::Value = serde_json::from_str(&serialize_harpchart(&loaded)).unwrap();
    assert_eq!(
        saved["timing"]["time_signature_map"],
        serde_json::json!([
            { "tick": 0, "time_signature": "6/8" },
            { "tick": 42, "time_signature": "7/8" }
        ])
    );
}

#[test]
fn editor_meter_map_always_has_tick_zero_and_sorts_later_changes() {
    let state = EditorState {
        time_signature: "4/4".into(),
        meter_changes: vec![(84, "7/8".into()), (48, "3/4".into())],
        ..Default::default()
    };
    let map = state.time_signature_map();
    assert_eq!(map[0].tick, 0);
    assert_eq!(map[0].time_signature, "4/4");
    assert_eq!(map[1].tick, 48);
    assert_eq!(map[2].tick, 84);
}

#[test]
fn a_note_placed_after_a_tempo_change_keeps_its_tick_across_save_and_load() {
    let mut s = EditorState {
        tempo: "120".into(),
        tempo_changes: vec![(960, 180.0)],
        ..Default::default()
    };
    // Tick 960 is exactly the tempo-change boundary; this note starts a
    // beat later, well inside the faster section.
    select_or_add(&mut s, 3, 960 + TICKS_PER_BEAT);

    let json_str = serialize_harpchart(&s);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");

    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);

    assert_eq!(loaded.notes.len(), 1);
    assert_eq!(loaded.notes[0].tick, 960 + TICKS_PER_BEAT);
}

#[test]
fn loading_a_foreign_resolution_rescales_ticks_into_the_editors_own_unit() {
    // A chart authored at MIDI-style resolution 480 (4x the editor's own
    // TICKS_PER_BEAT of 4) with a note at tick 480 (one beat in) and a
    // tempo change at tick 960 (two beats in).
    let v = serde_json::json!({
        "song": { "tempo_bpm": 120.0 },
        "timing": {
            "resolution": 480,
            "tempo_map": [{"tick": 0, "bpm": 120.0}, {"tick": 960, "bpm": 180.0}]
        },
        "track": [
            {"tick": 480, "duration": 0.5, "events": [{"hole": 3, "action": "blow"}]}
        ]
    });
    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);

    // 480 file-ticks * (4 editor-ticks / 480 file-ticks) = 4 editor-ticks.
    assert_eq!(loaded.notes[0].tick, TICKS_PER_BEAT);
    // 960 file-ticks -> 8 editor-ticks.
    assert_eq!(loaded.tempo_changes, vec![(2 * TICKS_PER_BEAT, 180.0)]);
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
    assert_eq!(none.alpha, 0.5, "base's own alpha is preserved, not blended");

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
    let natural = GridNote { dir: Dir::Draw, ..GridNote::plain(0, 3, 0, 1) };
    assert!(
        !note_in_scale(&natural, &harp, &scale),
        "unbent B (major 7th) is outside the blues scale"
    );

    // Bending draw-3 down a step-and-a-half reaches Bb (the ♭7) — exactly
    // how a blues player accesses that blue note. Should read as in-scale.
    let bent = GridNote { dir: Dir::Draw, pitch: Pitch::Bend(1.5), ..GridNote::plain(0, 3, 0, 1) };
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
    assert_eq!(parse_pitch_expr(&[serde_json::json!({ "type": "overblow" })]).0, Pitch::Overblow);
    assert_eq!(parse_pitch_expr(&[serde_json::json!({ "type": "overdraw" })]).0, Pitch::Overdraw);
    // No `oscillation_hz` in the JSON (e.g. a chart saved before it was
    // per-note) falls back to the default rate.
    assert_eq!(parse_pitch_expr(&[serde_json::json!({ "type": "vibrato" })]).1, Expr::Vibrato(5.5));
    assert_eq!(parse_pitch_expr(&[serde_json::json!({ "type": "wah-wah" })]).1, Expr::Wah(4.0));
    assert_eq!(parse_pitch_expr(&[serde_json::json!({ "type": "slide" })]).0, Pitch::Slide);
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
    let note = GridNote::plain(0, 1, 0, 1);
    let (left, top, width, height) = note_rect(&note);
    assert_eq!(left, 1.0);
    assert_eq!(top, HEADER_H + NOTE_PAD);
    assert_eq!(width, TICK_W - 2.0);
    assert_eq!(height, ROW_H - 2.0 * NOTE_PAD);
}

#[test]
fn note_rect_advances_one_row_per_hole_and_scales_width_with_len() {
    let a = GridNote::plain(0, 1, 0, 3);
    let b = GridNote::plain(1, 2, 0, 3);
    let (_, top_a, width_a, _) = note_rect(&a);
    let (_, top_b, width_b, _) = note_rect(&b);
    assert_eq!(top_b - top_a, ROW_H, "hole 2 sits exactly one row below hole 1");
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

// ── the ruler's bar / beat numbering ─────────────────────────────────────────
//
// The ruler now reads `EditorState::meter_map()` directly; these pin the
// answers it draws, through the same `position` the grid asks.

/// "bar.beat" as the ruler numbers it (1-based) for `tick`; a pickup's bar
/// reads as 0.
fn ruler_position(s: &EditorState, tick: usize) -> (usize, usize) {
    let map = s.meter_map();
    let p = map.position(tick as u64);
    (map.bar_number(p.bar).unwrap_or(0), p.beat + 1)
}

// ── pickups ──────────────────────────────────────────────────────────────────

#[test]
fn a_pickup_is_typed_in_beats_of_the_opening_meter() {
    let with = |beats: &str, meter: &str| EditorState {
        pickup_beats: beats.into(),
        time_signature: meter.into(),
        ..Default::default()
    };
    assert_eq!(with("1", "4/4").pickup_ticks(), TICKS_PER_BEAT);
    assert_eq!(with("0.5", "4/4").pickup_ticks(), TICKS_PER_BEAT / 2);
    assert_eq!(with("1", "6/8").pickup_ticks(), TICKS_PER_BEAT / 2, "a 6/8 beat is an eighth");
    for blank in ["", "abc", "-1", "0"] {
        assert_eq!(with(blank, "4/4").pickup_ticks(), 0, "{blank:?}");
    }
}

#[test]
fn the_ruler_numbers_a_pickup_as_the_last_beat_of_bar_zero() {
    let s = EditorState { pickup_beats: "1".into(), ..Default::default() };
    assert_eq!(ruler_position(&s, 0), (0, 4));
    assert_eq!(ruler_position(&s, TICKS_PER_BEAT), (1, 1));
    assert_eq!(super::timeline::describe_tick(0, &s.meter_map()), "0.4");
}

#[test]
fn a_pickup_round_trips_through_save_and_load() {
    let state = EditorState { pickup_beats: "0.5".into(), ..Default::default() };
    let value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    assert_eq!(value["timing"]["pickup_ticks"], serde_json::json!(TICKS_PER_BEAT / 2));
    let mut loaded = EditorState::default();
    load_harpchart(&value, &mut loaded, &mut Scroll::default());
    assert_eq!(loaded.pickup_beats, "0.5");

    let none = EditorState::default();
    let value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&none)).expect("valid chart JSON");
    assert!(value["timing"].get("pickup_ticks").is_none(), "no pickup, no field");
}

#[test]
fn a_pickup_at_a_foreign_resolution_is_rescaled_on_load() {
    let value = serde_json::json!({
        "song": {"title": "t", "artist": "a", "tempo_bpm": 100, "key": "C"},
        "timing": {"resolution": 480, "tempo_map": [{"tick": 0, "bpm": 100}], "pickup_ticks": 480},
        "harmonica": {"type": "diatonic", "holes": 10, "bending_profile": "richter_standard"},
        "track": [],
        "scoring": {"perfect_window_ms": 60, "good_window_ms": 120, "miss_window_ms": 220}
    });
    let mut loaded = EditorState::default();
    load_harpchart(&value, &mut loaded, &mut Scroll::default());
    assert_eq!(loaded.pickup_beats, "1", "480 ticks at 480/quarter is a beat");
}

// ── repeats ──────────────────────────────────────────────────────────────────

#[test]
fn repeats_round_trip_as_written_and_rescale_from_a_foreign_resolution() {
    let bar = 4 * TICKS_PER_BEAT as u64;
    let mut state = EditorState::default();
    super::repeat_marks::toggle_repeat(&mut state.repeats, 0, 2 * bar);
    super::repeat_marks::toggle_ending(&mut state.repeats, bar, 2 * bar);
    super::repeat_marks::toggle_ending(&mut state.repeats, 2 * bar, 3 * bar);

    let mut value: serde_json::Value =
        serde_json::from_str(&serialize_harpchart(&state)).expect("valid chart JSON");
    let mut loaded = EditorState::default();
    load_harpchart(&value, &mut loaded, &mut Scroll::default());
    assert_eq!(loaded.repeats, state.repeats, "kept as written, not played out");

    // The same chart at 480 ticks a quarter comes back in editor ticks.
    let scale = 480 / TICKS_PER_BEAT as u64;
    value["timing"]["resolution"] = serde_json::json!(480);
    let repeat = &mut value["timing"]["repeats"][0];
    repeat["end_tick"] = serde_json::json!(2 * bar * scale);
    repeat["endings"][0]["start_tick"] = serde_json::json!(bar * scale);
    repeat["endings"][0]["end_tick"] = serde_json::json!(2 * bar * scale);
    repeat["endings"][1]["start_tick"] = serde_json::json!(2 * bar * scale);
    repeat["endings"][1]["end_tick"] = serde_json::json!(3 * bar * scale);
    let mut rescaled = EditorState::default();
    load_harpchart(&value, &mut rescaled, &mut Scroll::default());
    assert_eq!(rescaled.repeats, state.repeats);

    assert!(
        serde_json::from_str::<serde_json::Value>(&serialize_harpchart(&EditorState::default()))
            .unwrap()["timing"]
            .get("repeats")
            .is_none(),
        "no repeats, no field"
    );
}

#[test]
fn repeats_undo_like_any_other_edit() {
    let mut state = EditorState::default();
    let mut history = super::undo::UndoHistory::default();
    history.record_if_changed(&state);
    super::repeat_marks::toggle_repeat(&mut state.repeats, 0, 48);
    history.record_if_changed(&state);
    history.undo(&mut state);
    assert!(state.repeats.is_empty());
}

#[test]
fn the_ruler_counts_bars_on_the_downbeat_and_beats_within_one() {
    let s = EditorState::default(); // 4/4
    let beat = TICKS_PER_BEAT;
    let got: Vec<(usize, usize)> = (0..8).map(|i| ruler_position(&s, i * beat)).collect();
    assert_eq!(got, [(1, 1), (1, 2), (1, 3), (1, 4), (2, 1), (2, 2), (2, 3), (2, 4)]);
    assert_eq!(ruler_position(&s, 36 * 4 * beat), (37, 1), "bar 37, not another 1");
}

#[test]
fn the_ruler_counts_a_seven_eight_bar_as_seven_eighths() {
    // 7/8 is 3.5 quarter-note columns; the map counts the meter's own beat
    // and puts bar 2 at tick 42, which is not a column boundary at all.
    let s = EditorState { time_signature: "7/8".into(), ..Default::default() };
    let got: Vec<usize> = (0..7).map(|i| ruler_position(&s, i * 6).1).collect();
    assert_eq!(got, [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(ruler_position(&s, 42), (2, 1));
    assert!(!42_usize.is_multiple_of(TICKS_PER_BEAT));
}

#[test]
fn the_ruler_re_bars_everything_after_a_meter_change() {
    // Two bars of 4/4, then 3/4 from bar 3: bar 4 starts 36 ticks later,
    // not 48, and beats within it count to three.
    let s = EditorState { meter_changes: vec![(96, "3/4".into())], ..Default::default() };
    assert_eq!(ruler_position(&s, 96), (3, 1));
    assert_eq!(ruler_position(&s, 96 + 24), (3, 3));
    assert_eq!(ruler_position(&s, 96 + 36), (4, 1));
    // And the ruler's bar lines land there too.
    let bars: Vec<u64> = s.meter_map().bar_starts(0, 200).into_iter().map(|(t, _)| t).collect();
    assert_eq!(bars, [0, 48, 96, 132, 168]);
}

#[test]
fn describe_tick_matches_the_ruler_after_a_change() {
    use super::timeline::describe_tick;
    let s = EditorState {
        meter_changes: vec![(84, "3/4".into())], // off a bar line: bar 2 is cut short
        ..Default::default()
    };
    let map = s.meter_map();
    assert_eq!(describe_tick(83, &map), "2.3");
    assert_eq!(describe_tick(84, &map), "3.1");
    assert_eq!(describe_tick(84 + 36, &map), "4.1");
}

// ── resize_grip_position ──────────────────────────────────────────────────────

fn grid_note(tick: usize, len: usize) -> GridNote {
    GridNote { id: 1, hole: 1, tick, len, dir: Dir::Blow, pitch: Pitch::Normal, expr: Expr::None }
}

#[test]
fn grips_sit_entirely_outside_the_note_at_every_length() {
    // The whole point of moving them out of the note: however short the
    // note gets, neither grip eats into the body that drags it. One beat in
    // (tick 12) so the left grip isn't up against the clamp.
    for len in 1..=(TICKS_PER_BEAT * 2) {
        let note = grid_note(TICKS_PER_BEAT, len);
        let (left, _, width, _) = note_rect(&note);
        let (lx, _) = resize_grip_position(&note, Edge::Left);
        let (rx, _) = resize_grip_position(&note, Edge::Right);
        assert!(lx + GRIP_D <= left, "len {len}: left grip overlaps the note body");
        assert!(rx >= left + width, "len {len}: right grip overlaps it");
        assert!(rx > lx, "len {len}: grips crossed over");
    }
}

#[test]
fn a_grip_is_the_same_size_however_short_the_note_is() {
    // A 16th note and a half note put their grips exactly as far apart as
    // their own widths differ — the grips never shrink to fit.
    let short = grid_note(TICKS_PER_BEAT, TICKS_PER_BEAT / 4);
    let long = grid_note(TICKS_PER_BEAT, TICKS_PER_BEAT * 2);
    let span = |n: &GridNote| {
        resize_grip_position(n, Edge::Right).0 - resize_grip_position(n, Edge::Left).0
    };
    let widths = note_rect(&long).2 - note_rect(&short).2;
    assert!((span(&long) - span(&short) - widths).abs() < 0.001);
}

#[test]
fn grips_are_centred_on_the_notes_vertical_middle() {
    let note = grid_note(TICKS_PER_BEAT, TICKS_PER_BEAT);
    let (_, top, _, height) = note_rect(&note);
    let want = top + (height - GRIP_D) / 2.0;
    assert_eq!(resize_grip_position(&note, Edge::Left).1, want);
    assert_eq!(resize_grip_position(&note, Edge::Right).1, want);
}

#[test]
fn the_left_grip_of_a_note_at_tick_zero_stays_on_screen() {
    // `GridArea` clips its overflow and the grid can't scroll left of 0, so
    // an unclamped grip there would never be reachable.
    let note = grid_note(0, TICKS_PER_BEAT);
    assert_eq!(resize_grip_position(&note, Edge::Left).0, 0.0);
}

// ── envelope ──────────────────────────────────────────────────────────────────

#[test]
fn envelope_starts_at_zero_and_stays_in_unit_range() {
    let dur = SAMPLE_RATE as usize; // 1 second, comfortably longer than attack+release
    for i in [0, 100, dur / 2, dur - 100, dur - 1] {
        let e = envelope(i, dur);
        assert!((0.0..=1.0).contains(&e), "envelope({i}, {dur}) = {e} out of range");
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
    assert!(near_end < mid, "release should pull the tail down from full sustain");
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

#[test]
fn song_end_tick_is_the_last_notes_end() {
    let notes = vec![
        GridNote::plain(0, 1, 0, 4),
        GridNote::plain(1, 2, 10, 2),
        GridNote::plain(2, 3, 4, 4),
    ];
    assert_eq!(song_end_tick(&notes), 12);
}

#[test]
fn song_end_tick_of_an_empty_song_is_zero() {
    assert_eq!(song_end_tick(&[]), 0);
}

// ── Tempo map ──────────────────────────────────────────────────────────────

#[test]
fn tempo_map_with_no_changes_is_a_single_tick_zero_point() {
    let map = build_tempo_map("140", &[]);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].tick, 0);
    assert_eq!(map[0].bpm, 140.0);
}

#[test]
fn tempo_map_sorts_changes_by_tick_regardless_of_insertion_order() {
    let map = build_tempo_map("120", &[(960, 180.0), (480, 150.0)]);
    let ticks: Vec<u64> = map.iter().map(|p| p.tick).collect();
    assert_eq!(ticks, vec![0, 480, 960]);
    assert_eq!(map[1].bpm, 150.0);
    assert_eq!(map[2].bpm, 180.0);
}

#[test]
fn tempo_map_falls_back_to_120_for_an_unparseable_opening_tempo() {
    let map = build_tempo_map("not a number", &[]);
    assert_eq!(map[0].bpm, 120.0);
}

#[test]
fn tempo_map_keeps_the_opening_tempo_when_a_change_collides_with_tick_zero() {
    // A tempo-change point placed at tick 0 (where the opening tempo
    // already applies) shouldn't produce two competing tick-0 entries.
    let map = build_tempo_map("120", &[(0, 200.0)]);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].bpm, 120.0);
}

// ── toggle_tempo_point ───────────────────────────────────────────────────────

#[test]
fn toggle_tempo_point_adds_a_point_at_the_clicked_tick() {
    let mut s = EditorState { tempo: "120".into(), ..Default::default() };
    toggle_tempo_point(&mut s, 100);
    assert_eq!(s.tempo_changes.len(), 1);
    assert_eq!(s.tempo_changes[0].0, 100);
    // Steps up from the 120 already in effect there.
    assert_eq!(s.tempo_changes[0].1, 130.0);
}

#[test]
fn toggle_tempo_point_removes_a_point_clicked_again_nearby() {
    let mut s = EditorState { tempo_changes: vec![(100, 150.0)], ..Default::default() };
    toggle_tempo_point(&mut s, 101); // within snap distance, not exact
    assert!(s.tempo_changes.is_empty());
}

#[test]
fn toggle_tempo_point_ignores_a_click_too_close_to_tick_zero() {
    let mut s = EditorState::default();
    toggle_tempo_point(&mut s, 0);
    assert!(s.tempo_changes.is_empty());
}

#[test]
fn toggle_tempo_point_steps_from_whichever_tempo_is_already_in_effect() {
    let mut s = EditorState {
        tempo: "120".into(),
        tempo_changes: vec![(100, 200.0)],
        ..Default::default()
    };
    // Clicking well past the existing point should step from *its* tempo
    // (200), not the opening one (120).
    toggle_tempo_point(&mut s, 300);
    assert_eq!(s.tempo_changes.len(), 2);
    let added = s.tempo_changes.iter().find(|&&(t, _)| t == 300).unwrap();
    assert_eq!(added.1, 210.0);
}

// ── Silence track ──────────────────────────────────────────────────────────

#[test]
fn silence_gaps_reports_the_space_between_consecutive_notes() {
    let notes = vec![GridNote::plain(0, 1, 0, 4), GridNote::plain(1, 2, 10, 2)];
    assert_eq!(silence_gaps(&notes), vec![(4, 10)]);
}

#[test]
fn silence_gaps_ignores_leading_and_trailing_silence() {
    // A single note has no "next" note to measure a gap up to.
    let notes = vec![GridNote::plain(0, 1, 4, 4)];
    assert!(silence_gaps(&notes).is_empty());
}

#[test]
fn silence_gaps_treats_overlapping_notes_across_holes_as_one_sounding_span() {
    // A chord (same tick, different holes) and a note whose tail
    // overlaps the next note's onset must not read as silence.
    let notes = vec![
        GridNote::plain(0, 1, 0, 4),
        GridNote::plain(1, 2, 0, 4),  // chord with note 0
        GridNote::plain(2, 3, 2, 6),  // overlaps note 0's tail
        GridNote::plain(3, 4, 20, 2), // a real gap follows
    ];
    assert_eq!(silence_gaps(&notes), vec![(8, 20)]);
}

#[test]
fn silence_gaps_skips_touching_notes_since_nothing_is_ever_silent() {
    let notes = vec![GridNote::plain(0, 1, 0, 4), GridNote::plain(1, 2, 4, 4)];
    assert!(silence_gaps(&notes).is_empty());
}

#[test]
fn silence_gaps_of_an_empty_song_is_empty() {
    assert!(silence_gaps(&[]).is_empty());
}

#[test]
fn normalize_range_orders_a_backwards_span() {
    assert_eq!(normalize_range(10, 4), (4, 10));
    assert_eq!(normalize_range(4, 10), (4, 10));
    assert_eq!(normalize_range(5, 5), (5, 5));
}

#[test]
fn split_side_range_left_is_song_start_to_the_split() {
    let notes = vec![GridNote::plain(0, 1, 0, 20)];
    assert_eq!(split_side_range(8, Side::Left, &notes), (0, 8));
}

#[test]
fn split_side_range_right_is_the_split_to_song_end() {
    let notes = vec![GridNote::plain(0, 1, 0, 20)];
    assert_eq!(split_side_range(8, Side::Right, &notes), (8, 20));
}

#[test]
fn split_side_range_right_never_ends_before_the_split_on_an_empty_song() {
    assert_eq!(split_side_range(8, Side::Right, &[]), (8, 8));
}

#[test]
fn erase_range_deletes_only_overlapping_notes_and_shifts_nothing() {
    let notes = vec![
        GridNote::plain(0, 1, 0, 4),  // 0..4, fully before the range
        GridNote::plain(1, 2, 4, 4),  // 4..8, inside the range
        GridNote::plain(2, 3, 6, 4),  // 6..10, partially overlaps
        GridNote::plain(3, 4, 12, 4), // 12..16, fully after the range
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
        GridNote::plain(0, 1, 0, 4),  // 0..4, before the range — untouched
        GridNote::plain(1, 2, 4, 4),  // 4..8, inside the range — deleted
        GridNote::plain(2, 3, 10, 4), // 10..14, after the range — shifts left by 6
    ];
    let out = remove_range(&notes, 4, 10);
    let ids: Vec<u32> = out.iter().map(|n| n.id).collect();
    assert_eq!(ids, vec![0, 2]);
    assert_eq!(out.iter().find(|n| n.id == 0).unwrap().tick, 0);
    assert_eq!(out.iter().find(|n| n.id == 2).unwrap().tick, 4);
}

#[test]
fn remove_range_closes_the_gap_exactly_the_removed_length() {
    let notes = vec![GridNote::plain(0, 1, 20, 4)];
    let out = remove_range(&notes, 5, 8); // remove a 3-tick span before it
    assert_eq!(out[0].tick, 17);
}

#[test]
fn erase_and_remove_on_a_zero_length_range_are_no_ops() {
    let notes = vec![GridNote::plain(0, 1, 0, 4), GridNote::plain(1, 2, 8, 4)];
    assert_eq!(erase_range(&notes, 6, 6), notes);
    assert_eq!(remove_range(&notes, 6, 6), notes);
}

#[test]
fn timeline_tool_is_active_is_false_only_for_none() {
    assert!(!TimelineTool::None.is_active());
    assert!(TimelineTool::Erase.is_active());
    assert!(TimelineTool::Remove.is_active());
    assert!(TimelineTool::Meter.is_active());
}

#[test]
fn meter_tool_adds_a_change_on_the_nearest_active_beat() {
    let mut state = EditorState::default();
    cycle_meter_point(&mut state, 50);
    assert_eq!(state.meter_changes, vec![(48, "3/4".into())]);
}

#[test]
fn meter_tool_cycles_an_existing_change_and_eventually_removes_it() {
    let mut state = EditorState { meter_changes: vec![(48, "3/4".into())], ..Default::default() };
    cycle_meter_point(&mut state, 48);
    assert_eq!(state.meter_changes, vec![(48, "2/4".into())]);
    for _ in 0..8 {
        cycle_meter_point(&mut state, 48);
    }
    assert!(state.meter_changes.is_empty());
}

#[test]
fn meter_tool_uses_the_changed_meters_beat_grid() {
    let mut state = EditorState { meter_changes: vec![(48, "7/8".into())], ..Default::default() };
    cycle_meter_point(&mut state, 57);
    assert_eq!(state.meter_changes, vec![(48, "7/8".into()), (60, "5/8".into())]);
}

#[test]
fn meter_tool_leaves_tick_zero_to_the_details_picker() {
    let mut state = EditorState::default();
    cycle_meter_point(&mut state, 3);
    assert!(state.meter_changes.is_empty());
}

// ── drag_end_tick ─────────────────────────────────────────────────────────

#[test]
fn drag_end_tick_advances_by_whole_ticks_moved_right() {
    assert_eq!(drag_end_tick(4, TICK_W, 1.0, 0.0), 5);
    assert_eq!(drag_end_tick(4, 3.0 * TICK_W, 1.0, 0.0), 7);
}

#[test]
fn drag_end_tick_moves_back_left_and_clamps_at_zero() {
    assert_eq!(drag_end_tick(4, -TICK_W, 1.0, 0.0), 3);
    assert_eq!(drag_end_tick(4, -10.0 * TICK_W, 1.0, 0.0), 0);
}

#[test]
fn drag_end_tick_divides_out_the_ui_scale_before_converting() {
    // At 2x UI zoom, the same visual tick of motion is twice as many
    // raw window pixels — dividing by `ui_scale` first is what keeps
    // the drag tracking the pointer 1:1 regardless of zoom level, the
    // same correction `grid.rs`'s note-move drag already applies.
    assert_eq!(drag_end_tick(4, 2.0 * TICK_W, 2.0, 0.0), 5);
}

#[test]
fn drag_end_tick_adds_the_grid_scroll_since_the_press() {
    // A mid-drag wheel pan scrolls the content under a stationary
    // pointer: the span's end must follow what's now under the pointer,
    // so scroll delta counts like pointer motion.
    assert_eq!(drag_end_tick(4, 0.0, 1.0, 2.0 * TICK_W), 6);
    // Scroll delta is in logical px (like `Scroll::px` itself), so it is
    // NOT divided by the UI scale the way raw pointer pixels are.
    assert_eq!(drag_end_tick(4, 2.0 * TICK_W, 2.0, 2.0 * TICK_W), 7);
    // Scrolling back before the press position clamps at zero like any
    // other leftward motion.
    assert_eq!(drag_end_tick(4, 0.0, 1.0, -10.0 * TICK_W), 0);
}

// ── TimelineSurfaceGeometry::tick_at ─────────────────────────────────────────

#[test]
fn tick_at_recenters_the_minus_half_to_half_normalized_range() {
    // `RelativeCursorPosition::normalized` is -0.5..0.5 across the
    // surface's own width, not 0..1 — a click at the surface's left
    // edge (-0.5) must resolve to tick 0, not get clamped away.
    let geom = TimelineSurfaceGeometry { scroll_px: 0.0, width_px: 20.0 * TICK_W };
    assert_eq!(geom.tick_at(-0.5), 0);
    assert_eq!(geom.tick_at(0.0), 10);
    assert_eq!(geom.tick_at(0.5), 20);
}

#[test]
fn tick_at_offsets_by_the_surfaces_own_scroll_position() {
    let geom = TimelineSurfaceGeometry { scroll_px: 16.0 * TICK_W, width_px: 20.0 * TICK_W };
    // Scrolled 16 ticks in: the surface's left edge sits at tick 16.
    assert_eq!(geom.tick_at(-0.5), 16);
}

#[test]
fn tick_at_clamps_outside_the_surfaces_own_bounds() {
    let geom = TimelineSurfaceGeometry { scroll_px: 0.0, width_px: 20.0 * TICK_W };
    assert_eq!(geom.tick_at(-5.0), 0);
    assert_eq!(geom.tick_at(5.0), 20);
}

// ── scrollbar_marker ─────────────────────────────────────────────────────

#[test]
fn scrollbar_marker_maps_ticks_onto_track_percentages() {
    // A note from tick 25 to 50 of a 100-tick song: left 25%, width 25%.
    let (left, width) = super::view_scroll::scrollbar_marker(25, 25, 100);
    assert_eq!(left, 25.0);
    assert_eq!(width, 25.0);
}

#[test]
fn scrollbar_marker_floors_the_width_of_a_tiny_note() {
    // One tick of a very long song would be invisibly thin without the floor.
    let (_, width) = super::view_scroll::scrollbar_marker(0, 1, 10_000);
    assert!(width >= 0.3);
}

#[test]
fn scrollbar_marker_never_pokes_past_the_track_end() {
    // A floored marker on the song's very last tick must stay inside 100%.
    let (left, width) = super::view_scroll::scrollbar_marker(9_999, 1, 10_000);
    assert!(left + width <= 100.0);
}

// ── UndoHistory ───────────────────────────────────────────────────────────

fn state_with_notes(notes: Vec<GridNote>) -> EditorState {
    EditorState { notes, ..EditorState::default() }
}

#[test]
fn the_first_record_seeds_history_without_anything_to_undo() {
    let mut history = UndoHistory::default();
    let state = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&state);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn a_genuine_content_change_becomes_one_undo_step() {
    let mut history = UndoHistory::default();
    let before = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&before);

    let after = state_with_notes(vec![
        note(1, Dir::Blow, Pitch::Normal),
        note(2, Dir::Draw, Pitch::Normal),
    ]);
    history.record_if_changed(&after);
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn recording_the_same_content_twice_is_a_no_op() {
    let mut history = UndoHistory::default();
    let mut state = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&state);
    // Only `selected` changes — not part of the undo snapshot at all, see
    // `undo`'s module doc comment.
    state.selected = vec![1];
    history.record_if_changed(&state);
    assert!(!history.can_undo());
}

#[test]
fn undo_restores_the_previous_content_and_enables_redo() {
    let mut history = UndoHistory::default();
    let before = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&before);

    let mut state = state_with_notes(vec![
        note(1, Dir::Blow, Pitch::Normal),
        note(2, Dir::Draw, Pitch::Normal),
    ]);
    history.record_if_changed(&state);

    history.undo(&mut state);
    assert_eq!(state.notes, before.notes);
    assert!(!history.can_undo());
    assert!(history.can_redo());
}

#[test]
fn undo_drops_a_selection_pointing_at_a_removed_note() {
    let mut history = UndoHistory::default();
    let before = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&before);

    let mut added = note(2, Dir::Draw, Pitch::Normal);
    added.id = 7;
    let mut state = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal), added]);
    state.selected = vec![7];
    history.record_if_changed(&state);

    history.undo(&mut state);
    assert!(state.selected.is_empty(), "the undone note's id must not stay selected");
}

#[test]
fn redo_reapplies_the_undone_content() {
    let mut history = UndoHistory::default();
    let before = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&before);

    let after_notes = vec![note(1, Dir::Blow, Pitch::Normal), note(2, Dir::Draw, Pitch::Normal)];
    let mut state = state_with_notes(after_notes.clone());
    history.record_if_changed(&state);

    history.undo(&mut state);
    history.redo(&mut state);
    assert_eq!(state.notes, after_notes);
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn a_fresh_edit_after_undo_clears_the_redo_stack() {
    let mut history = UndoHistory::default();
    let before = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&before);

    let mut state = state_with_notes(vec![
        note(1, Dir::Blow, Pitch::Normal),
        note(2, Dir::Draw, Pitch::Normal),
    ]);
    history.record_if_changed(&state);
    history.undo(&mut state);
    assert!(history.can_redo());

    // A genuinely new edit, not another undo/redo.
    state.notes.push(note(3, Dir::Blow, Pitch::Normal));
    history.record_if_changed(&state);
    assert!(!history.can_redo());
}

#[test]
fn undo_and_redo_are_no_ops_with_nothing_on_their_stack() {
    let mut history = UndoHistory::default();
    let mut state = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    let original = state.notes.clone();
    history.undo(&mut state);
    history.redo(&mut state);
    assert_eq!(state.notes, original);
}

#[test]
fn history_evicts_the_oldest_entry_past_the_limit() {
    let mut history = UndoHistory::default();
    let mut state = state_with_notes(vec![]);
    history.record_if_changed(&state);
    // One content-changing edit per iteration, well past the cap.
    for i in 0..(HISTORY_LIMIT + 10) {
        state.notes = vec![note(1, Dir::Blow, Pitch::Bend(0.0))];
        state.notes[0].tick = i;
        history.record_if_changed(&state);
    }
    // Undoing HISTORY_LIMIT times must exhaust the stack even though more
    // edits than that were made — the earliest ones fell off the front.
    for _ in 0..HISTORY_LIMIT {
        history.undo(&mut state);
    }
    assert!(!history.can_undo());
}

#[test]
fn undo_skips_recording_while_a_take_is_active() {
    // Mirrors `track_changes`'s own gating, without spinning up a
    // `Schedule`: a recording take grows a note's length every frame, and
    // none of that should land in the undo history until the take stops —
    // otherwise undo would only ever step back one frame of growth.
    let mut history = UndoHistory::default();
    let mut state = state_with_notes(vec![note(1, Dir::Blow, Pitch::Normal)]);
    history.record_if_changed(&state);

    // Simulate several frames of a take growing a note, none recorded —
    // `track_changes` itself is what skips these in the real system; here
    // we just don't call `record_if_changed` for them, the same effect.
    for len in 4..20 {
        state.notes[0].len = len;
    }
    // Take stops: exactly one record_if_changed call, one undo step for
    // the whole take.
    history.record_if_changed(&state);
    assert!(history.can_undo());
    history.undo(&mut state);
    assert_eq!(state.notes[0].len, 4);
    assert!(!history.can_undo());
}

// ── time signature ────────────────────────────────────────────────────────

#[test]
fn meter_reads_the_chart_meter_not_a_fixed_four() {
    let mut s = EditorState::default();
    let m = s.meter();
    assert_eq!((m.numerator, m.denominator), (4, 4)); // the 4/4 default
    s.time_signature = "3/4".into();
    assert_eq!(s.meter().beats_per_bar(), 3.0);
    // 6/8 is six eighths — three quarter-note beats, which is the unit
    // the staff counts in — and six of its own beats, which the ruler and
    // the metronome count in. Both answers come off the same meter.
    s.time_signature = "6/8".into();
    assert_eq!(s.meter().beats_per_bar(), 3.0);
    assert_eq!(s.meter().numerator, 6);
}

#[test]
fn ticks_per_bar_is_exact_where_a_whole_quarter_count_has_to_round() {
    let mut s = EditorState::default();
    // 4/4: four quarters, 48 ticks.
    assert_eq!(s.meter_map().segment_at(0).ticks_per_bar as usize, 4 * TICKS_PER_BEAT);
    assert_eq!(s.meter_map().segment_at(0).ticks_per_beat as usize, TICKS_PER_BEAT);

    // 7/8: seven eighths, 3.5 quarters — which rounded to whole quarters
    // would claim a 48-tick bar; the real one is 42.
    s.time_signature = "7/8".into();
    assert_eq!(s.meter().beats_per_bar(), 3.5);
    assert_eq!(s.meter_map().segment_at(0).ticks_per_bar as usize, 42);
    assert_eq!(s.meter_map().segment_at(0).ticks_per_beat as usize, TICKS_PER_BEAT / 2);

    // 5/8: 2.5 quarters, the other direction.
    s.time_signature = "5/8".into();
    assert_eq!(s.meter_map().segment_at(0).ticks_per_bar as usize, 30);

    // 6/8 was always a whole number of quarters, so only the *beat* changes:
    // six eighths per bar rather than three quarters.
    s.time_signature = "6/8".into();
    assert_eq!(s.meter_map().segment_at(0).ticks_per_bar as usize, 36);
    assert_eq!(s.meter_map().segment_at(0).ticks_per_beat as usize, TICKS_PER_BEAT / 2);
}

#[test]
fn every_offered_meter_divides_the_tick_grid_exactly() {
    // Nothing the picker offers may fall back — a fallback would silently
    // mis-place bar lines again, which is the whole bug.
    let mut s = EditorState::default();
    for sig in harmonicon_ui::music_score::TIME_SIGNATURES {
        s.time_signature = sig.into();
        let meter = harmonicon_ui::music_score::parse_time_signature(sig);
        assert_eq!(
            s.meter_map().segment_at(0).ticks_per_bar as usize,
            meter.ticks_per_bar(TICKS_PER_BEAT as u32).unwrap() as usize,
            "{sig} fell back instead of dividing exactly"
        );
    }
}

#[test]
fn a_meter_too_fine_for_the_tick_grid_falls_back_rather_than_panicking() {
    let s = EditorState { time_signature: "4/32".into(), ..Default::default() };
    assert_eq!(s.meter_map().segment_at(0).ticks_per_beat as usize, TICKS_PER_BEAT);
    assert_eq!(s.meter_map().segment_at(0).ticks_per_bar as usize, TICKS_PER_BEAT * 4);
}

#[test]
fn meter_survives_a_malformed_signature() {
    // The meter is picked, not typed, but a chart on disk can still carry
    // anything; that must not wedge the grid or divide by zero.
    let mut s = EditorState::default();
    for junk in ["", "3", "3/", "3/0", "x/y"] {
        s.time_signature = junk.into();
        assert!(s.meter_map().segment_at(0).ticks_per_bar as usize >= 1, "{junk:?}");
        assert!(s.meter_map().segment_at(0).ticks_per_beat as usize >= 1, "{junk:?}");
        assert!(s.meter().bar_secs(120.0) > 0.0, "{junk:?}");
    }
}

#[test]
fn a_time_signature_round_trips_through_save_and_load() {
    let s = EditorState { time_signature: "6/8".into(), ..Default::default() };
    let v: serde_json::Value = serde_json::from_str(&serialize_harpchart(&s)).unwrap();
    let mut loaded = EditorState::default();
    let mut scroll = Scroll::default();
    load_harpchart(&v, &mut loaded, &mut scroll);
    assert_eq!(loaded.time_signature, "6/8");
    assert_eq!(loaded.meter().numerator, 6);
}

// ── metadata alignment through edits ─────────────────────────────────────────
//
// Phrase annotations are keyed by onset tick and expression intensities by
// note id, both stored *beside* the notes. Every bulk edit below has to
// carry them along or clean them up; these pin what "along" means.

fn annotated(section: &str) -> PhraseAnnotation {
    PhraseAnnotation { section: Some(section.into()), ..Default::default() }
}

/// Two notes at tick 0 (a chord on holes 1 and 2), one at tick 24, with a
/// section label on each onset and an intensity on the first note.
fn state_with_metadata() -> EditorState {
    let mut s = state_with_notes(vec![
        GridNote::plain(0, 1, 0, 4),
        GridNote::plain(1, 2, 0, 4),
        GridNote::plain(2, 3, 24, 4),
    ]);
    s.next_id = 3;
    s.phrase_annotations.insert(0, annotated("A"));
    s.phrase_annotations.insert(24, annotated("B"));
    s.expression_intensities.insert(0, "0.8".into());
    s
}

fn section_at(s: &EditorState, tick: usize) -> Option<&str> {
    s.phrase_annotations.get(&tick).and_then(|a| a.section.as_deref())
}

#[test]
fn moving_a_whole_onset_group_takes_its_annotation_along() {
    let mut s = state_with_metadata();
    // Both tick-0 notes move to tick 12: nothing is left at 0.
    s.move_notes(&[(0, 1, 12), (1, 2, 12)]);
    assert_eq!(section_at(&s, 0), None);
    assert_eq!(section_at(&s, 12), Some("A"));
    assert_eq!(section_at(&s, 24), Some("B"), "an unrelated phrase is untouched");
}

#[test]
fn moving_the_only_note_at_an_onset_takes_its_annotation_along() {
    let mut s = state_with_metadata();
    s.move_notes(&[(2, 3, 36)]);
    assert_eq!(section_at(&s, 24), None);
    assert_eq!(section_at(&s, 36), Some("B"));
}

#[test]
fn moving_part_of_an_onset_group_leaves_the_annotation_with_the_phrase() {
    let mut s = state_with_metadata();
    // Only hole 1 moves; hole 2 still starts at tick 0, so the phrase — and
    // its label — is still there.
    s.move_notes(&[(0, 1, 12)]);
    assert_eq!(section_at(&s, 0), Some("A"));
    assert_eq!(section_at(&s, 12), None);
}

#[test]
fn moving_a_phrase_onto_an_existing_one_keeps_the_destinations_annotation() {
    let mut s = state_with_metadata();
    // The tick-24 note joins the chord at tick 0 (on a free hole). Tick 0
    // already has a phrase with its own label; that label wins and B is
    // not carried in over it.
    s.move_notes(&[(2, 3, 0)]);
    assert_eq!(section_at(&s, 0), Some("A"));
    assert_eq!(section_at(&s, 24), None, "nothing starts at 24 any more");
}

#[test]
fn moving_a_note_keeps_its_expression_intensity() {
    // Id-keyed, so this is free — but pin it, since a move that reassigned
    // ids would silently lose it.
    let mut s = state_with_metadata();
    s.move_notes(&[(0, 1, 12), (1, 2, 12)]);
    assert_eq!(s.expression_intensities.get(&0).map(String::as_str), Some("0.8"));
}

#[test]
fn copy_paste_carries_intensity_and_annotation_to_the_new_notes() {
    let mut s = state_with_metadata();
    s.selected = vec![0, 1];
    let clip = s.copy_selection();
    assert!(!clip.is_empty());
    assert!(s.paste(&clip, 48));
    // The pasted notes are the new selection, with fresh ids.
    let pasted: Vec<u32> = s.selected.clone();
    assert_eq!(pasted.len(), 2);
    assert!(pasted.iter().all(|&id| id >= 3));
    // Hole 1's intensity followed it to its new id; hole 2 had none.
    let new_hole_1 = s.notes.iter().find(|n| pasted.contains(&n.id) && n.hole == 1).unwrap();
    let new_hole_2 = s.notes.iter().find(|n| pasted.contains(&n.id) && n.hole == 2).unwrap();
    assert_eq!(s.expression_intensities.get(&new_hole_1.id).map(String::as_str), Some("0.8"));
    assert!(!s.expression_intensities.contains_key(&new_hole_2.id));
    // The onset's label came too; the originals are untouched.
    assert_eq!(section_at(&s, 48), Some("A"));
    assert_eq!(section_at(&s, 0), Some("A"));
    assert_eq!(s.expression_intensities.get(&0).map(String::as_str), Some("0.8"));
}

#[test]
fn pasting_onto_an_existing_phrase_does_not_overwrite_its_annotation() {
    let mut s = state_with_metadata();
    s.selected = vec![0]; // hole 1 at tick 0, labelled "A"
    let clip = s.copy_selection();
    // Paste at tick 24, where "B" already sits on hole 3 — hole 1 is free.
    assert!(s.paste(&clip, 24));
    assert_eq!(section_at(&s, 24), Some("B"));
}

#[test]
fn a_paste_that_places_nothing_leaves_metadata_alone() {
    let mut s = state_with_metadata();
    s.selected = vec![0];
    let clip = s.copy_selection();
    // Hole 1 at tick 0 is exactly where the copy came from — it collides.
    assert!(!s.paste(&clip, 0));
    assert_eq!(s.phrase_annotations.len(), 2);
    assert_eq!(s.expression_intensities.len(), 1);
}

#[test]
fn deleting_one_note_of_a_phrase_keeps_the_phrases_annotation() {
    let mut s = state_with_metadata();
    s.selected = vec![0];
    delete_selected(&mut s);
    assert_eq!(section_at(&s, 0), Some("A"), "hole 2 still starts there");
    assert!(!s.expression_intensities.contains_key(&0), "its intensity goes with it");
}

#[test]
fn deleting_every_note_of_a_phrase_drops_its_annotation() {
    let mut s = state_with_metadata();
    s.selected = vec![0, 1];
    delete_selected(&mut s);
    assert_eq!(section_at(&s, 0), None);
    assert_eq!(section_at(&s, 24), Some("B"));
}

#[test]
fn erase_range_drops_annotations_only_where_no_note_remains() {
    let mut s = state_with_metadata();
    // Erases the tick-24 note (24..28) and nothing else.
    s.erase_notes_in(20, 30);
    assert_eq!(section_at(&s, 24), None);
    assert_eq!(section_at(&s, 0), Some("A"));
    assert_eq!(s.notes.len(), 2);
}

#[test]
fn remove_range_shifts_later_annotations_by_the_removed_length() {
    let mut s = state_with_metadata();
    // Removing 6..18 (nothing starts there) closes a 12-tick gap: the
    // tick-24 phrase, label included, now starts at 12.
    s.remove_range_closing_gap(6, 18);
    assert_eq!(s.notes.iter().find(|n| n.id == 2).unwrap().tick, 12);
    assert_eq!(section_at(&s, 12), Some("B"));
    assert_eq!(section_at(&s, 24), None);
    assert_eq!(section_at(&s, 0), Some("A"));
}

#[test]
fn remove_range_shifts_timing_points_and_preserves_the_end_state() {
    let mut s = EditorState {
        tempo: "120".into(),
        tempo_changes: vec![(8, 140.0), (24, 160.0), (36, 180.0)],
        time_signature: "4/4".into(),
        meter_changes: vec![(8, "3/4".into()), (24, "7/8".into()), (36, "6/8".into())],
        ..Default::default()
    };

    s.remove_range_closing_gap(6, 30);

    assert_eq!(s.tempo_changes, vec![(6, 160.0), (12, 180.0)]);
    assert_eq!(s.meter_changes, vec![(6, "7/8".into()), (12, "6/8".into())]);
}

#[test]
fn removing_from_tick_zero_promotes_the_timing_at_the_cut_end() {
    let mut s = EditorState {
        tempo: "120".into(),
        tempo_changes: vec![(8, 140.0), (24, 160.0)],
        time_signature: "4/4".into(),
        meter_changes: vec![(8, "3/4".into()), (24, "7/8".into())],
        ..Default::default()
    };

    s.remove_range_closing_gap(0, 12);

    assert_eq!(s.tempo, "140");
    assert_eq!(s.tempo_changes, vec![(12, 160.0)]);
    assert_eq!(s.time_signature, "3/4");
    assert_eq!(s.meter_changes, vec![(12, "7/8".into())]);
}

#[test]
fn remove_range_drops_annotations_inside_the_cut() {
    let mut s = state_with_metadata();
    s.remove_range_closing_gap(20, 30);
    assert_eq!(section_at(&s, 24), None);
    assert!(s.notes.iter().all(|n| n.id != 2));
    // Everything before the cut is where it was.
    assert_eq!(section_at(&s, 0), Some("A"));
}

#[test]
fn switching_harmonica_kind_drops_metadata_of_the_holes_it_removes() {
    // A 12-hole chromatic chart with a phrase on hole 12 alone; going to a
    // 10-hole diatonic removes that note and must take its label and
    // intensity with it, not leave them pointing at nothing.
    let mut s = state_with_notes(vec![GridNote::plain(0, 12, 0, 4), GridNote::plain(1, 1, 24, 4)]);
    s.harmonica_kind = HarmonicaKind::Chromatic;
    s.phrase_annotations.insert(0, annotated("high"));
    s.phrase_annotations.insert(24, annotated("low"));
    s.expression_intensities.insert(0, "0.9".into());
    s.set_harmonica_kind(HarmonicaKind::Diatonic);
    assert!(s.notes.iter().all(|n| n.hole <= 10));
    assert_eq!(section_at(&s, 0), None);
    assert!(!s.expression_intensities.contains_key(&0));
    assert_eq!(section_at(&s, 24), Some("low"));
}

#[test]
fn undo_restores_annotations_and_intensities_together_with_the_notes() {
    let mut history = UndoHistory::default();
    let mut s = state_with_metadata();
    history.record_if_changed(&s);
    s.selected = vec![0, 1];
    delete_selected(&mut s);
    history.record_if_changed(&s);
    assert_eq!(section_at(&s, 0), None);
    history.undo(&mut s);
    assert_eq!(section_at(&s, 0), Some("A"));
    assert_eq!(s.expression_intensities.get(&0).map(String::as_str), Some("0.8"));
}

// ── phrase editor ────────────────────────────────────────────────────────────

#[test]
fn opening_the_phrase_editor_selects_every_note_of_the_phrase() {
    let mut s = state_with_metadata();
    s.open_phrase_editor(0);
    assert_eq!(s.phrase_editor, Some(0));
    let mut selected = s.selected.clone();
    selected.sort_unstable();
    assert_eq!(selected, vec![0, 1], "both tick-0 notes, not just one");
}

#[test]
fn opening_the_phrase_editor_on_an_empty_onset_does_nothing() {
    let mut s = state_with_metadata();
    s.selected = vec![2];
    s.open_phrase_editor(12);
    assert_eq!(s.phrase_editor, None);
    assert_eq!(s.selected, vec![2], "the selection is left alone");
}

#[test]
fn set_annotation_writes_the_phrase_at_that_tick_not_the_selection() {
    let mut s = state_with_metadata();
    s.selected = vec![2]; // tick 24 selected...
    s.set_annotation(0, Field::Chord, "C7".into()); // ...but tick 0 edited
    assert_eq!(s.annotation_text(0, Field::Chord), "C7");
    assert_eq!(s.annotation_text(24, Field::Chord), "");
    assert_eq!(s.annotation_text(0, Field::Section), "A", "other fields kept");
}

#[test]
fn set_annotation_refuses_a_tick_nothing_starts_on() {
    // Such an annotation is exactly the orphan `drop_orphaned_metadata`
    // removes, so accepting it would only lose the text at the next prune.
    let mut s = state_with_metadata();
    s.set_annotation(12, Field::Section, "ghost".into());
    assert!(!s.phrase_annotations.contains_key(&12));
}

#[test]
fn clearing_the_last_field_drops_the_annotation() {
    let mut s = state_with_metadata();
    s.set_annotation(24, Field::Section, "  ".into());
    assert!(!s.phrase_annotations.contains_key(&24));
}

#[test]
fn popover_sits_under_its_marker_and_inside_the_grid_area() {
    use super::phrase_editor::{WIDTH, popover_left};
    // Unscrolled, a marker at tick 24 is at x = 24 * TICK_W.
    assert_eq!(popover_left(24, 0.0, 800.0), 24.0 * TICK_W);
    // Scrolling moves it with the marker.
    assert_eq!(popover_left(24, 50.0, 800.0), 24.0 * TICK_W - 50.0);
    // A marker near the right edge pulls the panel back inside the area.
    assert_eq!(popover_left(1000, 0.0, 800.0), 800.0 - WIDTH);
    // A marker scrolled off the left never puts the panel left of 0.
    assert_eq!(popover_left(0, 300.0, 800.0), 0.0);
    // An area narrower than the panel pins it to the left edge.
    assert_eq!(popover_left(100, 0.0, 100.0), 0.0);
}

// ── the note column: Depth / Call / Split / Phrase ───────────────────────────

#[test]
fn depth_steps_through_the_four_quarters_and_wraps() {
    use super::selected_metadata::next_depth_step;
    assert_eq!(next_depth_step("0.5"), "0.75");
    assert_eq!(next_depth_step("0.75"), "1");
    assert_eq!(next_depth_step("1"), "0.25");
    // ½ is the default and is spelled the way the map's "absent" is.
    assert_eq!(next_depth_step("0.25"), "0.5");
}

#[test]
fn depth_steps_up_from_a_value_between_the_quarters() {
    use super::selected_metadata::next_depth_step;
    // A chart can carry any 0–1 depth; a click always visibly moves.
    assert_eq!(next_depth_step("0.8"), "1");
    assert_eq!(next_depth_step("0.1"), "0.25");
    assert_eq!(next_depth_step("garbage"), "0.75", "unparseable counts as the default");
}

#[test]
fn depth_label_is_a_percentage_or_nothing() {
    use super::selected_metadata::depth_label;
    assert_eq!(depth_label("0.75"), "75%");
    assert_eq!(depth_label("1"), "100%");
    assert_eq!(depth_label(""), "");
}

#[test]
fn depth_button_steps_the_selected_notes_depth() {
    let mut s = state_with_notes(vec![GridNote {
        expr: Expr::Vibrato(5.0),
        ..GridNote::plain(0, 1, 0, 4)
    }]);
    s.selected = vec![0];
    assert_eq!(s.depth_for_button(), "0.5", "default shown before any click");
    apply_modifier(&mut s, ModButton::Depth);
    assert_eq!(s.expression_intensities.get(&0).map(String::as_str), Some("0.75"));
    assert_eq!(s.depth_for_button(), "0.75");
    // Back round to the default, which is stored as absence.
    apply_modifier(&mut s, ModButton::Depth);
    apply_modifier(&mut s, ModButton::Depth);
    apply_modifier(&mut s, ModButton::Depth);
    assert!(!s.expression_intensities.contains_key(&0));
}

#[test]
fn depth_button_leaves_a_note_with_no_expression_alone() {
    let mut s = state_with_notes(vec![GridNote::plain(0, 1, 0, 4)]);
    s.selected = vec![0];
    assert_eq!(s.depth_for_button(), "", "nothing to show a depth of");
    apply_modifier(&mut s, ModButton::Depth);
    assert!(s.expression_intensities.is_empty());
}

#[test]
fn depth_button_arms_the_sticky_depth_that_a_new_note_gets() {
    // Dual-mode like the rate: with nothing selected, the click arms what
    // the next placed note gets — but only a note with an expression.
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Depth);
    assert_eq!(s.sticky_intensity, "0.75");
    assert_eq!(s.depth_for_button(), "0.75");
    s.sticky_expr = Expr::Wah(3.0);
    select_or_add(&mut s, 2, 0);
    let id = s.notes[0].id;
    assert_eq!(s.expression_intensities.get(&id).map(String::as_str), Some("0.75"));
    // Without an expression the armed depth has nothing to apply to.
    s.sticky_expr = Expr::None;
    select_or_add(&mut s, 3, 24);
    let id = s.notes[1].id;
    assert!(!s.expression_intensities.contains_key(&id));
}

#[test]
fn call_and_split_buttons_toggle_the_selected_notes_phrase() {
    let mut s = state_with_metadata();
    s.selected = vec![0]; // hole 1 at tick 0; hole 2 shares the onset
    apply_modifier(&mut s, ModButton::Call);
    assert!(s.phrase_annotations[&0].call);
    // The phrase's, not the note's: hole 2 sees it too.
    s.selected = vec![1];
    assert!(s.selected_call());
    apply_modifier(&mut s, ModButton::Split);
    assert!(s.phrase_annotations[&0].split);
    apply_modifier(&mut s, ModButton::Call);
    assert!(!s.phrase_annotations[&0].call);
    apply_modifier(&mut s, ModButton::Split);
    assert!(!s.phrase_annotations[&0].split);
    // Nothing selected: no phrase to toggle.
    s.selected.clear();
    apply_modifier(&mut s, ModButton::Call);
    assert_eq!(s.phrase_annotations.len(), 2);
}

#[test]
fn phrase_button_opens_the_editor_on_the_selected_notes_onset() {
    let mut s = state_with_metadata();
    s.selected = vec![2];
    apply_modifier(&mut s, ModButton::Phrase);
    assert_eq!(s.phrase_editor, Some(24));
    // And on an onset with no annotation yet — the way to add the first.
    let mut s = state_with_notes(vec![GridNote::plain(0, 1, 36, 4)]);
    s.selected = vec![0];
    apply_modifier(&mut s, ModButton::Phrase);
    assert_eq!(s.phrase_editor, Some(36));
    // Nothing selected: nothing to open.
    let mut s = EditorState::default();
    apply_modifier(&mut s, ModButton::Phrase);
    assert_eq!(s.phrase_editor, None);
}

// ── two_finger_pan_delta ──────────────────────────────────────────────────

use bevy::math::Vec2;

#[test]
fn one_finger_is_not_a_pan_gesture() {
    // A single touch is note placement/drag/resize — panning must not steal
    // it, which is the whole reason the gesture needs two fingers.
    assert_eq!(super::view_scroll::two_finger_pan_delta(&[Vec2::new(30.0, 0.0)]), None);
}

#[test]
fn no_touches_is_not_a_pan_gesture() {
    assert_eq!(super::view_scroll::two_finger_pan_delta(&[]), None);
}

#[test]
fn three_fingers_is_not_a_pan_gesture() {
    let deltas = [Vec2::new(5.0, 0.0), Vec2::new(5.0, 0.0), Vec2::new(5.0, 0.0)];
    assert_eq!(super::view_scroll::two_finger_pan_delta(&deltas), None);
}

#[test]
fn two_fingers_moving_together_pan_by_their_shared_motion() {
    // Both fingers drag 20px right; the view should follow by 20px, not 40.
    let deltas = [Vec2::new(20.0, 4.0), Vec2::new(20.0, -4.0)];
    let pan = super::view_scroll::two_finger_pan_delta(&deltas).expect("two touches pan");
    assert!((pan.x - 20.0).abs() < f32::EPSILON, "got {pan:?}");
}

#[test]
fn a_pinch_barely_pans() {
    // Fingers moving apart have near-opposite deltas, so averaging cancels
    // them — this is what lets a future pinch-zoom coexist with panning
    // instead of the view lurching sideways every time you zoom.
    let deltas = [Vec2::new(-25.0, 0.0), Vec2::new(25.0, 0.0)];
    let pan = super::view_scroll::two_finger_pan_delta(&deltas).expect("two touches pan");
    assert!(pan.x.abs() < f32::EPSILON, "pinch should not pan, got {pan:?}");
}

#[test]
fn an_off_centre_pinch_pans_only_by_its_drift() {
    // A pinch that also drifts right: one finger moves +30, the other -10,
    // so the gesture's real translation is +10.
    let deltas = [Vec2::new(30.0, 0.0), Vec2::new(-10.0, 0.0)];
    let pan = super::view_scroll::two_finger_pan_delta(&deltas).expect("two touches pan");
    assert!((pan.x - 10.0).abs() < f32::EPSILON, "got {pan:?}");
}

// ── vertical_overflow_px ──────────────────────────────────────────────────

#[test]
fn a_view_that_already_fits_cannot_be_panned_vertically() {
    // Desktop: everything fits, so the gesture must be inert rather than
    // letting the player drag the whole editor off the top of the window.
    assert_eq!(super::view_scroll::vertical_overflow_px(800.0, &[5.0, 424.0, 200.0]), 0.0);
}

#[test]
fn overflow_is_exactly_what_hangs_off_the_bottom() {
    // Phone-shaped: 5px progress bar + 424px chrome + 120px form = 549,
    // against ~400 of viewport, so 149px has to be reachable by panning.
    let overflow = super::view_scroll::vertical_overflow_px(400.0, &[5.0, 424.0, 120.0]);
    assert!((overflow - 149.0).abs() < f32::EPSILON, "got {overflow}");
}

#[test]
fn an_empty_root_has_no_overflow() {
    assert_eq!(super::view_scroll::vertical_overflow_px(400.0, &[]), 0.0);
}

#[test]
fn the_sticky_bend_cap_is_the_deepest_any_hole_of_the_harp_allows() {
    use super::interaction::deepest_bend;
    // Richter: hole 3's three semitones. Paddy Richter flattens hole 3 to
    // one, so its deepest is elsewhere — hole 2's two.
    assert_eq!(deepest_bend(&build_harp("C", HarmonicaKind::Diatonic)), 3.0);
    assert_eq!(deepest_bend(&build_harp("C", HarmonicaKind::PaddyRichter)), 2.0);
    // And the cycle actually stops there: on Richter, 3.0 wraps to Normal.
    let mut s = EditorState { sticky_pitch: Pitch::Bend(3.0), ..Default::default() };
    super::interaction::cycle_sticky_bend(&mut s);
    assert_eq!(s.sticky_pitch, Pitch::Normal);
}

#[test]
fn the_bend_button_respects_the_tuning_not_a_richter_table() {
    // Country tuning's raised draw 5 bends a semitone; Richter's hole 5
    // does not.
    let mut country =
        EditorState { harmonica_kind: HarmonicaKind::CountryTuned, ..Default::default() };
    select_or_add(&mut country, 5, 0);
    apply_modifier(&mut country, ModButton::Bend);
    assert_eq!(country.notes[0].pitch, Pitch::Bend(0.5), "country hole 5 bends");

    let mut richter = EditorState::default();
    select_or_add(&mut richter, 5, 0);
    apply_modifier(&mut richter, ModButton::Bend);
    assert_eq!(richter.notes[0].pitch, Pitch::Normal, "Richter hole 5 can't");

    // Paddy Richter's hole 3 bends one semitone, not Richter's three: the
    // cycle wraps to Normal after 1.0 instead of letting an author place a
    // bend the instrument can't make.
    let mut paddy =
        EditorState { harmonica_kind: HarmonicaKind::PaddyRichter, ..Default::default() };
    select_or_add(&mut paddy, 3, 0);
    for _ in 0..2 {
        apply_modifier(&mut paddy, ModButton::Bend);
    }
    assert_eq!(paddy.notes[0].pitch, Pitch::Bend(1.0));
    apply_modifier(&mut paddy, ModButton::Bend);
    assert_eq!(paddy.notes[0].pitch, Pitch::Normal);
}

#[test]
fn phrase_marker_prioritizes_section_chord_and_compact_technique_icons() {
    let annotation = PhraseAnnotation {
        section: Some("Bridge".into()),
        chord: Some("G7alt".into()),
        groove: Some("laid back".into()),
        lyric: Some("hal-".into()),
        call: true,
        split: true,
    };
    assert_eq!(
        super::annotation_lane::label(&annotation),
        "§ Bridge · \"hal-\" · ♬ G7alt · ↩ · TB · laid back"
    );
}

#[test]
fn dense_phrase_markers_clip_before_the_next_anchor() {
    assert_eq!(super::annotation_lane::width(100, Some(100)), 4.0);
    assert!(
        super::annotation_lane::width(100, Some(101))
            < super::annotation_lane::width(100, Some(200))
    );
    assert_eq!(super::annotation_lane::width(100, None), 150.0);
}

// ── technique buttons on a selection ─────────────────────────────────────────

/// Notes on `holes`, one per beat, all selected with the last as primary.
fn selection_on(holes: &[u8]) -> EditorState {
    let mut state = EditorState::default();
    for (beat, &hole) in holes.iter().enumerate() {
        select_or_add(&mut state, hole, beat * TICKS_PER_BEAT * 2);
    }
    state.selected = state.notes.iter().map(|n| n.id).collect();
    state
}

fn pitches(state: &EditorState) -> Vec<Pitch> {
    state.notes.iter().map(|n| n.pitch).collect()
}

#[test]
fn a_technique_button_applies_to_every_selected_note() {
    let mut state = selection_on(&[4, 5, 6]);
    apply_modifier(&mut state, ModButton::Vibrato);
    assert!(state.notes.iter().all(|n| matches!(n.expr, Expr::Vibrato(_))), "{:?}", state.notes);
    assert_eq!(state.technique_notice, None);
}

#[test]
fn notes_that_cannot_take_a_technique_keep_theirs_and_are_counted() {
    // Richter: hole 2 can't overblow; holes 4 and 5 can.
    let mut state = selection_on(&[4, 2, 5]);
    apply_modifier(&mut state, ModButton::Overblow);
    assert_eq!(pitches(&state), vec![Pitch::Overblow, Pitch::Normal, Pitch::Overblow]);
    assert_eq!(state.technique_notice, Some(1));
    assert!(state.notes.iter().all(|n| n.dir == Dir::Blow));

    // Pressed again, the primary is overblown, so it switches them all off.
    apply_modifier(&mut state, ModButton::Overblow);
    assert_eq!(pitches(&state), vec![Pitch::Normal; 3]);
}

#[test]
fn a_bend_steps_as_deep_as_any_selected_hole_allows() {
    // Draw 1 bends one semitone, draw 3 three; primary is hole 1.
    let mut state = selection_on(&[3, 1]);
    for n in &mut state.notes {
        n.dir = Dir::Draw;
    }
    apply_modifier(&mut state, ModButton::Bend);
    apply_modifier(&mut state, ModButton::Bend);
    assert_eq!(pitches(&state), vec![Pitch::Bend(1.0), Pitch::Bend(1.0)]);
    apply_modifier(&mut state, ModButton::Bend);
    assert_eq!(
        pitches(&state),
        vec![Pitch::Bend(1.5), Pitch::Bend(1.0)],
        "hole 1 stops at its own depth"
    );
    assert_eq!(state.technique_notice, Some(1));
}

#[test]
fn one_selected_note_that_cannot_take_it_is_silently_unchanged() {
    let mut state = selection_on(&[2]);
    apply_modifier(&mut state, ModButton::Overblow);
    assert_eq!(pitches(&state), vec![Pitch::Normal]);
    assert_eq!(state.technique_notice, None);
}

#[test]
fn depth_steps_every_selected_note_with_an_expression() {
    let mut state = selection_on(&[4, 5, 6]);
    for n in &mut state.notes[..2] {
        n.expr = Expr::Vibrato(5.0);
    }
    // Primary is hole 6, which has no expression: Depth does nothing.
    apply_modifier(&mut state, ModButton::Depth);
    assert!(state.expression_intensities.is_empty());

    state.selected.rotate_right(1); // hole 5 is primary now
    apply_modifier(&mut state, ModButton::Depth);
    let depth = |i: usize| state.expression_intensities.get(&state.notes[i].id).cloned();
    assert_eq!(depth(0), depth(1));
    assert!(depth(0).is_some());
    assert_eq!(depth(2), None);
    assert_eq!(state.technique_notice, Some(1));
}
