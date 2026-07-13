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
| Tongue blocking (as an embouchure choice) | **Instructional only** | Acoustically identical output to puckering on a single note — unscoreable. Ships as a text/diagram explainer plus the *same* single-note chart from the lesson above, with copy that says "now try it with tongue blocking." |
| Tongue blocking → octave splits / corner switches | **Scored via proxy** | This *is* distinguishable: two holes 3 apart sounding together (an octave chord) is a genuine tongue-blocking-specific technique with a distinct acoustic signature — two `PitchInfo` entries an octave apart. Reuses the same chord-target primitive as "multiple notes" above. This is the concrete, scoreable half of "tongue blocking," and should be framed as such in the lesson copy (teach the unverifiable embouchure first, then verify the musical result it unlocks).
| Slides | **Scored via proxy** | Two different real techniques share this name: (a) a physical slide of the harmonica sideways across the mouth between adjacent holes, and (b) a bend release/attack glissando into a target note. (a) is acoustically identical to just switching holes normally — chart it as a same-direction adjacent-hole run and teach the physical technique in the instructional copy; scoring is just normal consecutive note hits. (b) is already fully scoreable — it's the existing `Bend` modifier, validated at onset via `target_pitch`. No new engine work for either; this lesson is a content/authoring task (author a chart that exercises hole-to-hole runs and bend releases), not a code task. |
| Hand shape / wah (cupping for tone/wah-wah) | **Scored** | Already built end-to-end: `Modifier::WahWah`'s `oscillation_matches_rate` validates measured amplitude-oscillation rate against the chart's declared `oscillation_hz`. This lesson is pure content authoring on top of existing scoring — chart a held note with a `wah-wah` modifier and a generous tolerance for a first lesson, tightening in later ones. |

### Unit 2 — How to count the blues rhythm

| Lesson | Scoreable? | Mechanism |
|---|---|---|
| Reading the 12-bar blues grid | **Instructional + visual** | `TwelveBarBluesOverlay` already exists and visually highlights the current bar/chord. The lesson is: show the overlay large and explained, over a simple I-IV-V backing (reuse Jam Session's existing 12-bar cycle), with a light "does the player stay in time" pass criterion from ordinary timing-window scoring — no new mechanism. |
| Using your feet (internalizing tempo by tapping) | **Instructional only** | Foot taps aren't harmonica pitch and can't be reliably separated from harmonica audio on a single mic channel — out of scope for the scoring pipeline. Ships as instructional copy plus a metronome-only "keep time" chart (the existing `MetronomeFeel`/tempo machinery), where the *implicit* check is just normal timing-window accuracy on notes played against the beat. |
| Call and response | **Scored** (needs the feature — see `PLAN.md`) | Game synthesizes a short call via the song editor's existing WAV synth path (`song_editor::playback`), then opens a wait window (reusing `pause_menu::WaitForNoteMode`/`first_due_unresolved_note`) for the player to echo it, scored by the normal pipeline. This is the one lesson-driven item that's also a standalone `PLAN.md`/`ROADMAP.md` feature, not lesson-specific plumbing — build it there, lessons just author call phrases. |
| Blues improvisation | **Scored via proxy** | Can't judge "good" improvisation, but can judge scale/chord-tone adherence: reuse `jam_session::JamHoleGuide`'s existing blues-scale/chord-tone classification (already used for hole-map tinting) to accumulate "% of notes played that were in-scale/on the current bar's chord tones" over an open jam window, with a pass threshold (e.g. ≥80% in-scale). This is a small new stats accumulator parallel to `SongStats`, not new music-theory work — the classification already exists. |

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
time. Tallied as a new `SongStats::chord: TechniqueStats` bucket (one entry
per sibling note, same convention as every other technique bucket) — added
to `lesson_schema.dtd.json`'s technique enum as `"chord"`, so it rides the
same `PassCriteria::Technique` machinery `clean-attack` does, no new
criterion variant. A chord note is deliberately excluded from `clean_attack`
tallying (see primitive 1) — "only one pitch sounding" is the wrong question
for a note that's supposed to have company.

### 3. Lesson manifest, loader, and menu page (structural, no new DSP)

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
   both pass: ≥50% chord-technique accuracy. Both are original
   scale/chord-tone drills, not melodic content — the safe-to-author subset
   per `TODO.md`'s content-gap item.
4. Blues-scale-adherence stats accumulator → improvisation lesson (depends
   on nothing above; can move earlier if it's higher-value to ship first).
5. Call-and-response (tracked as its own `PLAN.md` item since it's also a
   standalone Jam Session feature, not lesson-only) → the call-and-response
   lesson.
6. Content pass: author the actual lesson bodies/charts. Scale drills,
   technique exercises, and the 12-bar chord-tone walkthrough are safe to
   author freely (no copyrighted melody involved); anything built on a real
   tune follows the same rights/judgment carve-out as `TODO.md`'s bundled-
   song gap.
7. Jazz unit — separate milestone (0.6), after the above ships and proves
   the chord-target/manifest machinery out.
