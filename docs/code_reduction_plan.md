# Code reduction plan

How to shrink the workspace's Rust without losing tests or behaviour. The
totals below were measured after the `use_small_heuristics = "Max"`
reformat; the per-site counts further down were measured before it, so
their line spans are a little shorter now. Screens that hold still are
compared before and after with `scripts/capture_static_screens.py`. Both come from these scripts:

```bash
python3 scripts/loc_report.py --files 40       # lines by crate and kind
python3 scripts/find_duplicates.py --window 6  # repeated line runs
python3 scripts/find_duplicates.py --tests     # same, including tests
```

Re-run them after each phase; the phase is done when its target moves.

## Where the lines are

| Kind | Lines | Share |
|---|---:|---:|
| Code | 47,630 | 46% |
| Tests | 28,084 | 27% |
| Comments | 18,594 | 18% |
| Blank | 8,624 | 8% |
| **Total** | **102,932** | |

Gameplay and the Song Editor together hold over 40% of the total.

**There isn't much copy-paste.** Only about 2.1k normalised non-test lines (4%
of the code) sit in a repeated 6-line run, or 3.6k counting tests. Removing
clones, then, saves a few thousand lines at most. Most of the size comes
from two sources:

1. **Imperative UI.** There are 736 `spawn(`/`spawn_empty(` calls against
   265 `bsn!` blocks, which breaks the "UI is authored with `bsn!`" convention.
   Imperative spawning takes 2–3× the lines of the same tree in `bsn!`.
   `artist_list.rs` alone has 1,141 code lines, 0 `bsn!` blocks and a
   330-line `setup_artist_list`.
2. **Comment volume.** At 18.5k lines, comments make up 22% of non-blank
   lines: 101 blocks run 15 lines or more (2,177 lines in total), and 100
   comment lines narrate history ("used to", "previously", "before this").
   That breaks the rule that comments explain current behaviour only.

`rustfmt.toml` deliberately leaves out `fn_params_layout = "Compressed"`:
it would save another ~1.9k lines, but it packs several system parameters
onto one line, which makes Bevy signatures harder to scan. A future
formatting-only commit goes in `.git-blame-ignore-revs`.

Resource groups repeated verbatim across signatures are already bundled
(`settings::PersistedSettings`, `song_editor::transport::Transport`). A
`Localization` + `LoadedTheme` bundle was considered and dropped: only 22
systems take both, so it would save ~22 lines while renaming every use in
their bodies. Bundle a group when it recurs as a *list*, not as a pair.

Real clones are merged: lines inside a repeated 6-line run fell from 2,085
to 1,229, but the total only by ~250, since a shared helper costs lines of
its own. What `find_duplicates.py` still reports is mostly *shape*, not
logic: system signatures listing the same resources, hover/out observer
pairs differing only in the colour they write, and asset-loader trait
boilerplate. Merging those would need generics that cost more clarity than
lines; leave them unless one starts carrying real logic.

## `bsn!` conversion is not a size lever

Converting the song picker (`artist_list.rs`) to `bsn!` saved 12 lines
(373 added, 385 removed). Since the `use_small_heuristics = "Max"`
reformat, imperative struct literals mostly fit on one line, while rustfmt
does not format inside `bsn!`, which needs `{...}` around every
expression and puts one field per line. Convert a screen when the
convention calls for it — a new screen, or one being reworked — not to
cut lines. `dialogs::button::tinted` and `scripts/capture_static_screens.py`
exist for that.

## Phase 5 — Comments (~−2k)

- Remove history narration: the 100 lines matching `used to|previously|no
  longer|before this|originally|an earlier version|the old`. Where a line
  carries a real constraint, restate it as a present-tense rule.
- Shorten `//!` headers over ~15 lines: 101 blocks, led by `build.rs` (73),
  `lesson_tree/mod.rs` (41) and `song_progress_overlay.rs` (40). Move design
  rationale into the `contributing/` chapter for that subsystem, and keep only
  what a reader needs at the call site.
- The per-crate `CLAUDE.md` files aren't Rust, but they suffer from the same
  problem: `harmonicon-editor/CLAUDE.md` is 906 lines, mostly feature history.
  Trim them alongside the Rust comments; the root `CLAUDE.md` also repeats its
  `dev` feature and `harmonicon-core` test paragraphs twice.
- Comment-only commits need no build (`feedback_no_verify_comment_only_edits`).

## Phase 6 — Tests (~−3k, no coverage lost)

Coverage stays exactly the same. What shrinks is how each case is written:

- `song_editor/tests.rs` (4,678 lines) builds `EditorState { … ..default() }`
  174 times and `GridNote { … }` 32 times. Add `state_with(notes)` and
  `note(hole, tick, len)` builders, as `grid_note` already does for one shape.
- `gameplay/tests.rs` repeats the same 17-line `World` + schedule setup
  (`:1152`, `:1239`, …). Move it into one `judge_world(notes)` fixture.
- Convert runs of near-identical `#[test]` functions that differ only in
  inputs into table-driven tests: `serialize_lesson_*`, the `paste_targets_*`
  family and the lesson-tree layout tests.
- Don't merge tests whose names document distinct behaviour just to save
  lines; a table row needs a label that says the same thing.

## Order and expected result

| Phase | Effort | Risk | Est. lines |
|---|---|---|---:|
| 5 Comments | ~1 day | none | −2,000 |
| 6 Test builders/tables | 2–3 days | low | −3,000 |
| **Total** | | | **≈ −5,000 (−5%)** |

Phase 5 shrinks the files without changing the code. Phase 6
changes its structure, and lands as its own commit with
`cargo test --features dev` and `cargo clippy --all-targets -- -D warnings`
green.

Two other findings:

- `cargo machete` flags `thiserror` and `winit` as possibly unused. Confirm
  this before removing either.
- There are 14 `allow(dead_code|unused)` attributes. Each one should either
  justify itself or be removed.
