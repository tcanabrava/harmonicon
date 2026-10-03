# Roadmap

Where Harmonicon goes from `0.1.0`. The theme of the project is **teaching
blues and jazz harmonica through play** — every milestone should move a
self-taught player further than a YouTube tutorial would.

Near-term bug/cleanup work lives in `TODO.md`; the open work, in order,
lives in `PLAN.md`.

**0.2 "Trustworthy" and 0.3 "Practice" are fully shipped** (git history
has the detail). Neither has a tag of its own: releases follow a separate
`v0.0.x` line (newest `v0.0.11`, matching `Cargo.toml`) that never tracked
these phase numbers, and reconciling the two is part of 1.0 below.

## 1.0 — "Ready for strangers" (desktop)

**A readiness gate, not the end of the roadmap.** 1.0 does *not* require
0.5–0.7: those milestones are *more*, and 1.0 is about what a first-time
player hits before any of it matters.

Definition: **a stranger with a C harmonica installs it, gets their
microphone working, and learns something, without hitting a dead end.**

Scope is **desktop only** — Linux, Windows, macOS. Android and wasm are
deliberately post-1.0 (see 0.7+): Android's mic has never been verified on
real hardware, and for this game mic capture *is* the product, so shipping
1.0 on an unverified input path risks the one first impression that can't
be retaken. wasm has no mic at all until cpal gains Web Audio input.

### 1.0-rc1 — the first ten minutes

The first ten minutes decide whether a player with a working harmonica ever
hears the game hear them.

- *(Done)* First-launch detection (absence of `profile.json`) opens a
  welcome page offering microphone setup → guided tour → a first lesson,
  each returning to it when done, every step skippable, and the whole page
  re-runnable from Help / About.
- Surface mic trouble where a confused player is actually looking.
  `MicStatus::{Failed, AwaitingPermission}` and its Options banner already
  exist; nothing shows them on the screens where the silence is noticed.

### 1.0-rc2 — don't lose the player's work — **largely already true**

An earlier draft of this section claimed 92 `.unwrap()`s outside test
modules, 46 of them in `harmonicon-editor`. **That number was wrong**, and
the correction is worth keeping because the same mistake is easy to repeat:
the scan treated any file without a literal `#[cfg(test)]` line as
production code, so it swallowed `song_editor/tests.rs` whole (a pure test
file — `tests.rs` carries its `cfg` on the `mod` declaration, which is why
`physical_design.rs` exempts those files from the line budget too) and also
missed `#[cfg(any(test, feature = "test-support"))]`.

Counted properly — excluding `tests.rs` and stopping at the first `cfg`
gate — it is **four**, and all four are safe:

| | |
|---|---|
| `core/midi_file.rs` | behind `#[cfg(any(test, feature = "test-support"))]`; not in a shipped build |
| `core/note_parser.rs` | `analyze_notes` has no caller outside its own tests — dead `pub` code |
| `dsp/lib.rs` ×2 | guarded by construction: `n < 2` returns early, and `nmf_dict` is assigned in the branch immediately above |

The three degradation paths this section asked for are already in place:
the chart loader is fully `Result`-based over a typed `SongLoadError`
(including schema validation), and the theme loader falls back to defaults
with a `warn!` rather than aborting.

What remains genuinely open, and is *not* yet verified:

- A microphone that disappears **mid-session** (unplugged while playing),
  as opposed to one that fails at startup.
- The nine `.expect(...)` calls are all programmer invariants with real
  messages ("embedded song schema must compile", "checked by caller"), and
  the four `unreachable!()` are enum arms behind prior filtering. Worth a
  read, not a rewrite.

### 1.0-rc3 — content that matches the theme

**No bundled song is blues**, though blues/jazz is the stated theme of the
project. The 11 shipped charts are 3 demos plus public-domain classical and
traditional melodies (Bach, Handel, Beethoven, Greensleeves, …). A blues
starter pack across the four difficulties is the gap.

Authoring it is rights-sensitive and needs real musical judgment, so it
belongs to a person — `TODO.md` already says as much. What *is* automatable
is the supporting work: chart validation, difficulty calibration against the
existing scoring primitives, and Record-mode tooling.

### 1.0 — release engineering

- **Reconcile the version.** `Cargo.toml` has said `0.1.0` through 0.2, 0.3
  and most of 0.4; the tag line ran to `v0.0.9.1` independently.
