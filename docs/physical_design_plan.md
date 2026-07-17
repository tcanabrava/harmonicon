# Physical design: restructuring plan

Goal: make the codebase fast to *navigate and analyze* — for humans and for
AI assistants, whose cost per request is dominated by how much unrelated
code they must read to find the relevant ten lines. The two rules driving
everything here:

1. **Unrelated things do not share a file.** A file is one concern; its
   name says what the concern is; a grep hit lands you in a file where
   *everything* is relevant.
2. **Folders match modules, and dependencies point downward.** A module's
   physical location reflects its level: low-level shared vocabulary at the
   bottom, features in the middle, app wiring at the top. Nothing imports
   *upward*.

No behavior changes anywhere in this plan. Every step is a mechanical move
verified by `cargo test` + `cargo clippy`, done as `git mv` / cut-paste
with temporary `pub use` re-export shims so each commit stays small and
reviewable. Shims are removed in the same phase once callers are updated.

## Diagnosis (measured 2026-07)

~41k lines across `src/`. The specific problems, worst first:

### A. God files mixing unrelated concerns

| File | Lines | Distinct concerns co-located |
|---|---|---|
| `gameplay/mod.rs` | 2,921 | Plugin wiring + system sets; ~30 resource/component/message types; the `ScheduledNote`/`SongNotes` score-state model; bar-position math; pure chart-time helpers; `tick_clock` + sink anchoring + loop boundary; the 250-line `score_notes` system; score HUD; song-end detection; cleanup; **plus 1,250 lines of inline tests** (43% of the file) |
| `song_editor/mod.rs` | 1,571 | 180 lines of module header/plugin — and 1,390 lines of tests (88% of the file) |
| `menu/mod.rs` | 1,137 | `AppState` (the app-wide state machine), `GameplayMode`, `SelectedSong`, four `ReturnTo*` routing flags; shared scene helpers; setup systems for seven different menu pages; the loading gate; `route_menu_entry` |
| `gameplay/gameplay_3d.rs` | 1,427 | 3D scene/camera/prop setup + note spawning + tail rendering + tests |
| `gameplay/bending_trainer.rs` | 1,415 | Drill state machine + pitch judging + full UI + tests |
| `main.rs` | 178 | Small, but `process_audio` — a load-bearing stage of the audio pipeline — lives in the *binary*, invisible to anyone reading `audio_system/` |

Everything else ≤ ~1,100 lines with the same pattern repeating at smaller
scale (`jam_session.rs`, `options.rs`, `calibration.rs`, `pitch_detect.rs`).

### B. Layering inversions (things imported from the wrong place)

- **`AppState` lives in `menu`** — so `gameplay` (7 files), `song_editor`,
  `spectrogram`, and `profile` all `use crate::menu::...`. The menu is a
  *feature*; the app state machine is *vocabulary* every feature shares.
  Today, "what depends on the menu?" has a misleading answer, and any
  analysis of menu code pulls in readers who only wanted the state enum.
  Measured: 10 of the 11 outside-`menu` imports are for
  `AppState`/`GameplayMode`/`SelectedSong`/`ReturnToSongList` — none of
  them menu concerns.
- **`gameplay::call_response` imports `song_editor::playback`** for the
  note synth (`PhraseNote`/`render_pcm`). Two peer features are welded
  sideways; the synth is shared audio infrastructure that has no business
  living inside an editor tool.

### C. Names/locations that don't match ownership

- `jam_backing.rs` sits at top level while its consumers and siblings
  (`jam_session`, `jam_generate`) live under `gameplay/` and `menu/` —
  three homes for one feature.
- `lessons.rs` (domain: manifest, loader, unlock rules, pass criteria) is
  one 600-line file; its UI is `menu/lessons.rs`. Fine as a *split*, but
  the domain file bundles schema types + filesystem scan + progress logic.

## Target layout

