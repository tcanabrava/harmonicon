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
  actually feels right before saving it. **⏺ Record** goes the other
  direction: play your harmonica and have it write notes onto the grid for
  you (see [Recording notes live](#recording-notes-live) below).
- **Lock** — freezes the grid against accidental edits while you're just
  reviewing or practicing.

## Chart metadata

The meta-form covers a chart's song-level fields: music tempo, harp key,
playing position, harmonica type (diatonic/chromatic, and hole layout),
background music file, and song name/author — everything under `song` and
`harmonica` in the `.harpchart` format.

## Authoring a lesson

The **Recording** field in the meta form cycles between **Record Song**
and **Record Lesson**. Switching to Record Lesson doesn't change anything
about editing notes, playing back, or practicing — it just adds a
curriculum layer on top of the chart you're building, so a lesson's chart
is really just an ordinary chart with some extra metadata attached.

Click the **▸ Lesson Details** header to expand the curriculum fields
(collapsed by default, so it stays out of the way while you're just
placing notes):

- **Lesson ID** and **Unit** — the lesson's identity and which curriculum
  unit it's grouped under in the [Lessons](lessons.md) list.
- **Explanation** — the instructional text shown on the lesson's reader
  page.
- **Prerequisites** — a comma-separated list of lesson IDs that must be
  passed first, before this one unlocks.
- **Pass Criteria** — how the lesson is judged: an accuracy threshold, a
  specific technique's accuracy, or (for an open-jam lesson with no fixed
  notes) scale adherence, chord-tone adherence, or phrase discipline.
  **Threshold** and **Technique** only appear when the chosen criterion
  actually needs them.
- **Progression** — the backing chord progression an open-jam lesson
  starts with (standard, quick-change, minor, or none).

Saving writes a `lesson.json` file (validated against the lesson schema
before writing) alongside the chart, if the grid has any notes on it.
**One thing `lesson.json` can't do**: it stores the lesson's title and
explanation as Fluent *keys*, never the actual display text you typed —
Harmonicon's translated-text system needs real entries in each supported
language's locale file, which this tool can't generate for you. After
saving, check the game's log/console: it prints the exact key/text pairs
to add by hand.

A lesson save doesn't carry over a MIDI-imported backing track — author
the chart as an ordinary song first if it needs one, then switch to
Record Lesson to add the curriculum fields on top.

## Erasing and removing parts of a song

The **Erase** and **Remove** buttons in the mod panel (next to Delete) turn
the ruler above the grid into an editing tool for a whole span of time
rather than one note at a time — handy for a song built from an imported
MIDI track that starts later than beat 1, or just cutting a section you
don't want.

With one of them selected: click a point on the ruler to drop a split
marker, then click either side of it to act on everything from there to
that edge of the song — or click-drag-release across a span instead to
pick an explicit range. Either way, a confirmation dialog names the exact
range before anything happens. **Erase** deletes the notes in that range
and leaves a gap; **Remove** deletes them *and* shifts every note after the
range earlier to close the gap, shortening the song. Escape cancels a
pending split or drag before you confirm it.

## Silence track

A thin strip below the last hole lane, labeled "Silence", shows the gap
between consecutive notes as a block giving its length in seconds —
useful for spotting an unintentionally long rest, or confirming a deliberate
one lines up with the phrasing you meant. A chord, or notes placed back to
back with no rest between them, shows no block; there's also none before the
first note or after the last, since there's no gap to measure there. It's
purely a display — nothing on it is clickable.

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

## Recording notes live

**⏺ Record** (Perform mode) writes a chart by ear: click it — the button
changes to **⏹ Stop Recording** — then play your harmonica. Each note
appears on the grid the instant you start playing it and keeps growing for
as long as you hold it, so you watch it take shape in real time rather than
only seeing it once you stop. Notes are mapped onto your currently selected
harp key and type exactly the way MIDI import maps a file's notes (an exact
note where one exists, a bend or slide where one doesn't) — a bend you
actually play and hold is recorded as a bend, not snapped to the nearest
natural note. The status bar shows a running count of notes captured while
you play. Click Stop Recording, or Stop, to end the take — whatever note
you're still holding at that instant stops growing right there.

While recording, pitch detection is tuned to your selected harp: only
sounds that harp can actually make are considered (a stray harmonic or
room noise at an impossible pitch is ignored rather than snapped onto the
grid), a note that flickers for only a single instant is treated as noise
and removed again, and a brief detection dropout mid-note won't split a
held note in two. Detected notes are also placed slightly earlier than the
moment they're recognized, compensating for the analysis delay — plus
whatever input latency you've calibrated on the Options page — so takes
land on the beat you actually played. Two tips for cleaner takes: wear
headphones if the chart has background music (otherwise the microphone
hears the music too and can record its notes as yours), and the **MPM**
pitch algorithm on the Options page is a strong choice for single-note
playing.

Recording only ever *adds* notes; it never deletes or replaces what's
already on the grid, so you can record several takes (or record over an
imported/hand-placed part) without losing earlier work. If the chart has
background music set, it plays automatically while you record, the same as
Play and Practice, so you can play along to it.

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
