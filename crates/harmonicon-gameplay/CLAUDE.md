# harmonicon-gameplay

Scored play: the audio-anchored clock, note judging, the 2D/3D highways,
HUD overlays and the Bending Trainer.

Owns the clock/bar vocabulary and the overlay spawners that
`harmonicon-jam` and `harmonicon-editor` both build on, which is why it
sits below them. It must never reach *up* into either — anything they
both need lives here or lower.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **`GameplayRoot` goes on top-level entities only.** It is the marker
  `lifecycle::cleanup_gameplay` sweeps on `OnExit(AppState::Playing)`, and
  `despawn` is recursive — so a *child* that also carries it gets removed
  by its ancestor's despawn first, and its own despawn then lands on a
  dead entity. Bevy 0.20 reports that through the command error handler
  (`Entity despawned: the entity with ID … is invalid`, once per entity);
  0.19 passed it silently, which is why `beat_guides`' pool carried the
  marker unnoticed. Anything spawned inside a `with_children` block under
  a `GameplayRoot` needs no marker of its own.

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
  Jam Session — **and always under wasm** (`SINK_POSITION_IS_RELIABLE`).
  Anchoring assumes the sink position is effectively a *hardware* clock,
  advanced by the audio device's own callback no matter how busy the game
  is; that's true natively and false in a browser, where cpal's WebAudio
  backend pulls samples from rodio inside **main-thread** callbacks
  (`setTimeout` to prime it, then each `AudioBufferSourceNode`'s `ended`
  event) and schedules them ahead onto the WebAudio timeline. A Bevy frame
  loop saturates that same thread, so the counter slips behind wall-clock
  while the audio itself plays on — and once it slips past the 0.5 s
  threshold, `advance_clock` reads it as a stall/seek and snaps the clock
  *backwards* to meet it, so the song visibly rewinds ~half a second, over
  and over. Anchoring defends against slow drift; it is not worth a
  repeated backwards jump. Two invariants, the second enforced by the type
  rather than just documented:
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

