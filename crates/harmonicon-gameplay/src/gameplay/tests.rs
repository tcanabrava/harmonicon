// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use harmonicon_app::app::GameplayMode;
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::{AudioFrame, PitchAlgorithm, PitchInfo, PitchRange};
use harmonicon_core::chart::Modifier;
use harmonicon_core::midi::{midi_to_freq_hz, note_to_midi};
use harmonicon_core::scoring::{combo_label, compute_multiplier};

use super::bars::{beat_ticks_in_range, pickup_lead_ticks, ticks_per_beat};
use super::lifecycle::cleanup_gameplay;
use super::*;
use harmonicon_core::chart::HarpChart;

// ── TechniqueStats / SongStats::record_technique ───────────────────────

#[test]
fn technique_stats_accuracy_is_none_when_never_exercised() {
    assert_eq!(TechniqueStats::default().accuracy(), None);
}

#[test]
fn technique_stats_accuracy_divides_hits_by_total() {
    let s = TechniqueStats { hits: 3, misses: 1 };
    assert_eq!(s.total(), 4);
    assert!((s.accuracy().unwrap() - 0.75).abs() < 1e-6);
}

#[test]
fn record_technique_with_no_modifiers_goes_to_normal() {
    let mut stats = SongStats::default();
    stats.record_technique(&[], true);
    stats.record_technique(&[], false);
    assert_eq!(stats.normal.hits, 1);
    assert_eq!(stats.normal.misses, 1);
    assert_eq!(stats.bend.total(), 0);
}

#[test]
fn record_technique_routes_each_modifier_to_its_own_bucket() {
    let mut stats = SongStats::default();
    stats.record_technique(&[Modifier::Bend { semitones: -1.0, intensity: None }], true);
    stats.record_technique(&[Modifier::Overblow], false);
    stats.record_technique(&[Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }], true);
    stats.record_technique(&[Modifier::WahWah { oscillation_hz: 3.0, intensity: None }], true);
    stats.record_technique(&[Modifier::Overdraw], true);
    stats.record_technique(&[Modifier::Slide], true);

    assert_eq!(stats.bend.hits, 1);
    assert_eq!(stats.overblow.misses, 1);
    assert_eq!(stats.vibrato.hits, 1);
    assert_eq!(stats.wah.hits, 1);
    assert_eq!(stats.overdraw.hits, 1);
    assert_eq!(stats.slide.hits, 1);
    assert_eq!(stats.normal.total(), 0, "no plain notes were recorded");
}

#[test]
fn record_technique_with_two_modifiers_credits_both() {
    // A note that's both bent and vibrato'd counts as a data point for
    // both techniques' accuracy — hitting/missing it is informative for both.
    let mut stats = SongStats::default();
    stats.record_technique(
        &[
            Modifier::Bend { semitones: -1.0, intensity: None },
            Modifier::Vibrato { oscillation_hz: 5.0, intensity: None },
        ],
        true,
    );
    assert_eq!(stats.bend.hits, 1);
    assert_eq!(stats.vibrato.hits, 1);
}

// ── chart_meter ───────────────────────────────────────────────────────────────

