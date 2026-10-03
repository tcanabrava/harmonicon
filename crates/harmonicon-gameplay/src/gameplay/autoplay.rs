// SPDX-License-Identifier: MIT

//! Dev-only autoplay: sounds every note of the chart itself, so the whole
//! judged-note path — head pops, the tail's hold fill, live vibrato/wah
//! confirmation, the results screen's timing bar — can be seen and captured
//! over BRP without a microphone, where the screenshot tour otherwise only
//! ever produces misses. It writes the same `ActivePitches`/`AudioFrame` the
//! detector would, *after* `collect_pitches`, so the judge is exercised
//! unchanged; nothing downstream knows the pitches were synthesised.
//!
//! `#[cfg(feature = "dev")]` — never in a shipped build.

use bevy::prelude::*;

use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::{AudioFrame, PitchInfo};
use harmonicon_core::chart::Modifier;
use harmonicon_core::midi::{midi_to_freq_hz, midi_to_note};

use super::clock::GameplayClock;
use super::judge::judged_instant;
use super::notes::{ScheduledNote, SongNotes};
use super::state::{ActivePitches, HarmonicaPitchFilter};

/// Switched on over BRP (`scripts/brpctl.py autoplay on [late_ms]`).
/// `late_ms` sounds every attack that much after the beat, so a run can be
/// made deliberately lopsided to reach the results screen's Input-lag
/// suggestion; zero plays dead on.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct Autoplay {
    pub enabled: bool,
    pub late_ms: f32,
}

/// A sounding note stops this long before its end, so a legato successor
/// on the same pitch sees a gap and reads as a fresh attack to `PitchGate`.
const RELEASE_GAP_SECS: f64 = 0.06;
/// Vibrato swing, in cents — well over `VIBRATO_MIN_SWING_CENTS`.
const VIBRATO_CENTS: f32 = 30.0;
/// Steady loudness while a note sounds, and the wah swing around it —
/// well over `WAH_MIN_SWING_FRAC` of the baseline.
const BASE_AMPLITUDE: f32 = 0.2;
const WAH_SWING: f32 = 0.06;
/// Far enough ahead of the judged instant that nothing past it could be due.
const LOOKAHEAD_SECS: f64 = 0.5;

/// What the autoplayer sounds this frame: `(midi, cents offset)` per pitch,
/// and the loudness. `judged` is the clock as the judge sees it (input
/// latency already removed); `clock` is the raw clock the sustain samples
/// are stamped with. Pure, so the attack/sustain/release rules are testable.
pub fn sounding(
    notes: &[ScheduledNote],
    cursor: usize,
    judged: f64,
    clock: f64,
    late_secs: f64,
) -> (Vec<(u8, f32)>, f32) {
    let mut pitches = Vec::new();
    let mut amplitude = 0.0;
    for note in &notes[cursor.min(notes.len())..] {
        if note.time > judged + LOOKAHEAD_SECS {
            break;
        }
        let Some(midi) = note.expected_pitch.filter(|_| note.playable) else {
            continue;
        };
        let sustaining = note.hit
            && !note.sustain_scored
            && clock < note.time + note.duration - RELEASE_GAP_SECS;
        let attacking = !note.hit && !note.missed && judged >= note.time + late_secs;
        if !(sustaining || attacking) {
            continue;
        }
        let mut cents = 0.0;
        let mut amp = BASE_AMPLITUDE;
        if sustaining {
            for m in &note.modifiers {
                match m {
                    Modifier::Vibrato { oscillation_hz, .. } => {
                        cents = VIBRATO_CENTS
                            * (std::f32::consts::TAU * oscillation_hz * clock as f32).sin();
                    }
                    Modifier::WahWah { oscillation_hz, .. } => {
                        amp += WAH_SWING
                            * (std::f32::consts::TAU * oscillation_hz * clock as f32).sin();
                    }
                    _ => {}
                }
            }
        }
        pitches.push((midi, cents));
        amplitude = amp;
    }
    (pitches, amplitude)
}

