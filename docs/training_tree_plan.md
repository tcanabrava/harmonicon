# Training tree: lessons, trainings, and a graph to walk them

Design plan for turning the Lessons feature into a technique DAG with
practice trainings hanging off each lesson, shown as a horizontal skill
tree. Curriculum content design stays in `docs/lessons_plan.md`; this
covers structure, progression, UI and motivation.

Nothing here is built yet.

## What already exists

Worth stating plainly, because roughly half of point 1 is already done and
the plan should not re-propose it.

- **41 lessons across 5 units**, each a folder with a `lesson.json` and
  usually a `.harpchart` (`assets/lessons/<unit>/<lesson>/`).
- **A real prerequisite DAG.** `LessonManifest::prerequisites: Vec<String>`
  is populated across the whole curriculum — `bar-counting` needs
  `counting-four` *and* `twelve-bar`, `train-whistle` needs `train-rolling`
  *and* `hand-wah`, and so on. `lessons::is_unlocked` gates on it.
- **Five pass criteria** already judge real things: `Accuracy`, `Technique`
  (per-technique hit rate), `ScaleAdherence`, `ChordToneAdherence`,
  `PhraseDiscipline`.
- **Per-lesson progress**: `LessonRecord { passed, best_accuracy, attempts }`,
  and `DrillRecord { attempts, hits }` per (hole, technique) from the
  Bending Trainer's adaptive drill.
- **Difficulty machinery**: `gameplay::adaptive_difficulty` already unlocks
  a chart phrase by phrase as the player earns it.
- **Generated charts have precedent**: `jam::backing::generated_chart(key,
  bpm, progression, position, genre, total_secs)` builds a playable chart
  from parameters, with tests.
- **Asset validation**: `tests/asset_layout.rs` checks schema, file
  completeness, locale keys, and that every prerequisite names a lesson
  that exists.

What is missing: the graph is never *shown* (the Lessons page is a flat
list behind unit tabs), there is no cycle check on it, there is no notion
of a technique track, and there is nothing between "read the lesson" and
"you're done".

## 0. The distinction this introduces

- A **lesson** teaches. Text, optionally a diagram, and a guided chart the
  student can follow — explanation present, aids on.
- A **training** does not teach. It presents a variation of the same
  material with no explanation, harder than the last one, for the student
  to keep going.

## 1. The technique DAG

The prerequisite edges exist; what's missing is a **spine** to lay them out
along and the checks that keep the graph drawable.

**Add `track` to the manifest** — the row a lesson belongs to, which is
what the skill-tree view draws. Proposed tracks, derived from the lessons
already written:

| Track | Lessons today |
|---|---|
| `tone` | single-note, breathing, articulation |
| `hand` | hand-wah |
| `tongue` | tongue-blocking, octave-split |
| `bend` | first-bend, deep-bends |
| `vibrato` | vibrato |
| `slide` | chromatic-slide-basics |
| `time` | counting-four, shuffle-feel, swing-eighths, using-your-feet |
| `form` | twelve-bar, bar-counting, turnaround |
| `train` | train-chug, train-rolling, train-whistle |
| `scales` | blues-scale, major-scale, minor-pentatonic-scale, country-scale, circle-of-fifths |
| `vocabulary` | first-licks, bent-licks, licks-over-changes |
| `improv` | call-response, improvisation, question-answer, chord-tone-improv, minor-blues, quick-change, the -improv scale lessons, jazz-blues-form, ii-v-i |

Twelve tracks against the screenshot's nine — close enough that the layout
reads the same. `unit` stays as it is (it groups for the list view and the
locale keys); `track` is purely the drawing/progression spine.

**New validation, as tests** — the layout is only drawable if the graph
behaves, and none of this is checked today:

- **No cycles.** Nothing checks this now. A cycle would lock every lesson
  in it forever and the renderer would not terminate.
- **Every lesson reachable** from a no-prerequisite root.
- **Cross-track edges point forward**, i.e. a prerequisite is never at a
  greater depth than its dependent, so no edge is drawn backwards.
- **At least two nodes available at every reachable progress state** — see
  §4, autonomy. This one is a motivation constraint expressed as a graph
  property, and it is checkable.

