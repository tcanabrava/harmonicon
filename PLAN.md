# Plan

Execution order and implementation notes for what's currently in flight.
Companion to `TODO.md` (the open checklist) and `ROADMAP.md` (the
destination). Once a phase ships, its detail belongs to git history, not
this file — prune it back to a one-line summary under "Shipped" below.

## Shipped

Full design detail for anything below lives in `CLAUDE.md` (architecture)
and git history (implementation); this list is intentionally just a
one-line index of what's landed.

- **0.2 "Trustworthy"** — audio-synced clock, chart-derived detection
  range, mic device picker/retry, per-song persistence.
- **0.3 "Practice"** — A–B looping, practice speed, wait-for-note, tab
  display, shuffle metronome, bend trainer progression.
- **0.4, most of it** — adaptive difficulty, jam position/scale overlays,
  the lessons engine + content wave 1, generated 12-bar backing,
  selectable jam progressions/positions, freeform call-and-response in
  Jam Session.
- **Lessons content wave 2** — Units 1–3 (basics extensions, bar-counting/
  train-rhythm drills, blues-vocabulary licks + improvisation), 19 lessons.
- **Lessons content wave 3** — new Unit 4 "Scales and Improvisation" (6
  lessons: major/minor-pentatonic/country scale run drills, major/minor-
  pentatonic open-jam improvisation, quick-change improvisation) plus the
  `JamScale` engine work making Jam Session's live scale feedback
  configurable instead of hardcoded to blues (`docs/lessons_plan.md`).
- **Lessons Unit 5 "jazz", 4 of 5 lessons** — swing-eighths, ii-V-I chord
  tones, jazz-blues form (open jam), chromatic slide basics; all pure
  content, no engine work needed. "Jazz heads" (actual repertoire) is
  deliberately not built — blocked on specific pieces confirmed public
  domain, not just content authoring (`docs/lessons_plan.md`).
- **Genre-selectable Jam Session backing** — a `Genre` axis (Blues/Jazz/
  Rock/Reggae/Country, `jam::backing`) alongside the existing `Progression`,
  each with its own bass rhythm pattern and straight/swing feel; a new
  "Genre" combobox on Generate Jam. Also fixed that page's comboboxes
  getting their open dropdowns clipped (a `ScrollArea` clips to its
  content's own height, not the full window, when content is shorter than
  available space) via a new non-scrolling `spawn_menu_root_plain`.
- **Harmonica rhythm guide + metronome swing-visual fix** — a live pulse
  row in Jam Session (`jam::rhythm_guide`) showing when the picked genre's
  groove wants a harmonica attack, reusing `jam::backing::genre_pattern`'s
  rhythm data directly so it can't drift from the bass audio it's modeled
  on. Along the way, fixed the metronome's visual beat dot to actually
  pulse twice per beat in Shuffle feel (matching the audio click, which
  already did) instead of a single beat-long decay.
