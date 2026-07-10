# Harmonicon

A rhythm game for **blues harmonica** (diatonic and chromatic), built in Rust
with the [Bevy](https://bevyengine.org/) engine. Notes scroll toward a hit line
and you play them on a *real harmonica* — Harmonicon listens to your
microphone, detects the pitches you're playing in real time, and scores you on
timing.

It ships with two render modes (a clean 2D lane view and a 3D view with an
animated harmonica model), a free-play **Jam Session** mode over a 12-bar blues
backing, a **Bending Trainer**, a full in-game **song editor**, a live audio
spectrogram, and a small toolchain for turning MIDI files into playable charts.

> Status: early/experimental (`0.1.0`), tracking Bevy `0.19`.

---

## Features

- **Play with a real harmonica.** Microphone input is captured with
  [`cpal`](https://crates.io/crates/cpal) and analysed in real time with a
  choice of five pitch-detection algorithms (FFT, YIN, pYIN, MPM, NMF —
  selectable in Options) to detect the notes you play and grade them as
  `PERFECT` / `GOOD` / miss.
- **Two gameplay modes:**
  - **2D** — falling notes, one lane per harmonica hole (sized from the
    chart's harmonica — 10-hole diatonic or chromatic), with a hit line, hole
    indicators, and a live score/combo HUD.
  - **3D** — the same gameplay rendered around a 3D harmonica model that grooves
    to the beat, with a configurable per-model hole layout.
- **Jam Session** — free play over a rolling 12-bar blues chart and metronome,
  with a live hole map that highlights chord tones and blues-scale notes per
  bar of the cycle.
- **Practice tools** — A–B section looping (drag a range on the song-progress
  waveform while paused), practice speed (50–100%), wait-for-note mode (the
  chart holds at each note until you play it), and a harmonica tab readout of
  the current phrase.
- **Bending Trainer** — per-hole bend/overblow/overdraw drills with progress
  tracked across sessions.
- **Scoring system** — perfect/good/miss timing windows, combo multipliers with
  optional decay, a post-song results screen with hit statistics and one-click
  latency compensation, and persistent per-song best scores.
- **Note techniques** — charts can annotate notes with bends, overblows,
  overdraws, chromatic slides, vibrato, wah-wah, and holds, shown as on-note
  badges and a HUD legend.
- **Live spectrogram** — a built-in audio visualizer (bar spectrum and
  oscilloscope styles) driven by the same audio pipeline used for scoring.
- **Audio options** — microphone device picker with visible failure/retry,
  pitch-algorithm selection, latency calibration screen, and music/metronome
  volume sliders that affect playback live.
- **Song editor** — author charts in-game (diatonic and chromatic), with
  synthesized preview playback and a practice mode that scores your mic input
  against the chart as you edit.
- **Localization** — English, Portuguese (pt-BR), and Spanish (es-ES).
- **Authoring tools** — `midi-to-chart` converts a MIDI track into a playable
  chart; `hole-editor` positions the clickable holes on a 3D harmonica model.

---

## Requirements

- A recent **Rust** toolchain (Rust 2024 edition; use the latest stable via
  [rustup](https://rustup.rs/)).
- A working **microphone** to play along (the game still runs without one;
  you just won't be able to hit notes).
- Bevy's system dependencies for your platform — see Bevy's
  [setup guide](https://bevyengine.org/learn/quick-start/getting-started/setup/)
  (on Linux you'll typically need ALSA/udev and graphics dev packages).

---

## Running

From the repository root:

```bash
# Play the game
cargo run

# Faster, smoother frame rate (still debuggable thanks to the dev profile tweaks)
cargo run --release
```

The dev profile builds your code at `opt-level = 1` while compiling all
dependencies at `opt-level = 3`, so debug builds are already playable.

### Optional: dev feature

For local iteration, the `dev` feature dynamic-links Bevy and enables its dev
tools and the asset file watcher (never ship a build with it — the dynamic
linking needs Bevy's `.so` alongside the binary):

```bash
cargo run --features dev
```

---

## Controls

| Key      | Action                                         |
| -------- | ---------------------------------------------- |
| `Esc`    | Pause / resume (opens the pause menu)          |
| `M`      | Toggle the metronome click on/off              |
| `V`      | Cycle the spectrogram visualization style      |
| Mouse    | Navigate menus, drag the Options volume sliders |

You play notes by **blowing and drawing on your harmonica** — the detected pitch
is matched against the note currently in the hit window.

---

## How to play

1. Launch the game and pick **Play → Play Song**, then choose a render mode
   (2D or 3D), an artist, and a song. (Or pick **Jam Session** for free play.)
2. A short countdown runs, then the backing track starts and notes begin to
   scroll toward the hit line.
3. Play each note on your harmonica as it reaches the line. Good timing keeps
   your combo and multiplier climbing; missed notes break the combo.
4. When the song ends, a results screen summarizes your perfect/good/delayed/miss
   counts and final score.

---

## Project layout

The crate is split into a library (`src/lib.rs`) so the game binary and the
helper tools can share the same subsystems.

```
src/
  main.rs              # Game entry point: wires up plugins, mic capture, pitch loop
  lib.rs               # Library root, re-exports the subsystems below
  menu/                # App states, menu pages, Options, latency calibration
  gameplay/            # Core gameplay
    gameplay_2d.rs     #   2D lane renderer
    gameplay_3d.rs     #   3D harmonica renderer
    jam_session.rs     #   free-play 12-bar mode
    bending_trainer.rs #   bend/overblow/overdraw drills
    clock.rs           #   the gameplay clock (audio-anchored time authority)
    results.rs         #   end-of-song results screen
    *_overlay.rs       #   countdown, metronome, phrase, song-progress HUDs
  scoring.rs           # Pure scoring math (timing windows, combo/multiplier),
                       #   shared by gameplay and the song editor's practice mode
  song_editor/         # In-game chart editor (grid, playback synth, practice)
  audio_system/        # Microphone capture (cpal) + pitch detection algorithms
  song/                # Chart format, harmonica layouts, asset loader
  spectrogram/         # Audio visualizers (bars, oscilloscope)
  dialogs/             # Shared UI widgets (buttons, tooltips, comboboxes)
  assets_management/   # Font loading, song/harmonica discovery
  profile.rs           # Persistent player progress (best scores, drills)
  settings.rs          # Persistent settings (figment-layered JSON)
  localization.rs      # Fluent localization plumbing
  theme.rs             # Visual theme config
  bin/
    midi_to_chart.rs   # MIDI → chart converter
    hole_editor.rs     # 3D harmonica hole-layout editor
    note_editor.rs     # Visual editor for 2D note layouts

assets/
  songs/<artist>/<song>/   # chart.harpchart + music.ogg + background/elements art
  harmonicas/3d/<name>/     # harmonica.glb + holes.json (3D model + hole layout)
  locales/<locale>/         # Fluent translations (en-US, es-ES, pt-BR)
  midi/                     # source MIDI files
  sounds/                   # metronome clicks
  fonts/  shaders/          # UI fonts and WGSL shaders
  song_schema.dtd.json      # JSON schema charts are validated against
```

---

## Songs & charts

Each song lives under `assets/songs/<artist>/<song>/` and is loaded as a single
`SongManifest` made of:

- `chart.harpchart` — a JSON chart describing tempo, the harmonica layout, and
  the timed track of notes (validated against `assets/song_schema.dtd.json`).
- `music.ogg` — the backing track.
- `background.png` / `elements.png` — per-song artwork.

A chart's `track` is a list of timed items, each with a duration and one or more
note events (hole + `blow`/`draw` + the expected pitch), optionally carrying
technique modifiers (`bend`, `overblow`, `overdraw`, `slide`, `vibrato`,
`wah-wah`, `hold`). Charts declare their harmonica (diatonic or chromatic — the
lane count and overlays adapt), and can also define a `loop` section and scoring
windows. Songs can also be loaded from `~/Harmonicon` outside the bundled
assets.

### Authoring tools

```bash
# List the named tracks inside a MIDI file
cargo run --bin midi-to-chart -- path/to/song.mid

# Convert one track into a chart.hpchart (validated against the schema),
# and write the MIDI back out with that track removed
cargo run --bin midi-to-chart -- path/to/song.mid "Harmonica"

# Edit the clickable hole positions for a 3D harmonica model
cargo run --bin hole-editor
```

`midi-to-chart` maps MIDI pitches onto a standard C richter diatonic harp,
reaching unavailable notes with a draw/blow bend where possible and snapping to
the nearest playable note otherwise.

The `scripts/` directory contains the Python helpers used to generate the 3D
harmonica `.glb` models.

---

## Tech stack

- **[Bevy](https://bevyengine.org/) 0.19** — ECS engine, rendering, UI, audio
- **[cpal](https://crates.io/crates/cpal)** — cross-platform microphone capture
- **[rustfft](https://crates.io/crates/rustfft)** — FFT for pitch detection and
  the spectrogram
- **[serde](https://serde.rs/) / serde_json / jsonschema** — chart parsing and
  validation
- **[midly](https://crates.io/crates/midly)** — MIDI parsing for the chart
  converter

---

## Development

```bash
cargo build                # compile the library, game, and tools
cargo test                 # run the unit tests (scoring, timing, charts, …)
cargo clippy               # keep clean
cargo run --features dev   # local iteration (dynamic linking + asset watcher)
cargo run                  # play
```

---

## License

MIT 2.0
