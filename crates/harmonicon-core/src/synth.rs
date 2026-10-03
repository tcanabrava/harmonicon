// SPDX-License-Identifier: MIT

//! Additive-synthesis harmonica voice shared by every feature that needs to
//! render notes to PCM without a sampled/GM instrument: the Song Editor's
//! Play/Practice preview and MIDI-import backing mixdown
//! (`song_editor::playback`/`midi_import`), and `gameplay::call_response`'s
//! one-shot "call" demo audio. None of those own the synth — it's audio
//! infrastructure, not an editor or gameplay concern.

use std::f32::consts::TAU;

/// CD-quality mono output used by every consumer of this synth.
pub const SAMPLE_RATE: u32 = 44_100;

/// The tick grid every [`PhraseNote`] list is scheduled on — one beat split
/// into this many ticks. Shared vocabulary between the Song Editor's own
/// note grid (`song_editor::TICK_W`'s pixel width, and a chart's exported
/// `timing.resolution`) and `gameplay::call_response`'s phrase-to-tick
/// conversion; both need to agree on the same resolution to build a
/// [`PhraseNote`] list this module's [`render_pcm`] can render correctly.
///
/// 12, not 4: the lowest number divisible by both 4 (straight 16ths) and 3
/// (triplets) — see `song_editor::state::SnapMode`, which needs actual
/// triplet-position ticks on the grid to snap onto (a genuine blues/
/// shuffle feel is a triplet subdivision, not a straight-16th grid). This
/// only changes what resolution the editor writes into *new* saves; a
/// chart's own `timing.resolution` field is self-describing and every
/// tick/time conversion (`song::chart::tick_to_seconds`/`seconds_to_tick`)
/// already takes it as an explicit parameter, so existing bundled charts
/// (still at `resolution: 4`) need no migration.
pub const TICKS_PER_BEAT: usize = 12;

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

/// An expression technique layered on top of a note's pitch. At most one at a
/// time. Both variants carry their oscillation rate in Hz. Shared between the
/// Song Editor's own note model (`song_editor::state::GridNote::expr`, cycled
/// through by repeatedly clicking a mod button) and [`PhraseNote`] — the same
/// per-note tag drives both the editor's live preview and any other phrase
/// this synth renders.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Expr {
    None,
    Wah(f32),
    Vibrato(f32),
}

/// The phase [`harmonica_wave`] takes for `freq` at `t` seconds into a note,
/// plus vibrato's `phase_mod`. Whole cycles are dropped in `f64` first:
/// `TAU·freq·t` reaches tens of thousands of radians in a long note, where
/// an `f32` phase can only step in thousandths of a radian, and every
/// partial multiplies that error by its harmonic number.
fn voice_phase(freq: f32, t: f64, phase_mod: f32) -> f32 {
    TAU * (f64::from(freq) * t).fract() as f32 + phase_mod
}

/// The voice at phase `x = TAU·freq·t + phase_mod`, given `sin x` and
/// `cos x`: `(bright, muffled)`.
///
/// `bright` is the full harmonic stack — sounds open, like uncupped hands —
/// and `muffled` the fundamental alone, as if hands fully cup the harmonica
/// (the dark extreme of the hand-wah crossfade). `phase_mod` belongs inside
/// `x` so vibrato is a bounded phase deviation shared by every partial,
/// rather than a drifting frequency × time product.
///
/// Every partial is `sin(k·x)`, so they come from the one sine/cosine pair
/// by the Chebyshev recurrence `sin((k+1)x) = 2·cos(x)·sin(kx) −
/// sin((k−1)x)` instead of six `sin` calls — this runs per sample for every
/// note the synth renders, including a whole song when the Song Editor
/// plays. The pair comes from an [`Oscillator`] for a steady note and from
/// `voice_phase(..).sin_cos()` under vibrato.
fn harmonica_wave(sin_x: f32, cos_x: f32) -> (f32, f32) {
    const AMPS: [f32; 6] = [P1, P2, P3, P4, P5, P6];
    let two_cos = 2.0 * cos_x;
    let (mut previous, mut current) = (0.0f32, sin_x);
    let mut sum = AMPS[0] * sin_x;
    for amp in &AMPS[1..] {
        let next = two_cos * current - previous;
        previous = current;
        current = next;
        sum += amp * next;
    }
    (sum / PARTIALS_SUM, sin_x)
}