```
src/
  app.rs               AppState, GameplayMode, SelectedSong, ReturnTo* flags
                       (level 1: pure vocabulary, no systems, imports nothing local)
  audio_system/
    pipeline.rs        process_audio + log_pitches (moved out of main.rs)
    synth.rs           PhraseNote, render_pcm (moved out of song_editor)
    …                  audio_input, pitch_detect, midi, wav, waveform as today
  scoring.rs           unchanged (already correctly placed pure logic)
  song/                unchanged
  lessons/
    mod.rs             plugin + re-exports only
    manifest.rs        LessonManifest, PassCriteria, schema validation
    catalog.rs         startup scan, unit/lesson discovery
    progress.rs        is_unlocked, LessonContext, pass judging
  jam/
    mod.rs             plugin + re-exports only
    backing.rs         (from jam_backing.rs) generated manifest + bass synth
    session.rs         (from gameplay/jam_session.rs) hole map, live systems
    improv.rs          ImprovStats, classify_note_fit, in_rest_window
  gameplay/
    mod.rs             mods + re-exports ONLY (~50 lines)
    plugin.rs          GameplayPlugin, GameplayLogic/OverlaySet, schedule wiring
    state.rs           Score, SongStats, TechniqueStats, PitchGate,
                       ActivePitches, ValidHarpNotes, NoteScored, Paused, …
    notes.rs           ScheduledNote, SongNotes, target_pitch,
                       resolve_item_time, last_note_end, LOOKAHEAD
    bars.rs            parse_beats, secs_per_bar, bar indices,
                       CurrentBar/AbsoluteBar/BarChanged, track_current_bar
    clock.rs           existing GameplayClock + tick_clock,
                       should_anchor_to_sink, handle_loop_boundary (the
                       anchoring invariant finally lives in ONE file)
    judge.rs           score_notes + technique_confirmed, style_bonus_points,
                       modifier_fx_key — the scoring *system* (pure fns stay
                       in src/scoring.rs)
    hud.rs             ScoreText/ComboText/FeedbackText, update_score_display
    lifecycle.rs       reset_score, setup_scoring_config, detect_song_end,
                       apply_music_volume, cleanup_gameplay
    …                  existing per-feature files (overlays, 2d/3d, pause,
                       results, bending_trainer, call_response) as today
  menu/
    mod.rs             plugin + re-exports only
    routing.rs         route_menu_entry, check_loading, handle_menu_escape
    scene.rs           menu_bg, menu_root_scene, heading_scene
    pages/             one file per page: main.rs, play.rs, mode_select.rs,
                       jam_session.rs, artist_list.rs, song_list.rs,
                       help_about.rs (jam_generate, options, lessons,
                       calibration, theme_picker, tutorial, credits move in)
  song_editor/         internal split as today; mod.rs sheds its test blob
```

Level order (imports may only point left→right, never back):
`app.rs` / `scoring.rs` / `song/` / `audio_system/` → `settings` / `profile`
/ `theme` / `localization` → `lessons/` / `jam/` / `gameplay/` /
`song_editor/` / `spectrogram/` → `menu/` → `main.rs`.

## Rules to adopt (the part that keeps it fixed)

1. **File budget: ~500 lines of non-test code.** Not a hard wall — a
   cohesive 600-line file beats two incoherent 300s — but crossing it in a
   PR needs a sentence of justification.
2. **Tests move to sibling files once they dominate.** Any `#[cfg(test)]
   mod tests` over ~150 lines becomes `#[cfg(test)] mod tests;` resolving
   to `<module>/tests.rs` (or `tests_<topic>.rs` split by subject). Same
   crate, same visibility, zero test-code changes — pure relocation. This
   alone removes ~4,000 lines from the five worst files.
3. **`mod.rs` contains wiring only**: `mod` declarations, the plugin, and
   `pub use` re-exports. Logic in a `mod.rs` is the seed of every god file
   in the table above.
4. **A file states its contents in a `//!` header** (most already do —
   make it universal). First line = what's in the file; that's what an
   assistant's grep-then-skim lands on.
5. **New concern → new file; second concern in a file → split it then.**
   Boy-scout rule: the person touching a file that violates the budget
   splits it *first*, in its own commit, before the feature change.
6. **Enforce mechanically, in the repo's existing style** (build.rs lint,
   `locales_define_the_same_keys`): add a `tests/physical_design.rs` that
   walks `src/`, fails on any file whose non-test line count exceeds the
   budget, with an explicit, shrinking allowlist for the current offenders.
   New violations can't land silently; the allowlist is the burndown chart.

## Phases

Each phase is independently shippable, ordered by (leverage ÷ risk).

### Phase 1 — cut the layering inversions (small, highest leverage)

1. Create `src/app.rs`: move `AppState`, `GameplayMode`, `JamProgression`,
   `SelectedSong`, `SelectedArtist`, and the `ReturnTo*` flags out of
   `menu/mod.rs`. Leave `pub use crate::app::*;` re-exports in `menu` for
   one commit, migrate the ~11 importing files, drop the shims.
