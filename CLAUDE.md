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
  changes, not just internal ones. `docs/book/src/images/*.png` are
  placeholder screenshots (script-generated captioned frames) pending real
  captures — keep the filenames stable when swapping them in so the
  `![...](images/foo.png)` references throughout `docs/book/src/*.md`
  don't need touching.

## Commands

```bash
cargo run --features dev   # local iteration (dynamic linking + asset watcher)
cargo run --release        # playable build; never ship the dev feature
cargo test                 # ~590 pure-logic tests; safe headless
cargo clippy               # keep clean
```

Binaries: main game, plus `hole-editor`, `note_editor` (in `src/bin/`).
Manual testing needs a mic, audio out, and a display.

## Architecture (load-bearing facts)

- **Crate = lib + bins.** `src/lib.rs` re-exports subsystems so the game and
  tools share them: `audio_system`, `song`, `gameplay`, `scoring`,
  `song_editor`, `lessons`, `menu`, `dialogs`, `spectrogram`, `theme`,
  `localization`, `settings`, `profile`, `assets_management`.
- **Audio input path:** cpal callback → mono downmix → 4096-sample chunks
  with 50% overlap (`audio_system/audio_input.rs`) → crossbeam channel →
  `process_audio` in `main.rs` → one FFT per chunk (`pitch_detect::analyze`)
  → `PitchEvent` message + `AudioFrame` resource (shared with spectrogram).
  Five selectable algorithms (FFT/YIN/pYIN/MPM/NMF) in
  `audio_system/pitch_detect.rs`.
  - The capture callback must stay allocation-free: chunk buffers come from
    a recycling pool (`AudioCapture::free_sender`); `process_audio` returns
    the previous `AudioFrame::samples` buffer to that pool each frame. Keep
    that contract intact when touching either side.
  - Mic lifecycle lives in `start_capture(&mut World)` + the `MicStatus`
    resource (`Connected`/`Failed`); Options has a device picker and retry,
    persisted as `AudioSettings::input_device`. Startup capture is ordered
    `.after` settings load so the saved device preference wins.
- **Detection range is chart-driven:** the `PitchRange` resource (defined in
  `pitch_detect.rs`, default 200–2500 Hz) is derived from
  `Harmonica::frequency_range()` at song start and from the selected key in
  the bend trainer; both reset it on state exit. The NMF dictionary's
  staleness check includes the range — keep it that way if you add inputs.
- **Time authority:** `GameplayClock`, ticked by `tick_clock` — both in
  `gameplay/clock.rs`, along with `should_anchor_to_sink` and
  `handle_loop_boundary` (the anchoring invariant lives in that one file).
  Negative during the 3 s countdown; music starts at clock 0. Once music
  plays (and outside Jam Session) the clock is anchored to the `AudioSink`
  position via `GameplayClock::advance`, which rate-slews toward it (±0.5%
  speed, not a fixed per-frame step — a proportional nudge is
  inaudible/invisible and doesn't bias every judged offset by a constant
  amount) and snaps outright past 0.5 s of drift (a stall/seek, not
  ordinary jitter). Free-runs on frame deltas during countdown, pause, and
  Jam Session. Two invariants, the second enforced by the type rather than
  just documented:
  - Every clock-reading system must be ordered `.after(GameplayLogic)` or
    notes stutter (see the SystemSet docs in `gameplay/plugin.rs`).
  - **Anything that jumps the clock must also seek the music sink or suspend
    anchoring** — otherwise the anchor drags the clock forward again every
    frame (the sink is always "ahead", so the correction always saturates).
    `GameplayClock`'s inner value is private; the only ways to change it are
    `set_free(t)` (anchoring guaranteed inactive — setup/countdown, Jam
    Session, Bending Trainer), `advance(dt, audio_pos)` (`tick_clock`'s own
    per-frame update), and `rewind_to(t, sink)` (jumps *and* seeks the sink
    in one call — what `handle_loop_boundary` uses, and what any future A–B
    looping UI or practice-speed feature must use too).
