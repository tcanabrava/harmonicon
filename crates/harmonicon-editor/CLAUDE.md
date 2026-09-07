# harmonicon-editor

The Song Editor: Record/Edit/Play authoring for charts and lessons.

The largest crate, and a leaf — only `harmonicon-menu` registers it, and
nothing depends on it. `harmonicon-jam` is its sibling; neither imports
the other.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

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
  bend within `max_bend`'s per-hole cap (diatonic), an overblow/overdraw,
  or a slide (chromatic, one semitone up), else the nearest playable note
  — reusing `state::pitch_compatible` so an import can never produce a
  note the editor's own UI wouldn't allow. (**The resolution itself lives
  in `harmonicon_core::pitch_map`**, where gameplay and score importers
  can reach it too; `song_editor::pitch_map` is the thin adapter
  translating core's `HoleAssignment` into the editor's own `Dir`/`Pitch`
  — the vocabulary the mod-panel buttons produce and a `GridNote` stores.
  It keeps the `map_pitch` / no-fallback `map_pitch_playable` /
  `suggest_key` names its two callers already used, MIDI import and live
  recording, which want opposite fallback behaviour; see the recording
  bullet below. `state`'s `max_bend`/`overblow_ok`/`overdraw_ok`/
  `HARP_KEYS` are re-exports of core's and `pitch_compatible` delegates to
  `technique_fits_hole`, so what the UI lets you place cannot drift from
  what the resolver considers reachable. Core's resolver also reaches
  overblows and overdraws, which the editor's own never did — those
  pitches previously fell through to the nearest-note fallback, so a MIDI
  import could silently relocate a note the harp could actually have
  played.) The key itself isn't just whatever was
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
  **Which track is the harmonica is not the editor's question to answer**:
  `default_track` defers to `harmonicon_score::pick_harmonica_track`, the
  same rule that applies when a `.mid` is played directly as a song
  (`harmonicon_song::song::score_song`), so the two can't disagree about
  what a harmonica part is called. A *named* match is imported the moment
  the file loads rather than merely pre-selected — picking the value a
  combobox already displays fires no `ComboboxSelect`, so pre-selecting
  alone would leave the grid empty behind the right answer. Failing a
  name, the picker opens on the first track *with notes* rather than
  track 0, which conventionally carries only tempo and metadata; the
  editor deliberately keeps the fallback the asset loader can't have —
  an ambiguous file shows a chooser, where a loader has nowhere to ask.
  Track listing goes through `MidiScore` too, which means a MIDI with no
  notes in any track is now refused with that reason instead of opening
  a picker full of empty tracks.
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
- **The Song Editor has a click track and a Record count-in**
  (`song_editor::metronome`). Reuses `gameplay::metronome_overlay`'s pure
  tick math and the same global `MetronomeTempo`/`MetronomeFeel`/
  `MetronomeMuted`/`MetronomeSounds` resources gameplay and the Bending
  Trainer already share (so a player's mute preference carries over
  instead of resetting) — but not that module's own click-driving
  systems, which are tied to `GameplayClock`; the editor has its own
  clock (`playback::Playhead`), so `metronome::click_metronome` reads
  from that instead, sharing only the actual click-selection/audio-spawn
  logic via a new `metronome_overlay::play_click_if_due` (extracted from
  that module's own `click_metronome` so both clocks drive the exact
  same click behavior). `metronome::sync_tempo` keeps `MetronomeTempo`
  seeded from `EditorState::tempo` continuously (not just once
  `OnEnter`, since the field is itself live-editable) — safe to write
  unconditionally since gameplay/the Bending Trainer/the editor are
  different `AppState`s and only one is ever active. Pressing Play on a
  *fresh* Record take (not resuming a paused one) doesn't call
  `record::start_record` immediately: `metronome::begin_count_in` arms a
  `CountIn` resource for one bar's worth of clicks first (`tick_count_in`
  counts it down and clicks against its own elapsed-since-start clock,
  since `Playhead` isn't running yet), and `finish_count_in` hands off to
  `start_record` for real the instant it reaches zero — split into two
  systems purely because one system with every parameter both steps need
  would exceed Bevy's per-system parameter limit. `record::stop_record`
  also cancels a pending count-in unconditionally (nothing has actually
  started recording yet at that point, but leaving it ticking would
  silently start a take moments after the player asked to stop), so
  every one of its existing callers (the Stop/Finish buttons, every
  mode-switch-away-from-Record) gets that for free. The status bar shows
  a "get ready" countdown while counting in, ahead of the drag/record/
  practice messages `panel::update_status_bar` already prioritized.
