# scripts/

## `release.sh` — cut a release

```bash
./scripts/release.sh --patch          # 0.0.10 -> 0.0.11
./scripts/release.sh --minor          # 0.0.10 -> 0.1.0
./scripts/release.sh --major          # 0.0.10 -> 1.0.0
./scripts/release.sh --patch --dry-run   # print every change and stop
```

Exactly one of `--major`/`--minor`/`--patch`. It bumps the version
everywhere it's written down, commits, tags, and pushes — asking before the
push, and printing a diffstat first.

### What it writes, and what it deliberately doesn't

| File | Why |
|---|---|
| `Cargo.toml` | the root `[package]` version — this is `CARGO_PKG_VERSION`, what Help / About shows |
| `Cargo.lock` | carries the workspace's own package version; refreshed by `cargo metadata`, not by hand |
| `packaging/android/app/build.gradle.kts` | `versionName` **and** `versionCode` |
| `packaging/flatpak/*.metainfo.xml` | prepends a `<release>` entry dated today; Flathub reads this |
| `CHANGELOG.md` | prepends a section for the new tag, one bullet per commit subject since the last tag |

**Not** touched, on purpose:

- `packaging/macos/Info.plist` — a `%%VERSION%%` placeholder that CI
  substitutes from the tag.
- `packaging/windows/harmonicon.iss` — CI passes `/DMyAppVersion` from the
  tag.

Both already derive from the tag this script creates. Writing a number into
them would add a second source of truth for no gain.

### Why a script instead of a checklist

These numbers are read by different things at different times, so they drift
silently and only disagree in front of a user. They already had: `Cargo.toml`
said `0.1.0` through every `0.0.x` tag, so the version shown inside the game
was one no release ever carried — and Android's `versionName` was still
`0.1.0` when this script was written, because nothing reads it until someone
installs an APK.

`release.yaml`'s `check_version_matches_tag` catches a tag that disagrees
with `Cargo.toml`, but nothing catches the packaging files.

### The changelog

Each release prepends a section to `CHANGELOG.md`, newest first, built from
`git log --no-merges` since the last tag — one bullet per commit subject.

Subjects rather than a hand-written summary because this project's commit
messages already lead with a real sentence about what changed. The useful
changelog is sitting in them, and anything maintained alongside would be a
second thing to keep true.

### A dirty tree, when it's only the version

Bumping often starts by hand — editing `Cargo.toml` to line the manifest back
up with the tags, which also touches `Cargo.lock`. Refusing that would be
refusing the normal way in, so the script allows it and sweeps it into the
release commit.

The exemption is narrow on purpose. Only those two files may be modified,
**and** every changed line in them must itself be a `version = "x.y.z"` line
— a dependency bump in `Cargo.lock` is not a version-only change. Anything
else is still refused, because an unrelated half-finished edit must not ride
along in a commit called `Release vX.Y.Z`.

### Refusals

It stops rather than doing something surprising when:

- any *other* uncommitted change is present (see above);
- the new version wouldn't be **above** the current one, or above the
  highest existing tag (comparison is `sort -V`, and the tag line's
  four-component strays like `v0.0.9.1` are normalised to three parts);
- the tag already exists;
- you're not on `main`;
- the branch is behind `origin/main`;
- `minor` or `patch` would reach 100, which would break the Android
  `versionCode` encoding (`major*10000 + minor*100 + patch` — chosen because
  Play Store requires a monotonically increasing integer, separate from the
  name).

### After it pushes

The tag push starts `.github/workflows/release.yaml`, whose first job
re-checks the tag against `Cargo.toml` before anything is built.

## `brpctl.py` — drive a running game from the shell

Needs a game started with `--features dev`, which serves the Bevy Remote
Protocol on `127.0.0.1:15702`. `contributing/src/remote-control.md` explains
the protocol; this is it with the fiddly parts handled.

```bash
python3 scripts/brpctl.py buttons            # what is clickable right now
python3 scripts/brpctl.py click "Play Song"
python3 scripts/brpctl.py shot               # -> target/screenshots/
python3 scripts/brpctl.py resize 1920 1080   # physical px
python3 scripts/brpctl.py --take-all-screenshots [outdir]
```

Standard library only — no virtualenv, unlike the asset tools below, because
this is meant to be reachable in the middle of debugging something else.

`loop 12 24` sets an A–B range directly — the real path is a pointer drag
the protocol can't perform — and `loop off` clears it.

Getting to a screen a state write can't reach — anything that needs a song
picked first — is a route: `enter "Play 2D" <artist> <song>`, `jam`,
`editor` (loads a chart through the editor's own file dialog) and `results`
(plays the shortest bundled chart to its end). `scripts/audit_page_layout.py`
uses the same routes for the screens `NextState` can't reach.

`--take-all-screenshots` drives the whole app and writes one **named** PNG per
screen: Play 2D on three chart fixtures (no techniques / bend+vibrato+wah /
chromatic), Play 3D, the pause menu, wait-for-note and results, at 1920×1080 —
the supported floor, and the only size worth baselining, since anything
narrower is Android portrait rather than a small desktop. Takes a few minutes.
The alternative is a pile of `shot_<millis>.png` distinguishable only by
remembering what order you took them in.

Two things it knows that are easy to get wrong by hand:

- **A button's label is every text node under it, joined**, and the
  discriminator between two identically-labelled buttons is their *parent's*
  subtree, not their index — indices come from ECS query order. The HUD's
  pause control and the pause menu's wait-for-note toggle are both exactly
  `"⏸"`.
- **Toggles are read before they are set.** Wait-for-note is persisted, so a
  blind toggle does the opposite of what you meant whenever the last session
  left it on.

## `run-dev.sh` — rebuild and relaunch with BRP up

```bash
./scripts/run-dev.sh              # rebuild, relaunch, wait for BRP
./scripts/run-dev.sh --no-build   # relaunch only
./scripts/run-dev.sh --stop       # stop a running instance
```

Waits until the Bevy Remote Protocol server actually answers before
returning, so the next command can drive it without guessing at a sleep.

One command rather than a chain of three on purpose: a chain can only be
permitted as a whole, so `cargo build && nohup env … && python3 …` matches no
narrow allowlist pattern and prompts every single time.

It kills by the built path (`target/release/harmonicon`), not the bare name —
`pkill -f harmonicon` also matches the shell running the script.

## Everything else here

`generate_lesson_files.py` and `make_note_cube.py` are one-off
authoring/asset tools, run by hand.
`git-hooks/` holds the hooks and `install.sh`; run that once per clone.
