# TODO

Open, actionable items only — once something lands, delete it from here
rather than annotating it done (git log and commit messages are the
historical record; see `CLAUDE.md`).

## Correctness / consistency

- [ ] **`profile.json`'s `phrase_learned: Vec<f32>` is indexed by a
  section's ordinal position in the track** (`gameplay/adaptive_difficulty.
  rs`). If a chart's phrase tags are ever reordered/added/removed by a
  re-edit, old progress silently applies to the wrong section instead of
  resetting. Low urgency (no in-place chart edits happen today outside the
  song editor, and the editor doesn't touch shipped charts), but worth a
  content-versioned key (e.g. phrase name, or a stable id) if that changes.

## Content

- [ ] **Only one bundled example artist** (`assets/songs/Example Artist`,
  three example songs used for 2D/3D/fallback testing). Ship a starter
  pack of public-domain blues heads/riffs across difficulties before wider
  release. **Deliberately not attempted unsupervised**: authoring
  rights-clear, well-judged chart content needs real musical judgment.
- [ ] **Lessons content, Unit 4 "jazz" (0.6).** Wave 2 (harmonica-basics
  extensions, bar-counting drills, the train trio, and the new Unit 3
  blues-vocabulary unit — licks via call-and-response, chord-tone/
  minor-blues/phrase-discipline improvisation) is fully shipped; see
  `docs/lessons_plan.md`. What's left is the jazz unit, gated on 0.6's
  jazz chord-tone tables and a ii–V–I/jazz-blues `Progression` variant.
  Original arpeggio/vocabulary drills are the safe-to-author subset;
  actual jazz-standard repertoire needs the same rights judgment as the
  item above.
