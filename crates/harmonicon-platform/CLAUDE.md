# harmonicon-platform

What the game needs from the machine it runs on: asset discovery,
localization, persisted settings, the visual theme, and the narrow-window
breakpoint.

Nothing here knows what a song, a note or a screen is.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **Asset sources:** bundled `assets/` plus an `external://` source mapped to
  `~/Harmonicon` (registered in `main.rs` before DefaultPlugins). When
  loading siblings of an asset, propagate its source or external songs
  silently resolve against the bundled tree (see comment in `song/loader.rs`).
  - **`~/Harmonicon` is watched live**, not just scanned once at Startup:
    `assets_management::watch` starts one recursive `notify-debouncer-full`
    watcher on it (our own direct dependency, no-op if the folder doesn't
    exist — most players never create it), debounces bursts of filesystem
    events, and fires one generic `ExternalFolderChanged{top_level_dirs}`
    message per batch naming which immediate subfolders (`songs`, `themes`,
    `lessons`, ...) something changed under
    (`watch::changed_top_level_dirs`) — `watch.rs` itself stays agnostic of
    what any of those subfolders *mean* (see "dependencies point downward"
    in `docs/physical_design_plan.md`). `assets_management::mod.rs`'s own
    `rescan_on_external_change` consumes that message for the two kinds
    this module owns (`songs`/`themes`), re-running `scan_all_songs`/
    `scan_ui_themes`; `lessons::catalog` has its own sibling consumer for
    `lessons` (see the Lessons bullet below) — one small
    `lessons`-depends-on-`assets_management` edge rather than the reverse.
    Every scan function fully replaces its resource's contents rather than
    appending, so each is safe to call again at runtime (`scan_all_songs`
    clears `AvailableSongs` first — it didn't always; `scan_ui_themes`/
    `scan_lessons` already assigned wholesale). A successful live rescan
    also fires its own specific `SongsRescanned`/`ThemesRescanned`/
    `LessonsRescanned` — a message, not a bare `is_changed()` poll, because
    the menu pages that consume them only run their consuming system while
    open, so their own change-detection tick would otherwise read
    stale-as-changed on every re-entry rather than only on a genuine live
    drop-in. Deliberately **not** built on `bevy::asset::io::file::
    FileWatcher`/Bevy's own asset-hot-reload path: that path only reloads
    already-loaded `Handle`s (useless for content that was never loaded to
    begin with), and whether *any* source watches at all is one global
    `AssetPlugin::watch_for_changes_override` flag applied to every
    registered source uniformly — turning it on for `external://` would
    also enable asset hot-reloading for the bundled `assets/` tree in
    shipped builds, which is exactly the `--features dev`-only behavior
    this file's Commands section says never to ship.

- **Lesson and song packs** (`content_packs.rs`; design in
  `docs/content_packs_plan.md`). `ContentSources` is the configured
  repository list, persisted in `settings.json` (a player's list *replaces*
  the default official pair — figment replaces arrays, tested). `ContentPacks`
  is each one's state on disk, read at `Startup` in `ContentPacksSet` and
  again on a `RefreshContentPacks` message, which then fires
  `ContentPacksChanged` so the song and lesson scans rerun.
  - **`packs://` is one source with a custom reader**, not a
    `FileAssetReader` per pack: sources are fixed once `AssetPlugin` is built,
    while packs appear at runtime, and a local author's pack can be anywhere
    on disk. The first path segment is the pack's slug, routed through the
    shared `PackRoots` table; a path that tries to leave its pack (`..`) is
    `NotFound`. It reads with `std::fs`, which is what makes it work on
    Android, where the default reader is the APK's.
  - **What a pack may require is decided by the composition root**
    (`PackEngine`), the only place that knows both the game's version and
    `harmonicon-song`'s `LESSON_FORMAT_VERSION`.
  - **Scans skip hidden directories** (`assets_management::is_visible_dir`):
    a pack's checkout keeps its `.git`, which would otherwise read as an
    artist or a unit.
  - **Song asset paths are relative to the scanned root**, with the root's
    prefix (`songs/`, `external://songs/`, `packs://<slug>/`) prepended — a
    pack checkout has no `songs` folder above its content to anchor on.