/// A minimal chart with the given `song`-level and `timing`-level meter
/// fields, for exercising `chart_meter`'s precedence.
fn chart_with_meter(song_sig: Option<&str>, map_sig_at_zero: Option<&str>) -> HarpChart {
    let song_field = song_sig.map_or(String::new(), |s| format!(r#", "time_signature": "{s}""#));
    let map_field = map_sig_at_zero.map_or(String::new(), |s| {
        format!(r#", "time_signature_map": [{{"tick": 0, "time_signature": "{s}"}}]"#)
    });
    serde_json::from_str(&format!(
        r#"{{
            "song": {{ "title": "T", "artist": "A", "genre": "Test", "tempo_bpm": 120.0,
                      "key": "C", "difficulty": "easy"{song_field} }},
            "timing": {{ "resolution": 480, "tempo_map": [{{"tick": 0, "bpm": 120.0}}]{map_field} }},
            "harmonica": {{
                "type": "diatonic", "holes": 10,
                "bending_profile": "richter_standard",
                "layout": {{
                    "blow": ["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                    "draw": ["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]
                }}
            }},
            "track": [],
            "scoring": {{ "perfect_window_ms": 50, "good_window_ms": 100,
                         "miss_window_ms": 130 }}
        }}"#
    ))
    .unwrap()
}

#[test]
fn chart_meter_reads_the_song_field() {
    let m = chart_meter(&chart_with_meter(Some("3/4"), None));
    assert_eq!((m.numerator, m.denominator), (3, 4));
}

#[test]
fn chart_meter_defaults_to_common_time() {
    let m = chart_meter(&chart_with_meter(None, None));
    assert_eq!((m.numerator, m.denominator), (4, 4));
    // Unparseable falls back the same way rather than failing to load.
    let m = chart_meter(&chart_with_meter(Some("invalid"), None));
    assert_eq!((m.numerator, m.denominator), (4, 4));
}

#[test]
fn chart_meter_lets_the_map_at_tick_zero_win() {
    // The precedence `setup_scoring_config` always applied — and that the
    // metronome used to skip, so the two could disagree on one chart.
    let m = chart_meter(&chart_with_meter(Some("4/4"), Some("6/8")));
    assert_eq!((m.numerator, m.denominator), (6, 8));
}

#[test]
fn chart_meter_keeps_the_denominator() {
    // The bug the numerator-only readings had: 6/8 is six *eighths*, three
    // quarter-note beats — reading "6" as a beat count made a bar twice as
    // long as the music.
    let m = chart_meter(&chart_with_meter(Some("6/8"), None));
    assert_eq!(m.beats_per_bar(), 3.0);
    assert!((m.bar_secs(180.0) - 1.0).abs() < 1e-9);
}

#[test]
fn the_song_info_meter_follows_the_map_like_everything_else() {
    use super::song_info::time_signature_label;
    assert_eq!(time_signature_label(&chart_with_meter(Some("4/4"), Some("6/8"))), "6/8");
    assert_eq!(time_signature_label(&chart_with_meter(Some("3/4"), None)), "3/4");
    assert_eq!(time_signature_label(&chart_with_meter(None, None)), "4/4");
}

// ── beat guides (tick-space beat placement) ──────────────────────────────

#[test]
fn ticks_per_beat_is_the_meters_own_beat_not_a_quarter() {
    // 480 ticks per quarter note. In 4/4 a beat *is* a quarter; in 6/8 it's
    // an eighth, so half as many ticks — the same rule `MusicScoreMeter::
    // beat_secs` applies in seconds. Reading the numerator alone and calling
    // it a beat count is the mistake `chart_meter`'s comment documents.
    assert_eq!(ticks_per_beat(480, &chart_meter(&chart_with_meter(Some("4/4"), None))), 480);
    assert_eq!(ticks_per_beat(480, &chart_meter(&chart_with_meter(Some("6/8"), None))), 240);
    assert_eq!(ticks_per_beat(480, &chart_meter(&chart_with_meter(Some("3/4"), None))), 480);
}

#[test]
fn ticks_per_beat_never_returns_zero() {
    // A denominator big enough to divide below one tick would otherwise make
    // the beat stride zero, and `beat_ticks_in_range` loop forever on it.
    assert!(ticks_per_beat(1, &chart_meter(&chart_with_meter(Some("4/64"), None))) >= 1);
    assert!(ticks_per_beat(0, &chart_meter(&chart_with_meter(Some("4/4"), None))) >= 1);
}

#[test]
fn beat_ticks_marks_every_bar_start_as_a_downbeat() {
    // Two bars of 4/4 at 480 ticks per beat: beats 0 and 4 start a bar.
    let beats = beat_ticks_in_range(0, 480 * 7, 480, 4, 0).collect::<Vec<_>>();
    assert_eq!(beats.len(), 8);
    let downbeats: Vec<u64> =
        beats.iter().filter(|(_, is_downbeat)| *is_downbeat).map(|(tick, _)| *tick).collect();
    assert_eq!(downbeats, vec![0, 480 * 4]);
}

#[test]
fn beat_ticks_counts_the_bar_from_tick_zero_not_from_the_window() {
    // A window starting mid-bar must not relabel its first visible beat as a
    // downbeat — the bar phase belongs to the song, not to whatever happens
    // to be on screen.
    let beats = beat_ticks_in_range(480 * 5, 480 * 9, 480, 4, 0).collect::<Vec<_>>();
    assert_eq!(beats.first(), Some(&(480 * 5, false)));
    assert!(beats.contains(&(480 * 8, true)), "beat 8 starts bar 3: {beats:?}");
}

#[test]
fn a_pickup_puts_the_first_downbeat_where_it_ends() {
    // A one-beat pickup in 4/4 is bar 0's last beat: the grid starts three
    // beats before tick 0, so tick 480 is bar 1's downbeat.
    let mut chart = chart_with_meter(Some("4/4"), None);
    chart.timing.pickup_ticks = Some(480);
    let lead = pickup_lead_ticks(&chart);
    assert_eq!(lead, 480 * 3);
    let beats = beat_ticks_in_range(0, 480 * 5, 480, 4, lead).collect::<Vec<_>>();
    assert_eq!(beats.first(), Some(&(0, false)), "the pickup's own beat");
    let downbeats: Vec<u64> =
        beats.iter().filter(|(_, is_downbeat)| *is_downbeat).map(|(tick, _)| *tick).collect();
    assert_eq!(downbeats, vec![480, 480 * 5]);
}

#[test]
fn a_pickup_ends_the_staffs_first_bar() {
    // 4/4 at 120 with a one-beat pickup (lead three beats): the pickup note
    // is beat 3 of the staff, and a half note from bar 1's last beat ties
    // at bar 2's line (staff beat 8).
    let tempo = [harmonicon_core::chart::TempoPoint { tick: 0, bpm: 120.0 }];
    let mut pickup = loop_test_note(0.0);
    pickup.duration = 0.5;
    let mut across = loop_test_note(2.0);
    across.duration = 1.0;
    let bar_map = harmonicon_ui::music_score::MusicScoreBarMap {
        meter: harmonicon_ui::music_score::MeterMap::with_pickup([(0, "4/4")], 480, 480),
        quarter_ticks: 480,
    };
    let staff = super::notes_to_notation(&[pickup, across], 480, &tempo, &bar_map);
    let starts: Vec<f64> = staff.iter().map(|n| n.start_beat).collect();
    assert_eq!(starts, vec![3.0, 7.0, 8.0]);
    assert!(staff[2].tied_from_previous);
}

#[test]
fn a_pickup_leads_by_the_meters_own_beat() {
    // Two eighths of 6/8 leave four eighths of bar 0 unplayed.
    let mut chart = chart_with_meter(Some("6/8"), None);
    chart.timing.pickup_ticks = Some(480);
    assert_eq!(pickup_lead_ticks(&chart), 240 * 4);
    chart.timing.pickup_ticks = None;
    assert_eq!(pickup_lead_ticks(&chart), 0);
    // A whole bar of pickup is no offset at all.
    chart.timing.pickup_ticks = Some(240 * 6);
    assert_eq!(pickup_lead_ticks(&chart), 0);
}

#[test]
fn beat_ticks_includes_a_beat_exactly_on_the_window_edge() {
    assert_eq!(beat_ticks_in_range(480, 480, 480, 4, 0).collect::<Vec<_>>(), vec![(480, false)]);
}

#[test]
fn beat_ticks_is_empty_for_an_inverted_or_degenerate_window() {
    assert!(beat_ticks_in_range(960, 480, 480, 4, 0).collect::<Vec<_>>().is_empty());
    assert!(beat_ticks_in_range(0, 960, 0, 4, 0).collect::<Vec<_>>().is_empty());
}

#[test]
fn beat_ticks_treats_a_zero_beat_count_as_one_bar_per_beat() {
    // `numerator.max(1)` upstream should make this unreachable; guard the
    // modulo anyway rather than divide by zero if a chart ever says 0/4.
    let beats = beat_ticks_in_range(0, 480 * 2, 480, 0, 0).collect::<Vec<_>>();
    assert!(beats.iter().all(|(_, is_downbeat)| *is_downbeat), "{beats:?}");
}

// `advance_clock`'s own tests live in `clock.rs` alongside the type.

// ── handle_loop_boundary ─────────────────────────────────────────────────

/// No `AudioSink`/`MusicPlayer` entity is spawned in these tests —
/// `handle_loop_boundary`'s seek-on-wrap degrades gracefully to a no-op
/// when no sink exists (as before the music sink spawns, or if audio init
/// failed), so clock/note-reset behaviour is testable headlessly; the
/// actual seek call needs a live sink and is a manual check (see
/// `docs/gameplay_validation.md`).
fn loop_test_note(time: f64) -> ScheduledNote {
    ScheduledNote {
        time,
        duration: 0.5,
        hole: 1,
        is_blow: true,
        expected_pitch: Some(60), // C4
        hit: true,
        missed: true,
        held: 1.0,
        sustain_scored: true,
        modifiers: Vec::new(),
        pitch_samples: vec![(0.0, 440.0)],
        amp_samples: vec![(0.0, 0.5)],
        phrase_section: 0,
        chord_pitches: Vec::new(),
        playable: true,
        // Pre-dirtied like the other resolved state above, so the reset
        // assertions below cover the failure attribution too.
        miss_evidence: Some(MissReason::WrongPitch {
            expected: HoleTab { hole: 1, is_blow: true },
            heard: None,
        }),
        force_wait: false,
    }
}

// ── start_at_practice_range ──────────────────────────────────────────────

fn pending_note(time: f64) -> ScheduledNote {
    ScheduledNote {
        hit: false,
        missed: false,
        sustain_scored: false,
        held: 0.0,
        miss_evidence: None,
        ..loop_test_note(time)
    }
}

#[test]
fn skip_notes_before_resolves_exactly_the_prefix() {
    let mut hit_early = pending_note(1.0);
    hit_early.hit = true;
    let mut notes = vec![pending_note(0.5), hit_early, pending_note(4.0), pending_note(4.5)];
    super::lifecycle::skip_notes_before(&mut notes, 4.0);
    assert!(notes[0].missed, "an unplayed note before the range is skipped");
    assert!(notes[1].hit && !notes[1].missed, "an already-hit note keeps its hit");
    assert!(!notes[2].missed && !notes[3].missed, "notes from the range start on stay pending");
}

#[test]
fn practice_start_jumps_the_clock_once_music_is_running() {
    use harmonicon_app::app::SelectedSong;
    use harmonicon_song::song::SongManifest;

    let mut world = World::new();
    world.insert_resource(PracticeRequest(Some(PracticeRange { start_time: 8.0, end_time: 12.0 })));
    world.insert_resource(MusicStarted(false));
    world.insert_resource(SelectedSong(Handle::default()));
    world.insert_resource(Assets::<SongManifest>::default());
    world.insert_resource(GameplayClock::new(-1.0));
    world.insert_resource(SongNotes {
        notes: vec![pending_note(2.0), pending_note(9.0)],
        cursor: 0,
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(super::lifecycle::start_at_practice_range);

    // Still counting down: nothing happens, the request is kept.
    schedule.run(&mut world);
    assert_eq!(world.resource::<GameplayClock>().get(), -1.0);
    assert!(world.resource::<PracticeRequest>().0.is_some());

    // Music on (no sink: an unknown manifest reads as a music-less song, so
    // there's nothing to wait for): jump, skip the prefix, consume.
    world.resource_mut::<MusicStarted>().0 = true;
    schedule.run(&mut world);
    assert_eq!(world.resource::<GameplayClock>().get(), 8.0);
    assert_eq!(world.resource::<PracticeRequest>().0, None);
    let notes = &world.resource::<SongNotes>().notes;
    assert!(notes[0].missed && !notes[1].missed);

    // A second run is a no-op: the jump happens once per request.
    world.resource_mut::<GameplayClock>().set_free(9.5);
    schedule.run(&mut world);
    assert_eq!(world.resource::<GameplayClock>().get(), 9.5);
}

#[test]
fn an_active_loop_holds_off_the_results_screen_and_clearing_it_releases() {
    use bevy::state::app::StatesPlugin;
    use harmonicon_app::app::AppState;

    let mut app = App::new();
    app.add_plugins(StatesPlugin).init_state::<AppState>();
    app.insert_resource(GameplayClock::new(30.0));
    app.insert_resource(SongEnd(20.0));
    app.insert_resource(MusicStarted(true));
    app.insert_resource(GameplayMode::Play2D);
    app.insert_resource(LoopConfig { active: true, start_time: 2.0, end_time: 10.0 });
    app.add_systems(Update, super::lifecycle::detect_song_end);

    app.update();
    assert!(
        matches!(*app.world().resource::<NextState<AppState>>(), NextState::Unchanged),
        "a looping song must not finish, however far past SongEnd the clock is"
    );

    *app.world_mut().resource_mut::<LoopConfig>() = LoopConfig::default();
    app.update();
    assert!(
        matches!(
            *app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::Results)
        ),
        "with the loop cleared, the song past its end goes to Results"
    );
}

#[test]
fn loop_boundary_rewinds_the_clock_and_resets_notes_in_range() {
    let mut world = World::new();
    world.insert_resource(LoopConfig { active: true, start_time: 2.0, end_time: 10.0 });
    world.insert_resource(GameplayClock::new(10.0));
    // Sorted by time, as `SongNotes` requires: before-range, in-range,
    // just-past-range-but-within-LOOKAHEAD (still gets a reset — see
    // `loop_reset_range`), and genuinely far beyond it.
    world.insert_resource(SongNotes {
        notes: vec![
            loop_test_note(1.0),
            loop_test_note(5.0),
            loop_test_note(11.0),
            loop_test_note(20.0),
        ],
        cursor: 4, // as if all four had already resolved and rolled off.
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(handle_loop_boundary);
    schedule.run(&mut world);

    assert_eq!(world.resource::<GameplayClock>().get(), 2.0);

    let song_notes = world.resource::<SongNotes>();
    for i in [1, 2] {
        let reset = &song_notes.notes[i];
        assert!(
            !reset.hit && !reset.missed && !reset.sustain_scored && reset.held == 0.0,
            "note {i} (in range or a LOOKAHEAD preview past it) should be reset"
        );
        assert!(
            reset.pitch_samples.is_empty() && reset.amp_samples.is_empty(),
            "note {i} must not carry the previous lap's sustain samples"
        );
        assert!(
            reset.miss_evidence.is_none(),
            "note {i} must not carry the previous lap's failure attribution"
        );
    }
    assert_eq!(song_notes.cursor, 1, "cursor rewinds to the in-range note");

    let untouched = &song_notes.notes[0];
    assert!(
        untouched.hit && untouched.missed && untouched.sustain_scored,
        "a note before start_time must not be reset"
    );
    let untouched = &song_notes.notes[3];
    assert!(
        untouched.hit && untouched.missed && untouched.sustain_scored,
        "a note well beyond end_time + LOOKAHEAD must not be reset"
    );
}

#[test]
fn loop_boundary_is_a_no_op_before_end_time_or_when_inactive() {
    let mut world = World::new();
    world.insert_resource(LoopConfig { active: true, start_time: 2.0, end_time: 10.0 });
    world.insert_resource(GameplayClock::new(9.999));
    world.insert_resource(SongNotes::default());
    let mut schedule = Schedule::default();
    schedule.add_systems(handle_loop_boundary);
    schedule.run(&mut world);
    assert_eq!(world.resource::<GameplayClock>().get(), 9.999);

    let mut world = World::new();
    world.insert_resource(LoopConfig { active: false, start_time: 2.0, end_time: 10.0 });
    world.insert_resource(SongNotes::default());
    world.insert_resource(GameplayClock::new(10.0));
    let mut schedule = Schedule::default();
    schedule.add_systems(handle_loop_boundary);
    schedule.run(&mut world);
    assert_eq!(world.resource::<GameplayClock>().get(), 10.0);
}

#[test]
fn current_bar_index_at_zero() {
    assert_eq!(current_bar_index(0.0, 2.0), 0);
}

#[test]
fn current_bar_index_advances() {
    assert_eq!(current_bar_index(2.0, 2.0), 1);
    assert_eq!(current_bar_index(4.0, 2.0), 2);
}

#[test]
fn current_bar_index_wraps_at_12() {
    // 12 bars × 2 s/bar = 24 s → wraps back to bar 0
    assert_eq!(current_bar_index(24.0, 2.0), 0);
}

#[test]
fn current_bar_index_clamps_negative_clock() {
    // During countdown the clock is negative — should give bar 0
    assert_eq!(current_bar_index(-1.5, 2.0), 0);
}

// ── should_anchor_to_sink (tick_clock's audio-anchoring gate) ────────────

#[test]
fn anchors_once_playing_with_a_nonempty_sink() {
    assert!(should_anchor_to_sink(1.0, true, &GameplayMode::Play2D, false));
}

#[test]
fn does_not_anchor_during_the_countdown() {
    assert!(!should_anchor_to_sink(-1.0, true, &GameplayMode::Play2D, false));
}

#[test]
fn does_not_anchor_before_music_started() {
    assert!(!should_anchor_to_sink(1.0, false, &GameplayMode::Play2D, false));
}

#[test]
fn does_not_anchor_in_jam_session() {
    assert!(!should_anchor_to_sink(1.0, true, &GameplayMode::JamSession, false));
}

#[test]
fn does_not_anchor_once_the_sink_is_empty() {
    // A finished sink's reported position freezes rather than continuing
    // to advance — anchoring to it would repeatedly snap the clock back
    // once real time drifts past it.
    assert!(!should_anchor_to_sink(1.0, true, &GameplayMode::Play2D, true));
}

// ── loop_range_valid (progress-bar drag loop range) ──────────────────────

#[test]
fn loop_range_valid_requires_end_strictly_after_start() {
    assert!(loop_range_valid(4.0, 8.0));
    assert!(!loop_range_valid(8.0, 8.0));
    assert!(!loop_range_valid(8.0, 4.0));
}

// ── resolve_item_time ───────────────────────────────────────────────────────

use harmonicon_core::chart::{TempoPoint, Timing, TrackItem};

fn track_item(time: Option<f64>, tick: Option<u64>) -> TrackItem {
    TrackItem {
        id: None,
        time,
        tick,
        duration: 0.5,
        phrase: None,
        groove: None,
        chord: None,
        play_mode: None,
        call: false,
        lyric: None,
        events: vec![],
    }
}

fn timing_120bpm() -> Timing {
    Timing {
        resolution: 480,
        tempo_map: vec![TempoPoint { tick: 0, bpm: 120.0 }],
        time_signature_map: None,
        pickup_ticks: None,
        repeats: Vec::new(),
    }
}

#[test]
fn resolve_item_time_prefers_explicit_time() {
    let item = track_item(Some(2.5), Some(9999));
    assert!((resolve_item_time(&item, &timing_120bpm()) - 2.5).abs() < 1e-9);
}

#[test]
fn resolve_item_time_falls_back_to_tick() {
    // One quarter note (480 ticks) at 120 BPM = 0.5 s
    let item = track_item(None, Some(480));
    assert!((resolve_item_time(&item, &timing_120bpm()) - 0.5).abs() < 1e-9);
}

#[test]
fn resolve_item_time_defaults_missing_tick_to_zero() {
    let item = track_item(None, None);
    assert_eq!(resolve_item_time(&item, &timing_120bpm()), 0.0);
}

// ── last_note_end ─────────────────────────────────────────────────────────────

#[test]
fn last_note_end_is_latest_finish() {
    // Items at 0.0 and 2.0, each 0.5 s long → latest finish is 2.5 s.
    let track = vec![track_item(Some(0.0), None), track_item(Some(2.0), None)];
    assert!((last_note_end(&track, &timing_120bpm()) - 2.5).abs() < 1e-9);
}

#[test]
fn last_note_end_ignores_order() {
    // The latest end wins even when the longest note isn't last in the track.
    let track = vec![track_item(Some(5.0), None), track_item(Some(1.0), None)];
    assert!((last_note_end(&track, &timing_120bpm()) - 5.5).abs() < 1e-9);
}

#[test]
fn last_note_end_empty_track_is_zero() {
    assert_eq!(last_note_end(&[], &timing_120bpm()), 0.0);
}

// ── modifier_fx_key ───────────────────────────────────────────────────────────

#[test]
fn modifier_fx_keys_match_technique_names() {
    use harmonicon_core::chart::Modifier::*;
    assert_eq!(modifier_fx_key(&Bend { semitones: -1.0, intensity: None }), "bend");
    assert_eq!(modifier_fx_key(&Vibrato { oscillation_hz: 5.0, intensity: None }), "vibrato");
    assert_eq!(modifier_fx_key(&WahWah { oscillation_hz: 3.0, intensity: None }), "wah-wah");
    assert_eq!(modifier_fx_key(&Overblow), "overblow");
    assert_eq!(modifier_fx_key(&Overdraw), "overdraw");
    assert_eq!(modifier_fx_key(&Slide), "slide");
}

// `PitchGate` is now a thin `Resource` wrapper around the shared
// `AttackGate` (see `harmonicon_core::scoring`) — its re-attack-detection behaviour
// is covered by `AttackGate`'s own tests there, not duplicated here.

// ── target_pitch (bend validation) ───────────────────────────────────────────

#[test]
fn bend_targets_the_bent_pitch() {
    let bend = vec![Modifier::Bend { semitones: -1.0, intensity: None }];
    // A 1-semitone draw bend on B4 (71) must be played as A#4 (70), not
    // the natural B4.
    assert_eq!(target_pitch("B4", &bend), Some(70));
}

#[test]
fn deeper_bend_targets_lower_pitch() {
    let bend = vec![Modifier::Bend { semitones: -2.0, intensity: None }];
    assert_eq!(target_pitch("B4", &bend), Some(69)); // A4
}

#[test]
fn non_bend_techniques_keep_the_natural_pitch() {
    let vib = vec![Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }];
    assert_eq!(target_pitch("D5", &vib), Some(74));
    assert_eq!(target_pitch("D5", &[]), Some(74));
}

#[test]
fn unknown_pitch_name_has_no_target() {
    // The "—" placeholder for a hole/direction the harp can't produce
    // isn't a parseable note name, so there's no valid target at all —
    // this note can never be hit.
    let bend = vec![Modifier::Bend { semitones: -1.0, intensity: None }];
    assert_eq!(target_pitch("\u{2014}", &bend), None);
}

// ── style_bonus_points ───────────────────────────────────────────────────────

fn bonus_table() -> HashMap<String, f32> {
    [("bend".to_string(), 50.0), ("vibrato".to_string(), 25.0)].into_iter().collect()
}

#[test]
fn style_bonus_sums_matched_techniques() {
    let mods = vec![
        Modifier::Bend { semitones: -1.0, intensity: None },
        Modifier::Vibrato { oscillation_hz: 5.0, intensity: None },
    ];
    assert_eq!(style_bonus_points(&mods, &bonus_table()), 75.0);
}

#[test]
fn style_bonus_ignores_techniques_absent_from_the_table() {
    let mods = vec![Modifier::WahWah { oscillation_hz: 3.0, intensity: None }];
    assert_eq!(style_bonus_points(&mods, &bonus_table()), 0.0);
}

#[test]
fn style_bonus_is_zero_without_modifiers() {
    assert_eq!(style_bonus_points(&[], &bonus_table()), 0.0);
}

// ── sustained-technique validation (vibrato / wah) ──────────────────────────

#[test]
fn vibrato_and_wah_are_sustained_bend_and_overblow_are_not() {
    let vibrato = Modifier::Vibrato { oscillation_hz: 5.0, intensity: None };
    let wah = Modifier::WahWah { oscillation_hz: 3.0, intensity: None };
    let bend = Modifier::Bend { semitones: -1.0, intensity: None };
    assert!(is_sustained_technique(&vibrato));
    assert!(is_sustained_technique(&wah));
    assert!(!is_sustained_technique(&bend));
    assert!(!is_sustained_technique(&Modifier::Slide));
    assert!(!is_sustained_technique(&Modifier::Overblow));
    assert!(!is_sustained_technique(&Modifier::Overdraw));
}

// Timestamped sine samples around `offset`, `n` samples spaced `dt` seconds apart.
fn timestamped_sine(
    freq_hz: f32,
    offset: f32,
    amplitude: f32,
    n: usize,
    dt: f64,
) -> Vec<(f64, f32)> {
    (0..n)
        .map(|i| {
            let t = i as f64 * dt;
            let v = offset + amplitude * (2.0 * std::f32::consts::PI * freq_hz * t as f32).sin();
            (t, v)
        })
        .collect()
}

#[test]
fn technique_confirmed_requires_real_wobble_for_vibrato() {
    let vibrato = Modifier::Vibrato { oscillation_hz: 5.0, intensity: None };
    let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.0)).collect();
    let wobbling = timestamped_sine(5.0, 0.0, 25.0, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&vibrato, &steady, &[]));
    assert!(technique_confirmed(&vibrato, &wobbling, &[]));
}

