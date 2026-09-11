# Harmonicon

A rhythm game for **blues harmonica** (diatonic and chromatic), built in Rust
with the [Bevy](https://bevyengine.org/) engine. Notes scroll toward a hit line
and you play them on a *real harmonica* — Harmonicon listens to your
microphone, detects the pitches you're playing in real time, and scores you on
timing.

It ships with two render modes (a clean 2D lane view and a 3D view with an
animated harmonica model), a free-play **Jam Session** mode over a 12-bar blues
backing, a guided **Lessons** curriculum, a **Bending Trainer**, a full
in-game **song editor**, a live audio spectrogram, and a small toolchain for
turning MIDI files into playable charts.

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
  bar of the cycle. A song authored from a MIDI file with its tracks kept
  separate (rather than pre-mixed) shows a per-track mute row, so you can
  drop out a part and play it yourself.
- **Lessons** — a guided curriculum grouped into units, from first breath
  technique through bends, rhythm, and improvisation, gated by prerequisites
  and tracked per player; some open-ended lessons run as an unscored Jam
  Session judged on scale/chord-tone adherence or phrase discipline instead
  of hit notes.
- **Adaptive difficulty** — an optional per-song setting that starts a chart
  with only the first slice of each phrase live, unlocking more of it as you
  clear it cleanly, with a manual per-phrase override on the pause menu.
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
- **Song editor** — author charts in-game (diatonic and chromatic): place,
  drag, and resize notes on a piano-roll grid with a swing/triplet-aware
  snap mode, multi-select and copy/paste, undo/redo, a metronome with
  count-in, a real variable tempo map, live recording from your own
  playing, MIDI import (pick a track and drop its notes straight onto the
  grid), a practice mode that scores your mic input against the chart as
  you edit, and lesson authoring alongside plain songs.
- **Localization** — English, Portuguese (pt-BR), and Spanish (es-ES).
- **Authoring tools** — `hole-editor` positions the clickable holes on a 3D
  harmonica model.

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

`dev` turns on Bevy's dev tools, the asset file watcher, and a
[Bevy Remote Protocol](contributing/src/remote-control.md) server on `127.0.0.1:15702`
that lets you inspect, screenshot and record a *running* game from a shell.
Never ship it: that server is unauthenticated and can mutate arbitrary world
state, which is exactly why it rides on a compile-time feature.

```bash
cargo run --features dev
```

`dynamic_linking` is separate, and opt-in on top:

```bash
cargo run --features dev,dynamic_linking   # ~7s relink instead of ~91s
```

The two are split because they want opposite things — dynamic linking makes
the edit/run loop far faster, but breaks `cargo test` outright (rustdoc's
doctest binary can't load a dynamically-linked stdlib, so every doctest
fails). Keeping them apart means `cargo test --features dev` runs the whole
suite, doctests included. Ship neither.

### Other platforms

```bash
# Web — see contributing/src/cross-platform-wasm.md
trunk serve --release

# Android — see contributing/src/android-build.md
cd packaging/android && ./gradlew assembleRelease
```

Both build and run. The web build plays but cannot hear you: cpal has no
browser microphone backend yet. The Android APK runs on an emulator and has
now been exercised on real hardware for menu and lesson-tree navigation. Phone
microphone latency, gain, and pitch detection under device audio processing
still need a recorded real-device result.

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

## Contributing setup

Git hooks aren't version-controlled, so install this repo's once per clone:

```bash
./scripts/git-hooks/install.sh
```

That wires up `cargo fmt` on commit and rejects the `Co-Authored-By`
trailer (see `CLAUDE.md`'s "Rules that override defaults"). CI enforces the
trailer rule regardless, so a missing hook fails the PR rather than
landing quietly.

## Project layout

A Cargo workspace: twelve library crates under `crates/`, and a root package
holding the binaries plus the composition root. Each crate may depend only on
ones *earlier* in this list, so the layering is enforced by Cargo rather than
by convention — a circular dependency between crates simply cannot be
expressed.

```
crates/
  harmonicon-core/     # Pure logic, NO Bevy: music theory, chart types, scoring
                       #   math, pitch/MIDI conversion, the harmonica synth, WAV.
                       #   Builds and tests in seconds; keep it engine-free.
  harmonicon-dsp/      # Pure DSP, NO Bevy: the FFT/YIN/pYIN/MPM/NMF pitch
                       #   detectors and their windowing. 33 crates in its
                       #   dependency tree, so it tests in seconds.
  harmonicon-audio/    # Microphone capture (cpal) and the ECS-facing wrapper
                       #   around harmonicon-dsp, plus offline waveform analysis
  harmonicon-platform/ # Asset discovery, Fluent localization, persisted settings,
                       #   visual theme, the narrow-window breakpoint
  harmonicon-song/     # Chart/manifest asset loading, MIDI-backed songs, and the
                       #   lessons curriculum discovered on disk
  harmonicon-app/      # App-wide vocabulary: state machine, routing flags, profile
  harmonicon-ui/       # Reusable widgets (buttons, comboboxes, dialogs, page
                       #   chrome), the SMuFL notation staff, the spectrogram
  harmonicon-gameplay/ # Scored play: audio-anchored clock, note judging, the 2D/3D
                       #   highways, HUD overlays, the Bending Trainer
  harmonicon-jam/      # Jam Session: free play, generated 12-bar backing, improv
                       #   scoring, call-and-response      (sibling of the editor)
  harmonicon-editor/   # The Song Editor: record/edit/play, MIDI import, undo/redo
  harmonicon-menu/     # Page state machine, routing, shared chrome, general pages
  harmonicon-lessons/  # Lesson tree/reader UI and tree layout
  harmonicon-bench/    # Developer tooling: pitch-detection benchmark + synthetic
                       #   dataset generator (not shipped game code)
  harmonicon-android/  # `android_main` only — the one crate *above* the root, and
                       #   the only cdylib. Its dependency on the game is
                       #   target-gated, so off Android it builds empty.

src/                   # Binaries + the composition root
  lib.rs               # Composition root: `run()` registers every plugin. A library
                       #   because Android never calls `main` — it loads a shared
                       #   object and calls `android_main`, so both entry points are
                       #   thin wrappers around one shared `run()`.
  main.rs              # Desktop entry point (three lines: calls `run()`)
  dev_capture.rs       # `--features dev` only: Bevy Remote Protocol server, so a
                       #   running game can be inspected, screenshotted and recorded
                       #   from a shell (contributing/src/remote-control.md)
  bin/
    hole_editor.rs     # 3D harmonica hole-layout editor
    note_editor.rs     # Visual editor for 2D note layouts
    note_bench.rs      # Pitch-detection algorithm benchmark runner

assets/
  songs/<artist>/<song>/         # background/elements art, 2d/3d note layouts
    song/                        #   the chart itself (*.harpchart) + either
                                 #   music.ogg/.wav or music.mid (per-track stems)
  harmonicas/3d/<name>/     # harmonica.glb + holes.json (3D model + hole layout)
  lessons/<unit>/<lesson>/   # lesson.json + its own chart, for the Lessons curriculum
  themes/<name>/             # theme.json + art/sounds for the theme picker
  locales/<locale>/         # Fluent translations (en-US, es-ES, pt-BR)
  midi/                     # source MIDI files for the Song Editor's import tool
  sounds/                   # metronome clicks
  fonts/  shaders/          # UI fonts and WGSL shaders
  song_schema.dtd.json      # JSON schema charts are validated against
  lesson_schema.dtd.json    # JSON schema lessons are validated against
```

---

## Songs & charts

Each song lives under `assets/songs/<artist>/<song>/` and is loaded as a single
`SongManifest` made of:

- `song/<name>.harpchart` — a JSON chart describing tempo, the harmonica
  layout, and the timed track of notes (validated against
  `assets/song_schema.dtd.json`) — any filename, not a fixed name.
- `song/music.ogg` (or `.wav`) — the backing track, or `song/music.mid` to
  keep a MIDI file's tracks separate instead of pre-mixed, so Jam Session
  can play and mute them individually.
- `background.png` / `elements.png` — per-song artwork.

Every one of these except the chart itself is optional — a song can ship
with no art, no backing track, or no separate note layouts, and Harmonicon
fills in a sensible default for whatever's missing.

A chart's `track` is a list of timed items, each with a duration and one or more
note events (hole + `blow`/`draw` + the expected pitch), optionally carrying
technique modifiers (`bend`, `overblow`, `overdraw`, `slide`, `vibrato`,
`wah-wah`, `hold`). Charts declare their harmonica (diatonic or chromatic — the
lane count and overlays adapt), and can also define a `loop` section and scoring
windows. Songs can also be loaded from `~/Harmonicon` outside the bundled
assets.

### Authoring tools

```bash
# Edit the clickable hole positions for a 3D harmonica model
cargo run --bin hole-editor
```

To turn a MIDI file into a chart, use the in-game Song Editor's own MIDI
import instead: pick a `.mid`/`.midi` file, choose a track, and its notes
drop straight onto the grid, auto-mapped onto the best-fitting harp key —
reaching unavailable notes with a bend/slide where possible and snapping to
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
- **[midly](https://crates.io/crates/midly)** — MIDI parsing for the Song
  Editor's import and per-track backing

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
