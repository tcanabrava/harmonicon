# harmonicon-editor

The Song Editor: Record/Edit/Play authoring for charts and lessons.

The largest crate, and a leaf — only `harmonicon-menu` registers it, and
nothing depends on it. `harmonicon-jam` is its sibling; neither imports
the other.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **`EditorState::effective_harp()` is the only harmonica the editor
  plays, draws, maps or saves.** A chart's authored `harmonica.layout` is
  kept on load as `loaded_harmonica: Option<LoadedHarmonica>` — the reeds
  plus the key and `HarmonicaKind` they were loaded for — and
  `effective_harp` returns it while `key` and `harmonica_kind` still match
  that identity, else the named preset from `playback::build_harp`. Before
  this, load identified the *kind* and threw the layout away, so saving a
  chart with one re-tuned reed silently regenerated the preset. Every
  pitch consumer (grid labels, audition, practice, playback, recording,
  notation, `serialize_harpchart`) takes the harp from `effective_harp`;
  **don't call `build_harp` from feature code** — its two legitimate
  callers are `effective_harp`'s own fallback and `pitch_map::
  suggest_key`, which scores every preset key as a *candidate*. Three
  rules keep it safe:
  - **Only an intentional instrument change drops the layout**:
    `set_key` and `set_harmonica_kind`, and only when the value actually
    changes — re-picking what's already selected keeps the reeds. Writing
    `key` directly bypasses the drop, which is why `effective_harp` also
    checks the identity itself rather than trusting the field to be
    `None`.
  - **MIDI import drops it unconditionally** (`apply_imported_track`),
    not via `set_key`: every imported pitch was resolved against the
    *preset* for the suggested key, so if that key happens to equal the
    chart's, `set_key` would keep reeds the imported holes weren't placed
    with.
  - **It's undo-tracked and cache-tracked** (`undo::Snapshot`,
    `grid_cache::Snapshot`) like `harmonica_kind`, for the same reason:
    it changes which pitch every hole means.

  **Bend depth comes from the reeds too**: `pitch_compatible(pitch, harp,
  hole)` and `max_bend(harp, hole)` (core) count the semitones between a
  hole's two reeds, so country tuning's raised draw 5 bends, Paddy
  Richter's hole 3 bends one semitone rather than Richter's three, and a
  custom layout gets whatever its widest reed pair gives. Before this
  they were a Richter table, which disagreed with the alternate tunings
  on nine holes in both directions — forbidding real bends and permitting
  impossible ones — and the same table capped gameplay's harp-substitution
  and the drill generator. `overblow_ok`/`overdraw_ok` deliberately stay
  by hole number: *which* holes get overblown is a convention of the
  instrument (1/4/5/6 and 7–10), not reed physics. The hole-free sticky
  cap is `interaction::deepest_bend(harp)`, asked of the effective harp
  at cycle time, not a constant.
- **The Song Editor can import a MIDI file** (`song_editor::midi_import`).
  The harmonica selector cycles through standard Richter, Paddy Richter,
  natural-minor, 12-hole chromatic, and 16-hole chromatic layouts. These use
  their matching core constructors; `bending_profile` round-trips the
  diatonic tuning and `harmonica.holes` round-trips the chromatic size.
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
  `state::HARP_KEYS` entry via `suggest_key`/`key_fit_score_for_harp`, built
  with the selected tuning and hole count (the fraction of the track's raw
  MIDI pitches landing on an exact blow/draw match —
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
- **Play renders its preview on a worker thread**
  (`playback::start_playback` → `PendingPlayback` →
  `finish_pending_playback`). A whole song takes tens of milliseconds to
  synthesize, so the render runs on `AsyncComputeTaskPool`. The synth
  track, the playhead and the background music all start on the frame it
  lands, so they stay aligned. The pending render is itself an
  `EditorAudio` entity, so every stop path that despawns editor audio
  also cancels it; a new stop path needs nothing extra.
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
  click, since there's nothing existing yet to extend onto). **Technique
  buttons act on the whole selection, with the primary note deciding the
  value** (`interaction::apply_modifier`, `cycle_depth`): the *primary*
  selection, `selected.last()` (`EditorState::selected_note`), steps the
  button's cycle exactly as it would alone, and every selected note then
  takes that value where its hole allows it — a bend's cap is the deepest
  any selected hole allows, and switching a technique off clears it only
  where it is. Notes that can't take it keep theirs and are counted into
  `technique_notice`, reported like a transposition
  (`report_technique_skips`); with one note selected the button still
  silently does nothing, as it always has. **Delete and Move act on the
  whole selection** too: `interaction::delete_selected` removes every
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
- **A note is resized by two grips *outside* it, not handles inside it**
  (`ui::ResizeGrip`, spawned by `interaction::spawn_resize_grips` and
  positioned by `update_resize_grips`/the pure `resize_grip_position`):
  small circles centred on the note's vertical middle, just beyond its
  left and right edges. **The note's own body is never a resize target**,
  which is the whole reason for the design — in-note handles and the note
  compete for the same pixels, and a 16th note (`3 * TICK_W - 2.0` =
  13px) has nowhere near enough for two grab targets plus a strip to drag
  it by. Two fixed 8px handles covered such a note whole, leaving it
  unmovable; capping them at a third of the note gave 4px targets nobody
  can hit. Out here a grip is `GRIP_D` px whatever the note's length.
  Three things this relies on:
  - **They're persistent, spawned once in `ui::setup`** — same reason
    `timeline_overlay::TimelineSurface` is (a rebuild mid-gesture
    despawns the entity `bevy_picking` captured the drag on). Exactly two
    exist for the session; they carry no note id and resolve one from
    `EditorState::selected_note` at `DragStart`. `rebuild_grid` only
    despawns `With<GridItem>`, which they deliberately are not.
  - **Shown only for a selection of exactly one note**, and never while
    `locked()`. With several selected a drag moves the group as a rigid
    shape (`DragState::group`), so "which note's edge" has no answer.
  - **A hidden grip is not a dead zone**: `bevy_ui`'s picking backend
    skips anything whose `InheritedVisibility` isn't true ("Nodes that
    are not rendered should not be interactable"), unlike tab navigation,
    which *does* reach invisible nodes and is why modals need
    `TabGroup::modal()`. The two subsystems differ here — don't assume
    one's rule from the other.

  The dev-only expected-notes layer keeps its own in-note handles
  (`expected_notes::EXPECTED_HANDLE_W`): its notes are authored at
  whole-beat lengths, so it never hits this.
- **Ctrl+C/Ctrl+V copy and paste the current selection**
  (`song_editor::clipboard`; wired in `interaction::handle_copy_paste`).
  `NoteClipboard` holds the last Ctrl+C'd notes verbatim — copying with
  nothing selected leaves a previous clipboard alone rather than
  clearing it. Ctrl+V reads the tick under the *mouse*, not a click —
  `GridArea` carries its own `RelativeCursorPosition` (added just for
  this) so `handle_copy_paste` can resolve a live hover position the
  same way a grid click already resolves its own tick, without needing
  a click first — and does nothing if the pointer isn't over the grid
  at all, or the clipboard is empty. Both the keyboard path and the
  mod panel's buttons go through `EditorState::copy_selection`/`paste`
  (`metadata_sync`), not `clipboard` directly — see the metadata bullet
  below for why. `clipboard::paste_targets_with_sources` (pure)
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
- **Transpose recomputes playable tab on the current harp**
  (`song_editor::transpose`). The ♯/♭ buttons and Ctrl+Up/Down shift the
  selection, or the whole chart when nothing is selected; Shift changes the
  interval to an octave. Resolve every destination through core's
  `pitch_map::playable_assignments`, trying easier fingerings first and then
  alternate holes when simultaneous notes compete. An unplayable or occupied
  destination leaves that note unchanged and the status bar reports it.
  Timing, ids, expression, and phrase metadata stay attached to the note, and
  the ordinary snapshot tracker makes the whole operation undoable.
- **The toolbar is two columns, and the right one is the per-note
  surface** (`mod_panel::spawn_mod_panel`; `ui::NoteColumn`). Left:
  the document and the tools — navigation, modes, lock, undo/redo,
  copy/paste, the timeline tools, metronome, legend, files. Right: **the
  note** — Blow/Draw, the pitch techniques, Wah/Vibrato, Depth, the
  phrase's Call/Split, the phrase editor, Delete. Named for what it *is*,
  not "the selection", because it never empties: `panel::update_mod_panel`
  shows the selected note's state when there is one and the sticky-armed
  defaults the *next* note gets otherwise, and a click both edits the
  selection and arms the sticky (the Word-bold-button pattern). That
  dual-mode rule is why per-note controls live here and nowhere else — a
  second per-note surface (a popup under the note was considered) would
  light up in two places for one note. What follows from the split:
  - **Side by side only in icon-only style** (`two_columns`): two 56 px
    columns, and the toolbar scrolls half as far. In the text styles two
    168 px columns would be a third of a small screen, so the note column
    stacks below the document column instead.
  - **The note column is Edit-mode content** and collapses to
    `Display::None` outside it, and `EditorToolbar` carries both widths so
    `panel::update_note_column` can hand the 56 px back to the grid in
    Record/Play — where the column would be empty.
  - **Depth is a stepped cycle** (`selected_metadata::next_depth_step`:
    ¼ ½ ¾ 1, wrapping), dual-mode like the Hz rate — the selected note's
    `expression_intensities` entry, or `sticky_intensity` for the next
    note. A chart may carry any 0–1 depth; the label shows it as-is
    (`depth_label`, a percentage) and a click steps *up* to the next
    quarter so it always visibly moves. `"0.5"` is the default and is
    never stored — stepping onto it removes the entry. A note with no
    expression shows no depth and ignores the click, the same "silently
    do nothing on an incompatible note" rule Overblow follows.
  - **Call/Split are phrase properties reached through the note**
    (`set_selected_call`/`set_selected_split`, keyed on the selected
    note's onset), so they highlight from the phrase, not from
    `mod_button_active`'s per-note fields, and have no sticky meaning.
  - **Phrase opens `phrase_editor` on the selected note's onset** — the
    way to annotate a phrase that has no marker on the lane to click yet.
  - `expected_notes`' dev-only layer ignores all four: its notes have no
    depth and belong to no phrase.
- **Every bulk edit to `notes` goes through `metadata_sync`, never a
  direct mutation.** Two metadata stores live *beside* the notes rather
  than on them: `phrase_annotations` (section/chord/groove/call/split),
  keyed by onset tick because it describes a *phrase* — the notes that
  start together — and `expression_intensities`, keyed by note id. A
  move, paste, delete or range edit that only touched `notes` left them
  pointing at ticks nothing starts on and ids nothing has; before
  `metadata_sync` existed, none of those edits touched
  `phrase_annotations` at all. The methods, and the rule each encodes:
  - `move_notes(&[(id, hole, tick)])` — the grid's drag-end, anchor and
    group in **one call**. An annotation moves with its phrase only when
    the *whole* phrase moves (every note at that onset) and only onto a
    tick with no phrase already; part of a phrase leaving keeps the label
    with the part that stayed, and a phrase joining an existing one keeps
    the existing label. Moving the group one note at a time would never
    see an onset as vacated.
  - `copy_selection()`/`paste(clip, tick)` — `NoteClipboard` carries
    intensities by *source* id and annotations by *source* tick, and
    paste re-keys them onto exactly what landed (a skipped note's
    metadata is skipped with it), never overwriting an annotation the
    destination already has.
  - `erase_notes_in`/`remove_range_closing_gap` — the timeline tools.
    Remove also shifts every later annotation back by the gap and drops
    those inside it. Tempo and meter points follow the same cut; the timing
    active at the old end is restored at the new join.
  - `drop_orphaned_metadata()` — drops annotations at ticks nothing
    starts on and intensities for ids nothing has. **`prune_selection`
    calls it**, so every removal path that already pruned the selection
    (delete, a harmonica-kind switch, a recording take punching out
    overlaps, undo, the timeline tools) cleans metadata up for free —
    which is also why a new removal path must call `prune_selection`.
  Undo snapshots both stores alongside `notes`, so an undone delete
  brings a phrase's label back with its notes.
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
  key/text pairs to add by hand to the lesson's own `locales/` in its pack,
  the same manual step authoring any lesson already requires. A lesson save also
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
  span — both driven entirely by `PointerDragStart`/`Drag`/`DragEnd`,
  deliberately **not** `PointerClick`: `bevy_picking` fires
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
    pointer motion (`PointerDrag::distance` ÷ `UiScale`, same as note
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
  (`harmonicon_core::snap`, re-exported as `song_editor::snap` by
  `mod.rs` — Bevy-free and unit-tested there; `EditorState::snap_mode` is
  still `state.rs`'s own field). `TICKS_PER_BEAT` (`harmonicon_core::
  synth`) is 12, not 4 — the
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
  together, not just its anchor; the resize-grip drag observer
  (`interaction::spawn_resize_grips`) snaps whichever
  edge moved after `state::apply_resize` computes it, then re-clamps to
  the same left/right-neighbor bounds `apply_resize` itself already
  enforced (snapping can push a value back out of them — e.g. snap the
  right edge forward past a following note it was already clamped
  against). `move_target`/`apply_resize` themselves stay snap-agnostic,
  pure pixel-to-tick conversions — snapping is a post-processing step
  applied at the call site, not a parameter threaded through them, so
  their own existing tests didn't need touching. `snap_mode` is a UI
  preference, not chart content or undo-tracked. **`snap_mode` also
  decides what the grid's background draws**, not just where a click
  lands: `snap::sub_beat_gridlines` returns the active mode's own
  `grid_points()` (minus tick 0, already the beat/bar line) with a
  `GridlineKind` tier for each, so only reachable positions get a line —
  `Half`/`Sixteenth` take `quarter_line`/`half_line`, `Triplet` takes the
  hue-distinct `SongEditorColors::triplet_line` (also in the color legend,
  `meta_form::spawn_color_legend`). Drawing *both* families at once
  instead divides a beat at ticks 3, 4, 6, 8 and 9 — two of the six gaps
  one tick (5px) wide and the rest three — which reads as neither a 2- nor
  a 3-way split, and marks positions the active mode can't reach.
  `snap::off_beat_labels` applies the same rule to the beat ruler's
  counting syllables, returning *localization keys* (not glyphs) since
  "&"/"a" are language-specific: `e`/`a` in pt-BR, `y`/`a` in es-ES. The
  two syllables name the same ticks in every mode, which is what makes a
  shuffle read as the triplet it is — ticks 0 and 8, the "1 … a" of
  "1 & a". Note `triplet_line` is the one `SongEditorColors` field whose
  weight matters *relative to nothing else on screen*: in Shuffle and
  Triplet it is the beat's only sub-beat line, so it belongs a shade under
  `half_line`, not brighter. Both bundled `theme.json`s must define it —
  `SongEditorColors` is `#[serde(default)]`, so a field a theme file omits
  silently takes the Rust default while its siblings come from the file,
  and `assets/themes/theme_schema.dtd.json` is `additionalProperties:
  false`, so a new colour needs an entry there too or every theme fails
  `asset_layout::theme_json_validates_against_schema`.