- **Scoring:** pure functions in top-level `src/scoring.rs` (shared by
  gameplay and the song editor's practice mode), driven by the
  `score_notes` system in `gameplay/judge.rs` (alongside
  `update_active_targets`, `technique_confirmed`, `style_bonus_points`, and
  `modifier_fx_key`). `ScheduledNote`/`SongNotes` and the chart-time pure
  helpers (`target_pitch`, `resolve_item_time`, `last_note_end`,
  `LOOKAHEAD`) live in `gameplay/notes.rs`; the score/combo/config
  resources (`Score`, `SongStats`, `PitchGate`, `ScoringConfig`, …) live in
  `gameplay/state.rs`; the score HUD (`update_score_display`) lives in
  `gameplay/hud.rs`; song-lifetime setup/teardown (`reset_score`,
  `setup_scoring_config`, `detect_song_end`, `cleanup_gameplay`) lives in
  `gameplay/lifecycle.rs`. `gameplay/mod.rs` itself is wiring + re-exports
  only — every path below still resolves as `crate::gameplay::X`. Key
  concepts:
  - **Pitch identity is a MIDI note number (`u8`), not a formatted name
    string** — `PitchInfo::midi`, `ValidHarpNotes(HashSet<u8>)`,
    `PitchGate`'s `consumed: HashSet<u8>`, `ScheduledNote::expected_pitch:
    Option<u8>` (`None` for a hole/direction the harp can't produce —
    `target_pitch` returns that). This is what lets `score_notes` compare
    detected-vs-expected pitch by integer equality with zero per-frame
    allocation, and rules out enharmonic mismatches (`"A#4"` vs `"Bb4"`)
    entirely — they're the same `u8`. `note`/`octave` strings still exist on
    `PitchInfo` and `Harmonica::wind_direction_label`/`slide_label` purely
    for display; `Harmonica::wind_direction_midi` is the identity-comparison
    sibling of `wind_direction_label`. Pitch-*class* sets (`blues_scale_
    classes`, chord tones — no octave, so no MIDI number to key on) are
    still strings; that's a deliberately separate, narrower concern.
  - **Score state lives in `SongNotes` (`Vec<ScheduledNote>` + a `cursor`),
    not on ECS components.** `ScheduledNote` is plain data — this is what
    lets `gameplay_2d`/`gameplay_3d` spawn note *visuals* (`NoteVisual`/
    `NoteVisual3D`, carrying only a `note_id` index into `SongNotes::notes`)
    in a rolling `LOOKAHEAD` window instead of the whole song at once
    (`spawn_visible_notes`/`spawn_visible_notes_3d`, sharing the windowing
    logic via `notes_needing_spawn`), and lets `handle_loop_boundary` reset a
    note's state with a binary search + slice mutation instead of an ECS
    query. `notes` is kept sorted by `time` (sorted once at song load) so
    both the scoring cursor and the render window can use `partition_point`/
    early-break instead of scanning the whole song every frame. Recolor-on-
    hit systems (`update_note_visuals*`) have no `Changed<ScheduledNote>`
    filter (not a component, nothing to filter on) and just re-sync every
    currently-*spawned* note each frame — cheap, since only the window's
    worth of notes ever have a visual. Despawn-on-scroll-past needs no
    looping special case either: a note can freely despawn and get
    respawned fresh, since its score state was never on the entity to lose.
  - Candidates are scored in `|offset|`-sorted order (two-pass over
    `SongNotes`) so overlapping same-pitch notes resolve deterministically;
    notes beyond the good window are skipped before the sort (and, being
    sorted by time, end the scan outright rather than just being skipped).
  - `input_latency_ms` shifts the judged clock; calibration screen exists,
    and the results screen offers one-click application of the measured
    mean offset.
  - Bends are validated at onset via `target_pitch` (expected pitch is the
    bent one, rounded to the nearest semitone); vibrato/wah are verified
    from `(time, value)` samples collected during the sustain — measured
    oscillation rate must match the chart's `oscillation_hz` within ±40%
    (`oscillation_matches_rate`).
  - **Chord/octave-split notes** (a chart `TrackItem` with more than one
    `events` entry — `PlayMode::Chord`/`Split`) still spawn one
    `ScheduledNote` per event, but every sibling note carries the full
    target set in `ScheduledNote::chord_pitches` (empty for an ordinary
    single-event item). `score_notes` ANDs `scoring::chord_is_sounding`
    (every pitch in the set present at once) into that note's existing
    per-pitch `PitchGate` freshness check, so a chord only scores when its
    siblings are struck together — playing the same holes one at a time
    doesn't satisfy it (also excluded from `clean_attack`: a chord note is
    supposed to have company). No chart schema change was needed —
    multi-event `TrackItem`s already existed for the visual chord/split
    badge; nothing previously required their events to sound together.
    Unlike `clean_attack`, this needed no dedicated `SongStats` bucket —
    `chord_is_sounding` gates `Hit` itself, so an out-of-sync chord already
    reads as a plain miss in ordinary accuracy.
  - There's a headless end-to-end test driving `score_notes` with a
    scripted pitch stream (`end_to_end_synthetic_song_drives_score_combo_
    and_stats`) — extend it when changing scoring behaviour.
- **Chart format:** JSON `.harpchart`, schema-validated at load against
  `assets/song_schema.dtd.json` (`song/loader.rs`). Types in
  `song/chart.rs`. Time is `time` (seconds) or `tick` + tempo map.
  - The schema uses `additionalProperties: false` at every level, so
    *removing* a field from the schema breaks previously-authored charts at
    validation (serde would have ignored it). Removals must keep the old
    key as an allowed-but-ignored property, or bump `format_version` and
    accept the break — the `fx_mapping` removal did neither (a known,
    accepted break; `ROADMAP.md` 0.5 covers its possible reintroduction).
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
    — falling back to `song/music.wav` if that's absent too, before giving
    up (`SongManifest::music: Option<Handle<AudioSource>>` — `None` plays
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
  - **The Song Editor can import a MIDI file** (`song_editor::midi_import`).
    The actual MIDI-file *parsing* (tempo map, note on/off pairing, track
    names) is pure, shared code in `song::midi`, kept separate from any
    pitch-to-harp resolution: picking a `.mid`/`.midi` file lists its tracks in a
    dynamically-rebuilt `dialogs::combobox` (rebuilt only on a fresh file
    load — `MidiFileLoaded` — not on every track pick, so selecting doesn't
    fight the dropdown's own open/close state); picking a track parses that
    track's notes onto the editor's tick grid (quantized, same resolution
    manually-placed notes already use) and resolves each MIDI pitch onto a
    harp key via `pitch_map::map_pitch` — an exact blow/draw match, else a
    bend within `state::max_bend`'s per-hole cap (diatonic) or a slide
    (chromatic, one semitone up), else the nearest playable note — reusing
    `state::pitch_compatible` so an import can never produce a note the
    editor's own UI wouldn't allow. (`song_editor::pitch_map` holds all
    pitch-onto-harp resolution — `map_pitch`, its no-fallback sibling
    `map_pitch_playable`, and `suggest_key` — shared between MIDI import
    and live recording, which want opposite fallback behaviour; see the
    recording bullet below.) The key itself isn't just whatever was
    already selected: `on_midi_track_selected` first scores every
    `state::HARP_KEYS` entry via `suggest_key`/`key_fit_score` (the fraction
    of the track's raw MIDI pitches landing on an exact blow/draw match —
    no bend/slide/fallback needed) and imports onto whichever key scores
    highest, updating `EditorState::key` to match; the harmonica *kind*
    (diatonic/chromatic) is left alone regardless, since flipping that is a
    much bigger, more disruptive change than a key, which is one more click
    to undo via the meta form's own Key field. Saving while a track
    is selected additionally writes a processed copy of the MIDI with that
    track removed (a "processed" file next to the chart) and a synthesized
    WAV mixdown of every *other* track — via
    the editor's own `playback::render_pcm` synth, which already sums
    overlapping notes — as `song/music.wav`, since the engine cannot play a
    raw MIDI file and no OGG encoder is in the dependency tree; this is what
    "the MIDI file becomes the background song" resolves to. `MidiImport`
    stores the raw file bytes (not a parsed `midly::Smf`, which borrows
    them) and re-parses on demand, so switching the picked track needs no
    lifetime bookkeeping across frames.
  - **The Song Editor can also record notes live** (`song_editor::record`;
    `Mode::Record` is its own top-level mode alongside Edit and Play —
    `state::Mode`, one visibility-toggled button group each, see
    `panel::update_mode_visibility` — with its own Play/Pause/Stop/Finish
    transport in `transport::spawn_record_buttons`: Play starts a take
    *from the current playhead position* or resumes a paused one; Pause
    freezes the take in place, closing any held note
    (`record::pause_record`); Stop ends the take leaving the playhead
    where it stopped; Finish ends it and rewinds to zero. While no take
    runs, clicking the beat ruler parks the playhead at that tick as a
    paused transport (`timeline::on_timeline_click_seek`) and the next
    take records from there — the background music is sought to the same
    offset via `playback::PendingMusicSeek`, a one-shot applied when the
    freshly spawned sink appears, since `AudioSink` doesn't exist yet in
    the system that spawns the `AudioPlayer`. Recording also *punches in*:
    a recorded note removes any note overlapping its span that isn't part
    of the current take (`RecordState::take_ids` — same-take chords must
    coexist; `punch_out_overlaps`), so re-recording replaces instead of
    layering impossible blow-and-draw-at-once combos.) Recording shares
    `pitch_map`'s resolution instead of reading file bytes — the
    microphone/pitch pipeline (`main.rs`'s `process_audio`) already runs
    continuously regardless of `AppState`, the same `PitchEvent` stream
    Practice mode's own `practice_tick` already consumes, so recording
    needs no capture lifecycle of its own. A note is pushed onto
    `EditorState::notes` the instant its onset is detected — at minimum
    length — rather than only once it's released, and grown every frame
    while held, so the player watches each note appear and extend on the
    grid in real time instead of only seeing it once they stop playing it.
    Unlike gameplay scoring, recording has no chart of expected notes to
    lean on, so it defends against raw detector noise itself, everything
    precomputed once at `start_record` (see `record.rs`'s module docs):
    `PitchRange` is narrowed to the selected harp (same as gameplay's
    chart-driven narrowing; restored to default by `stop_record`); a
    128-entry MIDI→(hole, dir, pitch) table built from
    `pitch_map::map_pitch_playable` — the *no-fallback* variant — resolves
    each detection, discarding pitches the harp can't produce instead of
    letting `map_pitch`'s nearest-note fallback disguise noise as a
    plausible hole; and onsets/releases are debounced (`CONFIRM_EVENTS`/
    `RELEASE_GRACE_EVENTS`): a note deleted again unless seen in 2
    consecutive pitch events, a held note surviving a dropout chunk
    without splitting. Onset timestamps subtract the detection delay (half
    the 4096-sample analysis window + the calibrated
    `AudioSettings::input_latency_ms`, cached as `RecordState::
    detect_delay`) so notes land where they were played, not where they
    were recognized. `record::record_tick` applies each arriving event via
    the pure `apply_detected_pitches` (onset/release diff against
    `RecordState::open`), then calls `grow_open_notes` every frame to
    extend still-sounding notes (a note inside its release-grace window is
    frozen, not grown, so the grace chunks don't pad its length);
    `stop_record`'s `finish_open_notes` closes everything out the same way
    so a note doesn't freeze one frame short of the actual release. A bend
    played and held resolves correctly because of the shared resolution —
    `PitchInfo::midi` rounds a bent pitch to *its own* nearest semitone,
    not the unbent note's, so it lands on `Pitch::Bend`, not the
    nearest natural note. Recording reuses `Playhead` for its clock rather
    than inventing a second one, with `total: f32::MAX` since a take has no
    natural end the way Play/Practice (bounded by the chart's own last
    note) do — `PlayheadLine`'s existing moving cursor becomes live
    "where's this landing" feedback for free. Recording only ever appends
    to `EditorState::notes` (never replaces, unlike MIDI import's one-shot
    `state.notes = imported.notes`), so re-recording a take can't silently
    destroy earlier work; Stop and the Edit-mode switch both call
    `stop_record` unconditionally alongside `stop_practice` (closing out
    any note still open at that instant), the same "stop whatever's
    running" pattern already used for Practice — and starting Play or
    Practice while a recording is in progress does the same, since both
    would otherwise silently repurpose the `Playhead` clock `RecordState::
    open`'s timings are still anchored to.
  - **The Song Editor can author lessons, not just plain songs**
    (`song_editor::lesson_form`): a "Record Song"/"Record Lesson" toggle
    (`EditorState::content_kind: ContentKind`, its own click-to-cycle
    button in the meta form next to the harmonica-kind one) switches
    Save/Load to write/read a `lesson.json` instead of a `.harpchart`, and
    shows a second fields panel (`LessonFormGroup`, shown/hidden via
    `Node::display` — the same approach `EditModeGroup`/`PerformModeGroup`
    already use, not `Visibility`, which would still reserve layout space)
    for everything `assets/lesson_schema.dtd.json` needs beyond the
    ordinary song fields: lesson id, unit, an explanation text field,
    comma-separated prerequisites, and three more click-to-cycle fields —
    pass-criteria kind, technique (only meaningful when the kind is
    Technique), and progression — sharing `meta_form::spawn_field_row`
    (made `pub(super)`) with the ordinary Key/Position fields; all five
    click-to-cycle fields now share one `state::cycle_next(options,
    current)` helper rather than repeating the same lookup-and-wrap logic.
    Note editing, playback, and practice are completely unaffected by
    which `ContentKind` is active — a chart-backed lesson's chart is an
    ordinary `.harpchart`, written to `song/chart.harpchart` next to the
    manifest (exactly the layout every shipped lesson already uses) via
    the same `harpchart::serialize_harpchart` a plain song save calls.
    Save/Load each have one system per `ContentKind`
    (`harpchart::handle_save_chosen`/`handle_load_chosen` for Song,
    `lesson_form::handle_save_lesson_chosen`/`handle_load_lesson_chosen`
    for Lesson) reading the same `FileChosen` message and skipping
    whichever `ContentKind` isn't theirs, rather than one function
    branching internally — `serialize_lesson` validates its own output
    against the schema via `lessons::parse_lesson` before writing, printing
    a warning (not a silent invalid write) if it doesn't pass.
    **Deliberate scope boundaries**: `lesson.json` only stores Fluent
    *keys* (`title_key`/`body_key`), never display text, so an author's
    typed title/explanation can't be written as a real translation —
    `serialize_lesson` derives the keys from the lesson id and prints the
    key/text pairs to add to the locale files by hand, the same manual
    step authoring any bundled lesson already requires. A lesson save also
    skips `harpchart::save_midi_backing` (the MIDI-import backing-track
    convenience, `ContentKind::Song`-only) — author the chart as a song
    first if it needs a MIDI-derived backing track, then switch to Lesson
    mode to add the curriculum fields.
  - **The Song Editor's Select/Erase/Remove timeline tools**
    (`song_editor::timeline` — interaction; `song_editor::
    timeline_overlay` — the persistent overlay entities and their
    per-frame redraw): with Select active (`EditorState::timeline_tool`),
    the beat ruler above the grid builds a range selection, which the
    Erase/Remove mod-panel buttons then act on (`panel_widgets::
    timeline_tool_button` → `dialogs::confirm_dialog`; only a confirmed
    `ConfirmChosen` runs the pure `state::erase_range` — deletes notes in
    range, nothing else moves — or `state::remove_range` — deletes them
    *and* shifts every later note earlier, closing the gap). Selecting
    works two ways — click-hover-click on a placed split point
    (`EditorState::timeline_split`), or click-drag-release for an explicit
    span — both driven entirely by `Pointer<DragStart>`/`Drag`/`DragEnd`,
    deliberately **not** `Pointer<Click>`: `bevy_picking` fires
    `DragStart` on any nonzero pixel motion while pressed (mouse jitter
    routinely produces one on an intended click), and fires `Click` *and*
    `DragEnd` on the same release, `Click` first — so
    `on_timeline_drag_end` alone decides (a span that genuinely moved is
    a drag-select; a same-tick one is a click against the split point).
    Load-bearing structural facts:
    - **The span lives in the `TimelineSelection` resource, not
      `EditorState`** — same separation (and reason) as `Scroll`: it
      updates every pointer-move, and routing that through `EditorState`
      would either rebuild the grid per-move or (the old guard against
      exactly that) suppress the scroll-driven rebuilds a mid-drag wheel
      pan needs. `rebuild_grid`'s early-return guard covers *only*
      `state.dragging` (note drags own picking-captured note entities);
      scrolling mid-selection rebuilds freely.
    - **The ruler's drag catcher (`TimelineSurface`) is persistent**,
      spawned once via `timeline_overlay::spawn_persistent_entities`
      (with `MoveGhost`/`PlayheadLine`), *not* respawned per rebuild — a
      mid-gesture rebuild would despawn the entity picking captured the
      drag on. `sync_timeline_surface` keeps it glued to the visible
      viewport (`left = Scroll::px`).
    - **A mid-drag wheel pan extends the selection**: the span end is
      pointer motion (`Pointer<Drag>::distance` ÷ `UiScale`, same as note
      drags — a drag routinely leaves the ruler's thin strip) *plus* the
      scroll delta since the press (`TimelineDrag::scroll_px`,
      `drag_end_tick`); and since `Drag` only fires on pointer *motion*,
      `sync_selection_with_scroll` re-derives the end from the stored
      `pointer_px` on scroll-only frames. `TimelineDrag::live`
      distinguishes the in-flight gesture from the persisted (frozen)
      selection a release leaves behind.
    - `RelativeCursorPosition::normalized` is **-0.5..0.5** across a
      node's own width, not 0..1 (`TimelineSurfaceGeometry::tick_at`'s
      `+ 0.5` re-centering, same correction `gameplay::
      song_progress_overlay::cursor_to_time` applies). The hover-side
      preview (`timeline_overlay::update_timeline_overlays`) reads it
      fresh each frame as a local value rather than writing it anywhere,
      so previewing can't trigger rebuilds.
  - **The Song Editor's silence track** is a read-only summary strip
    (`SILENCE_ROW_H`, below the last hole lane — `grid_height` folds it into
    every height that already derives from hole count, so the row container/
    grid area/playhead/timeline overlays all extend to cover it for free)
    showing the gap, in seconds, between consecutive notes. `state::
    silence_gaps` is pure: it merges every note's `[tick, tick+len)`
    interval *across all holes* first (a chord, or one note's tail
    overlapping the next note's onset, must not read as silence — silence
    means nothing at all is sounding, not just one hole), then returns the
    tick ranges between what's left; leading/trailing silence is excluded
    since there's no "next note" to measure up to. `grid::rebuild_grid`
    renders one block per gap that intersects the currently-visible tick
    window (same visibility filter already used for notes), labeled via
    `state.tempo_map()` + `tick_to_seconds` (see the tempo-map bullet below)
    rather than a flat BPM multiply. Purely informational — every block and
    the row's own background strip are `Pickable::IGNORE`.
  - **The Song Editor's grid header shows the chart's music file as a
    waveform** (`song_editor::waveform`), aligned against the chart's own
    tempo map (see below) rather than a single constant BPM. Reuses
    `audio_system::waveform`'s existing decoders (the same ones a shipped
    song's own music gets analyzed with at asset-load time) rather than
    duplicating any audio-decoding logic; `MusicWaveform::path` is the
    resource's own cache of the `EditorState::music` value it was last
    decoded from, so `sync_music_waveform` only re-decodes when the path
    actually changes rather than depending on `Changed<EditorState>` (which
    fires far more often than the music field itself does) — the decode is
    synchronous on the main thread, same as `midi_import`'s own file-picker
    handling. `grid::rebuild_grid` only spawns bars for buckets whose time
    falls in the currently-scrolled-into-view beat range
    (`waveform::visible_waveform_buckets`), the same windowing principle as
    the note grid's own column loop. The strip lives in the header:
    `HEADER_H` grew (`WAVEFORM_TOP`/`WAVEFORM_H` added on top of the
    existing beat/bar-label space) rather than adding a whole separate
    reserved row like the silence track's — every other module that reads
    `HEADER_H` as "where hole row 1 starts" (the hole column's own spacer,
    `sync_chrome_height`, `note_rect`, the timeline ruler) adjusts for free.
  - **The Song Editor supports a real variable tempo map**, not just one
    flat BPM: `EditorState::tempo_changes: Vec<(usize, f32)>` (tick, BPM)
    plus the fixed BPM-field-derived point at tick 0, combined via
    `state::build_tempo_map` into a `song::chart::TempoPoint` list — the
    same type gameplay's own chart-driven tempo map already uses, so
    editor and engine share one tick↔seconds representation
    (`tick_to_seconds`, and its new inverse `seconds_to_tick`, both in
    `song::chart`). A Tempo timeline tool
    (`TimelineTool::Tempo`, alongside Select/Erase/Remove in the same
    mod-panel row) turns a click on the beat ruler into
    `state::toggle_tempo_point`: click near an existing point removes it,
    otherwise a new one is added at the clicked tick, stepped
    `TEMPO_STEP_BPM` above whatever BPM is already in effect there
    (`bpm_at`) — unlike Erase/Remove it never opens a confirm dialog (one
    tempo point is trivially undoable with another click), so it wires
    `Pointer<Click>` directly rather than reusing the Drag-based span
    machinery those tools need to dodge the Click/DragEnd race (see that
    tool's own doc above). Points are rendered as vertical markers + a
    `♩=<bpm>` label on the grid header (`grid.rs`). Save/load
    (`harpchart.rs`) round-trips the full map through `Timing.tempo_map`
    (writing every point, not just the first) and, on load, rescales a
    *foreign* `timing.resolution` (e.g. a MIDI-derived chart authored at a
    different tick resolution than the editor's own `TICKS_PER_BEAT`) into
    the editor's own tick units by a constant ratio — fixing a pre-existing
    bug where `resolution` was never read at all, silently mis-scaling any
    chart not authored at `resolution: TICKS_PER_BEAT`. MIDI import
    (`midi_import::import_track_notes`, via `midi_parse::editor_tempo_map`)
    carries a track's real tempo automation into `tempo_changes` instead of
    collapsing it to one average BPM, converting each point by real-time
    position (`tick_to_seconds`/`seconds_to_tick`) since a MIDI file's own
    `tpq` has no fixed ratio to the editor's tick unit the way two
    `resolution: TICKS_PER_BEAT` charts do.
    **Scope boundary, deliberate:** this covers the editor's grid/waveform
    *display* and the chart's on-disk tempo map only. Play/Practice/Record
    audio synthesis (`song_editor::playback`'s `render_pcm`, shared with
    `gameplay::call_response`) still renders against one flat nominal BPM —
    the same already-accepted simplification `call_response` documents
    above for mid-phrase tempo automation. Extending the synth to follow a
    variable tempo map is future work, not a gap in this feature.
- **Asset sources:** bundled `assets/` plus an `external://` source mapped to
  `~/Harmonicon` (registered in `main.rs` before DefaultPlugins). When
  loading siblings of an asset, propagate its source or external songs
  silently resolve against the bundled tree (see comment in `song/loader.rs`).
  - **`~/Harmonicon` is watched live**, not just scanned once at Startup:
    `assets_management::watch` starts one recursive `notify-debouncer-full`
    watcher on it (our own direct dependency, no-op if the folder doesn't
    exist — most players never create it), debounces bursts of filesystem
    events, and fires one generic `ExternalFolderChanged{top_level_dirs}`
    message per batch naming which immediate subfolders (`songs`, `themes`,
    `lessons`, ...) something changed under
    (`watch::changed_top_level_dirs`) — `watch.rs` itself stays agnostic of
    what any of those subfolders *mean* (see "dependencies point downward"
    in `docs/physical_design_plan.md`). `assets_management::mod.rs`'s own
    `rescan_on_external_change` consumes that message for the two kinds
    this module owns (`songs`/`themes`), re-running `scan_all_songs`/
    `scan_ui_themes`; `lessons::catalog` has its own sibling consumer for
    `lessons` (see the Lessons bullet below) — one small
    `lessons`-depends-on-`assets_management` edge rather than the reverse.
    Every scan function fully replaces its resource's contents rather than
    appending, so each is safe to call again at runtime (`scan_all_songs`
    clears `AvailableSongs` first — it didn't always; `scan_ui_themes`/
    `scan_lessons` already assigned wholesale). A successful live rescan
    also fires its own specific `SongsRescanned`/`ThemesRescanned`/
    `LessonsRescanned` — a message, not a bare `is_changed()` poll, because
    the menu pages that consume them only run their consuming system while
    open, so their own change-detection tick would otherwise read
    stale-as-changed on every re-entry rather than only on a genuine live
    drop-in. Deliberately **not** built on `bevy::asset::io::file::
    FileWatcher`/Bevy's own asset-hot-reload path: that path only reloads
    already-loaded `Handle`s (useless for content that was never loaded to
    begin with), and whether *any* source watches at all is one global
    `AssetPlugin::watch_for_changes_override` flag applied to every
    registered source uniformly — turning it on for `external://` would
    also enable asset hot-reloading for the bundled `assets/` tree in
    shipped builds, which is exactly the `--features dev`-only behavior
    this file's Commands section says never to ship.
- **States:** `AppState` (Startup/Menu/SongLoading/Playing/Results/
  Calibration/Credits/SongEditor2/BendingTrainer) + `MenuPage` sub-states in
  `menu/mod.rs`. `GameplayMode` (Play2D/Play3D/JamSession) selects which
  setup/update chains run within `Playing`.
  - **`SongManifest` doesn't have to come from the `AssetServer`.**
    `jam::backing::build_generated_manifest` synthesizes one at runtime (a
    procedurally-generated 12-bar bass line + chart, for `menu::pages::
    jam_generate`'s "Generate Jam" flow — Jam Session without picking an
    existing song) and registers it with a plain `Assets::add`. Such a
    manifest has no tracked `LoadState`, so `menu::routing::check_loading`'s
    `asset_server.is_loaded_with_dependencies` would never return true for
    it — both the initial launch and Restart route around `SongLoading`
    entirely (the `jam_generate` Start button sets `AppState::Playing`
    directly; `pause_menu::on_restart` targets `Playing` instead of
    `SongLoading` when `jam::backing::GeneratedJamSession` is present, safe
    because `NextState::set` always re-fires `OnExit`/`OnEnter` even for a
    same-state transition, per `bevy_state`). `GeneratedJamSession`'s
    presence is also what `menu::routing::route_menu_entry` checks to route "Quit
    Song" back to the jam setup page instead of `MenuPage::SongList` (a
    generated jam never went through the song list) — same end-of-life
    pattern as `lessons::LessonContext`.
