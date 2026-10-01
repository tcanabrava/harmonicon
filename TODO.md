# TODO

Open, actionable items only — once something lands, delete it from here
rather than annotating it done (git log and commit messages are the
historical record; see `CLAUDE.md`).

## 1.0 blockers (desktop)

See `ROADMAP.md`'s 1.0 section for the bar and `PLAN.md` for the order.

- [ ] **No blues content is bundled**, though blues/jazz is the project's
  stated theme: all 11 shipped charts are demos or public-domain classical/
  traditional. Same rights-and-judgment constraint as the content item
  below — **not to be authored unsupervised**.

- [ ] **Bevy is pinned to the `v0.20.0-rc.1` git tag**, because a release
  candidate is not published to crates.io. When 0.20.0 final lands: drop the
  `git`/`tag` keys from `Cargo.toml`'s `bevy` entry and the matching
  `allow-git` exemption from `deny.toml` (`unknown-git = "deny"` rejects the
  source otherwise). `bevy_fluent` may have a 0.20-compatible release by
  then too, at which point `harmonicon-platform`'s absorbed
  `localization::ftl` *could* go back to being a dependency — owning ~300
  lines is arguably the better trade, since that crate's version tracks
  Bevy's and gated this upgrade.

## Bring your own harp, bring your own songs (post-1.0)

`ROADMAP.md` has the reasoning, `PLAN.md` the order. Phase 0 blocks both.

- [ ] **The harp picker is only on the song-list route.** Lessons and the
  guided tour still force the chart's own harmonica — deliberate for now (a
  lesson prescribes its harp as part of the teaching), but a player who
  doesn't own that key can't take those lessons at all.
- [ ] **`.gp2` is still unread.** Everything else a player is likely to own
  now loads — MIDI, Guitar Pro 3 through 7, MuseScore, plain and compressed
  MusicXML. `.gp2` predates what the `guitarpro` crate reads and is not urgent.
- [ ] **Tab timing is reconstructed, and only round-trip-tested.** The
  `guitarpro` crate ships no fixtures, so `guitar_pro`'s measure/beat
  placement is verified against songs this codebase writes itself plus an
  in-memory model — not against a file Guitar Pro produced. Repeats and
  alternate endings are played out, but the pass count of a repeat played
  three or more times is a guess: the `guitarpro` crate's readers disagree
  on what `repeat_close` counts (`guitar_pro::repeats_from_marks`). Worth
  revisiting with a real tab in hand.

## Mobile (post-1.0)

- [ ] **Touch gestures are unverified on real hardware.** The two-finger pan
  and the toolbar's drag-scroll are unit-tested and the sidebar was checked
  on an emulator, but multi-touch can't be scripted there (`adb shell input`
  has no multi-touch; raw `sendevent` never reaches the app). See
  `contributing/src/android-build.md`.

## Content

- [ ] **Only one example artist** (`Example Artist` in the
  `harmonicon-songs` pack,
  three example songs used for 2D/3D/fallback testing). Ship a starter
  pack of public-domain blues heads/riffs across difficulties before wider
  release. **Deliberately not attempted unsupervised**: authoring
  rights-clear, well-judged chart content needs real musical judgment.
- [ ] **Lessons content, Unit 4 "jazz" (0.6).** Wave 2 (harmonica-basics
  extensions, bar-counting drills, the train trio, and the new Unit 3
  blues-vocabulary unit — licks via call-and-response, chord-tone/
  minor-blues/phrase-discipline improvisation) is fully shipped, and Unit
  4's own engine prerequisites (jazz chord-tone tables, the jazz-blues
  `Progression` variant) are now done too — see `docs/lessons_plan.md`.
  What's left is authoring the actual jazz unit content. Original
  arpeggio/vocabulary drills are the safe-to-author subset; actual
  jazz-standard repertoire needs the same rights judgment as the item
  above.

## Known open items (design detail)

Moved out of `CLAUDE.md`: these are status, not load-bearing
architecture, so they belong with the rest of the planning docs.

- Content: besides the Example Artist gameplay demos, bundled songs now
  include public-domain melodies (Greensleeves on a G harp, Jesu Joy and
  the Toccata in D minor on C harps, Für Elise on a C chromatic,
  "O Pulo da Gaita" transcribed from the Mr. Dirsom harmonica tab score,
  Amazing Grace, the Hallelujah chorus from Handel's Messiah on a D harp,
  and Mulher Rendeira), now in the `harmonicon-songs` pack, whose CI runs
  `validate-pack` over every chart. Deliberately skipped as still under copyright:
  Feira de Mangaio (Sivuca/Glorinha Gadelha) and Asa Branca (Luiz
  Gonzaga/Humberto Teixeira) — chart those yourself via Record mode
  instead of bundling a transcription.
- **Song editor color legend**: a third meta-form column
  (`meta_form::spawn_color_legend`) explains every color the editor uses,
  grouped by where it appears — note technique colors in the grid
  (`state::pitch_color`; direction is the ↑/↓ arrow glyph, not a color),
  the out-of-scale red tint, the selected-note border, drag-ghost valid/
  invalid, and the timeline/scrollbar colors — deliberately calling out
  that the scrollbar minimap's blue/orange means blow/draw
  (`interaction::SCROLLBAR_BLOW_COLOR`/`SCROLLBAR_DRAW_COLOR`), a
  different meaning than the grid note's blue (which means the Normal
  technique, regardless of blow/draw). Several colors that were private
  `const`s or local `let` bindings (`grid::OUT_OF_SCALE_TINT`/
  `TEMPO_MARKER_COLOR`, `timeline_overlay::SPLIT_LINE_COLOR`/
  `RANGE_HIGHLIGHT_COLOR`) were widened to `pub(super)` so the legend
  reuses the exact values instead of duplicating literals that could
  drift out of sync.