- **The grid's beat ruler is laid out in ticks, and numbers bars**
  (`grid::ruler_label`, driven by `EditorState::ticks_per_bar`/
  `ticks_per_signature_beat`). A bar line's label is the 1-based *bar*
  number in `colors.accent` at `BAR_LABEL_FONT`, every other beat its
  index within the bar in `colors.label` at the smaller
  `BEAT_LABEL_FONT` — the two can read as the same digit (beat 3 of bar 1
  vs. bar 3's downbeat), so colour and size are what tell them apart.
  Three things worth knowing:
  - **A bar boundary is not assumed to be a column boundary.** Lane cells
    are one *quarter note* wide, but a 7/8 bar is 42 ticks — three and a
    half of them — so its bar line genuinely falls mid-column. The ruler
    and the bar lines therefore run off their own tick loops after the
    column loop, not inside it. In 4/4 every position works out exactly
    where the column loop would have put it.
  - **`EditorState::meter` is the only place the editor reads the time
    signature**, and there is deliberately no `beats_per_bar` on it any
    more. There used to be one that rounded a bar to whole quarter notes
    — exact only when the meter's bar happens to be one; 7/8's 3.5 became
    4 and put every bar line half a beat out, and 3/8's 1.5 became 2 and
    made the metronome's accent walk around the bar. Everything bar-shaped
    (`ticks_per_bar`/`ticks_per_signature_beat` for the grid and
    timeline, `metronome::sync_tempo` and `count_in_secs` for the click)
    asks the `MusicScoreMeter` it returns. Gameplay's counterpart is
    `bars::chart_meter`; see that crate's notes for the four-way
    disagreement this closed.
  - **Beat numbers count the meter's own beat**, so 7/8 reads 1–7 and
    6/8 reads 1–6. Counting syllables (`snap::off_beat_labels`) stay
    *quarter*-relative, because they mirror the snap grid rather than the
    meter — and are skipped wherever one would collide with a numbered
    beat, which is all of them in a meter counted in eighths, where the
    "&" position *is* a beat.

  `timeline::describe_tick` shares the same tick math, or the confirm
    dialog would name a different bar than the ruler behind it.
- **Phrase metadata is visible in a dedicated header lane**
  (`annotation_lane.rs`). It reads `EditorState::phrase_annotations` directly
  and positions markers in the same absolute tick coordinate as notes. Section
  and chord text precede the compact call (`↩`) and tongue-block (`TB`) marks;
  marker width ends at the next anchor and clips overflow, with the full value
  available in a tooltip. Keep the lane separate from note rows so dense
  annotations cannot cover notes or their external resize grips.
  **Clicking a marker edits the phrase in place** (`phrase_editor.rs`):
  the marker is a real `WidgetButton` whose `Activate` calls
  `EditorState::open_phrase_editor(tick)` — which selects every note at
  that onset and sets `phrase_editor: Some(tick)` — and a popover with the
  Section/Chord/Groove/Lyric text boxes opens directly beneath it. Three facts
  about that popover:
  - **It is persistent, not a marker's child.** `bevy_ui_widgets::Popover`
    positions off its ECS parent, and markers are `GridItem`s respawned on
    every `rebuild_grid` — a text box childed to one loses focus mid-word.
    So it's spawned once in `ui::setup` (contents filled in by
    `populate_phrase_editor` on a later frame, the same spawn-once gate the
    Scale combobox uses) and positioned per frame by
    `update_phrase_editor`.
  - **It is a child of `GridArea`, not `GridContent`**, so its position is
    `tick * TICK_W - Scroll::px` in the area's own coordinates
    (`popover_left`, clamped to the area's width) with no global-transform
    conversion, and it neither scrolls away with the notes nor gets clipped
    at the grid's right edge.
  - **Every write goes through `EditorState::set_annotation(tick, …)`**,
    which refuses a tick no note starts on, and the popover closes itself
    the moment its onset has no notes (undo, delete, Remove) — an
    annotation without a phrase is exactly the orphan `metadata_sync`
    drops, so accepting text there would lose it at the next prune.
    Escape closes it first, before it deselects or leaves the editor
    (`interaction::grid_keys`). `phrase_editor` is a UI preference like
    `snap_mode`: not chart content, not undo-tracked, not in the grid
    cache.
- **Meter maps mirror tempo maps in editor state.** `time_signature` is the
  tick-zero value and `meter_changes` holds later `(tick, signature)` points;
  `EditorState::time_signature_map()` returns the sorted map with an explicit
  tick-zero point. Load and save rescale its anchors with the chart resolution,
  and undo/grid caching track the later points. The load validator accepts
  meter maps because every bar-shaped editor consumer now uses them.
  The timeline's Meter tool (`cycle_meter_point`) authors later changes:
  clicks snap to the active meter's beat, cycle through `TIME_SIGNATURES`,
  and remove a point when it cycles back to the preceding meter. Tick zero
  remains owned by the Details picker.
  The live metronome derives a clock local to the active meter segment, so
  every change starts with a downbeat even when it truncates the preceding
  bar. Record count-in stores the meter at the parked playhead and uses it for
  both its duration and click accents. Staff notation splits sustained notes
  at every meter-map bar boundary, and its displayed meter follows the active
  segment at the playhead (or the viewport while stopped). MIDI import reads
  time-signature events across every track and converts all their anchors to
  editor ticks, including events stored on a separate conductor track.
- **A pickup is typed in beats and lives in the meter map.**
  `EditorState::pickup_beats` holds the Details text (beats of the opening
  meter, decimals allowed); `pickup_ticks()` converts it, and `meter_map()`
  builds with `MeterMap::with_pickup`, which makes the pickup the end of an
  unnumbered bar 0. Everything bar-shaped already asks the map, so the ruler,
  bar lines, 12-bar tint, staff splitting and `describe_tick` follow for
  free — but a bar *number* must come from `MeterMap::bar_number`, never
  `bar + 1`, or bar 1 reads as 2. The metronome's segment clock adds the
  first segment's `phase_ticks` so the pickup clicks the bar's last beats.
  Like the tempo field it is not undo-tracked; the grid cache and the
  metronome's cached map compare it. Saved as `timing.pickup_ticks` (only
  when non-zero); MIDI import clears it.