- **Settings:** figment-layered `<config>/harmonicon/settings.json`
  (`settings.rs`); saves are debounced (`PendingSave`, 0.5 s) with a flush
  on `AppExit` — route new persisted fields through that path.
- **Profile:** `<config>/harmonicon/profile.json` (`profile.rs`) — per-song
  best score/accuracy, per-technique best accuracy, bend-trainer drill
  records, total play time. Unlike settings it saves directly at the
  (infrequent) points where a record changes, plus a flush on `AppExit` for
  play time — deliberately no debounce machinery; keep new fields on that
  pattern.
- **Adaptive difficulty** (`gameplay/adaptive_difficulty.rs`): a chart is
  divided into "sections" via the existing `TrackItem::phrase` tag (no
  schema change) — the same boundary rule `phrase_overlay` uses. Each
  section has a persisted, independent "learned" fraction
  (`profile::SongRecord::phrase_learned`, indexed by the section's ordinal
  position in the track); only a prefix of a section's notes are
  spawned/scored at a time, growing on a clean clear. Whether the feature
  is on at all is a single **global** setting
  (`settings::AdaptiveDifficultyEnabled`, an Options-menu toggle, off by
  default) — not per-song; only the learned progress itself is per-song.
  The pause menu's manual override and its own on/off toggle both take
  effect **immediately, mid-song** — `gameplay_2d`/`gameplay_3d`'s
  `resync_notes_on_adaptive_change` rebuilds `SongNotes` the moment
  `AdaptiveDifficulty` changes, carrying over already-resolved hit/miss
  state via `carry_over_note_state` (matched by `(time, hole, is_blow)`, not
  array position) so notes already judged don't reset just because the list
  was rebuilt around them; the pause-menu toggle flips
  `AdaptiveDifficultyEnabled` (persisted) and the live `AdaptiveDifficulty::
  enabled` (session cache) together, so the change is both immediate and
  becomes the new default for the next song.