- **A chart's meter is read in exactly one place: `bars::chart_meter`.**
  It returns a `harmonicon_ui::music_score::MusicScoreMeter`, applying the
  `timing.time_signature_map`-at-tick-0-beats-`song.time_signature`
  precedence, and every bar-shaped number in this crate, `harmonicon-jam`
  and the metronome derives from that value through its methods —
  `beats_per_bar()` for quarter-note beats (what `split_at_bar_lines` and
  the tick clock want, 3.0 for 6/8), `numerator` for the meter's own beat
  count (what the HUD's beat dots and the downbeat accent want, 6 for
  6/8), `bar_secs(bpm)`/`beat_secs(bpm)` for seconds. **Don't derive one
  of those yourself** — hold the meter and ask. `MetronomeTempo` and
  `ScoringConfig` both carry the meter for exactly this reason, not a
  number computed from it: taking the numerator as a beat count makes
  6/8 accent every second bar. Three things that follow:
  - **The metronome clicks the meter's own beat, not a quarter note.**
    `MusicScoreMeter::beat_secs` is `60 / bpm × 4 / denominator` — an
    eighth's worth in 6/8. This is what makes odd meters accentable at
    all: a 3/8 bar is one and a half quarter-note clicks, so no quarter
    click ever lands on its downbeat. `bpm` itself still counts quarter
    notes everywhere (`tick_to_seconds` defines it so).
  - **`dialogs::metronome::tick_index` takes a beat *length*, not a
    BPM.** Passing `60 / bpm` silently re-assumes x/4; the one caller that
    does so (the lesson reader's metronome widget) does it on purpose and
    says why — that widget has a BPM and a beat count and no meter, so
    its beat genuinely is its BPM beat.
  - **A pickup moves the bar grid, never the clock.** `timing.pickup_ticks`
    is the end of an unnumbered bar 0, so the grid starts
    `bars::pickup_lead_ticks` before tick 0. Beat guides pass that lead to
    `beat_ticks_in_range`, the metronome holds it as
    `MetronomeTempo::lead_beats` (read through `grid_clock`), both staff
    bridges add it to every notation beat, since the staff counts bars from
    beat 0, and `track_current_bar` subtracts the pickup so the 12-bar
    cycle starts on bar 1. `GameplayClock`, note times and judging are
    untouched. Anything else that counts bars or beats needs the same
    lead, or it disagrees with the metronome on exactly the charts that
    have one.
  - **Compound-meter *feel* is still open.** 6/8 now accents the right
    downbeat and clicks six eighths to the bar, but it does not group them
    3+3 as two dotted-quarter pulses; that's a `MetronomeFeel`-shaped
    design decision, not a bar-length one, and wasn't folded in here.

- **Bending Trainer pitch feedback is target-family locked.** Lifecycle,
  the target model and the technique hints stay in `bending_trainer/mod.rs`;
  where things sit on screen is `bending_trainer/layout.rs`; live feedback
  and readiness checks live in `bending_trainer/feedback.rs`; short pitch
  history and stability math live in `bending_trainer/trace.rs`; pure practice
  state machines live in `bending_trainer/gesture.rs`; adaptive target selection
  and scoring live in `bending_trainer/drill.rs`; the diagram's keyboard path
  and progress bars live in `bending_trainer/diagram.rs`; the Advanced drawer
  lives in `bending_trainer/advanced.rs`. A detected pitch
  must match a playable note in the selected hole before either the tuner or
  drill computes target cents. Keep octave errors and unrelated harmonics as
  explicit wrong-pitch feedback instead of turning them into large cents
  values or successful attempts. The optional natural check confirms the
  selected target's source reed and never counts as a completed bend. The
  natural-to-target rail consumes the same `BendTrace`; keep its overshoot
  visible and derive its tolerance band from the judging tolerance.

- **The trainer's reference settings enter the pitch maths at exactly one
  point.** The A4 reference and the per-harp/hole measured reed centre are
  both plain *additive cents shifts*, so `feedback::reference_shift_cents`
  folds them into one number that `tuner_observation` subtracts from the
  distance it reports. Everything downstream — tuner readout, bend rail,
  gesture state machines, drill — reads that one value, so they cannot
  disagree about where the target is. **Intervals within a hole are
  deliberately excluded**: the rail's natural-to-target span and its
  intermediate slot positions are ratios of two table entries, unaffected by
  either shift, and subtracting it there would double-count. Everything
  adjustable lives in `harmonicon_platform::settings::BendingTrainerSettings`
  (one resource, not one per knob — see its doc comment). Two things that
  follow:
  - **`clamped` runs on load, not only on edit.** `settings.json` is a file
    a player can edit, and a `tolerance_cents` of `0` would otherwise reach
    the maths as a divide-by-zero.
  - **`trace_smoothing` is display-only.** `smoothed_cents` filters the
    drawn path; `residual_rms`, `attempt_stability` and `vibrato` always run
    on raw samples, or "steadiness" would become a function of a display
    knob.

- **The drill measures practice, not accuracy.** `DrillStat::weight` reads
  recent control, recent steadiness and staleness — never the lifetime hit
  rate, which lets an early run of beginner failures dominate a target
  forever. Three consequences worth keeping:
  - **A timeout with nothing heard is a *skip*, not a miss**
    (`DrillOutcome::Skipped` → `record_skip`). It counts and stamps the
    target as seen, but moves no estimate of control: putting the harp down
    is not evidence about bending. Only `record_attempt` touches
    `attempts`/`hits`, which is what the progress map's `drill_accuracy`
    reads.
  - **Staleness is a drill ordinal, not a timestamp.** `practiced_at` is
    compared against `next_sequence`, which derives "now" from the stamps
    themselves. `SystemTime::now()` panics on `wasm32-unknown-unknown`, and
    a wall clock would also have needed a counter kept in step across
    save/load.
  - **The drill advances on the practice shape's terminal state, not on a
    hold** (`drill_outcome`). Only `PracticeShape::Free` finishes on a bare
    `DRILL_HOLD_TO_ADVANCE`; for every structured shape the hold is a
    *stage*, so advancing there would swap the target out mid-gesture — and
    waiting for `GesturePhase::Complete` is also what stops one noisy frame
    from churning the target. This is why `drill_update` is ordered
    `.after(update_gesture_practice)`.

- **The Bending Trainer's layout follows orientation live, and its
  drawers float.** Landscape puts the diagram beside the target card and
  portrait stacks it below. `apply_trainer_orientation` rewrites the body's
  `flex_direction` every frame from the window's aspect, because a tablet
  gets rotated mid-session and the direction is the only difference between
  the two layouts. That makes this screen an exception to the
  read-once-at-setup `CompactLayout` convention. The Setup and Advanced
  drawers are absolutely positioned `dialogs::drawer::Drawer`s with
  `GlobalZIndex(2)`: in flow they reflowed the live trace, and a scrolling
  column would clip the Key/Detect dropdowns. Two consequences:
  - **The diagram is one Tab stop** (`TabIndex` on `OverlayHost`), navigated
    with the arrow keys by `navigate_diagram`. Movement follows the grid as
    *drawn* (`step_target`), so Right from hole 6's draw bend finds nothing,
    because holes 7–10 bend on the blow side. A click and Enter/Space both go
    through `choose_cell`.
  - **`attach_progress_bars` runs in `PostUpdate`.** `rebuild_overlay`
    despawns and respawns the cells through deferred commands, and on the
    first frame (`TrainerKey` reads as changed) an `Update`-scheduled
    decoration queued inserts on cells that were gone by the time they
    landed, which panics. Anything else that decorates the diagram's cells
    must follow the same rule.

- **`collect_pitches` is registered twice in `Update`** — once in
  `GameplayLogic`, once for the Bending Trainer — so `.after(collect_pitches)`
  names an ambiguous `SystemTypeSet` and panics when the schedule is built.
  The trainer's copy is in `TrainerPitchSet` for its readers to order
  against; same rule as `MusicVolumeSet`, and it applies *within* a crate,
  not just across them.

- **Two schedule defects here are invisible to the compiler and only fire
  when the screen is first entered**: the ambiguous-set panic above, and
  B0001 (a system taking one component mutably in two of its own queries
  with no `Without` proving them disjoint — `update_bend_rail` has four
  `&mut Node` and four `&mut Text` queries, so every pair needs one).
  `gameplay::tests::every_schedule_the_gameplay_plugin_builds_initializes`
  runs `Schedule::initialize` over the real `GameplayPlugin`, which performs
  exactly those two checks and needs no resources, window or GPU — add
  systems to the plugin, not to a mirror list, and it keeps covering them.

- **Scoring:** pure functions in `harmonicon-core`'s `scoring` (reachable
  as `harmonicon_core::scoring`, shared by
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
  only — every path below still resolves as `harmonicon_gameplay::gameplay::X`. Key
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
    mean offset — only when the timing histogram says the lean is real
    (`coaching::latency_suggestion`, below).
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
    supposed to have company). Unlike `clean_attack`, this needed no dedicated `SongStats` bucket —
    `chord_is_sounding` gates `Hit` itself, so an out-of-sync chord already
    reads as a plain miss in ordinary accuracy.
  - There's a headless end-to-end test driving `score_notes` with a
    scripted pitch stream (`end_to_end_synthetic_song_drives_score_combo_
    and_stats`) — extend it when changing scoring behaviour.

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

