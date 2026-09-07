# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

Harmonicon: a rhythm game for diatonic and chromatic harmonica (Rust + Bevy
0.19). The player plays a *real* harmonica into the microphone; pitches are
detected in real time and scored against a scrolling chart. Goal: teach
blues/jazz harmonica through play.

Planning docs — keep these current as work lands; prune finished items rather
than accumulating history (git log/commit messages are the historical record):
- `TODO.md` — open, actionable items only
- `ROADMAP.md` — versioned feature direction (0.4 → 0.6+)
- `PLAN.md` — execution order and implementation notes for what's in flight
- `docs/lessons_plan.md` — curriculum design for the Lessons feature
- `docs/gameplay_validation.md` — manual + automated validation checklist;
  update it when changing gameplay/timing behaviour
- `docs/book/` — player-facing mdBook user guide (`mdbook build`/`mdbook
  serve` from that directory); update it when a user-visible feature
  changes, not just internal ones. `docs/book/src/images/*.png` are real
  captures of a running game, taken over BRP
  (`contributing/src/remote-control.md`) — re-take one the same way when a
  screen changes, and keep the filename stable so the
  `![...](images/foo.png)` references throughout `docs/book/src/*.md` don't
  need touching.
- `contributing/` — a *second* mdBook, the contributor architecture guide.
  **Contributor-facing reference lives here, not in `docs/`**: anything a
  contributor would want to *read* (the Android build/run story, the BRP
  remote-control workflow, Tracy profiling) is a chapter in
  `contributing/src/` and gets published. What stays loose in `docs/` is
  planning and process — `lessons_plan.md`, `physical_design_plan.md`,
  `gameplay_validation.md` — which describe unshipped work or internal
  procedure and would read as public promises. Adding a chapter means a
  file in `contributing/src/` **and** an entry in its `SUMMARY.md`; mdBook
  silently ignores a file no `SUMMARY.md` lists, and only files under
  `src/` are included at all.
  16 of its chapters embed ```plantuml blocks (34 diagrams), so building it
  needs a `plantuml` on PATH **and a JRE** — PlantUML is a Java program, and
  without one `mdbook build` aborts the whole book rather than skipping the
  diagram. `book.toml` sets `fail-on-error = true` so a diagram that doesn't
  *parse* fails the build too, rather than publishing a blank space. Keys in
  that `[preprocessor.plantuml]` block are **kebab-case**: the config derives
  `rename_all = "kebab-case"` with `#[serde(default)]`, so a snake_case key
  is silently ignored rather than rejected.

Both books publish to one GitHub Pages site
(`.github/workflows/pages.yaml`): `/guide/` is `docs/book`,
`/architecture/` is `contributing`, and `docs/site/index.html` is the
front door above them. Pages serves one site per repository, which is why
they're subdirectories rather than two deployments. The workflow builds on
every PR touching either book and deploys only on `main`; the URL prefix
is derived from the repo name at build time, so renaming the repository
doesn't break it.

## Commands

```bash
cargo run --features dev,dynamic_linking   # local iteration; ~7s relink
cargo run --release             # playable build; never ship dev/dynamic_linking
cargo test --features dev       # 1106 tests, whole workspace, incl. doctests

# The two loops want different things, which is why dynamic_linking is its
# own feature rather than part of `dev`:
#   - running wants it: it drops a relink after a main.rs edit from ~91s to
#     ~7s, which dominates the edit/run cycle.
#   - testing cannot have it: rustdoc's doctest binary then fails to load
#     the dynamically-linked stdlib ("libstd-*.so: cannot open shared
#     object file") and every doctest errors.
# Never ship either — dynamic_linking needs libbevy_dylib*.so beside the
# binary, which packaged builds (e.g. the flatpak) don't bundle.

# `dev` is not just a speed switch: `#[cfg(feature = "dev")]` modules
# (song_editor::debug_record, the expected-notes layer) are *not compiled
# at all* without it. A bare `cargo test` will happily pass with those
# broken — check the dev build before claiming a change is clean.

