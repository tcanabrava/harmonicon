---
name: add-lesson
description: Author a new Harmonicon lesson in the harmonicon-lessons pack (../harmonicon-lessons/<unit>/<lesson>/), or add the engine support a new lesson needs (a widget, a pass criterion). Covers the manifest schema, chart layout, the pack's own translations, prerequisite integrity, LESSON_FORMAT_VERSION and what is honestly scoreable.
---

# Authoring a lesson

Lessons are not in this repository. They live in the **lesson pack**
`harmonicon-lessons` (checked out beside this repo as `../harmonicon-lessons`,
published at `github.com/tcanabrava/harmonicon-lessons`), which the game
downloads at first start. Its README has the layout; curriculum design is
still in `docs/lessons_plan.md`, engine detail in
`crates/harmonicon-song/CLAUDE.md`, and the pack format in
`docs/content_packs_plan.md`.

## First: is it actually scoreable?

The engine judges *pitch*, plus amplitude/timing patterns for a few
modifiers. It cannot tell puckering from tongue blocking. Classify the
lesson honestly:

- **Scored** — an existing primitive judges it directly.
- **Scored via proxy** — the technique is unverifiable but a musical
  outcome requiring it is not (tongue blocking → the octave splits it
  enables).
- **Instructional only** — text/diagram, passed with Mark-as-Done.

Don't build scoring machinery for the third category.

## Layout (in the pack)

```
<NN>_<unit>/<NN>_<lesson>/
  lesson.json
  locales/en-US.ftl, pt-BR.ftl, es-ES.ftl   # this lesson's text
  song/chart.harpchart                      # only for chart-backed lessons
locales/<lang>.ftl                          # lesson-unit-*, lesson-track-*
```

The `NN_` prefixes are the ordering mechanism — the scan sorts by
directory name.

## lesson.json

Validated against this repo's `assets/lesson_schema.dtd.json`, which is
`additionalProperties: false`, so a typo fails loudly.

```json
{
  "id": "stable-id",
  "unit": "scales",
  "track": "scales",
  "title_key": "lesson-stable-id-title",
  "body_key": "lesson-stable-id-body",
  "chart": "song/chart.harpchart",
  "prerequisites": ["earlier-lesson-id"],
  "pass_criteria": { "type": "scale-adherence", "threshold": 0.8 }
}
```

- **`id` is permanent** — it is the profile key and other lessons'
  prerequisite reference. Never rename a shipped one.
- `title_key`/`body_key` are Fluent **keys**, never display text. Define
  them in the lesson's own `locales/<lang>.ftl`, in every language the pack
  ships, `en-US` always — **not** in this repo's `ui.ftl`, which is the
  game's own UI. Prefix them with the lesson id. A new unit or track needs
  `lesson-unit-<unit>`/`lesson-track-<track>` in the pack's root `locales/`.
- A new `track` also wants a colour in `harmonicon-lessons`' (the crate)
  `lesson_tree::declared_track_color`, or it draws grey;
  `validate-pack` warns.
- Optional: `progression`, `scale`, `diagram`, `position_cycle` — all
  schema-enforced enums/booleans seeded into jam resources on Start.

## Pass criteria

| Type | Judged from | Where |
|---|---|---|
| `accuracy` | overall weighted accuracy | results screen |
| `technique` | one `SongStats` bucket (`bend`, `wah-wah`, `clean-attack`, …) | results screen |
| `scale-adherence` | `ImprovStats::adherence` | **open jam**, pause-menu "Finish Lesson" |
| `chord-tone-adherence` | `ImprovStats::chord_tone_adherence` | same |
| `phrase-discipline` | `ImprovStats::phrase_discipline` | same |

The last three route into `GameplayMode::JamSession` and never reach the
results screen — there is no score for an open jam. They need a chart
anyway (a single marker `TrackItem` satisfies the schema's `minItems: 1`
and gives the progress bar a length).

## A lesson needing something the engine doesn't have yet

A new widget type, pass criterion or manifest field is an **engine** change
here, then a content change there:

1. Add it in `harmonicon-song` (`lessons/manifest.rs` + the schema) and
   whatever renders or judges it.
2. **Bump `LESSON_FORMAT_VERSION`** (`lessons/manifest.rs`) in the same
   commit — an older game cannot parse the new variant.
3. In the pack, raise `pack.json`'s `requires.lesson_format` to the new
   value once a lesson uses it, so older games refuse the pack with
   "update Harmonicon" instead of failing lesson by lesson.

## Prerequisites

`lessons::is_unlocked` gates on them, and `validate-pack` checks every
referenced id exists, the graph has no cycle, and no prerequisite points at
a later unit (unit gating could never satisfy it). Under `--features dev`
everything shows unlocked so you can jump straight to a lesson while
iterating — that bypass is in the menu, not in `is_unlocked`, which stays
fully tested.

## Verify

```bash
cargo run --bin validate-pack -- ../harmonicon-lessons
```

It validates the schema, every chart (as the loader would), prerequisite
integrity, the curriculum graph, tab notation in phrase loopers, Fluent
syntax, and that every key a lesson uses is defined in `en-US` and in every
other language the pack ships. The pack's CI runs the same command.

To play the lesson, point the game at the checkout instead of the
download — in `settings.json`,
`"content_sources": {"lessons": [{"path": "../harmonicon-lessons"}]}` — a
local folder is read in place. `cargo test` here also round-trips every
chart in a sibling checkout through the Song Editor.
