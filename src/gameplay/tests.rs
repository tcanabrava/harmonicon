// SPDX-License-Identifier: MIT

use super::*;

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
    stats.record_technique(
        &[Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        }],
        true,
    );
    stats.record_technique(&[Modifier::Overblow], false);
    stats.record_technique(
        &[Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        }],
        true,
    );
    stats.record_technique(
        &[Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        }],
        true,
    );
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
            Modifier::Bend {
                semitones: -1.0,
                intensity: None,
            },
            Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: None,
            },
        ],
        true,
    );
    assert_eq!(stats.bend.hits, 1);
    assert_eq!(stats.vibrato.hits, 1);
}

#[test]
fn parse_beats_4_4() {
    assert_eq!(parse_beats(Some("4/4")), 4.0);
}

#[test]
fn parse_beats_3_4() {
    assert_eq!(parse_beats(Some("3/4")), 3.0);
}

#[test]
fn parse_beats_none_defaults_to_4() {
    assert_eq!(parse_beats(None), 4.0);
}

#[test]
fn parse_beats_malformed_defaults_to_4() {
    assert_eq!(parse_beats(Some("invalid")), 4.0);
}

#[test]
fn secs_per_bar_120bpm_4beats() {
    assert!((secs_per_bar(120.0, 4.0) - 2.0).abs() < 1e-9);
}

#[test]
fn secs_per_bar_60bpm_4beats() {
    assert!((secs_per_bar(60.0, 4.0) - 4.0).abs() < 1e-9);
}

// `advance_clock`'s own tests live in `clock.rs` alongside the type.

// ── handle_loop_boundary ─────────────────────────────────────────────────

/// No `AudioSink`/`MusicPlayer` entity is spawned in these tests — the
/// seek-on-wrap fix (see the doc comment on `handle_loop_boundary`)
/// degrades gracefully to a no-op when no sink exists (as it does for a
/// real chart before the music sink spawns, or if audio init failed), so
/// the clock/note-reset behaviour is testable headlessly without a real
/// audio backend; the actual seek call needs a live sink and is a manual
/// check (see `docs/gameplay_validation.md`).
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
        pitch_samples: Vec::new(),
        amp_samples: Vec::new(),
        phrase_section: 0,
        chord_pitches: Vec::new(),
        force_wait: false,
    }
}

#[test]
fn loop_boundary_rewinds_the_clock_and_resets_notes_in_range() {
    let mut world = World::new();
    world.insert_resource(LoopConfig {
        active: true,
        start_time: 2.0,
        end_time: 10.0,
    });
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
    world.insert_resource(LoopConfig {
        active: true,
        start_time: 2.0,
        end_time: 10.0,
    });
    world.insert_resource(GameplayClock::new(9.999));
    world.insert_resource(SongNotes::default());
    let mut schedule = Schedule::default();
    schedule.add_systems(handle_loop_boundary);
    schedule.run(&mut world);
    assert_eq!(world.resource::<GameplayClock>().get(), 9.999);

    let mut world = World::new();
    world.insert_resource(LoopConfig {
        active: false,
        start_time: 2.0,
        end_time: 10.0,
    });
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
    assert!(should_anchor_to_sink(
        1.0,
        true,
        &GameplayMode::Play2D,
        false
    ));
}

#[test]
fn does_not_anchor_during_the_countdown() {
    assert!(!should_anchor_to_sink(
        -1.0,
        true,
        &GameplayMode::Play2D,
        false
    ));
}

#[test]
fn does_not_anchor_before_music_started() {
    assert!(!should_anchor_to_sink(
        1.0,
        false,
        &GameplayMode::Play2D,
        false
    ));
}

#[test]
fn does_not_anchor_in_jam_session() {
    assert!(!should_anchor_to_sink(
        1.0,
        true,
        &GameplayMode::JamSession,
        false
    ));
}

#[test]
fn does_not_anchor_once_the_sink_is_empty() {
    // A finished sink's reported position freezes rather than continuing
    // to advance — anchoring to it would repeatedly snap the clock back
    // once real time drifts past it.
    assert!(!should_anchor_to_sink(
        1.0,
        true,
        &GameplayMode::Play2D,
        true
    ));
}

// ── loop_range_valid (progress-bar drag loop range) ──────────────────────

#[test]
fn loop_range_valid_requires_end_strictly_after_start() {
    assert!(loop_range_valid(4.0, 8.0));
    assert!(!loop_range_valid(8.0, 8.0));
    assert!(!loop_range_valid(8.0, 4.0));
}

