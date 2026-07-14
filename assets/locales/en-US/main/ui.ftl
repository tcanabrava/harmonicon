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
menu-tutorial = Tutorial
menu-quit = Quit

# Play menu
play-song = Play Song
jam-session = Jam Session
jam-generate = Generate Jam
bending-trainer = Bending Trainer

# Mode select
select-mode = Select Mode
play-2d = Play 2D
play-3d = Play 3D

# Generate Jam (synthesized backing, no song required)
jam-generate-title = Generate a Jam Backing
jam-generate-start = Start Jam

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
lesson-goal-scale-adherence = Goal: %pct%% of notes in-scale or better
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

# Lesson: tongue blocking (instructional)
lesson-tongue-blocking-title = Tongue Blocking
lesson-tongue-blocking-body =
    So far you've shaped single notes with your lips (puckering) — tongue blocking is the other classic embouchure: cover several holes with your mouth, then rest your tongue flat against the harmonica to block out all but one.
    Lift the tongue off a hole and it sounds on its own, exactly like a puckered single note — the microphone genuinely can't tell the two techniques apart, so this lesson can't verify which one you're using.
    What tongue blocking unlocks that puckering can't: pull the tongue away from two side holes at once (blocking only the ones in between) and you get an octave split — two notes, an octave apart, ringing together.
    It also lets you slap the tongue on and off a hole rhythmically for a percussive "chukka-chukka" pulse, and switch corners of your mouth mid-phrase without losing your air seal.
    Try the octave-split drill next — it's the concrete, scoreable payoff of this technique: the game can hear whether both notes of the split actually sound together, even though it can't hear tongue blocking itself.

# Lesson: octave splits (tongue blocking)
lesson-octave-split-title = Octave Splits
lesson-octave-split-body =
    Tongue blocking lets you play two holes at once while muting the ones between them — the classic move is an octave split.
    Rest your tongue flat against the harmonica, covering the two holes in the middle, and let air pass only through the hole on each side.
    Holes 1 and 4 blown together give you C4 and C5 — the same note, an octave apart. Holes 2 and 5, and holes 3 and 6, work the same way.
    In this drill, both holes of each split must ring out together, exactly like a chord — the tongue-blocking technique itself can't be verified by the mic, but the octave it produces can.
    If you only hear one note, check that your tongue fully covers the middle holes rather than resting off to one side.

# Lesson: slides
lesson-slides-title = Slides
lesson-slides-body =
    Two different techniques share the name "slide" on harmonica — this drill covers both.
    The first is a physical slide: move the harmonica sideways across your embouchure from one hole to the next, keeping the seal unbroken instead of stopping and re-starting your breath for each note. In this drill, slide smoothly through holes 4-5-6 blow — the game hears three ordinary notes, but the technique is in how you connect them, not just in playing them correctly.
    The second is a bend release: attack a note already bent down, then let it glide back up to its natural pitch — a classic blues cry. In this drill, bend the 2 draw down a half step and hold it, then release smoothly up to the natural note; the game validates the bent pitch at the moment you strike it.
    Keep your air steady through both — the slide should sound like one continuous breath, not a series of separate attacks.

# Lesson: hand shape / wah
lesson-hand-wah-title = Hand Shape and the Wah
lesson-hand-wah-body =
    Your hands are the harmonica's tone control. Cup them around the back of the harp to make a sealed air chamber, then open and close the seal to speak: "wah".
    Hold the harp between the thumb and index finger of one hand, and seal the other hand around the back like a clamshell.
    Closed cup = dark, muffled tone. Open cup = bright and loud. Opening the cup rhythmically while a note sounds makes the classic wah-wah.
    In this drill, hold each note steadily and open-close your cup about twice per second — the game listens for that pulse in your sound.
    Keep the breath constant; only the hands move. If nothing registers, tighten the cup seal — most of the effect lives in the last centimetre of closure.

# Lesson: call and response
lesson-call-response-title = Call and Response
lesson-call-response-body =
    This is call-and-response: the game plays a short phrase, then it's your turn to play it back.
    Listen for the synthesized demo — a run of one, two, then three notes — then echo exactly what you heard, in your own time; the game freezes and waits for you, however long you need.
    There's no rush and no clock ticking against you here: only the pitch matters, not the timing.
    If you play the wrong note, nothing bad happens — the game just keeps waiting until you get it, so take another listen in your head and try again.
    This is the same "hear it, play it" skill you'll use jamming with other musicians: someone plays a lick, you answer it.

