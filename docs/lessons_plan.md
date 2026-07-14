# Lessons: curriculum & engine design

Design doc for the "Lessons" item in `ROADMAP.md` 0.4. Covers the content
list, what already-built machinery each lesson reuses, what genuinely needs
new engine work, and a phased build order. Read this before writing any
lesson code — `PLAN.md`'s Lessons entry just points here.

A note on sourcing: this content list was drafted from general, widely-taught
blues/jazz harmonica pedagogy (single note → tongue blocking → bending →
12-bar form → positions → improvisation is the standard progression taught
by most harmonica method books and instructors), not from a specific
external source — none was available to consult. If you have a particular
curriculum or reference you want the lesson order/content matched to, share
it and this doc should be revised against it before content authoring starts.

## Design principle: honest about what's scoreable

The engine judges *pitch* (and, for a few modifiers, amplitude/timing
patterns). Some technique topics change *how* a player produces a note
without changing the note itself — the mic literally cannot tell puckering
from tongue blocking if both land on the same clean single note. Every
lesson below is tagged:

- **Scored** — the existing (or a small, new) scoring primitive can judge
  it directly.
- **Scored via proxy** — can't verify the technique itself, but can verify
  a musical outcome that requires it (e.g. tongue blocking is unverifiable,
  but the octave splits/chord shots it enables are).
- **Instructional only** — text/diagram content plus a normal note-hit
  chart to practice on; the game cannot verify the technique choice, only
  that the player is now playing the notes.

Don't build scoring machinery for something in the last category — that's
effort spent on a check that can't actually check anything.

## Content list, mapped to engine reality

### Unit 1 — How to blow the harmonica

| Lesson | Scoreable? | Mechanism |
|---|---|---|
| Single note (embouchure precision) | **Scored** | Existing pitch scoring already does this: a chart of single held notes, judged normally. The one gap is that today's scoring only checks whether the *expected* pitch is present within the window — it doesn't check for or penalize *other* simultaneous pitches, so a breathy multi-hole leak currently scores as a clean hit. A "clean single note" pass criterion needs `ActivePitches` to show only the expected MIDI note (see New engine work below). |
| Multiple notes (2–3 hole blow/draw chords) | **Scored** | Needs a chord-target note type — see New engine work. `ActivePitches` is already `Vec<PitchInfo>` (multiple concurrent pitches), and NMF is explicitly a polyphonic-capable algorithm per its dictionary design, so simultaneous-pitch detection is feasible without new DSP; the gap is purely on the chart/scoring side. |
| Tongue blocking (as an embouchure choice) | **Instructional only — done** | `tongue-blocking` (Unit 1, no chart, Mark-as-Done): acoustically identical output to puckering on a single note — unscoreable, so it's explainer copy pointing at the octave-split lesson as its concrete, scoreable payoff. |
| Tongue blocking → octave splits / corner switches | **Scored via proxy — done** | Shipped earlier as `octave-split` (see the chord-target primitive above). |
| Slides | **Scored via proxy — done** | `slides` (Unit 1): (a) a physical slide of the harmonica sideways across the mouth between adjacent holes — charted as holes 4-5-6 blow, ordinary consecutive single notes (no new mechanism, the technique is taught in the copy, not verified); (b) a bend release/attack glissando — the existing `Bend` modifier on the 2 draw half-step bend, already fully scoreable via `target_pitch`. Pure content-authoring task, no engine work. |
| Hand shape / wah (cupping for tone/wah-wah) | **Scored** | Already built end-to-end: `Modifier::WahWah`'s `oscillation_matches_rate` validates measured amplitude-oscillation rate against the chart's declared `oscillation_hz`. This lesson is pure content authoring on top of existing scoring — chart a held note with a `wah-wah` modifier and a generous tolerance for a first lesson, tightening in later ones. |

### Unit 2 — How to count the blues rhythm