- **The Song Editor auditions a note's pitch on selection**
  (`song_editor::audition`): the instant `EditorState::selected_note`
  (the primary selection) changes to a *different* note id,
  `audition_on_select` renders a short (0.6 s) blip of its resolved
  pitch and plays it — confirming a bend/overblow/overdraw actually
  sounds like what was intended without running Play/Practice or
  reaching for a real harp. Reuses `harmonicon_core::synth`'s additive
  harmonica voice via `playback::note_freq`/`render_pcm` — the same
  synth Play/Practice/Record preview already render with — rather than
  a separate reference-tone generator (unlike the Bending Trainer's own
  "Listen" button, which predates this synth and still uses its own
  simpler sine-only tone), so the audition matches what the note
  actually sounds like in context. The `PhraseNote` it builds sets
  `tick: 0` and hands `AUDITION_SECS` to `render_pcm` as `secs_per_tick`
  with `len: 1` — a trick that renders exactly `AUDITION_SECS` of audio
  regardless of the note's own on-grid duration, since audition is
  "how long you need to hear it to judge it," not "how long it plays in
  the song." Deliberately scoped to selection *changing* — clicking an
  already-selected note again doesn't replay it (a plain "play this
  note again" action this doesn't attempt to be), and every selection
  call site (a fresh placement, clicking an existing note, Ctrl+click,
  paste) already funnels through the same `selected_note`, so none of
  them need touching.
