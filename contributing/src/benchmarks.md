# Criterion benchmarks

Tracy shows where one real session spends its time. Criterion answers a
narrower question repeatably: how long one function takes on a fixed
input, and whether a change made it faster. Use it before and after
optimizing anything on a live path.

```bash
cargo bench -p harmonicon-bench --bench detectors
cargo bench -p harmonicon-gameplay --bench judge -- score_frame   # filter by name
```

Run benches without `dev`: that is the build players get, and `dev`
compiles in extra systems and Bevy's debug features. Reports land in
`target/criterion/`, with HTML under `report/index.html`.

## What exists

Each bench's `//!` header says what it measures and why.

| Bench | Measures | Runs |
|---|---|---|
| `harmonicon-bench` `detectors` | `pitch_detect::analyze` per algorithm, warm and fresh `FftState` | every audio hop |
| `harmonicon-bench` `core_hot_paths` | `HarmonicaNoteTracker::update`, `tick_to_seconds`/`seconds_to_tick`, `synth::render_pcm` | every hop / every frame / on demand |
| `harmonicon-bench` `score_import` | MIDI parse, track note extraction, all-track harmonica conversion on two bundled files | opening an imported song |
| `harmonicon-bench` `harp_mapping` | key and harp selection, strict pitch-to-hole mapping | score import / live pitch event |
| `harmonicon-gameplay` `judge` | `score_notes`, `build_scheduled_notes` | every frame / on a note-list rebuild |
| `harmonicon-gameplay` `note_ribbons` | `animate_note_ribbons` with the asset plugin | every frame |
| `harmonicon-jam` `backing` | backing stems, band answer, ending hit | Start / mid-session / session end |
| `harmonicon-ui` `notation` | stem and beam roles, accidentals, tie lookup on short and long scores | staff window rebuild |
| `harmonicon-audio` `waveform` | OGG/WAV decode to peaks, and reduction of existing PCM | song load / editor music change |
| `harmonicon-song` `chart_validation` | JSON parse and cached-schema validation of bundled native charts | opening a `.harpchart` song |
| `harmonicon-bench` `midi_tempo` | MIDI note extraction with 1, 64 or 256 tempo points | opening a tempo-changing MIDI song |
| `harmonicon-gameplay` `adaptive_rebuild` | carry scored note state across a mid-song difficulty change | pause-menu adaptive resync |

`CODE_ANALYSIS.md` records the baseline numbers and what each one means for
the frame budget.

The notation bench uses fixed synthetic scores to track the new chord and tie
work. A quick local run measured `stem_roles` at about 0.17 ms for 256
two-note chords and 1.30 ms for 2,048; `accidentals` took about 0.064 ms and
0.52 ms. These prepare the full score only when the staff window is rebuilt,
not on every playhead move. Re-run with normal Criterion sampling before
using the figures to judge an optimization.

A quick local waveform run measured the short (lesson-loop) OGG fixture at
about 82 ms and the long (full-song) one at about 350 ms, including decoding and peak
extraction. A generated 60-second mono WAV took about 9 ms; reducing its PCM
after decoding took about 1.1 ms. The OGG path runs during asset loading or
on the editor's worker, so these are load costs rather than frame costs.

For harp mapping, a quick run took about 142 µs to resolve 64 pitches. The
full `suggested_harp` search took about 0.9 ms for 256 notes and 7 ms for
2,048 notes. This is an import-time cost; repeat with normal sampling before
comparing an optimization.

Native chart validation is smaller: a quick run on the largest bundled chart
(58 KB) took about 0.26 ms to parse JSON and 0.29 ms to validate and
deserialize the parsed value with the cached schema. This is a load-time
regression check, not a likely startup bottleneck.

MIDI extraction from a synthetic 2,048-note part took about 0.13 ms with one
tempo point and 1.25 ms with 256 tempo points in a quick run. The denser map
costs more because both ends of every note are converted to absolute time.

The adaptive state transfer took about 57 µs for 500 matching notes and
1.83 ms for 3,000; adding a newly unlocked note after every fourth old note
raised the 3,000-note case to about 2.95 ms. This happens on a manual
mid-song adaptive change, not during ordinary frames.

## Where a new bench goes

- **Code in Bevy-free crates goes in `harmonicon-bench/benches/`**, even when
  the function lives in `harmonicon-core`, `harmonicon-dsp` or
  `harmonicon-score`. A dev-dependency is compiled for `cargo test` too, and
  Criterion would slow down the core crates' fast test loop. Pure functions
  in a Bevy crate can stay in that crate's benches, as notation does.
- **ECS code goes in the owning crate's `benches/`**, running the real
  system in a minimal `World` or `App` the way the crate's tests do. A
  bench can only reach public items, so a system it measures has to be
  `pub`.
- Each bench needs a `[[bench]]` entry with `harness = false`.

## Comparing a change

```bash
cargo bench -p <crate> --bench <name> -- --save-baseline before
# make the change
cargo bench -p <crate> --bench <name> -- --baseline before
```

Keep inputs identical across iterations. If the routine mutates state,
rebuild the state in `iter_batched`'s setup closure, and return anything
large from the routine so Criterion drops it outside the timed section.
Otherwise the timing measures the drop of a song-sized `Vec` instead of
the function.

## Limits

- **Main-world CPU only.** A modified material's re-extraction and GPU
  upload happen in the render world, which needs a device. Capture a
  session in [Tracy](profiling.md) for that half.
- **Not timed in CI.** Shared runners are too noisy for results to mean
  anything. `cargo clippy --all-targets` already compiles every bench, so
  a bench cannot silently stop building.
- Numbers are specific to the machine they ran on, and to its power
  state: a laptop on battery in power-save mode ran the synth bench about
  1.7× slower with the same code. Compare baselines taken on the same
  machine in the same power state, not figures copied from
  `CODE_ANALYSIS.md`.
