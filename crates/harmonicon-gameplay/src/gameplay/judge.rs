// SPDX-License-Identifier: MIT

//! The scoring *system*: [`score_notes`] and the pure helpers it depends on
//! (technique confirmation, style-bonus lookup, attack-cleanliness inputs).
//! The underlying pure scoring primitives (hit classification, points,
//! combo math) stay in top-level `harmonicon_core::scoring`, shared with the Song
//! Editor's practice mode; this module is gameplay's own driver of them.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::{AudioFrame, PitchInfo};
use harmonicon_core::chart::{Action, Modifier};
use harmonicon_core::midi::midi_to_freq_hz;
use harmonicon_core::pitch_map::map_pitch_playable;
use harmonicon_core::scoring::{
    HitQuality, NoteOutcome, VIBRATO_MIN_SWING_CENTS, WAH_MIN_SWING_FRAC, chord_is_sounding,
    classify_note, compute_points, is_clean_attack, measured_oscillation_hz,
    measured_relative_oscillation_hz, oscillation_matches_rate, should_decay_combo, sustain_points,
};

use super::clock::GameplayClock;
use super::notes::{ScheduledNote, SongNotes};
use super::state::{
    ActivePitches, ActiveTargets, FAILURE_FEEDBACK_SECS, HIT_FEEDBACK_SECS, HarmonicaPitchFilter,
    HitFeedback, HoleTab, JudgmentFeedback, MissReason, NoteScored, PitchGate, PlayedHarp, Score,
    ScoringConfig, SongStats, ValidHarpNotes, bump,
};

pub(crate) fn update_active_targets(
    clock: Res<GameplayClock>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    pitch_filter: Option<Res<HarmonicaPitchFilter>>,
    song_notes: Res<SongNotes>,
    mut targets: ResMut<ActiveTargets>,
) {
    targets.0.clear();
    if clock.get() < 0.0 {
        return;
    }
    // The judge's own instant, so the highlighted hole tracks what the
    // player is *actually* hearing, not what the raw clock says.
    let judged = judged_instant(clock.get(), &audio, pitch_filter.as_deref());
    // Starting from `score_notes`'s cursor (possibly a frame stale — that's
    // fine, it only ever lags a monotonically-advancing lower bound) means
    // this never re-scans notes long done. `notes` is sorted by `time`, so
    // once a not-yet-due note is too far out, everything after it is too.
    for note in &song_notes.notes[song_notes.cursor..] {
        if note.time > judged + config.good_window {
            break;
        }
        if note.hit || note.missed {
            continue;
        }
        if (judged - note.time).abs() <= config.good_window {
            targets.0.push((note.hole, note.is_blow));
        }
    }
}

/// Vibrato and wah are hand/throat articulations sustained *through* the
/// note, not a pitch shift validated by the onset alone (unlike a bend, whose
/// `expected_pitch` already encodes the bent target). Their style bonus is
/// deferred to the end of the sustain window and only paid out if
/// [`technique_confirmed`] finds the player actually wobbled the pitch/level.
pub(crate) fn is_sustained_technique(modifier: &Modifier) -> bool {
    matches!(modifier, Modifier::Vibrato { .. } | Modifier::WahWah { .. })
}

/// How far a measured vibrato/wah rate may drift from the chart's declared
/// `oscillation_hz` and still count — generous, since hand technique speed
/// varies naturally between players and even between notes.
pub(crate) const OSCILLATION_RATE_TOLERANCE_FRAC: f32 = 0.4;

