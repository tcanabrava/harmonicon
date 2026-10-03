// SPDX-License-Identifier: MIT

//! Generated Jam Session backing: a synthesized rhythm section of bass,
//! drums and chordal comping, for any key, tempo and progression, so Jam
//! Session doesn't require picking an existing song. Each [`Genre`] has its
//! own `GrooveArrangement` (per-role rhythmic slots, fills at bars 4, 8 and
//! 11, a bar-12 turnaround), shaped over a four-chorus arc by [`BandEnergy`]
//! and humanized deterministically from a stored seed. The three roles
//! render as sample-aligned stems, so each can be muted live without
//! drifting from the others. Design and open listening work:
//! `docs/jam_feel_plan.md`.
//!
//! Deliberately not the harmonica-timbre synth in `harmonicon_core::synth`
//! that `gameplay::call_response` uses. A backing band is a different set of
//! instruments, and reusing harmonica partials here would risk sounding like
//! a second harmonica part to echo instead of backing to play over.

use std::f32::consts::TAU;
use std::path::PathBuf;

use bevy::audio::AudioSource;
use bevy::prelude::*;

use harmonicon_audio::waveform::{WAVEFORM_BUCKETS, bucket_peaks};
use harmonicon_core::chart::{
    Action, Difficulty, Feel, HarpChart, Metadata, NoteEvent, Scoring, Song, TempoPoint, Timing,
    TrackItem,
};
use harmonicon_core::harmonica::{
    Position, Progression, chord_intervals, progression_bars, richter_harp, semitone,
};
use harmonicon_core::midi::{midi_to_freq_hz, note_to_midi};
use harmonicon_core::wav::encode_wav;
use harmonicon_song::song::{BackingStemAudio, NoteThemeConfig, SongManifest};

mod humanize;
mod preview;
use humanize::{GrooveRole, performance_variation, varied_slot};
pub use preview::generate_listening_preview;

pub const SAMPLE_RATE: u32 = 44_100;

/// How many 12-bar choruses to render into one generated backing buffer.
/// Four matches the musical arc planned for generated jams while keeping
/// three sample-aligned stems reasonably small. Playback queues another
/// buffer before this one ends, so session length is still unlimited.
pub const CHORUSES: u32 = 4;

const ATTACK_SECS: f32 = 0.01;
const RELEASE_SECS: f32 = 0.05;
/// Fraction of each note's own slot left as silence before the next bass
/// note, so consecutive notes don't blur into one continuous tone.
const NOTE_GAP_FRAC: f32 = 0.08;

/// Which rhythmic/groove character a generated jam's bass line uses —
/// selectable on the "Generate Jam" config page alongside `Progression`/
/// `Position`. Deliberately its own axis rather than a `Progression`
/// variant: `Progression` only changes which chord *roots* play over the
/// 12-bar form, but the thing that actually makes a genre sound like that
/// genre is almost entirely rhythm/groove, not chord choice — see
/// [`groove_arrangement`].
///
/// A player can freely combine any `Genre` with any `Progression` (e.g.
/// `Rock` rhythm over the `JazzBlues` changes) — the two are independent
/// choices, not a fixed pairing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Genre {
    #[default]
    Blues,
    Jazz,
    Rock,
    Reggae,
    Country,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BandEnergy {
    Low,
    #[default]
    Medium,
    High,
}

impl BandEnergy {
    pub fn all() -> &'static [BandEnergy] {
        &[BandEnergy::Low, BandEnergy::Medium, BandEnergy::High]
    }

    pub fn label(self) -> &'static str {
        match self {
            BandEnergy::Low => "Low",
            BandEnergy::Medium => "Medium",
            BandEnergy::High => "High",
        }
    }
}

impl Genre {
    /// Every selectable genre, in the order the "Generate Jam" combobox
    /// offers them.
    pub fn all() -> &'static [Genre] {
        &[Genre::Blues, Genre::Jazz, Genre::Rock, Genre::Reggae, Genre::Country]
    }

    /// Display label for the picker.
    pub fn label(self) -> &'static str {
        match self {
            Genre::Blues => "Blues",
            Genre::Jazz => "Jazz",
            Genre::Rock => "Rock",
            Genre::Reggae => "Reggae",
            Genre::Country => "Country",
        }
    }

    /// Inverse of [`label`](Self::label) — same `all().find(...)` pattern
    /// as `Progression::from_label`/`Position::from_label`/`Scale::from_label`.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().iter().copied().find(|g| g.label() == label)
    }

    /// The straight/shuffle metronome feel this genre implies, seeded into
    /// a generated chart's `Song::feel` (see `generated_chart`) — picked up
    /// for free by `gameplay::metronome_overlay::feel_from_chart`, the same
    /// mechanism a hand-authored chart's own `feel` field already drives.
    fn metronome_feel(self) -> Feel {
        match self {
            Genre::Blues | Genre::Jazz => Feel::Shuffle,
            Genre::Rock | Genre::Reggae | Genre::Country => Feel::Straight,
        }
    }
}

/// The genre picked for the current generated jam, kept around after
/// `build_generated_manifest` bakes it into the audio/chart so a live
/// gameplay-side system (`jam::rhythm_guide`) can still read it — unlike
/// `Genre` itself, which is otherwise only ever a plain function parameter.
/// Lives here (not `app::`, alongside `JamProgression`/`JamScale`) because
/// `Genre` lives in this same `jam::` layer already; `app::` is reserved
/// for wrapping types from *lower* layers (`song::`) that a higher one
/// like `jam::` reads — putting `Genre` there would mean `app::` importing
/// from `jam::`, inverting the "dependencies point downward" rule `jam`
/// itself already relies on (`jam::session` imports `harmonicon_app::app::
/// JamProgression`/`JamScale`, not the other way around).
///
/// Set on Start by `menu::pages::jam_generate` (alongside `JamProgression`/
/// `JamScale`) and reset to the default (`Genre::Blues`) by the real-song
/// "Jam Session" button (`menu::pages::jam_session`) — a real song has no
/// genre concept attached to it; `jam::rhythm_guide`'s widget only ever
/// spawns for a `GeneratedJamSession` regardless, so this reset is just
/// defense against a stale value lingering, not load-bearing on its own.
#[derive(Resource, Default)]
pub struct JamGenre(pub Genre);

/// Semitone offsets of the classic 12-bar "blues box" bass shape, relative
/// to whatever chord root is sounding: root, root, 5th, 5th, flat-7th,
/// flat-7th, 5th, flat-7th — 8 slots per bar. Quality-agnostic like every
/// pattern below: root/5th/flat-7th are shared between a dominant-7th and
/// minor-7th chord (`song::harmonica::chord_intervals`) — only the 3rd
/// differs, and none of these patterns ever play one.
const BLUES_PATTERN: [Option<i32>; 8] =
    [Some(0), Some(0), Some(7), Some(7), Some(10), Some(10), Some(7), Some(10)];
