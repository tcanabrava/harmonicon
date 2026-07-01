// SPDX-License-Identifier: MIT

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::audio_system::midi::note_to_midi;
use crate::settings::AudioSettings;
use super::{TICKS_PER_BEAT, TICK_W};
use super::state::{Dir, Expr, GridNote, Pitch};

// ── Constants ────────────────────────────────────────────────────────────────

/// CD-quality mono output used for the in-editor synthesis preview.
pub(super) const SAMPLE_RATE: u32 = 44_100;

/// Standard Richter-tuned C-harp blow notes, holes 1–10.
pub(super) const C_BLOW: [&str; 10] =
    ["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"];
/// Standard Richter-tuned C-harp draw notes, holes 1–10.
pub(super) const C_DRAW: [&str; 10] =
    ["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"];

// ── Synthesis parameters ─────────────────────────────────────────────────────

/// Short breath-attack transient — harmonica reed takes ~18 ms to speak.
const ATTACK_SECS: f32 = 0.018;
/// Natural reed release after the player stops — audible ~45 ms tail.
const RELEASE_SECS: f32 = 0.045;
/// Extra silence appended after the last note so the release isn't clipped.
const TAIL_SECS: f32 = 0.25;

/// Breath-noise amplitude relative to the tonal signal (7 %).
const BREATH_NOISE_AMP: f32 = 0.07;
/// Exponential decay rate of the breath-noise burst after the attack peak.
/// Higher = faster decay; -3.0 gives a ~333 ms half-life from the attack end.
const BREATH_NOISE_DECAY: f32 = -3.0;

/// Per-note output level before final peak normalisation.
/// 0.25 leaves headroom for up to four simultaneously overlapping notes.
const NOTE_LEVEL: f32 = 0.25;

// Vibrato — pitch (frequency) modulation mimicking tongue/diaphragm flutter.
/// LFO rate in Hz. 7 Hz sits at the fast end of realistic harmonica vibrato.
const VIBRATO_RATE: f32 = 7.0;
/// Frequency deviation as a fraction of the base pitch (±1.5 %).
const VIBRATO_DEPTH: f32 = 0.015;

// Hand-wah — player cups/uncovers hands around the harmonica body.
/// LFO rate in Hz. Hand movement is naturally slower than diaphragm vibrato.
const WAH_RATE: f32 = 3.0;
/// Minimum amplitude fraction when hands are fully cupped (closed position).
const WAH_AMP_CLOSED: f32 = 0.35;

// Partial amplitudes for the additive-synthesis harmonica model.
// Values are subjectively tuned to approximate a diatonic harp timbre.
/// Fundamental (k=1) amplitude — dominant component of the reed sound.
const P1: f32 = 1.00;
/// Second harmonic (k=2) — adds body/warmth.
const P2: f32 = 0.50;
/// Third harmonic (k=3) — characteristic harmonica "buzz".
const P3: f32 = 0.35;
/// Fourth harmonic (k=4) — upper brightness.
const P4: f32 = 0.18;
/// Fifth harmonic (k=5) — subtle edge.
const P5: f32 = 0.10;
/// Sixth harmonic (k=6) — air and shimmer.
const P6: f32 = 0.05;
/// Sum of all partial amplitudes — used to normalise the wave to [-1, 1].
const PARTIALS_SUM: f32 = P1 + P2 + P3 + P4 + P5 + P6;

// LCG (linear-congruential generator) parameters for per-note breath noise.
// These are the Numerical Recipes / glibc constants, chosen for good spectral
// distribution at low cost — not for cryptographic quality.
/// LCG seed mixed with hole and tick so each note has a unique noise stream.
const LCG_SEED: u32 = 0x9e3779b9; // Fibonacci hashing constant
/// Per-hole mixing multiplier (Knuth multiplicative hash).
const LCG_HOLE_MIX: u32 = 2_654_435_761;
/// Per-tick mixing multiplier (co-prime to 2^32, good avalanche).
const LCG_TICK_MIX: u32 = 1_013_904_223;
/// LCG multiplier (Numerical Recipes `ranqd1`).
const LCG_MUL: u32 = 1_664_525;
/// LCG increment (same source).
const LCG_INC: u32 = 1_013_904_223;

// ── Components / Resources ───────────────────────────────────────────────────

/// Marks an audio player spawned by the editor's Play button.
#[derive(Component)]
pub(super) struct EditorAudio;

/// The moving playback cursor (a vertical line) drawn over the grid.
#[derive(Component)]
pub(super) struct PlayheadLine;

/// The growing fill of the top progress bar.
#[derive(Component)]
pub(super) struct EditorProgressFill;

#[derive(Resource, Default)]
pub(super) struct Playhead {
    pub(super) playing: bool,
    pub(super) elapsed: f32,
    pub(super) total: f32,
    pub(super) secs_per_tick: f32,
}