- **Circle-of-fifths lesson** — a new `dialogs::circle_of_fifths` diagram
  widget (this crate's first circular UI layout) visualizing that a
  harmonica `Position` is just a step count around the circle of fifths;
  `Position` also grew `Fourth`/`Fifth`/`Twelfth` variants.
- **Circle-of-fifths, live in Jam Session** — `jam::position_guide` adds a
  live position compass (always shown in Jam Session) and a
  `position_cycle` lesson mechanic that calls a new position every 4 bars,
  reusing the existing `ScaleAdherence` scoring against a moving target
  instead of a fixed one; new lesson `circle-of-fifths-jam`.
- **Workspace split, done** — eleven library crates under `crates/`, the
  root package reduced to the binary (`main.rs` + `src/bin/*`, keeping
  `assets/`, `build.rs`, `tests/`, so `cargo run` and packaging are
  unchanged). Layering is now compiler-enforced: a crate cycle isn't
  expressible. `harmonicon-core` is Bevy-free, so pure-logic work iterates
  in seconds. No re-export facades — call sites name the crate they depend
  on. Extractions surfaced several real leaks: widgets whose internals were
  reachable only because a caller ordered against a system by name (now
  `ComboboxEscapeSet`/`MusicVolumeSet`), re-export chains hiding where code
  lived, and a test that found `assets/` only by accident of CWD.
- **Acyclic module graph, enforced** — broke every dependency cycle
  (`settings ↔ audio_system`, and the `{gameplay, jam, menu, song_editor}`
  component covering two thirds of the tree) and added
  `no_module_dependency_cycles` to `tests/physical_design.rs` to keep it
  that way, with no allowlist. Prerequisite for the workspace split, since
  Cargo cannot express a circular crate dependency.
- **Physical-design restructuring** — layering fixes, inline tests evicted
  to `tests.rs` files, `gameplay`/`menu`/`lessons` split into their target
  layouts, `jam` gathered into `harmonicon-jam`, a file-size budget test.
- **Song editor: full authoring tool** — Record/Edit/Play modes with
  live-mic recording, MIDI import, multi-select, copy-paste,
  Select/Erase/Remove/Tempo tools, a real tempo map, selectable
  out-of-scale coloring, lesson authoring alongside plain songs.
- **Song editor: undo/redo, metronome + count-in, pitch audition on
  select, save/validation feedback in the status bar, swing/triplet grid
  snap** — the UX pass that closed out the authoring tool's remaining gaps.
- **Code-duplication cleanup** (whole-tree scan) — shared note-builder/
  glow-step/legend/diagram/MIDI-parsing helpers replacing near-identical
  copies across gameplay 2D/3D, the Song Editor, and menus.
- **Build-time message-registration check** — `build.rs` fails the build
  if a `#[derive(Message)]` type is never registered, instead of only
  surfacing as a runtime panic.
- **Packaging CI fixes** — Flatpak `eu-strip` dependency fix; macOS
  packaging now checked on every push, not just at a release tag.
- **0.5: live auto-refresh of `~/Harmonicon`** — songs/themes/lessons
  dropped into the external folder show up without a manual refresh or
  restart.
- **Options: fullscreen toggle**.
- **Song-progress bar: per-hole note lanes + phrase overlay**, survives a
  song with no backing track.
- **Menu pages auto-scroll** instead of silently overflowing.
- **0.6: jazz engine prerequisites** — ii–V–I chord tables, jazz-blues
  progression. Content authoring is what's left (`TODO.md`).
- **Alternate harmonica tunings** — Paddy Richter, natural minor.
- **Accessibility: colorblind-safe note palette** (Play 2D/3D highway).
- **`phrase_learned` stable keying** — adaptive-difficulty progress keyed
  by phrase name, not track position.
- **Jam Session: MIDI multi-track backing with per-track mute**.
- **Shared music-notation staff** (`harmonicon-ui`'s `music_score/`, Bravura/SMuFL) —
  below the song-progress bar in Play 2D/3D and in the Song Editor.
- **Compact layout for narrow windows** (`harmonicon-platform`'s `responsive.rs`) — Play
  2D/3D and the Song Editor adapt below a shared width breakpoint; menus
  were already scroll-safe and out of scope.
- **Android/iOS prep, desktop-verifiable groundwork** — on-screen
  equivalents for every keyboard-only action found (UI zoom, the Song
  Editor's Delete/Copy/Paste, the spectrogram's style cycle), plus
  `MicStatus::AwaitingPermission` groundwork for a future mobile
  permission-prompt flow.
- **Android port: a real APK builds and runs on an emulator; never on real
  hardware.**
  `contributing/src/android-build.md` is the full record, including exactly what is and isn't
  verified. `packaging/android` (Gradle + cargo-ndk, alongside the existing
  flatpak/macos/windows packaging) emits a signed 147 MB APK whose contents
  were inspected rather than assumed — arm64 cdylib exporting `android_main`
  and `GameActivity_onCreate`, `GameActivity` in `classes.dex`,
  `RECORD_AUDIO` declared, all 186 asset entries present, dev-only
  `debug_songs` excluded — and CI's `android_check` job type-checks the
  target via `cargo ndk`. Landed: `crates/harmonicon-android` (a cdylib
  exporting `android_main`, with the composition root moved to
  `src/lib.rs`'s `run()` so both entry points share it), GameActivity over
  NativeActivity for its IME handling, manifest-backed asset *and lesson*
  discovery for targets whose `assets/` isn't a readable directory, a real
  `RECORD_AUDIO` runtime permission flow over JNI feeding the pre-existing
  `MicStatus::AwaitingPermission`, and the `external://`/`~/Harmonicon`
  paths gated off. It has since been **run on an Android 15 emulator**: it launches, the
  menu renders, assets load out of the APK, and granting RECORD_AUDIO opens
  a 44.1 kHz capture stream. Two bugs were found only by running it — the
  games-activity POM declares no dependencies, so `androidx.appcompat` (its
  own superclass) had to be added explicitly along with an AppCompat theme;
  and `ndk_context` hands back the *Application*, not the Activity, so
  `requestPermissions` threw `NoSuchMethodError`. Still open: **a real
  phone** — nobody has played a harmonica into it, and an emulator says
  nothing about mic latency or AGC — plus **nothing persists on Android**
  (`dirs::config_dir()` is `None`, so progress and settings are lost on
  exit) — plus a touch/hit-target pass, an app icon
  (there is none), arm64-only ABI coverage, and replacing the debug signing
  key. iOS is untouched and needs Xcode; the asset-discovery cfgs
  deliberately exclude it, since an app bundle's Resources directory reads
  like any other. Fixing Android's lesson discovery also fixed **wasm**,
  where the Lessons menu had been silently empty.

## Current work

Finishing 0.4:

1. **Backing track variety, remainder** (0.4): recorded loops per style
   (shuffle, slow blues, swing) as a richer alternative to the generated
   bass — real audio content, not a code task.
2. **Lessons Unit 5 "jazz"** — 4 of 5 lessons shipped (see Shipped above);
   what's left is just "jazz heads", blocked on rights-verified repertoire,
   and it isn't part of finishing 0.4 (`ROADMAP.md`).

No open Song Editor items remain in `TODO.md` — undo/redo, the
metronome/count-in, note audition, save/validation feedback, and the
swing/triplet grid snap are all done (see Shipped above).

**Note detection benchmarking** (see `Harmonica Note Detection Roadmap.md`,
repo root, not checked in): a **harmonica constraint solver**
(`song::harmonica_constraints::plausible_notes` — rejects any simultaneous
blow+draw pitch mix, keeps chords/octaves) and a **synthetic benchmark
dataset generator** (`synthetic_dataset.rs` / `cargo run --bin
gen_synthetic_dataset`, writing into `assets/debug_songs/` in
`note_bench`'s own format, as a stand-in until real recordings exist) are
done — `note_bench` prints a `<algorithm>+HC` row showing the solver's
effect. It reliably cuts NMF's phantom count (e.g. single notes 67→43,
octaves 26→15 in the synthetic benchmark) but occasionally drops a genuine
hit too (its majority-wind-direction heuristic misjudging).

**Deliberately not wired into live gameplay yet** — decided, not just
not-gotten-to:
- Only validated against synthetic (sample-accurate, noiseless) audio; a
  real reed's blow/draw transition frames could get needlessly stripped
  the same way a phantom does.
- A dropped note costs the player score/combo live, while an un-filtered
  phantom is already mostly harmless to scoring (it just doesn't match
  the expected pitch) — so this trade needs real debug recordings to
  confirm it's a net win before it's worth building, since right now
  there's no plumbing at all to thread the active chart's `Harmonica`
  down to where `PitchEvent` is consumed (`audio_system` deliberately
  doesn't depend on `song`).
- When it does go in, scope it to NMF only — FFT/YIN/MPM barely
  benefited in the synthetic benchmark.
- Blocked on: a real harmonica + real debug recordings via
  `song_editor::debug_record` (`--features dev`), then re-running
  `note_bench` against them before revisiting this decision.

3. **Repo-wide comment-shortening pass — done.** Every directory under
   `src/` (`song_editor/`, `gameplay/`, `menu/`, `dialogs/`, `jam/`,
   `audio_system/`, `song/`, `lessons/`, `music_score/`,
   `assets_management/`, `bin/`, `spectrogram/`, and the top-level
   `src/*.rs` files) has been swept file by file for overly long
   `///`/`//!` doc comments, tightening restatement/padding and cutting
   historical narration ("used to be", "the old code") while keeping
   every load-bearing fact (invariants, workaround reasons, cross-
   references). A final full-repo scan turned up nothing left worth
   trimming — remaining dense blocks are genuinely load-bearing technical
   rationale (SMuFL glyph geometry in `music_score/`, psychoacoustic bass
   in `jam/backing.rs`, etc.), not padding. Parallel subagents repeatedly
   hit the session's usage limit mid-run with most work lost (uncommitted
   edits don't survive a killed agent, tried twice); doing this file-by-
   file directly worked reliably. Comment-only edits were committed
   without a build/test/clippy cycle per file, per explicit instruction —
   a final sanity build was still run at the end of the whole pass.

## Road to 1.0 (desktop)

`ROADMAP.md`'s 1.0 section defines the bar and why the scope is desktop
only. Execution order, most valuable first:

1. **First-run flow** (rc1). Nothing exists today — grep for `first_run`/
   `has_seen` and the tree comes back empty. Build it as a state the app
   enters when `profile.json` is absent, not as a flag each screen checks;
   `AppState` already carries a `Startup` variant that currently just falls
   through to `Menu`. Steps chain calibrate → tour → a beginner lesson,
   each skippable, and the whole thing re-runnable from Help / About so
   it's testable without deleting a profile.
2. **Mic trouble where it's noticed** (rc1). `MicStatus::{Failed,
   AwaitingPermission}` and the Options banner already exist; Play 2D/3D
   and Jam Session don't surface either, so a dead mic reads as "the game
   isn't scoring me".
3. **Unwrap triage** (rc2) — **done, and it was almost entirely a
   measurement error.** The "92, 46 in the editor" figure counted
   `song_editor/tests.rs` as production code. The real count outside test
   code is four, all safe; the chart and theme loaders already degrade
   rather than abort. See `ROADMAP.md` for the breakdown and the counting
   pitfall. Still open: a mic unplugged *mid-session*, which nothing has
   exercised.
4. **Blues starter pack** (rc3) — **not to be authored unsupervised**, same
   rights/judgment reasoning as `TODO.md`'s content item. The automatable
   half is validation and difficulty calibration.
5. **Release engineering**: version reconciliation, Flathub, signing keys,
   and the four line-budget splits.

## Bring your own harp, bring your own songs

`ROADMAP.md` has the shape and the reasoning; this is the order. Phase 0
serves both features, so it goes first regardless of which one ships.

### Phase 0 — the pitch↔hole primitive, in a crate that can be reached

`song_editor::pitch_map`'s `map_pitch`/`map_pitch_playable` are the only
inverse mapping in the tree and are `pub(super)` in `harmonicon-editor`,
which sits above gameplay and would be above an importer. Move them into
`harmonicon-core`.

Not a copy-paste: they're written against editor-local `Dir`, `Pitch` and
`HarmonicaKind`, which restate core's own `Action`, `Modifier` and
`Harmonica` variants. The move is a re-expression in core's vocabulary,
returning `(hole, Action, Option<Modifier>)`. The editor then calls the
moved functions and drops its duplicates, or keeps thin adapters if the
diff gets unwieldy — decide when the shape is visible, not now.

Then extend it with **overblow/overdraw**, which it does not do today
(exact match, bends within `max_bend`'s per-hole caps, and the chromatic
slide are all it resolves). This is the piece with actual musical content:
on a Richter harp the overblows sit on holes 1/4/5/6 and overdraws on
7/9/10, and the resulting pitch has to be computed and written into the
note's own `note` field — `Modifier::Overblow` deliberately doesn't imply
it (see its doc comment).

Pure functions, so this phase is testable end to end with no Bevy.

### Phase 1 — play a chart on a different harmonica

1. `EffectiveHarmonica { harp, mapping }` resource, defaulting to the
   chart's own harp with an identity mapping, so nothing changes until
   something sets it.
2. Repoint the reads. 22 `.harmonica` sites in `harmonicon-gameplay`, 10
   in `harmonicon-jam`. Most are display (hole count, labels) and want the
   effective harp too; sort them as you go rather than assuming.
3. **The microphone invariant.** `expected_pitch` (via `target_pitch`),
   `PitchRange` (`frequency_range`) and `ValidHarpNotes`
   (`build_valid_notes`) must all follow the effective harp. Write the test
   first: for a chart played on a non-default harp, every expected pitch
   must be in that harp's own `build_valid_notes()`. That single assertion
   is what stops the game listening for notes the player can't make.
4. The pre-play screen, after song selection and before `SongLoading`.
   Shows what the chart wants (`harp_banner` already renders it), a
   key/tuning picker, the two modes, and **what the choice costs** — how
   many notes need a bend, an overblow, or can't be reached at all. A
   beginner melody quietly turned into an overblow study is a worse outcome
   than "this harp doesn't fit"; the count is what prevents that.

### Phase 2 — `harmonicon-score`

Follow the `add-crate` skill. Bevy-free, between `harmonicon-core` and
`harmonicon-song`.

1. The trait and `ScoreTrack`/`ScoreNote` types, with `.harpchart` as the
   first implementation — the native format going through the same door
   keeps the trait honest.
2. MIDI, mostly re-homing `core::midi_file` (`track_name_of`,
   `extract_notes`, `collect_tempo_map`, `tick_to_seconds`).
3. Track auto-selection: case-insensitive match against `harmonica`,
   `gaita`, `mouth harp`, `blues harp`; otherwise the player picks. The
   Song Editor's existing MIDI track combobox is the model.
4. Conversion — `ScoreFile + track + Harmonica + mapping → HarpChart` —
   reusing Phase 0 rather than growing a second mapper.

### Phase 3 — Guitar Pro, after a spike

**Spike first, design second.** The `guitarpro` crate (MIT, v0.4.3,
codeberg.org/slundi/scorelib) is a candidate only: its gp3/gp4 API is
unverified, and gpx is a different container again — GP6's proprietary
BCFS, GP7's zipped XML — which it may not cover at all. The spike answers
one question: which of gp3/gp4/gp5/gpx can we actually read, and through
whose code.

Expect the honest outcome for many files to be "this song has no
harmonica part". A guitar track mapped onto a harp is mostly unreachable
notes; the name-based lookup is the mitigation, and refusing is better
than emitting a chart nobody can play. Never bundle Guitar Pro
transcriptions — they're copyrighted; opening one from `~/Harmonicon` is
the supported path.

## Post-1.0 (mobile + tooling)

Ordered by value, not effort. All three came out of actually running the
Android build; see `TODO.md` for the full statements. Deliberately after
1.0 — `ROADMAP.md` explains why an unverified mic path shouldn't carry a
1.0 label.

1. **Height-aware `CompactLayout`.** `is_compact` keys on width alone, so
   nothing adapts to a short screen. The Song Editor has bespoke
   workarounds; Play 2D/3D have the same exposure and none. Fix it in
   `responsive.rs` so every screen benefits.

Then, needing hardware: confirm the mic actually captures usably through a
phone, and a touch/hit-target pass. `contributing/src/android-build.md` lists the rest.

## Working practices

- Keep the pure-logic/ECS split: new mechanics get pure functions + unit
  tests first, systems second.
- Update `docs/gameplay_validation.md` whenever a phase adds a mode or
  changes timing behaviour.
- Chart schema changes must stay backward compatible (new fields optional);
  bump `metadata.format_version` when adding any.
- One phase per release; cut a tag when the phase's exit criteria pass —
  none have been cut yet even though 0.2/0.3 are done (see `ROADMAP.md`).
- Prune this file as work lands — a "done" item belongs in git history and,
  if it's an architectural invariant future code must respect, in
  `CLAUDE.md`; it doesn't need to live here too.