- **Save/Load outcomes show up in the status bar, not just the log**
  (`song_editor::save_feedback`). `harpchart::handle_save_chosen`/
  `handle_load_chosen` and `lesson_form::handle_save_lesson_chosen`/
  `handle_load_lesson_chosen` used to report every outcome with a bare
  `println!` — invisible in a normal, non-terminal launch of a packaged
  build. Each now also calls `SaveFeedback::set` with a localized
  success/warning/failure message, displayed by `panel::
  update_status_bar` as its own highest-priority tier (above even a
  count-in) for `save_feedback::DISPLAY_SECS` (4 s) before falling back
  to whatever the bar would otherwise show; every outcome is still
  logged via `info!`/`warn!` too; for developers running from a
  terminal, that's strictly more visible than the old `println!` (structured,
  filterable). `lesson_form::serialize_lesson` now returns its
  validation warnings (empty id/unit, or the manifest not passing its
  own schema) as `Vec<String>` instead of printing them directly —
  `save_lesson` folds them into the save's own status ("saved with
  warnings" instead of a plain "Saved" when there's something to flag)
  — except the locale-key-pairs-to-add reminder, deliberately left as a
  console-only `println!`: it's a multi-line block meant to be
  copy-pasted into a `.ftl` file, not a one-line status. A save/load's
  *secondary* outcome (the MIDI-backing/processed-MIDI bonus files
  `harpchart::save_midi_backing` writes; a lesson's own chart write)
  stays log-only too, same "primary vs. secondary" split — the status
  bar reports whether the thing the player actually clicked Save/Load
  for worked, not every file touched along the way.
- **The Song Editor supports multi-note selection**: `EditorState::
  selected` is a `Vec<u32>`, not a single `Option<u32>` — a plain click
  replaces it wholesale (`select_only`), Ctrl+click toggles one note in
  or out without disturbing the rest (`toggle_selected`,
  `interaction::select_or_add_ctrl` — the Ctrl+click sibling of
  `select_or_add`; Ctrl+clicking empty space still behaves like a plain
  click, since there's nothing existing yet to extend onto). Mod-panel
  technique edits (Bend, Overblow, ...) still act on one note — the
  *primary* selection, `selected.last()` (`EditorState::selected_note`/
  `_mut`) — since editing several notes' pitch technique at once has no
  obviously-correct single meaning, but **Delete and Move act on the
  whole selection**: `interaction::delete_selected` removes every
  selected note, and dragging any note that's part of a multi-selection
  (more than one selected, the dragged note among them) moves the whole
  group together, preserving relative offsets — `DragState::group`
  carries every *other* selected note's original position, and
  `grid::group_move_targets`/`group_move_valid` shift/validate them by
  the same delta the dragged anchor moved by. Deliberately one combined
  validity check across the anchor *and* the group (not the anchor via
  an overlap check against the group's stale positions, plus the group
  checked separately) — two same-hole notes swapping past each other as
  a rigid pair would otherwise falsely read as a collision, since each
  one's *target* would land on the *other's* not-yet-moved spot. A
  second, dynamically spawned/despawned ghost per non-anchor member
  (`GroupMoveGhost`/`update_group_move_ghosts`, rebuilt every frame like
  `update_scrollbar_markers`) previews the group during the drag, next
  to the anchor's own persistent `MoveGhost`.
- **Ctrl+C/Ctrl+V copy and paste the current selection**
  (`song_editor::clipboard`; wired in `interaction::handle_copy_paste`).
  `NoteClipboard` holds the last Ctrl+C'd notes verbatim — copying with
  nothing selected leaves a previous clipboard alone rather than
  clearing it. Ctrl+V reads the tick under the *mouse*, not a click —
  `GridArea` carries its own `RelativeCursorPosition` (added just for
  this) so `handle_copy_paste` can resolve a live hover position the
  same way a grid click already resolves its own tick, without needing
  a click first — and does nothing if the pointer isn't over the grid
  at all, or the clipboard is empty. `clipboard::paste_targets` (pure)
  lands the clipboard's own *earliest* note at that tick and shifts
  every other member by the same offset it had from that earliest one,
  preserving the copied shape; holes never change, since paste is
  keyed on "when", not "which hole". Each note is silently skipped
  (not forced) if its hole doesn't exist on the current harp or its
  computed target would collide with an existing note — pasting where
  nothing fits is a no-op for that one note, same "silently skip" spirit
  as `select_or_add`'s sticky-pitch fallback. Ids are always freshly
  assigned (never the clipboard's own copied ids, which would collide
  with the originals still sitting in `EditorState::notes`), and the
  pasted notes become the new selection — ready to drag into place
  immediately, the same way a fresh `select_or_add` selects what it
  just placed.
- **Ctrl+Z/Ctrl+Y undo and redo** (`song_editor::undo`; keyboard wired in
  `interaction::handle_undo_redo`, buttons in `mod_panel`). Snapshot-
  based, not command-based: `UndoHistory::record_if_changed` runs every
  frame `EditorState` changes and diffs its content (`notes` +
  `tempo_changes` only — deliberately narrower than `EditorState` itself,
  excluding transient fields like `selected`/`scroll_beat`/`dragging`)
  against the last-seen snapshot, pushing the *previous* one onto the
  undo stack only when they actually differ. This needs no
  instrumentation at each note-mutating call site (a grid click, a drag
  release, Delete, paste, Erase/Remove, MIDI import, ...) — whichever of
  them just ran, the diff catches it on the next check, since `undo`/
  `redo` themselves also keep the cached "last" snapshot in sync (so a
  `track_changes` pass immediately after either is a correctly-detected
  no-op, not a spurious extra step or a redo-stack-clearing "new edit").
  The one deliberate exception is live recording
  (`record::RecordState::active`): a note's length grows every single
  frame while a take is running, so diffing continuously would flood the
  history with one entry per frame — `track_changes` simply skips while
  a take is active, so the *entire* take (onset through Stop/Finish,
  including any pauses) becomes one undo step. Capped at 100 entries
  (`undo::HISTORY_LIMIT`); a fresh edit after an undo clears the redo
  stack, same rule every undo implementation follows. The Undo/Redo
  buttons dim (not disable) when their stack is empty
  (`panel::update_undo_redo_buttons`) — clicking still no-ops either way,
  same as the keyboard shortcut, just with a visible signal it won't do
  anything.
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
- **The Song Editor's grid supports a swing/triplet-aware snap mode**
  (`song_editor::snap`, split out of `state.rs` purely for that file's
  line budget — `EditorState::snap_mode` is still `state.rs`'s own
  field). `TICKS_PER_BEAT` (`harmonicon_core::synth`) is 12, not 4 — the
  lowest resolution divisible by both 4 (straight 16ths, the old
  resolution) and 3 (triplets): a true triplet position doesn't exist as
  an integer tick on a 4-ticks-per-beat grid at all, which is why this
  needed a resolution change rather than a snapping tweak. This is
  forward-only: a chart's own `timing.resolution` is self-describing and
  every tick/time conversion (`song::chart::tick_to_seconds`/
  `seconds_to_tick`) already takes a chart's resolution as an explicit
  parameter rather than hardcoding the constant, so existing bundled
  charts (still at `resolution: 4`) load and play unchanged — only new
  saves write the finer resolution. A Straight/Shuffle/Triplet toggle
  (`SnapMode`, next to the harmonica-kind toggle in the meta form)
  constrains which within-beat tick a click lands on:
  `SnapMode::Sixteenth` (ticks 0/3/6/9 — reproduces the old
  any-of-4-ticks grid exactly, just at the new resolution),
  `SnapMode::Shuffle` (0/8, a 2:1 long-short swing pair — the classic
  blues shuffle bounce), `SnapMode::Triplet` (0/4/8, three equal
  subdivisions) — `snap_tick_in_beat` picks whichever of `SnapMode::
  grid_points()` is nearest the raw click fraction, for a *new* note.
  Dragging an *existing* note (move or resize) snaps too, via a second
  pure function, `snap_absolute_tick` — unlike `snap_tick_in_beat`'s
  fractional 0.0..1.0 input (a click's offset within one beat cell),
  this one snaps an already-absolute tick, so it has to consider
  crossing a beat boundary: `grid_points()` always includes 0, so the
  current beat's own points plus the *next* beat's tick 0 are the only
  candidates that can ever be nearest (the previous beat's last point
  never is — it's always farther than the current beat's own 0). The
  move-drag observer snaps the anchor's tick immediately after computing
  it (`grid::move_target`), before deriving the multi-select group's
  shared tick delta from it, so a dragged group moves onto the grid
  together, not just its anchor; the resize-drag observer snaps whichever
  edge moved after `grid::apply_resize` computes it, then re-clamps to
  the same left/right-neighbor bounds `apply_resize` itself already
  enforced (snapping can push a value back out of them — e.g. snap the
  right edge forward past a following note it was already clamped
  against). `move_target`/`apply_resize` themselves stay snap-agnostic,
  pure pixel-to-tick conversions — snapping is a post-processing step
  applied at the call site, not a parameter threaded through them, so
  their own existing tests didn't need touching. `snap_mode` is a UI
  preference, not chart content or undo-tracked. The grid's own sub-beat
  gridlines are tiered by color rather than one line per raw tick (which
  at resolution 12 would be 11 lines per beat, unreadably cluttered):
  straight-16th positions keep `quarter_line`/`half_line`, triplet
  positions get a new, hue-distinct `SongEditorColors::triplet_line` —
  also called out in the color legend (`meta_form::spawn_color_legend`).
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
  `harmonicon_audio::waveform`'s existing decoders (the same ones a shipped
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
