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

## Wave 2 — planned

The next batch: harmonica basics, counting the bars, train sounds, blues
licks, blues improvisation, and the jazz unit. Almost everything rides the
five primitives above — only three engine items are needed, listed after
the content tables. All charts named here are original scale/chord-tone/
vocabulary drills (the safe-to-author subset per `TODO.md`); nothing
melodic or rights-sensitive except where explicitly flagged.

### Unit 1 additions — harmonica basics (`01_blowing/`)

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **Breathing** (`breathing`, `07_breathing`) — diaphragm breathing and long tones: whole notes held 3–4 beats on holes 1–4 blow/draw at a slow tempo; body copy teaches breathing *through* the harp, relaxed shoulders, steady air | **Scored** | Existing sustain + clean-attack scoring — a wavering or leaky long tone fails clean attack | `single-note` | technique `clean-attack` ≥ 0.5 |
| **First bend** (`first-bend`, `08_first_bend`) — the 4-draw half-step bend, charted: alternating 4D and 4D' whole notes; body copy points at the Bending Trainer as the freeform companion tool | **Scored** | `Modifier::Bend` onset validation via `target_pitch` — already fully built | `single-note` | technique `bend` ≥ 0.5 |
| **Deep bends** (`deep-bends`, `09_deep_bends`) — 2-draw half/whole-step and 3-draw half/whole-step bends, the notes 2nd-position blues lives on | **Scored** | Same as above, more of it | `first-bend` | technique `bend` ≥ 0.6 |
| **Vibrato** (`vibrato`, `10_vibrato`) — throat/diaphragm vibrato on held middle-register notes, generous `oscillation_hz` (~4–5 Hz, ±40% tolerance is built in) | **Scored** | `Modifier::Vibrato` sustain-oscillation scoring — already fully built | `breathing` (diaphragm control is the mechanism) | technique `vibrato` ≥ 0.5 |
| **Articulation** (`articulation`, `11_articulation`) — "ta / ka / ta-ka" tonguing: repeated eighth notes on one hole, several bars per hole | **Scored via proxy** | `PitchGate`'s fresh-attack requirement means a slurred held note scores only its first onset — re-articulating each note is genuinely required to hit the chart, which is exactly what tonguing is for | `single-note` | accuracy ≥ 0.6 |

### Unit 2 additions — counting the bars & train sounds (`02_rhythm/`)

Counting first — these teach *where you are* in the bar and in the form:

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **Counting four** (`counting-four`, `05_counting_four`) — count 1-2-3-4: quarter notes on every beat, then beats 1+3 only, then beat 1 only, metronome prominent; body copy teaches counting aloud | **Scored** | Ordinary timing windows, moderately tightened (the `using-your-feet` pattern) | none (second Unit 2 entry point, parallel to `twelve-bar`) | accuracy ≥ 0.6 |
| **Counting the bars** (`bar-counting`, `06_bar_counting`) — through a full 12-bar cycle, play only the chord root on beat 1 of each bar: 2D over I, 4B over IV, 4D over V (2nd position); `TwelveBarBluesOverlay` explained in the body | **Scored** | Timing + pitch over the existing 12-bar machinery; teaches bar counting *and* hearing the changes at once | `counting-four`, `twelve-bar` | accuracy ≥ 0.6 |
| **The turnaround** (`turnaround`, `07_turnaround`) — feel bars 11–12: sparse chart that rests through most of the form, lands the V root in bar 12 and resolves to I at the top of the next chorus | **Scored** | Same; the *rests* are the lesson — a player who loses count plays into silence and misses the landing | `bar-counting` | accuracy ≥ 0.6 |
| **Shuffle feel** (`shuffle-feel`, `08_shuffle_feel`) — straight vs. swung eighths: swung eighth-note pairs on one hole, chart declares `feel: shuffle` so the metronome swings with it | **Scored** | Timing windows against swung note placement; `Song::feel` already drives the metronome | `counting-four` | accuracy ≥ 0.6 |