- **One score readout, two modes.** `hud::spawn_score_readout` spawns the
  `ScoreText`/`ComboText`/`FeedbackText`/`FeedbackDetailText` markers, and
  both `gameplay_2d` and `gameplay_3d` call it, so the two modes can't
  drift into looking like different games. Composition is shared; only the
  `ScoreReadoutAnchor` differs, because the hit line is a different kind of
  thing in each: 2D's is the bottom of a real UI node (so the readout is a
  child of the highway, offset by `HIT_H_PCT`), while 3D's is a mesh at
  `HIT_Z` whose screen position comes from the fixed camera (so it's a
  measured `HIT_PLANE_BOTTOM_PCT` of window height — **re-measure that if
  the 3D camera ever moves**). The readout hugs the two edges of its band
  and leaves the middle clear, so it doesn't sit on top of the lanes a note
  actually falls down.

- **The scored-play HUD is one spawner, not two.** `hud_panel::
  spawn_hud_panel` builds the side panel — song title, phrase banner, tab
  ribbon, metronome, technique legend — and both modes call it, on the same
  side of the screen, in the same order. Likewise
  `hud_panel::used_modifiers`: both flattened `chart.track` themselves, and a
  technique legend that disagrees with the notes is worse than none.
  - **The one honest difference is the blow/draw key**, and it follows from
    the lane surface rather than from drift: 2D prints it under its hole
    strip, beside the colours it explains; 3D has no strip, so it asks for
    the key inside the panel (`HudPanel::blow_draw_legend`).
  - 3D's panel uses `theme::HUD_PANEL_BG`, not its own near-black. 2D
    darkens the whole screen behind its panel; 3D shows the song's artwork
    through, so a lighter wash left the technique legend unreadable over a
    bright background.
  - Still mode-specific and deliberately so: 3D has no hole map, and the
    beat guides are 2D-only (its lane is world-space geometry with no UI
    node to hang percentages off).
  - **Which contextual pieces appear is decided in one place**,
    `hud_panel::contextual_panels`: the side panel, notation staff,
    progress-bar note markers, technique legend and the in-panel blow/draw
    key, from the lane surface, compact layout, an aural lesson and whether
    the chart uses techniques. Both setups read its answer, and its tests
    pin each rule — including that the modes differ only in the key. A new
    conditional piece belongs there, not in an `if` in either setup.
  - **The karaoke strip** (`karaoke.rs`) is one of those pieces: shown
    whenever the chart has lyrics, compact and aural included, under the
    staff or in its place. It draws `harmonicon_core::lyrics`' answer and
    decides nothing: lines, word joins and "how far through is the singer"
    are pure functions there. It reads the *expanded* chart, so a
    repeated passage sings its words again. The current line is one `Text`
    with a sung and an unsung `TextSpan`, rewritten only when the
    `KaraokePosition` changes, which also covers an A–B loop rewinding.

