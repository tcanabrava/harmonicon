# harmonicon-song

Playable content: the chart/manifest asset pipeline and the lessons
curriculum built on it.

`harmonicon-core` owns the chart *types*; this crate owns loading them
through Bevy's `AssetServer`, decoding a song's sibling audio, and
discovering lessons on disk.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **Chart format:** JSON `.harpchart`, schema-validated at load against
  `assets/song_schema.dtd.json` (`song/loader.rs`). Types in
  `harmonicon-core`'s `chart.rs`. Time is `time` (seconds) or `tick` + tempo map.
  - The schema uses `additionalProperties: false` at every level, so
    *removing* a field from the schema breaks previously-authored charts at
    validation (serde would have ignored it). Removals must keep the old
    key as an allowed-but-ignored property, bump `format_version` and
    accept the break, or add a migration step — the `fx_mapping` removal
    did none of these when it landed, which is exactly what broke loading
    old charts on a fresh install; `chart::migrate_chart_json` (below)
    fixes that one retroactively.
  - **Old charts self-heal on load, they aren't rewritten on disk.**
    `song::chart::migrate_chart_json` runs on the raw JSON `Value` *before*
    schema validation (a chart that fails validation never reaches typed
    deserialization, so migrating after would be too late) — a small
    ordered list of `Migration { target_version, apply }` steps, each
    skipped once a chart's own `metadata.format_version` already meets
    `target_version` (missing/unparsable counts as older than everything,
    since almost every chart old enough to need migrating predates the
    field). Currently one step: `strip_legacy_fx_mapping`, folded into
    `1.1.0`. A step's `apply` returns whether it actually changed anything,
    separate from whether it was *attempted* — most charts below a step's
    threshold don't actually have the specific problem it fixes (e.g. a
    declared-1.0.0 chart that never used `fx_mapping`), so `song::loader`
    only logs when real content changed, though `format_version` itself is
    always stamped to `CURRENT_FORMAT_VERSION` whenever any step's
    threshold applied, so a chart that passes through never gets
    re-evaluated against the same migrations again. The fix lives entirely
    in memory for that load — nothing writes the migrated JSON back to the
    `.harpchart` file on disk (a re-save through the Song Editor would,
    naturally, since it serializes current in-memory state).
  - `metadata.format_version` is actively checked, not just descriptive:
    `song::chart::CURRENT_FORMAT_VERSION` is the newest version this
    build's loader understands, and `song::loader` rejects (with a clear
    `SongLoadError::Validation` message, via the pure
    `chart::format_version_supported`) any chart declaring a *newer*
    version than that — catching "this chart needs a newer Harmonicon"
    up front instead of a confusing downstream schema/field error. A
    missing field (most charts) or an older-or-equal version always
    loads; `format_version` still only needs bumping per the paragraph
    above (per-chart, tracking the newest feature that specific chart
    actually uses) — bump `CURRENT_FORMAT_VERSION` too whenever that
    bump introduces something an older loader genuinely can't read.
  - Chromatic harps are fully supported: `Harmonica::hole_count()` sizes
    lanes/overlays/editor everywhere (never hardcode 10), and
    `Modifier::Slide` is onset-validated like overblow/overdraw. No
    chromatic 3D prop mesh exists yet (art gap). BendingTrainer is
    diatonic-only by design.
  - `Song::feel: Option<song::chart::Feel>` (`Straight`/`Shuffle`) declares
    the metronome subdivision a chart is written for; `None` (the common
    case — most charts don't set it) leaves the player's current metronome
    feel choice untouched rather than forcing straight.
    `metronome_overlay::set_tempo_from_song` (`OnEnter(AppState::Playing)`)
    applies it via the pure `feel_from_chart` mapping, same place per-song
    tempo/beats-per-bar already get seeded from the chart. The Bending
    Trainer has no chart to read from, so it sets `MetronomeFeel` from its
    own controls.
  - **A song's sibling assets are all optional except the chart itself.**
    `assets_management::scan_artist_song` discovers a song by the first
    `*.harpchart` file under its `song/` subfolder — any filename, not a
    fixed `chart.harpchart` — and `song::loader::SongChartLoader` mirrors
    that tolerance for everything else a song folder can ship:
    `background.png` (falls back to a generated in-memory gradient, seeded
    from the chart's own artist/title so different art-less songs still
    look distinct — `generate_background_image`), `elements.png` (unused by
    gameplay today; falls back to `Handle::default()`), `song/music.ogg`
    — falling back to `song/music.wav`, then `song/music.mid` (see
    `SongManifest::midi_tracks`, further below, for what that last one
    does instead of populating `music`), before giving up
    (`SongManifest::music: Option<Handle<AudioSource>>` — `None` plays
    the chart with no backing track, clock free-running instead of
    anchoring to a sink, see `should_anchor_to_sink`) — and the `2d/`/`3d/`
    note asset folders (already-established fallback to the selected note
    theme).
    `Example Song 3` ships only a chart, deliberately, to exercise all of
    these at once. The load-order subtlety: every sibling is checked with
    `read_asset_bytes` *before* being handed to `load_context.load()` —
    `load()` registers the path as a hard dependency of the `SongManifest`
    asset, and a dependency pointing at a file that doesn't exist never
    resolves, so `AssetServer::is_loaded_with_dependencies` (`menu::
    check_loading`'s gate out of `SongLoading`) would wait on it forever
    instead of erroring — the game would just hang on the loading screen
    with no message, rather than "complain."

- **A score file can be a song's chart, not just its backing track**
  (`song::score_song`). Dropping a MIDI, Guitar Pro tab (`.gp3`/`.gp4`/
  `.gp5`/`.gpx`/`.gp`), MuseScore `.mscz` or MusicXML file into
  `~/Harmonicon/songs/<artist>/<song>/song/` makes it playable: a second
  `AssetLoader` (`ScoreSongLoader`, registered alongside `SongChartLoader`)
  converts it through `harmonicon-score` at load time. Only the *chart*
  differs between the two loaders — background, backing audio, waveform and
  note art are shared via `loader::assemble_manifest`, split out of
  `SongChartLoader::load_inner` for exactly that reason.
  - **Nothing in this crate names a file format.** The extensions come from
    `harmonicon_score::IMPORT_EXTENSIONS` (which is what `extensions()`
    returns — never a hand-written list, or the loader could advertise a
    format that then fails), the reader from
    `harmonicon_score::parse_import`, and everything downstream works
    through the `ScoreFile` trait. **A new format is a module in
    `harmonicon-score` and changes nothing here**; a loader that said
    `MidiScore::parse` would have made every future format a copy of
    `score_song.rs`, which is the mistake the trait exists to prevent.
  - **`song/music.mid` already meant backing audio** for a charted song
    (`SongManifest::midi_tracks`). Both readings are legitimate, so
    `assets_management::scan_artist_song` looks for a `.harpchart` in one
    pass and only falls back to an importable extension in a second —
    first-match over one `read_dir` would sometimes have played a charted
    song's backing track *as* its chart, nondeterministically.
  - **Every playable track is converted, not just the chosen one**
    (`SongManifest::source_tracks`, one `TrackChart` each). A file written
    for a band names no harmonica part, so whichever track the loader picks
    is a guess — carrying the alternatives is what lets the harp-check
    screen offer a picker and swap the choice in place, with no reload and
    with nothing downstream learning that a song can have alternates
    (gameplay still just reads `manifest.chart`). Each part is fitted to
    *its own* harmonica, since the right harp for a melody is rarely the
    right one for a bass line.
  - **The harmonica is chosen, not assumed** — `convert::suggested_harp`
    picks the key needing the fewest bends via `pitch_map::suggest_key`,
    and prefers a diatonic unless a chromatic genuinely fits better (a
    chromatic reaches every note and would otherwise always win).
  - **Title and artist come from the folder, not the file** — but as a
    *fallback*, not an override: `MidiScore::title` reports `None` because
    MIDI's "title" is the first track's name, which for a harmonica file is
    the word "Harmonica" (observed in the game as a song called exactly
    that). A format with a real title keeps it.
  - A track named harmonica/gaita/mouth harp/blues harp wins over a
    better-scoring one — the file is telling us, and extra bends mean
    harder, not wrong. It does **not** win over being unplayable: a bass
    line named "Harmonica" converts to a chart with no notes at all, which
    a test caught. Failing a name, the best-surviving part wins, which beats
    "the busiest track" (routinely a guitar) and is safe only because it's a
    correctable default. A file where *no* part clears
    `convert::MIN_REACHABLE` is refused, with how close the best one got.

- **Lessons** (`harmonicon-song`'s `lessons/` — `manifest.rs`/`catalog.rs`/`progress.rs` —
  plus `harmonicon-menu`'s `menu/pages/lessons.rs`; design in `docs/lessons_plan.md`):
  `assets/lessons/<unit>/<lesson>/lesson.json` (schema
  `assets/lesson_schema.dtd.json`, validated at startup scan; ids are
  stable — profile keys and prerequisites reference them). A chart-backed
  lesson plays its `.harpchart` through the *ordinary* song pipeline — no
  lesson-specific scoring — with a `LessonContext` resource in flight:
  results judge `pass_criteria` against it instead of recording a song
  best, `setup_adaptive_difficulty` forces gating off, and
  `route_menu_entry` returns to the lesson list and removes it (Menu entry
  is the context's end-of-life; Results→Retry never passes through Menu, so
  retries keep it). Manifest text fields are Fluent *keys*
  (`title_key`/`body_key`, `lesson-unit-<unit>`), never display strings;
  `tests/asset_layout.rs` validates every bundled lesson (schema, chart,
  file completeness, prereq integrity, locale-key existence). Prerequisite
  gating (`lessons::is_unlocked`) is bypassed in `menu::pages::lessons::
  populate_lesson_rows` under `--features dev` — every lesson shows
  unlocked for quick manual access while iterating; `is_unlocked` itself is
  untouched and still fully covered by its own prerequisite tests.

- **Lessons can also live in `~/Harmonicon/lessons`**, same
  bundled-plus-external pattern as songs/themes:
  `lessons::catalog::scan_all_lessons` scans `assets/lessons` then, if
  present, the external drop folder (bundled entries first, so shipped
  curriculum ordering/prerequisites are unaffected by whatever a player
  drops in), tagging external lessons' `chart_asset_path` with
  `external://lessons` the same way `assets_management::scan_artist_song`
  tags external songs. Both are kept live via the single shared
  `~/Harmonicon` watcher (`assets_management::watch`, see the Asset
  sources bullet above) — `lessons::catalog` is its own consumer of that
  watcher's generic `ExternalFolderChanged` message (checking for the
  `"lessons"` subfolder), rather than `assets_management` knowing what a
  lesson is: `assets_management` is low-level shared vocabulary, `lessons`
  a feature built on it, so the dependency points that way and not the
  reverse (`docs/physical_design_plan.md`). A live rescan fires
  `LessonsRescanned`; `menu::pages::lessons::rebuild_on_lessons_rescanned`
  forces a same-page rebuild if the Lessons list happens to be open.

- **Lesson discovery is `#[cfg]`-split, and this crate has its own
  `build.rs` because of it.** `scan_lessons_root` walks `assets/lessons`
  with `std::fs::read_dir` and parses each `lesson.json`'s bytes *directly*,
  not through `AssetServer` — which means it finds nothing at all on a
  target whose `assets/` tree isn't a readable local directory (wasm has no
  filesystem; Android's assets live inside the APK). So
  `#[cfg(any(target_arch = "wasm32", target_os = "android"))]` selects a
  sibling `scan_all_lessons` that reads `build.rs`'s generated
  `BUNDLED_LESSONS` instead. Three things about it:
  - It embeds the **JSON text** via `include_str!`, unlike
    `harmonicon-platform`'s manifests which carry only names — because of
    that direct-bytes read above.
  - It can't live in `harmonicon-platform`'s build script:
    `include!(concat!(env!("OUT_DIR"), ...))` reads the *including* crate's
    `OUT_DIR`, and `OUT_DIR` is per-package.
  - It has no external-folder half. There's no `~/Harmonicon` to drop a
    lesson into on either target, which is why it takes no root path.
  Until the Android port added this, the module had no manifest path at
  all — so the Lessons menu was silently empty on wasm as well.
