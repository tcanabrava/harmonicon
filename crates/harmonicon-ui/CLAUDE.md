# harmonicon-ui

Reusable presentation with no gameplay knowledge: the widget library,
the SMuFL notation staff and the audio visualiser.

A widget here must work for *any* caller. Anything that knows what a
note, a score or a lesson is belongs in the feature that owns it.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **A shared music-notation staff renders with the Bravura SMuFL font**
  (`harmonicon-ui`'s `music_score/`, a new top-level module, sibling to `spectrogram` —
  used by Play 2D, below the song-progress bar, and by the Song Editor,
  in its own fixed chrome below the grid). Deliberately coarse, not a
  sight-reading tool: noteheads (whole/half/filled by duration), stems,
  ledger lines, sharp accidentals, ties across a bar line, bar lines and a
  time signature, beams joining short notes inside a beat, and durations
  spelled with the dots and flags they actually take, down to a sixteenth
  — but no slanted beams (they are horizontal, every stem in a group drawn
  to one shared line), no partial beams in a mixed group, nothing shorter
  than a sixteenth, and no double dots.
  - **The clef is chosen from the music, not fixed** (`choose_clef`):
    treble, treble-8va or bass, by *median* pitch so one stray note can't
    drag the staff. 8va is the common case here — measured across the
    bundled charts the median is F5, the treble staff's own top line, so
    reading everything in plain treble put half the music in ledger lines
    (eleven of them for the highest note). Clef and meter are both picked
    once per song, never mid-scroll.
  - **`Clef::base_midi` is the single place a staff's vertical origin
    lives.** `staff_step` subtracts it; `y_for_step`/`ledger_line_steps`
    then work in clef-relative steps and need no clef of their own.
  - **The pure notation maths lives in `music_score/notation.rs`**, Bevy-free
    and unit-tested (clef choice, beam grouping, bar-line positions, time
    signature parsing); `mod.rs` is the translation of those answers into
    UI nodes and is not itself test-covered. `assets/fonts/Bravura.otf`
  (SIL OFL, `Bravura-OFL.txt` alongside it) is bundled and loaded the same
  `Font::from_bytes` way as `dialogs::font_fallback`'s small icon fonts;
  every glyph codepoint and every relative measurement (notehead width,
  stem attachment point, ledger-line extension) comes straight from
  Bravura's own published `bravura_metadata.json`. **Glyph and rectangle
  must agree on where a staff position is.** Bevy places a `Text` node by
  its box's top-left, not the glyph's SMuFL origin, so every Bravura glyph
  is offset by `GLYPH_BASELINE_CORRECTION`: half its pinned line box
  (`GLYPH_LINE_HEIGHT_PX`), which is exactly where parley puts the baseline
  because Bravura's ascent equals its descent. The line box is a fixed 40
  px, not relative to the font size, so that half of it is a whole number
  of physical pixels at every common display scale — parley rounds there,
  and a relative line height let the baseline drift by up to 0.6 px with
  the scale. Stems, beams and ledger
  lines are `Node` rectangles placed from the same `y_for_step`, so a
  wrong offset shows as heads off their lines *and* stems stopping short
  of their heads — a half-em guess did both. Every Bravura `Text` must
  carry `LineHeight::Px(GLYPH_LINE_HEIGHT_PX)`, or it inherits
  Bevy's default and the derivation no longer holds;
  `tests/glyph_coverage.rs::bravura_ascent_equals_its_descent` guards the
  font side.
  - **`NotationNote { start_beat, duration_beats, midi, .. }`** (beats, not
    ticks or seconds) and `MusicScoreBarMap` are the module's inputs. It never touches a
    chart's tempo map or an editor's own tick resolution, so each of the
    three call sites converts its own time representation first: gameplay
    (`gameplay::music_score_bridge`) goes `ScheduledNote::time` (seconds)
    through `song::chart::seconds_to_tick`; the Song Editor
    (`song_editor::music_score_bridge`) just divides its own
    already-tempo-independent `GridNote::tick` by `TICKS_PER_BEAT`.
    `highlighted` draws a note's own marks (head, stem, flag, dot,
    accidental) in gold; the editor sets it for its selection, gameplay
    never does. A beam stays white, since it belongs to its whole group. This
    is the same split `gameplay::metronome_overlay`/`song_editor::
    metronome` already use for the same reason (two genuinely different
    clocks/note models) — not duplicated logic, since the actual
    rendering stays 100% inside `music_score` either way.
  - **A note that crosses one or more bar lines is split into per-bar
    segments and tied together** (`MusicScoreBarMap::split_note`, used by both
    bridges) rather than drawn as one
    oversized notehead — `NotationNote::tied_from_previous` marks every
    segment after the first, which suppresses that segment's own
    accidental (a tie doesn't restate one) and draws a tie mark back to
    the segment before it. The tie itself is a *real curved arc*, not a
    flat rectangle: `music_score::tie_material::TieMaterial` is a
    `UiMaterial` fragment shader (`assets/shaders/music_score_tie.wesl`),
    the same "custom shader for a shape a plain `Node` can't express"
    pattern `gameplay::note_ribbon_2d::NoteRibbon2dMaterial` already
    established — one shared material handle covers every tie, since
    unlike the note tail this shape never varies (two instances: one
    bowing down, one up). It runs from just past the first head's right
    edge to just before the second head's left edge — positions taken from
    the immediately-preceding entry in `MusicScoreNotes`, which
    `MusicScoreBarMap::split_note` guarantees is the tied-from segment — and sits on
    the side away from the stems: under the heads for stems up, over them
    for stems down.
  - **Rests come from gaps in the union of sounding notes.** The shared
    renderer leaves the tail of a bar blank when a silent gap crosses its
    boundary, then spells whole through sixteenth rests from the next
    bar's start. Chords and tied continuations do not create false rests;
    the renderer does not infer silence after the final note. Staff spacing
    widens to fit the shortest rest and leave air before the next notehead;
    the capped scale still omits a rest too small to draw without overlap.
  - **The visible window sizes itself from the panel's own on-screen
    width**, independent of any other host UI (the falling-note highway's
    own lookahead, the Song Editor grid's own column count): `visible_
    beats(panel_width_px)` derives how many beats fit on each side of the
    "now" reference line from `ComputedNode` (read via a `MusicScorePanel`
    marker on the panel's root), converted from physical to logical px the
    same way `gameplay_2d::size_note_ribbons` already has to. Rebuilds also
    fire on `Changed<ComputedNode>` (first layout pass, a window resize),
    not just on note/playhead changes. The Song Editor's own
    `MusicScorePlayhead` — which only gameplay's clock naturally drives —
    falls back to `EditorState::scroll_beat` whenever nothing is actually
    playing, so panning the grid pans the score with it instead of the
    score staying pinned whichever beat playback last stopped at (usually
    0).
  - **`dialogs::font_fallback::apply_font_fallback` runs on *every* `Text`
    entity in the game**, not just the ones its own small icon fonts
    cover — for a single-run string whose character isn't one of its own
    known gaps, it unconditionally resets `TextFont.font` back to
    `FontSource::default()`, since that system's whole contract assumes
    it's the only thing ever touching `TextFont.font`. `music_score`'s
    Bravura glyphs (Private-Use-Area SMuFL codepoints, never in that
    system's own gap lists) got silently clobbered back to the default
    font — the actual cause of an early "tofu box" bug, not a font or
    Bevy/Parley limitation. Fixed with a general opt-out marker,
    `dialogs::font_fallback::SkipFontFallback`, rather than special-casing
    SMuFL codepoints into that system.
  - **This surfaced a real, unrelated harp-model bug while testing**: the
    Song Editor's own `state::overblow_ok` allowed Overblow on holes 1–6,
    but `song::harmonica::hole_notes` only defines a real overblow reed
    for 1/4/5/6 — holes 2/3 fell through the gap (the editor accepted the
    click, but no pitch existed anywhere downstream: scoring, playback,
    or this staff). Fixed `overblow_ok` to match `hole_notes` exactly.