- **Beat guides reuse the note's own time→screen path, not a second one**
  (`gameplay/beat_guides.rs`, 2D only — the 3D lane is world-space geometry
  with no UI highway to hang percentages off). A guide's position is
  `bars::beat_ticks_in_range` (off `chart_meter`, so 6/8 gets six eighths to
  the bar) → `chart::seconds_to_tick`/`tick_to_seconds` (which honour the
  tempo map) → `gameplay_2d::note_attack_pct` (the same mapping the
  notes get). A local `60.0 / bpm` anywhere in that chain puts the guides
  somewhere the notes aren't, on exactly the charts where a visible pulse
  would have earned its keep. Two implementation notes:
  - **They carry no `GlobalZIndex`.** `GlobalZIndex(0)` reads like "behind
    the notes" and instead drops them below the gameplay root's own
    `GlobalZIndex(1)` background, which paints over them — the trap
    `spawn_gameplay_music_score` documents. Child order already puts them
    behind notes, which join the highway later.
  - Entities are a fixed pool repositioned each frame, not spawned per beat.

- **Score HUD is message-driven, not polled:** `score_notes` emits a
  `NoteScored` message at the instant a note is judged;
  `update_score_display` is a `MessageReader` consumer, not a per-frame
  `format!` into `Text`. Follow this pattern for any future HUD element
  whose trigger is a discrete scoring event rather than a
  continuously-varying value. What the message carries is a
  `JudgmentFeedback` (`state.rs`) — *the scorer's own decision*, not a
  quality code the UI re-interprets:
  - `Hit { quality, offset }` carries the same signed offset the scorer
    classified from and fed to `SongStats::offset_sum`, so the HUD's
    `EARLY`/`LATE` split costs no second timing calculation and cannot
    disagree with the stats. `None` means `Score` moved without anything to
    say at the hit line (a sustain payout, combo decay).
  - **Miss attribution accumulates while the note is pending, on
    `ScheduledNote::miss_evidence`** — it is *not* computed when the miss
    window elapses. By then the offending pitch has almost always stopped
    sounding, so classifying from that last frame alone reports nearly every
    miss as `NoAttack`. `judge::observed_failure` runs each frame from the
    same `harp_pitches`/`PitchGate` state the hit test on that frame uses,
    and the first frame that blames something wins. Anything that resets a
    note's score state must clear it too (`handle_loop_boundary` does;
    `carry_over_note_state` carries it).
  - Both `WrongPitch` and `IncompleteChord` require a *fresh* attack
    somewhere in the frame. A pitch merely held over from an earlier note is
    a missing attack, not a wrong one — reporting `WRONG NOTE` there would
    coach the player to change notes when the real fix is to re-articulate.
  - `WrongPitch` carries the expected and heard tabs, so the HUD's second
    line can say *wanted 4↑ · heard 4↓*. The heard pitch is resolved to a
    hole in the judge (`pitch_map::map_pitch_playable` against
    `PlayedHarp`), not in the HUD — the UI formats, it doesn't resolve.
    Several pitches can be attacked at once and they arrive from a
    `HashSet`, so `nearest_attacked` picks by a rule (closest to the target,
    ties low) rather than taking the first; iteration order would otherwise
    make the same frame report differently on different runs.
  - **`PlayedHarp` and `ValidHarpNotes` are built together** by
    `ValidHarpNotes::for_played_harp`, which returns both. `ValidHarpNotes`
    answers "may this pitch score"; `PlayedHarp` answers "which hole makes
    it". Both 2D and 3D setup go through that one call precisely so they
    cannot resolve the `EffectiveHarmonica` substitution differently.
  - **Nothing on screen reads `chart.harmonica` for the instrument.** The
    hole strip's notes, the hole glow (`update_holes`/`update_holes_3d`,
    off `PlayedHarp` rather than re-fetching the manifest), the lane
    count, `SongInfo`'s harp line, the countdown's "grab a G harp" hint and
    Jam Session's hole map all take `effective.harp_for(chart)` — a
    substituted harp has other notes in the same holes, and a strip
    labelled with the chart's would contradict the judge. The hint's key
    is `EffectiveHarmonica::song_key_for`: under `SameHoles` the tune moves
    with the harp (C chart on a G harp sounds in G), under `Transpose` it
    doesn't. The one legitimate `chart.harmonica` read left in the note
    path is `harp_remap::remap_event`'s *source* harp.