// ── Pure functions ────────────────────────────────────────────────────────────

pub(super) fn key_offset(key: &str) -> i32 {
    note_to_midi(&format!("{}4", key.trim())).map_or(0, |m| m - 60)
}

pub(super) fn note_freq(note: &GridNote, key_offset: i32) -> Option<f32> {
    let idx = (note.hole as usize).checked_sub(1)?;
    let label = match note.dir {
        Dir::Blow => *C_BLOW.get(idx)?,
        Dir::Draw => *C_DRAW.get(idx)?,
    };
    let midi = note_to_midi(label)? as f32 + key_offset as f32;
    let semitones = match note.pitch {
        Pitch::Normal => 0.0,
        Pitch::Bend(a) => -a,
        Pitch::Overblow | Pitch::Overdraw => 1.0,
    };
    Some(440.0 * 2f32.powf((midi + semitones - 69.0) / 12.0))
}

/// Full harmonic content — sounds bright/open, like uncupped hands.
/// `phase_mod` is an additional phase offset applied to all harmonics; use it
/// for vibrato so the modulation is expressed as bounded phase deviation rather
/// than a drifting frequency × time product.
fn harmonica_wave(freq: f32, t: f32, phase_mod: f32) -> f32 {
    let mut s = 0.0f32;
    for (k, amp) in [(1.0f32, P1), (2.0, P2), (3.0, P3), (4.0, P4), (5.0, P5), (6.0, P6)] {
        s += amp * (TAU * freq * k * t + k * phase_mod).sin();
    }
    s / PARTIALS_SUM
}

/// Muffled version — fundamental only, as if hands fully cup the harmonica.
/// Used as the dark extreme of the hand-wah crossfade.
fn harmonica_wave_muffled(freq: f32, t: f32, phase_mod: f32) -> f32 {
    (TAU * freq * t + phase_mod).sin()
}

fn envelope(i: usize, dur: usize) -> f32 {
    let attack  = (SAMPLE_RATE as f32 * ATTACK_SECS)  as usize;
    let release = (SAMPLE_RATE as f32 * RELEASE_SECS) as usize;
    let atk = if attack > 0 && i < attack { i as f32 / attack as f32 } else { 1.0 };
    let rel = if dur > release && i > dur - release {
        (dur - i) as f32 / release as f32
    } else {
        1.0
    };
    atk.min(rel).clamp(0.0, 1.0)
}

pub(super) fn render_pcm(notes: &[GridNote], bpm: f32, key_offset: i32) -> Vec<f32> {
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let end_tick = notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    let total = ((end_tick as f32 * secs_per_tick + TAIL_SECS) * SAMPLE_RATE as f32).ceil() as usize;
    let mut buf = vec![0.0f32; total.max(1)];

    let attack_samples = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;

    for n in notes {
        let Some(freq) = note_freq(n, key_offset) else { continue };
        let start = (n.tick as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;
        let dur   = (n.len  as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;

        // Unique per-note LCG seed so each note has an independent breath-noise stream.
        let mut rng: u32 = LCG_SEED
            .wrapping_add((n.hole as u32).wrapping_mul(LCG_HOLE_MIX))
            .wrapping_add((n.tick as u32).wrapping_mul(LCG_TICK_MIX));

        for i in 0..dur {
            let s = start + i;
            if s >= buf.len() { break; }
            let t   = i as f32 / SAMPLE_RATE as f32;
            let env = envelope(i, dur);

            // ── Vibrato: phase-correct pitch fluctuation ─────────────────────
            // Naively writing sin(TAU * f_mod * t) where f_mod varies with t
            // causes the modulation term (depth * sin(rate*t) * t) to grow
            // without bound, making the pitch appear to rise over time.
            //
            // The correct approach is to integrate the instantaneous frequency:
            //   f(t) = freq * (1 + depth * sin(TAU * rate * t))
            //   φ(t) = TAU * freq * t  +  freq*depth/rate * (1 - cos(TAU*rate*t))
            //
            // The second term is the bounded phase deviation Δφ(t); it
            // oscillates symmetrically between 0 and 2*freq*depth/rate, so the
            // pitch wobbles evenly above and below the base frequency.
            let phase_mod = match n.expr {
                Expr::Vibrato => {
                    freq * VIBRATO_DEPTH / VIBRATO_RATE
                        * (1.0 - (TAU * VIBRATO_RATE * t).cos())
                }
                _ => 0.0,
            };

            // ── Hand Wah: amplitude + tone-color modulation ──────────────────
            // `wah_open` oscillates between 0.0 (hands fully cupped = dark,
            // quiet) and 1.0 (hands uncovered = bright, full volume).
            // Amplitude dips toward WAH_AMP_CLOSED when cupped.
            // Tone color is crossfaded from muffled (fundamental only) to the
            // full bright harmonic stack as the hands open.
            let (tone, amp_mod) = if n.expr == Expr::Wah {
                let wah_open = ((TAU * WAH_RATE * t).sin() + 1.0) * 0.5;
                let bright   = harmonica_wave(freq, t, 0.0);
                let muffled  = harmonica_wave_muffled(freq, t, 0.0);
                let blended  = muffled + wah_open * (bright - muffled);
                let amp      = WAH_AMP_CLOSED + (1.0 - WAH_AMP_CLOSED) * wah_open;
                (blended, amp)
            } else {
                (harmonica_wave(freq, t, phase_mod), 1.0)
            };

            // ── Breath noise ─────────────────────────────────────────────────
            rng = rng.wrapping_mul(LCG_MUL).wrapping_add(LCG_INC);
            let noise_sample = (rng as i32) as f32 / i32::MAX as f32;
            let noise_env = if i < attack_samples {
                1.0
            } else {
                (BREATH_NOISE_DECAY * (i - attack_samples) as f32 / SAMPLE_RATE as f32).exp()
            };
            let breath = noise_sample * BREATH_NOISE_AMP * noise_env;

            buf[s] += NOTE_LEVEL * env * amp_mod * (tone + breath);
        }
    }

    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 1.0 {
        for x in &mut buf { *x /= peak; }
    }
    buf
}

pub(super) fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    // WAV/RIFF layout per the PCM subset of the WAVE spec:
    //   RIFF chunk (12 bytes) + fmt subchunk (24 bytes) + data subchunk header (8 bytes)
    //   = 44 bytes of header, followed by raw 16-bit LE PCM samples.
    let channels: u16 = 1;            // mono
    let bits_per_sample: u16 = 16;    // 16-bit PCM
    let bytes_per_sample = (bits_per_sample / 8) as u32;
    let data_len  = (samples.len() as u32) * bytes_per_sample;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample;
    let block_align = (channels as u32 * bytes_per_sample) as u16;

    let mut v = Vec::with_capacity(44 + data_len as usize);
    // RIFF chunk descriptor
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes()); // total file size minus 8
    v.extend_from_slice(b"WAVE");
    // fmt subchunk (16 bytes for PCM)
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());            // subchunk size for PCM
    v.extend_from_slice(&1u16.to_le_bytes());             // AudioFormat = PCM (no compression)
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&byte_rate.to_le_bytes());
    v.extend_from_slice(&block_align.to_le_bytes());
    v.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data subchunk
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let q = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        v.extend_from_slice(&q.to_le_bytes());
    }
    v
}

