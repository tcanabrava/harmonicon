# Getting Started

## Running Harmonicon

Harmonicon is a native desktop application (Windows, macOS, Linux — see the
project's release page or your package manager for a prebuilt binary). If
you're building from source:

```bash
cargo run --release
```

You'll need a **display**, **audio output**, and a **microphone** —
Harmonicon listens to your harmonica through the mic the same way it plays
music through your speakers/headphones.

## What you need to play

- **A harmonica.** Diatonic (10-hole, any key) and chromatic (12- or 16-hole)
  harmonicas are both fully supported. If you don't have one yet, a
  diatonic harp in the key of C is the standard beginner's choice and
  matches most of the bundled [Lessons](lessons.md) content.
- **A microphone.** A laptop's built-in mic works, but a dedicated mic (even
  a cheap USB one) picking up less room noise will make pitch detection
  noticeably more reliable. Position it close to the harmonica, not close
  to your mouth.
- **Headphones are recommended** over speakers — they stop the backing
  track/metronome from bleeding into the mic and being misread as notes
  you played.

## The first launch

Harmonicon's lessons and songs are not part of the download: they live in
their own online repositories, so they can grow without waiting for a new
version of the game. The first time it starts, Harmonicon fetches them —
only their latest version, so it stays small — and shows **Getting lessons
and songs** until they arrive. This needs an internet connection once; if it
fails, the screen says why and offers **Retry**. After that the game starts
straight away, and never updates them without asking you.

The first time Harmonicon starts — before it has saved a profile — it opens
on a welcome page instead of the main menu, with the three things worth
doing before anything else: **Set up your microphone** (the Options page,
with the device picker and the "no microphone" warning), **Take the guided
tour**, and **Start with a lesson**. Each one brings you back here when
you're done, with a check beside it, so you can take the next; **Skip for
now** goes straight to the main menu. You can come back to this page any
time from **Help / About → First-time setup**.

![The welcome page](images/welcome.png)

## Picking your microphone

Harmonicon uses whatever input device your system reports as default,
but you can pick a specific one from **Options → Microphone**: a dropdown
lists every input device your system exposes. If the game shows a
"no microphone" warning banner, open Options and confirm a working device is
selected — see [Troubleshooting](troubleshooting.md#no-microphone-detected)
if none of them pick up sound.

![The Options page's microphone picker](images/options-microphone.png)

Before your first scored song, it's worth a quick trip to
**[Calibrating Input Lag](calibration.md)** — every microphone and audio
setup has a slightly different delay between the sound leaving your
harmonica and Harmonicon detecting it, and correcting for it is a
one-time, 30-second step that makes every subsequent judged note more
accurate.

## Adding your own content

Harmonicon also reads from `~/Harmonicon` (a folder in your home
directory) as another source of songs, lessons and themes, alongside the
ones it downloads — drop a song folder or a theme folder in there and it shows
up in the song list / theme picker without needing to reinstall anything.
See [Song Editor](song-editor.md) for how to author a chart of your own.

A song folder only strictly needs one thing: a `.harpchart` file (any
filename) inside its `song/` subfolder. Everything else is optional —
`background.png`, `song/*.ogg` for the backing track, and the `2d/` note
art folder — Harmonicon fills in a generated background, plays no backing
track, and falls back to the selected note theme respectively for
whatever's missing, rather than refusing to load the song. (A `3d/` folder
is accepted but no longer used: Play 3D draws every note as a ribbon.)
