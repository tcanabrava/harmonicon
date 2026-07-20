# Plan

Execution order and implementation notes for what's currently in flight.
Companion to `TODO.md` (the open checklist) and `ROADMAP.md` (the
destination). Once a phase ships, its detail belongs to git history, not
this file — prune it back to a one-line summary under "Shipped" below.

## Shipped

- **0.2 "Trustworthy"** — audio-synced clock, chart-derived detection
  range, mic device picker/retry, per-song persistence.
- **0.3 "Practice"** — A–B looping, practice speed, wait-for-note, tab
  display, shuffle metronome, bend trainer progression.
- **0.4, most of it** — adaptive difficulty, jam position/scale overlays,
  the full lessons engine plus a first content pass (Units 1–2),
  generated 12-bar backing ("Generate Jam"), selectable jam progressions
  (standard / quick-change / minor) and playing positions (1st/2nd/3rd —
  `song::harmonica::Position::harp_key` picks the matching cross-harp key),
  and freeform (unscored) call-and-response practice in Jam Session
  (`jam::call_response` — an opt-in toggle plays a generated chord-tone
  lick, then a turn-taking banner + hole-map ghost highlight cue the
  player's echo; no scoring, distinct from the chart-scripted
  call-and-response *lesson* primitive in `gameplay::call_response`).
  Architectural invariants live in `CLAUDE.md`; the curriculum design
  lives in `docs/lessons_plan.md`.
- **Lessons content wave 2** — Unit 1 basics extensions (breathing,
  charted bends, vibrato, articulation), Unit 2 bar-counting and
  train-rhythm drills, and a new Unit 3 blues-vocabulary unit (licks via
  call-and-response, then chord-tone/minor-blues/phrase-discipline
  improvisation) — 19 lessons total, plus the three engine items wave 2
  needed: `PassCriteria::ChordToneAdherence`/`PhraseDiscipline`
  (`jam::improv::in_rest_window`/`ImprovStats::rest_violations`), and the
  lesson manifest's `progression` field. See `docs/lessons_plan.md`.
- **Physical-design restructuring** (`docs/physical_design_plan.md`) —
  all 6 phases done: layering inversions fixed (`app.rs`, `audio_system::
  synth`), inline test blobs evicted to sibling `tests.rs` files,
  `gameplay/mod.rs` and `menu/mod.rs` split into their target layouts,
  the jam feature gathered into `src/jam/`, `src/lessons.rs` split into
  `lessons/{manifest,catalog,progress}.rs`, and `tests/physical_design.rs`
  now enforces the file-size budget going forward.
- **Song editor UI responsiveness pass** — tooltip clamped to the window
  (`dialogs::tooltip`), the editor's content wrapped in a scrollable
  column (`song_editor::ui::setup`), the mod panel split into a fixed
  transport strip + `flex_wrap: Wrap` tool strip, and `transport_button`'s
  colors routed through `SongEditorColors` — plus the accompanying
  `song_editor/ui.rs` split into `panel_widgets.rs`/`mod_panel.rs`/
  `meta_form.rs`/`ui.rs`. The meta form was later reworked again into two
  side-by-side columns (`meta_form::spawn_form_column`, split at
  `FIELDS.len() / 2`) after the first (flex-wrap) pass didn't read well;
  the note grid also gained its own horizontal scrollbar
  (`song_editor::interaction::update_grid_scrollbar`/`drag_grid_scrollbar`),
  hidden unless the song's notes actually run wider than the visible area.
- **0.5 (early): MIDI import key suggestion** — `song_editor::midi_import`
  now auto-picks the `HARP_KEYS` entry needing the fewest bend/slide/
  nearest-note fallbacks for the track being imported
  (`suggest_key`/`key_fit_score`), instead of importing onto whatever key
  the editor happened to already be set to. `bin/midi_to_chart` (the
  separate, simpler, C-diatonic-only CLI converter this used to leave
  untouched) has since been removed entirely — see the code-duplication
  cleanup entry below for where its shared parsing logic ended up.
