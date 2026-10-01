# Content Packs

The game ships no lessons and, eventually, no songs. Both are **packs**: git
repositories the game downloads and keeps up to date. The official ones are
`github.com/tcanabrava/harmonicon-lessons` and
`github.com/tcanabrava/harmonicon-songs`, and a player can add any other
repository laid out the same way. Content therefore changes on its own
schedule, not the engine's release schedule.

## What a pack is

A repository with a `pack.json` at its root:

```json
{
  "schema": 1,
  "kind": "lessons",
  "id": "harmonicon-lessons",
  "name": "Harmonicon lessons",
  "version": "1.0.0",
  "requires": { "harmonicon": ">=0.0.13", "lesson_format": 1 }
}
```

Beside it is the content, laid out as the game already reads it:
`<unit>/<lesson>/lesson.json` for lessons, `<artist>/<song>/song/` for songs.
Translations travel with the content: every `locales/<lang>.ftl` anywhere in
a pack is loaded, at the root for strings shared across the pack, and beside
each lesson for that lesson's text.

## Compatibility

`harmonicon-packs::pack::PackManifest::check` refuses a pack, with a message
saying what to do, when:

- its `schema` is newer than `PACK_SCHEMA`;
- `requires.harmonicon`, a semver range, doesn't match the game's version
  (a release candidate counts as its release);
- `requires.lesson_format` exceeds `LESSON_FORMAT_VERSION`.

`LESSON_FORMAT_VERSION` (`harmonicon-song`'s `lessons/manifest.rs`) is bumped
in the same change that adds anything an older build can't parse, such as a
widget or pass-criteria type. The composition root combines it with
`CARGO_PKG_VERSION` into the `PackEngine` resource, because it is the only
place that knows both.

## Where the pieces live

| Concern | Where |
|---|---|
| `pack.json`, compatibility, configured and installed repositories, the git transport | `harmonicon-packs` (no Bevy) |
| The configured list (`ContentSources`, in `settings.json`), each pack's state (`ContentPacks`), the `packs://` asset source | `harmonicon-platform::content_packs` |
| Downloads and update checks | `harmonicon-platform::content_sync` |
| Pack translations | `harmonicon-platform::localization::packs` |
| The first-run download screen (`AppState::Syncing`) | `harmonicon-menu`'s `pages::content_sync` |
| Validation | `harmonicon-song`'s `lessons::validate` and `song::validate`, run by the `validate-pack` binary |

## Downloading

Each clone is **depth 1**, made with `gix` (so no `git` binary is needed),
into a staging directory. It is checked (`pack.json` present and compatible,
no symbolic links, under a size cap) and only then swapped into place. An
update is a fresh shallow clone rather than a fetch, so history never
accumulates on the player's disk, and a failed or refused update leaves the
installed copy exactly as it was. Checking for updates asks the remote only
for its ref advertisement, which downloads nothing.

Downloads run on their own named threads rather than a Bevy task pool,
because a clone is a long blocking network call and would otherwise stall
asset loading. Only the main thread writes `installed.json`.

At startup a configured repository that has never been downloaded is
installed, and the game waits on the download screen until it is. An
installed one is only checked for updates, and nothing updates without the
player asking.

## Reading

One asset source, `packs://`, serves every pack: the first path segment is
the pack's slug, which `PackRoots` maps to a directory. It is one source
with a custom reader, not a source per pack, because sources are fixed once
`AssetPlugin` is built while packs come and go at runtime, and a local pack
can be anywhere on disk. It reads with `std::fs`, which is also what makes it
work on Android, whose default reader is the APK's.

Scans skip hidden directories, since a checkout keeps its `.git`. For each
language the game's own strings are consulted before a pack's, so a pack can
add strings but never replace one of the game's.

wasm can neither clone nor list a directory. Its lessons are bundled at build
time from the directory `HARMONICON_LESSONS_DIR` names (see
`harmonicon-song`'s `build.rs`).

## Authoring and validating

```sh
cargo run --bin validate-pack -- ../harmonicon-lessons
```

The tool checks a pack the way this build will read it: schema, every chart,
prerequisites and the curriculum graph, translations (Fluent syntax, and
every key a lesson uses defined in `en-US` and in every other language the
pack ships), and tab notation. Each pack's CI runs it against the engine's
`main`. Its rules are tested here against `tests/fixtures/lesson-pack`.

To try a change before publishing it, configure the checkout as a local
pack, which is read in place:

```json
"content_sources": { "lessons": [{ "path": "/path/to/harmonicon-lessons" }] }
```
