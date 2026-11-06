# Code review plan

A correctness-first review of the whole workspace, **one crate at a time**,
bottom-up in dependency order. This is not another size/duplication pass —
`code-reduction-review.md` and `structural-code-reduction-review.md` cover
that. The question here is: *what is wrong, fragile, or unguarded?*

## Why bottom-up

A crate may only depend on crates earlier in the workspace list, so
reviewing in that order means every API a crate calls has already been read.
Findings in a lower crate (a wrong invariant in `chart`, a panic in a
parser) often explain symptoms higher up; going top-down would rediscover
them as "weird call sites".

## Method (every crate)

1. **Read first, build later.** Start with the crate's `CLAUDE.md` (where
   one exists) and `lib.rs`'s `//!` header, then read the source. Reviewing
   is a reading task; cargo only runs to verify a fix.
2. **Apply the cross-cutting checklist** below, then the crate-specific
   focus list.
3. **Classify each finding**:
   - **Bug** — wrong behaviour reachable today (with the input/state that
     triggers it).
   - **Hazard** — correct today, but an invariant nothing checks (the kind
     of rule that belongs in the "Rules that override defaults" table).
   - **Gap** — missing test for logic that has real branches.
   - **Cleanup** — dead code, stale docs, misleading names.
4. **Act**: small in-scope Bugs/Gaps/Cleanups are fixed in the same pass,
   verified (`cargo test -p <crate> --features dev` where the crate has
   the feature, `cargo clippy -p <crate> --all-targets --no-deps -- -D
   warnings`), and committed as one commit per crate (or per session for
   the split crates). Anything design-sized becomes a line in `PLAN.md` /
   `TODO.md` naming this doc.
