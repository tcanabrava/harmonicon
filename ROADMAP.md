# Roadmap

Where Harmonicon goes from `0.1.0`. The theme of the project is **teaching
blues and jazz harmonica through play** — every milestone should move a
self-taught player further than a YouTube tutorial would.

Near-term bug/cleanup work lives in `TODO.md`; the execution order and
implementation notes live in `PLAN.md`.

**0.2 "Trustworthy" and 0.3 "Practice" are fully shipped** (see `PLAN.md`'s
Shipped section). No release tags have been cut for either yet
(`Cargo.toml` still says `0.1.0`; the newest tag is `v0.0.1.1`); cut one
per phase once its exit criteria pass.

## 0.4 — "Blues school" (curriculum & jam) — in progress

Adaptive difficulty, the jam-session position/scale overlays, the lessons
engine with its first content pass, generated backing tracks, selectable
jam progressions and playing positions (1st/2nd/3rd, picking the matching
cross-harp key), freeform (unscored) call-and-response practice in Jam
Session (`jam::call_response`), and lessons content wave 2 (harmonica
basics, bar-counting drills, train-rhythm chugging, and the
blues-vocabulary Unit 3 — licks via call-and-response, chord-tone/
minor-blues/phrase-discipline improvisation) are all done — see
`docs/lessons_plan.md`. Open:

- **Backing track variety, remainder**: a set of recorded loops per style
  (shuffle, slow blues, swing) as a richer alternative/addition to the
  generated bass.

## 0.5 — "Content" (authoring & ecosystem)