A `lessons::graph` module (pure, Bevy-free, in `harmonicon-song`) owning
`depth()`, `topological order`, `available(profile)` and the validators.

## 2. Five trainings per lesson

### The ladder

One ladder for every technique, so the shape is learnable:

| Tier | Name | What changes |
|---|---|---|
| 1 | Isolate | the technique alone, slow, adaptive difficulty on, notation on, metronome only |
| 2 | Consolidate | same material, tempo up ~20%, adaptive off |
| 3 | Vary | different holes/intervals, same technique, tempo up again |
| 4 | In context | the technique inside a musical phrase, over a backing progression |
| 5 | Interleave | mixed with two already-passed techniques, target tempo, no aids |

Isolate → consolidate → vary → contextualise → interleave is the standard
motor-skill progression, and every knob it turns already exists: tempo,
`AdaptiveDifficulty::enabled`, chart generation, jam backing.

### Generated, not authored

41 lessons × 5 tiers is 205 charts. Hand-authoring those is not realistic,
and it is the kind of bulk content the standing rule says not to produce
unsupervised.

So a training is a **spec, not a file**: technique, holes, tier, and a
fixed seed. A new pure `training::drill_chart(spec) -> HarpChart` in
`harmonicon-core` (sibling to what `jam::backing::generated_chart` already
does for backing tracks), deterministic on the seed so re-attempting gives
the *same* exercise.

```json
"training": {
  "technique": "bend",
  "holes": [2, 3, 4],
  "seed": 5170
}
```

Tiers come from the shared ladder. A tier may be overridden with a real
authored chart (`"chart": "trainings/04.harpchart"`) where the generator
isn't good enough — the generator is the default, not the ceiling.

**The main risk in this whole plan is that generated drills are musically
dull.** Mitigation: build the generator for *one* track (`bend`) end to
end and actually play it before rolling out the other eleven.

### Not every lesson gets trainings

`docs/lessons_plan.md` already establishes that some lessons are
instructional-only because the microphone cannot verify the technique —
tongue-blocking is the clear case, circle-of-fifths another. Forcing five
trainings onto a lesson with nothing scoreable would be five exercises
pretending to check something they can't.

Trainings exist only where a real `pass_criteria` does: roughly 30 of the
41. The plan should not claim 205.

### Do trainings gate progression?

**Recommendation: no.** Passing the *lesson* unlocks what comes next, as
today; trainings are optional practice that feed mastery and the review
queue. Requiring five trainings per lesson would make the curriculum six
times longer and remove the student's choice of what to work on.

This is a real design fork and worth your call before anything is built.

## 3. The horizontal graph view

Matching the reference: rows labelled at the left, nodes chained left to
right with connectors, vertical connectors where one row branches from
another, locked nodes desaturated with a padlock, a summary bar at the
bottom.

Mapping onto the data: **row = track**, **first node = the lesson**,
**following nodes = its five trainings**, **vertical connectors =
prerequisite edges between tracks**. That is the same shape as the
screenshot without forcing anything.

Node states: `Locked` / `Available` / `Passed` / `Mastered` (all five
trainings passed).

Implementation notes specific to this codebase:

- **Layout is a pure function**, per the repo's testing convention:
  `layout(lessons, profile) -> Vec<PlacedNode>` returning row, column,
  state and edges, tested without Bevy. The spawn code is a translation of
  its output, as `music_score` does.
- **Horizontal panning is new work.** `dialogs::scroll_area` is
  `Overflow::scroll_y()` only. Either extend it to take an axis or add a
  pan-and-scrollbar view; `song_editor::scroll` already solves the
  horizontal case and is the thing to copy.
- **Keyboard navigation is mandatory** (root `CLAUDE.md`): every node is a
  real `bevy_ui_widgets::Button` with `TabIndex(0)`, the graph root gets a
  `TabGroup`, handlers are `On<Activate>`. Tab order follows reading order.
- **Keep the list view for compact layouts.** `responsive::is_compact`
  exists; a twelve-row graph on a phone is unreadable. The graph is the
  wide-screen presentation, not a replacement.