# Working on pure logic? Skip the engine entirely — seconds, not a minute:
cargo test -p harmonicon-core   # ~200 tests, no Bevy in its dependency tree
cargo clippy --all-targets -- -D warnings               # what CI runs

# `dev` is not just a speed switch: `#[cfg(feature = "dev")]` modules
# (song_editor::debug_record, the expected-notes layer) are *not compiled
# at all* without it. A bare `cargo test` will happily pass with those
# broken — check the dev build before claiming a change is clean.

# Working on pure logic? Skip the engine entirely — seconds, not a minute:
cargo test -p harmonicon-core   # ~200 tests, no Bevy in its dependency tree
# Debug builds were 300 GB of `target/`. Three settings in Cargo.toml keep
# a clean full build *plus the whole test suite* at 7.1 GB, and the game
# binary at 287 MB (it was 2,410 MB). Each was measured, not guessed:
#
#   [profile.dev.package."*"] debug = false   2410 MB -> 644 MB
#     Dependencies only — Cargo's `"*"` glob excludes workspace members, so
#     Bevy loses its debug info and our own crates keep theirs. 2.16 GB of
#     that original 2.41 GB was `.debug_*`, nearly all Bevy's.
#
#   [profile.dev] debug = "line-tables-only"   644 MB -> 380 MB
#     Keeps `.debug_line`, which is all a backtrace needs to name a file and
#     line (verified with a real panic). Drops the variable/type detail a
#     step debugger wants. `.debug_line` is only 13 MB of the binary; the
#     rest was `.debug_str`/`.debug_info` type names.
#
#   [profile.dev] split-debuginfo = "unpacked" 380 MB -> 287 MB
#     Debug info stays in the object files rather than being copied into
#     every binary that links them, which matters most across the ~50 test
#     binaries. **The binary now depends on those `.o` files staying in
#     `target/`** — copy it elsewhere and backtraces lose their line
#     numbers. Fine for a dev profile; don't carry it into `release`.
#
# Put `debug = true` back on the dependency block to step through Bevy.
#
# Separately: **cargo never garbage-collects `target/`.** Every distinct
# feature set or source state leaves another binary behind and nothing
# removes it — that is how 300 GB happened (99 stale binaries over 1 GB
# each). Cargo has no subcommand for this; `cargo-sweep` does:

# This runs on every commit already — scripts/git-hooks/post-commit, a
# ~0.04s no-op while under the cap. By hand:
cargo sweep --maxsize 20GB   # evict stale artifacts until the rest fits
cargo sweep --time 7         # or by age: anything untouched for a week
cargo sweep --installed      # artifacts from toolchains rustup no longer has
# `--dry-run` on any of them first. Deleting artifacts can only cost rebuild
# time, never correctness — cargo re-derives whatever it still needs.
#
# `--maxsize` caps *accumulation*, not the tree: a freshly built 3.0 GB tree
# under `--maxsize 1GB` cleans nothing, since that is what a current build
# needs. Which is why it is safe on a hook — it can't decide your working
# set is the problem.