/// Walking-quarter-note contour: root, 5th, flat-7th, 5th, one note per
/// beat (the off-beat slots rest) — swung like the classic blues shape,
/// just sparser.
const JAZZ_PATTERN: [Option<i32>; 8] =
    [Some(0), None, Some(7), None, Some(10), None, Some(7), None];
/// Straight, driving root pulse with a 5th lift in the second half of the
/// bar — a simplified "power chord" bass line, no swing.
const ROCK_PATTERN: [Option<i32>; 8] =
    [Some(0), Some(0), Some(0), Some(0), Some(7), Some(7), Some(0), Some(0)];
/// The classic reggae "skank": silence on every downbeat, a hit on every
/// off-beat. This is a deliberate simplification, not a transcription of
/// real reggae form (which usually isn't a 12-bar blues at all) — the
/// genre character here comes entirely from this off-beat rhythm sitting
/// on top of the same 12-bar practice-loop scaffold every other genre
/// uses, not from an authentic reggae chord progression.
const REGGAE_PATTERN: [Option<i32>; 8] =
    [None, Some(0), None, Some(7), None, Some(10), None, Some(7)];
/// Straight "boom-chick": root on the beat, 5th on the off-beat, no swing.
const COUNTRY_PATTERN: [Option<i32>; 8] =
    [Some(0), None, Some(7), None, Some(0), None, Some(7), None];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DrumEvent {
    pub kick: bool,
    pub snare: bool,
    pub hat: bool,
    pub accent: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompingEvent {
    pub hit: bool,
    pub accent: f32,
}

/// One bar of shared musical intent for every generated backing role. Audio
/// renderers and the live pulse guide consume this same value, so genre feel
/// cannot diverge between stems or between sound and screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GrooveArrangement {
    pub swung: bool,
    pub bass: [Option<i32>; 8],
    pub drums: [DrumEvent; 8],
    pub comping: [CompingEvent; 8],
}

const fn drum(kick: bool, snare: bool, hat: bool, accent: f32) -> DrumEvent {
    DrumEvent { kick, snare, hat, accent }
}

const fn comp(hit: bool, accent: f32) -> CompingEvent {
    CompingEvent { hit, accent }
}

pub(crate) fn groove_arrangement(genre: Genre) -> GrooveArrangement {
    let (swung, bass, drums, comping) = match genre {
        Genre::Blues => (
            true,
            BLUES_PATTERN,
            [
                drum(true, false, true, 1.0),
                drum(false, false, true, 0.55),
                drum(false, true, true, 0.9),
                drum(false, false, true, 0.55),
                drum(true, false, true, 0.85),
                drum(false, false, true, 0.55),
                drum(false, true, true, 0.95),
                drum(false, false, true, 0.55),
            ],
            [
                comp(false, 0.0),
                comp(true, 0.75),
                comp(false, 0.0),
                comp(true, 0.65),
                comp(false, 0.0),
                comp(true, 0.75),
                comp(false, 0.0),
                comp(true, 0.65),
            ],
        ),
        Genre::Jazz => (
            true,
            JAZZ_PATTERN,
            [
                drum(true, false, true, 0.65),
                drum(false, false, true, 0.55),
                drum(false, true, true, 0.45),
                drum(false, false, true, 0.7),
                drum(false, false, true, 0.55),
                drum(true, false, true, 0.45),
                drum(false, true, true, 0.5),
                drum(false, false, true, 0.75),
            ],
            [
                comp(false, 0.0),
                comp(true, 0.55),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(true, 0.5),
                comp(false, 0.0),
                comp(false, 0.0),
            ],
        ),
        Genre::Rock => (
            false,
            ROCK_PATTERN,
            [
                drum(true, false, true, 1.0),
                drum(false, false, true, 0.6),
                drum(false, true, true, 1.0),
                drum(false, false, true, 0.6),
                drum(true, false, true, 0.9),
                drum(false, false, true, 0.6),
                drum(false, true, true, 1.0),
                drum(false, false, true, 0.6),
            ],
            [
                comp(true, 0.9),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(true, 0.8),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(false, 0.0),
            ],
        ),
        Genre::Reggae => (
            false,
            REGGAE_PATTERN,
            [
                drum(false, false, false, 0.0),
                drum(false, false, true, 0.5),
                drum(false, true, false, 0.7),
                drum(false, false, true, 0.5),
                drum(true, false, false, 0.9),
                drum(false, false, true, 0.5),
                drum(false, true, false, 0.7),
                drum(false, false, true, 0.5),
            ],
            [
                comp(false, 0.0),
                comp(true, 0.8),
                comp(false, 0.0),
                comp(true, 0.75),
                comp(false, 0.0),
                comp(true, 0.8),
                comp(false, 0.0),
                comp(true, 0.75),
            ],
        ),
        Genre::Country => (
            false,
            COUNTRY_PATTERN,
            [
                drum(true, false, true, 0.9),
                drum(false, false, true, 0.45),
                drum(false, true, true, 0.75),
                drum(false, false, true, 0.45),
                drum(true, false, true, 0.85),
                drum(false, false, true, 0.45),
                drum(false, true, true, 0.75),
                drum(false, false, true, 0.45),
            ],
            [
                comp(false, 0.0),
                comp(false, 0.0),
                comp(true, 0.7),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(false, 0.0),
                comp(true, 0.65),
                comp(false, 0.0),
            ],
        ),
    };
    GrooveArrangement { swung, bass, drums, comping }
}

/// Applies restrained phrase and turnaround variations to the base groove.
/// `bar` is zero-based within the 12-bar form. Variations occupy only the
/// tail of bars 4, 8, 11, and 12 so the next downbeat stays open and clear.
pub(crate) fn groove_arrangement_for_position(
    genre: Genre,
    chorus: usize,
    bar: usize,
) -> GrooveArrangement {
    groove_arrangement_for_energy(genre, BandEnergy::Medium, chorus, bar)
}