5. **Record** a short findings list under the crate's heading below
   (finding → fixed in `<sha>` / moved to PLAN.md / won't fix + why), and
   mark the crate done. Prune the details once fixes land; the commits are
   the history.

### Cross-cutting checklist

- **Panics on external input**: `unwrap`/`expect`/indexing/slicing/integer
  overflow/`as` truncation on anything from a file, the network, the mic,
  or a user. Charts, packs, score files, settings and profiles are all
  external input.
- **Float hazards**: NaN/∞ propagation, division by zero durations or
  tempos, `partial_cmp().unwrap()`, `f32` equality.
- **Time**: frame-rate dependence (`Time::delta` vs the gameplay clock),
  wall-clock vs audio-clock mixing, accumulating rounding error.
- **ECS**: system ordering ambiguities, `Changed<…>` filters that miss the
  first frame, state-scoped entities that leak across states, resources
  read before insertion, unregistered messages (build.rs catches
  `#[derive(Message)]`, not everything).
- **Persistence**: non-atomic writes, a corrupt/old file bricking startup
  instead of falling back, schema changes without `format_version` bumps.
- **UI conventions** (CLAUDE.md): `bsn!` children separated by `--`,
  `On<Activate>` only on real `WidgetButton`s, `TabIndex`/`TabGroup`
  coverage, modals unreachable when closed, every string through `loc.msg`.
- **Platform cfg pairs**: each `#[cfg(any(wasm32, android))]` sibling
  matches its native twin and `build.rs`'s manifest generation.
- **Tests that can't fail**: assertions that pass vacuously (empty
  iterators, `if let` skipping the assert, fixtures that aren't loaded).
- **Docs vs code**: `CLAUDE.md`, `//!` headers and `contributing/` chapters
  that describe behaviour the code no longer has.

## Order and scope

Sizes are production code lines from `scripts/loc_report.py`
(2026-11-05). Roughly 4–5k code lines fits one session; the two largest
crates are split by module.

Several crates carry **uncommitted WIP** at the time of writing (core
`instrument.rs`, platform, song lessons, lessons, menu, editor
`lesson_form.rs`). Review those files only once that work is committed,
or explicitly against `HEAD` — don't review a moving target.

| # | Crate | Code | Sessions |
|---|---|---:|---:|
| 1 | `harmonicon-core` | 4,731 | 1 |
| 2 | `harmonicon-dsp` | 554 | ½ |
| 3 | `harmonicon-score` | 1,056 | ½ |
| 4 | `harmonicon-packs` | 543 | ½ |
| 5 | `harmonicon-bench` | 844 | ½ |
| 6 | `harmonicon-audio` | 556 | ½ |
| 7 | `harmonicon-platform` | 2,826 | 1 |
| 8 | `harmonicon-song` | 1,684 | ½ |
| 9 | `harmonicon-app` | 395 | ½ |
| 10 | `harmonicon-ui` | 4,509 | 1 |
| 11 | `harmonicon-gameplay` | 12,174 | 4 |
| 12 | `harmonicon-jam` | 3,399 | 1 |
| 13 | `harmonicon-editor` | 11,863 | 4 |
| 14 | `harmonicon-menu` | 4,502 | 1 |
| 15 | `harmonicon-lessons` | 2,087 | ½ |
| 16 | root + `harmonicon-android` | 1,386 | ½ |

### 1. `harmonicon-core`

- Still Bevy-free: `cargo tree -p harmonicon-core` shows only
  serde/serde_json/midly (plus whatever is deliberately there).
- `chart.rs`: every new field optional, `format_version` bumped with each
  schema change, deserialising the oldest supported chart still works.
- `instrument.rs` / `harmonica.rs` / `harmonica_constraints.rs`: tuning
  tables, octave/register edges, chromatic slide, guitar/flute ranges;
  `u8` MIDI arithmetic near 0 and 127.
- `pitch_map.rs`: pitch→hole ambiguity and tie ordering.
- `scoring`: zero-length notes, overlapping notes, perfect/miss boundaries
  are inclusive/exclusive consistently.
- `synth.rs`: vibrato uses `vibrato_phase_mod` everywhere; no `freq × t`.
- WAV/MIDI: malformed input returns `Err`, never panics.

### 2. `harmonicon-dsp`

- The five detectors against `docs/pitch_detection_plan.md`: which
  constants are measured vs placeholder, and whether the code says so.
- Silence, clipping, DC offset, buffer shorter than the window.
- Allocation in the per-frame path; window functions computed once.

### 3. `harmonicon-score`

- Untrusted binary parsers (Guitar Pro 3–7, MIDI) and XML (MusicXML,
  MuseScore): bounds checks, length fields trusted for allocation size,
  recursion depth, overflow in tick→time conversion.
- `guitar_pro` tab timing (read its `//!` header first): tuplets, ties,
  tempo changes mid-track.
- `convert`: what happens to out-of-range pitches on harmonica/guitar.
- Adding a format still needs only a module + a `parse_import` arm.

### 4. `harmonicon-packs`

- `git.rs`: URL scheme allowlist, clone target path confinement (no `..`
  or absolute paths from config), partial-clone cleanup on failure, the
  re-clone-instead-of-fetch update path.
- `pack.rs`: version-compatibility rules for every comparison direction.
- `retained_songs.rs`: never deletes a player's own data.

### 5. `harmonicon-bench`

- Light pass: does the synthetic dataset generator produce what the
  benchmark assumes; are results reproducible (seeded RNG).

### 6. `harmonicon-audio`

- cpal callback thread: no locks/allocation that can glitch, bounded
  channel behaviour when the main thread stalls, device loss/reconnect.
- `permission.rs` on each platform; chart-driven detection range updates
  when the chart (or instrument) changes mid-session.

### 7. `harmonicon-platform`

- `assets_management`: each cfg-split scan matches build.rs's manifest
  rule exactly.
- `settings.rs`/`theme.rs`: corrupt or old files fall back to defaults;
  writes are atomic; `PendingTheme` can't get stuck.
- `localization`: `msg_args` bidi stripping, missing-key behaviour,
  pack-provided locales (`localization/packs.rs`) not shadowing UI keys.
- `content_packs.rs`/`content_sync.rs`: background task results applied
  on the right state; failure surfaces to the player.

### 8. `harmonicon-song`

- `song/loader.rs`: schema migration, optional sibling assets missing,
  `external://` vs bundled resolution.
- `lessons/`: manifest validation (`validate.rs`) agrees with what
  `catalog.rs`/`graph.rs` assume — prerequisite cycles, dangling ids,
  `LESSON_FORMAT_VERSION` gating; `build.rs` embedding for wasm.

### 9. `harmonicon-app`

- `AppState`/`MenuPage` transition graph: unreachable or dead-end states.
- `profile.rs`: persistence and migration of practice history/streak
  (date math around midnight/time zones).

### 10. `harmonicon-ui`

- `dialogs/`: TabGroup/TabIndex correctness for every modal and the
  combobox; `confirm_dialog`/`file_dialog` `DialogId` scoping (two open
  dialogs can't answer each other).
- `music_score/notation.rs`: glyphs all in Bravura (glyph_coverage test
  covers them?), meter/tempo maps at bar edges.
- `spectrogram`: per-frame cost, texture reallocation.

### 11. `harmonicon-gameplay` (4 sessions)

- **a. Timing and judging** — `clock`, `judge`, `notes`, `state`,
  `lifecycle`, `plugin`, `autoplay`, `countdown_overlay`,
  `wait_freeze_overlay`. One time authority; pause/resume/seek/loop keep
  the clock and audio in step; judgment windows at tempo changes.
- **b. Scoring feedback** — `adaptive_difficulty`, `coaching`,
  `technique_coach`, `technique_cue`, `note_feedback`, `practice_badges`,
  `results`, `call_response`, `karaoke`.
- **c. Highways** — `gameplay_2d`, `highway_2d`, `highway_axis`,
  `note_ribbon*`, `bars`, `beat_guides`, `guitar_visuals`,
  `flute_visuals`, `piano_visuals`, `harmonica_overlay`. Entity churn per frame, despawn on exit,
  layout at 1920×1080 (the floor).
- **d. Trainers and chrome** — `note_trainer*`, `bending_trainer/`,
  `hud*`, `pause_menu`, `pause_score`, `song_progress_overlay`,
  `phrase_overlay`, `metronome_overlay`, `modifier_legend`,
  `warning_banner`, `twelve_bar_blues_overlay`, `song_info`.
- Also: `tests.rs` exercises the paths above, and
  `docs/gameplay_validation.md` matches current behaviour.

### 12. `harmonicon-jam`

- No import of `harmonicon-editor` (sibling rule).
- `backing.rs` (1.4k lines): generated-manifest correctness per genre/key,
  MIDI stem timing; `session.rs` lifecycle; call-and-response phrase
  generation determinism and edge cases (empty phrase, key without notes).

### 13. `harmonicon-editor` (4 sessions)

Highest data-loss risk in the codebase — prioritise save/load.

- **a. Document and persistence** — `state`, `undo`, `harpchart`,
  `instrument_chart`, `note_model`, `document_picker`, `save_feedback`,
  `metadata_sync`, `clipboard`. Round-trip losslessness (unknown fields
  kept), undo covers every mutation, save failure is visible.
- **b. Timeline editing** — `grid`, `grid_cache`, `interaction`,
  `timeline`, `timeline_overlay`, `scroll`, `view_scroll`, `note_motion`,
  `ranges`, `repeat_marks`, `annotation_lane`, `phrase_editor`.
- **c. Import and transforms** — `midi_import`, `transpose/`,
  `pitch_map`, `*_note.rs`, `guitar_setup`, `music_score_bridge`.
- **d. Audio and panels** — `record`, `debug_record` (dev-only: check
  the `--features dev` build), `playback`, `audition`, `metronome`,
  `practice`, `waveform`, `panel*`, `meta_form`, `details_fields`,
  `lesson_form`, `loop_settings`, `scoring_settings`, `mod_panel`,
  `legend`, `instrument_ui`, `ui`, `material`.

### 14. `harmonicon-menu`

- `routing.rs`: every page reachable and escapable (Esc/back).
- `pages/artist_list.rs` (1.8k lines), `calibration`, `harp_check`,
  `content_sources`, `tutorial` (tour targets still exist).
- Keyboard navigation on every page root.

### 15. `harmonicon-lessons`

- Tree layout/placement/transition: pure functions tested; layouts with
  one node, disconnected nodes, wide fan-out.
- Reader widgets vs the lesson schema (`assets/lesson_schema.dtd.json`).

### 16. Root package + `harmonicon-android`

- `lib.rs` is assembly only; plugin order matches the `SystemSet`s
  crates publish.
- `build.rs` lints: can each be fooled (macros, `use … as`, line breaks)?
  Each needs a test proving it fires.
- `src/bin/*`: `validate-pack`, `port-part` error paths and exit codes.
- `tests/`: `physical_design`, `shader_compile`, `asset_layout`,
  `glyph_coverage` still guard what CLAUDE.md says they guard.

## Findings

Add a `### <crate>` heading here as each review completes.

### 1. `harmonicon-core` — done 2026-11-05

Still Bevy-free (`midly`, `serde`, `serde_json` only).

- **Bug, fixed `9878f171`**: nothing checked a tempo map's order, and
  `tick_to_seconds`/`seconds_to_tick` subtract neighbouring `u64` ticks.
  An out-of-order point panicked a debug build. Now
  `chart_check::timing_problems`, plus saturating arithmetic.
- **Bug, fixed `9878f171`**: `migrate_chart_json` stamped the current
  version over a malformed `format_version` before validation, so neither
  the schema's pattern nor `format_version_supported` could reject one.
- **Bug, fixed `98c70b29`**: `render_pcm` sized its buffer from the last
  note with no bound, and `ticks_per_quarter` accepted 0. A corrupt
  `song/music.mid` could overflow or exhaust memory while loading.
- **Cleanup, fixed**: `note_parser` was dead (no caller outside its own
  tests) and could panic on non-UTF-8 input; deleted. `lib.rs` described
  re-export facades the project rules forbid.
- **Hazard, open**: `LaneLayout::span` recognises a piano layout by
  shape (a vertical axis and every label equal to its pitch's note name)
  rather than by being told. Today only `keyboard_rows` matches; a
  future vertical pitch-row layout would silently get white-key spacing.
  A `LaneLayout` kind field would make it explicit.
- Checked and sound: scoring windows and combo maths, repeat expansion
  (passes clamped, overlaps dropped), WAV parsing (checked throughout),
  atomic config writes, every 1-based hole/string lookup
  (`checked_sub(1)`), pitch arithmetic near the `u8` edges.

Carried into the song session (8): `song/mod.rs` still re-exports
`chart`, `harmonica`, `harmonica_constraints` and `midi_file` as
`song::*`, against the no-facades rule. `SongManifest::waveform` is
computed at every song load and no longer read by anything.

### 2. `harmonicon-dsp` — done 2026-11-05

- **Hazard, fixed**: a `PitchRange` with a zero `min_freq` overflowed
  YIN/MPM's lag arithmetic and sent NMF's dictionary builder into
  billions of columns. Unreachable today (every range comes from
  instrument pitches through `from_freqs`), but both are now guarded:
  `from_freqs` drops non-positive/non-finite input, YIN and MPM share
  one checked `lag_bounds`, and NMF clamps its notes to MIDI's range.
- **Docs, fixed**: `contributing/src/audio-pipeline.md` called FFT the
  only polyphonic detector, described pYIN as weighting lags rather than
  thresholds, and NMF's dictionary as harmonica spectra limited to the
  harp's notes; it is synthetic templates for every chromatic note in
  range.
- **Open, needs the corpus**: `suppress_harmonics` only drops a peak that
  is a multiple of a *stronger* one. A fundamental weaker than its own
  second harmonic, common low on a harmonica, is reported alongside its
  octave as two notes. Whether that happens on real input, and how often,
  is a question for the recorded corpus in `docs/pitch_detection_plan.md`,
  not a constant to change blind.
- Checked and sound: silence and short-input gates, bin bounds in FFT
  peak-picking, buffer reuse across chunks, NaN handling in MPM's peak
  scan and NMF's ranking.

### 3. `harmonicon-score` — done 2026-11-05

The binary and XML parsing is the `guitarpro` crate's (0.4.4 in the
lock); this crate adapts its model. Findings:

- **Bug, fixed**: `guitarpro` trusts size fields it reads. A `.gpx`
  whose inner file claims a negative size widens it with `as usize` and
  panics in `Vec::with_capacity`, one changed field in a valid file, and
  this runs in an asset loader. `GpScore::parse` now runs the third-party
  read under `catch_unwind` (no profile sets `panic = "abort"`), with a
  test built from the crate's own `pack_bcfs`/`compress_bcfz`. **Worth
  reporting upstream**: `io/gpx.rs`'s `read_le_i32(...) as usize`.
- **Bug, fixed**: `.mscz`/`.mxl`/`.gp` are zips, and both `guitarpro`
  and `unzip_mxl` read an entry whole with no bound; a small archive can
  inflate to gigabytes, an abort `catch_unwind` can't catch. Entries over
  64 MiB unpacked are refused before parsing, and `unzip_mxl` reads
  through `take`.
- **Bug, fixed**: converted charts took the source tempo with only
  `.max(1.0)`; a MIDI tempo of 0 µs/quarter is an infinite BPM, and the
  schema allows 20–300. Clamped.
- **Efficiency, fixed**: `GpScore::notes` linear-scanned every onset for
  every beat; now a binary search. File-derived tick arithmetic
  (`Beat::start`, measure origins) saturates.
- Checked and sound: track picking, harpchart adapter, MIDI adapter
  (core's `midi_file` already refuses a 0-tick header), the zero-numerator
  meter (downstream parses `0/4` as 4/4), fret/string lookups.

### 4. `harmonicon-packs` — done 2026-11-06

- **Bug, fixed `93823b8c`**: an address with nothing after its scheme
  (`https://`) passed `validate` and slugged to `""`, and
  `packs_dir.join("")` is `packs_dir`. Removing such an entry from Content
  Sources ran `remove_dir_all(packs_dir)`, deleting every pack.
  `RepoSpec::checkout_dir` now builds every pack path and refuses an empty
  slug.
- **Hazard, open**: `retained_songs::has_chart` keeps its own list of
  chart extensions, a copy of `harmonicon_score::IMPORT_EXTENSIONS` plus
  `harpchart`. A format added there and not here makes a removed song in
  that format be deleted rather than kept. The crate can't take the score
  crate as a dependency cheaply (it brings `guitarpro`); a test in a crate
  that sees both would hold them together.
- **Hazard, open**: retained songs accumulate in the checkout with no
  bound, and every update counts them against `MAX_PACK_BYTES`. Once they
  cross 1 GiB, every later update fails with "larger than 1024 MB" and
  nothing says why or how to recover.
- **Hazard, open**: `MAX_PACK_BYTES` is checked after the clone, so a
  hostile or mistaken repository can fill the disk before it's refused.
  Only reachable through an address the player chose.
- Checked and sound: scheme allowlist (`ext::`, `http` refused), slug
  sanitizing, symlink refusal, staged swap with rollback, compatibility
  rules (pre-releases match their release), corrupt `installed.json`.

### 5. `harmonicon-bench` — done 2026-11-06

Light pass, as planned. Datasets are deterministic (the core synth's
noise is a fixed-seed LCG) and `GAP_SECS` is held above twice the timing
tolerance by a compile-time assert. Only a docs fix: two references to
`Harmonicon Note Detection Roadmap.md` misspelled it as "Harmonica…",
and one called it not checked in.

### 6. `harmonicon-audio` — done 2026-11-06

No bugs. The capture callback is allocation-free (proved by a counting
allocator in its tests), every channel is bounded with `try_send`, a
device dying mid-session reaches `MicStatus`, stale chunks are discarded
after 250 ms, and silence is republished while disconnected so no
consumer keeps an old pitch. Cleanups: two `println!`s in
`create_audio_capture` now go through `info!`, and `waveform.rs` said its
output was drawn on the gameplay progress bar, which no longer exists.

Carried into the song session (8): `analyze_ogg_waveform` decodes the
whole file **twice** (a frame count, then the peaks) at every song load.
Gameplay only needs the duration, which the decoder can often report
without a pass, and nothing reads `SongManifest::waveform`.

### Carried into later sessions

Found while making `cargo clippy --all-targets -- -D warnings` pass again
before the review began (main was failing it in gameplay, editor and
lessons):

- **gameplay (a), possible Bug**: `pause_menu::resume_section_repeat`
  rewinds on `paused.is_changed()`. That is also true on the system's
  first run, so a song started with an active `LoopConfig` (Practice missed
  section) may rewind during the countdown before the sink exists. This
  would race `start_at_practice_range`. Verify with an App test.
- **gameplay (a), Hazard**: the per-note reset block in
  `resume_section_repeat` duplicates `handle_loop_boundary`'s; a new
  `ScheduledNote` score field has to be cleared in both. Share one
  `reset_note_score_state`.
- **gameplay (c), Cleanup**: the guitar neck's "next" hand region was
  dropped (a `for next in [false]` loop); `NeckRegion(bool)`/
  `NeckLabel(bool)` and `update_neck_overview`'s `next_span` branches
  still carry the dead half.