#[test]
fn technique_confirmed_requires_real_wobble_for_wah() {
    let wah = Modifier::WahWah { oscillation_hz: 3.0, intensity: None };
    let steady_volume: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.2)).collect();
    let pumping_volume = timestamped_sine(3.0, 0.2, 0.06, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&wah, &[], &steady_volume));
    assert!(technique_confirmed(&wah, &[], &pumping_volume));
}

#[test]
fn live_technique_status_distinguishes_unmeasurable_from_wrong() {
    let vibrato = Modifier::Vibrato { oscillation_hz: 5.0, intensity: None };
    let modifiers = vec![vibrato.clone()];
    // Nothing sustained declared (an overblow is judged at onset), or
    // nothing measurable yet: no reading, not a fail.
    assert_eq!(live_technique_status(&[Modifier::Overblow], &[], &[]), None);
    assert_eq!(live_technique_status(&[], &[], &[]), None);
    let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.0)).collect();
    assert_eq!(live_technique_status(&modifiers, &steady, &[]), None);
    assert!(
        !technique_confirmed(&vibrato, &steady, &[]),
        "…but the end-of-hold verdict still counts a never-measured wobble as not played"
    );
    // A wobble at the declared rate reads as confirmed; at the wrong rate,
    // as heard-but-wrong — the two the highway paints differently.
    let right = timestamped_sine(5.0, 0.0, 25.0, 40, 1.0 / 60.0);
    let wrong = timestamped_sine(1.5, 0.0, 25.0, 40, 1.0 / 60.0);
    assert_eq!(live_technique_status(&modifiers, &right, &[]), Some(true));
    assert_eq!(live_technique_status(&modifiers, &wrong, &[]), Some(false));
    // Two declared techniques: both have to be right.
    let both = vec![vibrato, Modifier::WahWah { oscillation_hz: 3.0, intensity: None }];
    let pumping = timestamped_sine(3.0, 0.2, 0.06, 40, 1.0 / 60.0);
    assert_eq!(live_technique_status(&both, &right, &pumping), Some(true));
    assert_eq!(live_technique_status(&both, &wrong, &pumping), Some(false));
}