/// Samples between exact re-seeds of an [`Oscillator`]. Rotation rounding
/// grows by about one `f32` epsilon per step, so 1024 steps stay near 1e-4,
/// about −80 dB below the signal.
const OSCILLATOR_RESEED_SAMPLES: usize = 1024;

/// `sin`/`cos` of a steady tone's phase, sample by sample, by rotating the
/// previous pair through one sample's phase step: two multiply-adds instead
/// of a `sin_cos` per sample. Re-seeded from [`voice_phase`] every
/// [`OSCILLATOR_RESEED_SAMPLES`] so rounding cannot accumulate over a long
/// note. Only for a constant frequency; vibrato's phase is not a fixed step.
struct Oscillator {
    freq: f32,
    step_sin: f32,
    step_cos: f32,
    sin: f32,
    cos: f32,
}

impl Oscillator {
    fn new(freq: f32) -> Self {
        let (step_sin, step_cos) = (TAU * freq / SAMPLE_RATE as f32).sin_cos();
        Self { freq, step_sin, step_cos, sin: 0.0, cos: 1.0 }
    }

    /// `(sin, cos)` of the phase at sample `i` of the note. Must be called
    /// for `i = 0, 1, 2, …` in order.
    fn next(&mut self, i: usize) -> (f32, f32) {
        if i.is_multiple_of(OSCILLATOR_RESEED_SAMPLES) {
            (self.sin, self.cos) =
                voice_phase(self.freq, i as f64 / f64::from(SAMPLE_RATE), 0.0).sin_cos();
        }
        let current = (self.sin, self.cos);
        (self.sin, self.cos) = (
            self.sin * self.step_cos + self.cos * self.step_sin,
            self.cos * self.step_cos - self.sin * self.step_sin,
        );
        current
    }
}

pub fn envelope(i: usize, dur: usize) -> f32 {
    let attack = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;
    let release = (SAMPLE_RATE as f32 * RELEASE_SECS) as usize;
    let atk = if attack > 0 && i < attack { i as f32 / attack as f32 } else { 1.0 };
    let rel =
        if dur > release && i > dur - release { (dur - i) as f32 / release as f32 } else { 1.0 };
    atk.min(rel).clamp(0.0, 1.0)
}

/// One note to synthesize: a resolved frequency at a `tick`/`len` position on
/// a caller-chosen tick grid, with an optional expression LFO. Decouples
/// [`render_pcm`] from any particular note-source type so it can render a
/// phrase from *any* source that can resolve a frequency and a tick position
/// — the Song Editor's own notes (via `song_editor::playback::note_freq`)
/// and, sharing this same synth, a chart's call-and-response phrase (via
/// `ScheduledNote::expected_pitch` → `midi_to_freq_hz`, see
/// `gameplay::call_response`). `freq: None` means a hole/technique
/// combination that can't be produced — silently skipped.
#[derive(Clone, Copy)]
pub struct PhraseNote {
    pub tick: usize,
    pub len: usize,
    pub freq: Option<f32>,
    pub expr: Expr,
}

/// Vibrato's phase deviation Δφ at time `t` — the integral of the
/// instantaneous frequency, *not* a modulated frequency multiplied by `t`.
///
/// With f(t) = freq · (1 + depth · sin(2π·rate·t)), integrating gives
/// φ(t) = 2π·freq·t + (freq·depth/rate)·(1 − cos(2π·rate·t)), and this is
/// that second term. It is **bounded** — it oscillates in
/// `0 ..= 2·freq·depth/rate` forever — which is the whole point: the naive
/// `modulated_freq × t` form grows without bound and slides the pitch
/// sharp over a long note. See CLAUDE.md's "Rules that override defaults".
pub fn vibrato_phase_mod(freq: f32, rate: f32, t: f32) -> f32 {
    freq * VIBRATO_DEPTH / rate * (1.0 - (TAU * rate * t).cos())
}

/// The largest phase deviation [`vibrato_phase_mod`] may ever reach.
pub fn vibrato_phase_bound(freq: f32, rate: f32) -> f32 {
    2.0 * freq * VIBRATO_DEPTH / rate
}