fn groove_arrangement_for_energy(
    genre: Genre,
    energy: BandEnergy,
    chorus: usize,
    bar: usize,
) -> GrooveArrangement {
    let mut arrangement = groove_arrangement(genre);
    match chorus % CHORUSES as usize {
        0 => {
            let mut heard = 0;
            for event in &mut arrangement.comping {
                if event.hit {
                    event.hit = heard % 2 == 0;
                    heard += 1;
                }
            }
            for event in &mut arrangement.drums {
                event.accent *= 0.78;
            }
        }
        1 => {
            for event in &mut arrangement.comping {
                event.accent = (event.accent * 1.08).min(1.0);
            }
        }
        2 => {
            for event in &mut arrangement.drums {
                event.accent = (event.accent * 1.08).min(1.0);
            }
            for event in &mut arrangement.comping {
                event.accent = (event.accent * 1.12).min(1.0);
            }
        }
        _ => {
            for event in &mut arrangement.drums {
                event.accent *= 0.88;
            }
            for event in &mut arrangement.comping {
                event.accent *= 0.82;
            }
        }
    }
    match energy {
        BandEnergy::Low => {
            let mut heard = 0;
            for event in &mut arrangement.comping {
                if event.hit {
                    event.hit = heard % 2 == 0;
                    heard += 1;
                }
            }
            for (slot, event) in arrangement.drums.iter_mut().enumerate() {
                event.accent *= 0.72;
                if slot % 2 == 1 {
                    event.hat = false;
                }
            }
        }
        BandEnergy::Medium => {}
        BandEnergy::High => {
            for event in &mut arrangement.drums {
                event.accent = (event.accent * 1.12).min(1.0);
            }
            for event in &mut arrangement.comping {
                event.accent = (event.accent * 1.12).min(1.0);
            }
            arrangement.comping[7].hit = true;
            arrangement.comping[7].accent = arrangement.comping[7].accent.max(0.55);
        }
    }
    match bar % 12 {
        3 | 7 => {
            arrangement.drums[7].snare = true;
            arrangement.drums[7].accent = arrangement.drums[7].accent.max(0.65);
        }
        10 => {
            arrangement.bass[7] = Some(11);
            arrangement.drums[6].snare = true;
            arrangement.drums[6].accent = arrangement.drums[6].accent.max(0.7);
        }
        11 => {
            arrangement.bass[5] = Some(9);
            arrangement.bass[6] = Some(10);
            arrangement.bass[7] = Some(11);
            for slot in 5..8 {
                arrangement.drums[slot].snare = true;
                arrangement.drums[slot].accent = 0.55 + (slot - 5) as f32 * 0.15;
                arrangement.comping[slot].hit = false;
            }
        }
        _ => {}
    }
    arrangement
}

/// The long eighth of a swung pair takes this fraction of the beat (the
/// short one takes the rest) — the same 2:1 "triplet swing" ratio
/// `metronome_overlay`'s `MetronomeFeel::Shuffle` clicks to, so a genre
/// that swings (see [`groove_arrangement`]) swings in step with the shuffle-feel
/// metronome. A straight genre splits the beat evenly instead. `pub(crate)`
/// so `jam::rhythm_guide::active_slot` can use the identical split for its
/// live pulse timing instead of a second, possibly-drifting copy.
pub(crate) const SWING_LONG_FRAC: f32 = 2.0 / 3.0;

/// One simple bass tone: a sine fundamental plus a second and third harmonic
/// for warmth, and a short attack/release envelope. The harmonics matter for
/// more than tone color: octave 2's fundamentals (see [`bar_beat_freqs`])
/// sit around 65–110 Hz, below what small/laptop speakers can reproduce, so
/// the *speaker-audible* part of this tone is disproportionately the
/// 2nd/3rd harmonics (130–330 Hz) — the classic "psychoacoustic bass"
/// problem, not a playback bug.
fn bass_tone(freq_hz: f32, duration_secs: f32) -> Vec<f32> {
    let n = (duration_secs * SAMPLE_RATE as f32).max(1.0) as usize;
    let attack = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;
    let release = (SAMPLE_RATE as f32 * RELEASE_SECS) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let atk = if attack > 0 && i < attack { i as f32 / attack as f32 } else { 1.0 };
            let rel =
                if n > release && i > n - release { (n - i) as f32 / release as f32 } else { 1.0 };
            let env = atk.min(rel).clamp(0.0, 1.0);
            let s = (TAU * freq_hz * t).sin()
                + 0.4 * (TAU * freq_hz * 2.0 * t).sin()
                + 0.22 * (TAU * freq_hz * 3.0 * t).sin();
            // Leave room for drums and comping when Bevy mixes the three
            // independently mutable stems.
            env * s * 0.28
        })
        .collect()
}

/// A two-beat tonic hit for all three generated-band roles. Keeping the
/// ending as separate stems preserves the live mixer state: a role muted
/// during the jam stays muted for the final chord as well.
pub fn generate_ending_stems(
    key: &str,
    quality: harmonicon_core::harmonica::ChordQuality,
    bpm: f32,
    genre: Genre,
) -> [Vec<f32>; 3] {
    let duration = 2.0 * 60.0 / bpm.max(1.0);
    let samples = (duration * SAMPLE_RATE as f32).max(1.0) as usize;
    let Some(midi) = note_to_midi(&format!("{key}3")) else {
        return std::array::from_fn(|_| Vec::new());
    };
    let mut bass = bass_tone(midi_to_freq_hz(midi as f32), duration * 0.9);
    bass.resize(samples, 0.0);

    let mut drums = drum_slot(drum(true, genre != Genre::Jazz, true, 0.9), 0, duration, genre);
    drums.resize(samples, 0.0);

    let mut comping = comping_slot(key, quality, duration, genre);
    comping.resize(samples, 0.0);
    [bass, drums, comping]
}

/// The 8 note frequencies (Hz) of one bar of `pattern` (see the `*_PATTERN`
/// constants and [`groove_arrangement`]), rooted on `root`, in the bass register
/// (octave 3 — one octave higher than a real bass guitar, deliberately: a
/// single sine-ish voice with no amp/cabinet coloring, and octave 2's
/// ~65–110 Hz fundamentals are below what small/laptop speakers reproduce,
/// see [`bass_tone`]). `None` for a rest slot, or a note whose resolved
/// name doesn't parse — the latter shouldn't happen for the roots
/// `progression_bars` produces.
fn bar_beat_freqs(root: &str, pattern: &[Option<i32>; 8]) -> [Option<f32>; 8] {
    pattern.map(|slot| {
        let semitones = slot?;
        let note_class = semitone(root, semitones);
        note_to_midi(&format!("{note_class}3")).map(|m| midi_to_freq_hz(m as f32))
    })
}

/// Renders [`CHORUSES`] repeats of a `progression`'s 12-bar bass line in
/// `key` at `bpm` (4/4 throughout), shaped by `genre`'s rhythm pattern and
/// straight/swing feel (see [`groove_arrangement`]). Pure and deterministic —
/// the whole backing loop is fully described by
/// `key`/`bpm`/`progression`/`genre`. The public helper uses seed zero;
/// generated sessions call the seeded renderer below.
pub fn generate_bass_pcm(key: &str, bpm: f32, progression: Progression, genre: Genre) -> Vec<f32> {
    generate_bass_pcm_seeded(key, bpm, progression, genre, 0)
}