# Profiling: start the Tracy UI (https://github.com/wolfpld/tracy), click
# "Connect", then:
cargo run --release --features trace_tracy
```

Binaries: main game (`src/main.rs`), plus `hole-editor`, `note_editor`,
`note_bench`, `gen_synthetic_dataset` (in `src/bin/`). The root package is
the binaries **plus one library**: `src/lib.rs`, the composition root, whose
`run()` assembles every plugin. That library exists because Android never
calls a `main` — it loads a shared object and calls `android_main`
(`crates/harmonicon-android`), so both entry points had to become thin
wrappers around one shared `run()`. Everything *else* still lives in
`crates/`; `src/lib.rs` is assembly only, not logic.
Manual testing needs a mic, audio out, and a display.

## Architecture (load-bearing facts)
- **Cargo workspace — thirteen library crates plus a root package holding the
  binaries and the composition root.** A crate may depend only on ones
  *earlier* in this list, and **peers may not depend on each other**:

  | Crate | Holds | Bevy? |
  |---|---|---|
  | `harmonicon-core` | music theory, chart types, scoring math, pitch/MIDI conversion, pitch→hole resolution (`pitch_map`), the harmonica synth, WAV, grid snapping | **no** |
  | `harmonicon-score` | reading foreign score files (MIDI today, Guitar Pro next) behind one `ScoreFile` trait, and converting a track onto a harmonica | **no** |
  | `harmonicon-dsp` | the five pitch detectors (FFT/YIN/pYIN/MPM/NMF) and their windowing | **no** |
  | `harmonicon-audio` | cpal capture, the ECS wrapper over `harmonicon-dsp`, waveform analysis | yes |
  | `harmonicon-platform` | asset discovery, localization, settings, theme, responsive | yes |
  | `harmonicon-song` | chart/manifest loading, MIDI-backed songs, lessons | yes |
  | `harmonicon-app` | state machine, routing flags, profile | yes |
  | `harmonicon-ui` | `dialogs`, `music_score`, `spectrogram` | yes |
  | `harmonicon-gameplay` | clock, judging, 2D/3D highways, overlays, bend trainer | yes |
  | `harmonicon-jam` / `harmonicon-editor` | Jam Session / Song Editor — **siblings**, neither imports the other | yes |
  | `harmonicon-menu` | page state machine, routing, one file per screen | yes |
  | `harmonicon-bench` | pitch-detection benchmark + dataset generator (dev tooling) | yes |
  | `harmonicon` (root) | `lib.rs` (composition root) + `main.rs` + `src/bin/*`; owns `assets/`, `build.rs`, `tests/` | yes |
  | `harmonicon-android` | `android_main` only — the one crate *above* the root, and the only cdylib | yes |

  - **Keep `harmonicon-core` Bevy-free.** Its whole dependency tree is
    `serde`/`serde_json`/`midly`, which is why its ~200 tests run in
    seconds. Anything needing `Resource`/`Component`/`App` belongs a level
    up. This is the single most valuable property of the split — don't
    trade it away for convenience.
  - **No re-export facades.** A call site names the crate it depends on
    (`harmonicon_core::chart`, `harmonicon_gameplay::gameplay::…`), so every
    dependency is visible where it's taken. Re-exporting a moved module
    under its old path was tried and deliberately removed: it hid which
    crate code came from and let modules reach for things casually.
  - **Cross-crate ordering goes through a `SystemSet`, never a system
    name.** `.after(some_private_fn)` forces the owning crate to make the
    system *and its parameter types* public. `dialogs::combobox::
    ComboboxEscapeSet` and `gameplay::plugin::MusicVolumeSet` exist for
    exactly this — publish an ordering point, keep the implementation
    private.
  - **A crate cycle is not expressible**, so Cargo enforces the layering.
    Rust still permits cyclic *modules* inside one crate, which is what
    `tests/physical_design.rs::no_module_dependency_cycles` catches (no
    allowlist — see `docs/physical_design_plan.md`).
  - **Paths reaching `assets/` from a crate need `../../`.** `include_str!`/
    `include_bytes!` resolve relative to the source file, and
    `env!("CARGO_MANIFEST_DIR")` now points at the crate, not the repo.
    A wrong `include_*!` is a compile error; a wrong runtime path is not,
    so tests that read `assets/` must build the path explicitly rather than
    relying on the working directory.
  - **`assets_management`'s wasm manifest is generated by
    `harmonicon-platform`'s own `build.rs`**, because
    `include!(concat!(env!("OUT_DIR"), …))` reads the *including* crate's
    `OUT_DIR` and `OUT_DIR` is per-package. The workspace-root `build.rs`
    keeps the source-scanning lints (localization, message registration),
    which walk `src/` **and** every `crates/*/src/`.
  - **A new crate must forward the `dev`/`trace_tracy` features** to its own
    `bevy` (`"harmonicon-x/dev"`), or feature unification breaks and the
    tree ends up with two differently-configured Bevy builds.
- **Profiling is Tracy-based** — see `contributing/src/profiling.md` for the whole
  story (why the `LogPlugin` filter is feature-gated, and which paths need
  a manual span because Bevy's per-system instrumentation can't reach
  them). Add a span for anything running off the main schedule (another
  thread, an asset loader, a decode task) or burning real time inside one
  system call.
- **Android's APK builds but has never been run on a device** —
  `contributing/src/android-build.md` has the whole story, and is the file to read before
  touching anything mobile. `packaging/android` (Gradle + cargo-ndk, matching
  `packaging/{flatpak,macos,windows}`) produces a signed installable APK, and
  CI's `android_check` job type-checks the target. Nobody has launched it, and
  **nobody has confirmed a phone mic captures usably** — which for this game
  is the whole product. Four facts that bite:
  - **The GameActivity AAR version is pinned to the Rust crate's vendored
    C++.** `android-activity`'s `GameActivity.h` declares version 4.4.0, so
    Gradle pins `androidx.games:games-activity:4.4.0`. A mismatch aborts at
    *runtime* in `RegisterNatives`, not at build time.
  - **API 28 is a hard floor.** cpal links `libaaudio`, which only exists in
    the NDK sysroot from API 26 up; below it the link fails with a bare
    `unable to find library -laaudio`. `minSdk`, cargo-ndk's `-P` and CI must
    agree. (cargo-ndk spells platform `-P`; `-p` is cargo's package flag.)
  - **The Android-only `bevy` feature selection lives in
    `harmonicon-android`'s own Cargo.toml**, not the root package, so
    `cargo ndk -p harmonicon-android` keeps it. Moving it back silently drops
    the activity backend.
  - **GameActivity compiles C++ from the NDK**, so a plain
    `cargo check --target aarch64-linux-android` fails with `ToolNotFound`;
    it must go through `cargo ndk`. `harmonicon-android` still keeps its
    dependency on the game target-gated, so on every other target it builds
    as an empty cdylib (4.2 MB, zero Bevy symbols) instead of relinking the
    whole app on each desktop `cargo build`.
- **A `--features dev` build serves the Bevy Remote Protocol** on
  `127.0.0.1:15702` (`src/dev_capture.rs`), so a *running* game can be
  inspected, mutated, screenshotted (`target/screenshots/`) and recorded
  (`target/video/`) from a shell with no rebuild — `contributing/src/remote-control.md`
  has the verified request shapes. Never shipped: it is unauthenticated and
  can mutate arbitrary world state, which is why it rides on the
  compile-time `dev` feature rather than a runtime flag. Every image in
  `docs/book/src/images/` was captured this way. Four things that bite:
  - **Only reflected, registered types are reachable** (BRP goes through
    `AppTypeRegistry`). Bevy's own components cover the whole UI tree; this
    codebase's own types need `#[derive(Reflect)]` + `register_type` each,
    which today means `dev_capture::VideoCapture`,
    `NextState<AppState>`/`NextState<MenuPage>`, and
    `bevy_ui_widgets::Activate`.
  - **`NextState` only reaches screens needing no prior selection.** Play
    2D/3D, Jam Session and Results each want a `SelectedSong` first, and
    that holds a `Handle<SongManifest>` — not expressible as a JSON value.
    Those are reached by *clicking*: `world.trigger_event` on `Activate`
    is a remote click on any button, since every click handler here is an
    `On<Activate>` on a real `bevy_ui_widgets::Button` (the rule in the
    table below is what makes this work at all). `dev_capture` registers
    `Activate` for exactly this — `bevy_ui_widgets` derives `Reflect` on
    it but never registers it.
  - **Type paths are exact**: UI text is `bevy_ui::widget::text::Text`, not
    `bevy_text::text::Text` — the latter exists, is registered, and matches
    nothing on a UI node. Likewise the state resource is
    `bevy_state::state::resources::NextState<T>`, not `…::states::`.
  - **BRP cannot see rendering.** It reports the string, so it would never
    have caught the five tofu glyphs a screenshot did. State via BRP,
    appearance via the PNG.
