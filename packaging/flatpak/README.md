# Flatpak packaging

Files for building Harmonicon as a Flatpak:

| File | Purpose |
|---|---|
| `io.github.tcanabrava.Harmonicon.yaml` | Flatpak build manifest |
| `harmonicon.sh` | Launcher (sets the asset root + working dir, then runs the binary) |
| `io.github.tcanabrava.Harmonicon.desktop` | Desktop entry |
| `io.github.tcanabrava.Harmonicon.metainfo.xml` | AppStream metadata |

The app icon is the repo's `assets/icons/icon.png`.

## Prerequisites

```sh
flatpak install -y flathub \
    org.freedesktop.Platform//24.08 \
    org.freedesktop.Sdk//24.08 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08
# build tool:
flatpak install -y flathub org.flatpak.Builder   # or: distro package flatpak-builder
```

## Build & run (local, network build)

Run from the repository root:

```sh
flatpak run org.flatpak.Builder --user --install --force-clean \
    build-dir packaging/flatpak/io.github.tcanabrava.Harmonicon.yaml
# or, with a system flatpak-builder:
# flatpak-builder --user --install --force-clean \
#     build-dir packaging/flatpak/io.github.tcanabrava.Harmonicon.yaml

flatpak run io.github.tcanabrava.Harmonicon
```

The manifest's git source builds the committed `main` branch. To build your
**local working tree** instead, swap the `sources:` entry for the commented
`type: dir` block in the manifest.

This local manifest grants the build network access (`--share=network`) so cargo
can fetch crates. That's fine for development but **Flathub requires offline
builds** — see below.

## Permissions

`finish-args` grants only what the game needs:

- Wayland / X11 fallback + `--device=dri` — windowing and GPU (Bevy/wgpu).
- `--socket=pulseaudio` — audio output **and microphone input** (pitch detection
  records from the mic; the user may still need to allow mic access in the
  portal/sound settings).

Settings persist automatically in `~/.var/app/io.github.tcanabrava.Harmonicon/config/`
(the game uses `dirs::config_dir()`, which Flatpak redirects per-app).

## Offline / Flathub build (vendored dependencies)

Flathub builds have no network. Generate a cargo sources lockfile and switch the
module to an offline build:

```sh
# Fetch the generator once:
wget https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
```

Then in the manifest:

- add `packaging/flatpak/cargo-sources.json` to the module's `sources:`,
- remove the `build-args: [--share=network]`,
- run cargo offline: `cargo --offline build --release --bin harmonicon`,
- set `CARGO_HOME` to the vendored dir the generator expects (it documents this).

## Notes / caveats

- **App ID vs. repo name.** The GitHub repo is `tcanabrava/harmonicon`, but the app is
  `Harmonicon`, so the ID here is `io.github.tcanabrava.Harmonicon`. Flathub
  requires the `io.github.<user>.<Repo>` ID to match the hosting repo, so a
  Flathub submission would need the ID to track the actual repo name (or the repo
  renamed). For local/self-hosted builds the current ID is fine.
- **Screenshots.** A Flathub submission also needs `<screenshot>` entries in the
  metainfo (hosted image URLs); they're omitted here since none are published yet.
- The build removes the repo's `.cargo/config.toml` (it pins the `wild` linker +
  clang, which aren't in the SDK) so cargo uses the toolchain defaults.