#[test]
fn technique_confirmed_rejects_vibrato_at_the_wrong_rate() {
    // The chart declares a 5 Hz vibrato, but the player wobbled at ~1.5 Hz
    // — real oscillation, just not the declared rate. A flip-count-only
    // check couldn't tell these apart.
    let vibrato = Modifier::Vibrato { oscillation_hz: 5.0, intensity: None };
    let slow_wobble = timestamped_sine(1.5, 0.0, 25.0, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&vibrato, &slow_wobble, &[]));
}

#[test]
fn technique_confirmed_rejects_wah_at_the_wrong_rate() {
    let wah = Modifier::WahWah { oscillation_hz: 3.0, intensity: None };
    let fast_pumping = timestamped_sine(9.0, 0.2, 0.06, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&wah, &[], &fast_pumping));
}

#[test]
fn technique_confirmed_is_always_true_for_onset_validated_modifiers() {
    // Bend/overblow/overdraw/slide are judged at onset, not from the
    // sustain buffers — this should never gate them on empty/steady samples.
    assert!(technique_confirmed(&Modifier::Bend { semitones: -1.0, intensity: None }, &[], &[]));
    assert!(technique_confirmed(&Modifier::Overblow, &[], &[]));
    assert!(technique_confirmed(&Modifier::Slide, &[], &[]));
}

fn pitch_info(midi: u8, note: &str, octave: i32, frequency: f32) -> PitchInfo {
    PitchInfo { midi, note: note.into(), octave, frequency }
}

#[test]
fn active_frequency_for_matches_by_midi_number() {
    let active = vec![pitch_info(62, "D", 4, 293.66), pitch_info(67, "G", 4, 392.00)];
    assert_eq!(active_frequency_for(&active, 62), Some(293.66));
    assert_eq!(active_frequency_for(&active, 69), None);
}

// ── cleanup_gameplay ──────────────────────────────────────────────────────────

#[test]
fn cleanup_despawns_only_gameplay_entities() {
    // Leaving Playing must tear down the scene (every `GameplayRoot`) while
    // leaving unrelated entities (e.g. the persistent camera) untouched.
    let mut world = World::new();
    world.init_resource::<PitchRange>();
    // Cleanup also expires the per-song harmonica choice, so the resource
    // has to exist here as it does in the real plugin.
    world.init_resource::<harmonicon_app::app::EffectiveHarmonica>();
    let scene_a = world.spawn(GameplayRoot).id();
    let scene_b = world.spawn((GameplayRoot, Transform::default())).id();
    let keep = world.spawn_empty().id();

    let mut schedule = Schedule::default();
    schedule.add_systems(cleanup_gameplay);
    schedule.run(&mut world);

    assert!(!world.entities().contains(scene_a), "GameplayRoot should be despawned");
    assert!(!world.entities().contains(scene_b), "GameplayRoot should be despawned");
    assert!(world.entities().contains(keep), "unrelated entities must survive");
}

// ── score_notes (same-pitch overlap ordering) ───────────────────────────