- **This crate owns the whole Fluent stack**, down to the asset loaders.
  `localization::ftl` holds the `.ftl`/`.ftl.ron` `AssetLoader`s and the
  `LocaleBundle` asset; `localization::Locale` does the language
  negotiation; `localization::Localization` is the negotiated bundle set
  every other crate takes as `Res<Localization>`. All of that used to be
  `bevy_fluent`, whose version tracks Bevy's — absorbed here (see
  `LICENSE-bevy_fluent`, MIT) so a Bevy upgrade can't be blocked on a
  third-party plugin's release cadence. Everything *below* it (`fluent`,
  `fluent_content`, `fluent-langneg`, `intl-memoizer`) has no Bevy in its
  tree and stays a plain dependency.
  - Two things the absorbed version deliberately doesn't have: the
    `.ftl.yaml`/`.ftl.yml` bundle formats (only `.ftl.ron` ships) and
    upstream's `LocalizationBuilder`, which builds from a
    `Handle<LoadedFolder>` — `build_from_bundles` negotiates from the
    explicit per-locale handles instead, because this module never
    enumerates a directory (see its own doc comment for why).

- **Settings:** figment-layered `<config>/harmonicon/settings.json`
  (`settings.rs`); saves are debounced (`PendingSave`, 0.5 s) with a flush
  on `AppExit` — route new persisted fields through that path.

- **Responsive/compact layout for narrow windows** (`harmonicon-platform`'s `responsive.rs`).
  `CompactLayout` (a `Resource`) is derived every frame from the primary
  window's width divided by `UiScale` (the same effective-width math
  `song_editor::interaction`'s own scroll-clamping already used) crossing a
  single shared `COMPACT_BREAKPOINT_PX` (900.0) — one definition of
  "compact" reused everywhere, rather than each screen picking its own
  threshold. Deliberately **not live-reactive**: `gameplay_2d::setup`/
  `gameplay_3d::setup` and `song_editor::ui::setup` each read
  `Res<CompactLayout>` once, at `OnEnter(AppState::Playing)`/
  `OnEnter(AppState::SongEditor2)`, and branch what they spawn — a resize
  *during* a song or edit session doesn't retroactively reflow it. Live-
  reflowing an already-spawned scene would need despawn/respawn logic on
  par with a second setup system, for a scenario that matters far less than
  "the screen you land on already fits" — extend this only if that turns
  out to be wrong.
  - **Play 2D/3D compact**: the note highway (2D) / 3D scene stays, plus a
    minimal score/combo/feedback readout; everything else gameplay's HUD
    normally shows — song info, phrase banner, tab ribbon, the 12-bar
    grid, metronome, technique legend, and the `music_score` notation
    staff — is skipped entirely rather than shrunk, since none of it is
    essential to actually playing. `gameplay_3d::setup` bundles `theme`/
    `loc`/`bravura`/`compact` into a new `HudContext` `SystemParam`
    (mirroring the file's own pre-existing `NoteBuildState`) purely
    because plain individual params would have put it one over Bevy's
    function-system arity limit — nothing about the four belonging
    together otherwise.
  - **Song Editor compact**: `meta_form::spawn_meta_form`'s three
    side-by-side columns (content-kind/harmonica/snap fields, the rest of
    the fields + MIDI row, the color legend) stack vertically instead —
    the tightest rigid constraint found anywhere in the app (≈1122px
    before clipping, no wrap, only vertical scrolling available). The top
    transport strip (`mod_panel.rs`) also gained `flex_wrap`, unconditionally
    (not gated on `CompactLayout` at all — wrapping only engages once
    content overflows, so it's a strict improvement on wide screens too).
    The note grid needed no changes — it already self-scrolls horizontally
    regardless of window width (`grid.rs`'s own `visible_beats`/
    `GridScrollTrack`).
  - **Menu pages are out of scope** — they already handle overflow via
    `menu::scene::spawn_menu_root`'s scroll area, so they were judged
    reasonably small-screen-safe already.
