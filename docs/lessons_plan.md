# Lessons: curriculum & engine design

Design doc for the Lessons feature (`ROADMAP.md` 0.4 → 0.6). Covers what's
shipped (wave 1, compactly), the scoring primitives lessons are built from,
and the plan for the next batch of exercises (wave 2). Read this before
writing any lesson code — `PLAN.md`'s Lessons entry just points here.

**Structure, progression and motivation live in
`docs/training_tree_plan.md`** — the technique DAG, the five-tier training
ladder hung off each lesson, the horizontal skill-tree view, and what the
evidence says about gamifying practice. This file stays curriculum: what
each lesson teaches and what is honestly scoreable.

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

## Wave 2, part 2 — shipped

Unit 3, the bridge from drills to *music*, is live — 7 lessons,
`assets/lessons/03_blues/`. Licks are taught call-and-response — the
primitive built in wave 1 for exactly this — and improvisation deepens from
"stay in the scale" (wave 1's `improvisation`) to chord-tone targeting and
phrasing. All charts are original scale/chord-tone/vocabulary drills (the
safe-to-author subset per `TODO.md`); nothing melodic or rights-sensitive.

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **The blues scale** (`blues-scale`, `01_blues_scale`) — 2nd-position blues scale up and down: 2D · 3D' · 4B · 4D' · 4D · 5D · 6B (needs the two bends, which is why bending comes first) | **Scored** | Plain chart + existing bend scoring | `deep-bends` (Unit 1) | accuracy ≥ 0.6 |
| **First licks** (`first-licks`, `02_first_licks`) — three short original licks (three blues-scale notes each, no bends: 2D-3D-4B, 4B-4D-5D, 6B-5D-4D), each taught call-and-response | **Scored** | Call-and-response primitive, unchanged | `call-response` (Unit 2) | accuracy ≥ 0.7 (frozen waits make this pitch-recognition, not timing — same rationale as `call-response`) |
| **Bent licks** (`bent-licks`, `03_bent_licks`) — licks built around 3D' and 4D' ("the crying notes"), call-and-response | **Scored** | Call-and-response + bend scoring | `first-licks`, `deep-bends` | technique `bend` ≥ 0.5 |
| **Licks over the changes** (`licks-over-changes`, `04_licks_over_changes`) — a full 12-bar chorus placing one lick per chord (adapted to I/IV/V), phrase-tagged per 4-bar line so the phrase overlay shows the form | **Scored** | Plain chart over the 12-bar; combines everything above | `bent-licks`, `bar-counting` | accuracy ≥ 0.6 |
| **Chord-tone improvisation** (`chord-tone-improv`, `05_chord_tone_improv`) — open jam; don't just stay in the scale, *land on chord tones* when the chord changes | **Scored via proxy** | `PassCriteria::ChordToneAdherence`, reads `ImprovStats::chord_tone_adherence()` | `improvisation` (Unit 2), `blues-scale` | chord-tone fraction ≥ 0.4 |
| **Minor blues** (`minor-blues-improv`, `06_minor_blues`) — improvise over the minor blues progression; body copy covers what changes (b3 is home now) | **Scored via proxy** | `Progression::Minor` — the lesson manifest's `progression: "minor"` field seeds `menu::JamProgression` on Start | `chord-tone-improv` | `ScaleAdherence` ≥ 0.8 |
| **Question & answer** (`question-answer`, `07_question_answer`) — phrasing: improvise for 2 bars, *rest* for 2 bars, alternating through the form; leaving space is the lesson | **Scored via proxy** | `PassCriteria::PhraseDiscipline`, reads `ImprovStats::phrase_discipline()`; the 2-on/2-off pattern is fixed (`jam_session::PHRASE_PLAY_BARS`/`PHRASE_REST_BARS`) | `improvisation` | phrase discipline ≥ 0.7 |
| **Quick change improvisation** (`quick-change-improv`, `08_quick_change_improv`) — added in wave 3 (below), but lives here since it's blues-scale improv, not a new scale; same open jam as `improvisation`, backing progression set to `Progression::QuickChange` | **Scored via proxy** | `PassCriteria::ScaleAdherence` — existing `progression` manifest field, zero engine work | `improvisation` | `ScaleAdherence` ≥ 0.8 |

Like the `improvisation` lesson it builds on, each of the jam-criteria
lessons still needs a minimal `chart` field — a single long placeholder
`TrackItem` spanning the jam, supplying `song.key`/`harmonica` for
`JamHoleGuide` construction — even though no notes are individually scored.
(An earlier draft of this doc said these three needed "no chart"; that was
imprecise — the code has always required one, and the shipped lessons
follow that pattern.)

## Wave 3 — Scales and Improvisation — shipped

A new unit, `04_scales/` (`unit: "scales"`), 6 lessons — the direct
response to "there's nothing here about scales other than blues, or
improvising over anything but the blues scale." Two parts:

1. **Scale run drills, content-only** — three fixed charts teaching scale
   *shapes* the way `blues-scale` (Unit 3) already does, scored by plain
   accuracy: `major-scale` (1st position, reusing `deep-bends`'s
   whole-step draw bends for F/A), `minor-pentatonic-scale` (2nd
   position — literally the shipped `blues-scale` chart's fingering minus
   its flatted-5th step, since minor pentatonic *is* the blues hexatonic
   minus one degree), and `country-scale` (1st position major pentatonic,
   no bending at all — the accessible entry point of the three, unlocked
   off `multiple-notes` instead of `deep-bends`).
2. **True non-blues jam improvisation, needing new engine work** — Jam
   Session's live hole-map/scale-adherence feedback
   (`jam::session::JamHoleGuide`) was hardcoded to
   `blues_scale_classes` regardless of what a lesson or "Generate Jam"
   picked. Fixed by a new `JamScale` resource (`harmonicon-app`'s `app.rs`, mirrors the
   existing `JamProgression`) plus a `LessonManifest::scale` field
   (mirrors `progression`, parsed by `harmonicon-lessons::lesson_reader`)
   — `major-scale-improv` and `minor-pentatonic-improv` are the first
   lessons to actually exercise it, each pairing with its run-drill
   sibling (`song.key` matches, so the same notes taught in the drill are
   what the jam judges against). A chart's own declared
   `Harmonica::scale()` still wins over `JamScale` when it sets one (see
   `jam::session::setup`); "Generate Jam" also grew a Scale combobox
   alongside Progression/Position so this isn't lesson-only.

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **The major scale** (`major-scale`, `01_major_scale`) — 1st-position major scale up and down: 1B · 1D · 2B · 2D'' · 3B · 3D'' · 3D · 4B (the two `''` steps are the whole-step draw bends from `deep-bends`) | **Scored** | Plain chart + existing bend scoring | `deep-bends` (Unit 1) | accuracy ≥ 0.6 |
| **Minor pentatonic scale** (`minor-pentatonic-scale`, `02_minor_pentatonic_scale`) — 2nd-position minor pentatonic: `blues-scale`'s own fingering minus its 4D' (flatted 5th) step | **Scored** | Plain chart, reuses `blues-scale`'s bend | `blues-scale` (Unit 3) | accuracy ≥ 0.6 |
| **The Country scale** (`country-scale`, `03_country_scale`) — 1st-position major pentatonic, all-natural notes, no bending: 1B · 1D · 2B · 2D · 4B · 4D · 5B · 6B · 6D | **Scored** | Plain chart, no bend/technique scoring needed | `multiple-notes` (Unit 1) | accuracy ≥ 0.6 |
| **Major scale improvisation** (`major-scale-improv`, `04_major_scale_improv`) — open jam judged against the plain major scale instead of blues | **Scored via proxy** | `PassCriteria::ScaleAdherence` + `"scale": "major"` (new `JamScale` engine work) | `major-scale`, `improvisation` | `ScaleAdherence` ≥ 0.8 |
| **Minor pentatonic improvisation** (`minor-pentatonic-improv`, `05_minor_pentatonic_improv`) — open jam judged against the minor pentatonic scale | **Scored via proxy** | `PassCriteria::ScaleAdherence` + `"scale": "minor-pentatonic"` | `minor-pentatonic-scale`, `improvisation` | `ScaleAdherence` ≥ 0.8 |

`quick-change-improv` (content-only, no new scale) is the 6th lesson of
this wave but lives in `03_blues/` — see its row in Unit 3's table above.

**Known gap, not built in this wave:** the Song Editor's lesson-authoring
form (`song_editor::lesson_form`) still only round-trips `progression`,
not the new `scale` field — authoring a scale-based jam lesson through
the editor UI means hand-editing `lesson.json`'s `"scale"` key afterward.

## Wave 4 — Circle of fifths — shipped

One lesson, `circle-of-fifths` (`04_scales/06_circle_of_fifths`) —
instructional only, no chart, no pass criteria (Mark-as-Done). Makes
visible a relationship the engine already runs on but never showed: a
harmonica position (`song::harmonica::Position`) *is* a step count around
the circle of fifths — `interval_below_jam_key`'s existing 0/7/2 values
for 1st/2nd/3rd are exactly 0/1/2 fifths, just expressed in semitones.

Two new pieces, both generic (not lesson-specific):

- **`dialogs::circle_of_fifths`** — this crate's first circular UI layout
  (a `circle_point` pure function computing evenly-spaced points via
  `sin`/`cos`; every other absolutely-positioned UI elsewhere in this
  codebase is edge/percentage-anchored, never N points around a ring).
  `spawn_circle_of_fifths(harp_key, positions, colors)` iterates whatever
  `Position::all()` currently contains — it doesn't hardcode 1st/2nd/3rd,
  so extending `Position` needs no changes here. New `theme::
  CircleOfFifthsColors` (mirrors `TwelveBarColors`' exact shape) themes it.
- **`LessonManifest::diagram`** — a new schema-enforced enum field
  (`"circle-of-fifths"`, room for more later), mirroring `progression`/
  `scale`'s own shape. The lesson reader page
  (`harmonicon-lessons::lesson_reader::setup_lesson_reader`) was text-only before this
  — every prior instructional lesson is prose, some of it literally
  telling the player to go look at a live diagram elsewhere (`twelve-bar`'s
  own body text does exactly that) rather than embedding one. This is the
  first lesson with real embedded visual content.

Alongside it, `song::harmonica::Position` grew three more variants —
`Fourth`/`Fifth`/`Twelfth`, the next most commonly taught blues/jazz
positions beyond 1st–3rd (confirmed formula: position *N* sits
`((N-1) * 7) mod 12` semitones below the jam key, `Twelfth` being one step
the *other* way around the circle rather than 11 steps forward). The
entire compiler-enforced surface for new variants turned out to be two
functions in `harmonica.rs` (`all()`, `interval_below_jam_key()`) plus
`label()` — nothing outside that file matches over `Position`
exhaustively, so the Generate Jam Position combobox and this lesson's own
diagram picked the new positions up automatically, no other file touched.
Small, bounded follow-up if the editor UI is ever extended (mirror
`progression`'s existing click-to-cycle field).

## Wave 5 — Circle of fifths, live in Jam Session — shipped

Wave 4's diagram was purely instructional — nothing the player could
actually play against. This wave makes it a live playing aid and adds a
lesson that requires actually switching position, not just reading about
it.

- **`jam::position_guide`** — a live "position compass" (reusing
  `dialogs::circle_of_fifths` as-is) always spawned by `jam::session::setup`
  next to the harmonica bend diagram, showing the equipped harp's own key
  (detected from its hole-1 blow note — `song::harmonica::detected_harp_key`,
  factored out of `harp_banner` so both share it) and whichever `Position`
  the loaded chart declares.
- **`LessonManifest::position_cycle`** (a new schema-enforced boolean, seeded
  into a new `crate::app::JamPositionCycle` resource on Start, mirroring
  `progression`/`scale`): when on, `jam::position_guide::cycle_position`
  walks `JamScale` through `FirstPosition`/`SecondPosition`/`ThirdPosition`
  every 4 bars — the three `Scale` variants with a matching `Position`,
  i.e. the practical, stay-on-one-harp meaning of "switching position"
  (blues-hexatonic pitch classes rooted at the jam key +0/+7/+2 semitones).
  `on_position_called` re-highlights the compass and patches the live
  `JamHoleGuide::scale_classes` in place (it's normally computed once at
  session setup and never re-derived) so the *existing*
  `PassCriteria::ScaleAdherence`/`jam::improv::ImprovStats` machinery judges
  played notes against whichever position was actually called — no new
  scoring machinery, just a moving target for what already exists.
- **New lesson `circle-of-fifths-jam`** (`04_scales/07_circle_of_fifths_jam`)
  — an open Jam Session on a fixed C harmonica with `position_cycle: true`,
  prerequisites `circle-of-fifths` + `improvisation`, judged by
  `scale-adherence` at a 0.65 threshold (looser than the fixed-scale improv
  lessons' 0.8, since chasing a moving target is harder).

### Unit 5 — jazz (`05_jazz/`) — 4 of 5 lessons shipped

Its own engine work (jazz chord-tone tables, a jazz-blues `Progression`
variant) was already done before this unit's content — see "Engine work
(done)" below — so all four drills below needed zero further engine
changes, only content:

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **Swing eighths** (`swing-eighths`, `01_swing_eighths`) — swung-eighth drills at a tighter timing window than `shuffle-feel`, alternating a 4-hole and a 5-hole blow/draw pair | **Scored** | `feel: shuffle` + tighter per-chart windows (100/220/380ms vs. `shuffle-feel`'s 150/350/550), zero engine work | `shuffle-feel` (Unit 2) | accuracy ≥ 0.6 |
| **ii-V-I chord tones** (`ii-v-i-chord-tones`, `02_ii_v_i_chord_tones`) — the Dm7-G7-Cmaj7 turnaround in C, each chord arpeggiated low to high | **Scored** | Plain chart, notes computed from `song::harmonica::ii_v_i_chords("C", false)`; reuses `deep-bends`'s whole-step draw bends for F/A | `swing-eighths`, `deep-bends` (Unit 1) | accuracy ≥ 0.6 |
| **The jazz blues form** (`jazz-blues-form`, `03_jazz_blues_form`) — open jam over the full jazz-blues progression (a real ii-V-I turnaround in the last few bars) | **Scored via proxy** | `PassCriteria::ChordToneAdherence` + existing `progression: "jazz-blues"` field — the same mechanism `minor-blues-improv`/`quick-change-improv` already use, just a different `Progression` value | `ii-v-i-chord-tones`, `chord-tone-improv` (Unit 3) | chord-tone fraction ≥ 0.4 |
| **Chromatic slide basics** (`chromatic-slide-basics`, `04_chromatic_slide_basics`) — a full ascending/descending chromatic scale on a 12-hole chromatic harp, computed directly from the chromatic layout tables (not transcribed) | **Scored** | `Modifier::Slide` onset scoring, already built; first bundled chromatic lesson content | `swing-eighths` | technique `slide` ≥ 0.5 |
| **Jazz heads** — actual repertoire | **Scored** | Ordinary charts | Rights-sensitive: public-domain only, human judgment required (`TODO.md`) — **deliberately not built**; needs specific pieces confirmed public domain before charting |

### Unit 6 — navigation (`06_navigation/`) — 8 of 8 lessons shipped

Hole navigation and register changes, from the "highest priority"
curriculum gap in `docs/lesson_tree_layout_plan.md`. Needed no engine work
at all: finding a hole *is* producing its pitch, so every drill is judged
by the ordinary `accuracy` criterion against a plain chart.

Appended as the last unit rather than inserted after `blowing`, which is
where it belongs pedagogically — inserting would have re-locked every
later unit for anyone mid-course, since `is_unlocked` requires *all*
earlier units satisfied. Revisit if the chain is ever renumbered.

All eight use `track: "navigation"` and a C diatonic. The unit leans on
one fact about Richter tuning: blow 1, 4, 7 and 10 are all C, an octave
apart, which is what makes them usable as landmarks.

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **Landmarks** (`landmarks`, `01_landmarks`) — blow 1/4/7/10, up and back | **Scored** | Plain chart; the four octave C's | `single-note` (Unit 1) | accuracy ≥ 0.6 |
| **The middle register** (`middle-register`, `02_middle_register`) — holes 4–6 blow/draw, spelling C D E F G A | **Scored** | Plain chart | `landmarks` | accuracy ≥ 0.6 |
| **Jumping between holes** (`hole-jumps`, `03_hole_jumps`) — octave leaps between landmarks, landing without sliding | **Scored** | Plain chart; all targets are C, so a wrong landing is audible | `landmarks` | accuracy ≥ 0.55 |
| **Crossing 3–4** (`crossing-three-four`, `04_crossing_three_four`) — G B C D across the low/middle seam | **Scored** | Plain chart | `middle-register` | accuracy ≥ 0.6 |
| **Crossing 6–7** (`crossing-six-seven`, `05_crossing_six_seven`) — G A B C across the blow/draw inversion | **Scored** | Plain chart | `middle-register` | accuracy ≥ 0.6 |
| **The low register** (`low-register`, `06_low_register`) — holes 1–3, slower warmer air | **Scored** | Plain chart. Takes G from draw 2, **not** blow 3 — see the duplicate-pitch note below | `crossing-three-four` | accuracy ≥ 0.55 |
| **The high register** (`high-register`, `07_high_register`) — holes 7–10, draw below blow throughout | **Scored** | Plain chart | `crossing-six-seven` | accuracy ≥ 0.55 |
| **Register leaps** (`register-leaps`, `08_register_leaps`) — capstone, full range | **Scored** | Plain chart | `low-register`, `high-register` | accuracy ≥ 0.5 |

**The one honesty caveat.** The engine judges pitch, not which hole
produced it. On a C diatonic that is a distinction without a difference
everywhere except **G4, which is both blow 3 and draw 2** — the only
duplicated pitch on the instrument. No drill here is allowed to depend on
telling those two apart: `low-register` deliberately takes its G from
draw 2 so every note of that exercise is a distinct pitch. Any future
navigation lesson must respect the same constraint, or it will score a
player correct for landing on the wrong hole.

### Unit 7 — advanced (`07_advanced/`) — 3 lessons shipped

An elective-only unit for techniques that add range and expression without
blocking the main course. A unit with no core lessons has a zero unlock
requirement, and every lesson here declares `optional: true`.

| Lesson (id, folder) | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **High blow bends** (`high-blow-bends`, `01_high_blow_bends`) — half-step bends on 8/9/10, the whole-step 10 bend, and a descending application | **Scored** | Existing bend detection and generated bend ladder over holes 8/9/10 | `deep-bends`, `high-register` | technique `bend` ≥ 0.55 |
| **Overblows** (`overblows`, `02_overblows`) — isolate holes 4/5/6 and release to their natural reeds | **Scored** | Existing overblow pitch and technique bucket | `deep-bends`, `high-blow-bends` | technique `overblow` ≥ 0.5 |
| **Overdraws** (`overdraws`, `03_overdraws`) — isolate holes 7–10 against adjacent natural notes | **Scored** | Existing overdraw pitch and technique bucket | `overblows`, `high-register` | technique `overdraw` ≥ 0.5 |

### Unit 8 — rhythm lab (`08_rhythm_lab/`) — 5 lessons shipped

The second rhythm module moves from recognizing the blues pulse to controlling
subdivisions, silence, articulation, and changing tempos. Every chart is an
original rhythm exercise. Tick-based placement keeps triplets and sixteenths
exact, and the final drill exercises multiple tempo-map boundaries.

| Lesson | Scoreable? | Mechanism | Prereq | Pass |
|---|---|---|---|---|
| **Eighth-note triplets** (`eighth-note-triplets`) | **Scored** | Tick-aligned three-part subdivisions | `shuffle-feel` | accuracy ≥ 0.65 |
| **Straight versus shuffle** (`straight-vs-shuffle`) | **Scored** | Equal eighths followed by long-short pairs | `eighth-note-triplets` | accuracy ≥ 0.7 |
| **Syncopation and rests** (`syncopation-and-rests`) | **Scored** | Offbeat onsets with explicit empty beats | `straight-vs-shuffle` | accuracy ≥ 0.7 |
| **Sixteenth-note articulation** (`sixteenth-articulation`) | **Scored via proxy** | Fresh-attack detection on four-note cells | `syncopation-and-rests`, `articulation` | clean attacks ≥ 0.65 |
| **Tempo stability** (`tempo-stability`) | **Scored** | Tight timing across 64, 72, 80, and 68 BPM map segments | `sixteenth-articulation` | accuracy ≥ 0.75 |

### Unit 9 — applied positions (`09_positions/`) — 3 lessons shipped

The circle and position-cycle lessons explain and demonstrate position
changes. These drills give first, second, and third position a playable home:
each uses the same C diatonic but changes the song key, tonic landmarks, and
chord-tone destinations. Pitch accuracy can score every landing directly.

| Lesson | Position and key | Prereq | Pass |
|---|---|---|---|
| **First-position landings** (`first-position-landings`) | 1st, C; resolves on 4/7 blow | `major-scale` | accuracy ≥ 0.65 |
| **Second-position landings** (`second-position-landings`) | 2nd, G; resolves on 2 draw | `first-position-landings`, `blues-scale` | accuracy ≥ 0.65 |
| **Third-position landings** (`third-position-landings`) | 3rd, D minor color; resolves on 4/8 draw | `second-position-landings`, `minor-pentatonic-scale` | accuracy ≥ 0.65 |

### Unit 10 — ear training (`10_ear_training/`) — 4 lessons shipped

These drills reuse synthesized call-and-response, but declare `aural: true`.
That manifest flag withholds scrolling note prompts while leaving the call,
forced response wait, pitch scoring, and feedback intact. The sequence moves
from one-note matching through contour and phrase memory to tonic resolution.

| Lesson | Listening task | Prereq | Pass |
|---|---|---|---|
| **Match a heard note** (`match-heard-note`) | Find middle-register pitches from sound | `middle-register`, `call-response` | accuracy ≥ 0.65 |
| **Up, down, or same** (`melodic-motion`) | Recognize and reproduce melodic direction | `match-heard-note` | accuracy ≥ 0.7 |
| **Repeat a hidden phrase** (`hidden-short-phrase`) | Retain contour and rhythm without prompts | `melodic-motion` | accuracy ≥ 0.7 |
| **Find the tonic** (`find-the-tonic`) | Resolve heard phrases to C by ear | `hidden-short-phrase`, `first-position-landings` | accuracy ≥ 0.75 |

### Unit 11 — tongue-block electives (`11_tongue_block/`) — 6 lessons shipped

Slap, pull, side-pull, rake, and flutter remain instructional because a
microphone cannot identify the player's embouchure from the resulting sound.
Moving octaves provides the scored capstone: every target is a simultaneous
pair of blow notes exactly one octave apart, which chord scoring can verify.

| Lesson | Classification | Prereq |
|---|---|---|
| **Tongue slap** (`tongue-slap`) | Instructional | `tongue-blocking` |
| **Tongue pull** (`tongue-pull`) | Instructional | `tongue-slap` |
| **Side pull** (`side-pull`) | Instructional | `tongue-pull` |
| **Tongue rake** (`tongue-rake`) | Instructional | `tongue-pull` |
| **Tongue flutter** (`tongue-flutter`) | Instructional | `tongue-rake` |
| **Moving octaves** (`moving-octaves`) | Scored through simultaneous octave pitches, accuracy ≥ 0.6 | `octave-split`, `side-pull` |

### Unit 12 — ornaments (`12_ornaments/`) — 4 lessons shipped

This elective sequence scores only the produced pitch order and timing. It
does not claim to distinguish whether the player moved their head, hands,
tongue, or instrument. Slow alternation establishes control before the faster
shake and trill cells; glissandi finish with measured multi-hole sweeps.

| Lesson | Observable outcome | Prereq | Pass |
|---|---|---|---|
| **Slow warble** (`slow-warble`) | Clear adjacent-hole alternation | `middle-register` | accuracy ≥ 0.65 |
| **Fast shake** (`fast-shake`) | Even sixteenth-note alternation | `slow-warble`, `sixteenth-articulation` | accuracy ≥ 0.7 |
| **Trill cells** (`trill-cells`) | Timed neighbor-note cells | `fast-shake` | accuracy ≥ 0.7 |
| **Glissandi** (`glissandi`) | Ordered sweep and destination | `trill-cells`, `slides` | accuracy ≥ 0.65 |

### Unit 13 — expression (`13_expression/`) — 6 lessons shipped

Hand, throat, and diaphragm vibrato are separate instructional approaches;
the microphone cannot attribute the motion that produced a modulation.
`vibrato-control` scores the pitch-oscillation rate the existing analyzer can
measure. Crescendo/diminuendo remains instructional because no envelope
criterion exists. The long-tone capstone scores sustained pitch and duration.

| Lesson | Classification | Prereq |
|---|---|---|
| **Hand vibrato** (`hand-vibrato`) | Instructional | `hand-wah`, `vibrato` |
| **Throat vibrato** (`throat-vibrato`) | Instructional | `vibrato` |
| **Diaphragm vibrato** (`diaphragm-vibrato`) | Instructional | `breathing`, `vibrato` |
| **Vibrato rate control** (`vibrato-control`) | Measured 3 Hz and 5 Hz oscillation, technique ≥ 0.6 | all three families |
| **Crescendo and diminuendo** (`dynamic-shaping`) | Instructional | `breathing` |
| **Long-tone control** (`long-tone-control`) | Sustained pitch/duration, accuracy ≥ 0.75 | `dynamic-shaping`, `vibrato-control` |

### Engine work (done)

Unit 14 (`14_accompaniment/`) adds four elective accompaniment lessons:
scored chord rhythm, instructional singer support, timed vocal-gap fills, and
trading fours judged with the existing phrase-discipline criterion.

All three wave-2 engine items are built (see "Wave 2, part 1 — shipped"
above): `PassCriteria::ChordToneAdherence`/`PhraseDiscipline`, the lesson
manifest's `progression` field, and `jam_session::in_rest_window` +
`ImprovStats::rest_violations`/`chord_tone_adherence`/`phrase_discipline`.
`harmonicon-lessons::lesson_reader::is_jam_criteria` routes all three jam-based criteria (plus
`ScaleAdherence`) into `GameplayMode::JamSession`;
`gameplay::pause_menu::jam_fraction_for` picks the right `ImprovStats`
fraction for whichever criterion a given lesson declares. Unit 3 (above)
used every one of these with no further engine changes. Wave 3's `JamScale`
resource + `LessonManifest::scale` field (see above) is the other piece of
engine work landed so far. Unit 5 (jazz) below needed none of its own —
every mechanism its four shipped lessons use already existed; only "jazz
heads" (rights-blocked content, not an engine gap) remains.

Cross-cutting authoring notes:

- Every new lesson: `lesson.json` + locale keys in all three languages
  (`tests/asset_layout.rs` enforces schema, prereq integrity, and key
  existence — it will catch omissions).
- Cross-unit prerequisites (`train-chug` ← `multiple-notes`,
  `blues-scale` ← `deep-bends`) are just ids — `is_unlocked` doesn't care
  about units — but double-check the skill tree presents a locked
  lesson's prerequisite name legibly when it lives in another unit.
- New charts using only existing schema features need no `format_version`
  bump; `train-rolling`'s tempo map and the multi-modifier charts all use
  long-supported fields.

### Suggested build order (what's left)

The remaining curriculum work is guided practice skills.
The chromatic branch now ends in an original jazz study, avoiding the earlier
repertoire-rights blocker. Reduced motion belongs to shared application
settings; a lesson-map minimap remains conditional on usability evidence.