pub(super) fn overlap_test_note(time: f64) -> ScheduledNote {
    ScheduledNote {
        time,
        duration: 1.0,
        hole: 1,
        is_blow: true,
        expected_pitch: Some(60), // C4
        hit: false,
        missed: false,
        held: 0.0,
        sustain_scored: false,
        modifiers: Vec::new(),
        pitch_samples: Vec::new(),
        amp_samples: Vec::new(),
        phrase_section: 0,
        chord_pitches: Vec::new(),
        playable: true,
        miss_evidence: None,
        force_wait: false,
    }
}

#[test]
fn score_notes_credits_the_closest_offset_when_two_same_pitch_notes_overlap() {
    // Two C4 notes both sit inside the hit window at clock=0.5 while C4 is
    // sounding: one 0.01s away (should score), one 0.10s away (should
    // stay `Waiting` — the pitch is fresh only once). Array order alone
    // would coincidentally put the closer note second too, so this
    // checks that classification actually goes by |offset|, not array
    // position.
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(0.5));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(vec![PitchInfo {
        midi: 60,
        note: "C".to_string(),
        octave: 4,
        frequency: midi_to_freq_hz(60.0),
    }]));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from([60u8])));
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.insert_resource(PlayedHarp(Some(harmonicon_core::harmonica::richter_harp("C"))));
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes {
        // Sorted by time: index 0 is farther from `judged` (offset
        // -0.10), index 1 is closer (offset -0.01).
        notes: vec![overlap_test_note(0.40), overlap_test_note(0.49)],
        cursor: 0,
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    let song_notes = world.resource::<SongNotes>();
    assert!(song_notes.notes[1].hit, "the note actually due should be credited");
    assert!(
        !song_notes.notes[0].hit,
        "the farther note must not steal the attack meant for the closer one"
    );
}

#[test]
fn score_notes_leaves_a_far_future_note_untouched() {
    // A note well beyond `good_window` classifies as `TooEarly` — a no-op
    // — so it's skipped before the sort/classify pass entirely (the
    // optimization for long charts). Confirm that skip doesn't change its
    // observable state: still neither hit nor missed.
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(0.0));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(vec![]));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from([60u8])));
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.insert_resource(PlayedHarp(Some(harmonicon_core::harmonica::richter_harp("C"))));
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes { notes: vec![overlap_test_note(120.0)], cursor: 0 });

    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    let song_notes = world.resource::<SongNotes>();
    assert!(!song_notes.notes[0].hit);
    assert!(!song_notes.notes[0].missed);
}

// ── score_notes (clean-attack tallying) ──────────────────────────────────

/// Builds a world set up to score one `overlap_test_note` at clock=0.5
/// against whatever `ActivePitches` the caller supplies, for the
/// clean-attack tests below.
fn clean_attack_test_world(active: Vec<PitchInfo>) -> World {
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(0.5));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(active));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 64u8])));
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.insert_resource(PlayedHarp(Some(harmonicon_core::harmonica::richter_harp("C"))));
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes { notes: vec![overlap_test_note(0.49)], cursor: 0 });
    world
}

#[test]
fn score_notes_counts_a_solo_pitch_as_a_clean_attack() {
    let mut world = clean_attack_test_world(vec![pitch_info(60, "C", 4, midi_to_freq_hz(60.0))]);
    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    assert!(world.resource::<SongNotes>().notes[0].hit, "should still hit");
    let stats = world.resource::<SongStats>();
    assert_eq!(stats.clean_attack.hits, 1);
    assert_eq!(stats.clean_attack.misses, 0);
}

#[test]
fn score_notes_counts_a_breathy_leak_as_a_hit_but_not_a_clean_attack() {
    // A second, unintended harp-producible pitch (64 = E4) sounds
    // alongside the expected one (60 = C4): the note still scores — the
    // expected pitch is present and on time — but it must not count
    // toward `clean_attack`.
    let mut world = clean_attack_test_world(vec![
        pitch_info(60, "C", 4, midi_to_freq_hz(60.0)),
        pitch_info(64, "E", 4, midi_to_freq_hz(64.0)),
    ]);
    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    assert!(
        world.resource::<SongNotes>().notes[0].hit,
        "the expected pitch was present and on time, so it should still hit"
    );
    let stats = world.resource::<SongStats>();
    assert_eq!(stats.clean_attack.hits, 0);
    assert_eq!(stats.clean_attack.misses, 1);
}

// ── score_notes (chord-target simultaneity) ──────────────────────────────

/// Two `ScheduledNote`s from one chord `TrackItem` (same `time`, sharing
/// `chord_pitches: [60, 64]`), one per sibling pitch — the shape
/// `gameplay_2d::build_combined_notes`/`gameplay_3d::build_notes_3d`
/// actually produce for a multi-event item.
fn chord_test_notes() -> Vec<ScheduledNote> {
    let base = overlap_test_note(0.49);
    vec![
        ScheduledNote {
            hole: 1,
            expected_pitch: Some(60),
            chord_pitches: vec![60, 64],
            ..base.clone()
        },
        ScheduledNote { hole: 2, expected_pitch: Some(64), chord_pitches: vec![60, 64], ..base },
    ]
}

fn chord_test_world(active: Vec<PitchInfo>) -> World {
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(0.5));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(active));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 64u8])));
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.insert_resource(PlayedHarp(Some(harmonicon_core::harmonica::richter_harp("C"))));
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes { notes: chord_test_notes(), cursor: 0 });
    world
}

#[test]
fn score_notes_hits_both_chord_notes_when_both_pitches_sound_together() {
    let mut world = chord_test_world(vec![
        pitch_info(60, "C", 4, midi_to_freq_hz(60.0)),
        pitch_info(64, "E", 4, midi_to_freq_hz(64.0)),
    ]);
    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    let notes = &world.resource::<SongNotes>().notes;
    assert!(notes[0].hit, "60 should hit — both pitches sounded together");
    assert!(notes[1].hit, "64 should hit — both pitches sounded together");
    let stats = world.resource::<SongStats>();
    assert_eq!(stats.clean_attack.total(), 0, "chord notes aren't clean-attack notes");
}

#[test]
fn score_notes_judges_every_note_of_a_frame_at_one_instant() {
    // `score_notes` stops scanning at the first note past the good window,
    // which is only sound while `judged` is a single instant: notes are
    // sorted by `time`, so a judgment time that varies per note makes the
    // offsets non-monotonic and the scan can break before a later note that
    // was still in range. A chord is where that shows: its two notes share a
    // `time`, so they must always resolve together, at every distance from
    // the window's edge.
    let chord_hit = |chord_time: f64| {
        let mut world = chord_test_world(vec![
            pitch_info(60, "C", 4, midi_to_freq_hz(60.0)),
            pitch_info(64, "E", 4, midi_to_freq_hz(64.0)),
        ]);
        let mut filter = HarmonicaPitchFilter::default();
        filter.configure(harmonicon_core::harmonica::richter_harp("C"), Some(44_100));
        world.insert_resource(filter);
        world.resource_mut::<AudioSettings>().pitch_algorithm = PitchAlgorithm::Nmf;
        for note in &mut world.resource_mut::<SongNotes>().notes {
            note.time = chord_time;
        }
        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);
        let notes = &world.resource::<SongNotes>().notes;
        (notes[0].hit, notes[1].hit)
    };

    // Sweep the chord across the good-window edge. Whatever the verdict at
    // each step, both halves must reach the same one.
    let window = ScoringConfig::default().good_window;
    let mut verdicts = Vec::new();
    for step in 0..24 {
        let chord_time = 0.5 + window - 0.012 * step as f64;
        let (first, second) = chord_hit(chord_time);
        assert_eq!(
            first, second,
            "chord at {chord_time} judged inconsistently: 60 hit = {first}, \
             64 hit = {second}"
        );
        verdicts.push(first);
    }
    // The sweep has to actually straddle the edge, or it agrees with itself
    // for the uninteresting reason that nothing was ever in range.
    assert!(
        verdicts.contains(&true) && verdicts.contains(&false),
        "the sweep never crossed the good-window edge: {verdicts:?}"
    );
}

#[test]
fn score_notes_does_not_hit_a_chord_note_from_only_one_of_its_pitches() {
    // Only 60 sounds — 64 never joins it. Neither half of the chord
    // should score just because its own pitch happens to be present.
    let mut world = chord_test_world(vec![pitch_info(60, "C", 4, midi_to_freq_hz(60.0))]);
    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    let notes = &world.resource::<SongNotes>().notes;
    assert!(!notes[0].hit, "60 alone must not satisfy the chord");
    assert!(!notes[1].hit);
}