Then the train — the classic harmonica rhythm study, and the payoff for the
chord work in Unit 1. It's breathing, chords, and tempo control disguised
as a sound effect:

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **Train: the chug** (`train-chug`, `09_train_chug`) — alternating 1-2-3 blow / 1-2-3 draw chords in steady eighths at a slow tempo ("huff-puff"); body copy: breathe the rhythm, don't tongue it | **Scored** | Chord-target primitive (`chord_is_sounding`) — every note is a 3-hole chord | `multiple-notes` (Unit 1) | accuracy ≥ 0.5 |
| **Train: rolling** (`train-rolling`, `10_train_rolling`) — the same chug accelerating from ~70 to ~110 BPM via a tick-time chart with a tempo map (the train leaves the station) | **Scored** | Chord targets + the chart format's tempo map — this would be the first bundled chart to exercise tempo-map timing in anger, worth shipping for that validation alone | `train-chug` | accuracy ≥ 0.5 |
| **Train: the whistle** (`train-whistle`, `11_train_whistle`) — chug choruses punctuated by the whistle: a held 4-5 draw two-hole chord with a `wah-wah` modifier | **Scored** | Chord targets + wah oscillation scoring, combined in one chart | `train-rolling`, `hand-wah` | technique `wah-wah` ≥ 0.5 |

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
| **Chord-tone improvisation** (`chord-tone-improv`, `05_chord_tone_improv`) — open jam; don't just stay in the scale, *land on chord tones* when the chord changes | **Scored via proxy** | `ImprovStats` already counts `chord_tone` separately from `in_scale` — needs only the new `ChordToneAdherence` criterion (engine item 1) | `improvisation` (Unit 2), `blues-scale` | chord-tone fraction ≥ 0.4 |
| **Minor blues** (`minor-blues-improv`, `06_minor_blues`) — improvise over the minor blues progression; body copy covers what changes (b3 is home now) | **Scored via proxy** | `Progression::MinorBlues` already drives backing/hole-map; needs the lesson-manifest progression field (engine item 2) | `chord-tone-improv` | `ScaleAdherence` ≥ 0.8 |
| **Question & answer** (`question-answer`, `07_question_answer`) — phrasing: improvise for 2 bars, *rest* for 2 bars, alternating through the form; leaving space is the lesson | **Scored via proxy** | Needs the phrase-discipline primitive (engine item 3) — the one genuinely new mechanism in this wave | `improvisation` | phrase discipline ≥ 0.7 |

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

### New engine work required (wave 2)

Only three items; everything else above is pure content authoring.

1. **`PassCriteria::ChordToneAdherence { threshold }`** (tiny). The
   counter already exists (`ImprovStats::chord_tone`); add the variant, a
   `chord_tone / total` sibling of `.adherence()`, the schema enum entry in
   `lesson_schema.dtd.json`, and judge it in the same "Finish Lesson" path
   `ScaleAdherence` uses. Locale keys ×3 for any new reader/results copy.
2. **Lesson-selected jam progression** (tiny). An optional manifest field
   (`progression: "minor"` etc.) that the lesson reader's Start button
   seeds into `menu::JamProgression` when routing into `JamSession`;
   `JamProgression` already resets to Standard elsewhere, so end-of-life
   is free. Needed by `minor-blues-improv` (and later any ii–V–I jam).
3. **Phrase-discipline primitive** (medium — the one new mechanism).
   Measures "did you leave space": during an open jam, bars alternate
   between *play* and *rest* windows (e.g. 2 on / 2 off, derived from the
   same bar clock the metronome/12-bar overlay already track). Extend
   `ImprovStats` with a `rest_violations: u32` counter — an `ImprovGate`
   fresh attack landing inside a rest window — and add `PassCriteria::
   PhraseDiscipline { threshold }` = `1 − violations / total_attacks`,
   judged via Finish Lesson like the other jam criteria. Honest per the
   design principle: it can't judge whether the played phrases were *good*,
   but "stopped playing when the phrase should breathe" is exactly the
   teachable, verifiable outcome. Pure function first
   (`in_rest_window(bar_index, pattern) -> bool`), tests, then the system —
   per the testing convention.

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

### Suggested build order (wave 2)

1. **Zero-engine-work batch, basics first**: `breathing`, `first-bend`,
   `deep-bends`, `vibrato`, `articulation`; then `counting-four`,
   `bar-counting`, `turnaround`, `shuffle-feel`. Pure authoring, each
   independently shippable.
2. **The train trio**: `train-chug`, `train-rolling`, `train-whistle`.
   Validate tempo-map gameplay timing with `train-rolling` before shipping
   it (add a `gameplay_validation.md` entry).
3. **Unit 3 licks**: `blues-scale`, `first-licks`, `bent-licks`,
   `licks-over-changes`. Still zero engine work; this is where content
   judgment matters most — keep licks original vocabulary, not quotes.
4. **Small engine items 1–2** → `chord-tone-improv`, `minor-blues-improv`.
5. **Engine item 3 (phrase discipline)** → `question-answer`.
6. **Unit 4 jazz** — gated on the 0.6 milestone's chord-tone tables and
   progression work (`ROADMAP.md`).
