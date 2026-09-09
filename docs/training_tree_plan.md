# Training tree: lessons, trainings, and a graph to walk them

Design plan for turning the Lessons feature into a technique DAG with
practice trainings hanging off each lesson, shown as a top-down skill
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
what the skill-tree view draws.

**An improvisation lesson belongs to the track of whatever it improvises
over**, not to a general "improv" bucket. `major-scale-improv` sits in
`scales` after `major-scale`; `quick-change-improv` sits in `form`, because
the new thing it asks for is the form. This is what keeps the rows even —
a bucket collected eleven lessons while nothing else had more than five —
and it gives every track the same arc: **learn it, apply it, improvise with
it**. It also leaves an obvious empty slot at the end of the technique
tracks (an "improvise with bends" for `bend`, a "improvise the train" for
`train`) which is where new lessons should go.

| Track | Lessons today | |
|---|---|---|
| `scales` | blues-scale, major-scale, major-scale-improv, minor-pentatonic-scale, minor-pentatonic-improv, country-scale | 6 |
| `form` | twelve-bar, bar-counting, turnaround, quick-change-improv, jazz-blues-form | 5 |
| `tone` | single-note, breathing, articulation, multiple-notes | 4 |
| `time` | counting-four, shuffle-feel, swing-eighths, using-your-feet | 4 |
| `train` | train-chug, train-rolling, train-whistle | 3 |
| `harmony` | chord-tone-improv, minor-blues-improv, ii-v-i-chord-tones | 3 |
| `vocabulary` | first-licks, bent-licks, licks-over-changes | 3 |
| `improv` | call-response, improvisation, question-answer | 3 |
| `tongue` | tongue-blocking, octave-split | 2 |
| `bend` | first-bend, deep-bends | 2 |
| `slide` | slides, chromatic-slide-basics | 2 |
| `theory` | circle-of-fifths, circle-of-fifths-jam | 2 |
| `hand` | hand-wah | 1 |
| `vibrato` | vibrato | 1 |

Fourteen tracks, all 41 lessons placed exactly once, longest track 6.
`unit` stays as it is (it groups the list view and the locale keys);
`track` is purely the drawing and progression spine.

**A track is a family ordered by depth, not a dependency chain.** This is
the one place the reference screenshot differs: there, each row is strictly
sequential — buy node 1, then 2. Here `country-scale` does not lead to
`blues-scale`; they merely belong together. So the row draws its real
prerequisite edges and nothing else, and its left-to-right order is by
graph depth. Inventing prerequisites to force every row into a chain would
be lying about the curriculum to tidy the picture.

**Validation — built, and three of the four planned checks turned out not
to exist.** `lessons::graph` (pure, Bevy-free) owns `depth`, topological
order, `available`, `rows` and the checking. What survived contact:

- **No cycles — real, and genuinely unchecked before.** A lesson in a cycle
  can never unlock, and a renderer walking those edges would not terminate.
  `LessonGraph::build` refuses to construct rather than handing back
  something that looks fine until someone walks it. Downstream lessons are
  named in the same error, so fixing one cycle doesn't just reveal another.
- *Every edge points forward* — **vacuous**. Depth is the longest path from
  a root, so a prerequisite's depth is always below its dependent's by
  construction. There was never anything to check.
- *Every lesson reachable from a root* — **vacuous for the same reason**.
  Any node of a finite DAG is reachable from some root; the only real
  failure is a prerequisite naming a lesson that does not exist, which is
  its own error (`UnknownPrerequisite`) because the fix is different.
- *At least two lessons always available* — **false of the shipped
  curriculum**, so it reports rather than asserts. Sampled over the 41,
  the graph funnels to a single option at several points:

  | Chokepoint | How often it is the only option |
  |---|---|
  | `deep-bends` | 135 |
  | `swing-eighths` | 83 |
  | `blues-scale` | 80 |
  | `improvisation` | 64 |
  | `call-response` | 45 |

  (3000 seeded playthroughs, counting only states with ≥4 lessons still to
  go.) `graph::min_choices` returns the number so it can be watched;
  widening those gateways is curriculum work, not something a test can
  force. Asserting a threshold would either fail today or have to be
  weakened until it only described today's data.


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
end and actually play it before rolling out the other thirteen.

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

## 3. The skill-tree view

Flows **top-down**: tracks stack downward, each track reads left to right,
the page scrolls vertically. That is the reference screenshot's layout and
it needs no horizontal scrolling at all.

An earlier draft of this plan claimed horizontal panning would be needed.
That was wrong — it was written without measuring. With the redistribution
above:

| | |
|---|---|
| longest track | 6 nodes → **600 px** wide |
| 14 tracks at 64 px | **896 px** tall |

Wide enough to fit any window; tall enough to need vertical scrolling,
which `dialogs::scroll_area` already does. Nothing new is required.