#[test]
fn score_notes_misses_a_chord_note_that_never_sounded_together_with_its_partner() {
    let mut world = chord_test_world(vec![]);
    world.resource_mut::<GameplayClock>().set_free(10.0); // well past miss_window
    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);
    schedule.run(&mut world);

    let notes = &world.resource::<SongNotes>().notes;
    assert!(notes[0].missed);
    assert!(notes[1].missed);
}

// ── update_score_display (message-gated HUD writes) ─────────────────────

#[test]
fn update_score_display_only_writes_text_when_score_moved() {
    let mut world = World::new();
    world.insert_resource(Score { points: 100, combo: 3, max_combo: 3, last_hit_time: 0.0 });
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(Time::<()>::default());
    world.insert_resource(harmonicon_platform::localization::Localization::default());
    world.init_resource::<Messages<NoteScored>>();

    let score_entity = world.spawn((Text::new(""), ScoreText)).id();
    let combo_entity = world.spawn((Text::new(""), ComboText)).id();
    let feedback_entity = world
        .spawn((Text::new(""), TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)), FeedbackText))
        .id();

    let mut schedule = Schedule::default();
    schedule.add_systems(update_score_display);

    // No `NoteScored` this frame: the digits (still their spawn-time
    // default) must stay untouched.
    schedule.run(&mut world);
    assert_eq!(world.get::<Text>(score_entity).unwrap().0, "");
    assert_eq!(world.get::<Text>(combo_entity).unwrap().0, "");

    // `score_notes` would have set `HitFeedback` itself before emitting a
    // message with a quality — mirror that here rather than depending on
    // `update_score_display` to do it.
    world.insert_resource(HitFeedback {
        judgment: Some(JudgmentFeedback::Hit { quality: HitQuality::Perfect, offset: 0.0 }),
        timer: 0.75,
    });
    world.write_message(NoteScored {
        judgment: Some(JudgmentFeedback::Hit { quality: HitQuality::Perfect, offset: 0.0 }),
    });
    schedule.run(&mut world);

    assert_eq!(world.get::<Text>(score_entity).unwrap().0, "100");
    let expected_multiplier = compute_multiplier(3, 1.0, 0.1, 4.0);
    assert_eq!(world.get::<Text>(combo_entity).unwrap().0, combo_label(3, expected_multiplier));
    assert_eq!(world.get::<Text>(feedback_entity).unwrap().0, "gameplay-judgment-perfect");
    let color = world.get::<TextColor>(feedback_entity).unwrap();
    assert!(color.0.alpha() > 0.0, "the feedback flash should be visible right after a fresh hit");
}

#[test]
fn the_detail_line_explains_a_wrong_pitch_and_clears_on_the_next_judgment() {
    // The stale-detail failure this guards against: a wrong-pitch detail that
    // isn't overwritten on the next judgment sits under the *following*
    // note's verdict, captioning a note it has nothing to do with.
    let mut world = World::new();
    world.insert_resource(Score::default());
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(Time::<()>::default());
    world.insert_resource(harmonicon_platform::localization::Localization::default());
    world.init_resource::<Messages<NoteScored>>();
    let detail_entity = world
        .spawn((Text::new(""), TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)), FeedbackDetailText))
        .id();

    let mut schedule = Schedule::default();
    schedule.add_systems(update_score_display);

    let wrong_pitch = JudgmentFeedback::Miss(MissReason::WrongPitch {
        expected: HoleTab { hole: 4, is_blow: true },
        heard: Some(HoleTab { hole: 4, is_blow: false }),
    });
    world
        .insert_resource(HitFeedback { judgment: Some(wrong_pitch), timer: FAILURE_FEEDBACK_SECS });
    world.write_message(NoteScored { judgment: Some(wrong_pitch) });
    schedule.run(&mut world);
    assert!(
        !world.get::<Text>(detail_entity).unwrap().0.is_empty(),
        "a wrong-pitch miss should caption itself with the two tabs"
    );

    // Any other judgment has nothing to add, so the line must go back to
    // empty rather than keep the previous note's caption.
    let hit = JudgmentFeedback::Hit { quality: HitQuality::Perfect, offset: 0.0 };
    world.insert_resource(HitFeedback { judgment: Some(hit), timer: HIT_FEEDBACK_SECS });
    world.write_message(NoteScored { judgment: Some(hit) });
    schedule.run(&mut world);
    assert_eq!(world.get::<Text>(detail_entity).unwrap().0, "");
}

// ── notes_needing_spawn (windowed rendering) ─────────────────────────────
//
// `gameplay_2d`/`gameplay_3d` can't be smoke-tested headlessly (they need
// a real render/asset harness), so this pure windowing logic — the part
// that actually decides which notes get a visual and when — is the one
// piece of the windowed-spawn refactor that's directly testable. LOOKAHEAD
// is 3.0s throughout.

#[test]
fn notes_needing_spawn_is_empty_well_before_or_after_a_note() {
    let notes = [overlap_test_note(10.0)];
    let none = HashSet::new();
    assert_eq!(notes_needing_spawn(&notes, &none, 0.0).collect::<Vec<_>>(), Vec::<usize>::new());
    assert_eq!(notes_needing_spawn(&notes, &none, 20.0).collect::<Vec<_>>(), Vec::<usize>::new());
}

#[test]
fn notes_needing_spawn_includes_a_note_right_at_the_lookahead_edge() {
    let notes = [overlap_test_note(10.0)];
    let none = HashSet::new();
    // Window opens at note.time - LOOKAHEAD = 7.0.
    assert_eq!(notes_needing_spawn(&notes, &none, 7.0).collect::<Vec<_>>(), vec![0]);
    assert_eq!(notes_needing_spawn(&notes, &none, 6.999).collect::<Vec<_>>(), Vec::<usize>::new());
}