- Flathub submission — the flatpak workflow already builds.
- Real signing keys in place of the debug ones.
- Clear the four line-budget allowlist entries (`gameplay_2d`,
  `gameplay_3d`, `bending_trainer`, `options`). Cleanup, not a gate.

## Bring your own harp, bring your own songs — shipped

Both features share one primitive, the pitch↔hole mapping, which lives in
`harmonicon_core::pitch_map` (exact notes, bends within each hole's cap,
the chromatic slide, overblows and overdraws) with `harp_remap` on top.

- **Play a chart on the harmonica you actually own.** The harp-check page
  before every song (and jam) takes a key and type and offers *same holes*
  (the tune moves with the harp) or *transpose* (the tab is recomputed),
  saying what the choice costs — bends, overblows, unreachable notes —
  before it's made. `EffectiveHarmonica` is the one source for everything
  the microphone depends on and everything the screen shows about the
  instrument; `harmonicon-gameplay`'s CLAUDE.md states the invariant.
- **Play songs from other formats.** `harmonicon-score` reads MIDI, Guitar
  Pro 3–7, MuseScore and MusicXML behind one `ScoreFile` trait, with the
  native `.harpchart` going through the same door, and picks the harmonica
  track by name (`harmonica`, `gaita`, `mouth harp`, `blues harp`) before
  asking the player. Guitar Pro transcriptions are typically copyrighted:
  fine to open from `~/Harmonicon`, never to bundle.

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

- **Song editor maturity — done.** The happy-path authoring round-trip
  (record → edit → validate → play, without touching JSON) is functionally
  complete (Record/Edit/Play modes, MIDI import with key suggestion, a real
  multi-point tempo map, lesson authoring alongside plain songs), and the
  workflow/UX pass that followed (2026-07-27: undo/redo, metronome/
  count-in, note audition, save/validation feedback, and a swing/triplet-
  aware grid snap) closed out every item it found.