/// Runs right after `collect_pitches` in the `GameplayLogic` chain and
/// overwrites what it produced, so a silent (or absent) microphone and an
/// active autoplay give the judge exactly one source of pitches.
pub(super) fn autoplay_pitches(
    autoplay: Res<Autoplay>,
    clock: Res<GameplayClock>,
    audio: Res<AudioSettings>,
    pitch_filter: Res<HarmonicaPitchFilter>,
    song_notes: Res<SongNotes>,
    mut active: ResMut<ActivePitches>,
    mut frame: ResMut<AudioFrame>,
) {
    if !autoplay.enabled {
        return;
    }
    // The judge's own instant, or every attack reads early by the lag it
    // compensates for.
    let judged = judged_instant(clock.get(), &audio, Some(&pitch_filter));
    let (pitches, amplitude) = sounding(
        &song_notes.notes,
        song_notes.cursor,
        judged,
        clock.get(),
        f64::from(autoplay.late_ms) / 1000.0,
    );
    active.0.clear();
    active.0.extend(pitches.into_iter().map(|(midi, cents)| PitchInfo {
        midi,
        note: midi_to_note(i32::from(midi)),
        octave: i32::from(midi) / 12 - 1,
        frequency: midi_to_freq_hz(f32::from(midi) + cents / 100.0),
    }));
    // The judge reads loudness as the RMS of this block, so a flat block at
    // `amplitude` *is* that loudness.
    frame.samples.resize(256, amplitude);
    frame.samples.fill(amplitude);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time: f64, duration: f64, midi: u8) -> ScheduledNote {
        ScheduledNote { time, duration, hole: 4, expected_pitch: Some(midi), ..Default::default() }
    }

    #[test]
    fn silence_before_a_note_is_due() {
        let notes = [note(2.0, 1.0, 60)];
        let (pitches, amp) = sounding(&notes, 0, 1.9, 1.9, 0.0);
        assert!(pitches.is_empty());
        assert_eq!(amp, 0.0);
    }

    #[test]
    fn a_due_note_is_attacked_and_a_late_setting_delays_it() {
        let notes = [note(2.0, 1.0, 60)];
        assert_eq!(sounding(&notes, 0, 2.0, 2.0, 0.0).0, vec![(60, 0.0)]);
        assert!(sounding(&notes, 0, 2.0, 2.0, 0.05).0.is_empty());
        assert_eq!(sounding(&notes, 0, 2.05, 2.05, 0.05).0, vec![(60, 0.0)]);
    }

    #[test]
    fn a_hit_note_sustains_then_releases_before_its_end() {
        let mut n = note(2.0, 1.0, 60);
        n.hit = true;
        let notes = [n];
        assert_eq!(sounding(&notes, 0, 2.5, 2.5, 0.0).0, vec![(60, 0.0)]);
        // Inside the release gap: silent, so a same-pitch successor is fresh.
        assert!(sounding(&notes, 0, 2.97, 2.97, 0.0).0.is_empty());
    }

    #[test]
    fn a_resolved_or_unplayable_note_is_never_sounded() {
        let mut missed = note(2.0, 1.0, 60);
        missed.missed = true;
        let mut scored = note(2.0, 1.0, 62);
        scored.hit = true;
        scored.sustain_scored = true;
        let mut unplayable = note(2.0, 1.0, 64);
        unplayable.playable = false;
        let notes = [missed, scored, unplayable];
        assert!(sounding(&notes, 0, 2.5, 2.5, 0.0).0.is_empty());
    }

    #[test]
    fn vibrato_wobbles_the_pitch_and_wah_pumps_the_loudness() {
        let mut vib = note(0.0, 4.0, 60);
        vib.hit = true;
        vib.modifiers = vec![Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }];
        let mut wah = note(0.0, 4.0, 62);
        wah.hit = true;
        wah.modifiers = vec![Modifier::WahWah { oscillation_hz: 3.0, intensity: None }];
        let notes = [vib, wah];
        // A quarter of a 5 Hz cycle in: the vibrato is at its peak swing.
        let (pitches, amp) = sounding(&notes, 0, 0.05, 0.05, 0.0);
        assert!((pitches[0].1 - VIBRATO_CENTS).abs() < 1e-3);
        assert_eq!(pitches[1].1, 0.0, "wah doesn't move the pitch");
        assert!(amp != BASE_AMPLITUDE, "…but it does move the loudness");
    }

    #[test]
    fn the_cursor_and_lookahead_bound_the_scan() {
        let notes = [note(0.0, 1.0, 60), note(1.0, 1.0, 62), note(9.0, 1.0, 64)];
        // Cursor past the first note: it's never revisited even if due.
        let (pitches, _) = sounding(&notes, 1, 1.0, 1.0, 0.0);
        assert_eq!(pitches, vec![(62, 0.0)]);
    }
}