#[test]
fn notes_needing_spawn_skips_indices_already_spawned() {
    let notes = [overlap_test_note(10.0), overlap_test_note(10.5)];
    let one_spawned = HashSet::from([0]);
    assert_eq!(notes_needing_spawn(&notes, &one_spawned, 8.0).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn notes_needing_spawn_returns_every_note_whose_window_is_open() {
    let notes = [
        overlap_test_note(10.0),
        overlap_test_note(10.2),
        overlap_test_note(20.0), // window not open yet at elapsed=9.0
    ];
    let none = HashSet::new();
    assert_eq!(notes_needing_spawn(&notes, &none, 9.0).collect::<Vec<_>>(), vec![0, 1]);
}

#[test]
fn notes_needing_spawn_stops_scanning_once_a_note_is_too_far_out() {
    // A note far beyond the window sits after several already-open ones —
    // confirms the scan doesn't spuriously include (or choke on) it.
    let notes = [overlap_test_note(10.0), overlap_test_note(10.1), overlap_test_note(1000.0)];
    let none = HashSet::new();
    assert_eq!(notes_needing_spawn(&notes, &none, 9.0).collect::<Vec<_>>(), vec![0, 1]);
}

// ── loop_reset_range (A/B loop wrap note reset) ───────────────────────────

#[test]
fn loop_reset_range_covers_notes_from_start_through_end_time() {
    let notes = [
        overlap_test_note(4.0),  // before start_time — excluded
        overlap_test_note(5.0),  // == start_time — included
        overlap_test_note(8.0),  // inside the range — included
        overlap_test_note(10.0), // == end_time — included
    ];
    assert_eq!(loop_reset_range(&notes, 5.0, 10.0), (1, 4));
}

#[test]
fn loop_reset_range_extends_past_end_time_by_lookahead() {
    let notes = [
        overlap_test_note(10.0),                    // == end_time
        overlap_test_note(10.0 + LOOKAHEAD),        // exactly at the reach
        overlap_test_note(10.0 + LOOKAHEAD + 0.01), // just past — excluded
    ];
    assert_eq!(loop_reset_range(&notes, 5.0, 10.0), (0, 2));
}

// ── first_due_unresolved_note (wait-for-note freeze condition) ──────────

#[test]
fn first_due_unresolved_note_is_none_until_the_clock_reaches_it() {
    let notes = [overlap_test_note(10.0)];
    assert_eq!(first_due_unresolved_note(&notes, 0, 9.999), None);
    assert_eq!(first_due_unresolved_note(&notes, 0, 10.0), Some(0));
}

#[test]
fn first_due_unresolved_note_ignores_already_hit_or_missed_notes() {
    let mut hit = overlap_test_note(10.0);
    hit.hit = true;
    let mut missed = overlap_test_note(10.0);
    missed.missed = true;
    assert_eq!(first_due_unresolved_note(&[hit], 0, 10.0), None);
    assert_eq!(first_due_unresolved_note(&[missed], 0, 10.0), None);
}

#[test]
fn first_due_unresolved_note_ignores_unplayable_notes() {
    // A note the harp can't produce (`expected_pitch: None`) can never be
    // hit — freezing on one would wait forever.
    let mut unplayable = overlap_test_note(10.0);
    unplayable.expected_pitch = None;
    assert_eq!(first_due_unresolved_note(&[unplayable], 0, 10.0), None);
}

#[test]
fn first_due_unresolved_note_stops_scanning_once_a_note_is_not_due_yet() {
    // Sorted by time: an unresolved note far in the future shouldn't
    // match, and the earlier resolved note shouldn't either.
    let mut resolved = overlap_test_note(1.0);
    resolved.hit = true;
    let notes = [resolved, overlap_test_note(1000.0)];
    assert_eq!(first_due_unresolved_note(&notes, 0, 5.0), None);
}

#[test]
fn first_due_unresolved_note_returns_the_matching_index_after_the_cursor() {
    let mut resolved = overlap_test_note(1.0);
    resolved.hit = true;
    let notes = [resolved, overlap_test_note(2.0)];
    assert_eq!(first_due_unresolved_note(&notes, 0, 5.0), Some(1));
}

// ── wait_freeze_index (WaitForNoteMode + call-response force_wait) ──────

#[test]
fn wait_freeze_index_is_none_when_wait_mode_is_off_and_not_forced() {
    let notes = [overlap_test_note(1.0)];
    assert_eq!(wait_freeze_index(&notes, 0, 5.0, false), None);
}

#[test]
fn wait_freeze_index_freezes_when_wait_mode_is_on() {
    let notes = [overlap_test_note(1.0)];
    assert_eq!(wait_freeze_index(&notes, 0, 5.0, true), Some(0));
}

#[test]
fn wait_freeze_index_freezes_on_a_force_wait_note_even_with_wait_mode_off() {
    let mut note = overlap_test_note(1.0);
    note.force_wait = true;
    let notes = [note];
    assert_eq!(
        wait_freeze_index(&notes, 0, 5.0, false),
        Some(0),
        "a call-and-response note must freeze regardless of the player's practice toggle"
    );
}

#[test]
fn wait_freeze_index_ignores_a_force_wait_note_thats_not_due_yet() {
    let mut note = overlap_test_note(100.0);
    note.force_wait = true;
    let notes = [note];
    assert_eq!(wait_freeze_index(&notes, 0, 5.0, false), None);
}

/// A tiny synthetic 3-note "song" driven frame by frame through
/// `score_notes`, exercising the full detected-pitch → classify →
/// score/combo/stats path together rather than each piece in isolation.
/// This is the headless stand-in for
/// `docs/gameplay_validation.md`'s "HUD score/combo updates as you hit
/// notes" manual check.
#[test]
fn end_to_end_synthetic_song_drives_score_combo_and_stats() {
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(0.0));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(vec![]));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 62, 64]))); // C4, D4, E4
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.insert_resource(PlayedHarp(Some(harmonicon_core::harmonica::richter_harp("C"))));
    world.init_resource::<Messages<NoteScored>>();

    fn note(time: f64, pitch: u8) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.2,
            hole: 1,
            is_blow: true,
            expected_pitch: Some(pitch),
            hit: false,
            missed: false,
            held: 0.0,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: 0,
            chord_pitches: Vec::new(),
            playable: true,
            miss_evidence: None,
            force_wait: false,
        }
    }
    fn pitch(note: &str, octave: i32) -> PitchInfo {
        let midi = note_to_midi(&format!("{note}{octave}")).unwrap() as u8;
        PitchInfo { midi, note: note.to_string(), octave, frequency: midi_to_freq_hz(midi as f32) }
    }

    // C4 at t=0.0 is played right on time (Perfect); D4 at t=0.5 is played
    // 90ms late (inside `good_window` 130ms but past `perfect_window`
    // 60ms, so "Good"/delayed); E4 at t=1.0 is never played (Missed).
    // Already sorted by time, as `SongNotes` requires.
    world.insert_resource(SongNotes {
        notes: vec![note(0.0, 60), note(0.5, 62), note(1.0, 64)],
        cursor: 0,
    });
    let (perfect_idx, good_idx, missed_idx) = (0, 1, 2);

    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);

    // (clock time, active pitches this frame) — irregular steps are fine
    // since `score_notes` classifies purely from clock time, not frame
    // count; only the sustain-hold measurement cares about elapsed `dt`,
    // which `Time::advance_by` sets exactly per step below.
    let steps: &[(f64, &[(&str, i32)])] = &[
        (0.0, &[("C", 4)]),
        (0.05, &[("C", 4)]),
        (0.1, &[("C", 4)]),
        (0.15, &[("C", 4)]),
        (0.2, &[("C", 4)]),
        (0.21, &[]),
        (0.5, &[]),
        (0.59, &[("D", 4)]),
        (0.6, &[]),
        (1.0, &[]),
        (1.14, &[]),
        (1.3, &[]),
    ];
    let mut prev_t = 0.0f64;
    for &(t, pitches) in steps {
        world.resource_mut::<GameplayClock>().set_free(t);
        world.resource_mut::<ActivePitches>().0 =
            pitches.iter().map(|&(n, o)| pitch(n, o)).collect();
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f64(t - prev_t));
        schedule.run(&mut world);
        prev_t = t;
    }

    let song_notes = world.resource::<SongNotes>();
    assert!(song_notes.notes[perfect_idx].hit, "on-time note should be hit");
    assert!(song_notes.notes[good_idx].hit, "late-but-in-window note should still be hit");
    assert!(song_notes.notes[missed_idx].missed, "never-played note should be missed");

    let stats = world.resource::<SongStats>();
    assert_eq!(stats.perfect, 1);
    assert_eq!(stats.delayed, 1, "the D4 hit landed after its onset, inside the good window");
    assert_eq!(stats.miss, 1);

    let score = world.resource::<Score>();
    assert!(score.points > 0, "hits and sustain should award points");
    assert_eq!(
        score.max_combo, 2,
        "combo should have peaked at 2 (both hits) before the miss reset it"
    );
    assert_eq!(score.combo, 0, "the miss should have reset the live combo");

    // The same three decisions as they reach the HUD. Nothing else in the
    // run carries a judgment — sustain payouts and combo decay move `Score`
    // without saying anything at the hit line.
    let judgments: Vec<JudgmentFeedback> =
        world.resource_mut::<Messages<NoteScored>>().drain().filter_map(|m| m.judgment).collect();
    assert_eq!(judgments.len(), 3, "got {judgments:?}");
    assert!(
        matches!(judgments[0], JudgmentFeedback::Hit { quality: HitQuality::Perfect, .. }),
        "got {:?}",
        judgments[0]
    );
    match judgments[1] {
        JudgmentFeedback::Hit { quality: HitQuality::Good, offset } => {
            assert!(offset > 0.0, "the D4 hit landed late, so must read late")
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(judgments[2], JudgmentFeedback::Miss(MissReason::NoAttack));
}

// ── score_notes (judgment feedback) ──────────────────────────────────────

/// Drives `notes` through `score_notes` over a scripted `(clock time, MIDI
/// pitches sounding)` timeline and returns every judgment the run emitted, in
/// order — the exact values `hud::update_score_display` renders. Failure
/// attribution is only meaningful end-to-end like this: it accumulates across
/// the frames a note is pending, so a single-frame call can't observe it.
fn judgments_for(notes: Vec<ScheduledNote>, steps: &[(f64, &[u8])]) -> Vec<JudgmentFeedback> {
    judgments_on_harp(notes, steps, PlayedHarp(Some(harmonicon_core::harmonica::richter_harp("C"))))
}

/// [`judgments_for`] with the played harp chosen by the caller, for the cases
/// that turn on which harp (or no harp) is set up.
fn judgments_on_harp(
    notes: Vec<ScheduledNote>,
    steps: &[(f64, &[u8])],
    harp: PlayedHarp,
) -> Vec<JudgmentFeedback> {
    let mut world = World::new();
    world.insert_resource(GameplayClock::new(0.0));
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ActivePitches(vec![]));
    world.insert_resource(AudioFrame::default());
    world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 62, 64, 67])));
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(AudioSettings::default());
    world.insert_resource(Score::default());
    world.insert_resource(SongStats::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(PitchGate::default());
    world.insert_resource(harp);
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes { notes, cursor: 0 });

    let mut schedule = Schedule::default();
    schedule.add_systems(score_notes);

    let mut prev_t = 0.0f64;
    for &(t, sounding) in steps {
        world.resource_mut::<GameplayClock>().set_free(t);
        world.resource_mut::<ActivePitches>().0 =
            sounding.iter().map(|&m| pitch_info(m, "", 4, midi_to_freq_hz(m as f32))).collect();
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f64(t - prev_t));
        schedule.run(&mut world);
        prev_t = t;
    }

    world.resource_mut::<Messages<NoteScored>>().drain().filter_map(|m| m.judgment).collect()
}

