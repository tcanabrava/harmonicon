# Polyphonic Harmonica Detection Plan

## Implemented before real recordings

- Preserve raw detector events for diagnostics.
- Reject pitches the selected harmonica cannot produce.
- Infer one blow/draw family per frame and require two opposing frames before
  changing direction.
- Share one pure `HarmonicaNoteTracker` across gameplay, Song Editor recording,
  and the offline benchmark.
- In gameplay, confirm new pitches over two detector frames and tolerate one
  missing frame before release.
- Keep deterministic synthetic tests for legal chords, impossible mixed-wind
  sets, direction transitions, onset confirmation, and release grace.

## Ready for recordings

Record and annotate single natural notes, bends, overblows/overdraws, adjacent
hole chords, octave splits, tongue-block intervals, blow/draw transitions,
breath-only passages, and room noise. Include several loudnesses, microphone
distances, and at least two microphones where practical.

Run every take through `note_bench`, retaining raw and live-constrained results. Add
exact-set chord precision/recall, per-note precision/recall, direction accuracy,
onset latency, release latency, and per-scenario summaries before tuning.

## Recording-dependent work

1. Add detector strength to `PitchInfo` and replace strongest-first direction
   inference with summed evidence.
2. Tune onset, release, and direction-change hysteresis from measured errors.
3. Learn spectral templates for `(hole, direction, technique)` rather than
   using one ideal harmonic envelope for every chromatic pitch.
4. Fit templates with sparse non-negative reconstruction, including explicit
   broadband-noise and unexplained-residual components.
5. Add a frequency-dependent noise estimate and separate attack and sustain
   thresholds.
6. Accept changes only when the recorded corpus improves without regressing
   exact chord recall or interaction latency.

The first recordings should be treated as a baseline corpus. Keep them fixed
while comparing algorithms; add new takes as separate test groups so tuning to
one player or microphone remains visible.