pub(super) fn start_playback(
    state: &super::state::EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing: &Query<Entity, With<EditorAudio>>,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    *playhead = Playhead::default();

    let bpm = state.tempo.trim().parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    if !state.notes.is_empty() {
        let wav = encode_wav(&render_pcm(&state.notes, bpm, key_offset(&state.key)), SAMPLE_RATE);
        let handle = sources.add(AudioSource { bytes: wav.into() });
        commands.spawn((
            EditorAudio,
            AudioPlayer::<AudioSource>(handle),
            PlaybackSettings::DESPAWN,
        ));
        let end_tick = state.notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
        *playhead = Playhead {
            playing: true,
            elapsed: 0.0,
            total: end_tick as f32 * secs_per_tick,
            secs_per_tick,
        };
    }

    let music = state.music.trim();
    if !music.is_empty() {
        match std::fs::read(music) {
            Ok(bytes) => {
                let handle = sources.add(AudioSource { bytes: bytes.into() });
                commands.spawn((
                    EditorAudio,
                    AudioPlayer::<AudioSource>(handle),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.music_volume)),
                ));
            }
            Err(e) => warn!("Song editor: couldn't read background music {music:?}: {e}"),
        }
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

pub(super) fn advance_playhead(time: Res<Time>, mut playhead: ResMut<Playhead>) {
    if playhead.playing {
        playhead.elapsed += time.delta_secs();
        if playhead.elapsed >= playhead.total {
            playhead.playing = false;
        }
    }
}

pub(super) fn update_playhead_view(
    playhead: Res<Playhead>,
    mut line: Query<(&mut Node, &mut Visibility), With<PlayheadLine>>,
) {
    let Ok((mut node, mut vis)) = line.single_mut() else { return };
    if !playhead.playing || playhead.secs_per_tick <= 0.0 {
        *vis = Visibility::Hidden;
        return;
    }
    let cur_tick = playhead.elapsed / playhead.secs_per_tick;
    node.left = Val::Px(cur_tick * TICK_W);
    *vis = Visibility::Inherited;
}

pub(super) fn update_progress_bar(
    playhead: Res<Playhead>,
    mut fills: Query<&mut Node, With<EditorProgressFill>>,
) {
    let p = if playhead.total > 0.0 {
        (playhead.elapsed / playhead.total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for mut node in &mut fills {
        node.width = Val::Percent(p * 100.0);
    }
}