/// One short note at `time` expecting `pitch`, with no modifiers — the
/// baseline the judgment tests vary.
fn judged_note(time: f64, pitch: u8) -> ScheduledNote {
    ScheduledNote { duration: 0.2, expected_pitch: Some(pitch), ..overlap_test_note(time) }
}

#[test]
fn score_notes_reports_a_good_hit_played_before_the_beat_as_early() {
    // 90 ms ahead: past `perfect_window` (60 ms), inside `good_window` (130).
    let judgments = judgments_for(vec![judged_note(0.5, 60)], &[(0.41, &[60]), (0.42, &[60])]);
    match judgments.first() {
        Some(&JudgmentFeedback::Hit { quality: HitQuality::Good, offset }) => {
            assert!(offset < 0.0, "played ahead of the beat, so offset is early")
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn score_notes_blames_a_wrong_pitch_the_player_actually_attacked() {
    // 62 is attacked where 60 was wanted, and has stopped sounding well
    // before the miss window closes at 0.63 — the case a classification done
    // only at the miss instant would misreport as "no attack".
    let judgments = judgments_for(
        vec![judged_note(0.5, 60)],
        &[(0.5, &[62]), (0.55, &[62]), (0.56, &[]), (0.7, &[])],
    );
    // `overlap_test_note` is hole 1 blow (= C4/60); 62 is D4, hole 1 draw on
    // the same harp — the classic "drew when you should have blown".
    assert_eq!(
        judgments,
        vec![JudgmentFeedback::Miss(MissReason::WrongPitch {
            expected: HoleTab { hole: 1, is_blow: true },
            heard: Some(HoleTab { hole: 1, is_blow: false }),
        })]
    );
}

#[test]
fn a_wrong_pitch_miss_names_the_attacked_pitch_nearest_the_target() {
    // Two wrong pitches are attacked at once (62 and 67). The report has to
    // name one of them, and `harp_pitches` is a `HashSet` — so it must pick by
    // a rule, not by iteration order, or the same frame reports differently on
    // different runs. The rule is "nearest the target", as the likeliest miss.
    let heard: Vec<Option<HoleTab>> = (0..16)
        .map(|_| {
            let judgments = judgments_for(
                vec![judged_note(0.5, 60)],
                &[(0.5, &[62, 67]), (0.56, &[]), (0.7, &[])],
            );
            match judgments.as_slice() {
                [JudgmentFeedback::Miss(MissReason::WrongPitch { heard, .. })] => *heard,
                other => panic!("got {other:?}"),
            }
        })
        .collect();
    // 62 (D4, hole 1 draw) is a whole tone from the target; 67 is a fifth.
    assert!(
        heard.iter().all(|&h| h == Some(HoleTab { hole: 1, is_blow: false })),
        "picked inconsistently across runs: {heard:?}"
    );
}

#[test]
fn a_wrong_pitch_miss_still_names_the_target_when_the_harp_is_unknown() {
    // With no harp set up there is no hole to name the heard pitch by. The
    // verdict and the expected tab still stand; the heard half says so by
    // being absent, rather than by naming some fallback hole the player would
    // then go looking for.
    let judgments = judgments_on_harp(
        vec![judged_note(0.5, 60)],
        &[(0.5, &[62]), (0.56, &[]), (0.7, &[])],
        PlayedHarp::default(),
    );
    assert_eq!(
        judgments,
        vec![JudgmentFeedback::Miss(MissReason::WrongPitch {
            expected: HoleTab { hole: 1, is_blow: true },
            heard: None,
        })]
    );
}

#[test]
fn tab_label_reads_as_hole_number_then_breath_arrow() {
    assert_eq!(tab_label(HoleTab { hole: 4, is_blow: true }), "4\u{2191}");
    assert_eq!(tab_label(HoleTab { hole: 10, is_blow: false }), "10\u{2193}");
}

#[test]
fn score_notes_blames_no_attack_when_nothing_sounded_at_all() {
    let judgments = judgments_for(vec![judged_note(0.5, 60)], &[(0.5, &[]), (0.7, &[])]);
    assert_eq!(judgments, vec![JudgmentFeedback::Miss(MissReason::NoAttack)]);
}

#[test]
fn score_notes_blames_no_attack_when_the_player_only_held_the_previous_note() {
    // 60 is held unbroken from the first note through the second's whole
    // window. The second note wanted 62, but the player never articulated
    // anything — "wrong note" would be the wrong coaching here.
    let judgments = judgments_for(
        vec![judged_note(0.0, 60), judged_note(0.5, 62)],
        &[(0.0, &[60]), (0.1, &[60]), (0.3, &[60]), (0.5, &[60]), (0.6, &[60]), (0.7, &[60])],
    );
    assert_eq!(
        judgments.last(),
        Some(&JudgmentFeedback::Miss(MissReason::NoAttack)),
        "got {judgments:?}"
    );
}

#[test]
fn score_notes_blames_an_incomplete_chord_when_only_part_of_it_sounded() {
    // Both halves of a [60, 64] chord miss, and both must say why: the
    // player did attack, and did attack one of the chord's own pitches —
    // just never both at once.
    let judgments =
        judgments_for(chord_test_notes(), &[(0.49, &[60]), (0.55, &[60]), (0.56, &[]), (0.7, &[])]);
    assert_eq!(
        judgments,
        vec![
            JudgmentFeedback::Miss(MissReason::IncompleteChord),
            JudgmentFeedback::Miss(MissReason::IncompleteChord),
        ]
    );
}

#[test]
fn score_notes_reports_an_unconfirmed_sustained_technique_when_the_sustain_ends() {
    // The onset lands (so the note is a hit and keeps its points), but the
    // held pitch never wobbles, so the declared vibrato can't be confirmed.
    let note = ScheduledNote {
        modifiers: vec![Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }],
        ..judged_note(0.0, 60)
    };
    let judgments = judgments_for(
        vec![note],
        &[(0.0, &[60]), (0.05, &[60]), (0.1, &[60]), (0.15, &[60]), (0.25, &[60])],
    );
    assert_eq!(judgments.last(), Some(&JudgmentFeedback::TechniqueMiss), "got {judgments:?}");
    assert!(
        matches!(judgments[0], JudgmentFeedback::Hit { .. }),
        "the onset still scored as a hit: got {:?}",
        judgments[0]
    );
}

// ── GameplayPlugin's schedule ────────────────────────────────────────────

/// Two whole classes of defect in this crate's schedule are invisible to the
/// compiler and only surface as a panic when the screen is first entered:
///
/// - **B0001**, a system whose own queries take one component mutably twice
///   without a `Without` proving them disjoint;
/// - **an ambiguous `SystemTypeSet`**, from ordering `.after(some_system)`
///   when that system is registered more than once in the schedule (this
///   crate registers `collect_pitches` twice — see `TrainerPitchSet`).
///
/// `Schedule::initialize` performs exactly those two checks: it resolves the
/// ordering graph and initializes every system's queries. Nothing needs to
/// *run*, so no resources, assets or window are required — which is what
/// makes covering the real plugin cheap enough to be worth doing, rather
/// than re-listing its systems in a mirror that would drift.
#[test]
fn every_schedule_the_gameplay_plugin_builds_initializes() {
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use harmonicon_app::app::AppState;

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
        // The note-tail material plugins take a `Handle<Shader>` in their
        // own `build`, and nothing here brings in `RenderPlugin` (which
        // wants a GPU) to register that asset type for them.
        .init_asset::<bevy::shader::Shader>()
        .init_state::<AppState>()
        .add_plugins(super::plugin::GameplayPlugin);
    app.finish();

    // Taken out of the world rather than borrowed through `resource_scope`:
    // initializing a schedule can itself insert `Schedules` (state
    // transitions add their own), which that scope rejects.
    let mut schedules =
        app.world_mut().remove_resource::<Schedules>().expect("the app has schedules");
    let world = app.world_mut();
    let mut built = 0;
    for (label, schedule) in schedules.iter_mut() {
        schedule
            .initialize(world)
            .unwrap_or_else(|err| panic!("schedule {label:?} failed to build: {err}"));
        built += 1;
    }
    assert!(built > 0, "no schedules to check");
}
