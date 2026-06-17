# Windows installer

`harmonicon.iss` is an [Inno Setup](https://jrsoftware.org/isinfo.php) script that
produces a `harmonicon-setup-<version>.exe` installer. It's built in CI by
`.github/workflows/windows-installer.yaml` — you don't need Windows locally.

## What the installer does

- Installs `harmonicon.exe` + the `assets/` folder into `Program Files\Harmonicon`.
- Creates a Start Menu shortcut (and an optional desktop shortcut) whose
  **working directory is the install folder** — important, because the game both
  loads `assets/` next to the executable *and* reads a few files relative to the
  working directory.
- Registers an uninstaller and uses `assets/icons/icon.png` (converted to `.ico`)
  for the installer, shortcuts, and Add/Remove Programs entry.

## Building it (GitHub Actions)

Since you're on Linux, build via Actions:

- **Test run:** Actions tab → *Windows Installer* → *Run workflow* (the
  `workflow_dispatch` trigger). The installer is uploaded as the
  `harmonicon-windows-installer` artifact (version `0.0.0-dev`).
- **Release:** push a tag (e.g. `git tag v0.1.0 && git push --tags`). The same
  workflow builds the installer and attaches it to the GitHub Release for that
  tag.

The workflow: builds `cargo build --release --bin harmonicon` for
`x86_64-pc-windows-msvc`, stages the exe + assets + `LICENSE.txt`, makes
`icon.ico` from `assets/icons/icon.png` with ImageMagick, then runs Inno Setup
(`ISCC.exe`) on `harmonicon.iss`.

## Prerequisites in the repo

- `assets/icons/icon.png` must be **committed** (the workflow converts it to the
  installer icon).
- The new files here and the workflow must be committed/pushed for Actions to see
  them.

## Notes

- 64-bit only (`x64compatible`), matching the MSVC build target.
- The exe itself isn't given an embedded icon (only the shortcuts/installer are).
  To make Explorer show the icon on `harmonicon.exe` too, embed it at build time
  with the `winresource` crate + a `build.rs` — left out to keep the game build
  unchanged.
- To build locally on a Windows machine instead: install Rust + Inno Setup +
  ImageMagick, run the same steps from the workflow, then
  `ISCC /DMyAppVersion=0.1.0 /DStageDir=<abs path to stage> packaging\windows\harmonicon.iss`.