fn generate_bass_pcm_seeded(
    key: &str,
    bpm: f32,
    progression: Progression,
    genre: Genre,
    seed: u64,
) -> Vec<f32> {
    let secs_per_beat = 60.0 / bpm.max(1.0);
    let swung = groove_arrangement(genre).swung;
    // Each bar's 8 notes are 4 pairs, one pair per beat. A swung genre's
    // long note takes `SWING_LONG_FRAC` of the beat, the short note the
    // rest; a straight genre splits the beat evenly instead — either way
    // long+short always sums to exactly one beat, so a bar's total length
    // is unaffected by genre (still 4 beats), only how it's subdivided.
    let long_secs = if swung { secs_per_beat * SWING_LONG_FRAC } else { secs_per_beat * 0.5 };
    let short_secs = secs_per_beat - long_secs;
    let roots = progression_bars(key, progression).map(|(root, _)| root);
    let mut buf = Vec::new();
    for chorus in 0..CHORUSES as usize {
        for (bar, root) in roots.iter().enumerate() {
            let arrangement = groove_arrangement_for_position(genre, chorus, bar);
            for (slot, freq) in bar_beat_freqs(root, &arrangement.bass).into_iter().enumerate() {
                let note_secs = if slot % 2 == 0 { long_secs } else { short_secs };
                let sound = freq
                    .map(|hz| bass_tone(hz, note_secs * (1.0 - NOTE_GAP_FRAC)))
                    .unwrap_or_default();
                buf.extend(varied_slot(
                    sound,
                    note_secs,
                    performance_variation(seed, genre, GrooveRole::Bass, chorus, bar, slot),
                ));
            }
        }
    }
    buf
}

fn drum_slot(event: DrumEvent, slot: usize, duration_secs: f32, genre: Genre) -> Vec<f32> {
    let n = (duration_secs * SAMPLE_RATE as f32).max(1.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let mut sample = 0.0;
            if event.kick {
                let env = (-t * 18.0).exp();
                let start_hz = match genre {
                    Genre::Reggae => 64.0,
                    Genre::Country => 82.0,
                    _ => 72.0,
                };
                let phase = TAU * (start_hz * t - 18.0 * t * t);
                sample += phase.sin() * env * 0.24;
            }
            if event.snare {
                let hash = (i as u32).wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (hash as f32 / u32::MAX as f32) * 2.0 - 1.0;
                sample += match genre {
                    Genre::Jazz => noise * (-t * 16.0).exp() * 0.07,
                    Genre::Rock => noise * (-t * 24.0).exp() * 0.14,
                    Genre::Reggae => (TAU * 920.0 * t).sin() * (-t * 38.0).exp() * 0.10,
                    _ => noise * (-t * 24.0).exp() * 0.12,
                };
            }
            if event.hat {
                let hash = (i as u32)
                    .wrapping_mul(22_695_477)
                    .wrapping_add((slot as u32).wrapping_mul(1_103_515_245));
                let noise = (hash as f32 / u32::MAX as f32) * 2.0 - 1.0;
                sample += match genre {
                    Genre::Jazz => {
                        let metal = (TAU * 1_850.0 * t).sin() + 0.45 * (TAU * 2_730.0 * t).sin();
                        (metal * 0.026 + noise * 0.018) * (-t * 13.0).exp()
                    }
                    Genre::Country => noise * (-t * 42.0).exp() * 0.042,
                    _ => noise * (-t * 65.0).exp() * 0.055,
                };
            }
            sample * event.accent
        })
        .collect()
}

fn comping_slot(
    root: &str,
    quality: harmonicon_core::harmonica::ChordQuality,
    secs: f32,
    genre: Genre,
) -> Vec<f32> {
    let mut frequencies = [0.0; 3];
    let mut count = 0;
    for interval in chord_intervals(quality).iter().take(3) {
        if let Some(midi) = note_to_midi(&format!("{}3", semitone(root, *interval))) {
            frequencies[count] = midi_to_freq_hz(midi as f32);
            count += 1;
        }
    }
    let frequencies = &frequencies[..count];
    let n = (secs * SAMPLE_RATE as f32).max(1.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let attack_secs = if genre == Genre::Jazz { 0.035 } else { 0.015 };
            let attack = (t / attack_secs).min(1.0);
            let (decay_rate, harmonic, gain) = match genre {
                Genre::Blues => (4.0, 0.12, 0.13),
                Genre::Jazz => (0.8, 0.22, 0.085),
                Genre::Rock => (3.0, 0.28, 0.13),
                Genre::Reggae => (11.0, 0.18, 0.12),
                Genre::Country => (6.0, 0.10, 0.115),
            };
            let decay = (-t * decay_rate).exp();
            let chord = frequencies
                .iter()
                .map(|frequency| {
                    (TAU * frequency * t).sin() + harmonic * (TAU * frequency * 2.0 * t).sin()
                })
                .sum::<f32>()
                / frequencies.len().max(1) as f32;
            chord * attack * decay * gain
        })
        .collect()
}

fn generate_drums_pcm(bpm: f32, genre: Genre, energy: BandEnergy, seed: u64) -> Vec<f32> {
    let secs_per_beat = 60.0 / bpm.max(1.0);
    let swung = groove_arrangement(genre).swung;
    let long = if swung { secs_per_beat * SWING_LONG_FRAC } else { secs_per_beat * 0.5 };
    let short = secs_per_beat - long;
    let mut out = Vec::new();
    for chorus in 0..CHORUSES as usize {
        for bar in 0..12 {
            let arrangement = groove_arrangement_for_energy(genre, energy, chorus, bar);
            for slot in 0..8 {
                let secs = if slot % 2 == 0 { long } else { short };
                out.extend(varied_slot(
                    drum_slot(arrangement.drums[slot], slot, secs, genre),
                    secs,
                    performance_variation(seed, genre, GrooveRole::Drums, chorus, bar, slot),
                ));
            }
        }
    }
    out
}

fn generate_comping_pcm(
    key: &str,
    bpm: f32,
    progression: Progression,
    genre: Genre,
    energy: BandEnergy,
    seed: u64,
) -> Vec<f32> {
    let secs_per_beat = 60.0 / bpm.max(1.0);
    let swung = groove_arrangement(genre).swung;
    let long = if swung { secs_per_beat * SWING_LONG_FRAC } else { secs_per_beat * 0.5 };
    let short = secs_per_beat - long;
    let bars = progression_bars(key, progression);
    let mut out = Vec::new();
    for chorus in 0..CHORUSES as usize {
        for (bar, (root, quality)) in bars.iter().enumerate() {
            let arrangement = groove_arrangement_for_energy(genre, energy, chorus, bar);
            for slot in 0..8 {
                let secs = if slot % 2 == 0 { long } else { short };
                let event = arrangement.comping[slot];
                if event.hit {
                    let mut sound = comping_slot(root, *quality, secs, genre);
                    for sample in &mut sound {
                        *sample *= event.accent;
                    }
                    out.extend(varied_slot(
                        sound,
                        secs,
                        performance_variation(seed, genre, GrooveRole::Comping, chorus, bar, slot),
                    ));
                } else {
                    out.extend(std::iter::repeat_n(
                        0.0,
                        (secs * SAMPLE_RATE as f32).max(1.0) as usize,
                    ));
                }
            }
        }
    }
    out
}

