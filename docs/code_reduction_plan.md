# Code reduction plan

How to shrink the workspace's Rust without losing tests or behaviour. The
totals below were measured after the `use_small_heuristics = "Max"`
reformat; the per-site counts further down were measured before it, so
their line spans are a little shorter now. Both come from these scripts:

```bash
python3 scripts/loc_report.py --files 40       # lines by crate and kind
python3 scripts/find_duplicates.py --window 6  # repeated line runs
python3 scripts/find_duplicates.py --tests     # same, including tests
```

Re-run them after each phase; the phase is done when its target moves.

## Where the lines are

| Kind | Lines | Share |
|---|---:|---:|
| Code | 47,935 | 46% |
| Tests | 28,062 | 27% |
| Comments | 18,570 | 18% |
| Blank | 8,612 | 8% |
| **Total** | **103,179** | |

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

## Phase 3 — Real clones (~−1.5k code)

These are ordered by size. Each one should become a single helper that every
caller uses.

- **`harmonicon-ui/src/dialogs/button.rs`**: `small`, `sized`, `icon` and
  `default` each repeat the same 13-line interaction prelude (`WidgetButton`,
  `TabIndex`, colours, six `on(mouse_*)` observers). Extract it as one
  `interactive_button(node, label_scene, on_click)` scene; the four become
  thin `Node` presets.
- **`song_editor/panel_widgets.rs`**: `spawn_button_shell`,
  `timeline_tool_button` and `mod_button` duplicate the same `bsn!` shell. The
  shell's doc comment says the other two don't fit it. Fix that by having the
  shell return `EntityCommands` *without* children, plus one
  `with_label(text)` helper, so the extras can be added afterwards.
- **2D/3D gameplay twins**: these pairs share the same note-state logic, and
  only the last step (writing to `Node` versus `Transform`/`MeshMaterial3d`)
  differs:
  - `gameplay_2d::update_note_visuals` / `gameplay_3d::notes::update_note_visuals_3d`
  - `animate_judged_notes` / `animate_judged_notes_3d`
  - the hole-colour loop in `gameplay_2d.rs:904` / `gameplay_3d/scene.rs:185`
  - the `.notes` filter in `gameplay_2d.rs:287` / `gameplay_3d/mod.rs:244`

  Move the decision ("which colour/hold/width/label does this note have
  now?") into a pure function in `gameplay::notes`, unit-test it once, and
  leave each mode's system as a short apply loop. The
  `note_ribbon_2d`/`note_ribbon_3d` uniform structs can share one
  `#[derive(ShaderType)]` body.
- **`song_editor::expected_notes`** (dev-only, 740 lines): reimplements grid
  rendering and the Blow/Draw modifier logic from `grid.rs` and
  `interaction.rs` (~200 duplicated lines). Parameterise the shared parts over
  the note source instead of keeping a parallel copy. Because it is behind
  `#[cfg(feature = "dev")]`, check it with `--features dev`.
- **Smaller pairs, 8–20 lines each**:
  - `menu/scene.rs:201/228`
  - `credits.rs:399` and `options/harmonica.rs:181` (the same
    children-walk)
  - `text_input.rs:184/231`
  - `combobox.rs:536/551` and `tab_bar.rs:134/149` (two copies of the same
    keyboard step)
  - `pause_menu.rs:116/430`
  - `drill.rs:181/636` (`pick_next_target`)
  - `judge.rs:540`/`hud.rs:245` (combo multiplier)
  - `playback.rs:94/116` (dir→action)
  - `note_glyphs.rs` (4× the same `TextFont` block)
  - `score_song.rs`/`loader.rs` (loader preamble)
  - `assets_management/mod.rs:443` ↔ `harmonicon-platform/build.rs:150`
    (manifest walk — must stay mirrored, so share via `include!` of one
    file)

## Phase 4 — Convert imperative UI to `bsn!` (~−2.5k)

These are the files with the most imperative spawning, ranked by
`spawn(`/`spawn_empty(`/`with_children` count:

| File | Count |
|---|---:|
| `bending_trainer/layout.rs` | 51 |
| `jam/session.rs` | 50 |
| `menu/pages/artist_list.rs` | 42 |
| `song_editor/meta_form.rs` | 37 |
| `pause_menu.rs` | 36 |
| `song_editor/panel.rs` | 36 |
| `song_editor/ui.rs` | 34 |
| `calibration.rs` | 33 |
| `content_sources.rs` | 24 |
| `results.rs` | 24 |

Convert one screen per commit, and take a BRP screenshot before and after
each one (`scripts/brpctl.py`) so a layout regression shows up as a picture
rather than in prose. Do `artist_list.rs` first: it is the largest file in the
tree, and splitting `setup_artist_list` into scene functions also takes it
well under budget.

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
| 3 Clones | 2–3 days | low–medium (2D/3D) | −1,500 |
| 4 `bsn!` conversion | ~1 week, incremental | medium (visual) | −2,500 |
| 5 Comments | ~1 day | none | −2,000 |
| 6 Test builders/tables | 2–3 days | low | −3,000 |
| **Total** | | | **≈ −9,000 (−9%)** |

Phase 5 shrinks the files without changing the code. Phases 3, 4 and 6
change its structure, and each lands as its own commit with
`cargo test --features dev` and `cargo clippy --all-targets -- -D warnings`
green.

Two other findings:

- `cargo machete` flags `thiserror` and `winit` as possibly unused. Confirm
  this before removing either.
- There are 14 `allow(dead_code|unused)` attributes. Each one should either
  justify itself or be removed.
