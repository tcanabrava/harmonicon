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
- **Hazard, fixed**: `LaneLayout::span` recognised a piano layout by
  shape (a vertical axis and every label equal to its pitch's note name)
  rather than by being told, so a future vertical pitch-row layout would
  silently have got white-key spacing. `LaneLayout::white_keys` now says
  so, set only by `keyboard_rows`.
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
- **Hazard, fixed**: `retained_songs::has_chart` kept its own copy of
  the chart extensions, so a format added to
  `harmonicon_score::IMPORT_EXTENSIONS` and not there would be deleted
  rather than kept on update. It is now a named `CHART_EXTENSIONS`, held
  equal to the score crate's list (plus `harpchart`) by a test in
  `harmonicon-platform`, which sees both.
- **Hazard, fixed**: retained songs accumulate in the checkout with no
  bound, and the re-validation after `preserve_removed` counted them
  against `MAX_PACK_BYTES`, so once they crossed 1 GiB every update would
  have failed with "larger than 1024 MB". That pass now checks links and
  the manifest only; the limit applies to the download, which has already
  passed it.
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

### 7. `harmonicon-platform` — done 2026-11-06

- **Bug, fixed `34af0da4`**: `load_settings` fell back to defaults on
  any extraction error, and the Startup load marks every setting changed,
  so the defaults were saved over `settings.json` half a second later.
  One field that stopped parsing (a downgrade meeting a newer
  `pitch_algorithm`, a hand-edit typo) erased calibration, content
  sources and every preference. Now only the unreadable fields fall back,
  and the original is kept as `settings.json.unreadable`.
- **Bug, fixed `cc768186`**: removing a pack while it downloaded left the
  finished download installed and recorded, an orphan nothing listed.
  Removal cancels it, and a result for an unconfigured source is
  discarded.
- **Bug, fixed `8108f03e`**: a selected theme that failed to load (its
  external folder deleted) left every menu without backgrounds, art or
  sounds. It now falls back to the bundled `default`.
- **Bug, fixed**: pack translations skipped any folder named after an
  instrument at *any* depth when walking for curricula, so a legacy
  pack's lesson folder called `piano` lost its text. Only the top level
  is a curriculum.
- **Hazard, fixed**: three native paths read `assets/` relative to the
  working directory (`scan_ui_themes`, `scan_note_themes`,
  `theme_source_prefix`), while Bevy reads from `BEVY_ASSET_ROOT`, the
  manifest directory or beside the executable, so a binary started from
  anywhere but its install folder found no bundled themes. All three now
  start from `paths::bundled_assets_dir`, which asks Bevy's
  `FileAssetReader::get_base_path`. Checked by starting a release build
  from `/tmp`: both note themes and the bundled UI themes were found. The
  launchers' `cd` into the install folder is now unnecessary but harmless.
- **Cleanup, fixed**: `LocalizationExt::msg` and `msg_args` warned on
  *every* lookup of a missing key, and several callers run per frame, so
  one missing key flooded the log. They now warn once per key.
- Checked and sound: `packs://` path confinement (only normal components,
  first must be a registered pack), debounced atomic settings saves with
  an exit flush, the cfg guards against `build.rs`, scoped translation
  loading per instrument.

### 8. `harmonicon-song` — done 2026-11-06

- **Cleanup, fixed `533b9ae6`**: `song/mod.rs` re-exported core's
  `chart`, `harmonica`, `harmonica_constraints` and `midi_file` (as
  `song::midi`), against the no-facades rule. Three call sites still used
  it; about forty docs named the facade paths. All now name
  `harmonicon_core` directly.
- **Efficiency, fixed `a659313f`**: every song load decoded its
  `music.ogg` twice to build `SongManifest::waveform`, which nothing has
  read since the progress bar stopped drawing one. The field is gone; the
  loader measures the duration only (stated length, or one counting
  pass), checked against a full count on the songs pack's real music.
- Checked and sound: the loader's validate → migrate → check → expand
  order (with the new timing check from core), optional sibling assets
  checked before `load()` so a missing one can't hang `SongLoading`, the
  score loader's track choice, lesson graph cycle refusal and unit
  gating.

### 9. `harmonicon-app` — done 2026-11-06

- **Bug, fixed `46327449`**: the profile had the same whole-file fallback
  as settings, and `flush_profile_on_exit` saves on quit, so one
  unreadable field meant launching and quitting once erased every score,
  lesson pass, drill record and the streak. Both files now read through
  `paths::load_json_over_defaults`.
- **Hazard, fixed**: recovery was per *top-level* field, so one
  unreadable entry inside `songs` or `lessons` still dropped that whole
  map. `json_over_defaults` now retries a failing object field entry by
  entry and reports only the bad ones (`songs.<key>`), for settings'
  nested structs as much as the profile's maps.
- Checked and sound: streak date maths (missed day, lapse, clock set
  back), `FirstRun` sampled before anything can save, the state enums.

### 10. `harmonicon-ui` — done 2026-11-06 (`music_score` 2026-11-08)

- **Bug, fixed `6560e1ba`, `278245f2`, `bf9a23ce`**: a modal `TabGroup`
  confines focus only once focus is inside it, and none of the five
  modals (confirm, file picker, tutorial overlay, document picker,
  phrase editor) moved focus in. Tab walked the page behind each, and
  Enter re-pressed whatever opened it. `KeyboardNavPlugin` now moves focus
  into any modal on screen; the confirm dialog lands on No.
- **Bug, fixed `278245f2`**: in Save mode the file dialog took Enter and
  Space whatever had focus, so with a dialog button focused one Enter
  pressed it *and* saved, closing the dialog twice in a frame.
- **Bug, fixed `6560e1ba`**: the file dialog's Save label was a raw
  English literal.
- **Hazard, fixed**: the localization scan in `build.rs` flagged only
  literals that look like prose (several words), and a test pinned
  `Text({"Retry".to_string()})` as allowed, so single-word labels slipped
  through ("Save" did). It now also flags one capitalised word. Flagging
  any word of two letters or more was measured first: its five hits were
  all Fluent keys the lookahead reaches inside `loc.msg(...)`, which the
  capital-letter rule excludes. It flags nothing in the tree today.
- **`music_score`, reviewed 2026-11-08 against HEAD `15643ec9`**:
  - **Efficiency, fixed**: `key_signature::rebuild` had no `run_if`, so
    every frame, for every key-signature slot, it resolved the clef,
    which collects and sorts every note in the song, before finding
    nothing had changed. It now resolves only when the signature, the
    notes or the panel's instrument clef changed, or the slot is new.
  - **Hazard, fixed**: the clef and time-signature writes in
    `rebuild_score_notes` and `rebuild_full_score` weren't scoped to their
    own panel, so the paused full score and the live strip each rewrote
    the other's clef, agreeing only because both got the same instrument
    clef. Each now writes only its own root's children
    (`rebuilding_the_full_score_leaves_the_live_staffs_clef_alone`).
  - Checked and sound: `MeterMap` (always a tick-0 segment, zero-length
    bars fall back to 4/4), bar splitting and ties, rest spelling
    (deliberate blank bar tails, all tested), accidentals per staff
    position with the key signature per letter, the spelled-pitch
    `expect` in `projection::note_spelling` (core's
    `every_midi_name_parses_back_to_its_own_semitone`, uncommitted at
    review time, covers every `u8`), and every division by a beat, bar or
    duration.
- Checked and sound: no `unwrap`/`expect`/`panic!` outside tests; the
  file dialog's overwrite question; confirm dialog `DialogId` scoping.

### 11. `harmonicon-gameplay` — done 2026-11-06 (note trainer 2026-11-08)

- **Bug, fixed `683b9c31`**: `resume_section_repeat` rewound to the loop
  on any change to `Paused` while unpaused, and song setup writes
  `Paused(false)`. A song starting with an active loop (a practice range,
  a chart's repeating section) jumped from the countdown into the loop
  with no sink to seek. Now only a real resume counts; the per-note reset
  it copied from `handle_loop_boundary` is shared (`replay_loop_range`).
- **Bug, fixed `60845ab2`**: nothing required a chart's judging windows to
  nest, though `classify_note` tests miss, then good, then perfect.
  `chart_check::scoring_problems` now does, and the Song Editor saves the
  three sorted.
- **Cleanup, fixed `9ba09551`**: the guitar neck's never-spawned "next"
  hand region and every branch on it.
- **Note trainer, reviewed 2026-11-08 against HEAD `15643ec9`**:
  - **Bug, fixed**: Bevy's Tab navigation only reaches a `TabIndex`
    inside a `TabGroup`, and no gameplay-state screen had one except
    Results: Tab reached nothing on the Note Trainer, the Bending
    Trainer, the pause menu or Jam Session. Every one of their roots
    carries `GameplayRoot`, which now requires a `TabGroup`
    (`tab_reaches_a_widget_on_a_gameplay_screen`).
  - **Cleanup, fixed**: `nearest_midi` let an infinite frequency through
    (the chord path already filtered it); the staff pitch picker's
    written pitch truncated an `i16` with `as u8`; a guarded `unwrap` in
    the bend hint is now a `filter`.
  - Checked and sound: drill stepping and resume (empty lists guarded),
    the glide's start gate and zero-span trace, the chord check (every
    pitch in one frame, nothing extra), fretboard click bounds and the
    excused `PointerClick`, stale pitches cleared after 250 ms.
- Checked and sound: the clock (rate-slewed anchoring, snap threshold,
  every jump through `rewind_to`, play/pause only on edges), the judge's
  cursor (rebuilt in range by adaptive difficulty), every coaching ratio
  (behind a minimum-sample guard such as `MIN_NOTES = 8`), no
  `unwrap`/`expect` on a reachable path.

### 12. `harmonicon-jam` — done 2026-11-06

No findings. It imports nothing from the editor; there's no
`unwrap`/`expect` outside tests; every tempo and ratio division is guarded
(`bpm.max(1.0)`, `.max(1e-6)`, `total == 0`). The generated band's
determinism under a recorded beat stream is already pinned by its own
tests.

### 13. `harmonicon-editor` — done 2026-11-06

- **Bug, fixed `9c6bf557`**: Load replaced the open document without
  asking, though New asks first. Undo restores the notes but not the
  title, artist, scoring or other metadata, so unsaved work was lost to a
  misclick. Load now asks the same question.
- **Bug, fixed `60845ab2`** (with gameplay): typed scoring windows were
  saved in whatever order the author left them; now sorted, since the
  loader refuses windows that don't nest.
- Checked and sound: every save writes atomically (`write_atomic`;
  only dev-only `debug_record` uses `fs::write`); the document survives
  leaving the editor; every real chart round-trips (existing content
  tests); the `unwrap`/`panic!` sites are documented invariants or
  guarded on the line before; MIDI import rides core's now-bounded
  `midi_file` and `render_pcm`.

### 14. `harmonicon-menu` — done 2026-11-06

No findings. The Escape routing `match` has no catch-all arm, so every
one of the 15 pages must name where Escape leads or the build fails; the
only `unwrap`s are `write!` into a `String`.

### 15. `harmonicon-lessons` — done 2026-11-06

No findings. The new instrument chooser uses the shared menu root (its
`TabGroup`), shared buttons and `On<Activate>`; tree layout is pure and
densely tested.

### 16. Root package + `harmonicon-android` — done 2026-11-06

- **Gap, fixed**: two lints in the "Rules that override defaults" table,
  `pointer_click_violations` and `scene_root_violations`, had no test, so
  nothing showed either could fail. Both have tests now (run in CI by
  `rustc --test build.rs`).
- **Bug, fixed** (found by those tests): the `On<PointerClick>` excuse
  search walked back through any non-comment lines, so one
  `not-a-widget-button:` comment excused a mistake in a neighbouring
  function up to 30 lines below. A blank line now ends the walk, and the
  excuse must sit directly above the handler. Every existing excused site
  still builds.
- Checked and sound: `validate-pack` and `port-part` exit non-zero on
  failure (2 for bad usage), the physical-design and shader tests guard
  what CLAUDE.md says they do, `harmonicon-android` is `android_main`
  only.

## Review complete

All sixteen crates reviewed, including the two parts first deferred
(`music_score` and the note trainer). Open hazards are listed under each
crate above.