/// Which of [`generate_backing_stems`]' three stems is the chordal comping —
/// the one `jam::band` thins when the player has just been busy.
pub(crate) const COMPING_STEM: usize = 2;

/// A short reply the band may play in a phrase-end window when the player
/// has just finished a phrase (`jam::band`): a drum fill or a chord push.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BandAnswer {
    Drums,
    Chord,
}

/// Renders one [`BandAnswer`], two beats long on `genre`'s eighth grid so
/// it lands on beats 3–4 of a bar. The last eighth is always a rest and the
/// accents rise toward it, so the answer points at the next downbeat rather
/// than covering it. Same synthesis as the stems (`drum_slot`/
/// `comping_slot`), so it sounds like the same band.
pub fn render_band_answer(
    answer: BandAnswer,
    genre: Genre,
    root: &str,
    quality: harmonicon_core::harmonica::ChordQuality,
    bpm: f32,
) -> Vec<f32> {
    let secs_per_beat = 60.0 / bpm.max(1.0);
    let swung = groove_arrangement(genre).swung;
    let long = if swung { secs_per_beat * SWING_LONG_FRAC } else { secs_per_beat * 0.5 };
    let short = secs_per_beat - long;
    let mut out = Vec::new();
    for slot in 0..4 {
        let secs = if slot % 2 == 0 { long } else { short };
        let accent = 0.5 + slot as f32 * 0.12;
        let rest = slot == 3;
        match answer {
            BandAnswer::Drums => out.extend(drum_slot(
                drum(slot == 0, !rest, !rest, if rest { 0.0 } else { accent }),
                slot,
                secs,
                genre,
            )),
            BandAnswer::Chord if !rest && slot != 1 => out.extend(
                comping_slot(root, quality, secs, genre).into_iter().map(|sample| sample * accent),
            ),
            BandAnswer::Chord => {
                out.extend(std::iter::repeat_n(0.0, (secs * SAMPLE_RATE as f32).max(1.0) as usize))
            }
        }
    }
    out
}

/// Bass, drums and chordal comping rendered as sample-aligned stems. The bass
/// renderer remains the duration authority; the other pure renderers are
/// trimmed/padded to its length so independently spawned sinks cannot drift.
pub fn generate_backing_stems(
    key: &str,
    bpm: f32,
    progression: Progression,
    genre: Genre,
    energy: BandEnergy,
    seed: u64,
) -> [(String, Vec<f32>); 3] {
    let bass = generate_bass_pcm_seeded(key, bpm, progression, genre, seed);
    let target = bass.len();
    let mut drums = generate_drums_pcm(bpm, genre, energy, seed);
    let mut comping = generate_comping_pcm(key, bpm, progression, genre, energy, seed);
    drums.resize(target, 0.0);
    comping.resize(target, 0.0);
    drums.truncate(target);
    comping.truncate(target);
    [("Bass".to_string(), bass), ("Drums".to_string(), drums), ("Comping".to_string(), comping)]
}

/// The chart half of a generated jam: a diatonic Richter harp for `position`
/// in the jam's `key` (e.g. `Position::Second` picks a harp a 4th below
/// `key` — see `Position::harp_key`), timed to a standard 12-bar
/// progression, and a single marker track item (Jam Session never scores
/// notes, so its only job is satisfying the chart schema's `minItems: 1`
/// and giving the progress bar something to measure against). `song.feel`
/// is seeded from `genre` (see `Genre::metronome_feel`) — picked up by
/// `gameplay::metronome_overlay::feel_from_chart` the same way a
/// hand-authored chart's own `feel` field already is, so the on-screen
/// metronome's swing/straight toggle reflects the picked genre for free.
pub fn generated_chart(
    key: &str,
    bpm: f32,
    progression: Progression,
    position: Position,
    genre: Genre,
    total_secs: f64,
) -> HarpChart {
    let harp_key = position.harp_key(key);
    let mut harmonica = richter_harp(&harp_key);
    if let harmonicon_core::harmonica::Harmonica::Diatonic { position: pos, .. } = &mut harmonica {
        *pos = Some(position.label().to_string());
    }
    HarpChart {
        metadata: Some(Metadata {
            format_version: Some("1.1.0".to_string()),
            author: Some("Harmonicon".to_string()),
            source: Some("Procedurally generated".to_string()),
            license: Some("MIT".to_string()),
            description: Some(format!(
                "Generated {} {} 12-bar jam backing, key of {key}, {bpm:.0} bpm.",
                genre.label(),
                progression.label()
            )),
        }),
        song: Song {
            title: format!("Generated Jam \u{2014} Key of {key}"),
            artist: "Harmonicon".to_string(),
            genre: genre.label().to_string(),
            tempo_bpm: bpm,
            key: key.to_string(),
            time_signature: Some("4/4".to_string()),
            difficulty: Difficulty::Easy,
            feel: Some(genre.metronome_feel()),
        },
        timing: Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint { tick: 0, bpm }],
            time_signature_map: None,
            pickup_ticks: None,
            repeats: Vec::new(),
        },
        harmonica,
        track: vec![TrackItem {
            id: None,
            time: Some(0.0),
            tick: None,
            duration: total_secs,
            phrase: None,
            groove: None,
            chord: None,
            play_mode: None,
            call: false,
            lyric: None,
            events: vec![NoteEvent { hole: 1, action: Action::Blow, note: None, modifiers: None }],
        }],
        loop_section: None,
        scoring: Scoring {
            perfect_window_ms: 150,
            good_window_ms: 350,
            miss_window_ms: 600,
            combo: None,
            style_bonus: None,
        },
    }
}

/// A generated jam's backing with all the expensive work done: the three
/// stems synthesized and WAV-encoded, and their mix reduced to the progress
/// bar's waveform. It touches no ECS state, so [`render_generated_backing`]
/// can run on a worker thread; the render takes 100–250 ms.
pub struct RenderedBacking {
    stems: Vec<(String, Vec<u8>)>,
    waveform: Vec<f32>,
    music_duration_secs: f64,
}

/// Synthesizes independently mutable bass, drum and comping stems for
/// [`assemble_generated_manifest`].
pub fn render_generated_backing(
    key: &str,
    bpm: f32,
    progression: Progression,
    genre: Genre,
    energy: BandEnergy,
    seed: u64,
) -> RenderedBacking {
    let stems = generate_backing_stems(key, bpm, progression, genre, energy, seed);
    let music_duration_secs = stems[0].1.len() as f64 / SAMPLE_RATE as f64;
    let mut mix = vec![0.0; stems[0].1.len()];
    for (_, pcm) in &stems {
        for (mixed, sample) in mix.iter_mut().zip(pcm) {
            *mixed += *sample;
        }
    }
    RenderedBacking {
        waveform: bucket_peaks(&mix, WAVEFORM_BUCKETS),
        stems: stems.into_iter().map(|(name, pcm)| (name, encode_wav(&pcm, SAMPLE_RATE))).collect(),
        music_duration_secs,
    }
}