// ── resolve_item_time ───────────────────────────────────────────────────────

use crate::song::chart::{TempoPoint, Timing, TrackItem};

fn track_item(time: Option<f64>, tick: Option<u64>) -> TrackItem {
    TrackItem {
        id: None,
        time,
        tick,
        duration: 0.5,
        phrase: None,
        groove: None,
        play_mode: None,
        call: false,
        events: vec![],
    }
}

fn timing_120bpm() -> Timing {
    Timing {
        resolution: 480,
        tempo_map: vec![TempoPoint {
            tick: 0,
            bpm: 120.0,
        }],
        time_signature_map: None,
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
    use crate::song::chart::Modifier::*;
    assert_eq!(
        modifier_fx_key(&Bend {
            semitones: -1.0,
            intensity: None
        }),
        "bend"
    );
    assert_eq!(
        modifier_fx_key(&Vibrato {
            oscillation_hz: 5.0,
            intensity: None
        }),
        "vibrato"
    );
    assert_eq!(
        modifier_fx_key(&WahWah {
            oscillation_hz: 3.0,
            intensity: None
        }),
        "wah-wah"
    );
    assert_eq!(modifier_fx_key(&Overblow), "overblow");
    assert_eq!(modifier_fx_key(&Overdraw), "overdraw");
    assert_eq!(modifier_fx_key(&Slide), "slide");
}

// `PitchGate` is now a thin `Resource` wrapper around the shared
// `AttackGate` (see `crate::scoring`) — its re-attack-detection behaviour
// is covered by `AttackGate`'s own tests there, not duplicated here.

// ── target_pitch (bend validation) ───────────────────────────────────────────

#[test]
fn bend_targets_the_bent_pitch() {
    let bend = vec![Modifier::Bend {
        semitones: -1.0,
        intensity: None,
    }];
    // A 1-semitone draw bend on B4 (71) must be played as A#4 (70), not
    // the natural B4.
    assert_eq!(target_pitch("B4", &bend), Some(70));
}

#[test]
fn deeper_bend_targets_lower_pitch() {
    let bend = vec![Modifier::Bend {
        semitones: -2.0,
        intensity: None,
    }];
    assert_eq!(target_pitch("B4", &bend), Some(69)); // A4
}

#[test]
fn non_bend_techniques_keep_the_natural_pitch() {
    let vib = vec![Modifier::Vibrato {
        oscillation_hz: 5.0,
        intensity: None,
    }];
    assert_eq!(target_pitch("D5", &vib), Some(74));
    assert_eq!(target_pitch("D5", &[]), Some(74));
}

#[test]
fn unknown_pitch_name_has_no_target() {
    // The "—" placeholder for a hole/direction the harp can't produce
    // isn't a parseable note name, so there's no valid target at all —
    // this note can never be hit.
    let bend = vec![Modifier::Bend {
        semitones: -1.0,
        intensity: None,
    }];
    assert_eq!(target_pitch("\u{2014}", &bend), None);
}

// ── style_bonus_points ───────────────────────────────────────────────────────

fn bonus_table() -> HashMap<String, f32> {
    [("bend".to_string(), 50.0), ("vibrato".to_string(), 25.0)]
        .into_iter()
        .collect()
}

#[test]
fn style_bonus_sums_matched_techniques() {
    let mods = vec![
        Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        },
        Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        },
    ];
    assert_eq!(style_bonus_points(&mods, &bonus_table()), 75.0);
}

#[test]
fn style_bonus_ignores_techniques_absent_from_the_table() {
    let mods = vec![Modifier::WahWah {
        oscillation_hz: 3.0,
        intensity: None,
    }];
    assert_eq!(style_bonus_points(&mods, &bonus_table()), 0.0);
}

#[test]
fn style_bonus_is_zero_without_modifiers() {
    assert_eq!(style_bonus_points(&[], &bonus_table()), 0.0);
}

// ── sustained-technique validation (vibrato / wah) ──────────────────────────

#[test]
fn vibrato_and_wah_are_sustained_bend_and_overblow_are_not() {
    let vibrato = Modifier::Vibrato {
        oscillation_hz: 5.0,
        intensity: None,
    };
    let wah = Modifier::WahWah {
        oscillation_hz: 3.0,
        intensity: None,
    };
    let bend = Modifier::Bend {
        semitones: -1.0,
        intensity: None,
    };
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
            let v =
                offset + amplitude * (2.0 * std::f32::consts::PI * freq_hz * t as f32).sin();
            (t, v)
        })
        .collect()
}