- **Lyrics are a phrase annotation** (`PhraseAnnotation::lyric`, the chart
  item's `lyric`), since a syllable belongs to the notes starting together.
  Two rules the plain annotation fields don't have:
  - **The Lyric box takes a whole line** (`set_lyrics_from`): several
    space-separated syllables go one per onset from the open phrase on,
    `_` skipping one. A single syllable behaves like any other field.
  - **Saved once per onset.** Equal-onset notes of different lengths save
    as separate chart items; only the first carries the lyric
    (`serialize_harpchart_notes`), and `harmonicon_core::lyrics` also
    ignores a second syllable at the same instant, for charts from
    elsewhere.
  How lines are built and shown is `harmonicon_core::lyrics` and gameplay's
  `karaoke`; the editor only stores syllables.
- **Repeats are authored and kept as written; only the song loader plays
  them out** (`repeat_marks.rs`). `EditorState::repeats` is core's
  `Vec<Repeat>` in editor ticks, round-tripped through `timing.repeats`
  (rescaled on load like every other tick anchor). The Repeat and Ending
  buttons are *actions on the Select tool's span*, not timeline tools of
  their own, so the selection survives a press and a second press on the
  same bars counts another pass. Both snap the span to bar lines first
  (`bar_span`; tick 0 counts as one, so a tune with a pickup can repeat
  from the top). Three rules:
  - **The editor never expands.** Play, Practice, the staff and the
    metronome all run the written score once; `harmonicon_core::repeats::
    expand` belongs to the loader. Expanding here would make a save write
    the performance back as the written score.
  - **Endings get their passes from where they sit** (`fit_endings`):
    inside the passage, every pass but the last; starting on its sign, the
    last. That is re-applied whenever the repeat is edited, so a chart
    with hand-written passes keeps them only until the author touches it.
  - **They're undo- and cache-tracked** and follow Remove
    (`metadata_sync::remove_range_closing_gap` → `close_gap`); MIDI
    import clears them.
- **The meter is picked, never typed**
  (`meta_form::spawn_time_signature_combobox`, from
  `music_score::TIME_SIGNATURES`; there is no `Field::TimeSignature`). A
  time signature's lower number names a note value, so only a power of
  two belongs there — `parse_time_signature` only rejects what won't
  parse at all, so as free text `4/3` sailed through and was rounded into
  a bar length matching no meter. A list makes that unrepresentable
  rather than merely detectable. The slot lives in the *fixed chrome*
  beside the Scale picker for the dropdown-clipping reason
  `spawn_scale_combobox` documents. A chart on disk may hold a meter
  outside the list (it's the common ones, not all valid ones), so
  `sync_time_signature_combobox_value` writes `ComboboxValue` directly
  and displays it faithfully instead of snapping to a nearby option.
- **Difficulty and song feel are typed song settings.** Both use cycle rows
  in Details. Difficulty is always one of the schema's four values. Song feel
  is independent of placement `snap_mode`: `straight`/`shuffle` request a
  gameplay metronome subdivision, while `default` omits the optional field so
  loading the song leaves the player's current choice untouched.
- **`details_fields.rs` owns the song Details row registry and order.** Field
  behavior and values remain in `state.rs`/`meta_form.rs`; keep this registry
  separate so adding form rows does not make document state exceed its module
  budget.
- **Source, license, and description are typed metadata text fields** in
  Details. Blank source/license values omit their optional JSON properties;
  description defaults to the editor attribution used by newly created charts
  and uses a four-line, word-wrapped input that inserts newlines on Enter and
  commits on focus loss. `metadata.author` is held separately from the Details
  `Author` value (`song.artist`) so load/save does not conflate the two credits.
- **Scoring windows are typed as Details text fields** because authors enter
  exact millisecond values. Serialization accepts positive integers and falls
  back to the new-chart defaults (60/120/220 ms) for blank or invalid input;
  combo enabled/base/step/maximum/decay values are typed beside them. Invalid
  combo numbers fall back to schema-valid new-chart defaults. Style bonuses
  remain preserved JSON.
- **A note has at most one pitch technique and one expression.** A pair such
  as bend + vibrato is supported end to end and both earn scoring credit.
  Multiple pitch techniques have an ambiguous target, and multiple expressions
  cannot be reproduced by the synth, so load validation rejects each category
  with its phrase and event location.
- **Every real lesson/song chart must survive the editor.** The integration
  tests walk every `.harpchart` in `tests/fixtures`, `assets/songs` and — when
  checked out beside this repo — `../harmonicon-lessons` and
  `../harmonicon-songs` (`content_charts`), validate each source, load
  and serialize it, validate the saved result, and compare events, annotations,
  timing, instrument, scoring, loop, and metadata semantics after documented
  normalization. CI has only the fixtures; run the tests locally with the
  packs beside the repo before publishing a pack change that adds charts.
- **The status bar warns when the selected detector cannot score chords.** A
  duplicate `(tick, len)` among grid notes is exactly a multi-event item after
  serialization. If `AudioSettings::pitch_algorithm.is_polyphonic()` is false,
  the editor reuses gameplay's localized chord warning; active save/load,
  count-in, drag, and recording feedback take priority over it.
- **Loop settings are typed Details fields.** Type/repeat cycle through schema
  values; start/end accept inclusive phrase indices. Serialization clamps both
  to the current track and orders end at or after start, so note edits cannot
  leave a saved loop pointing outside the phrase list. `last` is the new-chart
  end sentinel and resolves to the final phrase at save time.
- **The 12-bar-blues lane tint is opt-in** (`EditorState::twelve_bar_tint`,
  a `dialogs::checkbox` in the meta form; off by default). When on,
  `grid::rebuild_grid` mixes `twelve_bar_grid::bar_bg` into each lane at
  `BAR_TINT_MIX`, tiling the standard I/IV/V form every 12 bars as the
  user scrolls. It is unconditionally `Progression::Standard`, which is
  why it defaults off: on a chart that isn't a 12-bar blues it colours the
  background with a progression the song doesn't have. Like `snap_mode`
  it's a UI preference — not chart content, not undo-tracked — but both
  **must be tracked in `grid_cache::Snapshot`**, since each changes what
  `rebuild_grid` draws and the cache is what decides whether it runs.
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
  `PointerClick` directly rather than reusing the Drag-based span
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
  audio synthesis (`harmonicon_core::synth::render_pcm`, which
  `song_editor::playback` shares with `gameplay::call_response`) still
  renders against one flat nominal BPM —
  the same already-accepted simplification `call_response` documents
  above for mid-phrase tempo automation. Extending the synth to follow a
  variable tempo map is future work, not a gap in this feature.
