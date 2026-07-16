# Lessons: curriculum & engine design

Design doc for the Lessons feature (`ROADMAP.md` 0.4 → 0.6). Covers what's
shipped (wave 1, compactly), the scoring primitives lessons are built from,
and the plan for the next batch of exercises (wave 2). Read this before
writing any lesson code — `PLAN.md`'s Lessons entry just points here.

A note on sourcing: this curriculum was drafted from general, widely-taught
blues/jazz harmonica pedagogy (single note → tongue blocking → bending →
12-bar form → positions → improvisation is the standard progression taught
by most harmonica method books and instructors), not from a specific
external source. If you have a particular curriculum or reference you want
the lesson order/content matched to, share it and this doc should be
revised against it before content authoring continues.

## Design principle: honest about what's scoreable

The engine judges *pitch* (and, for a few modifiers, amplitude/timing
patterns). Some technique topics change *how* a player produces a note
without changing the note itself — the mic literally cannot tell puckering
from tongue blocking if both land on the same clean single note. Every
lesson is tagged:

- **Scored** — an existing (or a small, new) scoring primitive judges it
  directly.
- **Scored via proxy** — can't verify the technique itself, but can verify
  a musical outcome that requires it (e.g. tongue blocking is unverifiable,
  but the octave splits it enables are; "ta-ka" tonguing is unverifiable,
  but rapid same-hole re-articulation — which the fresh-attack gate
  genuinely requires — is not).
- **Instructional only** — text/diagram content plus (optionally) a normal
  note-hit chart to practice on; the game cannot verify the technique
  choice, only that the player is playing the notes.

Don't build scoring machinery for something in the last category — that's
effort spent on a check that can't actually check anything.

## Wave 1 — shipped

Units 1–2 and the whole engine are live. `assets/lessons/`:

- **Unit 1 "How to blow"** (`01_blowing/`): `single-note` (clean-attack),
  `hand-wah` (wah oscillation), `multiple-notes` (chord targets),
  `tongue-blocking` (instructional), `octave-split` (chord targets),
  `slides` (adjacent-hole run + bend release).
- **Unit 2 "Counting the blues"** (`02_rhythm/`): `twelve-bar`
  (instructional + overlay), `call-response` (call-and-response
  primitive), `improvisation` (open jam, scale adherence),
  `using-your-feet` (tight-window timing pulse).

Prerequisite chains: Unit 1 is `single-note → {hand-wah, multiple-notes →
tongue-blocking → octave-split, slides}`; Unit 2 is `twelve-bar →
{call-response → improvisation, using-your-feet}`.

The scoring primitives all this rides on (architecture detail in
`CLAUDE.md`; each was built once and is reusable by any future lesson):

1. **Clean single note** — `scoring::is_clean_attack`, tallied as the
   `SongStats::clean_attack` technique bucket, judged via
   `PassCriteria::Technique { technique: "clean-attack" }`.
2. **Chord targets** — multi-event `TrackItem`s (`PlayMode::Chord`/`Split`)
   score only when `scoring::chord_is_sounding` sees every sibling pitch at
   once; plain accuracy already reflects it, no dedicated criterion needed.
3. **Scale adherence** — `jam_session::ImprovStats` (fresh-attack-gated
   `chord_tone`/`in_scale`/`out_of_scale` counters over an open jam),
   judged by `PassCriteria::ScaleAdherence` via the "Finish Lesson" pause
   button.
4. **Call-and-response** — `TrackItem::call: true` phrase groups are
   synthesized into an audible demo and their response notes force-freeze
   the clock (`ScheduledNote::force_wait`), scored by the normal pipeline.
5. **Modifier scoring** (predates lessons, fully reusable) — `Bend` /
   `Overblow` / `Overdraw` / `Slide` validated at onset via `target_pitch`;
   `Vibrato` / `WahWah` validated from sustain samples against the chart's
   `oscillation_hz` (±40%). Each has its own `SongStats` technique bucket
   usable in `PassCriteria::Technique`.

Also available to charts and relevant below: `tick` + tempo-map timing
(accelerando is chartable), `Song::feel: Shuffle` (swung metronome),
per-chart scoring windows (how `using-your-feet` tightens timing), and
`Progression` (standard / quick-change / minor) for jam backings.