pub fn render_pcm(notes: &[PhraseNote], secs_per_tick: f32) -> Vec<f32> {
    let end_tick = notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    let total =
        ((end_tick as f32 * secs_per_tick + TAIL_SECS) * SAMPLE_RATE as f32).ceil() as usize;
    let mut buf = vec![0.0f32; total.max(1)];

    let attack_samples = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;
    // One sample's worth of the breath noise's exponential decay. Multiplying
    // by it each sample replaces an `exp` per sample.
    let noise_decay_step = (BREATH_NOISE_DECAY / SAMPLE_RATE as f32).exp();

    for (idx, n) in notes.iter().enumerate() {
        let Some(freq) = n.freq else {
            continue;
        };
        let start = (n.tick as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;
        let dur = (n.len as f32 * secs_per_tick * SAMPLE_RATE as f32) as usize;

        // Unique per-note LCG seed so each note has an independent breath-noise
        // stream — the slice index stands in for a per-note identity (e.g.
        // `GridNote::hole`, not available here), just as good at telling apart
        // two notes that share a tick (e.g. a chord).
        let mut rng: u32 = LCG_SEED
            .wrapping_add((idx as u32).wrapping_mul(LCG_HOLE_MIX))
            .wrapping_add((n.tick as u32).wrapping_mul(LCG_TICK_MIX));
        let mut noise_env = 1.0f32;
        let mut oscillator = Oscillator::new(freq);

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
            let (sin_x, cos_x) = match n.expr {
                Expr::Vibrato(rate) => voice_phase(
                    freq,
                    i as f64 / f64::from(SAMPLE_RATE),
                    vibrato_phase_mod(freq, rate, t),
                )
                .sin_cos(),
                _ => oscillator.next(i),
            };

            // ── Hand Wah: amplitude + tone-color modulation ──────────────────
            // `wah_open` oscillates between 0.0 (hands fully cupped = dark,
            // quiet) and 1.0 (hands uncovered = bright, full volume).
            // Amplitude dips toward WAH_AMP_CLOSED when cupped.
            // Tone color is crossfaded from muffled (fundamental only) to the
            // full bright harmonic stack as the hands open.
            let (bright, muffled) = harmonica_wave(sin_x, cos_x);
            let (tone, amp_mod) = if let Expr::Wah(rate) = n.expr {
                let wah_open = ((TAU * rate * t).sin() + 1.0) * 0.5;
                let blended = muffled + wah_open * (bright - muffled);
                let amp = WAH_AMP_CLOSED + (1.0 - WAH_AMP_CLOSED) * wah_open;
                (blended, amp)
            } else {
                (bright, 1.0)
            };

            // ── Breath noise ─────────────────────────────────────────────────
            rng = rng.wrapping_mul(LCG_MUL).wrapping_add(LCG_INC);
            let noise_sample = (rng as i32) as f32 / i32::MAX as f32;
            // Full level through the attack, then e^(DECAY·t) from its end.
            let breath = noise_sample * BREATH_NOISE_AMP * noise_env;
            if i >= attack_samples {
                noise_env *= noise_decay_step;
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the FM rule in CLAUDE.md's "Rules that override defaults":
    /// vibrato must integrate frequency over time, never multiply a
    /// modulated frequency by `t`.
    ///
    /// Both forms look and sound close over a short note — the naive one's
    /// error grows with `t`, so it only shows up as the pitch sliding
    /// sharp across a long one. The invariant that separates them is
    /// boundedness: the correct phase deviation oscillates inside a fixed
    /// window forever, the naive one grows without limit.
    #[test]
    fn vibrato_phase_deviation_stays_bounded_however_long_the_note() {
        let (freq, rate) = (440.0, 5.0);
        let bound = vibrato_phase_bound(freq, rate);
        // Ten seconds is far longer than any charted note; the naive form
        // would be an order of magnitude past the bound well before here.
        for i in 0..100_000 {
            let t = i as f32 / 10_000.0;
            let phase = vibrato_phase_mod(freq, rate, t);
            assert!(
                (0.0..=bound + 1e-3).contains(&phase),
                "phase deviation {phase} escaped 0..={bound} at t={t}s — \
                 vibrato must integrate frequency over time"
            );
        }
    }

    #[test]
    fn vibrato_phase_deviation_returns_to_zero_every_lfo_cycle() {
        // A wobble around a steady centre, not a one-way slide: Δφ is back
        // at zero after each full LFO period, so successive cycles start
        // from the same pitch.
        let (freq, rate) = (440.0, 5.0);
        for cycle in 0..20 {
            let t = cycle as f32 / rate;
            assert!(vibrato_phase_mod(freq, rate, t).abs() < 1e-3);
        }
    }

    /// The six-`sin` form the recurrence replaced: each partial evaluated
    /// directly.
    fn reference_wave(freq: f32, t: f32, phase_mod: f32) -> (f32, f32) {
        let mut s = 0.0f32;
        for (k, amp) in [(1.0f32, P1), (2.0, P2), (3.0, P3), (4.0, P4), (5.0, P5), (6.0, P6)] {
            s += amp * (TAU * freq * k * t + k * phase_mod).sin();
        }
        (s / PARTIALS_SUM, (TAU * freq * t + phase_mod).sin())
    }

    #[test]
    fn the_recurrence_is_at_least_as_accurate_as_evaluating_every_partial() {
        // Both f32 forms round their large phase arguments, so neither is
        // exact over a long note; judge each against a double-precision
        // rendering of the same partials instead of against each other.
        // Across the harmonica's range, well past any charted note length,
        // with and without vibrato's phase deviation.
        let truth = |freq: f32, t: f64, phase_mod: f32| {
            let x = std::f64::consts::TAU * f64::from(freq) * t + f64::from(phase_mod);
            let amps = [P1, P2, P3, P4, P5, P6];
            let sum: f64 = (1..=6).map(|k| f64::from(amps[k - 1]) * (k as f64 * x).sin()).sum();
            (sum / f64::from(PARTIALS_SUM), x.sin())
        };
        let (mut new_worst, mut old_worst) = (0.0f64, 0.0f64);
        for freq in [130.0f32, 262.0, 523.0, 1047.0, 2093.0] {
            for i in (0..4 * SAMPLE_RATE as usize).step_by(7) {
                // What `render_pcm` does: f64 time for the phase, f32 for the
                // rest. The old form only ever had the f32 time.
                let t64 = i as f64 / f64::from(SAMPLE_RATE);
                let t = i as f32 / SAMPLE_RATE as f32;
                for phase_mod in [0.0, vibrato_phase_mod(freq, 5.5, t)] {
                    let error = |(bright, muffled): (f32, f32), (want_b, want_m): (f64, f64)| {
                        (f64::from(bright) - want_b).abs().max((f64::from(muffled) - want_m).abs())
                    };
                    let (sin_x, cos_x) = voice_phase(freq, t64, phase_mod).sin_cos();
                    new_worst = new_worst
                        .max(error(harmonica_wave(sin_x, cos_x), truth(freq, t64, phase_mod)));
                    old_worst = old_worst.max(error(
                        reference_wave(freq, t, phase_mod),
                        truth(freq, f64::from(t), phase_mod),
                    ));
                }
            }
        }
        assert!(
            new_worst <= old_worst,
            "recurrence error {new_worst} exceeds the direct form's {old_worst}"
        );
        // And far better than it, now that the phase is reduced first.
        assert!(new_worst < 1e-4, "recurrence error {new_worst}");
    }

    #[test]
    fn the_oscillator_tracks_the_exact_phase_over_a_long_note() {
        // Rotation rounding must stay bounded however long the note, and the
        // re-seed is what bounds it: without one, a minute of samples drifts
        // far past this tolerance. Judged against a double-precision phase,
        // like the recurrence test above.
        let mut worst = 0.0f64;
        for freq in [130.0f32, 262.0, 523.0, 1047.0, 2093.0] {
            let mut oscillator = Oscillator::new(freq);
            for i in 0..60 * SAMPLE_RATE as usize {
                let (sin, cos) = oscillator.next(i);
                let x =
                    std::f64::consts::TAU * f64::from(freq) * (i as f64 / f64::from(SAMPLE_RATE));
                worst = worst
                    .max((f64::from(sin) - x.sin()).abs())
                    .max((f64::from(cos) - x.cos()).abs());
            }
        }
        assert!(worst < 1e-4, "oscillator error {worst}");
    }

    #[test]
    fn vibrato_actually_modulates_rather_than_holding_a_flat_pitch() {
        // The drift test above would also pass if vibrato did nothing at
        // all, so pin that the modulation is really there: a vibrato note
        // and a plain one must not be sample-identical.
        let note =
            |expr| render_pcm(&[PhraseNote { tick: 0, len: 500, freq: Some(440.0), expr }], 0.001);
        let plain = note(Expr::None);
        let vib = note(Expr::Vibrato(5.0));
        let diff = plain.iter().zip(&vib).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(diff > 0.01, "vibrato left the waveform unchanged");
    }
}
