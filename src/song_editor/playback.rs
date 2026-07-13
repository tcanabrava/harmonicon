// SPDX-License-Identifier: MIT

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::f32::consts::TAU;

use super::state::{Dir, GridNote, HarmonicaKind, Pitch};
// `pub(crate)` re-export: `gameplay::call_response` needs `Expr` to build a
// `PhraseNote` for the call-and-response feature's demo audio.
pub(crate) use super::state::Expr;
use super::{TICK_W, TICKS_PER_BEAT};
use crate::audio_system::midi::{midi_to_freq_hz, note_to_midi};
use crate::settings::AudioSettings;
use crate::song::harmonica::{Harmonica, chromatic_harp, hole_notes, richter_harp};

// ── Constants ────────────────────────────────────────────────────────────────

/// CD-quality mono output used for the in-editor synthesis preview, and by
/// `gameplay::call_response`'s call-phrase demo audio (same synth).
pub(crate) const SAMPLE_RATE: u32 = 44_100;

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
// The LFO rate itself comes from the note's `Expr::Vibrato(hz)` (set per-note
// in the editor); only the depth is a fixed synthesis parameter here.
/// Frequency deviation as a fraction of the base pitch (±1.5 %).
const VIBRATO_DEPTH: f32 = 0.015;

// Hand-wah — player cups/uncovers hands around the harmonica body. The LFO
// rate comes from the note's `Expr::Wah(hz)`; only the closed-hand amplitude
// floor is fixed here.
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
    /// True while playback/practice is frozen mid-song by the Pause button.
    /// Left orthogonal to `playing` (which stays `true` throughout a pause) so
    /// the playhead line's existing `!playing` visibility check keeps showing
    /// it, just not advancing — see `update_playhead_view`.
    pub(super) paused: bool,
    pub(super) elapsed: f32,
    pub(super) total: f32,
    pub(super) secs_per_tick: f32,
}

// ── Pure functions ────────────────────────────────────────────────────────────

/// Builds the synthetic [`Harmonica`] the editor's own `GridNote`s (not a
/// loaded chart's authored layout) are resolved against — a Richter diatonic
/// or 12-hole chromatic, transposed to `key`. Shared with the Bending
/// Trainer via `crate::song::harmonica::{richter_harp, chromatic_harp}`, so
/// both agree on note names, key transposition, and (via [`hole_notes`])
/// which reed an overblow/overdraw actually sounds above.
pub(super) fn build_harp(key: &str, kind: HarmonicaKind) -> Harmonica {
    match kind {
        HarmonicaKind::Diatonic => richter_harp(key),
        HarmonicaKind::Chromatic => chromatic_harp(key),
    }
}

/// `note`'s resolved frequency (Hz) on `harp`, or `None` for a hole/technique
/// combination the harp can't produce (e.g. Overblow requested on a hole
/// outside 1–6). Bend depth is applied as a fractional semitone offset on the
/// natural blow/draw pitch; overblow/overdraw are resolved via
/// [`hole_notes`], which — unlike a flat "+1 semitone from whichever
/// direction the note is tagged with" — knows Overblow sits above the *draw*
/// reed on holes 1/4/5/6 and Overdraw above the *blow* reed on holes 7–10.
pub(super) fn note_freq(note: &GridNote, harp: &Harmonica) -> Option<f32> {
    let action = match note.dir {
        Dir::Blow => crate::song::chart::Action::Blow,
        Dir::Draw => crate::song::chart::Action::Draw,
    };
    let label = match note.pitch {
        Pitch::Normal => harp.wind_direction_label(note.hole, &action),
        Pitch::Slide => harp.slide_label(note.hole, &action),
        Pitch::Overblow | Pitch::Overdraw => hole_notes(harp, note.hole).over?,
        Pitch::Bend(a) => {
            let base = harp.wind_direction_label(note.hole, &action);
            let midi = note_to_midi(&base)?;
            return Some(midi_to_freq_hz(midi as f32 - a));
        }
    };
    Some(midi_to_freq_hz(note_to_midi(&label)? as f32))
}

/// Full harmonic content — sounds bright/open, like uncupped hands.
/// `phase_mod` is an additional phase offset applied to all harmonics; use it
/// for vibrato so the modulation is expressed as bounded phase deviation rather
/// than a drifting frequency × time product.
fn harmonica_wave(freq: f32, t: f32, phase_mod: f32) -> f32 {
    let mut s = 0.0f32;
    for (k, amp) in [
        (1.0f32, P1),
        (2.0, P2),
        (3.0, P3),
        (4.0, P4),
        (5.0, P5),
        (6.0, P6),
    ] {
        s += amp * (TAU * freq * k * t + k * phase_mod).sin();
    }
    s / PARTIALS_SUM
}

/// Muffled version — fundamental only, as if hands fully cup the harmonica.
/// Used as the dark extreme of the hand-wah crossfade.
fn harmonica_wave_muffled(freq: f32, t: f32, phase_mod: f32) -> f32 {
    (TAU * freq * t + phase_mod).sin()
}

pub(super) fn envelope(i: usize, dur: usize) -> f32 {
    let attack = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;
    let release = (SAMPLE_RATE as f32 * RELEASE_SECS) as usize;
    let atk = if attack > 0 && i < attack {
        i as f32 / attack as f32
    } else {
        1.0
    };
    let rel = if dur > release && i > dur - release {
        (dur - i) as f32 / release as f32
    } else {
        1.0
    };
    atk.min(rel).clamp(0.0, 1.0)
}