## Wave 2, part 1 — shipped

Unit 1's harmonica-basics extensions, Unit 2's bar-counting drills, and the
train trio are all live — 12 lessons, `assets/lessons/`:

- **Unit 1 additions** (`01_blowing/`): `breathing` (`07_breathing`,
  clean-attack sustain), `first-bend` (`08_first_bend`, the 4-draw
  half-step, `bend` technique), `deep-bends` (`09_deep_bends`, 2/3-draw
  half- and whole-step bends), `vibrato` (`10_vibrato`, sustain
  oscillation), `articulation` (`11_articulation`, "ta-ka" tonguing via the
  fresh-attack proxy).
- **Unit 2 additions** (`02_rhythm/`): `counting-four` (`05_counting_four`),
  `bar-counting` (`06_bar_counting`, 2D/4B/4D roots over the 12-bar form,
  2nd position), `turnaround` (`07_turnaround`, rests through the form to
  the bar-12 landing), `shuffle-feel` (`08_shuffle_feel`, `feel: shuffle`
  swung pairs), then the train trio: `train-chug` (`09_train_chug`, chord
  targets), `train-rolling` (`10_train_rolling` — the first bundled chart
  built on `tick` + a tempo map instead of fixed seconds, a genuine
  end-to-end exercise of that timing path), `train-whistle`
  (`11_train_whistle`, chord targets + wah oscillation combined).
- **All three wave-2 engine items are built** (see "Engine work" below,
  kept for reference): `PassCriteria::ChordToneAdherence`, the lesson
  manifest's `progression` field, and `PassCriteria::PhraseDiscipline` +
  `jam_session::in_rest_window`/`ImprovStats::rest_violations`. Nothing
  left to build for the rest of wave 2 — what remains below is pure content
  authoring.

## Wave 2, part 2 — planned

What's left: Unit 3 (blues vocabulary, licks, improvisation) and the Unit 4
jazz milestone. All charts named here are original scale/chord-tone/
vocabulary drills (the safe-to-author subset per `TODO.md`); nothing
melodic or rights-sensitive except where explicitly flagged.

### New Unit 3 — blues vocabulary & improvisation (`03_blues/`)

The bridge from drills to *music*. Licks are taught call-and-response —
the primitive was built for exactly this — and improvisation deepens from
"stay in the scale" to chord-tone targeting and phrasing.

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **The blues scale** (`blues-scale`, `01_blues_scale`) — 2nd-position blues scale up and down: 2D · 3D' · 4B · 4D' · 4D · 5D · 6B (needs the two bends, which is why bending comes first) | **Scored** | Plain chart + existing bend scoring | `deep-bends` (Unit 1) | accuracy ≥ 0.6 |
| **First licks** (`first-licks`, `02_first_licks`) — three short original licks (3–4 blues-scale notes each, no bends: e.g. 2D-3D-4B, 4B-4D-5D, 6B-5D-4D), each taught call-and-response | **Scored** | Call-and-response primitive, unchanged | `call-response` (Unit 2) | accuracy ≥ 0.7 (frozen waits make this pitch-recognition, not timing — same rationale as `call-response`) |
| **Bent licks** (`bent-licks`, `03_bent_licks`) — licks built around 3D' and 4D' ("the crying notes"), call-and-response | **Scored** | Call-and-response + bend scoring | `first-licks`, `deep-bends` | technique `bend` ≥ 0.5 |
| **Licks over the changes** (`licks-over-changes`, `04_licks_over_changes`) — a full 12-bar chorus placing one lick per chord (adapted to I/IV/V), phrase-tagged per 4-bar line so the phrase overlay shows the form | **Scored** | Plain chart over the 12-bar; combines everything above | `bent-licks`, `bar-counting` | accuracy ≥ 0.6 |
| **Chord-tone improvisation** (`chord-tone-improv`, `05_chord_tone_improv`) — open jam; don't just stay in the scale, *land on chord tones* when the chord changes | **Scored via proxy** | `PassCriteria::ChordToneAdherence` — built, reads `ImprovStats::chord_tone_adherence()` | `improvisation` (Unit 2), `blues-scale` | chord-tone fraction ≥ 0.4 |
| **Minor blues** (`minor-blues-improv`, `06_minor_blues`) — improvise over the minor blues progression; body copy covers what changes (b3 is home now) | **Scored via proxy** | `Progression::Minor` — the lesson manifest's `progression: "minor"` field (built) seeds `menu::JamProgression` on Start | `chord-tone-improv` | `ScaleAdherence` ≥ 0.8 |
| **Question & answer** (`question-answer`, `07_question_answer`) — phrasing: improvise for 2 bars, *rest* for 2 bars, alternating through the form; leaving space is the lesson | **Scored via proxy** | `PassCriteria::PhraseDiscipline` — built, reads `ImprovStats::phrase_discipline()`; the 2-on/2-off pattern is fixed (`jam_session::PHRASE_PLAY_BARS`/`PHRASE_REST_BARS`) | `improvisation` | phrase discipline ≥ 0.7 |