- **0.5: song editor variable tempo maps** — the grid header shows the
  chart's referenced music file as a peak-amplitude waveform
  (`song_editor::waveform`, reusing `audio_system::waveform`'s existing
  decoders), windowed to whatever's currently scrolled into view like the
  note grid itself, and now aligned against a real multi-point tempo map
  rather than one constant BPM. A new Tempo timeline tool
  (`TimelineTool::Tempo`, `state::toggle_tempo_point`) lets the editor
  add/step/remove tempo-change points directly on the timeline, rendered
  as markers on the grid header; `EditorState::tempo_changes` +
  `state::build_tempo_map`/`bpm_at` back it, `song::chart::seconds_to_tick`
  (new — inverse of the existing `tick_to_seconds`) is the shared tick↔time
  conversion, and `harpchart.rs` save/load round-trips the full tempo map
  (including rescaling a foreign `timing.resolution`, e.g. a MIDI-derived
  chart, into the editor's own tick units instead of silently
  mis-scaling it — a pre-existing load bug this fixed as a side effect).
  MIDI import now carries a track's real tempo changes into
  `tempo_changes` instead of collapsing to one average BPM
  (`midi_import::editor_tempo_map`). Scope is deliberately limited to the
  editor's grid/waveform display and the chart's on-disk tempo map — Play/
  Practice/Record audio synthesis (`song_editor::playback`'s `render_pcm`)
  still renders against one flat nominal BPM, matching `gameplay::
  call_response`'s already-documented simplification; see `ROADMAP.md`'s
  0.5 section. Per-technique playback effects (a 0.5 candidate raised
  earlier) was explicitly declined — real product-feel decision (a new
  sound on every scored note), not something to guess at unprompted.
- **0.5: live auto-refresh of the external song/theme/lesson folders** —
  `assets_management::watch` starts one recursive `notify-debouncer-full`
  watcher (same crate Bevy's own `file_watcher` feature uses internally,
  added as our own direct, always-on dependency rather than flipping that
  Bevy feature — see `CLAUDE.md`'s Asset sources bullet for why) on
  `~/Harmonicon` at Startup, if it exists, and fires one generic
  `ExternalFolderChanged{top_level_dirs}` message per debounced batch
  (`watch::changed_top_level_dirs`, pure/unit-tested) — deliberately
  agnostic of what `songs`/`themes`/`lessons` mean, since this module is
  low-level shared vocabulary and those are feature concerns above it.
  `assets_management::mod.rs` consumes it for `songs`/`themes`
  (`rescan_on_external_change`); `lessons::catalog` consumes the same
  message for `lessons` from the other side
  (`rescan_lessons_on_external_change`) — a `lessons`-depends-on-
  `assets_management` edge, not the reverse. Every scan function fully
  replaces its resource's contents rather than appending (`scan_all_songs`
  didn't always — fixed; `scan_ui_themes`/`scan_lessons` already did), so
  each is safe to call again at runtime. A successful live rescan fires
  its own specific `SongsRescanned`/`ThemesRescanned`/`LessonsRescanned`
  message (not a bare resource-change poll: the consuming page's own
  change-detection tick goes stale while the page is closed and would
  misfire as "changed" on every re-entry); the Artist List, Theme picker,
  and Lessons list pages each consume theirs to force a same-page rebuild
  (`NextState::set` re-fires `OnExit`/`OnEnter`, same pattern the pause
  menu's Restart already relies on) if that page happens to be open when a
  drop-in happens. No manual refresh button, no restart. Lessons also
  gained bundled-plus-external scanning itself (`lessons::catalog::
  scan_all_lessons`, mirroring `scan_all_songs`'s pattern — bundled first,
  external tagged `external://lessons`), which didn't exist before this.
  The rest of that roadmap item — actual downloadable/community song packs
  — is still open; see `ROADMAP.md`'s 0.5 section.
- **Options: fullscreen toggle** — `settings::FullscreenEnabled`
  (persisted, off by default) plus `settings::apply_fullscreen` mirroring
  it onto the primary window's `WindowMode` (borderless, not exclusive
  fullscreen); a pill-button toggle on the Options page, same shape as the
  adaptive-difficulty toggle.
- **0.5: Song editor can author lessons** — `EditorState::content_kind`
  (`ContentKind::Song`/`Lesson`, `song_editor::state`) is a "Record Song"/
  "Record Lesson" toggle button (`meta_form::spawn_content_kind_row`,
  same click-to-cycle shape as the harmonica-kind toggle). While Lesson is
  active, `song_editor::lesson_form::spawn_lesson_form` shows a second
  panel (`LessonFormGroup`, shown/hidden via `Node::display` like
  `EditModeGroup`/`PerformModeGroup` already are) with one row per
  `state::LESSON_FIELDS` — lesson id, unit, explanation, prerequisites
  (comma-separated), pass-criteria kind/threshold/technique, and
  progression — reusing `meta_form::spawn_field_row` (made `pub(super)`)
  for every row, including three new click-to-cycle fields alongside
  Key/Position (extracted their shared advance-through-a-const-array logic
  into `state::cycle_next`, used by all five now). Save/Load dispatch on
  `content_kind`: `harpchart::handle_save_chosen`/`handle_load_chosen`
  (Song) and `lesson_form::handle_save_lesson_chosen`/
  `handle_load_lesson_chosen` (Lesson) each skip the other's `ContentKind`,
  so exactly one acts per `FileChosen` message; the Lesson path builds a
  schema-shaped `lesson.json` (`lesson_form::serialize_lesson`, validated
  against `lesson_schema.dtd.json` via `lessons::parse_lesson` before
  writing — a warning if it doesn't pass, not a silent invalid write) and,
  if the editor has any notes, also writes `song/chart.harpchart` next to
  it via the ordinary `harpchart::serialize_harpchart` — the same
  `"chart": "song/chart.harpchart"` convention every shipped lesson uses.
  Loading a `lesson.json` round-trips the fields back
  (`lesson_form::populate_from_lesson_manifest`) and loads its chart too,
  if it has one. **Scope boundaries, deliberate**: `title_key`/`body_key`
  are derived from the lesson id, never round-tripped as raw text (they're
  keys, not values — this codebase's localization convention); the
  Explanation/title text an author types has nowhere to be written as a
  real translation, so `serialize_lesson` prints the key/text pairs to
  add to the locale files by hand instead. A lesson save also doesn't run
  `harpchart::save_midi_backing` (the MIDI-import backing-track
  convenience) — author the chart as a plain song first if it needs MIDI
  backing, then switch to Lesson mode to add the curriculum fields.

- **Code-duplication cleanup** (whole-tree duplicate-block scan,
  2026-07-19) — all 6 phases done, no behavior changes, `cargo test`/
  `cargo clippy`/`tests/physical_design.rs` clean throughout:
  `gameplay::notes::build_scheduled_notes` (+ `play_mode_label`) replaces
  `gameplay_2d`/`gameplay_3d`'s own near-identical note builders, and
  `adaptive_difficulty::rebuild_song_notes` replaces their duplicated
  `resync_notes_on_adaptive_change` middles; `gameplay_2d::{harp_pitches,
  step_hole_glow}` is now the shared per-cell glow step
  `update_holes`/`update_holes_3d` both call, and
  `gameplay_2d::spawn_blow_draw_legend` is the shared blow/draw legend
  (`note_tint`/`update_note_visuals`'s 2D/3D pairs were deliberately left
  separate — genuinely different render targets, no real savings from
  unifying); `gameplay::harmonica_overlay::spawn_diagram` is one
  parameterized grid builder behind all three harmonica-diagram spawners;
  `song_editor::playback` gained shared `secs_per_tick`/`playhead_for`/
  `spawn_background_music` used by `playback.rs`/`practice.rs`/
  `record.rs`, `song_editor::state::overlapping_group` replaced the
  duplicated transitive-overlap walk in `enforce_direction`/`enforce_expr`,
  and `meta_form::spawn_cycle_row`/`panel_widgets::spawn_button_shell`
  unified the click-to-cycle and plain-button scaffolds respectively; MIDI
  parsing (`tick_to_seconds`/`collect_tempo_map`/`track_name_of`/
  `note_on_count`/`extract_notes`) is now `song::midi`, a new public
  module `song_editor::midi_import` builds on (this used to also be shared
  with `bin/midi_to_chart`, since removed entirely — the Song Editor's own
  MIDI import covers the same ground in-game; the editor's own
  MIDI-tempo-map conversion, formerly the standalone `midi_parse.rs`, has
  since been folded directly into `midi_import.rs`, its only caller); small
  menu/UI fry (`results::spawn_stat_row`, `options`'s slider-row scaffold,
  `jam_generate`'s stepper rows, `menu::pages::lessons`'s reader-line
  spawn) each collapsed to one shared local helper; and the literal
  duplicate `richter_harp` reference-layout tests in
  `bending_trainer/tests.rs` were deleted in favor of `song::harmonica::
  tests`'s copies (the trivial `World::new()`/`Schedule::default()`
  scaffolding repeated across test files was deliberately left alone —
  already the established idiom everywhere, not worth an abstraction for
  two lines).
- **Build-time message-registration check** — `build.rs` now statically
  scans for every `#[derive(Message)]` type and fails the build if it's
  never registered with `.add_message::<T>()` anywhere, the same class of
  bug that shipped once (`ExternalFolderChanged`, fixed in
  `assets_management/mod.rs`) and only surfaced as a runtime panic the
  first time its `MessageReader`/`MessageWriter` system actually ran. Same
  static/textual approach as the existing localization-literal scan; see
  `CLAUDE.md`'s "Message registration is enforced" bullet.
- **Song editor: lesson-details panel UX pass** — `Record Lesson`'s 8
  extra fields didn't fit an ordinary window and gave no hint anything ran
  below it. Fixed three ways: `LessonThreshold`/`LessonTechnique` rows now
  hide unless `lesson_pass_criteria` actually needs them
  (`LessonConditionalRow`, `update_lesson_conditional_rows`); the fields
  split into the same two-column layout `meta_form::spawn_form_column`
  already gives the song fields (curriculum-identity fields vs. the
  pass-criteria cluster, `LESSON_FIELDS.len() / 2`); and the whole body
  now sits behind a "▸ Lesson Details" header, collapsed by default
  (`LessonDetailsBody`/`LessonDetailsToggleLabel`,
  `EditorState::lesson_details_expanded`). Separately, the editor's form
  area (meta form + lesson form + status bar) gained a real, visible
  vertical scrollbar (new `song_editor::scroll` module — `bevy_ui_widgets`'
  `Scrollbar`/`ScrollbarThumb`, hidden whenever everything already fits) —
  a first pass wrapped the grid in the same `ScrollArea`, which let
  scrolling the grid's own horizontal scrollbar also drag the vertical one
  (and vice versa) on a small window; the grid row and mod panel were
  pulled back out into fixed chrome above it (`ui::spawn_fixed_chrome`),
  and `GridArea`'s new `Hovered` component gates
  `interaction::pan_wheel` so the grid only pans horizontally while the
  pointer is actually over it.
- **Packaging CI fixes** — `flatpak.yaml`'s build was failing on a fresh
  runner (but not locally) because `flatpak-builder` shells out to the
  host's `eu-strip` (from `elfutils`) after the build to split/strip
  debuginfo, and `apt-get install --no-install-recommends` was skipping it
  (only a `Recommends` of the `flatpak-builder` package, not a hard
  dependency) — fixed by installing `elfutils` explicitly. Separately,
  macOS packaging gained the same "catch it on every push, not just at a
  tag" treatment `flatpak.yaml` already gave Linux: a new
  `.github/workflows/macos.yaml` builds the release binary, assembles the
  same bare `.app` bundle `release.yaml`'s tag-triggered
  `release-macOS-intel`/`release-macOS-apple-silicon` jobs already produce,
  and `hdiutil`-packages it into a `.dmg` (native-arch only — Apple
  Silicon, since this is a packaging-regression check, not a release
  artifact), uploading the result as a short-retention build artifact.
- **Record-mode detection robustness + latency** — live recording
  (`song_editor::record`) no longer trusts the raw detector: `start_record`
  narrows `PitchRange` to the selected harp (as gameplay already did from a
  chart) and precomputes a 128-entry MIDI→(hole, dir, pitch) table from
  `pitch_map::map_pitch_playable` (new no-fallback sibling of `map_pitch` —
  both now in `song_editor::pitch_map`, split out of `midi_import`), so
  detections the harp can't produce are discarded instead of snapped onto
  the grid; onsets/releases are debounced (a one-chunk blip is deleted
  again, a one-chunk dropout doesn't split a held note); and onset
  timestamps subtract the detection delay (half analysis window + the
  calibrated `input_latency_ms`) so takes land where they were played.
  `mpm_pitch` also gained an absolute clarity floor (`MPM_MIN_CLARITY`) —
  it previously reported a "pitch" for any breath noise loud enough to pass
  the RMS silence gate, since its 0.9 clarity threshold was only relative
  to the frame's own strongest lag. See `CLAUDE.md`'s recording bullet and
  `record.rs`'s module docs.
- **Song editor: scroll-while-selecting on the timeline** — the Select
  tool's span now lives in its own `TimelineSelection` resource (same
  separation/reason as `Scroll`), the ruler's drag catcher
  (`TimelineSurface`) is a persistent entity synced to the viewport
  instead of respawned by every grid rebuild, and `rebuild_grid`'s
  early-return guard shrank to note drags only — so wheel-panning the grid
  mid-selection rebuilds/spawns the notes it scrolls into view, and the
  span end tracks pointer motion *plus* scroll delta
  (`timeline::drag_end_tick`, `sync_selection_with_scroll` for
  scroll-only frames). The two-click split flow now produces a selection
  too (it previously placed a marker that nothing consumed), and
  `timeline.rs` split its overlay/rendering half into
  `timeline_overlay.rs` to stay under the file-size budget. See
  `CLAUDE.md`'s timeline-tools bullet.

## Current work

Finishing 0.4:

1. **Backing track variety, remainder** (0.4): recorded loops per style
   (shuffle, slow blues, swing) as a richer alternative to the generated
   bass — real audio content, not a code task.
2. **Lessons Unit 4 "jazz"** is explicitly gated on the 0.6 milestone
   (`ROADMAP.md`), not part of finishing 0.4.

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