/// Builds the full generated-jam `SongManifest` in one call:
/// [`render_generated_backing`] then [`assemble_generated_manifest`]. For
/// callers that can afford the render on the current thread.
pub fn build_generated_manifest(
    key: &str,
    bpm: f32,
    progression: Progression,
    position: Position,
    genre: Genre,
    energy: BandEnergy,
    seed: u64,
    background: Handle<Image>,
    elements: Handle<Image>,
    sources: &mut Assets<AudioSource>,
) -> SongManifest {
    let rendered = render_generated_backing(key, bpm, progression, genre, energy, seed);
    assemble_generated_manifest(
        rendered,
        key,
        bpm,
        progression,
        position,
        genre,
        background,
        elements,
        sources,
    )
}

/// Registers each rendered stem as an `AudioSource` and assembles the chart
/// around them. `key`/`bpm`/`progression`/`genre` must be the values the
/// backing was rendered with. `background`/`elements` are the caller's
/// choice of placeholder art — Jam Session never reads `elements` at all;
/// `background` paints behind the hole map/12-bar grid (see
/// `jam::session::setup`), so a theme's generic `default_background` is the
/// natural choice.
pub fn assemble_generated_manifest(
    rendered: RenderedBacking,
    key: &str,
    bpm: f32,
    progression: Progression,
    position: Position,
    genre: Genre,
    background: Handle<Image>,
    elements: Handle<Image>,
    sources: &mut Assets<AudioSource>,
) -> SongManifest {
    let RenderedBacking { stems, waveform, music_duration_secs } = rendered;
    let backing_stems = stems
        .into_iter()
        .map(|(name, wav)| BackingStemAudio {
            name,
            source: sources.add(AudioSource { bytes: wav.into() }),
        })
        .collect();

    SongManifest {
        path: PathBuf::from(format!("generated/{key}")),
        chart: generated_chart(key, bpm, progression, position, genre, music_duration_secs),
        background,
        music: None,
        backing_stems: Some(backing_stems),
        waveform,
        music_duration_secs,
        elements,
        assets_2d: None,
        assets_2d_config: NoteThemeConfig::default(),
        // A generated jam has no source file to have picked a track from,
        // so there is nothing for the harp-check picker to offer.
        source_tracks: Vec::new(),
        source_track: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bar_beat_freqs ───────────────────────────────────────────────────────

    #[test]
    fn bar_beat_freqs_follows_the_blues_box_shape() {
        // R R 5 5 b7 b7 5 b7 — see `BLUES_PATTERN`.
        let freqs = bar_beat_freqs("C", &BLUES_PATTERN);
        let hz = |note: &str| midi_to_freq_hz(note_to_midi(note).unwrap() as f32);
        let (root_hz, fifth_hz, flat7_hz) = (hz("C3"), hz("G3"), hz("A#3"));
        let expected =
            [root_hz, root_hz, fifth_hz, fifth_hz, flat7_hz, flat7_hz, fifth_hz, flat7_hz];
        for (i, (got, want)) in freqs.iter().zip(expected).enumerate() {
            assert!((got.unwrap() - want).abs() < 0.01, "note {i}: got {got:?}, expected {want}");
        }
    }

    #[test]
    fn bar_beat_freqs_stays_above_typical_small_speaker_cutoff() {
        // Regression guard for the "technically playing, inaudible on a
        // laptop speaker" bug: every note every genre can produce, across
        // every key, must clear ~100 Hz. C is the lowest pitch class, so if
        // it clears the bar every other key does too.
        for genre in Genre::all() {
            let arrangement = groove_arrangement(*genre);
            for &f in bar_beat_freqs("C", &arrangement.bass).iter().flatten() {
                assert!(f > 100.0, "{genre:?}: {f} Hz is below typical small-speaker cutoff");
            }
        }
    }

    // ── Genre ────────────────────────────────────────────────────────────────

    #[test]
    fn every_genre_label_round_trips_through_from_label() {
        for &genre in Genre::all() {
            assert_eq!(Genre::from_label(genre.label()), Some(genre));
        }
    }

    #[test]
    fn from_label_is_none_for_an_unknown_label() {
        assert_eq!(Genre::from_label("Ska"), None);
    }

    #[test]
    fn blues_and_jazz_swing_the_rest_play_straight() {
        assert!(groove_arrangement(Genre::Blues).swung);
        assert!(groove_arrangement(Genre::Jazz).swung);
        assert!(!groove_arrangement(Genre::Rock).swung);
        assert!(!groove_arrangement(Genre::Reggae).swung);
        assert!(!groove_arrangement(Genre::Country).swung);
    }

    #[test]
    fn metronome_feel_matches_swing_flag() {
        // Every swung genre should feel like Shuffle live, every straight
        // genre like Straight — the two shouldn't be able to disagree.
        for &genre in Genre::all() {
            let swung = groove_arrangement(genre).swung;
            let expected = if swung { Feel::Shuffle } else { Feel::Straight };
            assert_eq!(genre.metronome_feel(), expected, "{genre:?}");
        }
    }

    #[test]
    fn every_arrangement_has_bounded_role_events() {
        for &genre in Genre::all() {
            let arrangement = groove_arrangement(genre);
            assert!(arrangement.bass.iter().any(Option::is_some), "{genre:?} bass");
            assert!(
                arrangement.drums.iter().any(|event| event.kick || event.snare || event.hat),
                "{genre:?} drums"
            );
            assert!(arrangement.comping.iter().any(|event| event.hit), "{genre:?} comping");
            assert!(
                arrangement.drums.iter().all(|event| (0.0..=1.0).contains(&event.accent)),
                "{genre:?} drum accent"
            );
            assert!(
                arrangement.comping.iter().all(|event| (0.0..=1.0).contains(&event.accent)),
                "{genre:?} comping accent"
            );
        }
    }

    #[test]
    fn genre_changes_instrument_voice_not_only_event_pattern() {
        let quality = progression_bars("C", Progression::Standard)[0].1;
        let blues_comp = comping_slot("C", quality, 0.5, Genre::Blues);
        let jazz_comp = comping_slot("C", quality, 0.5, Genre::Jazz);
        let reggae_comp = comping_slot("C", quality, 0.5, Genre::Reggae);
        assert_ne!(blues_comp, jazz_comp);
        assert_ne!(blues_comp, reggae_comp);

        let event = drum(false, false, true, 1.0);
        assert_ne!(drum_slot(event, 1, 0.5, Genre::Blues), drum_slot(event, 1, 0.5, Genre::Jazz));
    }

    #[test]
    fn phrase_fills_are_restrained_and_turnaround_approaches_home() {
        for &genre in Genre::all() {
            let base = groove_arrangement(genre);
            let bar_four = groove_arrangement_for_position(genre, 2, 3);
            assert_eq!(bar_four.bass, base.bass, "{genre:?} bar 4 bass");
            assert!(bar_four.drums[7].snare, "{genre:?} bar 4 fill");

            let turnaround = groove_arrangement_for_position(genre, 2, 11);
            assert_eq!(turnaround.bass[5..], [Some(9), Some(10), Some(11)]);
            assert!(turnaround.drums[5..].iter().all(|event| event.snare));
            assert!(turnaround.comping[5..].iter().all(|event| !event.hit));
            assert_eq!(
                groove_arrangement_for_position(genre, 2, 12),
                groove_arrangement_for_position(genre, 2, 0),
                "{genre:?} wrapped bar returns to the same chorus pocket"
            );
        }
    }

    #[test]
    fn four_chorus_arc_opens_sparse_peaks_then_relaxes() {
        for &genre in Genre::all() {
            let active_comp = |chorus| {
                groove_arrangement_for_position(genre, chorus, 0)
                    .comping
                    .into_iter()
                    .filter(|event| event.hit)
                    .count()
            };
            let drum_energy = |chorus| {
                groove_arrangement_for_position(genre, chorus, 0)
                    .drums
                    .into_iter()
                    .map(|event| event.accent)
                    .sum::<f32>()
            };
            assert!(active_comp(0) <= active_comp(1), "{genre:?} sparse opening");
            assert!(drum_energy(2) > drum_energy(0), "{genre:?} third chorus lift");
            assert!(drum_energy(3) < drum_energy(2), "{genre:?} fourth chorus relax");
        }
    }

    #[test]
    fn band_energy_changes_density_without_changing_bass_or_feel() {
        for &genre in Genre::all() {
            let low = groove_arrangement_for_energy(genre, BandEnergy::Low, 2, 0);
            let medium = groove_arrangement_for_energy(genre, BandEnergy::Medium, 2, 0);
            let high = groove_arrangement_for_energy(genre, BandEnergy::High, 2, 0);
            let comp_hits = |arrangement: &GrooveArrangement| {
                arrangement.comping.iter().filter(|event| event.hit).count()
            };
            let drum_energy = |arrangement: &GrooveArrangement| {
                arrangement.drums.iter().map(|event| event.accent).sum::<f32>()
            };

            assert_eq!(low.bass, medium.bass, "{genre:?} low bass");
            assert_eq!(high.bass, medium.bass, "{genre:?} high bass");
            assert_eq!(low.swung, medium.swung, "{genre:?} low feel");
            assert_eq!(high.swung, medium.swung, "{genre:?} high feel");
            assert!(comp_hits(&low) <= comp_hits(&medium), "{genre:?} low density");
            assert!(comp_hits(&high) >= comp_hits(&medium), "{genre:?} high density");
            assert!(drum_energy(&low) < drum_energy(&medium), "{genre:?} low dynamics");
            assert!(drum_energy(&high) >= drum_energy(&medium), "{genre:?} high dynamics");
        }
    }

    // ── generate_bass_pcm ────────────────────────────────────────────────────

    #[test]
    fn generate_bass_pcm_is_audible() {
        let pcm = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        assert!(!pcm.is_empty());
        assert!(pcm.iter().any(|&s| s.abs() > 0.01), "generated backing should not be silent");
    }

    #[test]
    fn ending_uses_three_aligned_audible_stems_and_follows_tempo() {
        let quality = harmonicon_core::harmonica::ChordQuality::Dominant7;
        for &genre in Genre::all() {
            let slow = generate_ending_stems("C", quality, 60.0, genre);
            let fast = generate_ending_stems("C", quality, 120.0, genre);
            assert!(slow.iter().all(|stem| stem.len() == slow[0].len()));
            assert!(fast.iter().all(|stem| stem.len() == fast[0].len()));
            assert!(slow.iter().flatten().any(|sample| sample.abs() > 0.01));
            assert!(slow.iter().all(|stem| stem.iter().any(|s| s.abs() > 0.005)));
            let peak = (0..slow[0].len())
                .map(|sample| slow.iter().map(|stem| stem[sample]).sum::<f32>().abs())
                .fold(0.0_f32, f32::max);
            assert!(peak <= 0.95, "{genre:?} ending mix peaks at {peak}");
            assert_eq!(slow[0].len(), fast[0].len() * 2);
        }
    }

    #[test]
    fn every_genre_is_audible() {
        for &genre in Genre::all() {
            let pcm = generate_bass_pcm("C", 90.0, Progression::Standard, genre);
            assert!(
                pcm.iter().any(|&s| s.abs() > 0.01),
                "{genre:?}: generated backing should not be silent"
            );
        }
    }

    #[test]
    fn generate_bass_pcm_length_matches_chorus_count_and_tempo() {
        let bpm = 120.0;
        let pcm = generate_bass_pcm("C", bpm, Progression::Standard, Genre::Blues);
        let secs_per_beat = 60.0 / bpm;
        let expected_secs = CHORUSES as f64 * 12.0 * 4.0 * secs_per_beat as f64;
        let actual_secs = pcm.len() as f64 / SAMPLE_RATE as f64;
        assert!(
            (actual_secs - expected_secs).abs() < 0.5,
            "expected ~{expected_secs}s, got {actual_secs}s"
        );
    }

    #[test]
    fn faster_tempo_yields_a_shorter_loop() {
        let slow = generate_bass_pcm("C", 60.0, Progression::Standard, Genre::Blues);
        let fast = generate_bass_pcm("C", 120.0, Progression::Standard, Genre::Blues);
        assert!(fast.len() < slow.len());
    }

    #[test]
    fn every_progression_renders_the_same_length_loop() {
        // Only the chord *roots* differ between progressions — same 12
        // bars, same beats per bar, so the rendered length shouldn't budge.
        let standard = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        let quick = generate_bass_pcm("C", 90.0, Progression::QuickChange, Genre::Blues);
        let minor = generate_bass_pcm("C", 90.0, Progression::Minor, Genre::Blues);
        let jazz = generate_bass_pcm("C", 90.0, Progression::JazzBlues, Genre::Blues);
        assert_eq!(standard.len(), quick.len());
        assert_eq!(standard.len(), minor.len());
        assert_eq!(standard.len(), jazz.len());
    }

    #[test]
    fn every_genre_renders_the_same_length_loop() {
        // Every pattern is still 8 slots summing to exactly one bar (4
        // beats), swung or straight — genre reshapes the rhythm, not the
        // total loop duration. A small tolerance, not exact equality: the
        // swung-vs-straight timing math rounds to whole samples slightly
        // differently, the same reason `generate_bass_pcm_length_matches_
        // chorus_count_and_tempo` above tolerates float error rather than
        // asserting an exact sample count.
        let blues = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        for &genre in Genre::all() {
            let pcm = generate_bass_pcm("C", 90.0, Progression::Standard, genre);
            let diff_secs = (blues.len() as f64 - pcm.len() as f64).abs() / SAMPLE_RATE as f64;
            assert!(diff_secs < 0.1, "{genre:?} loop length diverged from Blues by {diff_secs}s");
        }
    }

    #[test]
    fn different_genres_produce_different_audio() {
        // Guards against a future refactor silently collapsing genres back
        // to identical output.
        let blues = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        let rock = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Rock);
        let reggae = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Reggae);
        assert_ne!(blues, rock);
        assert_ne!(blues, reggae);
        assert_ne!(rock, reggae);
    }

    #[test]
    fn rhythm_section_stems_are_audible_and_sample_aligned() {
        for &genre in Genre::all() {
            let stems = generate_backing_stems(
                "C",
                90.0,
                Progression::Standard,
                genre,
                BandEnergy::Medium,
                42,
            );
            let expected_len = stems[0].1.len();
            for (name, pcm) in stems {
                assert_eq!(pcm.len(), expected_len, "{genre:?} {name} drifted");
                assert!(
                    pcm.iter().any(|sample| sample.abs() > 0.005),
                    "{genre:?} {name} is silent"
                );
            }
        }
    }

    #[test]
    fn rhythm_section_mix_keeps_headroom() {
        for &genre in Genre::all() {
            for &energy in BandEnergy::all() {
                let stems =
                    generate_backing_stems("C", 90.0, Progression::Standard, genre, energy, 42);
                let peak = (0..stems[0].1.len())
                    .map(|sample| stems.iter().map(|(_, pcm)| pcm[sample]).sum::<f32>().abs())
                    .fold(0.0_f32, f32::max);
                assert!(peak <= 0.95, "{genre:?} {energy:?} mix peaks at {peak}");
            }
        }
    }

    // ── generated_chart ──────────────────────────────────────────────────────

    #[test]
    fn generated_chart_carries_the_requested_key_and_tempo() {
        let chart =
            generated_chart("G", 100.0, Progression::Standard, Position::First, Genre::Blues, 30.0);
        assert_eq!(chart.song.key, "G");
        assert_eq!(chart.song.tempo_bpm, 100.0);
        assert_eq!(chart.timing.tempo_map[0].bpm, 100.0);
    }

    #[test]
    fn generated_chart_harmonica_is_a_diatonic_richter_harp_in_key_at_first_position() {
        let chart =
            generated_chart("D", 90.0, Progression::Standard, Position::First, Genre::Blues, 30.0);
        match chart.harmonica {
            harmonicon_core::harmonica::Harmonica::Diatonic { holes, layout, position, .. } => {
                assert_eq!(holes, 10);
                let layout = layout.expect("richter_harp always sets a layout");
                assert_eq!(layout.blow.unwrap()[0], "D4");
                assert_eq!(position.as_deref(), Some("1st"));
            }
            _ => panic!("expected a diatonic harp"),
        }
    }

    #[test]
    fn generated_chart_second_position_picks_a_harp_a_fourth_below_the_jam_key() {
        // A cross-harp jam in G is played on a C harp.
        let chart =
            generated_chart("G", 90.0, Progression::Standard, Position::Second, Genre::Blues, 30.0);
        match chart.harmonica {
            harmonicon_core::harmonica::Harmonica::Diatonic { layout, position, .. } => {
                let layout = layout.expect("richter_harp always sets a layout");
                assert_eq!(layout.blow.unwrap()[0], "C4");
                assert_eq!(position.as_deref(), Some("2nd"));
            }
            _ => panic!("expected a diatonic harp"),
        }
    }

    #[test]
    fn generated_chart_track_is_never_empty() {
        // The chart schema requires `track.minItems: 1` — a generated jam
        // has no real notes to schedule, but must still satisfy it.
        let chart =
            generated_chart("C", 90.0, Progression::Standard, Position::First, Genre::Blues, 30.0);
        assert!(!chart.track.is_empty());
        assert!(!chart.track[0].events.is_empty());
    }

    #[test]
    fn generated_chart_feel_matches_genre() {
        for &genre in Genre::all() {
            let chart =
                generated_chart("C", 90.0, Progression::Standard, Position::First, genre, 30.0);
            assert_eq!(chart.song.feel, Some(genre.metronome_feel()), "{genre:?}");
        }
    }

    // ── render_band_answer ───────────────────────────────────────────────────

    #[test]
    fn band_answers_last_two_beats_and_leave_the_last_eighth_open() {
        use harmonicon_core::harmonica::ChordQuality;
        for genre in Genre::all() {
            for answer in [BandAnswer::Drums, BandAnswer::Chord] {
                let pcm = render_band_answer(answer, *genre, "C", ChordQuality::Dominant7, 120.0);
                let two_beats = (2.0 * 60.0 / 120.0 * SAMPLE_RATE as f32) as usize;
                assert!(
                    (pcm.len() as i64 - two_beats as i64).abs() <= 4,
                    "{genre:?} {answer:?}: {} samples for {two_beats}",
                    pcm.len()
                );
                let peak = pcm.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                assert!(peak > 0.02 && peak <= 1.0, "{genre:?} {answer:?} peak {peak}");
                // The final eighth (the short one when swung) is silence, so
                // the answer never covers the downbeat it points at.
                let short =
                    if groove_arrangement(*genre).swung { 1.0 - SWING_LONG_FRAC } else { 0.5 };
                let tail = (short * 60.0 / 120.0 * SAMPLE_RATE as f32) as usize - 8;
                assert!(
                    pcm[pcm.len() - tail..].iter().all(|s| *s == 0.0),
                    "{genre:?} {answer:?} sounds into the downbeat"
                );
            }
        }
    }

    // ── build_generated_manifest ─────────────────────────────────────────────

    #[test]
    fn build_generated_manifest_registers_three_stem_assets() {
        let mut sources = Assets::<AudioSource>::default();
        let manifest = build_generated_manifest(
            "C",
            90.0,
            Progression::Standard,
            Position::First,
            Genre::Blues,
            BandEnergy::Medium,
            42,
            Handle::default(),
            Handle::default(),
            &mut sources,
        );
        assert!(manifest.music.is_none());
        let stems = manifest.backing_stems.expect("generated jam always has rhythm-section stems");
        assert_eq!(stems.len(), 3);
        assert_eq!(
            stems.iter().map(|stem| stem.name.as_str()).collect::<Vec<_>>(),
            ["Bass", "Drums", "Comping"]
        );
        assert_eq!(stems[COMPING_STEM].name, "Comping");
        assert!(stems.iter().all(|stem| sources.get(&stem.source).is_some()));
        assert!(manifest.music_duration_secs > 0.0);
        assert_eq!(manifest.waveform.len(), WAVEFORM_BUCKETS);
    }
}
