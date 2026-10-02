# Controls Reference

Harmonicon is played with your **harmonica and microphone** — the keyboard
and mouse are only for navigating menus and controlling playback, never
for playing notes.

## Global

| Key | Action |
|---|---|
| `Esc` | Pause/resume during a song; go back one menu level otherwise. |
| Mouse | All menu navigation and pause-menu buttons. |

## During gameplay (Play 2D / Play 3D / Jam Session)

| Key | Action |
|---|---|
| `M` | Mute/unmute the metronome click (visual beat indicator keeps working). |
| `V` | Cycle the spectrogram's visual style. |

A **⏸** button in the bottom-right corner does the same thing as `Esc` for
pausing — no keyboard required.

## Song Editor

| Key | Action |
|---|---|
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+C` / `Ctrl+V` | Copy the selection / paste it at the mouse position |
| `Delete` / `Backspace` | Delete the selection |
| `←` / `→` | Pan the grid |
| `Esc` | Clear the selection, or back out of the editor |

See [Song Editor](song-editor.md) for everything else — grid snap modes,
multi-selection, the metronome/count-in, and more.

## Menus and dialogs

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Move keyboard focus to the next / previous control. |
| `Enter` or `Space` | Activate whatever has focus. |
| Arrow keys | Move between the options of a focused tab bar or radio group; nudge a focused slider (`←` / `→`). |
| `Home` / `End` | Send a focused slider to its minimum / maximum. |
| `Esc` | Close an open dropdown, cancel a file dialog, or back out one menu level — whichever applies where you are. |

The UI's overall zoom is the **Zoom** slider in [Options](options.md),
not a keybinding — focus it and `←`/`→` step it, or drag it with the
mouse.

**Every screen can be driven from the keyboard alone.** The control with
focus is outlined by a focus ring, and `Tab` stays inside whatever modal
is open — a confirmation dialog, a file picker, an open dropdown — so it
can't wander onto something you can't see. A focused button responds to
`Enter` and `Space` exactly as it does to a click.

## Scrolling and touch

Any screen with more content than fits scrolls, and there are three ways
to move it:

- the **mouse wheel** over the content,
- the **scrollbar** down its right-hand side (which hides itself entirely
  when everything already fits), or
- **dragging the content itself**, anywhere on it.

Dragging matters most on a touchscreen, where there's no wheel and the
scrollbar is a thin target. It works everywhere a page scrolls — song and
artist lists, the options pages, the lesson tree. Starting a drag on a
button doesn't press it: a short movement is still treated as a tap, and
once you've moved far enough to be clearly swiping, the button under your
finger is let go rather than activated.

The [Lessons](lessons.md) skill tree scrolls in **both** directions, so
dragging it pans diagonally as well.

## Pause menu (mouse-driven)

The pause menu itself is buttons and sliders, not keybindings, but it's
worth knowing what's there since it's easy to miss mid-song. It's two
columns: transport actions on the left, practice aids on the right.

- **Resume**, **Restart**, **Quit Song** (left column)
- **Wait for Note** — freeze the highway/music at the next unhit note
- **Practice Speed** — a slider, 50%–100%
- **A–B Loop** — drag on the song-progress bar to set a loop range;
  **Clear Loop** removes it
- **Adaptive Difficulty** toggle; override a phrase by clicking its
  rectangle on the progress bar's bottom strip, then dragging the
  **Learned** slider

See [Playing a Song](playing-a-song.md#pausing-and-quitting) for what each
of these actually does.

---

## Touch

Harmonicon runs on Android, where there's no keyboard and no mouse wheel.
Every keyboard-only action has an on-screen equivalent, plus two gestures
in the Song Editor:

- **Drag the tool sidebar** to scroll it, when the palette is taller than
  the screen. (A scrollbar thumb in a column that narrow is not a
  realistic target for a thumb.)
- **Two-finger drag** anywhere on the editor to pan the view — sideways
  through the chart, and vertically if anything is off-screen. Two fingers
  rather than one, because one finger is already placing, moving and
  resizing notes.

Switching Options → *Button style* to **icons only** makes the sidebar much
narrower, which is worth doing on a phone: a landscape screen is wide and
short, so spending width costs you nothing and spending height costs you
the grid.

Touch is **not yet verified on real hardware** — it's been exercised on an
emulator only, and hit-target sizes in particular haven't been tuned for
fingers.

In the song picker, the active column header has a highlighted background and a direction arrow; click it again to reverse the order. Sorting keeps the selected song in view. The current song has a bright border. Use Up/Down to move through songs, Home/End to jump to the first/last song, and Enter or Space to start the focused song. Tab and Shift+Tab move between sorting, search, view mode, and songs. Typing in Search filters the list without moving focus away from the field. Search matches partial words in song names, bands, and genres; multiple words can match across those fields. Words of 4–7 characters allow one typo, and longer words allow two, using Levenshtein distance.

The picker shows a result count and a selected-song preview. Hover a song to preview its details without leaving Search. Clear resets search and keeps the field focused; `/` focuses Search when you are outside a text field. The two-position `2d` / `3d` switch beside Play shows the active view with a filled background and remembers your choice when you reopen Play Song. Play starts the selected song and is disabled when no songs match.
