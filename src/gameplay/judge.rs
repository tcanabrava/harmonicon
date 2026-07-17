// SPDX-License-Identifier: MIT

//! The scoring *system*: [`score_notes`] and the pure helpers it depends on
//! (technique confirmation, style-bonus lookup, attack-cleanliness inputs).
//! The underlying pure scoring primitives (hit classification, points,
//! combo math) stay in top-level `crate::scoring`, shared with the Song
//! Editor's practice mode; this module is gameplay's own driver of them.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::audio_system::midi::midi_to_freq_hz;
use crate::audio_system::pitch_detect::{AudioFrame, PitchInfo};
use crate::scoring::{
    HitQuality, NoteOutcome, VIBRATO_MIN_SWING_CENTS, WAH_MIN_SWING_FRAC, chord_is_sounding,
    classify_note, compute_multiplier, compute_points, is_clean_attack, measured_oscillation_hz,
    measured_relative_oscillation_hz, oscillation_matches_rate, should_decay_combo, sustain_points,
};
use crate::settings::AudioSettings;
use crate::song::chart::Modifier;

use super::clock::GameplayClock;
use super::notes::SongNotes;
use super::state::{
    ActivePitches, ActiveTargets, HitFeedback, NoteScored, PitchGate, Score, ScoringConfig,
    SongStats, ValidHarpNotes, bump,
};

pub(crate) fn update_active_targets(
    clock: Res<GameplayClock>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    song_notes: Res<SongNotes>,
    mut targets: ResMut<ActiveTargets>,
) {
    targets.0.clear();
    if clock.get() < 0.0 {
        return;
    }
    // Shift the judgment point back by the microphone pipeline latency so the
    // highlighted hole tracks what the player is *actually* hearing, not what
    // the raw clock says.
    let judged = clock.get() - audio.input_latency_ms as f64 / 1000.0;
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
const OSCILLATION_RATE_TOLERANCE_FRAC: f32 = 0.4;

/// Did the player actually perform this sustained technique, judged from the
/// pitch/loudness samples collected while the note was held — both that it
/// swung enough to be a real wobble, and that it swung at roughly the
/// chart's declared `oscillation_hz` rather than some unrelated rate.
/// Non-sustained modifiers (bend, overblow, overdraw) are validated at onset
/// instead — this always returns `true` for them since it shouldn't be asked.
pub(crate) fn technique_confirmed(
    modifier: &Modifier,
    pitch_samples: &[(f64, f32)],
    amp_samples: &[(f64, f32)],
) -> bool {
    match modifier {
        Modifier::Vibrato { oscillation_hz, .. } => {
            measured_oscillation_hz(pitch_samples, VIBRATO_MIN_SWING_CENTS).is_some_and(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        Modifier::WahWah { oscillation_hz, .. } => {
            measured_relative_oscillation_hz(amp_samples, WAH_MIN_SWING_FRAC).is_some_and(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        _ => true,
    }
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
    modifiers
        .iter()
        .map(|m| table.get(modifier_fx_key(m)).copied().unwrap_or(0.0))
        .sum()
}

pub(crate) fn score_notes(
    clock: Res<GameplayClock>,
    time: Res<Time>,
    active: Res<ActivePitches>,
    frame: Res<AudioFrame>,
    valid_notes: Res<ValidHarpNotes>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
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
    let judged = clock.get() - audio.input_latency_ms as f64 / 1000.0;

    if config.combo_enabled
        && should_decay_combo(
            score.combo,
            clock.get(),
            score.last_hit_time,
            config.decay_secs,
        )
    {
        score.combo = 0;
        scored.write(NoteScored { quality: None });
    }

    let harp_pitches: HashSet<u8> = active
        .0
        .iter()
        .map(|p| p.midi)
        .filter(|m| valid_notes.0.contains(m))
        .collect();

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
    let mut pending: Vec<usize> = Vec::new();
    let len = song_notes.notes.len();

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
                if note
                    .expected_pitch
                    .is_some_and(|m| harp_pitches.contains(&m))
                {
                    note.held += dt;
                }
                // Track pitch/loudness through the hold so a declared vibrato
                // or wah can be verified (rather than trusted) once it ends.
                if note.modifiers.iter().any(is_sustained_technique) {
                    if let Some(midi) = note.expected_pitch
                        && let Some(hz) = active_frequency_for(&active.0, midi)
                    {
                        let expected_hz = midi_to_freq_hz(midi as f32);
                        note.pitch_samples
                            .push((clock.get(), 1200.0 * (hz / expected_hz).log2()));
                    }
                    note.amp_samples.push((clock.get(), rms(&frame.samples)));
                }
            } else {
                score.points += sustain_points(note.held, note.duration);

                let sustained: Vec<Modifier> = note
                    .modifiers
                    .iter()
                    .filter(|&x| is_sustained_technique(x))
                    .cloned()
                    .collect();
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
                        stats.record_technique(&unverified, false);
                    }
                }
                note.sustain_scored = true;
                scored.write(NoteScored { quality: None });
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
        offset_a
            .partial_cmp(&offset_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for i in pending {
        let note = &mut song_notes.notes[i];
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
                    || chord_is_sounding(&note.chord_pitches, &harp_pitches))
        });

        match classify_note(
            offset,
            playing,
            config.perfect_window,
            config.good_window,
            config.miss_window,
        ) {
            NoteOutcome::Missed => {
                note.missed = true;
                stats.miss += 1;
                stats.record_technique(&note.modifiers, false);
                if config.combo_enabled {
                    score.combo = 0;
                }
                scored.write(NoteScored { quality: None });
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
                        bump(&mut stats.clean_attack, is_clean_attack(&harp_pitches, m));
                    }
                }
                match quality {
                    HitQuality::Perfect => stats.perfect += 1,
                    // A late Good hit counts as "delayed"; early/on-time as "good".
                    HitQuality::Good if offset > 0.0 => stats.delayed += 1,
                    HitQuality::Good => stats.good += 1,
                }
                stats.offset_sum += offset;
                score.last_hit_time = clock.get();
                score.combo += 1;
                score.max_combo = score.max_combo.max(score.combo);
                let multiplier = if config.combo_enabled {
                    compute_multiplier(
                        score.combo,
                        config.base_multiplier,
                        config.step_multiplier,
                        config.max_multiplier,
                    )
                } else {
                    1.0
                };
                score.points += compute_points(quality, multiplier);
                // Reward executing the note's onset techniques. Bends are
                // genuinely validated (the note's expected pitch is the bent
                // one); the bonus is the payoff for nailing them. Vibrato/wah
                // bonuses are awarded later, once the sustain confirms them.
                score.points += style_bonus_points(&immediate, &config.style_bonus).round() as u32;
                feedback.quality = Some(quality);
                feedback.timer = 0.75;
                scored.write(NoteScored {
                    quality: Some(quality),
                });
            }
        }
    }
}
