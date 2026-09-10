# Polyphonic Harmonica Detection Plan

## Implemented before real recordings

- Preserve raw detector events for diagnostics.
- Reject pitches the selected harmonica cannot produce.
- Infer one blow/draw family per frame and require two opposing frames before
  changing direction.
- Share one pure `HarmonicaNoteTracker` across gameplay, Song Editor recording,
  and the offline benchmark.
- In gameplay, confirm new pitches over two detector frames (~46 ms at the
  shipped hop size, compensated out of the judged clock) and release them on
  the first frame they go missing.
- Keep deterministic synthetic tests for legal chords, impossible mixed-wind
  sets, direction transitions, onset confirmation, and re-articulation.

The release grace is deliberately off (`release_frames: 1`). It would bridge a
detector that drops a frame mid-sustain — which stops one held breath from
re-arming `AttackGate` and satisfying a second note — but a dropped frame is
indistinguishable from a real re-articulation, and at ~46 ms per frame a grace
of 2 swallows the gap between chugged eighth notes on one hole. Whether
mid-sustain dropouts happen often enough to be worth that is a question for
the corpus below, not for taste.

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