#[test]
fn technique_confirmed_requires_real_wobble_for_vibrato() {
    let vibrato = Modifier::Vibrato {
        oscillation_hz: 5.0,
        intensity: None,
    };
    let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.0)).collect();
    let wobbling = timestamped_sine(5.0, 0.0, 25.0, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&vibrato, &steady, &[]));
    assert!(technique_confirmed(&vibrato, &wobbling, &[]));
}

#[test]
fn technique_confirmed_requires_real_wobble_for_wah() {
    let wah = Modifier::WahWah {
        oscillation_hz: 3.0,
        intensity: None,
    };
    let steady_volume: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.2)).collect();
    let pumping_volume = timestamped_sine(3.0, 0.2, 0.06, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&wah, &[], &steady_volume));
    assert!(technique_confirmed(&wah, &[], &pumping_volume));
}

#[test]
fn technique_confirmed_rejects_vibrato_at_the_wrong_rate() {
    // The chart declares a 5 Hz vibrato, but the player wobbled at ~1.5 Hz
    // — real oscillation, just not the declared rate. A flip-count-only
    // check couldn't tell these apart.
    let vibrato = Modifier::Vibrato {
        oscillation_hz: 5.0,
        intensity: None,
    };
    let slow_wobble = timestamped_sine(1.5, 0.0, 25.0, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&vibrato, &slow_wobble, &[]));
}

#[test]
fn technique_confirmed_rejects_wah_at_the_wrong_rate() {
    let wah = Modifier::WahWah {
        oscillation_hz: 3.0,
        intensity: None,
    };
    let fast_pumping = timestamped_sine(9.0, 0.2, 0.06, 40, 1.0 / 60.0);
    assert!(!technique_confirmed(&wah, &[], &fast_pumping));
}

#[test]
fn technique_confirmed_is_always_true_for_onset_validated_modifiers() {
    // Bend/overblow/overdraw/slide are judged at onset, not from the
    // sustain buffers — this should never gate them on empty/steady samples.
    assert!(technique_confirmed(
        &Modifier::Bend {
            semitones: -1.0,
            intensity: None
        },
        &[],
        &[]
    ));
    assert!(technique_confirmed(&Modifier::Overblow, &[], &[]));
    assert!(technique_confirmed(&Modifier::Slide, &[], &[]));
}

fn pitch_info(midi: u8, note: &str, octave: i32, frequency: f32) -> PitchInfo {
    PitchInfo {
        midi,
        note: note.into(),
        octave,
        frequency,
    }
}

#[test]
fn active_frequency_for_matches_by_midi_number() {
    let active = vec![
        pitch_info(62, "D", 4, 293.66),
        pitch_info(67, "G", 4, 392.00),
    ];
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
    let scene_a = world.spawn(GameplayRoot).id();
    let scene_b = world.spawn((GameplayRoot, Transform::default())).id();
    let keep = world.spawn_empty().id();

    let mut schedule = Schedule::default();
    schedule.add_systems(cleanup_gameplay);
    schedule.run(&mut world);

    assert!(
        !world.entities().contains(scene_a),
        "GameplayRoot should be despawned"
    );
    assert!(
        !world.entities().contains(scene_b),
        "GameplayRoot should be despawned"
    );
    assert!(
        world.entities().contains(keep),
        "unrelated entities must survive"
    );
}

// ── score_notes (same-pitch overlap ordering) ───────────────────────────

fn overlap_test_note(time: f64) -> ScheduledNote {
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
    assert!(
        song_notes.notes[1].hit,
        "the note actually due should be credited"
    );
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
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes {
        notes: vec![overlap_test_note(120.0)],
        cursor: 0,
    });

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
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes {
        notes: vec![overlap_test_note(0.49)],
        cursor: 0,
    });
    world
}