- Song editor maturity: full authoring round-trip (record → edit → validate →
  play) without touching JSON. The editor already has a Practice mode
  scoring live mic input against the chart being edited (`src/scoring.rs` +
  `song_editor/practice.rs`). Its own MIDI import already auto-suggests the
  best-fitting harp key and infers bends/slides for notes the chosen layout
  can't play directly (`song_editor::midi_import::suggest_key`/`map_pitch`).
  The standalone `bin/midi_to_chart` CLI tool (a separate, simpler
  C-diatonic-only converter) has been removed now that the editor's own
  MIDI import covers the same ground in-game; the pure MIDI-parsing helpers
  it used to duplicate (`song::midi`) are shared library code the editor's
  import still builds on.
  - **Variable tempo maps are done**: a chart's `Timing.tempo_map` (multiple
    tempo-change points, not just one flat BPM) is now fully editable —
    `EditorState::tempo_changes` + `state::build_tempo_map`/`bpm_at`, a
    Tempo timeline tool (`TimelineTool::Tempo`, click to add/step a point's
    BPM, click near an existing one to remove it —
    `state::toggle_tempo_point`) rendered as markers on the grid header,
    correct save/load round-tripping through `harpchart.rs` (including
    rescaling a foreign `timing.resolution` — e.g. a MIDI-derived chart —
    into the editor's own tick units), and MIDI import carrying a track's
    real tempo changes into `tempo_changes` instead of collapsing to one
    average BPM. The header waveform (`song_editor::waveform`) aligns
    against this same tempo map, so it stays in sync with the grid as
    tempo points are added. `song::chart::seconds_to_tick` (new, the
    inverse of the existing `tick_to_seconds`) is the shared conversion
    both the editor and MIDI import build on.
    **Deliberate scope boundary**: this covers the editor's grid/waveform
    display and the chart's on-disk tempo map only. Audio *synthesis* for
    Play/Practice/Record preview (`song_editor::playback`'s `render_pcm`,
    shared with `gameplay::call_response`) still renders against one flat
    nominal BPM — the same already-accepted simplification
    `gameplay::call_response` documents for mid-phrase tempo automation.
    Rewriting the synth to follow a variable tempo map is future work if
    it's ever needed; nothing today depends on it.
  - **The editor can also author lessons, not just plain songs**: a
    "Record Song"/"Record Lesson" toggle (`EditorState::content_kind`,
    `song_editor::lesson_form`) switches the meta form to show curriculum
    fields (id, unit, explanation, prerequisites, pass criteria, technique,
    progression) alongside the existing song fields, and Save/Load write or
    read a `lesson.json` instead of a `.harpchart` — with the exact same
    grid/mod-panel/playback underneath, since a chart-backed lesson's chart
    *is* an ordinary chart (written to `song/chart.harpchart` next to the
    manifest, same as every shipped lesson). **Scope boundary**: a
    `lesson.json` only stores Fluent keys, never display text, so the
    editor can't write real pt-BR/es-ES translations for whatever an
    author types — it derives `title_key`/`body_key` from the lesson id
    and prints the key/text pairs to add to the locale files by hand, the
    same manual step authoring any bundled lesson already requires.
- Downloadable song packs / community sharing for the `~/Harmonicon`
  external-source folder. **Live auto-refresh of that folder is done**, for
  songs, themes, *and* lessons: `assets_management::watch` watches
  `~/Harmonicon` recursively (a `notify-debouncer-full` debounced watcher,
  the same crate Bevy's own `file_watcher` feature uses internally) and
  re-scans `songs/`, `themes/`, or `lessons/` the moment any of them
  changes — no manual refresh button, no restart, drop content in and it's
  registered live. Lessons also gained bundled-plus-external scanning
  itself (they were bundled-only before): `~/Harmonicon/lessons` works the
  same way `~/Harmonicon/songs` already did. If the Artist List, Theme
  picker, or Lessons list page happens to be open when a live rescan
  happens, it rebuilds itself immediately too. See `PLAN.md` for the
  implementation shape. The actual packaging/download/hosting mechanism for
  community song packs is still open — a product decision (where packs are
  hosted, how they're verified) rather than a small code task.
- More bundled public-domain songs across all four difficulties (see
  `TODO.md`'s content-gap item).
- Per-technique playback effects (pitch-bend/vibrato/wah DSP driven by chart
  modifiers) — a `fx_mapping`-style chart field was removed as an unbuilt
  stub; reintroduce something like it if/when this gets built.

## 0.6 — "Jazz" (advanced curriculum)

A distinct milestone from 0.4's blues curriculum — bigger scope, split out
because it needs its own content and, likely, chromatic-harmonica-specific
teaching. The lesson-level breakdown (swing-feel drills, ii–V–I arpeggio
lessons, chromatic slide curriculum, jazz-blues form) is in
`docs/lessons_plan.md`'s "Wave 2" section, Unit 4:

- Jazz chord-tone tables (ii–V–I, dominant/altered extensions) alongside
  the existing blues-scale/dominant-7th classification `JamHoleGuide` uses.
- A jazz-blues `Progression` variant (with the ii–V turnaround) for the
  generated jam and the twelve-bar overlay.
- Position work beyond blues 2nd position; chromatic slide technique as a
  first-class taught skill (the `Modifier::Slide` scoring already exists —
  see `CLAUDE.md`).
- Jazz-standard chart content — same rights/judgment-sensitive gap as
  blues content (`TODO.md`), likely worse (jazz standards are more often
  still in copyright); may need to lean on public-domain jazz-blues heads
  and original content rather than standards.

## 0.7+ — "Reach" (platforms & instruments)

- A 3D harmonica prop model for chromatic charts — `Play3D` lane geometry
  already adapts to a chromatic chart's hole count, but no matching mesh
  exists yet, so the bundled diatonic model still renders.
- Alternate tunings beyond Richter/Country (Paddy, natural minor).
- Packaged releases as first-class CI artifacts: Flathub submission, Windows
  installer (workflow exists), macOS bundle (workflow exists — `release.
  yaml` builds/DMGs both architectures at tag time, `macos.yaml` checks the
  same bundling on every push/PR so a regression doesn't wait for a tag).
- Explore web build (Bevy → wasm; mic via Web Audio) for zero-install trial.
- Accessibility: colorblind-safe note palettes, mirrored layout for
  left-handed players, fully keyboard-navigable menus.
- Localization beyond en-US/pt-BR/es-ES (infrastructure is already enforced).

## Non-goals (for now)

- Multiplayer / online leaderboards.
- Non-harmonica instruments.
- Mobile (mic latency + Bevy mobile maturity make this a poor fit today).
