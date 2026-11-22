# SoundFonts for note detection

These two SF2 banks supply reference recordings for all nine Harmonicon instrument families. All presets below use MIDI bank 0 and zero-based program numbers. Guitar has separate acoustic and electric presets.

**GeneralUser GS 2.0.3** by S. Christian Collins is bundled under the GeneralUser GS License 2.0. The author permits inclusion in software and describes the provenance of the samples in the license. The bundled file matches Git blob `298b552d2e9d1307e03e5c5c99d2c046aaed9ec3`. [Upstream revision](https://github.com/mrbumpy409/GeneralUser-GS/tree/684543d5e5efaef08d02be50dcda8d552478fa60) · [License](GeneralUser-LICENSE.txt)

**Levi’s Cavaquinho** by Levi Diniz is bundled under Creative Commons Attribution 3.0 Unported. The SF2 is the unmodified June 7, 2026 Internet Archive capture of the original download. Its SHA-1 matches the archive index. The current catalog lists a different file hash, so use the pinned capture to reproduce these results. [Author’s listing](https://musical-artifacts.com/artifacts/3205) · [Archived file](https://web.archive.org/web/20260607050956id_/https://musical-artifacts.com/artifacts/3205/Cavaco.sf2) · [License](https://creativecommons.org/licenses/by/3.0/)

| File | SHA-256 |
| --- | --- |
| GeneralUser-GS.sf2 | `9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe` |
| Cavaco.sf2 | `ee55f7407bb94252d069fcd84bf1d3c97625d4edb64da22f049aa91ada46945b` |

## Detector results on rendered exercises

FluidSynth 2.6.2 rendered each preset at 44.1 kHz, with reverb and chorus disabled. Each exercise contains low, middle and high single notes at two velocities. Polyphonic exercises also include chord voicings or double stops. Flute and clarinet use single notes.

The table reports the shipped JIT-MPE detector's frame-level note precision and recall. Exact chord recall is the share of annotated chord frames in which it found every pitch with no extras. Each polyphonic exercise contains 43 or 66 such frames. Short exercises can reveal a broken preset or detector regression. They give limited evidence about live performance.

| Instrument | SF2 preset | Program | Note precision | Note recall | Exact chord recall |
| --- | --- | ---: | ---: | ---: | ---: |
| Harmonica | Harmonica | 22 | 78% | 92% | 70% |
| Acoustic guitar | Steel Guitar | 25 | 71% | 86% | 79% |
| Electric guitar | Clean Guitar | 27 | 76% | 86% | 47% |
| Bass | Finger Bass | 33 | 66% | 87% | 70% |
| Cavaquinho | Cavaco | 0 | 72% | 85% | 84% |
| Piano | Grand Piano | 0 | 69% | 86% | 88% |
| Flute | Flute | 73 | 83% | 94% | — |
| Clarinet | Clarinet | 71 | 79% | 92% | — |
| Violin | Violin | 40 | 77% | 89% | 84% |
| Viola | Viola | 41 | 61% | 87% | 44% |

## Timbre templates on independent recordings

The offline benchmark measures the relative strength of the first eight harmonics for each reference pitch at MIDI velocities 52, 80 and 108. It compares the resulting templates with generic 1/h harmonic weights under the same nonnegative fit, level gate and six-pitch limit. A 4096-sample Hann window advances by 2048 samples. The ground truth is evaluated at the window end with zero timing tolerance.

| Independent audio | Generic exact chord frames | SF2 exact chord frames |
| --- | ---: | ---: |
| VCSL Hohner Special 20, 36 mixtures of recorded blow reeds | 545 / 776 (70.2%) | 589 / 776 (75.9%) |
| GuitarSet, five acoustic-guitar microphone performances | 325 / 3471 (9.4%) | 126 / 3471 (3.6%) |

The harmonica bank improved the fit to recorded reeds. The steel-guitar bank reduced chord recall on actual strumming. These templates are an offline experiment. The recording comparison covers harmonica and acoustic guitar. [VCSL recording source](https://github.com/sgossner/VCSL/tree/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/Aerophones/Free%20Aerophones/Harmonica-Hohner-Special20-C) · [GuitarSet](https://zenodo.org/records/3371780)

## Reproduce

From the repository root, with libfluidsynth, uv and Rust installed, run the two commands below. The first checks every preset by name, measures each semitone in its test range at three velocities, prints generic and SoundFont template scores and writes WAV/chart pairs. The second replays those pairs through the application's detectors.

```sh
OPENBLAS_NUM_THREADS=1 uv run --script scripts/benchmark_soundfonts.py --fixtures /tmp/harmonicon-sf2
cargo run --bin note_bench -- /tmp/harmonicon-sf2 0
```

To include the independent recordings, install ffmpeg and run the following. Both dataset preparation commands download public recordings to the given directories.

```sh
uv run --script scripts/vcsl_chord_benchmark.py /tmp/harmonicon-reeds/special20
python scripts/guitarset_chord_benchmark.py /tmp/harmonicon-guitarset-test
OPENBLAS_NUM_THREADS=1 uv run --script scripts/benchmark_soundfonts.py \
  --recordings harmonica /tmp/harmonicon-reeds \
  --recordings acoustic-guitar /tmp/harmonicon-guitarset-test
```