2. Move `PhraseNote`/`render_pcm` from `song_editor/playback.rs` to
   `audio_system/synth.rs` (its consumer `encode_wav` is already in
   `audio_system/wav.rs`). `song_editor::playback` keeps its editor-side
   Play-button systems and imports the synth like `call_response` does.
   Kills the only `gameplay → song_editor` edge; drops
   `playback`'s awkward `pub(crate)` widening.

Exit: `grep -rl "use crate::menu::" src | grep -v src/menu` returns only
tutorial's `tour_active` gates (a genuine menu export), and
`grep -rl "use crate::song_editor" src` returns only `menu` (launching the
editor is legitimately a menu concern).

### Phase 2 — evict the test blobs (mechanical, zero risk)

Relocate inline test modules per rule 2: `gameplay/mod.rs` (~1,250 lines),
`song_editor/mod.rs` (~1,390), `bending_trainer.rs` (~350),
`gameplay_3d.rs`, `jam_session.rs`, `harmonica.rs`, `scoring.rs`,
`pitch_detect.rs`. Test *content* untouched; `cargo test` count must not
change (~590). This halves several of the worst files before any real
surgery and makes Phase 3 diffs readable.

### Phase 3 — split `gameplay/mod.rs`

Into the eight files in the target layout (`plugin` / `state` / `notes` /
`bars` / `clock` / `judge` / `hud` / `lifecycle`), one commit per
extraction, `mod.rs` ending as re-exports so **no other file's imports
change** (`crate::gameplay::ScheduledNote` keeps working). The payoff for
analysis latency is the biggest in the plan: today *every* scoring, clock,
or HUD question requires opening a 3k-line file; after, each question maps
to one ≤400-line file whose name answers it. Update `CLAUDE.md`'s clock and
scoring bullets to the new paths in the same PR.

### Phase 4 — split `menu/mod.rs`, relocate `process_audio`

- `menu/mod.rs` → `routing.rs`, `scene.rs`, `pages/*` (one page per file;
  existing per-page files move under `pages/`). `mod.rs` becomes plugin +
  re-exports.
- `process_audio` + `log_pitches` → `audio_system/pipeline.rs`; `main.rs`
  becomes pure composition (~100 lines), and the audio input path is
  finally fully contained in the folder named after it.

### Phase 5 — gather the jam and lessons features

- `src/jam/`: `jam_backing.rs` → `jam/backing.rs`;
  `gameplay/jam_session.rs` → `jam/session.rs` with `ImprovStats` /
  `classify_note_fit` / `in_rest_window` split into `jam/improv.rs`.
  (`menu/pages/jam_generate.rs` stays a menu page — pages live with pages.)
  Do this *before* starting 0.4's remaining jam work (freeform
  call-and-response, cross-harp) so the new code lands in one place
  instead of three.
- `src/lessons/`: split `lessons.rs` into `manifest.rs` / `catalog.rs` /
  `progress.rs`.

### Phase 6 — the remaining big feature files, opportunistically

`bending_trainer.rs` (drill logic vs UI), `gameplay_3d.rs` /
`gameplay_2d.rs` (scene setup vs note spawn/despawn vs tails),
`options.rs` (one section per file), `calibration.rs` (measurement logic
vs UI). No dedicated push: rule 5 handles these as they're next touched.
Listed so the allowlist in the physical-design test can name them with a
destination.

### Explicitly deferred: workspace/crate split

Lakos-style physical design would eventually pull the pure logic
(`song/`, `scoring.rs`, `audio_system`'s DSP) into a `harmonicon-core`
crate with no Bevy dependency — better incremental compiles, enforced
layering. Deferred because it changes every import path at once and its
payoff is compile time, not analysis time; the in-crate levelization above
gets ~all the navigability benefit. Revisit if `cargo check` latency
becomes the complaint.

## Verification, per commit

- `cargo test` — identical pass count (moves can't change behavior).
- `cargo clippy` — clean.
- `git diff --stat` of a move commit should be ~pure relocation (adds ≈
  deletes); logic edits never share a commit with moves.
- After Phases 3–5: update `CLAUDE.md`'s file references (clock, scoring,
  jam, lessons bullets) — it is the assistant's map, and a stale map is
  worse than no map.