- **Per-crate architecture notes live in `crates/<name>/CLAUDE.md`.** Each
  crate documents its own load-bearing facts; they load when you're working
  in that crate rather than all at once. Start there for anything
  subsystem-specific:

  | Crate | Its `CLAUDE.md` covers |
  |---|---|
  | `harmonicon-core` | (no separate file — pure logic, documented at its `//!` headers) |
  | `harmonicon-score` | (no separate file — the `ScoreFile` trait's contract is at `lib.rs`'s `//!` header; **adding a format means a module there and an arm in `parse_import`, nothing outside the crate**) |
  | `harmonicon-audio` | the cpal→FFT input path, chart-driven detection range |
  | `harmonicon-platform` | asset sources + the `~/Harmonicon` watcher, settings, compact layout |
  | `harmonicon-song` | chart format, schema migration, optional sibling assets, lessons |
  | `harmonicon-app` | `AppState`/`MenuPage` states, profile persistence |
  | `harmonicon-ui` | the Bravura notation staff |
  | `harmonicon-gameplay` | clock/time authority, scoring, adaptive difficulty, HUD, progress bar, call-and-response |
  | `harmonicon-jam` | generated manifests, jam lesson criteria, freeform call-response, MIDI stems |
  | `harmonicon-editor` | the whole Song Editor (record, MIDI import, undo, timeline tools, tempo map) |
  | `harmonicon-menu` | the guided tutorial tour, menu page scrolling |
  | `harmonicon-bench` | benchmark-first pitch-detection workflow |
  | `harmonicon-android` | (no separate file — `contributing/src/android-build.md` covers the whole port) |

