# harmonicon-menu

Every screen outside gameplay: the page state machine, routing, the
shared page scaffolding, and one file per page.

The top of the library stack — it registers the editor and reaches jam,
so nothing may depend on it but the binary.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **Every menu page's content area auto-scrolls once it overflows.**
  `menu::scene::spawn_menu_root` no longer returns the outer root's own
  entity for callers to add buttons/rows to — it spawns a
  `dialogs::scroll_area::spawn_scroll_area` (a `bevy_ui_widgets::
  ScrollArea` column paired with a real `Scrollbar`/`ScrollbarThumb`,
  generalized out of the Song Editor's own `song_editor::scroll::
  spawn_editor_scrollbar`) as a child of the root and returns *that*
  entity instead, so all 22 existing `spawn_menu_root` call sites needed
  zero changes to gain scrolling. The scrollbar
  (`dialogs::scroll_area::update_scrollbar_visibility`, registered once
  app-wide via `ScrollAreaPlugin`) toggles both `Visibility` and
  `Node::display` together, collapsed to `Display::None` whenever content
  already fits — `Visibility::Hidden` alone would still reserve the
  track's width and nudge every short, perfectly-centered menu (Main,
  Options, ...) slightly off-center even with nothing to scroll to. A
  long list (Artist List, Lessons, Theme picker) that outgrows the screen
  gets a visible, draggable scrollbar instead of silently overflowing
  past the edges with no way to reach the rest.

- **The skill tree is the one page that scrolls both ways.**
  `menu/pages/lesson_tree.rs` builds its own
  `dialogs::scroll_area::spawn_scroll_area_xy` on top of
  `spawn_menu_root_plain` rather than taking `spawn_menu_root`'s vertical
  `ScrollArea`, because its canvas is genuinely wider than any window: a
  node's column is its depth in the prerequisite graph, and
  `lesson_tree::layout::spread_columns` then pushes whole sibling groups
  further right until no column stacks more than `MAX_PER_COLUMN` deep
  (the shipped curriculum goes from 7 columns by 9 rows to 16 by 4).
  Height is the axis a reader can't afford; width is the one that
  scrolls. Two things this needs that an ordinary page doesn't: the
  canvas carries `flex_shrink: 0.0` (every node inside is positioned
  absolutely, so a collapsed box would keep its children's pixel offsets
  while the scroll extent is computed from the collapsed size — the tree
  spills past both ends with neither reachable), and the content column
  `spawn_menu_root_plain` returns is reshaped with `min_height`/
  `min_width: 0` so it can actually shrink under the canvas, the same
  "min-height: auto" gotcha `scroll_area` documents one level further
  out.

- **Guided tutorial tour** (`harmonicon-menu`'s `menu/pages/tutorial.rs`): a "Tutorial" button on
  the Help/About menu drives a fixed sequence (`TOUR_STEPS`, each a
  `TourTarget`) on a timer, with a click-blocking overlay on top naming the
  current screen and briefly explaining it. Most steps are `TourTarget::
  Page` — the top-level, no-selection-required `MenuPage`s (Main, Play,
  Mode Select, Jam Session Menu, Generate Jam, Lessons, Options, Theme,
  Help/About; not `ArtistList`/`SongList`/`LessonReader`, which need an
  artist/song/lesson already picked) — but four steps actually enter live
  gameplay for a look:
  `TourTarget::Playing(GameplayMode::Play2D)` and `::JamSession` (both
  load the bundled `DEMO_SONG_PATH`, long enough that no step could ever
  run it to completion and trigger a real `AppState::Results`),
  `TourTarget::BendingTrainer`, and `TourTarget::SongEditor` — the exact
  same `AppState` transitions those screens' normal entry points use, so
  none of their own systems need to know a tour is happening.
  - **Crossing an `AppState` boundary back into `Menu` can't set
    `NextState<MenuPage>` directly** — same reason `ReturnToSongList`/
    `ReturnToOptions`/`ReturnToPlay`/`LessonContext`/`GeneratedJamSession`
    all exist as flags instead: setting it in the same tick as `NextState<AppState>`
    loses to the substate machinery resetting to its own default first.
    `enter_tour_target`'s `Page` case only ever queues `AppState::Menu`;
    `route_menu_entry` (extended to check the tour *first*, ahead of those
    other flags) reads `tour_menu_landing`/`tour_finished` to pick the
    right page and, once the tour has run its last step, actually remove
    the `TutorialTour` resource — `step == TOUR_STEPS.len()` is a one-frame
    "ending" sentinel `end_tutorial_tour` sets so `route_menu_entry` still
    sees the tour present (and can route to `return_to`) on that final pass.
  - The overlay's root entity is deliberately *not* `MenuRoot`/
    `GameplayRoot` — none of the screens the tour drives through despawn it
    as part of their own teardown; only the tour's own end logic does. Both
    tour-driving systems (`advance_tutorial_tour`/`sync_tutorial_overlay`)
    run unconditionally (each checks `Option<Res<TutorialTour>>` itself)
    rather than being gated to `AppState::Menu`, since some steps leave it.
  - `tour_active` (a `run_if` condition, `pub(crate)` precisely so other
    modules can gate on it without needing `TutorialTour`'s fields) is
    threaded into every Escape/pause handler a tour step could otherwise
    run into — `gameplay::pause_menu::handle_pause_input`,
    `gameplay::bending_trainer::handle_escape`,
    `song_editor::interaction::grid_keys` — so a tour can't be knocked off
    course by Esc while showing a live screen; the overlay's own "Skip
    Tutorial" button is the one deliberate way out.
