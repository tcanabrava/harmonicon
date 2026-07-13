# Harmonicon — English (en-US) UI strings.
#
# Fluent reference: https://projectfluent.org/fluent/guide/
# Keys are kebab-case and grouped by the screen that uses them. Add new keys
# here first, then mirror them into every other locale under assets/locales/.

app-title = Harmonicon

# Main menu
menu-play = Play
menu-song-editor-2 = Song Editor
menu-options = Options
menu-credits = Credits
menu-quit = Quit

# Play menu
play-song = Play Song
jam-session = Jam Session
bending-trainer = Bending Trainer

# Mode select
select-mode = Select Mode
play-2d = Play 2D
play-3d = Play 3D

# Song / artist selection
select-artist = Select Artist
select-song = Select Song
no-songs-found = No songs found. Add folders under assets/songs/<artist>/<song>/

# Options
options-title = Options
options-language = Language

# Shared
back = ← Back

# Song Editor 2 — transport & mod-panel buttons
editor-mode-edit = ✎ Edit
editor-mode-perform = 🎵 Perform
editor-lock = 🔒 Lock
editor-play = ▶ Play
editor-pause = ⏸ Pause
editor-stop = ■ Stop
editor-practice = 🎤 Practice
editor-save = 💾 Save
editor-load = 📂 Load
editor-browse = 📂 Browse
mod-blow = Blow
mod-draw = Draw
mod-bend = Bend
mod-overblow = Overblow
mod-overdraw = Overdraw
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Delete

# Song Editor 2 — meta-form field labels
editor-field-tempo = Music Tempo
editor-field-key = Harp Key
editor-field-position = Position
editor-field-harmonica = Harmonica
editor-field-music = Background Music
editor-field-name = Name
editor-field-author = Author
editor-harmonica-diatonic = ‹ Diatonic (10 holes) ›
editor-harmonica-chromatic = ‹ Chromatic (12 holes) ›

# Song Editor 2 — file-dialog titles
dialog-save-chart = Save chart
dialog-load-chart = Load chart
dialog-select-music = Select background music

# Song Editor 2 — drag validation messages
drag-denied-bend = This hole does not support this bend depth
drag-denied-overblow = Overblow is only available on holes 1–6
drag-denied-overdraw = Overdraw is only available on holes 7–10
drag-denied-overlap = Another note is already here

# Song Editor 2 — practice mode feedback
practice-no-music = No background music set — play along with the chart!
practice-prompt = ▶ Play %note%…
practice-wrong-note = ▶ %got% → need %expected%
practice-hit-perfect = ✓ PERFECT  %note%  +%pts% pts
practice-hit-good = ✓ GOOD  %note%  +%pts% pts
practice-missed = ✗ Missed %note%
practice-done = Done — %hits%/%total% notes  ·  %score% pts

# Song Editor 2 — button tooltips
editor-back-tooltip = Leave the editor and return to the main menu
editor-mode-edit-tooltip = Switch to Edit mode — place, move, and edit notes on the grid
editor-mode-perform-tooltip = Switch to Perform mode — play back or practice the chart
editor-lock-tooltip = Lock the grid to prevent accidental edits while reviewing
editor-save-tooltip = Save this chart to a .harpchart file
editor-load-tooltip = Load a chart from a .harpchart file
editor-play-tooltip = Start or resume playback of the chart
editor-pause-tooltip = Pause playback in place
editor-stop-tooltip = Stop playback and reset the playhead to the start
editor-practice-tooltip = Practice mode — play along on your harmonica with live feedback
mod-blow-tooltip = Set the selected note to a blow (exhale) note
mod-draw-tooltip = Set the selected note to a draw (inhale) note
mod-bend-tooltip = Cycle the selected note's bend depth: none → half step → whole step → step and a half
mod-overblow-tooltip = Set the selected note to an overblow (advanced blow technique, diatonic only)
mod-overdraw-tooltip = Set the selected note to an overdraw (advanced draw technique, diatonic only)
mod-slide-tooltip = Set the selected note to use the slide button (chromatic harmonicas only)
mod-wah-tooltip = Cycle the selected note's wah-wah rate
mod-vibrato-tooltip = Cycle the selected note's vibrato rate
mod-delete-tooltip = Delete the selected note
editor-harmonica-toggle-tooltip = Click to switch between Diatonic and Chromatic harmonica layouts
editor-field-key-tooltip = Click to cycle through harp keys
editor-field-position-tooltip = Click to cycle through playing positions
editor-browse-tooltip = Choose a background-music audio file for this chart

