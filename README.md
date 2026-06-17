# Harmonicon

A rhythm game for the **diatonic blues harmonica**, built in Rust with the
[Bevy](https://bevyengine.org/) engine. Notes scroll toward a hit line and you
play them on a *real harmonica* — Harmonicon listens to your microphone,
detects the pitches you're playing in real time, and scores you on timing.

It ships with two render modes (a clean 2D lane view and a 3D view with an
animated harmonica model), a free-play **Jam Session** mode over a 12-bar blues
backing, a live audio spectrogram, and a small toolchain for turning MIDI files
into playable charts.

> Status: early/experimental (`0.1.0`), tracking a Bevy `0.19` release
> candidate.

---

## Features

- **Play with a real harmonica.** Microphone input is captured with
  [`cpal`](https://crates.io/crates/cpal) and analysed with an FFT
  ([`rustfft`](https://crates.io/crates/rustfft)) to detect the notes you play
  and grade them as `PERFECT` / `GOOD` / miss.
- **Two gameplay modes:**
  - **2D** — falling notes in ten lanes (one per harmonica hole), with a hit
    line, hole indicators, and a live score/combo HUD.
  - **3D** — the same gameplay rendered around a 3D harmonica model that grooves
    to the beat, with a configurable per-model hole layout.
- **Jam Session** — free play over a rolling 12-bar blues chart and metronome,
  with no falling notes to hit.
- **Scoring system** — perfect/good/miss timing windows, combo multipliers with
  optional decay, and a post-song results screen with hit statistics.
- **Note techniques** — charts can annotate notes with bends, overblows,
  overdraws, vibrato, wah-wah, and holds, shown as on-note badges and a HUD
  legend.
- **Live spectrogram** — a built-in audio visualizer (bar spectrum and
  oscilloscope styles) driven by the same FFT used for scoring.
- **Audio options** — in-game Options page with music and metronome volume
  sliders that affect playback live.
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

### Optional: world inspector

An [`bevy-inspector-egui`](https://crates.io/crates/bevy-inspector-egui) overlay
is available behind a feature flag for debugging the ECS world:

```bash
cargo run --features inspector
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
  menu/                # App states, menu pages, Options (volume sliders)
  gameplay/            # Core gameplay
    gameplay_2d.rs     #   2D lane renderer
    gameplay_3d.rs     #   3D harmonica renderer
    jam_session.rs     #   free-play 12-bar mode
    scoring.rs         #   timing windows, combo/multiplier logic
    results.rs         #   end-of-song results screen
    *_overlay.rs       #   countdown, metronome, phrase, song-progress HUDs
  audio_system/        # Microphone capture (cpal) + FFT pitch detection (rustfft)
  song/                # Chart format, harmonica layouts, asset loader
  spectrogram/         # Audio visualizers (bars, oscilloscope)
  assets_management/   # Font loading, song/harmonica discovery
  bin/
    midi_to_chart.rs   # MIDI → chart converter
    hole_editor.rs     # 3D harmonica hole-layout editor

assets/
  songs/<artist>/<song>/   # chart.harpchart + music.ogg + background/elements art
  harmonicas/3d/<name>/     # harmonica.glb + holes.json (3D model + hole layout)
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
technique modifiers (`bend`, `overblow`, `overdraw`, `vibrato`, `wah-wah`,
`hold`). Charts can also define a `loop` section, scoring windows, and an
`fx_mapping` from modifiers to DSP effects.

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
cargo build      # compile the library, game, and tools
cargo test       # run the unit tests (scoring, timing, MIDI/note conversion, …)
cargo run        # play
```

---

## License

MIT 2.0