# Lesson: improvisation
lesson-improvisation-title = Improvising Over the Blues
lesson-improvisation-body =
    Now put it together: the 12-bar form, the blues scale, and your own choices, played live over a real jam.
    This lesson opens an ordinary Jam Session — the 12-bar chord grid and your harmonica's hole map recolor live as you play: gold means you just played a tone of the chord sounding right now, green means you're safely in the blues scale, amber means you've stepped outside it.
    This is 2nd position: your C harmonica plays in the key of G, the classic blues cross-harp setup — draw 2 is your home note.
    There's no fixed melody to hit; play whatever you like over the chords and let your ear follow the color of the hole map.
    When you feel ready to stop, open the pause menu and press "Finish Lesson" — the game tallies how many of your notes landed in-scale or on a chord tone and judges the drill from that.
    Aim for mostly green and gold; a stray amber note here and there is normal, even expressive — just don't live there.

# Lesson: reading the 12-bar grid
lesson-twelve-bar-title = Reading the 12-Bar Blues Grid
lesson-twelve-bar-body =
    Nearly every blues song follows the same 12-bar cycle — learn to read it once and you can follow along with any blues jam on the planet.
    Each cell in the grid is one bar of four beats. The Roman numerals name the chords: I is the home chord, IV the middle voyage, V the turnaround tension.
    The classic layout: four bars of I, two bars of IV, two bars of I, one bar of V, one of IV, and two final bars of I (the last often swaps to V to launch the next chorus — the "turnaround").
    Count it out loud: "ONE two three four, TWO two three four..." — twelve bars, then the cycle repeats.
    You'll see this grid live in Jam Session, where the current bar lights up as the backing plays. Open a Jam Session afterwards and just watch a few cycles go by, counting along, before you play a single note.

# Lesson: using your feet
lesson-using-your-feet-title = Using Your Feet
lesson-using-your-feet-body =
    Great time doesn't come from watching a screen — it comes from your body. Tap your foot on every beat, and let that physical pulse guide your playing instead of chasing the notes as they scroll by.
    Before you start, count "1, 2, 3, 4" out loud a few times at the drill's tempo, tapping your foot on each number, until it feels automatic rather than counted.
    In this drill, a steady quarter-note pulse scrolls by on hole 4 — keep your foot tapping the whole time, even between notes, and let each blow/draw land exactly on a tap.
    The timing window here is tighter than other drills on purpose: this lesson is entirely about landing on the beat, not about pitch or technique.
    If you're consistently early or late, don't watch the highway — close your eyes and just follow your foot.

# Gameplay — countdown, legend, harmonica overlay hints
gameplay-get-ready = GET READY
gameplay-legend-blow = ■ BLOW
gameplay-legend-draw = ■ DRAW
harmonica-overlay-hint-view = Harmonica  ·  lights up as you play
harmonica-overlay-hint-select = Harmonica  ·  click a note to select it

# Metronome overlay
metronome-click-off = click: off
metronome-click-on = click: on

# Bending Trainer
bending-drill-off = Drill: off
bending-drill-on = Drill: on · streak %streak%
bending-hint = Esc to go back  ·  M mutes the click  ·  feel toggles straight/shuffle
bending-no-note-for-technique = This hole has no note for that technique.

# Jam Session
jam-loop-off = Loop: off
jam-loop-on = Loop: on
jam-hole-map-hint = Your harmonica  ·  gold = chord tone right now  ·  green = blues-scale note  ·  top blow / bottom draw

# Results screen
results-song-complete = SONG COMPLETE
results-by-technique = By technique
results-new-best = ★ NEW BEST! ★

# Latency calibration
calibration-title = Latency Calibration
calibration-mean-offset-placeholder = Mean offset: —
calibration-mean-offset = Mean offset: %sign%%ms%ms
calibration-suggested-placeholder = Current: —   →   Suggested: —
calibration-suggested = Current: %current%ms   →   Suggested: %suggested%ms

# Options
options-input-lag = Input lag

# Guided tutorial tour (menu::tutorial)
tutorial-step = Step %n% of %total%
tutorial-skip = Skip Tutorial
tutorial-title-main = Main Menu
tutorial-body-main = Your home base — jump into a song, browse lessons, or open Options from here.
tutorial-title-play = Play
tutorial-body-play = Pick a real song, start a free jam, or practice bends — choose how you want to play.
tutorial-title-mode-select = Select Mode
tutorial-body-mode-select = Choose 2D (a scrolling note highway) or 3D (a harmonica model you play along with).
tutorial-title-options = Options
tutorial-body-options = Volume, note style, harmonica model, and microphone calibration all live here.
tutorial-title-theme = Theme
tutorial-body-theme = Pick a visual theme for the menus — swaps backgrounds and button style.
tutorial-title-lessons = Lessons
tutorial-body-lessons = A guided curriculum: single notes, chords, bends, and improvising over the blues.
tutorial-title-jam-generate = Generate Jam
tutorial-body-jam-generate = Spin up an instant backing track in any key and tempo — no song required.