## Procedural workflows

Three recurring jobs have their steps (and their traps) written up as
skills in `.claude/skills/`, loaded on demand rather than living here:

- **`add-lesson`** — authoring `assets/lessons/<unit>/<lesson>/`: manifest
  schema, pass criteria, prerequisites, what's honestly scoreable.
- **`add-crate`** — adding or extracting a workspace crate: extract
  bottom-up, forward the features, and what breaks on a move
  (`include_str!` depth, `CARGO_MANIFEST_DIR`, `OUT_DIR`, the budget
  allowlist).
- **`add-locale-string`** — the three-locale parity rule and Fluent's
  `{$var}` syntax.

## Conventions (enforced or established)

- **UI is authored with `bsn!`** wherever applicable; widget callbacks go
  inline as `on(...)` observers — not `Changed<Interaction>` systems, not
  imperative `spawn().observe()`. Prefer `bevy_ui_widgets` over hand-rolled
  widgets; buttons go through the shared `dialogs/button.rs` widget unless
  there's a real reason not to. For a destructive action needing "are you
  sure?", use `dialogs::confirm_dialog` (`OpenConfirmDialog{purpose,
  message}` in, `ConfirmChosen{purpose, confirmed}` out — same
  message-based, `DialogId`-scoped shape as `dialogs::file_dialog`) rather
  than firing immediately or hand-rolling another modal; the Song Editor's
  Erase/Remove timeline tool (`song_editor::timeline`) is its first user.
- **Keyboard navigation:** every interactive element must use a real
  `bevy_ui_widgets` widget (explicit `use bevy::ui_widgets::Button as
  WidgetButton` — plain `bevy::prelude::*` resolves the bare `Button` name
  to `bevy_ui`'s *legacy*, pre-headless-widgets marker instead, which has
  no keyboard support at all) with `TabIndex(0)` attached, never a
  hand-rolled `Pointer<Click>` observer on a plain `Node`. Every screen's
  root needs a `TabGroup` (`bevy::input_focus::tab_navigation`) for
  Tab/Shift+Tab to scope to; a modal (a confirm/file dialog, an open
  combobox dropdown) needs `TabGroup::modal()` or its items' `TabIndex`
  flipped negative while closed (see `dialogs::combobox::
  set_combobox_open`) so Tab can't reach something invisible — bevy's own
  tab-gathering walks the ECS tree by `TabIndex`/`Children` alone, with no
  `Display`/`Visibility` check. `dialogs::keyboard_nav::KeyboardNavPlugin`
  registers `TabNavigationPlugin` (not in `DefaultPlugins`, unlike
  `InputFocusPlugin`) and paints the focus ring; it does **not** bridge
  `Activate` to `Pointer<Click>` — every click handler on a real
  `WidgetButton` is written directly as `On<Activate>` (`Activate` is what
  `bevy_ui_widgets::Button` fires for both a real click and a focused
  Enter/Space, so one handler covers both). An earlier version routed
  `Activate` through a synthetic re-triggered `Pointer<Click>` so ~130
  existing handlers wouldn't need retyping — that bridge could recurse
  into `bevy_ui_widgets`' own `button_on_pointer_click` (which reacts to
  the synthetic click and, seeing the real click's still-`Pressed`
  component, re-emits `Activate`) and overflow the stack on an ordinary
  mouse click. A new button-shaped click handler must therefore take
  `On<Activate>`, not `On<Pointer<Click>>` — and only ever on an entity
  that actually carries a real `WidgetButton`; retyping a handler on a
  legacy `bevy_ui::widget::Button` (or on a plain `Node` with no Button at
  all, e.g. the Song Editor's note-grid drag surfaces or harmonica-diagram
  cells) to `On<Activate>` doesn't fail to compile — it just silently
  never fires, since neither ever emits `Activate`. Multi-option pickers
  (radio-shaped, not "click to advance") should use `RadioGroup`/
  `RadioButton` (see `dialogs::tab_bar`) rather than a hand-rolled
  mutually-exclusive button row — only the group itself gets `TabIndex`,
  per WAI-ARIA convention (individual radio buttons aren't Tab stops; the
  group's own arrow-key handling reaches them). `Checkbox` needs no
  `Button` alongside it — it already has independent Enter/Space and
  click handling that doesn't go through `Activate`, and adding `Button`
  risks a double-toggle (see `dialogs::checkbox.rs`).
- **Bevy 0.19 scene spawning:** use `WorldAssetRoot(handle)` for GLB/scene
  assets, not `SceneRoot`.
- **Localization is enforced:** user-visible strings must come from
  `loc.msg()` (Fluent); a `build.rs` scan + `LocalizedStr` newtype fail the
  build on raw literals. Locales: en-US, pt-BR, es-ES — add keys to all;
  `locales_define_the_same_keys` walks the directory and enforces parity.
  **A key with a variable uses Fluent's own `{$name}` syntax** (e.g.
  `jam-generate-key = Key: {$key}`) — `LocalizationExt::msg_args`
  (`localization.rs`) builds a real `fluent::FluentArgs` from the
  `&[(&str, String)]` it's given and resolves it through
  `fluent_content::Request::args`/`Content::content`, i.e. Fluent's own
  `format_pattern`, not a hand-rolled string replace. One wrinkle:
  Fluent wraps every interpolated argument in bidi-isolation marks
  (FSI/PDI, U+2068/U+2069) by default, meant for prose mixing scripts —
  overkill for short single-language UI labels, and `bevy_fluent`'s
  bundle loader exposes no setting to turn it off
  (`FluentBundle::set_use_isolating` isn't reachable through it), so
  `msg_args` strips those marks from the result via the pure, tested
  `strip_bidi_isolates` helper rather than let invisible formatting
  characters leak into rendered/logged text. `msg` (no args) skips all of
  this — it's a plain `self.content(key)` lookup, no `FluentArgs` involved.
  Runtime *loading* of those locales (`localization::load_locales`) is a
  fixed `LOCALES` list, each loaded by explicit path
  (`locales/<lang>/main.ftl.ron`), not `AssetServer::load_folder` — the
  wasm build's HTTP asset reader can't enumerate a directory
  (`bevy_asset::io::wasm::HttpWasmAssetReader`), and `load_folder` needing
  exactly that is what used to hard-panic the game on startup under wasm
  (`bevy_fluent`'s `LocalizationBuilder::build` indexing an empty map).
  `locales_const_matches_the_assets_directory` keeps `LOCALES` honest
  against what's actually on disk, since nothing else does anymore now
  that nothing scans the directory at runtime. The same directory-listing
  constraint applies to anything else that might run under wasm.
  `assets_management`'s song/note-theme/harmonica-model discovery
  (`scan_all_songs`, `scan_note_themes`, `scan_harmonica_models`,
  `scan_ui_themes`) takes the build-time-manifest approach instead: each is
  now two `#[cfg]`-gated implementations under the same name — the original
  `std::fs::read_dir`-based body, unchanged, behind
  `#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]` (this is
  what keeps native dynamic:
  a player can still drop a new song into `assets/songs/` or
  `~/Harmonicon/songs/` with no rebuild — a fixed manifest like
  localization's `LOCALES` isn't an option here, unlike the fixed set of
  three shipped locales), and a
  `#[cfg(any(target_arch = "wasm32", target_os = "android"))]` sibling
  that reads a `build.rs`-generated manifest instead
  (`generate_bundled_asset_manifest`, `assets_management::manifest`'s
  `include!(concat!(env!("OUT_DIR"), "/asset_manifest.rs"))`). **The cfg
  predicate is "this target's `assets/` is not a readable local directory",
  not "this target is not desktop"** — wasm has no filesystem, and Android's
  assets live inside the APK, reachable only through the JNI
  `AssetManager`; **iOS is deliberately excluded**, because an app bundle's
  Resources directory reads like any other and iOS therefore keeps the
  runtime scan. Generating
  that manifest at build time — rather than runtime — works specifically
  because a build script always runs on the native host and can do a real
  `std::fs::read_dir` walk of `assets/` regardless of the crate's own
  `--target`; `generate_bundled_asset_manifest` mirrors each scan
  function's discovery rule exactly (e.g. the first `*.harpchart` file
  directly under a song's `song/` subfolder) so the two implementations
  can't drift — and its own guard must stay in step with the `#[cfg]` on the
  `manifest` module, since a mismatch is a missing-file build error rather
  than a silent fallback. **Lessons need the same treatment but a different
  manifest**: `lessons::catalog` reads each `lesson.json`'s *bytes* directly
  instead of going through `AssetServer`, so
  `crates/harmonicon-song/build.rs` embeds the JSON text with `include_str!`
  rather than just directory names (and has to be its own build script, for
  the same per-package `OUT_DIR` reason the platform one does). That module
  had no manifest path at all until the Android port added one, so the
  Lessons menu was silently empty on wasm too. The `~/Harmonicon`
  external-folder equivalent has no manifest-backed
  version at all — no home directory concept in a browser, and an Android
  app can only reach its own sandbox — which the
  native functions already handle gracefully (`dirs::home_dir()` returning
  `None`), so nothing target-specific was needed there beyond skipping the
  `external://` asset-source registration in `lib.rs` (on Android
  `AssetSource::get_default_reader` yields the *APK* reader, so registering
  it would silently resolve against the bundle instead of the path given). UI *theme* content
  (not just names) also loads under wasm now: `theme::load_theme` used to
  read `theme.json`'s contents via a raw `std::fs::read_to_string`, which
  can't work under wasm either (a different mechanism than a directory
  listing — an actual file read) — `ThemeJson` is now an `Asset` loaded
  through a small custom `ThemeJsonLoader` (`AssetLoader`, matching
  `song::loader::SongChartLoader`'s pattern; registered by full filename,
  `extensions() -> &["theme.json"]`, so it can't collide with some other
  `.json` asset gaining its own loader later), fetched the same way
  `load_theme` already loaded its sibling images/sounds. Loading is now
  two systems instead of one synchronous function: `request_theme_load`
  (on `SelectedTheme` change) clears `LoadedTheme` and kicks off the
  `asset_server.load::<ThemeJson>(...)` call, stashing the `Handle` in a
  `PendingTheme` resource; `apply_theme_when_loaded` polls it every frame
  (a no-op whenever `PendingTheme` is absent) and populates `LoadedTheme`
  once the load resolves. `theme_source_prefix` (bundled vs.
  `external://`) is also `#[cfg]`-split the same way as the
  `assets_management` scan functions above — its native body does a real
  `Path::is_dir()` check, which needs a real local filesystem wasm doesn't
  have; the wasm sibling always answers "bundled", correct because
  `AvailableThemes` under wasm only ever lists bundled themes to begin
  with (same no-external-folder reasoning as everywhere else here).