| Lesson | Scoreable? | Mechanism |
|---|---|---|
| Reading the 12-bar blues grid | **Instructional + visual** | `TwelveBarBluesOverlay` already exists and visually highlights the current bar/chord. The lesson is: show the overlay large and explained, over a simple I-IV-V backing (reuse Jam Session's existing 12-bar cycle), with a light "does the player stay in time" pass criterion from ordinary timing-window scoring — no new mechanism. |
| Using your feet (internalizing tempo by tapping) | **Instructional + implicit check — done** | `using-your-feet` (Unit 2): foot taps aren't harmonica pitch and can't be reliably separated from harmonica audio on a single mic channel, so the lesson body just coaches tapping/counting; the chart is a steady quarter-note pulse on hole 4 blow/draw with *tighter than usual* scoring windows (80/180/300ms, vs. other beginner drills' 150+/350+/600+ms) — the implicit check the docs table originally called for, since this is the one lesson where timing precision, not pitch or technique, is actually the point. |
| Call and response | **Scored — done** | `gameplay::call_response` synthesizes each chart's `call: true` phrase groups via the song editor's WAV synth (`song_editor::playback`) and plays them as a one-shot demo; the response is the same notes, force-frozen (`ScheduledNote::force_wait`) via the existing `WaitForNoteMode`/`first_due_unresolved_note` machinery, scored by the normal pipeline. The `call-response` lesson (Unit 2) is the concrete instance — see `PLAN.md`. |
| Blues improvisation | **Scored via proxy — done** | `jam_session::ImprovStats` accumulates scale/chord-tone adherence over an open Jam Session (see "New engine work required" below); the `improvisation` lesson (Unit 2) pass criterion is `PassCriteria::ScaleAdherence{threshold: 0.8}`. |

### Unit 3 — How to play harmonica jazz

Scoped as a separate roadmap milestone (0.6, see `ROADMAP.md`) rather than
part of the 0.4 blues curriculum — it needs jazz-specific chord-tone tables
(ii–V–I, dominant/altered extensions) alongside the existing dominant-7th/
blues-scale classification, likely leans on chromatic harmonica and the
already-scoreable `Modifier::Slide`, and its own content (jazz standards are
more often still in copyright than blues heads, so content sourcing is a
harder version of the existing content-gap problem in `TODO.md`). Don't
start this before the blues curriculum below ships and the chord-target
primitive it needs already exists from Unit 1.

## New engine work required

Two primitives, both reused by multiple lessons above — build these once:

### 1. "Clean single note" scoring (small) — done

`is_clean_attack(harp_pitches: &HashSet<u8>, expected: u8) -> bool` in
`src/scoring.rs`: true only when `expected` is the sole harp-producible
pitch currently sounding (the caller passes the same `harp_pitches` set
`score_notes` already computes — `ActivePitches` intersected with
`ValidHarpNotes`). Tallied as a new `SongStats::clean_attack: TechniqueStats`
bucket on every onset hit (`score_notes`'s `NoteOutcome::Hit` arm), which
gets it a "Clean attack" row on *every* song's results screen for free and,
more importantly, lets it ride the existing `PassCriteria::Technique`
machinery unchanged — no new criterion variant needed, just a new bucket
name (`"clean-attack"`, added to `lesson_schema.dtd.json`'s technique enum).
The single-note lesson's `pass_criteria` now reads `{"type": "technique",
"technique": "clean-attack", "threshold": 0.6}` instead of overall accuracy.

### 2. Chord-target notes (bigger; shared by "multiple notes," octave-split
tongue-blocking drills, and any future jazz chord-tone lesson) — done

No chart schema change, no `format_version` bump: the chart format already
lets one `TrackItem` carry multiple simultaneous `events` (`PlayMode::Chord`/
`Split`, used for the visual badge only until now) — every event just became
its own independent `ScheduledNote`, scored with no idea the others existed.
The fix stays entirely on the scoring side: `ScheduledNote` gained
`chord_pitches: Vec<u8>` (`src/gameplay/mod.rs`), the full target set shared
identically by every sibling note `gameplay_2d::build_combined_notes`/
`gameplay_3d::build_notes_3d` produce from one multi-event item (empty for
an ordinary single-event item — no behavior change there). The new pure
primitive is `scoring::chord_is_sounding(expected: &[u8], harp_pitches:
&HashSet<u8>) -> bool` — true only when *every* pitch in the set sounds at
once, unlike a single note's exact-one-pitch match. `score_notes` ANDs it
into each chord note's existing per-pitch `AttackGate` freshness check
(still needed, still reused — a chord's individual pitches must each be
freshly articulated too, not just simultaneously present), so a chord scores
only when its sibling events are actually struck together, not one at a
time. A chord note is also excluded from `clean_attack` tallying (see
primitive 1) — "only one pitch sounding" is the wrong question for a note
that's supposed to have company.

No new `SongStats` bucket or `PassCriteria` variant, unlike `clean-attack`:
once `chord_is_sounding` gates whether a chord note can be `Hit` at all, an
out-of-sync chord already shows up as an ordinary miss in plain accuracy —
there's no separate blind spot to track. (`clean-attack` genuinely needed
its own bucket because `score_notes` only ever checked the *expected*
pitch's presence; a breathy leak alongside it still scored as an ordinary
hit, invisible to plain accuracy.) The two chord lessons below both pass on
`{"type": "accuracy", "threshold": 0.5}` — plain accuracy, since every note
in both charts is a chord note anyway.

### 3. Scale-adherence accumulator (small) — done

`jam_session::classify_note_fit(note, chord_tones, scale_classes) -> NoteFit`
is `update_hole_map`'s per-frame tinting logic extracted into a standalone,
directly-tested pure function (shared by both call sites so the live tint
and the tally can never silently disagree). `jam_session::ImprovStats`
(`chord_tone`/`in_scale`/`out_of_scale: u32` counters, `.adherence()` — the
`(chord_tone + in_scale) / total` fraction `PassCriteria::ScaleAdherence`
reads) is tallied by `accumulate_improv_stats`, the live twin of
`update_hole_map`: same classification, but gated on a fresh attack
(`ImprovGate`, a `scoring::AttackGate<u8>` — the same fresh-attack idea
`gameplay::PitchGate` uses for scored modes) so a held note counts once, not
every frame it stays sounding. Reset alongside `SongStats`/`PitchGate` in
`gameplay::reset_score` (unconditional — always in a known state, even
though only Jam Session ever writes to it, the same "always-on diagnostic"
convention `SongStats::clean_attack` established).

This is the one lesson type with no chart notes to score and no natural
end (an open jam loops indefinitely), so it can't be judged the way the
other two primitives are — see `PassCriteria::ScaleAdherence`'s doc comment
for how the whole judging path differs (routes into `GameplayMode::
JamSession` from the reader's Start button; judged by a dedicated "Finish
Lesson" pause-menu button instead of the results screen).

### 4. Call-and-response (bigger; also a standalone Jam feature) — done

Unlike the three primitives above, this genuinely needed a chart schema
addition: `TrackItem::call: bool` (`#[serde(default)]`, so every existing
chart parses unchanged — no `format_version` bump needed for *old* charts to
keep working, though new charts using it should still bump their own
`metadata.format_version` per the general convention). A maximal run of
consecutive `call: true` items (`gameplay::call_response::call_phrase_
groups`, pure and tested) is one phrase; its notes are ordinary
`ScheduledNote`s (nothing new about how they're scored) except for one flag,
`force_wait: bool`, set from `TrackItem::call`.

Two things happen around those notes:
- **The demo.** At song setup (`setup_call_cues`), each phrase's notes are
  converted to `song_editor::playback::PhraseNote`s (tick-relative to the
  phrase's own start, frequency resolved via the *same* `target_pitch` every
  other note uses — so a demo correctly reflects a bent/overblown call note
  too) and rendered through `render_pcm`/`encode_wav` — literally the same
  synth behind the song editor's own Play button, widened from `pub(super)`
  to `pub(crate)` for this (`PhraseNote` replaces `render_pcm`'s old
  `&[GridNote]` parameter with a source-agnostic tick/freq/expr triple, so
  the editor and this feature share the DSP without either depending on the
  other's note representation). Scheduled to *finish* `LEAD_BUFFER_SECS`
  before the phrase's first note, computed once from the synthesized
  buffer's own length — no author-facing lead-time field needed, just "leave
  enough silence before the phrase" (ordinary chart authoring). Firing
  (`fire_call_cues`) is a plain fire-and-forget `AudioPlayer` spawn, exactly
  like a hit-feedback sound — deliberately *not* a clock-jumping mechanism,
  so it can't fight the sink-anchoring invariant (`CLAUDE.md`'s clock notes)
  at all; this was the open design question `PLAN.md` flagged, resolved by
  sidestepping it rather than solving it.
- **The forced wait.** `force_wait` notes freeze the clock the same way
  `WaitForNoteMode` does, regardless of whether the player has that practice
  toggle on — `tick_clock`'s freeze condition, previously `wait_mode.0 &&
  first_due_unresolved_note(..)`, is now the small pure `wait_freeze_index`
  helper: `first_due_unresolved_note(..).filter(|&i| wait_mode || notes[i].
  force_wait)`. Because the *whole* pipeline (clock freeze, sink pause) was
  already anchoring-safe by construction for `WaitForNoteMode`, reusing it
  for call-and-response inherits that safety for free rather than needing
  new clock-jump logic — and self-paces correctly no matter how long a
  player takes on one response, since a later phrase's cue can't fire before
  the frozen clock reaches it.

### 5. Lesson manifest, loader, and menu page (structural, no new DSP)

- **Asset tree**: `assets/lessons/<unit>/<lesson>/` — reuses the `.harpchart`
  format directly for any lesson with notes to play; an instructional-only
  lesson (tongue blocking overview, using-your-feet) has no chart, just
  manifest text.
- **Manifest** (new small schema, its own `.dtd.json` like charts have):
  `id`, `title` (a `loc.msg()` key — lessons need localization like
  everything else user-visible, across en-US/pt-BR/es-ES), `unit`,
  `prerequisite: Option<Vec<String>>` (lesson ids gating this one), `body`
  (localized instructional text/steps), `chart: Option<PathBuf>` (absent =
  instructional-only), `pass_criteria` (an enum: technique-accuracy
  threshold from `SongStats`, overall accuracy/score threshold, or the new
  scale-adherence-% for improv).
- **Persistence**: `PlayerProfile` gains `lessons: HashMap<String,
  LessonRecord>` (id → `{ passed: bool, best_score: u32 }`), same shape as
  the existing `drills`/`songs` maps — follow `profile.rs`'s established
  direct-save-no-debounce pattern.
- **States/menu**: a new `MenuPage::Lessons` (or a small dedicated
  `AppState` if the unit/lesson list needs its own back-stack) → unit list →
  lesson list showing locked/unlocked (from `prerequisite`) and a pass
  checkmark (from `PlayerProfile.lessons`). A chart-backed lesson launches
  the normal `Playing` state with a `LessonContext` resource so `Results`
  can check `pass_criteria` and unlock the next lesson; an instructional-
  only lesson opens a simple text/diagram reader screen — no gameplay
  state needed for that case.

## Suggested build order

1. **Done.** Manifest schema + loader + menu list (no scoring changes at
   all), shipped with the three zero-new-primitive lessons: the
   12-bar-grid walkthrough (instructional-only, Mark-as-Done), the
   hand-shape/wah drill (pass: ≥50% wah-technique accuracy), and a plain
   single-note lesson (pass: ≥60% overall accuracy — without the "clean
   attack" check yet). Landed as `src/lessons.rs` + `src/menu/lessons.rs`
   + `assets/lesson_schema.dtd.json` + `assets/lessons/`; lesson runs
   carry a `LessonContext` resource through the normal gameplay pipeline
   (results judge pass criteria against it; adaptive difficulty is forced
   off for lesson runs; quitting/finishing returns to the lesson list).
2. **Done.** "Clean single note" primitive → wired into the single-note
   lesson (see "New engine work required" above).
3. **Done.** Chord-target primitive (see "New engine work required" above)
   → two new bundled lessons under `assets/lessons/01_blowing/`:
   `03_multiple_notes` (blow/draw triads on holes 1-2-3/2-3-4/4-5-6) and
   `04_octave_split` (tongue-blocked octave splits on holes 1+4/2+5/3+6),
   both pass: ≥50% overall accuracy (every note in both charts is a chord
   note, so plain accuracy already means "chord accuracy" here). Both are
   original scale/chord-tone drills, not melodic content — the
   safe-to-author subset per `TODO.md`'s content-gap item.
4. **Done.** Scale-adherence accumulator (see "New engine work required"
   above) → `improvisation` (Unit 2, `assets/lessons/02_rhythm/
   03_improvisation/`), prerequisite `call-response` (moved off `twelve-bar`
   directly once the call-and-response lesson landed between them — see
   step 5), pass: ≥80% of notes in-scale or better. The one lesson that
   opens `GameplayMode::JamSession` instead of a scored chart — see
   `PassCriteria::ScaleAdherence`.
5. **Done.** Call-and-response (`gameplay::call_response`, see "New engine
   work required" above) → `call-response` (Unit 2, `assets/lessons/
   02_rhythm/02_call_response/`), prerequisite `twelve-bar`, pass: ≥70%
   accuracy on three phrases (one, two, then three notes) — force-frozen
   response notes make this mostly a pitch-recognition check, timing is
   never at risk, hence the more generous threshold than a normal chart.
   `docs/lessons_plan.md`'s standalone-feature note above still applies:
   the primitive lives in `gameplay`, not lesson-specific plumbing, so any
   future non-lesson use (Jam Session call-and-response, say) can reuse it
   directly.
6. **Done, for the content originally scoped in this doc.** `tongue-blocking`
   (Unit 1, instructional), `slides` (Unit 1, adjacent-hole run + bend
   release), `using-your-feet` (Unit 2, tight-window quarter-note pulse) —
   all original scale/technique drills, not melodic content, per the
   safe-to-author subset. Unit 1 is now `single-note → {hand-wah,
   multiple-notes → tongue-blocking → octave-split, slides}`; Unit 2 is
   `twelve-bar → {call-response → improvisation, using-your-feet}`. Further
   content (more drills, harder variations) can still land under this step
   any time — it's open-ended, not a one-shot task.
7. Jazz unit — separate milestone (0.6), after the above ships and proves
   the chord-target/manifest machinery out.