- **The song-progress bar is a per-hole note-lanes strip, with the phrase
  overlay painted over it, and its timescale survives a music-less song**
  (`gameplay::song_progress_overlay`, shared by Play 2D/3D and Jam
  Session). The strip below the waveform spans the harmonica's whole hole
  range as `hole_count` equal lanes — the highest hole at the *top*, the
  lowest (hole 1) at the *bottom* (`note_lane_geometry`; the opposite
  vertical order from the Song Editor's own scrollbar minimap, which the
  rest of this design otherwise mirrors) — with one rectangle per note in
  its own hole's lane: left/width from `note_marker_geometry` (proportional
  to the note's own duration, floored so a very short note doesn't
  vanish), tinted blue (blow) or orange (draw), the same "note as a
  proportional colored rect" language `song_editor::interaction::
  scrollbar_marker` established for the Song Editor's scrollbar minimap.
  The per-phrase adaptive-
  difficulty rectangles are painted as a translucent *overlay* on that
  same strip (spawned first, so the note markers stay legible on top),
  not a separate row — one load-bearing
  consequence: a loop-range drag can now only start in the waveform band,
  since a phrase rect covering part of the note-lanes strip intercepts
  clicks there (see `spawn_song_progress`'s own comment on the trade-off).
  Note markers are `NoteMarker{time, duration, hole, is_blow}` — a small
  type decoupled from any richer one a caller has on hand, since callers
  differ: 2D/3D map it straight from `ScheduledNote`, but Jam Session has
  no `SongNotes` at all (nothing is scored there) and instead flattens the
  chart's own `TrackItem`s to one marker per *event* (matching the scored
  modes' own per-event granularity, so a chord/split item's notes each get
  their own marker in their own lane). Separately, `spawn_song_progress`'s
  `duration_secs` is normally `SongManifest::music_duration_secs`, but a
  chart with no backing track (`SongManifest::music: None`) has nothing
  decoded to measure there — `0.0` — even though the chart itself still
  has a real length; `effective_duration` falls back to the furthest
  extent of the passed-in note markers/phrase sections in that case, so
  the bar (playhead sweep, phrase overlay, note lanes) still lays out
  against something real instead of reading as empty. Only the waveform
  row itself stays blank in that case — there's genuinely no waveform
  data without decoded audio.

- **The results screen decides nothing; `gameplay::coaching` does.** Every
  judgment on it — the one observation, the timing lean, whether the
  Input-lag button is offered, the technique ranking, lesson progress, the
  range "Practice missed section" loops — is a pure function over
  `SongStats` with a minimum sample size, and `results.rs` only renders
  what those return. Keep it that way: a new thing to say to the player is
  a new `Observation` variant with a test for its floor, not a branch in
  the spawner. Two things that follow:
  - **`SongStats::timing` is a histogram, recorded beside `offset_sum` in
    `score_notes`.** The mean alone can't tell a consistently-late player
    from a scattered one, and only the former should be handed an Input-lag
    change (`latency_suggestion`: lopsided, enough hits, mean worth a
    click). Anything else that reasons about timing should read the
    buckets, not re-derive from the mean.
  - **"Practice missed section" is the A–B loop, entered from outside the
    song.** `PracticeRequest` is set by the results button and consumed in
    two places: `setup_scoring_config` copies the range into `LoopConfig`
    (after the chart's own default, so it wins), and `start_at_practice_
    range` — in the `GameplayLogic` chain right after `tick_clock` — jumps
    the clock via `rewind_to` the frame the music sink exists, having first
    marked every note before the range as resolved (`skip_notes_before`) so
    the judge doesn't tally the skipped stretch as misses. It has to wait
    for the sink (spawned by `update_countdown` through `Commands`, so a
    frame late) because a clock jump without the matching seek is exactly
    the anchoring violation described above. `SongEnd` is always the
    chart's real end — a loop doesn't make it infinite, `detect_song_end`
    just waits on `LoopConfig` — so the progress bar keeps its playhead and
    loop marker while looping, and clearing the loop mid-song still reaches
    Results.

- **What a judged note does on the highway is decided once, in
  `gameplay::note_feedback`, and applied by both renderers.** Both draw a
  note as a ribbon (`note_ribbon_2d`/`note_ribbon_3d`, with the technique
  drawing shared in `note_ribbon::ribbon_technique`). The pop/shrink curve
  (`judged_scale`), the ✓/✗ stamp, and the ribbon's `hold` uniform
  (`hold_uniform`) are pure functions; `gameplay_2d::animate_judged_notes`
  and `gameplay_3d::animate_judged_notes_3d` apply the curve to the
  ribbon's *width only* — its length is the note's duration and must stay
  true — through layout in 2D (a transform would squash the cap label) and
  `Transform::scale.x` in 3D. Four things that follow:
  - **The transition is observed, not the state.** Each note visual carries
    a `JudgedState`; the renderer compares it with `judged_now(note)` and
    only on a change inserts/removes `Judged { hit, at }` and rewrites the
    label. That's what makes an A–B loop clean — `handle_loop_boundary`
    clears `hit`/`missed`, the state reads the change back to `None`, and
    the ribbon is restored — and why nothing re-derives the animation from
    `ScheduledNote` per frame.
  - **Techniques are drawn in note time, so they cross the hit line at the
    charted rate.** A vibrato's crests and a wah's pinches are spaced by
    `oscillation_hz` along the ribbon — per second of note in 2D, per world
    unit at `lane_speed` in 3D — so the ribbon itself is the pulse to
    follow. Anything else drawn along a ribbon should use the same
    convention, or it will scroll at a rate that means nothing.
  - **The ribbon shows hold *state*, not a fill.** It scrolls through the hit
    line time-accurately, so the part already credited is below the line
    and off-screen within a fraction of a second; a fill was tried and is
    invisible. The part above the line shows whether the expected pitch is
    sounding this frame, how much of the hold so far was credited, and the
    sustained technique's live status (`judge::live_technique_status`, the
    same samples the end-of-hold verdict reads, with "not measurable yet"
    distinct from "heard at the wrong rate"). Elapsed time is measured on
    `judge::judged_instant` — the one definition of "when did this attack
    happen", shared by the judge, the autoplayer and the renderers —
    because `held` only starts once the judge sees the hit; measured on the
    raw clock, every hold read as having lost its start.
  - **A label that becomes an icon needs the font fallback to run after
    it.** `dialogs::font_fallback` runs in `PostUpdate` before
    `UiSystems::Content` for exactly this reason; in `Update` it was
    unordered against the writer and the ✓ stamp drew one frame of tofu.
- **`gameplay::autoplay` is the dev-only way to get hits without a mic**
  (`#[cfg(feature = "dev")]`, `brpctl.py autoplay on [late_ms]`). It
  writes `ActivePitches`/`AudioFrame` right after `collect_pitches` in the
  `GameplayLogic` chain — never earlier, or a live mic's events would
  overwrite it — and sounds notes on `judged_instant`, so a run reads as
  on-time rather than early by the filter's onset lag. Anything a hit shows
  (the ribbon pop, hold state, the results timing bar) is verified through it;
  what it cannot verify is detection of a real harmonica.

- **Call-and-response** (`gameplay::call_response`): a chart's consecutive
  `TrackItem::call: true` items are one phrase. Their notes are ordinary
  `ScheduledNote`s — scored the normal way — except each carries
  `force_wait: true`, which `tick_clock`'s freeze condition
  (`wait_freeze_index`) treats like `WaitForNoteMode` being on regardless
  of the player's own toggle, so the response always waits for them. At
  song setup those same notes are also synthesized (via
  `harmonicon_core::synth`'s `PhraseNote`/`render_pcm` and
  `harmonicon_core::wav::encode_wav`) into a one-shot "call" demo, scheduled to finish
  playing a fixed buffer before the phrase's first note. That playback is
  a plain fire-and-forget `AudioPlayer` spawn, like a hit-feedback sound —
  it never touches `GameplayClock` or the sink, so it can't run afoul of
  the sink-anchoring invariant above; reusing the wait-freeze path (rather
  than inventing a clock-jump) is what keeps the whole feature anchoring-
  safe and self-pacing (a slow response just delays every later cue with
  it, since the clock can't reach them before it reaches the frozen note).