**Superseded once it was built and looked at.** Track-as-row reads as a
*grid*: every row starts at column 0 and marches right in lockstep, so a
node's position says nothing about where it sits in the curriculum. What
works is a **layered drawing**: a node's column is its depth in the
prerequisite graph, its row is chosen to keep edges from crossing
(barycentre sweeps), and tracks become **colour** rather than rows. The
curriculum was also given a **single root** — `single-note` is now a
prerequisite of `twelve-bar` and `counting-four` — so the tree opens from
one place instead of three unrelated starts, which is the point: the basics
come first and everything fans out of them.

Measured on the real curriculum: 6 columns, widest layer 10, so about
900 × 920 px. Still fits horizontally and scrolls vertically.

Edges are real cubic Beziers. `bevy_ui` has no line primitive, but that is
a fact about *drawing*, not about the engine: `bevy_math::CubicBezier`
computes the curve, `iter_positions` samples it, and each sample pair
becomes a short `Node` rotated by `UiTransform::rotation` (a first-class UI
field in Bevy 0.19). No shader and no new dependency.

### The node

**A node is a circle carrying an image** — `BorderRadius::MAX` on a square
node with the lesson's art inside, the same shape the reference screenshot
uses for its skill icons.

Art is a per-lesson `icon.png` beside the `lesson.json`, and it is
**optional**: when absent the node falls back to
`assets/icons/lesson_placeholder.png`, a neutral hatched disc that is
deliberately obvious as placeholder art so it cannot be mistaken for
finished work. That mirrors how a song's `background.png` already works —
every sibling asset optional, with a fallback — so lessons can be given
real icons one at a time as the art appears, rather than all 41 at once.

**Trainings are not nodes — they are the ring.** Five extra nodes per
lesson would put the longest track at 36 across and bring the width problem
straight back. Instead the five training tiers are five segments around the
circle's edge, filling as each tier is passed. Going circular makes this
better than the strip an earlier draft proposed: the mastery meter becomes
a literal progress ring around the lesson's own art, so points 2, 3 and 4
of the brief render as one control rather than three.

Segments are plain absolutely-positioned nodes with `BorderRadius::MAX`,
not a shader — five pips around a circle needs no new material. A real arc
shader is the fallback if that reads poorly at node size;
`music_score::tie_material` is the precedent for adding one.

Node states, and how each reads:

| State | Treatment |
|---|---|
| `Locked` | desaturated art, padlock overlay, ring empty |
| `Available` | full-colour art, highlighted ring outline |
| `Passed` | full colour, lesson tick, ring shows the tiers earned so far |
| `Mastered` | all five ring segments filled |

Implementation notes specific to this codebase:

- **Layout is a pure function**, per the repo's testing convention:
  `layout(lessons, profile) -> Vec<PlacedNode>` returning row, column,
  state and edges, tested without Bevy. The spawn code is a translation of
  its output, as `music_score` does.
- **Keyboard navigation is mandatory** (root `CLAUDE.md`): every node is a
  real `bevy_ui_widgets::Button` with `TabIndex(0)`, the graph root gets a
  `TabGroup`, handlers are `On<Activate>`. Tab order follows reading order,
  track by track.
- **Keep the list view for compact layouts.** `responsive::is_compact`
  exists; fourteen rows of nodes on a phone is unreadable. The tree is the
  wide-screen presentation, not a replacement.
- Track labels and node titles are Fluent keys, three locales, as ever.

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

1. **A mastery meter per track, not XP.** 0–100% from tier progress and
   recent accuracy, and the per-node pip strip in §3 is its node-level
   form. Makes competence visible (SDT) without inventing a currency.
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

0. ~~`lessons::graph` — tracks, depth, validators, cycle test.~~ **Done.**
   `track` on all 41 lessons (optional in the schema, falling back to
   `unit` so externally authored lessons still land somewhere), the graph
   module, 17 tests, and an asset test that builds the shipped curriculum.
1. `training::drill_chart` + the tier ladder, for the `bend` track only.
   Play it. Decide whether generated drills are good enough before
   committing to the other thirteen tracks.
2. ~~`TrainingRecord` in the profile, and results routing for trainings.~~
   **Done**, and playable: the lesson reader carries a row of five tier
   buttons, so a training can be started before the tree exists. Records go
   to `PlayerProfile::trainings`, never `lessons` — a passed tier must not
   satisfy a prerequisite. `app::GeneratedSong` was split out of
   `GeneratedJamSession`, which conflated "built by `Assets::add`, no
   `LoadState`" with "this is a jam"; a training is the former and
   emphatically not the latter.
3. The tree view: pure layout first, then spawning. Largest single chunk,
   but no new scrolling machinery — it fits vertically.
4. Gamification: the mastery meter, then the review queue, then the streak.
5. Roll trainings out across the remaining tracks.

## Decided

- **Trainings do not gate progression.** Passing the lesson unlocks what
  follows, as today; trainings feed mastery and the review queue.
- **Top-down, vertical scrolling only.**
- **Improvisation lessons distribute into the track they improvise over**
  rather than forming a bucket, which is what evened the rows out.

## Open questions

- **Does the flat list survive** as the compact-layout view, or get
  replaced outright?
- `hand` and `vibrato` are single-lesson rows and will look thin next to
  `scales`. Fold them into `tone`, or leave the gap as somewhere the
  curriculum visibly wants more lessons?