- **Song editor: selectable scale** (`song::chart::Scale`, a new chart
  field): the grid's out-of-scale red tint used to always mean "outside
  the blues scale rooted on the harp key" unconditionally
  (`blues_scale_classes(&state.key)`); it's now `state.scale.classes(&state.
  key)`, `state.scale` picked via a combobox (`meta_form::
  spawn_scale_combobox`) — six options: 1st/2nd/3rd position (the blues
  hexatonic, same shape as everywhere else, just rooted at the harp key
  \+0/+7/+2 semitones — the same offsets `Position::interval_below_jam_key`
  uses for Jam Session's harp-picking, just applied upward from the harp's
  own key instead of downward from a separate jam key, since a chart has
  no jam key distinct from its harp) and Major/Minor Pentatonic/Country
  (alternative *shapes*, always rooted on the harp key — for melodies that
  aren't blues-vocabulary at all; "Country" = major pentatonic, the
  scale 2nd-position cross-harp playing reaches without bending, per
  harmonica-pedagogy convention). `FirstPosition` (the default, used when
  a chart doesn't set `scale` at all) reproduces the old unconditional-
  blues behavior exactly — `first_position_matches_blues_scale_classes_
  exactly` pins this down. `harmonica.scale` is a new, schema-`enum`-
  validated field (unlike its free-string `position` sibling), added to
  both `Harmonica::Diatonic`/`::Chromatic`; `CURRENT_FORMAT_VERSION`
  bumped to 1.2.0 since an older build's stricter schema would otherwise
  reject a chart that actually sets it with a confusing raw validation
  error instead of the intended "needs a newer Harmonicon" message — a
  chart that never sets `scale` needs no version bump, unaffected either
  way. The combobox itself is spawned once into a reserved
  `ScaleComboboxSlot` (`spawn_scale_combobox`, a `Without<Children>`
  spawn-once gate, unlike the MIDI track combobox's rebuild-on-message
  pattern, since `Scale::all()`'s option list never changes at runtime);
  Load pushes a different value into the already-spawned combobox by
  writing `ComboboxValue` directly (`sync_scale_combobox_value`) — the
  widget's own documented escape hatch for exactly this, `dialogs::
  combobox`'s always-on `sync_combobox_visuals` picks the change up from
  there. No existing bundled chart sets `harmonica.scale` — all keep
  reading as 1st position, i.e. unchanged from before this feature.
  **`ScaleComboboxSlot` lives in the fixed chrome** (`ui::
  spawn_fixed_chrome`, above the mod panel — not the scrollable meta form
  the rest of the fields are in), a deliberate, load-bearing placement:
  `bevy_ui_widgets::Popover`'s dropdown list must be a literal ECS child of
  its toggle to compute its own position, and Bevy's UI overflow clipping
  follows that same ancestry rather than the popover's computed screen
  position — a combobox nested inside the form's `Overflow::scroll_y()`
  `ScrollArea` gets its open dropdown clipped to that scroll viewport no
  matter how high its `GlobalZIndex` is, rendering behind (and stealing
  clicks from) whatever's in the unclipped fixed chrome instead. The MIDI
  track combobox has this same latent constraint (it's also inside that
  `ScrollArea`) but hasn't surfaced as a visible bug yet — if it ever does,
  the fix is the same: move its slot out of the scrollable area too.
  **Fixing the clipping surfaced a second, separate bug in `dialogs::
  combobox` itself, affecting every combobox, not just Scale's**:
  `Pointer<Click>` auto-propagates up the entity hierarchy (every
  `bevy_picking` pointer event does, `#[entity_event(propagate =
  PointerTraversal, auto_propagate)]`) — clicking a dropdown item bubbled
  the same click up to the toggle button (`list`'s ancestor), whose own
  `toggle_click` observer then saw the popup `item_click` had *just* closed
  and immediately reopened it, so picking an item never visually closed
  the dropdown. Fixed by calling `ev.propagate(false)` in all three of the
  widget's own click observers (`toggle_click`/`backdrop_click`/
  `item_click`) — a modal widget shouldn't leak its own clicks to whatever
  it happens to be nested inside, regardless of this specific bug.
- **Lessons**: engine, all five primitives, and the full wave 1 + wave 2
  content pass (Units 1–3, 19 lessons) are shipped — see
  `docs/lessons_plan.md`. Unit 4 "jazz"'s engine prerequisites are also done
  (`song::harmonica::ii_v_i_chords`, `ChordQuality::{Major7,
  HalfDiminished7,Dominant7Alt}`, `Progression::JazzBlues`); what's left is
  content only, the same rights/judgment-sensitive gap as blues content
  (`TODO.md`).
- Remaining 0.4 work (recorded backing loops) — see `ROADMAP.md`/`PLAN.md`.