- Row labels and node titles are Fluent keys, three locales, as ever.

## 4. Gamification

I read around this before proposing anything, because the obvious version
(points, badges, leaderboards) is the version the evidence supports least.

### What the research actually says

- **Deliberate practice** (Ericsson): improvement comes from targeting a
  specific weakness just beyond current ability, with immediate feedback
  and repetition — not from time spent.
- **The spacing effect** (Ebbinghaus; Cepeda et al.'s 2006 meta-analysis):
  distributed practice beats massed practice for long-term retention, and
  the effect is large.
- **Interleaving and "desirable difficulties"** (Shea & Morgan; Bjork):
  mixing skills within a session makes practice *feel worse* and
  performance during practice drop, while improving retention and
  transfer. This is best established for **motor** learning, which playing
  an instrument is.
- **Self-Determination Theory** (Deci & Ryan): intrinsic motivation rests
  on autonomy, competence and relatedness. The **overjustification effect**
  is the warning: extrinsic rewards attached to an activity someone
  already enjoys can *reduce* their motivation to do it. Playing music is
  exactly such an activity.
- **Goal-setting theory** (Locke & Latham): specific, moderately difficult
  goals outperform "do your best".
- **Flow** (Csikszentmihalyi): sustained engagement needs challenge
  tracking skill.
- **Gamification meta-analyses** (e.g. Hamari et al. 2014): effects are
  positive on average but modest and highly variable, and much of the gain
  decays as novelty wears off. Points/badges/leaderboards is the weakest
  and most-copied form.

The through-line: for a music learner, the mechanics that help are the ones
that **schedule practice well and make competence visible**. The ones that
bolt a currency onto playing music are the ones most likely to backfire.

### What to build

1. **Mastery bars per track, not XP.** 0–100% from tier progress and recent
   accuracy. Makes competence visible (SDT) without inventing a currency.
2. **A spaced review queue — "Warm-up".** Record when each technique was
   last passed; surface two or three due for review at the top of the tree.
   This is the highest-value item on the list and it is a *feature*, not a
   reward. It needs one new field per record and a due-date function.
3. **A practice streak with forgiveness.** Count practice days, never frame
   it as loss, auto-forgive a missed day. Habit support without the
   anxiety that streak mechanics are fairly criticised for.
4. **An explicit goal on every training.** "80% of the bends at 90 BPM" —
   specific and moderately hard, per Locke & Latham. Already expressible
   with existing `pass_criteria`; it needs *stating* in the UI.
5. **Tier 5 is interleaved by design**, so desirable difficulty is built
   into the ladder rather than added later.
6. **Never a single forced next step.** Keep ≥2 nodes available at all
   times (the graph property in §1) so the student always chooses — this
   is SDT's autonomy leg, expressed as something a test can enforce.

### What not to build, and why

Leaderboards, randomised or loot-style rewards, hearts/lives that stop a
student practising, and XP that pays out for time rather than improvement.
The first three are competence- and autonomy-hostile; the last rewards the
wrong variable. Given overjustification, the risk with a music app is not
that these do nothing — it is that they make playing feel like work.

## Order of work

Each phase is independently shippable and the later ones can be dropped
without stranding the earlier ones.

0. `lessons::graph` — tracks, depth, validators, cycle test. Data only, no
   UI. Cheap, and it unblocks everything else.
1. `training::drill_chart` + the tier ladder, for the `bend` track only.
   Play it. Decide whether generated drills are good enough before
   committing to the other eleven tracks.
2. `TrainingRecord` in the profile, and results routing for trainings.
3. The graph view: pure layout first, then spawning, then horizontal
   panning. Largest single chunk.
4. Gamification: mastery bars, then the review queue, then the streak.
5. Roll trainings out across the remaining tracks.

## Open questions

- **Do trainings gate progression?** Recommendation above is no. Your call.
- **Twelve tracks or fewer?** `scales` and `improv` are large enough to
  split; `hand`, `vibrato` and `slide` are single-lesson rows today and
  will look thin next to the others.
- **Does the flat list survive** as the compact-layout view, or get
  replaced outright?