- Downloadable song packs / community sharing for the `~/Harmonicon`
  external-source folder. Live auto-refresh of that folder (songs, themes,
  and lessons) is done. The actual packaging/download/
  hosting mechanism for community song packs is still open — a product
  decision (where packs are hosted, how they're verified) rather than a
  small code task.
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
`docs/lessons_plan.md`'s "Wave 2" section, Unit 4. Its engine prerequisites
(jazz chord-tone tables — `ii_v_i_chords`, `ChordQuality::{Major7,
HalfDiminished7,Dominant7Alt}` — and `Progression::JazzBlues`) are done; what
remains:

- Position work beyond blues 2nd position; chromatic slide technique as a
  first-class taught skill (the `Modifier::Slide` scoring already exists —
  see `CLAUDE.md`).
- Jazz-standard chart content — same rights/judgment-sensitive gap as
  blues content (`TODO.md`), likely worse (jazz standards are more often
  still in copyright); may need to lean on public-domain jazz-blues heads
  and original content rather than standards.

## 0.7+ — "Reach" (platforms & instruments)

- Packaged releases as first-class CI artifacts: Flathub submission, Windows
  installer (workflow exists), macOS bundle (workflow exists — `release.
  yaml` builds/DMGs both architectures at tag time, `macos.yaml` checks the
  same bundling on every push/PR so a regression doesn't wait for a tag).
- Explore web build (Bevy → wasm; mic via Web Audio) for zero-install
  trial. The crate compiles clean for `wasm32-unknown-unknown` (see
  `Cargo.toml`'s wasm32 target section), `trunk build`/`serve`
  (`index.html`, `Trunk.toml`) produces a real, servable bundle, and the
  app now genuinely boots and keeps running in a browser (verified with
  headless Chromium, checked for zero panics across a full run): WGPU
  initializes, localization loads (`harmonicon-platform`'s `localization`'s fixed
  `LOCALES` list, loaded by explicit path instead of `AssetServer::
  load_folder`'s directory scan — the wasm HTTP asset reader can't
  enumerate a directory, which used to hard-panic locale loading on
  startup), mic capture fails gracefully exactly like a real
  permission-less browser would (`MicStatus::Failed`, no panic), and
  bundled songs, note themes, harmonica models, and UI themes now actually
  load: `assets_management`'s directory-scanning discovery is `#[cfg(not(
  target_arch = "wasm32"))]` on native (unchanged — a player can still drop
  a new song into `~/Harmonicon/songs/` with no rebuild)
  and reads a `build.rs`-generated manifest on wasm instead
  (`generate_wasm_asset_manifest`, `$OUT_DIR/asset_manifest.rs` — safe
  because a build script always runs on the native host regardless of
  `--target`, so it can do a real `std::fs::read_dir` walk of `assets/` even
  when building for wasm32); `theme::load_theme`'s own `theme.json` read
  went from raw `std::fs::read_to_string` (can't work over HTTP either) to
  a proper `AssetServer`-loaded `Asset` via a small custom loader
  (`theme::ThemeJsonLoader`, matching `song::loader::SongChartLoader`'s
  pattern). Verified via headless Chromium: the old "No songs directory
  found"/"No note themes directory"/"No harmonica models directory"/
  "Could not find theme.json" warnings are all gone. (The wasm pass also
  surfaced a genuine WebGL2 incompatibility in the themed buttons' smoke
  shaders — a bare `f32` uniform, which WebGL2's downlevel backend
  requires to be 16-byte aligned — resolved by removing that shader
  effect rather than patching it.) What's left: a Web Audio bridge for mic
  capture in place of `cpal`, and a replacement for the `dirs`/`std::fs`-
  based settings/profile persistence and the `~/Harmonicon` external-folder
  watcher (`notify-debouncer-full`), none of which have browser
  equivalents.
- Android: **a real APK builds; it has never been run on a device.** See
  `contributing/src/android-build.md` for the full record and `PLAN.md` for what is still open.
  `packaging/android` (Gradle + cargo-ndk) emits a signed, installable APK
  with verified contents, and CI type-checks the target — so the port can't
  silently rot. It has been run on an emulator — it launches, renders, loads assets from
  the APK and opens a capture stream once RECORD_AUDIO is granted. What's
  left needs a phone in hand: above all confirming **the mic actually
  captures usably**, which for this game is the whole product. Before that
  is worth much, though, **persistence needs fixing** —
  `dirs::config_dir()` is `None` on Android, so lesson progress, scores and
  settings are all lost on exit. Then a touch/hit-target pass, an app
  icon (there is none yet), more than just arm64-v8a, and a real signing key
  in place of the debug one.
  - Mic input is *not* blocked the way it is for
    wasm — checked cpal 0.17.3's own source directly, and both Android
    (`host/aaudio`) and iOS (`host/coreaudio/ios`) have real, non-stub
    input support, unlike the wasm/Web Audio backend, which has none at
    all.
  - What remains genuinely unsettled is the *packaging* toolchain, not the
    code: no single current, actively-maintained tool goes all the way to a
    store-ready build (`cargo-apk`, which the committed metadata targets, is
    explicitly deprecated and **can't produce a Play Store AAB**; the
    community's `bevy_game_template` leans on a custom fork of `xbuild`,
    itself unmaintained upstream) — a Bevy maintainer's own words: shipping
    to mobile is "possible... but not easy." Expect to revisit this choice
    the moment a Play Store release is actually wanted; none of the Rust-side
    work depends on it.
- Explore iOS. Untouched, and it needs a Mac with Xcode, unavailable in this
  dev environment. Not blocked by anything the Android work left behind: the
  asset-discovery `#[cfg]`s deliberately exclude iOS (an app bundle's
  Resources directory reads like any other, so it keeps the runtime scan and
  the drop-folder dynamism), and `permission.rs` is structured to take an iOS
  arm — `NSMicrophoneUsageDescription` needs the same
  ask-then-park-then-poll shape `RECORD_AUDIO` now has.
- Accessibility: mirrored layout for left-handed players, fully
  keyboard-navigable menus. (Colorblind-safe note palettes are done — an
  Options-page toggle swaps the Play2D/Play3D note highway's blow/draw
  colors for a fixed blue/yellow pair; themes can also set their own
  `colors.notes` block. The Song Editor, harmonica overlay legend, and
  song-progress/scrollbar minimaps still use their own hardcoded blue/
  orange, which already reads reasonably under red-green colorblindness —
  extend them to the same toggle if that turns out not to be enough.)
- Localization beyond en-US/pt-BR/es-ES (infrastructure is already enforced).

## Non-goals (for now)

- Multiplayer / online leaderboards.
- Non-harmonica instruments.
