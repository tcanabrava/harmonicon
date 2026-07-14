# Playing a Song

**Play → Play Song** starts the scored song flow:

1. **Select Mode** — choose [Play 2D](play-2d.md) (a scrolling note
   highway) or [Play 3D](play-3d.md) (a 3D harmonica model you play along
   with). Both modes share the same scoring, timing, and pause menu — it's
   purely a visual choice.
2. **Select Artist**, then **Select Song** — browse the bundled songs (and
   anything you've dropped into `~/Harmonicon/songs/`, see
   [Getting Started](getting-started.md#adding-your-own-content)).
3. A **3-2-1 countdown** plays, showing the song title, key, and which
   physical harmonica to grab, then the chart starts scrolling and the
   backing track plays.

![Mode select screen](images/mode-select.png)

## Scoring

As notes reach the hit line, Harmonicon compares the pitch it hears against
what the chart expects, at that instant:

- **Perfect** / **Good** hits, based on how close your timing was to the
  note's onset.
- **Miss**, if the window passes with nothing (or the wrong pitch) played.
- Longer notes reward **holding** the correct pitch for their full
  duration, not just landing the onset.
- Special techniques — **bends**, **vibrato**, **wah**, **overblow/
  overdraw**, and (chromatic only) **slides** — are validated on their own
  terms, not just "was some pitch playing": a bend note checks you actually
  bent to the target pitch, a vibrato/wah note checks the oscillation rate
  you played matches what the chart asks for.
- **Chords and octave-split notes** only score when every note in the
  group sounds *together* — playing the same holes correctly but one at a
  time doesn't count.

A combo multiplier builds on consecutive hits and resets on a miss. The
**Results screen** after each song breaks your accuracy down **by
technique** (not just an overall percentage), so you can see at a glance
whether it was your bends or your timing that need work.

![Results screen](images/results-screen.png)

## Pausing and quitting

Press **Esc** mid-song to pause. The pause menu offers:

- **Resume** / **Restart** (reloads the song from the countdown).
- **Quit Song** — returns to the song list, not the Main Menu.
- **Wait for Note** — freezes the highway and music the instant an unhit
  note reaches the hit line, and holds there until you play it — useful
  for slowing down a hard passage without losing your place. There's no
  way to "miss" a frozen note; it just waits.
- **Practice Speed** — slows the highway and metronome (down to 50%)
  without pitch-shifting the audio; it mutes instead, so you never hear a
  chipmunked backing track.
- **A–B Looping** — drag on the song-progress bar at the top of the screen
  to mark a section and loop it, for drilling one phrase repeatedly.
- **Adaptive Difficulty** — on by default: a song's notes unlock
  gradually as you clear each phrase cleanly, instead of throwing the full
  chart at you immediately. You can override an individual phrase's
  progress, or turn the whole feature off, from the pause menu.

See the [Controls Reference](controls.md) for every in-game keybinding.
