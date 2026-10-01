# Content packs: lessons and songs as git repositories

Lessons and songs move out of this repository into their own
(`github.com/tcanabrava/harmonicon-lessons`, `.../harmonicon-songs`), so
authoring content no longer follows the engine's release schedule and anyone
can install a pack from any git remote.

## Decisions

- **Backend: `gix`**, behind a small trait so tests run against local
  `file://` repositories. No `git` binary is assumed (flatpak has none).
- **Shallow only.** Clone and update with depth 1; history never accumulates.
  An update fetches into a temp directory and swaps it in atomically, so a
  failed update cannot corrupt an install.
- **Update checks download nothing** (`ls-remote` against the stored SHA), and
  an update is never applied without asking.
- **wasm** gets the latest repositories at *build* time and bundles them
  through the existing `build.rs` manifests (no runtime clone).
- **Android** syncs at first start like desktop. It needs the `INTERNET`
  permission, `rustls-platform-verifier`'s JNI initialisation (it reads the
  system trust store through Java on Android), and a `packs://` source built
  on a *filesystem* reader — `AssetSource::get_default_reader` yields the APK
  reader there (see `contributing/src/android-build.md`).
- **Offline first run**: the sync dialog shows an error with Retry and Quit.
- **Layout** stays `<unit>/<lesson>/`. Unit and track names, which today are
  derived from directory names plus `lesson-unit-*`/`lesson-track-*` keys in
  `ui.ftl`, move into the pack's own `locales/`.

## Pack format

```
pack.json                      {schema, kind, id, name, version,
                                requires: {harmonicon, lesson_format}}
locales/<lang>.ftl             unit-* and track-* names
<unit>/<lesson>/lesson.json
<unit>/<lesson>/locales/<lang>.ftl
```

- A pack is rejected, with a "needs Harmonicon ≥ X" message, when its
  `schema` is newer than the engine's, its `requires.harmonicon` doesn't
  match `CARGO_PKG_VERSION`, or its `lesson_format` exceeds
  `LESSON_FORMAT_VERSION` (bumped whenever a widget or pass-criteria variant
  is added, like `CURRENT_FORMAT_VERSION` for charts).
- A lesson may declare `requires_format`; an incompatible lesson is skipped
  **together with everything that transitively depends on it**, because
  `LessonGraph::build` errors on a missing prerequisite.
- Lesson localization keys must be prefixed with the lesson id (Fluent has no
  namespaces); pack bundles are consulted before the app's, and a pack may
  ship only some locales.
- Songs keep per-chart `format_version`; `pack.json` adds the repo-level gate.

## Phases (commit after each)

1. ~~**`harmonicon-packs`**~~ — landed. Not done from the original scope:
   no backend trait (tests drive gix against real `file://` repositories,
   which proved simpler than a fake), and no download progress yet (phase 4
   shows an indeterminate spinner until it needs more).
2. ~~**Engine integration**~~ — landed: `packs://`, `ContentSources` in
   settings, `ContentPacks`, pack scans for songs and lessons, rescans on
   `ContentPacksChanged`. Nothing downloads yet, so with the default
   settings both official packs read as not installed until phase 4.
3. ~~**Bundled localization**~~ — landed: `localization::packs`. The audit of
   which `lesson-*` keys in `ui.ftl` are engine chrome and which are lesson
   content moves to phase 6, where the content keys leave `ui.ftl`.
4. ~~**Startup sync**~~ — landed: `content_sync` and the
   `AppState::Syncing` screen. **Until the two official repositories have a
   commit, a fresh install cannot get past that screen** (it shows "the
   repository has no commits yet" with Retry and Quit) — phase 6 has to
   populate them before this ships.
5. **Options UI**: repository list per kind (version, short SHA, status),
   check/update/remove/add, confirmations through `dialogs::confirm_dialog`,
   a text-input widget in `dialogs/` if none exists, three locales.
6. **Extraction** — lessons done: they live in `harmonicon-lessons` with
   their translations, `validate-pack` checks them (and runs in that repo's
   CI), and this repo tests the validator on `tests/fixtures/lesson-pack`.
   Open: songs, the same way; a web build that sets
   `HARMONICON_LESSONS_DIR` and serves the pack under `assets/lessons/`
   (there is no web deploy yet, only CI's `cargo check`).
7. **Docs**: the contributor chapter (`contributing/src/content-packs.md`)
   and engine notes landed with phase 6; open is the player chapter in
   `docs/book` ("Content sources"), once the Options page exists.