- **Message registration is enforced:** `build.rs` also scans for every
  `#[derive(Message)]` type and fails the build if it's never registered
  with `.add_message::<T>()` anywhere — an unregistered message otherwise
  compiles fine and only panics at runtime ("Message not initialized") the
  first time some system's `MessageReader`/`MessageWriter` for it actually
  runs, which can be well after the type was added.
- **Audio synthesis:** vibrato/FM must integrate frequency over time (phase
  accumulation), never `modulated_freq × t` — the latter drifts pitch upward.
- **Testing style:** new mechanics get pure functions + unit tests first,
  ECS systems second. Scoring/chart/pitch logic all have dense test modules —
  match that. ECS behaviour is tested with minimal `World` + `Schedule` or
  `App` + `StatesPlugin` (see `menu/mod.rs`, `gameplay/tests.rs`).
- **Commits:** no `Co-Authored-By` trailer (see "Rules that override
  defaults" below — this one is hook- and CI-enforced). Chart schema changes
  must stay backward compatible (new fields optional); bump
  `metadata.format_version`.

## Rules that override defaults

Most conventions above describe how this codebase already works, so
following them is the path of least resistance. The ones below instead
*contradict* a common default, which means they have to win on every single
occurrence — and that is exactly the kind of rule that erodes silently. The
`Co-Authored-By` one was violated 19 commits in a row before anyone noticed,
because prose is not a check.

| Rule | The default it overrides | Enforced by |
|---|---|---|
| No `Co-Authored-By` trailer on commits | most tooling and assistants add one automatically | `scripts/git-hooks/commit-msg` + the `commit_messages` CI job |
| Click handlers are `On<Activate>`, never `On<Pointer<Click>>` | `Pointer<Click>` is the obvious Bevy reflex, and compiles fine — it just skips keyboard users | `build.rs` (`pointer_click_violations`); a genuinely non-button surface opts out with a `not-a-widget-button:` comment |
| `WorldAssetRoot`, not `SceneRoot`, for GLB/scene assets | `SceneRoot` is what Bevy examples show, and it compiles — it just renders nothing | `build.rs` (`scene_root_violations`) |
| Vibrato integrates frequency over time, never `freq × t` | the naive form looks right and drifts pitch upward | `synth::vibrato_phase_mod` + its boundedness test |
| Every character drawn must be in a bundled font | picking a nice-looking glyph "just works" in an editor and silently draws a box in the game | `tests/glyph_coverage.rs` (reads the fonts' cmaps, not the fallback lists) |
| A release tag must equal `Cargo.toml`'s version | nothing links them, and the packaging reads the *tag* while the About page reads `CARGO_PKG_VERSION` — so they drift silently and only disagree in front of a player | `release.yaml`'s `check_version_matches_tag`, which every release job `needs` |

**If you add a rule to this table, add a check with it.** Anything here
without one is a known liability, not a settled convention — every rule
listed here now has one.

Install the hooks once per clone:

```bash
./scripts/git-hooks/install.sh
```