### Unit 4 — jazz (`04_jazz/`, the 0.6 milestone)

Deliberately after everything above ships — it needs its own engine work
(jazz chord-tone tables) and its content sourcing is harder (jazz standards
are more often still in copyright than blues heads; lean on original
drills and public-domain jazz-blues heads). Planned shape:

| Lesson | Scoreable? | Mechanism | Notes |
|---|---|---|---|
| **Swing eighths** — swung-eighth drills at tightening windows | **Scored** | `feel: shuffle` + per-chart windows, zero engine work | Could ship early, but pedagogically belongs here |
| **ii–V–I chord tones** — arpeggio drills over a ii–V–I backing | **Scored** | Needs jazz chord-tone tables in `classify_note_fit`'s vocabulary and a ii–V–I / jazz-blues `Progression` variant (0.6 engine work, see `ROADMAP.md`) | Original arpeggio content, rights-safe |
| **Jazz-blues form** — the 12-bar with the ii–V turnaround, taught like `bar-counting` | **Scored** | Same progression variant + `TwelveBarBluesOverlay` labels | |
| **Chromatic slide basics** — half-steps with the slide button, on a chromatic chart | **Scored** | `Modifier::Slide` onset scoring already exists; chromatic charts fully supported | First bundled chromatic lesson content |
| **Jazz heads** — actual repertoire | **Scored** | Ordinary charts | Rights-sensitive: public-domain only, human judgment required (`TODO.md`) |

### Engine work (done)

All three wave-2 engine items are built (see "Wave 2, part 1 — shipped"
above): `PassCriteria::ChordToneAdherence`/`PhraseDiscipline`, the lesson
manifest's `progression` field, and `jam_session::in_rest_window` +
`ImprovStats::rest_violations`/`chord_tone_adherence`/`phrase_discipline`.
`menu::lessons::is_jam_criteria` routes all three jam-based criteria (plus
`ScaleAdherence`) into `GameplayMode::JamSession`;
`gameplay::pause_menu::jam_fraction_for` picks the right `ImprovStats`
fraction for whichever criterion a given lesson declares. Everything left
in Unit 3/4 below is pure content authoring.

Cross-cutting authoring notes:

- Every new lesson: `lesson.json` + locale keys in all three languages
  (`tests/asset_layout.rs` enforces schema, prereq integrity, and key
  existence — it will catch omissions).
- Cross-unit prerequisites (`train-chug` ← `multiple-notes`,
  `blues-scale` ← `deep-bends`) are just ids — `is_unlocked` doesn't care
  about units — but double-check the lesson list UI presents a locked
  lesson's prerequisite name legibly when it lives in another unit.
- New charts using only existing schema features need no `format_version`
  bump; `train-rolling`'s tempo map and the multi-modifier charts all use
  long-supported fields.

### Suggested build order (wave 2, part 2 — what's left)

1. **Unit 3 licks**: `blues-scale`, `first-licks`, `bent-licks`,
   `licks-over-changes`. Zero engine work (everything it needs already
   shipped); this is where content judgment matters most — keep licks
   original vocabulary, not quotes.
2. **The three jam-criteria lessons**: `chord-tone-improv`,
   `minor-blues-improv`, `question-answer` — content authoring only now
   that the engine side is done; each just needs `lesson.json` + locale
   keys, no chart (they route into an open Jam Session).
3. **Unit 4 jazz** — gated on the 0.6 milestone's chord-tone tables and
   progression work (`ROADMAP.md`).