/// One note to synthesize: a resolved frequency at a `tick`/`len` position on
/// the synth's tick grid (see [`TICKS_PER_BEAT`]), with an optional
/// expression LFO. Decouples [`render_pcm`] from `GridNote`/`Harmonica` so it
/// can render a phrase from *any* source that can resolve a frequency and a
/// tick position — the editor's own notes (via [`note_freq`]) and, sharing
/// this same synth, a chart's call-and-response phrase (via
/// `ScheduledNote::expected_pitch` → `midi_to_freq_hz`, see
/// `gameplay::call_response`). `freq: None` means a hole/technique
/// combination that can't be produced (matching `note_freq`'s convention for
/// a `GridNote`, and `target_pitch`'s for a chart note) — silently skipped,
/// same as before this was factored out.
#[derive(Clone, Copy)]
pub(crate) struct PhraseNote {
    pub(crate) tick: usize,
    pub(crate) len: usize,
    pub(crate) freq: Option<f32>,
    pub(crate) expr: Expr,
}

pub(crate) fn render_pcm(notes: &[PhraseNote], bpm: f32) -> Vec<f32> {
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let end_tick = notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    let total =
        ((end_tick as f32 * secs_per_tick + TAIL_SECS) * SAMPLE_RATE as f32).ceil() as usize;
    let mut buf = vec![0.0f32; total.max(1)];

    let attack_samples = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;

    for (idx, n) in notes.iter().enumerate() {
        let Some(freq) = n.freq else {
            continue;
        };
        let start = (n.tick as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;
        let dur = (n.len as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;

        // Unique per-note LCG seed so each note has an independent breath-noise
        // stream — the slice index stands in for `GridNote::hole` (no longer
        // available here), just as good at telling apart two notes that share
        // a tick (e.g. a chord).
        let mut rng: u32 = LCG_SEED
            .wrapping_add((idx as u32).wrapping_mul(LCG_HOLE_MIX))
            .wrapping_add((n.tick as u32).wrapping_mul(LCG_TICK_MIX));

        for i in 0..dur {
            let s = start + i;
            if s >= buf.len() {
                break;
            }
            let t = i as f32 / SAMPLE_RATE as f32;
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
                Expr::Vibrato(rate) => freq * VIBRATO_DEPTH / rate * (1.0 - (TAU * rate * t).cos()),
                _ => 0.0,
            };

            // ── Hand Wah: amplitude + tone-color modulation ──────────────────
            // `wah_open` oscillates between 0.0 (hands fully cupped = dark,
            // quiet) and 1.0 (hands uncovered = bright, full volume).
            // Amplitude dips toward WAH_AMP_CLOSED when cupped.
            // Tone color is crossfaded from muffled (fundamental only) to the
            // full bright harmonic stack as the hands open.
            let (tone, amp_mod) = if let Expr::Wah(rate) = n.expr {
                let wah_open = ((TAU * rate * t).sin() + 1.0) * 0.5;
                let bright = harmonica_wave(freq, t, 0.0);
                let muffled = harmonica_wave_muffled(freq, t, 0.0);
                let blended = muffled + wah_open * (bright - muffled);
                let amp = WAH_AMP_CLOSED + (1.0 - WAH_AMP_CLOSED) * wah_open;
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
        for x in &mut buf {
            *x /= peak;
        }
    }
    buf
}

/// Re-exported so existing call sites/tests in this module (and
/// `gameplay::call_response`, which shares this whole synth) don't need to
/// change; the real implementation is shared with the Bending Trainer's
/// reference-tone playback.
pub(crate) use crate::audio_system::wav::encode_wav;

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
        let harp = build_harp(&state.key, state.harmonica_kind);
        let phrase: Vec<PhraseNote> = state
            .notes
            .iter()
            .map(|n| PhraseNote {
                tick: n.tick,
                len: n.len,
                freq: note_freq(n, &harp),
                expr: n.expr,
            })
            .collect();
        let wav = encode_wav(&render_pcm(&phrase, bpm), SAMPLE_RATE);
        let handle = sources.add(AudioSource { bytes: wav.into() });
        commands.spawn((
            EditorAudio,
            AudioPlayer::<AudioSource>(handle),
            PlaybackSettings::DESPAWN,
        ));
        let end_tick = state
            .notes
            .iter()
            .map(|n| n.tick + n.len)
            .max()
            .unwrap_or(0);
        *playhead = Playhead {
            playing: true,
            paused: false,
            elapsed: 0.0,
            total: end_tick as f32 * secs_per_tick,
            secs_per_tick,
        };
    }

    let music = state.music.trim();
    if !music.is_empty() {
        match std::fs::read(music) {
            Ok(bytes) => {
                let handle = sources.add(AudioSource {
                    bytes: bytes.into(),
                });
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
    if playhead.playing && !playhead.paused {
        playhead.elapsed += time.delta_secs();
        if playhead.elapsed >= playhead.total {
            playhead.playing = false;
        }
    }
}

/// Toggles pause on the currently running playback/practice: pauses/resumes
/// every editor audio sink and freezes/unfreezes the playhead timer. The
/// playhead line stays visible while paused — only `paused` changes, not
/// `playing` (see the doc comment on [`Playhead::paused`]). A no-op if
/// nothing is currently playing.
pub(super) fn toggle_pause(playhead: &mut Playhead, sinks: &Query<&AudioSink, With<EditorAudio>>) {
    if !playhead.playing {
        return;
    }
    playhead.paused = !playhead.paused;
    for sink in sinks {
        if playhead.paused {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

pub(super) fn update_playhead_view(
    playhead: Res<Playhead>,
    mut line: Query<(&mut Node, &mut Visibility), With<PlayheadLine>>,
) {
    let Ok((mut node, mut vis)) = line.single_mut() else {
        return;
    };
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