# Lessons — menu, reader, results verdict
menu-lessons = Lessons
no-lessons-found = No lessons found. Add folders under assets/lessons/<unit>/<lesson>/
lesson-locked = locked
lesson-passed = Passed
lesson-start = Start Lesson
lesson-mark-done = Mark as Done
lesson-goal-accuracy = Goal: %pct%% overall accuracy
lesson-goal-technique = Goal: %pct%% accuracy on %technique% notes
lesson-goal-finish = Goal: play it through to the end
lesson-complete-banner = LESSON PASSED
lesson-failed-banner = Goal not reached — read the lesson again and retry

# Lessons — unit headings (keyed by each lesson.json's "unit" field)
lesson-unit-blowing = Unit 1 · Blowing the Harmonica
lesson-unit-rhythm = Unit 2 · Counting the Blues

# Lesson: single note
lesson-single-note-title = Playing a Single Note
lesson-single-note-body =
    The harmonica's biggest beginner hurdle: getting one clean note instead of a chord of neighbours.
    Pucker your lips as if whistling, or say the syllable "too" — the opening should be barely wider than one hole.
    Relax: the harmonica goes deep between your lips, resting on the moist inner part, not gripped by the dry edge.
    Tilt the back of the harmonica slightly upward and let your jaw drop so the air moves slow and warm, from the belly.
    In this drill, long notes on holes 4, 5 and 6 scroll toward the hit line. Breathe each one gently — volume doesn't matter, purity does.
    If you hear two notes at once, don't press harder; narrow the opening a touch and slow your breath.

# Lesson: multiple notes (chords)
lesson-multiple-notes-title = Playing Multiple Notes at Once
lesson-multiple-notes-body =
    A single note isn't the only target — some blues shots deliberately sound two or three holes together as a chord.
    Widen your embouchure to cover the holes you want and none beyond them; the same breath control that gave you one clean note now gives you a controlled group of them.
    Blow chords sit on adjacent holes: holes 1-2-3 blown together ring out a bright C major triad.
    Draw chords work the same way: holes 2-3-4 drawn together ring out a G major triad.
    In this drill, chord shots scroll toward the hit line — the game listens for every note of the chord sounding at the same instant, not one after another.
    If only part of the chord registers, you're probably not covering all the holes evenly; widen the embouchure a touch rather than blowing harder.

# Lesson: octave splits (tongue blocking)
lesson-octave-split-title = Octave Splits
lesson-octave-split-body =
    Tongue blocking lets you play two holes at once while muting the ones between them — the classic move is an octave split.
    Rest your tongue flat against the harmonica, covering the two holes in the middle, and let air pass only through the hole on each side.
    Holes 1 and 4 blown together give you C4 and C5 — the same note, an octave apart. Holes 2 and 5, and holes 3 and 6, work the same way.
    In this drill, both holes of each split must ring out together, exactly like a chord — the tongue-blocking technique itself can't be verified by the mic, but the octave it produces can.
    If you only hear one note, check that your tongue fully covers the middle holes rather than resting off to one side.

# Lesson: hand shape / wah
lesson-hand-wah-title = Hand Shape and the Wah
lesson-hand-wah-body =
    Your hands are the harmonica's tone control. Cup them around the back of the harp to make a sealed air chamber, then open and close the seal to speak: "wah".
    Hold the harp between the thumb and index finger of one hand, and seal the other hand around the back like a clamshell.
    Closed cup = dark, muffled tone. Open cup = bright and loud. Opening the cup rhythmically while a note sounds makes the classic wah-wah.
    In this drill, hold each note steadily and open-close your cup about twice per second — the game listens for that pulse in your sound.
    Keep the breath constant; only the hands move. If nothing registers, tighten the cup seal — most of the effect lives in the last centimetre of closure.

# Lesson: reading the 12-bar grid
lesson-twelve-bar-title = Reading the 12-Bar Blues Grid
lesson-twelve-bar-body =
    Nearly every blues song follows the same 12-bar cycle — learn to read it once and you can follow along with any blues jam on the planet.
    Each cell in the grid is one bar of four beats. The Roman numerals name the chords: I is the home chord, IV the middle voyage, V the turnaround tension.
    The classic layout: four bars of I, two bars of IV, two bars of I, one bar of V, one of IV, and two final bars of I (the last often swaps to V to launch the next chorus — the "turnaround").
    Count it out loud: "ONE two three four, TWO two three four..." — twelve bars, then the cycle repeats.
    You'll see this grid live in Jam Session, where the current bar lights up as the backing plays. Open a Jam Session afterwards and just watch a few cycles go by, counting along, before you play a single note.