/// Is the player performing this sustained technique, judged from the
/// pitch/loudness samples collected while the note is held — both that it
/// swings enough to be a real wobble, and that it swings at roughly the
/// chart's declared `oscillation_hz` rather than some unrelated rate.
/// `None` while there isn't yet enough of a swing to measure a rate from at
/// all — distinct from `Some(false)`, a rate that was measured and is wrong.
/// Non-sustained modifiers (bend, overblow, overdraw) are validated at onset
/// instead — this always answers `Some(true)` for them since it shouldn't
/// be asked.
pub(crate) fn technique_status(
    modifier: &Modifier,
    pitch_samples: &[(f64, f32)],
    amp_samples: &[(f64, f32)],
) -> Option<bool> {
    match modifier {
        Modifier::Vibrato { oscillation_hz, .. } => {
            measured_oscillation_hz(pitch_samples, VIBRATO_MIN_SWING_CENTS).map(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        Modifier::WahWah { oscillation_hz, .. } => {
            measured_relative_oscillation_hz(amp_samples, WAH_MIN_SWING_FRAC).map(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        _ => Some(true),
    }
}

/// The verdict at the end of a hold: [`technique_status`] with "never
/// measurable" counting as not performed — a declared vibrato that never
/// wobbled wasn't played.
pub(crate) fn technique_confirmed(
    modifier: &Modifier,
    pitch_samples: &[(f64, f32)],
    amp_samples: &[(f64, f32)],
) -> bool {
    technique_status(modifier, pitch_samples, amp_samples) == Some(true)
}

/// The live status of a note's sustained techniques mid-hold, for the
/// highway to show while the hold is still happening rather than only once
/// it ends: `None` when the note declares none, or none of them can be
/// measured yet; otherwise whether every measurable one is at the right
/// rate so far. Derived from the same samples `technique_confirmed` judges
/// at the end, so the live reading and the final verdict can't disagree.
pub fn live_technique_status(
    modifiers: &[Modifier],
    pitch_samples: &[(f64, f32)],
    amp_samples: &[(f64, f32)],
) -> Option<bool> {
    modifiers
        .iter()
        .filter(|m| is_sustained_technique(m))
        .filter_map(|m| technique_status(m, pitch_samples, amp_samples))
        .fold(None, |acc, ok| Some(acc.unwrap_or(true) && ok))
}

/// The instant on the chart's timeline that the sound arriving *now* was
/// made at: the clock with the player's input latency and the pitch
/// filter's onset lag taken off. The one definition of "when did this
/// attack happen" — the judge classifies against it, the dev autoplayer
/// sounds notes on it, and the highway measures a hold's elapsed time on it
/// (so `held`, which only starts once the judge sees the hit, isn't read
/// as a lost start).
pub fn judged_instant(
    clock: f64,
    audio: &AudioSettings,
    pitch_filter: Option<&HarmonicaPitchFilter>,
) -> f64 {
    clock
        - f64::from(audio.input_latency_ms) / 1000.0
        - pitch_filter.map_or(0.0, |filter| filter.onset_lag_secs(audio.pitch_algorithm))
}

/// The currently-detected frequency (Hz) matching `midi` (a MIDI note
/// number), or `None` if that exact pitch isn't among the detected pitches
/// this frame.
pub(crate) fn active_frequency_for(active: &[PitchInfo], midi: u8) -> Option<f32> {
    active.iter().find(|p| p.midi == midi).map(|p| p.frequency)
}

/// RMS loudness of a block of audio samples.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

pub(crate) fn modifier_fx_key(modifier: &Modifier) -> &'static str {
    match modifier {
        Modifier::Bend { .. } => "bend",
        Modifier::Vibrato { .. } => "vibrato",
        Modifier::WahWah { .. } => "wah-wah",
        Modifier::Overblow => "overblow",
        Modifier::Overdraw => "overdraw",
        Modifier::Slide => "slide",
    }
}

/// Style-bonus points awarded for a hit note's techniques, summed over its
/// modifiers using the chart's `style_bonus` table (keyed by technique name).
pub fn style_bonus_points(modifiers: &[Modifier], table: &HashMap<String, f32>) -> f32 {
    modifiers.iter().map(|m| table.get(modifier_fx_key(m)).copied().unwrap_or(0.0)).sum()
}

/// What one frame inside `note`'s window says about why it is failing, or
/// `None` if that frame blames nothing.
///
/// Called every frame the note is pending — not once when the miss window
/// elapses — because the offending pitch has usually stopped sounding by
/// then, and classifying from that last frame alone would report nearly every
/// miss as [`MissReason::NoAttack`]. It reads the same `sounding` set and
/// `gate` the hit test on that very same frame reads, so the explanation can
/// never disagree with the decision it explains.
///
/// Both answers require a *fresh* attack somewhere in the frame: a pitch the
/// player has simply been holding since an earlier note isn't a wrong note,
/// it's a missing one.
fn observed_failure(
    note: &ScheduledNote,
    sounding: &HashSet<u8>,
    gate: &PitchGate,
    harp: &PlayedHarp,
) -> Option<MissReason> {
    let mut attacked =
        sounding.iter().copied().filter(|&pitch| gate.is_fresh(pitch, true)).peekable();
    attacked.peek()?;
    if !note.chord_pitches.is_empty()
        && note.chord_pitches.iter().any(|pitch| sounding.contains(pitch))
        && !chord_is_sounding(&note.chord_pitches, sounding)
    {
        return Some(MissReason::IncompleteChord);
    }
    let expected = note.expected_pitch.filter(|e| !sounding.contains(e))?;
    Some(MissReason::WrongPitch {
        expected: HoleTab { hole: note.hole, is_blow: note.is_blow },
        heard: nearest_attacked(attacked, expected).and_then(|pitch| heard_tab(pitch, harp)),
    })
}

/// Which of several simultaneously-attacked pitches to name as "what I
/// heard": the one nearest the target, since that's the one the player was
/// most plausibly reaching for. Ties break low, and `attacked` arrives from a
/// `HashSet` — so this has to pick by a rule rather than take the first, or
/// the same frame could report different pitches on different runs.
pub(super) fn nearest_attacked(attacked: impl IntoIterator<Item = u8>, expected: u8) -> Option<u8> {
    attacked.into_iter().min_by_key(|&pitch| (pitch.abs_diff(expected), pitch))
}

/// The hole and breath that produce `pitch` on the harp the player is
/// holding. `None` when no harp is set up, or when it genuinely can't make
/// that pitch — possible after a harp substitution, where a detected pitch
/// passes `ValidHarpNotes` for the chart's harp but not the played one.
pub(super) fn heard_tab(pitch: u8, harp: &PlayedHarp) -> Option<HoleTab> {
    let assignment = map_pitch_playable(pitch, harp.0.as_ref()?)?;
    Some(HoleTab { hole: assignment.hole, is_blow: matches!(assignment.action, Action::Blow) })
}

/// Per-frame working storage for [`score_notes`], kept between frames so
/// judging allocates only when a frame needs more room than any before it.
#[derive(Default)]
pub struct JudgeScratch {
    harp_pitches: HashSet<u8>,
    pending: Vec<usize>,
}

/// Public for `benches/judge.rs`; the plugin is its only scheduler.
pub fn score_notes(
    mut scratch: Local<JudgeScratch>,
    clock: Res<GameplayClock>,
    time: Res<Time>,
    active: Res<ActivePitches>,
    frame: Res<AudioFrame>,
    valid_notes: Res<ValidHarpNotes>,
    played_harp: Res<PlayedHarp>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    pitch_filter: Option<Res<HarmonicaPitchFilter>>,
    mut song_notes: ResMut<SongNotes>,
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut gate: ResMut<PitchGate>,
    mut scored: MessageWriter<NoteScored>,
) {
    if clock.get() < 0.0 {
        return;
    }
    let dt = time.delta_secs_f64();
    // Compensate for microphone pipeline latency: a pitch detected at clock T
    // was actually played at T - latency. Shift the judgment window accordingly.
    //
    // Onset confirmation in `HarmonicaPitchFilter` is part of that same
    // pipeline and delays every pitch by the same fixed amount, so it belongs
    // in the same scalar rather than being applied per note. The scan below
    // stops early on the first note past the window and relies on `judged`
    // being one instant for the whole frame to do that: a per-note judgment
    // time makes `offset` non-monotonic over notes sorted by `time`, and the
    // scan can then break before a later note that was still in range.
    let judged = judged_instant(clock.get(), &audio, pitch_filter.as_deref());

    if config.combo_enabled
        && should_decay_combo(score.combo, clock.get(), score.last_hit_time, config.decay_secs)
    {
        score.combo = 0;
        scored.write(NoteScored { judgment: None });
    }

    let JudgeScratch { harp_pitches, pending } = &mut *scratch;
    harp_pitches.clear();
    harp_pitches.extend(active.0.iter().map(|p| p.midi).filter(|m| valid_notes.0.contains(m)));
    let harp_pitches = &*harp_pitches;

    // Re-arm any pitch the player has stopped sounding, so its next attack is
    // fresh. Pitches still held remain consumed and can't score again.
    gate.release_absent(|p| harp_pitches.contains(&p));

    // A prefix of `notes` (sorted by `time`) that's permanently resolved
    // (missed, or hit and fully sustained) never needs visiting again —
    // advance past it so a long chart's already-finished notes don't cost a
    // scan every frame. A later note occasionally resolving before an
    // earlier still-pending one (e.g. a chord) is fine: the cursor just
    // stays put until that earlier one finishes too.
    while song_notes.cursor < song_notes.notes.len() {
        let n = &song_notes.notes[song_notes.cursor];
        if n.missed || (n.hit && n.sustain_scored) {
            song_notes.cursor += 1;
        } else {
            break;
        }
    }

    // Not-yet-hit-or-missed notes are classified in a second pass below,
    // ordered by |offset| (closest to the judged instant first) rather than
    // array order, so when two same-pitch notes overlap the hit window,
    // whichever is actually due consumes the attack — not just whichever
    // happened to be classified first.
    pending.clear();
    let len = song_notes.notes.len();
    // One frame's loudness, shared by every note sustaining a wah.
    let mut frame_rms = None;

    for i in song_notes.cursor..len {
        let note = &mut song_notes.notes[i];
        if note.missed {
            continue;
        }

        // Already-hit notes are in their sustain phase: reward holding the pitch
        // through the note's length, then award the bonus once when it ends.
        if note.hit {
            if note.sustain_scored {
                continue;
            }
            if clock.get() < note.time + note.duration {
                // The held pitch stays "consumed" by the gate, so checking the
                // raw detected set keeps crediting this same note's sustain.
                if note.expected_pitch.is_some_and(|m| harp_pitches.contains(&m)) {
                    note.held += dt;
                }
                // Track pitch/loudness through the hold so a declared vibrato
                // or wah can be verified (rather than trusted) once it ends.
                if note.modifiers.iter().any(is_sustained_technique) {
                    if let Some(midi) = note.expected_pitch
                        && let Some(hz) = active_frequency_for(&active.0, midi)
                    {
                        let expected_hz = midi_to_freq_hz(midi as f32);
                        note.pitch_samples.push((clock.get(), 1200.0 * (hz / expected_hz).log2()));
                    }
                    let level = *frame_rms.get_or_insert_with(|| rms(&frame.samples));
                    note.amp_samples.push((clock.get(), level));
                }
            } else {
                score.points += sustain_points(note.held, note.duration);

                let sustained: Vec<Modifier> =
                    note.modifiers.iter().filter(|&x| is_sustained_technique(x)).cloned().collect();
                let mut technique_missed = false;
                if !sustained.is_empty() {
                    let (verified, unverified): (Vec<Modifier>, Vec<Modifier>) =
                        sustained.into_iter().partition(|m| {
                            technique_confirmed(m, &note.pitch_samples, &note.amp_samples)
                        });
                    if !verified.is_empty() {
                        score.points +=
                            style_bonus_points(&verified, &config.style_bonus).round() as u32;
                        stats.record_technique(&verified, true);
                    }
                    if !unverified.is_empty() {
                        technique_missed = true;
                        stats.record_technique(&unverified, false);
                    }
                }
                note.sustain_scored = true;
                let judgment =
                    if technique_missed { Some(JudgmentFeedback::TechniqueMiss) } else { None };
                if let Some(judgment) = judgment {
                    feedback.judgment = Some(judgment);
                    feedback.timer = FAILURE_FEEDBACK_SECS;
                }
                scored.write(NoteScored { judgment });
            }
            continue;
        }

        // Anything further out than `good_window` classifies as `TooEarly`
        // regardless of `playing` (see `classify_note`) — a guaranteed no-op
        // match arm below. `notes` is sorted by `time`, so once one note is
        // this far out, every note after it is too — stop scanning outright
        // instead of just skipping the push, so a long chart's untouched
        // future notes cost nothing per frame, not even a visit.
        let offset = judged - note.time;
        if offset < -config.good_window {
            break;
        }
        pending.push(i);
    }

    pending.sort_by(|&a, &b| {
        let offset_a = (judged - song_notes.notes[a].time).abs();
        let offset_b = (judged - song_notes.notes[b].time).abs();
        offset_a.partial_cmp(&offset_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    for &i in pending.iter() {
        let note = &mut song_notes.notes[i];
        // A note the player's harp cannot produce is not part of the
        // performance: never hit, never missed, absent from the stats. It
        // stays visible so the chart still reads correctly — see
        // `ScheduledNote::playable`.
        if !note.playable {
            continue;
        }
        let offset = judged - note.time;
        // A note counts as "playing" only on a fresh attack: the pitch must be
        // sounding and not already consumed by an earlier note in this sustain.
        // A note with no valid `expected_pitch` (the harp can't produce it) can
        // never be "playing". A chord/octave-split note (non-empty
        // `chord_pitches`) additionally requires every sibling pitch of its
        // `TrackItem` to be sounding at the same instant — its own freshness
        // alone isn't enough, or a chord could be "hit" one note at a time.
        let playing = note.expected_pitch.is_some_and(|m| {
            gate.is_fresh(m, harp_pitches.contains(&m))
                && (note.chord_pitches.is_empty()
                    || chord_is_sounding(&note.chord_pitches, harp_pitches))
        });

        // Keep the first frame's explanation rather than the last one's: the
        // player's mistake is the note they actually attacked, not whatever
        // happens to still be ringing when the window finally closes.
        if note.miss_evidence.is_none() {
            note.miss_evidence = observed_failure(note, harp_pitches, &gate, &played_harp);
        }

        match classify_note(
            offset,
            playing,
            config.perfect_window,
            config.good_window,
            config.miss_window,
        ) {
            NoteOutcome::Missed => {
                let reason = note.miss_evidence.unwrap_or(MissReason::NoAttack);
                note.missed = true;
                stats.miss += 1;
                stats.record_technique(&note.modifiers, false);
                if config.combo_enabled {
                    score.combo = 0;
                }
                let judgment = JudgmentFeedback::Miss(reason);
                feedback.judgment = Some(judgment);
                feedback.timer = FAILURE_FEEDBACK_SECS;
                scored.write(NoteScored { judgment: Some(judgment) });
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap | NoteOutcome::Waiting => {}
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                // Vibrato/wah are judged from the sustain, not the onset — see
                // the sustain branch above. A note with only those modifiers
                // has nothing to credit yet, so it's left out of `stats` here
                // rather than falling through to the "normal" bucket.
                let immediate: Vec<Modifier> = note
                    .modifiers
                    .iter()
                    .filter(|&m| !is_sustained_technique(m))
                    .cloned()
                    .collect();
                if note.modifiers.is_empty() || !immediate.is_empty() {
                    stats.record_technique(&immediate, true);
                }
                // Claim the attack so a held breath can't also clear the next
                // same-pitch note; the player must re-articulate for that one.
                // `playing` was only true above if `expected_pitch` is `Some`.
                if let Some(m) = note.expected_pitch {
                    gate.consume(m);
                    // `is_clean_attack` means "nothing else sounded" —
                    // meaningless for a chord note, where other pitches
                    // sounding is the whole point (see the doc comment on
                    // `SongStats::clean_attack`).
                    if note.chord_pitches.is_empty() {
                        bump(&mut stats.clean_attack, is_clean_attack(harp_pitches, m));
                    }
                }
                match quality {
                    HitQuality::Perfect => stats.perfect += 1,
                    // A late Good hit counts as "delayed"; early/on-time as "good".
                    HitQuality::Good if offset > 0.0 => stats.delayed += 1,
                    HitQuality::Good => stats.good += 1,
                }
                stats.offset_sum += offset;
                stats.timing.record(offset);
                score.last_hit_time = clock.get();
                score.combo += 1;
                score.max_combo = score.max_combo.max(score.combo);
                let multiplier = config.multiplier(score.combo);
                score.points += compute_points(quality, multiplier);
                // Reward executing the note's onset techniques. Bends are
                // genuinely validated (the note's expected pitch is the bent
                // one); the bonus is the payoff for nailing them. Vibrato/wah
                // bonuses are awarded later, once the sustain confirms them.
                score.points += style_bonus_points(&immediate, &config.style_bonus).round() as u32;
                let judgment = JudgmentFeedback::Hit { quality, offset };
                feedback.judgment = Some(judgment);
                feedback.timer = HIT_FEEDBACK_SECS;
                scored.write(NoteScored { judgment: Some(judgment) });
            }
        }
    }
}