- **Score HUD is message-driven, not polled:** `score_notes` emits a
  `NoteScored`-style message with the hit quality/points/new combo at the
  instant a note is judged; `update_score_display` is a `MessageReader`
  consumer, not a per-frame `format!` into `Text`. Follow this pattern for
  any future HUD element whose trigger is a discrete scoring event rather
  than a continuously-varying value.
- **Lessons** (`src/lessons/` — `manifest.rs`/`catalog.rs`/`progress.rs` —
  plus `src/menu/pages/lessons.rs`; design in `docs/lessons_plan.md`):
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
  - **One lesson type breaks the "ordinary pipeline" rule above:**
    `PassCriteria::ScaleAdherence` (the improvisation lesson) has no chart
    notes to score and no natural end — it's an open `GameplayMode::
    JamSession`. `menu::pages::lessons::setup_lesson_reader`'s Start button
    routes it into `JamSession` instead of `Play2D`; `jam::improv::
    ImprovStats` (fresh-attack-gated, like `PitchGate`) accumulates
    scale/chord-tone adherence live via `classify_note_fit` — the same
    classification `jam::session::update_hole_map`'s tint uses, factored
    out so the two can't disagree — and a dedicated "Finish Lesson"
    pause-menu button (visible only for a jam session with a
    `LessonContext` in flight) judges it and returns to the menu on
    demand, via the same `apply_quit` path the ordinary Quit button uses;
    `route_menu_entry` sees the still-present `LessonContext` and routes
    to the lesson list, same as any other lesson. It never touches the
    results screen at all.
  - **Two more jam-based criteria join `ScaleAdherence`:**
    `PassCriteria::ChordToneAdherence` (stricter — only counts chord tones,
    not "merely in-scale") and `PassCriteria::PhraseDiscipline` ("did you
    leave space" — `jam::improv::in_rest_window` classifies each fresh
    attack against a fixed repeating play/rest bar pattern,
    `PHRASE_PLAY_BARS`/`PHRASE_REST_BARS`, against `gameplay::AbsoluteBar` —
    an absolute, non-wrapped bar count kept alongside `CurrentBar` since the
    pattern must repeat consistently across an open-ended jam rather than
    resetting every 12 bars). All three read different fields off the same
    always-accumulating `ImprovStats` (`chord_tone`/`in_scale`/
    `out_of_scale`/`rest_violations`); `menu::pages::lessons::is_jam_criteria`
    routes any of the three into `JamSession` the same way
    `ScaleAdherence` alone used to, and `gameplay::pause_menu::
    jam_fraction_for` picks the one relevant fraction for whichever
    criterion a given lesson declares before calling `lesson_passed`.
    Separately, `LessonManifest::progression` (an optional
    `"standard"`/`"quick-change"`/`"minor"` string, `menu::pages::lessons::
    parse_progression`) seeds `crate::app::JamProgression` on Start for any
    jam-based lesson, defaulting to `Standard` — same "don't let a stale
    pick linger" reasoning the real-song Jam Session button already
    applies.
  - **Call-and-response** (`gameplay::call_response`): a chart's consecutive
    `TrackItem::call: true` items are one phrase. Their notes are ordinary
    `ScheduledNote`s — scored the normal way — except each carries
    `force_wait: true`, which `tick_clock`'s freeze condition
    (`wait_freeze_index`) treats like `WaitForNoteMode` being on regardless
    of the player's own toggle, so the response always waits for them. At
    song setup those same notes are also synthesized (via `song_editor::
    playback`'s synth — `PhraseNote`/`render_pcm`/`encode_wav`, widened to
    `pub(crate)` for this) into a one-shot "call" demo, scheduled to finish
    playing a fixed buffer before the phrase's first note. That playback is
    a plain fire-and-forget `AudioPlayer` spawn, like a hit-feedback sound —
    it never touches `GameplayClock` or the sink, so it can't run afoul of
    the sink-anchoring invariant above; reusing the wait-freeze path (rather
    than inventing a clock-jump) is what keeps the whole feature anchoring-
    safe and self-pacing (a slow response just delays every later cue with
    it, since the clock can't reach them before it reaches the frozen note).
  - **Freeform call-and-response** (`jam::call_response`) is `gameplay::
    call_response`'s unscored, chart-free sibling: an opt-in toggle next to
    `jam::session::JamLoop` (`CallResponseEnabled`, off by default) that,
    while an open Jam Session runs, has the game play a short generated
    lick and gives the player a couple of bars to echo it by ear —
    deliberately not judged at all (no `PitchGate`/`ImprovStats` involved),
    since there's no authored phrase to score against. Paced by
    `AbsoluteBar` alone (`CALL_BARS`/`RESPONSE_BARS`, both dividing evenly
    into 12 so the cycle always lines up with a fresh chorus — the same
    reasoning `jam::improv`'s phrase-discipline pattern rests on) rather
    than a separate timer; a lick is a handful of MIDI pitches rolled from
    the pool of harp-producible notes that are tones of the bar's current
    chord (`JamHoleGuide::chord_tones_by_bar`/`note_to_holes`), rendered
    through the same `audio_system::synth` additive harmonica voice and
    fired the same fire-and-forget way `gameplay::call_response` does.
    Feedback is purely visual/turn-taking, not a score: a banner reading
    "Listen…"/"Your turn" (`CallResponseState::phase`), and the lick's
    holes ghost-highlighted on the live hole map
    (`jam::session::update_hole_map`, layered in only for a hole not
    already lit by a live pitch, so actually echoing a note still shows its
    normal chord-tone/in-scale tint) until the next call replaces them.
- **Guided tutorial tour** (`src/menu/tutorial.rs`): a "Tutorial" button on
  the Help/About menu drives a fixed sequence (`TOUR_STEPS`, each a
  `TourTarget`) on a timer, with a click-blocking overlay on top naming the
  current screen and briefly explaining it. Most steps are `TourTarget::
  Page` — the top-level, no-selection-required `MenuPage`s (Main, Play,
  Mode Select, Jam Session Menu, Generate Jam, Lessons, Options, Theme,
  Help/About; not `ArtistList`/`SongList`/`LessonReader`, which need an
  artist/song/lesson already picked) — but four steps actually enter live
  gameplay for a look:
  `TourTarget::Playing(GameplayMode::Play2D)` and `::JamSession` (both
  load the bundled `DEMO_SONG_PATH`, long enough that no step could ever
  run it to completion and trigger a real `AppState::Results`),
  `TourTarget::BendingTrainer`, and `TourTarget::SongEditor` — the exact
  same `AppState` transitions those screens' normal entry points use, so
  none of their own systems need to know a tour is happening.
  - **Crossing an `AppState` boundary back into `Menu` can't set
    `NextState<MenuPage>` directly** — same reason `ReturnToSongList`/
    `ReturnToOptions`/`ReturnToPlay`/`LessonContext`/`GeneratedJamSession`
    all exist as flags instead: setting it in the same tick as `NextState<AppState>`
    loses to the substate machinery resetting to its own default first.
    `enter_tour_target`'s `Page` case only ever queues `AppState::Menu`;
    `route_menu_entry` (extended to check the tour *first*, ahead of those
    other flags) reads `tour_menu_landing`/`tour_finished` to pick the
    right page and, once the tour has run its last step, actually remove
    the `TutorialTour` resource — `step == TOUR_STEPS.len()` is a one-frame
    "ending" sentinel `end_tutorial_tour` sets so `route_menu_entry` still
    sees the tour present (and can route to `return_to`) on that final pass.
  - The overlay's root entity is deliberately *not* `MenuRoot`/
    `GameplayRoot` — none of the screens the tour drives through despawn it
    as part of their own teardown; only the tour's own end logic does. Both
    tour-driving systems (`advance_tutorial_tour`/`sync_tutorial_overlay`)
    run unconditionally (each checks `Option<Res<TutorialTour>>` itself)
    rather than being gated to `AppState::Menu`, since some steps leave it.
  - `tour_active` (a `run_if` condition, `pub(crate)` precisely so other
    modules can gate on it without needing `TutorialTour`'s fields) is
    threaded into every Escape/pause handler a tour step could otherwise
    run into — `gameplay::pause_menu::handle_pause_input`,
    `gameplay::bending_trainer::handle_escape`,
    `song_editor::interaction::grid_keys` — so a tour can't be knocked off
    course by Esc while showing a live screen; the overlay's own "Skip
    Tutorial" button is the one deliberate way out.

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
- **Bevy 0.19 scene spawning:** use `WorldAssetRoot(handle)` for GLB/scene
  assets, not `SceneRoot`.
- **Localization is enforced:** user-visible strings must come from
  `loc.msg()` (Fluent); a `build.rs` scan + `LocalizedStr` newtype fail the
  build on raw literals. Locales: en-US, pt-BR, es-ES — add keys to all;
  `locales_define_the_same_keys` walks the directory and enforces parity.
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
- **Commits:** no `Co-Authored-By` trailer. Chart schema changes must stay
  backward compatible (new fields optional); bump `metadata.format_version`.

## Known open items

- Content: besides the Example Artist gameplay demos, bundled songs now
  include public-domain melodies (Greensleeves on a G harp, Jesu Joy and
  the Toccata in D minor on C harps, Für Elise on a C chromatic,
  "O Pulo da Gaita" transcribed from the Mr. Dirsom harmonica tab score,
  Amazing Grace, the Hallelujah chorus from Handel's Messiah on a D harp,
  and Mulher Rendeira). `tests/asset_layout.rs` schema-validates every
  bundled song chart. Deliberately skipped as still under copyright:
  Feira de Mangaio (Sivuca/Glorinha Gadelha) and Asa Branca (Luiz
  Gonzaga/Humberto Teixeira) — chart those yourself via Record mode
  instead of bundling a transcription.
- **Song editor color legend**: a third meta-form column
  (`meta_form::spawn_color_legend`) explains every color the editor uses,
  grouped by where it appears — note technique colors in the grid
  (`state::pitch_color`; direction is the ↑/↓ arrow glyph, not a color),
  the out-of-scale red tint, the selected-note border, drag-ghost valid/
  invalid, and the timeline/scrollbar colors — deliberately calling out
  that the scrollbar minimap's blue/orange means blow/draw
  (`interaction::SCROLLBAR_BLOW_COLOR`/`SCROLLBAR_DRAW_COLOR`), a
  different meaning than the grid note's blue (which means the Normal
  technique, regardless of blow/draw). Several colors that were private
  `const`s or local `let` bindings (`grid::OUT_OF_SCALE_TINT`/
  `TEMPO_MARKER_COLOR`, `timeline_overlay::SPLIT_LINE_COLOR`/
  `RANGE_HIGHLIGHT_COLOR`) were widened to `pub(super)` so the legend
  reuses the exact values instead of duplicating literals that could
  drift out of sync.
- **Song editor: selectable scale** (`song::chart::Scale`, a new chart
  field): the grid's out-of-scale red tint used to always mean "outside
  the blues scale rooted on the harp key" unconditionally
  (`blues_scale_classes(&state.key)`); it's now `state.scale.classes(&state.
  key)`, `state.scale` picked via a combobox (`meta_form::
  spawn_scale_combobox`) — six options: 1st/2nd/3rd position (the blues
  hexatonic, same shape as everywhere else, just rooted at the harp key
  \+0/+7/+2 semitones — the same offsets `Position::interval_below_jam_key`
  uses for Jam Session's harp-picking, just applied upward from the harp's
  own key instead of downward from a separate jam key, since a chart has
  no jam key distinct from its harp) and Major/Minor Pentatonic/Country
  (alternative *shapes*, always rooted on the harp key — for melodies that
  aren't blues-vocabulary at all; "Country" = major pentatonic, the
  scale 2nd-position cross-harp playing reaches without bending, per
  harmonica-pedagogy convention). `FirstPosition` (the default, used when
  a chart doesn't set `scale` at all) reproduces the old unconditional-
  blues behavior exactly — `first_position_matches_blues_scale_classes_
  exactly` pins this down. `harmonica.scale` is a new, schema-`enum`-
  validated field (unlike its free-string `position` sibling), added to
  both `Harmonica::Diatonic`/`::Chromatic`; `CURRENT_FORMAT_VERSION`
  bumped to 1.2.0 since an older build's stricter schema would otherwise
  reject a chart that actually sets it with a confusing raw validation
  error instead of the intended "needs a newer Harmonicon" message — a
  chart that never sets `scale` needs no version bump, unaffected either
  way. The combobox itself is spawned once into a reserved
  `ScaleComboboxSlot` (`spawn_scale_combobox`, a `Without<Children>`
  spawn-once gate, unlike the MIDI track combobox's rebuild-on-message
  pattern, since `Scale::all()`'s option list never changes at runtime);
  Load pushes a different value into the already-spawned combobox by
  writing `ComboboxValue` directly (`sync_scale_combobox_value`) — the
  widget's own documented escape hatch for exactly this, `dialogs::
  combobox`'s always-on `sync_combobox_visuals` picks the change up from
  there. No existing bundled chart sets `harmonica.scale` — all keep
  reading as 1st position, i.e. unchanged from before this feature.
  **`ScaleComboboxSlot` lives in the fixed chrome** (`ui::
  spawn_fixed_chrome`, above the mod panel — not the scrollable meta form
  the rest of the fields are in), a deliberate, load-bearing placement:
  `bevy_ui_widgets::Popover`'s dropdown list must be a literal ECS child of
  its toggle to compute its own position, and Bevy's UI overflow clipping
  follows that same ancestry rather than the popover's computed screen
  position — a combobox nested inside the form's `Overflow::scroll_y()`
  `ScrollArea` gets its open dropdown clipped to that scroll viewport no
  matter how high its `GlobalZIndex` is, rendering behind (and stealing
  clicks from) whatever's in the unclipped fixed chrome instead. The MIDI
  track combobox has this same latent constraint (it's also inside that
  `ScrollArea`) but hasn't surfaced as a visible bug yet — if it ever does,
  the fix is the same: move its slot out of the scrollable area too.
  **Fixing the clipping surfaced a second, separate bug in `dialogs::
  combobox` itself, affecting every combobox, not just Scale's**:
  `Pointer<Click>` auto-propagates up the entity hierarchy (every
  `bevy_picking` pointer event does, `#[entity_event(propagate =
  PointerTraversal, auto_propagate)]`) — clicking a dropdown item bubbled
  the same click up to the toggle button (`list`'s ancestor), whose own
  `toggle_click` observer then saw the popup `item_click` had *just* closed
  and immediately reopened it, so picking an item never visually closed
  the dropdown. Fixed by calling `ev.propagate(false)` in all three of the
  widget's own click observers (`toggle_click`/`backdrop_click`/
  `item_click`) — a modal widget shouldn't leak its own clicks to whatever
  it happens to be nested inside, regardless of this specific bug.
- **Lessons**: engine, all five primitives, and the full wave 1 + wave 2
  content pass (Units 1–3, 19 lessons) are shipped — see
  `docs/lessons_plan.md`. Left: Unit 4 "jazz", explicitly gated on the 0.6
  milestone (needs its own chord-tone tables and a jazz-blues
  `Progression` variant — not a blocking task for 0.4/0.5).
- Remaining 0.4 work (recorded backing loops) — see `ROADMAP.md`/`PLAN.md`.
