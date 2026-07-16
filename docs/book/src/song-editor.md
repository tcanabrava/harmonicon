# Song Editor

**Play → Create Song** is Harmonicon's chart authoring tool — a piano-
roll-style grid for building or editing a `.harpchart` file by hand,
without writing JSON directly.

![Song Editor screen](images/song-editor.png)

## Modes

- **Edit mode** — place, move, resize, and delete notes on the grid.
  Click an empty cell to add a note; drag a note's edges to resize it or
  its body to move it. The **mod panel** on the side sets the selected
  note's technique: Blow/Draw direction, bend depth, overblow/overdraw,
  slide (chromatic only), wah/vibrato rate, or delete it outright.
- **Perform mode** — plays the chart back (▶ Play / ⏸ Pause / ■ Stop), or
  switches to **Practice** mode: play along on your actual harmonica and
  get the same live pitch feedback a real song gives, against the chart
  you're currently editing — the fastest way to sanity-check a chart
  actually feels right before saving it.
- **Lock** — freezes the grid against accidental edits while you're just
  reviewing or practicing.

## Chart metadata

The meta-form covers a chart's song-level fields: music tempo, harp key,
playing position, harmonica type (diatonic/chromatic, and hole layout),
background music file, and song name/author — everything under `song` and
`harmonica` in the `.harpchart` format.

## Importing MIDI

**Import MIDI** loads a `.mid`/`.midi` file and lists its tracks in a
dropdown; picking one drops that track's notes onto the grid, mapped onto
your currently selected harp key and type — an exact note where one exists,
a bend or (on a chromatic harp) a slide where one doesn't, otherwise the
nearest playable note — and sets the chart's tempo to match. Switching the
dropdown to a different track re-imports from that track instead.

Saving while a MIDI track is selected also writes two extra files next to
the chart: a copy of the MIDI file with the imported track removed (your
original file is never touched), and a synthesized backing track —
`song/music.wav` — built from every *other* track in the file, since
Harmonicon can't play a raw MIDI file directly. That backing track plays
automatically both in the editor's own Play preview and, once the song is
in place, during the real game.

## Saving and loading

**Save**/**Load** work with `.harpchart` files directly; **Browse** picks
the background-music audio file a chart references. A saved chart is
validated against Harmonicon's chart schema (`assets/song_schema.dtd.json`)
and tagged with the format version it was written against, so a chart
saved by a newer Harmonicon that added something this version's Song
Editor doesn't understand will point that out clearly instead of silently
mis-loading.

For songs you want the game to discover automatically without editing the
bundled assets, drop the finished chart folder into `~/Harmonicon/songs/`
(see [Getting Started](getting-started.md#adding-your-own-content)).