#[test]
fn score_notes_counts_a_solo_pitch_as_a_clean_attack() {
    let mut world =
        clean_attack_test_world(vec![pitch_info(60, "C", 4, midi_to_freq_hz(60.0))]);
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
        ScheduledNote {
            hole: 2,
            expected_pitch: Some(64),
            chord_pitches: vec![60, 64],
            ..base
        },
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
    world.init_resource::<Messages<NoteScored>>();
    world.insert_resource(SongNotes {
        notes: chord_test_notes(),
        cursor: 0,
    });
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
    world.insert_resource(Score {
        points: 100,
        combo: 3,
        max_combo: 3,
        last_hit_time: 0.0,
    });
    world.insert_resource(ScoringConfig::default());
    world.insert_resource(HitFeedback::default());
    world.insert_resource(Time::<()>::default());
    world.init_resource::<Messages<NoteScored>>();

    let score_entity = world.spawn((Text::new(""), ScoreText)).id();
    let combo_entity = world.spawn((Text::new(""), ComboText)).id();
    let feedback_entity = world
        .spawn((
            Text::new(""),
            TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            FeedbackText,
        ))
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
        quality: Some(HitQuality::Perfect),
        timer: 0.75,
    });
    world.write_message(NoteScored {
        quality: Some(HitQuality::Perfect),
    });
    schedule.run(&mut world);

    assert_eq!(world.get::<Text>(score_entity).unwrap().0, "100");
    let expected_multiplier = compute_multiplier(3, 1.0, 0.1, 4.0);
    assert_eq!(
        world.get::<Text>(combo_entity).unwrap().0,
        combo_label(3, expected_multiplier)
    );
    assert_eq!(world.get::<Text>(feedback_entity).unwrap().0, "PERFECT!");
    let color = world.get::<TextColor>(feedback_entity).unwrap();
    assert!(color.0.alpha() > 0.0, "the feedback flash should be visible right after a fresh hit");
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
    assert_eq!(notes_needing_spawn(&notes, &none, 0.0), Vec::<usize>::new());
    assert_eq!(
        notes_needing_spawn(&notes, &none, 20.0),
        Vec::<usize>::new()
    );
}

#[test]
fn notes_needing_spawn_includes_a_note_right_at_the_lookahead_edge() {
    let notes = [overlap_test_note(10.0)];
    let none = HashSet::new();
    // Window opens at note.time - LOOKAHEAD = 7.0.
    assert_eq!(notes_needing_spawn(&notes, &none, 7.0), vec![0]);
    assert_eq!(
        notes_needing_spawn(&notes, &none, 6.999),
        Vec::<usize>::new()
    );
}

#[test]
fn notes_needing_spawn_skips_indices_already_spawned() {
    let notes = [overlap_test_note(10.0), overlap_test_note(10.5)];
    let one_spawned = HashSet::from([0]);
    assert_eq!(notes_needing_spawn(&notes, &one_spawned, 8.0), vec![1]);
}

#[test]
fn notes_needing_spawn_returns_every_note_whose_window_is_open() {
    let notes = [
        overlap_test_note(10.0),
        overlap_test_note(10.2),
        overlap_test_note(20.0), // window not open yet at elapsed=9.0
    ];
    let none = HashSet::new();
    assert_eq!(notes_needing_spawn(&notes, &none, 9.0), vec![0, 1]);
}

#[test]
fn notes_needing_spawn_stops_scanning_once_a_note_is_too_far_out() {
    // A note far beyond the window sits after several already-open ones —
    // confirms the scan doesn't spuriously include (or choke on) it.
    let notes = [
        overlap_test_note(10.0),
        overlap_test_note(10.1),
        overlap_test_note(1000.0),
    ];
    let none = HashSet::new();
    assert_eq!(notes_needing_spawn(&notes, &none, 9.0), vec![0, 1]);
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
            force_wait: false,
        }
    }
    fn pitch(note: &str, octave: i32) -> PitchInfo {
        let midi = note_to_midi(&format!("{note}{octave}")).unwrap() as u8;
        PitchInfo {
            midi,
            note: note.to_string(),
            octave,
            frequency: midi_to_freq_hz(midi as f32),
        }
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
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f64(t - prev_t));
        schedule.run(&mut world);
        prev_t = t;
    }

    let song_notes = world.resource::<SongNotes>();
    assert!(
        song_notes.notes[perfect_idx].hit,
        "on-time note should be hit"
    );
    assert!(
        song_notes.notes[good_idx].hit,
        "late-but-in-window note should still be hit"
    );
    assert!(
        song_notes.notes[missed_idx].missed,
        "never-played note should be missed"
    );

    let stats = world.resource::<SongStats>();
    assert_eq!(stats.perfect, 1);
    assert_eq!(
        stats.delayed, 1,
        "the D4 hit landed after its onset, inside the good window"
    );
    assert_eq!(stats.miss, 1);

    let score = world.resource::<Score>();
    assert!(score.points > 0, "hits and sustain should award points");
    assert_eq!(
        score.max_combo, 2,
        "combo should have peaked at 2 (both hits) before the miss reset it"
    );
    assert_eq!(score.combo, 0, "the miss should have reset the live combo");
}
