# Code reduction review

Reviewed 2026-10-03. This is a focused duplication review, not an exhaustive review of every file.

## Size baseline

Before changes, `src/**/*.rs` and `crates/**/*.rs` contained 99,161 physical lines.
After both passes they contain 99,089 lines. Of the latter, 13,596 are
in separate test modules, 17,964 are comment-only lines, and 8,351 are blank
lines. These categories overlap; inline test modules are not included in the
separate-test count. Physical line count is therefore not the same as production
code size. Keep behavior tests and useful explanations when reducing code.

## Implemented

- `harmonicon-core/src/harmonica.rs`: share diatonic and chromatic layout
  construction across six public constructors. Public functions and tunings stay intact.
- In the same file, construct the standard twelve-bar progression once and
  apply the quick-change/jazz substitutions. Minor blues retains dominant V
  chords, and tonic roots retain the supplied spelling.
- Simplify slide lookup and compute frequency bounds from MIDI extrema,
  removing the temporary frequency vector and converting only two pitches.
- `harmonicon-core/src/pitch_map.rs`: build a 256-entry natural-reed lookup
  once for key-fit scoring. Work changes from O(notes × holes) reed parses to
  O(holes + notes), retaining repeated-note weighting and key tie ordering.
  A regression compares the previous search against the lookup for all u8
  pitches and both harp families in every key. No timing speedup is claimed.

- `harmonicon-editor/src/song_editor/meta_form.rs`: reuse the existing cycle
  button builder for all 12 cycle fields. One observer reads choices from a
  registry and writes through the field accessor, retaining `set_key` for
  its cache invalidation. Removed the redundant `is_cycle` registry.
- Share the file-dialog button builder for music browsing and MIDI import,
  keeping labels, tooltips, colors, extensions, and dialog purposes distinct.
  Single-line and multiline inputs also share their invocation.
- `interaction.rs` and `expected_notes.rs`: share sticky pitch/expression
  dispatch while retaining each mode's selected-note editing behavior.
- `harmonicon-core/src/harmonica_constraints.rs`: share pitch/breath traversal
  between single-pitch queries and precomputed tables. Batch filtering now
  builds its full-u8 table once, rather than deriving all holes for each
  candidate and each reachability query. Empty batches skip derivation.

Across both passes, production files shrink by 162 lines. Regression coverage
adds 90 lines, for 72 fewer Rust lines overall. The editor
regression activates actual controls to check key-cache invalidation, nested
field updates, and music/MIDI dialog purposes and extension filters.

## Concrete next candidates

| Location | Repeated work | Proposed simplification | Behavioral constraints |
| --- | --- | --- | --- |
| `harmonicon-gameplay/src/gameplay/harmonica_overlay.rs` | Ordinary and selectable diatonic diagrams repeat harp dispatch, note collection, and diagram setup. | A shared diatonic setup accepting the cell-spawning callback. | Keep chromatic fallback and different hint text; the generic diagram renderer already exists. |
| Editor panel tests and cross-crate chart fixtures | Repeated resource setup and standard harp JSON. | Extend existing test fixtures with small builders. | Keep per-test intent visible; avoid a large fixture with hidden state. |

The remaining candidates are smaller. No reliable multi-thousand-line saving
was established in these passes; a larger reduction needs a broader feature/UI
design review rather than shorter formatting or compressed expressions.

## Validation

- Core: 318 tests passed.
- Editor with `--release --features dev`: 411 tests passed, including the
  actual-control activation regression.
- Workspace Clippy with `--release --features dev --all-targets -D warnings`
  passed.
- Formatting and diff whitespace checks passed.

Builds used `RUSTC_WRAPPER=` to bypass the sandbox-incompatible sccache wrapper.
No interactive visual verification was performed; the refactor retains the
existing node styles and hierarchy, and checks events through Bevy's world.
