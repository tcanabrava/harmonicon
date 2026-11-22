# Polyphonic Harmonica Detection Plan

## Live chord detector

The chord trainer's **Use chord detection** button selects JIT-MPE. The same
detector is available in Options and the editor. It runs the released
S/seed 1 model locally. The immediate presence head scores MIDI 21–108 every
20 ms. Two asymmetric windows use up to 341 ms of past audio. The Rust frontend
scales the windows to the microphone rate and refines active pitches from
spectral peaks for continuous tuning feedback. At 44.1 kHz a regression test
compares calibrated logits against upstream PyTorch. The packed inference
weights take 333 KiB. The export script names the upstream revision.
[JIT-MPE paper](https://arxiv.org/abs/2610.33113) ·
[Released model](https://github.com/Frieve-A/jit-mpe)

Klapuri's iterative harmonic cancellation and Basic Pitch both address
overlapping partials. JIT-MPE provides an explicitly causal inference path
and small pretrained weights, which fit the live capture loop. Harmonicon
uses the immediate head because scoring consumes each pitch event once.
The paper's 50 ms results use a later head.
[Klapuri, ISMIR 2006](https://doi.org/10.5281/zenodo.1416740) ·
[Basic Pitch, ICASSP 2022](https://arxiv.org/abs/2203.09893) ·
[JIT-MPE evaluation](https://arxiv.org/abs/2610.33113)

### Reproduce the microphone comparison

With Python and ffmpeg installed, run these commands from the repository root.

```sh
python scripts/guitarset_chord_benchmark.py /tmp/harmonicon-guitarset-test
cargo run --bin note_bench -- /tmp/harmonicon-guitarset-test 0
```

This replays all five accompaniment tracks from JIT-MPE's GuitarSet test
split (one guitarist, five progressions) against the per-string note
annotations, using the chart's guitar range. The last argument sets timing
tolerance to zero. Each 4096-sample window is scored when it ends.
[GuitarSet v1.1](https://zenodo.org/records/3371780)

| Detector | Note precision | Note recall | Note F1 | Exact chord recall |
| --- | ---: | ---: | ---: | ---: |
| FFT | 54.5% | 74.1% | 62.8% | ~12% |
| NMF | 53.2% | 72.9% | 61.5% | ~10% |
| JIT-MPE | 81.2% | 88.6% | 84.7% | ~42% |

Note metrics pool pitch instances over all frames. Exact chord recall is
the mean of the five tracks' recalls. A frame counts only if every expected
pitch is present and there are no extras. JIT-MPE improved exact recall on
all five recordings, with per-track median note onsets of 36–54 ms.
FFT remains the low-cost global default. On the synthetic C-harp triads,
exact chord recall was 90% for FFT and 85% for JIT-MPE.

A second harmonica check mixed ten publicly recorded Hohner Special 20 reeds.
It covered eight adjacent blow triads, seven octave splits and three four-hole
chords, each at two balances. The one-second mixtures used the soft sustains
from 0.5 seconds into each recording, with equal peak levels or the lowest
reed at 40% amplitude. Exact chord recall was 85% for FFT, 70% for NMF and
53% for JIT-MPE. These mixtures test recorded reed timbre. They do not
capture the interaction of reeds played together in one breath.
[Versilian Community Sample Library recordings](https://github.com/sgossner/VCSL/tree/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/Aerophones/Free%20Aerophones/Harmonica-Hohner-Special20-C)

## Current baseline

- Preserve raw detector events for diagnostics.
- Reject pitches the selected harmonica cannot produce.
- Infer one blow/draw family per frame and require two opposing frames before
  changing direction.
- Share one pure `NoteTracker` across gameplay, Song Editor recording,
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

Run every take through `note_bench`, retaining raw and live-constrained results.
It reports, per recording and detector: notes heard, frame precision/recall,
exact-set chord precision/recall, direction accuracy, and median onset and
release latency, ending with a summary row per recording (one recording per
scenario). Frame recall is diluted by the ±0.25 s timing tolerance — about
55% even for a perfect detector on the synthetic dataset's short notes — so
compare it between runs rather than reading it absolutely.

On the synthetic dataset every detector hears every single note, bend and
overblow; the monophonic ones (YIN, pYIN, MPM) hear none or half of the
chord and octave-split notes, as they must. Onset latency is 25–47 ms raw
and 72–93 ms through the live constraint, which adds a frame of onset
confirmation. Synthetic numbers exercise the pipeline, not the detectors:
the recorded corpus is what tuning is judged against.

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
