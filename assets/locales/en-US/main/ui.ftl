# Harmonicon — English (en-US) UI strings.
#
# Fluent reference: https://projectfluent.org/fluent/guide/
# Keys are kebab-case and grouped by the screen that uses them. Add new keys
# here first, then mirror them into every other locale under assets/locales/.

app-title = Harmonicon

# Main menu
menu-play = Play
menu-options = Options
menu-help = Help / About
menu-credits = Credits
menu-tutorial = Tutorial
menu-quit = Quit

# Play menu
play-song = Play Song
menu-create-song = Create Song
jam-session = Jam Session
bending-trainer = Bending Trainer

# Jam Session submenu
jam-session-pick-song = Pick a Song
jam-generate = Generate Jam

# Help / About menu
help-about-title = Help / About
help-documentation = Documentation
help-docs-not-found = Documentation isn't built locally yet — run `mdbook build` in docs/book/.
menu-about = About
about-title = About Harmonicon
about-body = Harmonicon is a rhythm game for diatonic and chromatic harmonica: play a real harmonica into your microphone and it's scored in real time against a scrolling chart, built to teach blues and jazz harmonica through play.
about-version = Version { $version }

# Mode select
select-mode = Select Mode
play-2d = Play 2D
play-3d = Play 3D

# Generate Jam (synthesized backing, no song required)
jam-generate-title = Generate a Jam Backing
jam-generate-start = Start Jam
jam-generate-key = Key: %key%
jam-generate-tempo = Tempo: %bpm%
jam-generate-progression = Progression: %progression%
jam-generate-position = Position: %position%

# Credits
credits-back-to-menu = Back to Menu

# Song / artist selection
select-artist = Select Artist
select-song = Select Song
no-songs-found = No songs found. Add folders under assets/songs/<artist>/<song>/

# Options
options-title = Options
options-language = Language
options-adaptive-difficulty = Adaptive Difficulty
options-adaptive-difficulty-on = Adaptive Difficulty: on
options-adaptive-difficulty-off = Adaptive Difficulty: off
options-fullscreen = Fullscreen
options-fullscreen-on = Fullscreen: on
options-fullscreen-off = Fullscreen: off
options-pitch-detect = Pitch detect
options-microphone = Microphone
options-note-labels-button = Note labels
options-note-labels-arrows = Note labels: arrows
options-note-labels-numbers = Note labels: numbers
options-calibrate-input-lag = Calibrate input lag
theme-back-to-options = ← Back to Options

# Shared
back = ← Back

# Song Editor 2 — transport & mod-panel buttons
editor-mode-edit = ✎ Edit
editor-mode-record = ⏺ Record
editor-mode-play = 🎵 Play
editor-mode-expected = ✓ Draw correct notes
editor-lock = 🔒 Lock
editor-play = ▶ Play
editor-pause = ⏸ Pause
editor-stop = ■ Stop
editor-practice = 🎤 Practice
editor-finish = ⏹ Finish
editor-save = 💾 Save
editor-load = 📂 Load
editor-browse = 📂 Browse
editor-import-midi = 🎹 Import MIDI
mod-blow = Blow
mod-draw = Draw
mod-bend = Bend
mod-overblow = Overblow
mod-overdraw = Overdraw
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-delete = Delete
editor-tool-select = ✂ ⟕ Select
editor-tool-erase = ✂ Erase
editor-tool-remove = ✂ Remove
editor-tool-tempo = ♩ Tempo

# Song Editor 2 — meta-form field labels
editor-field-tempo = Music Tempo
editor-field-key = Harp Key
editor-field-position = Position
editor-field-harmonica = Harmonica
editor-field-music = Background Music
editor-field-name = Name
editor-field-author = Author
editor-field-midi-track = MIDI Track
editor-field-scale = Scale
editor-harmonica-diatonic = ‹ Diatonic (10 holes) ›
editor-harmonica-chromatic = ‹ Chromatic (12 holes) ›
editor-field-content-kind = Recording
editor-content-kind-song = ‹ Record Song ›
editor-content-kind-lesson = ‹ Record Lesson ›

# Song Editor 2 — color legend (third meta-form column)
editor-legend-notes = Note colors (grid)
editor-legend-normal = Normal blow/draw note
editor-legend-bend = Bend (deeper bend = redder)
editor-legend-overblow = Overblow
editor-legend-overdraw = Overdraw
editor-legend-slide = Slide (chromatic only)
editor-legend-out-of-scale = Red tint = outside the song's scale
editor-legend-selected = Gold border = selected note
editor-legend-blow = Blow
editor-legend-draw = Draw
editor-legend-dragging = While dragging a note
editor-legend-drag-ok = Valid drop position
editor-legend-drag-bad = Invalid (overlap or wrong technique)
editor-legend-elsewhere = Elsewhere on screen
editor-legend-tempo-marker = Tempo-change marker (grid header)
editor-legend-split-point = Select tool: placed split point
editor-legend-range-preview = Select tool: range preview
editor-legend-active-button = Currently active mode/tool button
editor-legend-scrollbar-blow = Scrollbar minimap: blow note
editor-legend-scrollbar-draw = Scrollbar minimap: draw note
editor-legend-scrollbar-note = Note: this blue/orange means blow/draw here — a different meaning than the note colors above, which encode technique instead.

# Song Editor 2 — lesson-only meta-form fields (shown while "Record Lesson"
# is active)
editor-lesson-details-header = Lesson Details
editor-field-lesson-id = Lesson ID
editor-field-lesson-unit = Unit
editor-field-lesson-explanation = Explanation
editor-field-lesson-prerequisites = Prerequisites
editor-field-lesson-pass-criteria = Pass Criteria
editor-field-lesson-threshold = Threshold
editor-field-lesson-technique = Technique
editor-field-lesson-progression = Progression

# Song Editor 2 — file-dialog titles
dialog-save-chart = Save chart
dialog-load-chart = Load chart
dialog-save-lesson = Save lesson
dialog-load-lesson = Load lesson
dialog-select-music = Select background music
dialog-select-midi = Select MIDI file
dialog-file-name = File name:
dialog-cancel-esc = Cancel  (Esc)

# Song Editor 2 — drag validation messages
drag-denied-bend = This hole does not support this bend depth
drag-denied-overblow = Overblow is only available on holes 1–6
drag-denied-overdraw = Overdraw is only available on holes 7–10
drag-denied-overlap = Another note is already here

# Song Editor 2 — Erase/Remove timeline tool confirmation
editor-confirm-erase = Erase bar %from% to bar %to%? Every note in that range will be deleted — the rest of the song stays exactly where it is.
editor-confirm-remove = Remove bar %from% to bar %to%? Every note in that range will be deleted, and everything after it will shift earlier to close the gap.

# Song Editor 2 — practice mode feedback
practice-no-music = No background music set — play along with the chart!
practice-prompt = ▶ Play %note%…
practice-wrong-note = ▶ %got% → need %expected%
practice-hit-perfect = ✓ PERFECT  %note%  +%pts% pts
practice-hit-good = ✓ GOOD  %note%  +%pts% pts
practice-missed = ✗ Missed %note%
practice-done = Done — %hits%/%total% notes  ·  %score% pts
editor-record-status = ⏺ Recording — %count% notes captured

# Song Editor 2 — button tooltips
editor-back-tooltip = Leave the editor and return to the main menu
editor-mode-edit-tooltip = Switch to Edit mode — place, move, and edit notes on the grid
editor-mode-record-tooltip = Switch to Record mode — record notes from your harmonica onto the grid
editor-mode-play-tooltip = Switch to Play mode — play back or practice the chart
editor-mode-expected-tooltip = Dev builds only: mark the correct notes on top of a recorded take, for the note-detection benchmark (note_bench)
editor-lock-tooltip = Lock the grid to prevent accidental edits while reviewing
editor-save-tooltip = Save this chart to a .harpchart file
editor-load-tooltip = Load a chart from a .harpchart file
editor-play-tooltip = Start or resume playback of the chart
editor-pause-tooltip = Pause playback in place
editor-stop-tooltip = Stop playback and reset the playhead to the start
editor-practice-tooltip = Practice mode — play along on your harmonica with live feedback
editor-record-play-tooltip = Start recording from the current position — or resume a paused take
editor-record-stop-tooltip = End the take — the playhead stays where it stopped
editor-finish-tooltip = Finish the take and rewind to the beginning — recording again replaces notes you play over
editor-record-detect-label = Detect
editor-debug-recording-button = Debug Recording
editor-debug-recording-tooltip = Dev builds only: also record the take's raw microphone audio to assets/debug_songs/<song>/ on Save, for diagnosing pitch-detection issues later
editor-debug-recording-erase = Erase Recording
editor-debug-recording-erase-tooltip = Discard the captured raw audio so the next take starts fresh
editor-debug-recording-off = Off
editor-debug-recording-armed = Armed — press Play to record
editor-debug-recording-status = Recording — %secs%s captured
mod-blow-tooltip = Set the selected note to a blow (exhale) note
mod-draw-tooltip = Set the selected note to a draw (inhale) note
mod-bend-tooltip = Cycle the selected note's bend depth: none → half step → whole step → step and a half
mod-overblow-tooltip = Set the selected note to an overblow (advanced blow technique, diatonic only)
mod-overdraw-tooltip = Set the selected note to an overdraw (advanced draw technique, diatonic only)
mod-slide-tooltip = Set the selected note to use the slide button (chromatic harmonicas only)
mod-wah-tooltip = Cycle the selected note's wah-wah rate
mod-vibrato-tooltip = Cycle the selected note's vibrato rate
mod-delete-tooltip = Delete the selected note
editor-tool-select-tooltip = Click a point on the timeline then click a side (or click-drag a range)
editor-tool-erase-tooltip = Remove all notes in the current selection.
editor-tool-remove-tooltip = Erase the selection, and shift everything after it earlier, closing the gap
editor-tool-tempo-tooltip = Click the ruler to add a tempo change there, or click an existing one to remove it
editor-harmonica-toggle-tooltip = Click to switch between Diatonic and Chromatic harmonica layouts
editor-content-kind-toggle-tooltip = Click to switch between authoring a plain song and a curriculum lesson
editor-field-key-tooltip = Click to cycle through harp keys
editor-field-position-tooltip = Click to cycle through playing positions
editor-lesson-form-tooltip = Curriculum fields for lesson.json — only used while "Record Lesson" is active
editor-lesson-details-toggle-tooltip = Click to show or hide the lesson curriculum fields
editor-field-lesson-pass-criteria-tooltip = Click to cycle how this lesson is judged — None, Accuracy, Technique, Scale Adherence, Chord-Tone Adherence, Phrase Discipline
editor-field-lesson-technique-tooltip = Click to cycle which technique bucket is judged — only used when Pass Criteria is Technique
editor-field-lesson-progression-tooltip = Click to cycle the backing progression seeded for a jam-based lesson — None, Standard, Quick-Change, Minor
editor-browse-tooltip = Choose a background-music audio file for this chart
editor-import-midi-tooltip = Load a MIDI file and pick a track to drop onto the note grid — Save then writes a backing track from its other tracks
editor-silence-track-label = Silence
editor-silence-track-tooltip = The gap, in seconds, between each pair of consecutive notes

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
lesson-goal-chord-tone-adherence = Goal: %pct%% of notes as chord tones
lesson-goal-phrase-discipline = Goal: %pct%% of notes played outside a rest — leave space
lesson-complete-banner = LESSON PASSED
lesson-failed-banner = Goal not reached — read the lesson again and retry

# Lessons — unit headings (keyed by each lesson.json's "unit" field)
lesson-unit-blowing = Unit 1 · Blowing the Harmonica
lesson-unit-rhythm = Unit 2 · Counting the Blues
lesson-unit-blues = Unit 3 · Blues Vocabulary

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

# Lesson: breathing
lesson-breathing-title = Breathing and Long Tones
lesson-breathing-body =
    Long, steady tones are the foundation everything else builds on — before bends, vibrato or fast licks, your air needs to be calm and controlled.
    Breathe from your diaphragm, not your chest: let your belly expand as you inhale, and keep your shoulders relaxed and still.
    In this drill, holes 1 through 4 (both blow and draw) hold for three to four beats each — breathe through the harp, don't push.
    A wavering or leaky tone won't score as clean; a steady, unwavering one will, even at a quiet volume.
    If you run out of air mid-note, you're using more than you need — back off and let the harmonica do less work for more sound.

# Lesson: first bend
lesson-first-bend-title = Your First Bend: 4 Draw
lesson-first-bend-body =
    The 4-draw half-step bend is the classic first bend every harmonica player learns — lower your tongue and jaw slightly while drawing, as if saying "eee" sliding to "ohh".
    Don't clench your throat; the bend comes from shaping the inside of your mouth, not from squeezing harder.
    In this drill, plain 4 draw alternates with a bent 4 draw — listen for the pitch dropping a half step lower each time you bend.
    Play with the Bending Trainer (from the Play menu) if you want to hear the target pitch and check your intonation before coming back here.
    A flat, wandering bend still counts if it lands close enough — precision comes with practice, so don't chase perfection on day one.

# Lesson: deep bends
lesson-deep-bends-title = Deep Bends: 2 and 3 Draw
lesson-deep-bends-body =
    Holes 2 and 3 draw are where 2nd-position blues really lives — both can bend down further than hole 4, a half step and a full whole step.
    The deeper the bend, the further back your tongue and jaw need to move — think the vowel sliding from "ee" through "oh" to "oo".
    In this drill, 2 draw bends a half step then a whole step, then 3 draw does the same — listen for two distinct pitches below the natural note on each hole.
    These are the two most expressive notes on the whole harmonica — the "blue" notes that give the instrument its voice.
    If the whole-step bend won't drop far enough, don't force it with pressure — relax your throat further instead; tension chokes the bend rather than deepening it.

# Lesson: vibrato
lesson-vibrato-title = Vibrato
lesson-vibrato-body =
    Vibrato adds a subtle wobble to a held note — a little movement in pitch or volume that makes a long tone feel alive instead of static.
    The classic source is your diaphragm: a gentle pulsing "huh-huh-huh" in your breath, the same muscle you used in the breathing drill.
    In this drill, hold each note steady, then let a slow pulse (about four or five times a second) ripple through it — the game listens for that oscillation.
    Too fast sounds like a shiver; too slow just sounds like separate notes. Aim for a smooth, even wave.
    If nothing registers, exaggerate the pulse more than feels natural at first — you can always dial it back once the mic confirms it's actually there.

# Lesson: articulation
lesson-articulation-title = Articulation: Ta-Ka Tonguing
lesson-articulation-body =
    Tonguing is how you separate notes cleanly without moving your breath or embouchure — say "ta" or "ka" with your tongue on each new note, like tapping a light switch.
    "Ta-ka" alternates the front and back of the tongue, letting you articulate fast repeated notes without tiring your air.
    In this drill, the same hole repeats in steady eighth notes — the game can't hear your tongue directly, but a slurred, un-tongued run of notes only scores its very first onset. Re-articulating each one is what makes the rest actually count.
    Start slow and exaggerated; speed comes later, clarity first.
    If your notes blur into one long tone on the highway, you're not fully stopping the air between them — a firmer tongue tap will fix it.

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

# Lesson: counting four
lesson-counting-four-title = Counting Four
lesson-counting-four-body =
    Every rhythm skill from here on builds on one habit: counting the beat out loud, or at least in your head, while you play.
    Count "1, 2, 3, 4" steadily with the metronome before you start, and keep counting once the notes begin — don't let the counting stop just because you're playing.
    In this drill, a note lands on every beat, then only on beats 1 and 3, then only on beat 1 — the gaps get wider, but your internal count should never skip.
    If you lose the beat, don't guess — stop, restart the count from 1, and come back in on the next downbeat.
    This is the single most useful habit in this whole curriculum: everything from the 12-bar form to the turnaround depends on always knowing exactly where beat 1 is.

# Lesson: bar counting
lesson-bar-counting-title = Counting the Bars
lesson-bar-counting-body =
    Now count bars instead of beats: this drill walks the full 12-bar form, one root note on beat 1 of each bar, so you can feel the changes arrive without needing to see them.
    This is 2nd position: your C harmonica plays in the key of G, so 2 draw is the I chord's root, 4 blow is the IV chord's root, and 4 draw is the V chord's root.
    Watch the 12-bar grid overlay light up as each bar plays — match what you hear and play to what you see, then try counting along with your eyes closed.
    The pattern is four bars of I, two of IV, two of I, one of V, one of IV, one of I, and one of V — the same shape you read about in "Reading the 12-Bar Blues Grid".
    If you land on the wrong root, you likely lost count somewhere in the middle — the fix is always the same: stop, recount from bar 1 on the next chorus.

# Lesson: the turnaround
lesson-turnaround-title = The Turnaround
lesson-turnaround-body =
    The turnaround is the last two bars of the 12-bar form — the moment the music tips back toward the top of the next chorus, and the part every blues player has to feel coming.
    This drill rests through almost the entire form on purpose: there is nothing to play until bar 12, so the only way to land it is to keep counting silently the whole way through.
    When bar 12 arrives, play the V chord's root; then, right at the top of the next chorus, play the I chord's root — that's the turnaround resolving home.
    If you play into the silence before bar 12, you lost count somewhere earlier — there's no note to chase there, only the beat to keep.
    This is the same landing you'll need to hear in real jams: the turnaround is often the one moment where a whole band lines back up together.

# Lesson: shuffle feel
lesson-shuffle-feel-title = Shuffle Feel
lesson-shuffle-feel-body =
    Most blues doesn't sit on straight, even eighth notes — it swings, with a long-short "shuffle" bounce instead.
    Say "huh-DUH, huh-DUH" to feel the ratio: the first half of each pair is about twice as long as the second.
    This chart declares a shuffle feel, so the metronome click swings along with the notes — listen to the click, not just the notes, to lock onto the bounce.
    In this drill, long-short pairs alternate blow and draw on hole 4 — land the long note squarely on the beat and let the short note bounce off it.
    If your pairs come out even instead of swung, you're probably still counting straight eighths in your head — try counting the shuffle as a triplet, holding the first two beats together.

# Lesson: train chug
lesson-train-chug-title = Train: The Chug
lesson-train-chug-body =
    The chug is the classic harmonica train sound — and secretly a rhythm and breath-control drill in disguise.
    Alternate a blow chord and a draw chord on holes 1-2-3, steady and even, like a slow locomotive building up steam.
    Breathe the rhythm rather than tonguing it: let your breath itself go "huff... puff... huff... puff", not a tongue tapping on and off.
    In this drill, the chord alternates in steady eighth notes at a slow, patient tempo — every note of each chord needs to sound together for it to count.
    If only part of a chord registers, widen your embouchure evenly across all three holes rather than pressing harder on one side.

# Lesson: train rolling
lesson-train-rolling-title = Train: Rolling
lesson-train-rolling-body =
    Now the train leaves the station: the same chug you just learned, but speeding up gradually as it gets rolling.
    Don't chase the speed — let it build naturally, the same way a real train doesn't jump straight to full speed.
    This chart is the first one in the curriculum built on a tempo map instead of a fixed tempo — the notes are positioned by tick, and the backing genuinely accelerates under you.
    Keep breathing the huff-puff pattern from the previous lesson; only the tempo changes, not the shape of your breath.
    If you fall behind as it speeds up, that's normal on your first few tries — the goal is staying loose, not rigid, as the tempo shifts.

# Lesson: train whistle
lesson-train-whistle-title = Train: The Whistle
lesson-train-whistle-body =
    Every train chug needs a whistle — a long, wailing two-note chord that cuts through the chugging rhythm.
    The whistle sits on holes 4 and 5 draw together, held long, with a wah worked into it — the same hand-cupping technique from the wah lesson.
    In this drill, chug choruses alternate with a held whistle chord — keep the chug steady, then open into the whistle and let your hand do the "wah" while you hold the note.
    The whistle needs both the chord (two notes sounding together) and the wah pulse at once — if one drops out, check that you're holding both holes evenly while your hand keeps moving.
    This combines everything from the chugging drills with the hand-wah technique — a good sign you're ready to bring both into a real jam.

# Lesson: blues scale
lesson-blues-scale-title = The Blues Scale
lesson-blues-scale-body =
    Seven notes, up and down: 2 draw, 3 draw bent, 4 blow, 4 draw bent, 4 draw, 5 draw, 6 blow.
    This is the same 2nd-position blues scale every lick in this unit — and most blues harmonica playing — draws from.
    You already have both bends from the deep-bends drill; this lesson is about stringing them into one shape you can play without thinking.
    Play it slowly at first, listening for how the bent notes sit between the natural ones rather than replacing them.
    Once this scale feels familiar under your fingers, everything else in this unit is just this same handful of notes rearranged.

# Lesson: first licks
lesson-first-licks-title = First Licks
lesson-first-licks-body =
    Three short phrases, three notes each, all drawn from the blues scale you just learned — no bends yet.
    Each one plays as a demo, then waits for you to echo it back, exactly like the call-and-response drill.
    These aren't just exercises — they're real blues phrases, the kind of thing you'll reach for instinctively once they're under your fingers.
    Take as long as you need on each echo; the game waits for you, so there's no rush to get there.
    Once you can play all three from memory, try mixing them into a jam session and see how they feel over the changes.

# Lesson: bent licks
lesson-bent-licks-title = Bent Licks
lesson-bent-licks-body =
    Now the licks get their voice: three phrases built around the 3 draw and 4 draw bends, the "crying" notes of the blues scale.
    Each one plays as a demo, then waits for you to echo it — same call-and-response pattern as the last lesson, but every phrase leans on a bend.
    Listen for the difference between a clean bend and a wavering one; a confident, held bend is what gives these licks their character.
    If a phrase feels out of reach, go back to the deep-bends drill for a few minutes and come back — the bend itself, not the phrase, is usually the sticking point.
    These are the same crying notes you'll hear in almost every blues harmonica solo — get comfortable with them here and they'll show up everywhere.

# Lesson: licks over the changes
lesson-licks-over-changes-title = Licks Over the Changes
lesson-licks-over-changes-body =
    A full 12-bar chorus, but instead of just roots or a scale run, each chord gets its own short lick: one shape over the I chord, another over the IV, another over the V, and the turnaround to close it out.
    This is the bar-counting drill and your new licks combined — you need to know where you are in the form and have the right phrase ready for it.
    The phrase overlay marks each 4-bar line so you can see the form's shape while you play.
    If you lose the thread, fall back on the blues scale you already know rather than freezing — landing something over the right chord beats landing nothing at all.
    Play this one through a few times until the licks start to feel like they belong to the chords they sit over, not just notes you're reciting in order.

# Lesson: chord-tone improvisation
lesson-chord-tone-improv-title = Chord-Tone Improvisation
lesson-chord-tone-improv-body =
    The improvisation drill judged you on staying in the blues scale. This one raises the bar: land specifically on a chord tone as each chord changes, not just anywhere safe in the scale.
    It opens the same kind of open Jam Session — the hole map recolors gold for a chord tone, green for in-scale, amber for outside it — but this time gold is the target, not just a nice surprise.
    Try anticipating the change a beat early: know the IV chord is coming and have your target note ready before it arrives, rather than reacting after the fact.
    There's still no fixed melody — play whatever you like, just make more of it land gold than the last drill asked for.
    When you feel ready to stop, open the pause menu and press "Finish Lesson" to have the game tally your chord-tone fraction.

# Lesson: minor blues improvisation
lesson-minor-blues-improv-title = Minor Blues
lesson-minor-blues-improv-body =
    Same open jam, same C harmonica, but the backing progression shifts to a minor blues — the flatted 3rd is home now instead of just a passing color tone.
    This changes what "in the scale" and "on a chord tone" mean under your fingers, even though you haven't touched a different harmonica or position.
    Lean into the darker, more mournful sound the minor progression brings out — it's a different mood from the major blues you've been playing, not a mistake to correct.
    The hole map still recolors live exactly like the other jam lessons; trust the color, not what you'd expect from a major blues.
    When you feel ready to stop, open the pause menu and press "Finish Lesson" — this one is judged the same way as the original improvisation drill, scale adherence against the minor blues scale.

# Lesson: question and answer
lesson-question-answer-title = Question and Answer
lesson-question-answer-body =
    This lesson isn't about what you play — it's about what you don't. Play for two bars, then actually stop for two bars, alternating through the whole form.
    Leaving real silence is the point: a phrase that gets an answer needs room for the answer, and that room only exists if you stop asking.
    It's tempting to keep noodling through the rest — resist it. The hole map and your own ears both know the difference between a rest and a held note.
    This is the same open Jam Session as the other improvisation lessons; play whatever licks or scale runs feel right in your two bars, then genuinely let go of the harmonica.
    When you feel ready to stop, open the pause menu and press "Finish Lesson" — the game judges how much of what you played landed outside those rest windows.

# Gameplay — countdown, legend, harmonica overlay hints
gameplay-get-ready = GET READY
gameplay-legend-blow = ■ BLOW
gameplay-legend-draw = ■ DRAW
harmonica-overlay-hint-view = Harmonica  ·  lights up as you play
harmonica-overlay-hint-select = Harmonica  ·  click a note to select it
gameplay-chart-info = Key: %key%  ♩ = %bpm%  %time_sig%
gameplay-chart-author = Chart: %author%
gameplay-techniques-toggle = %arrow% TECHNIQUES

# Pause menu
pause-quit-song = Quit Song
pause-finish-lesson = Finish Lesson
pause-wait-for-note-button = ⏸ Wait for Note
pause-wait-for-note-on = Wait for Note: on
pause-wait-for-note-off = Wait for Note: off
pause-speed = Speed: %pct%%
pause-adaptive-difficulty-button = Adaptive Difficulty
pause-adaptive-difficulty-on = Adaptive Difficulty: on
pause-adaptive-difficulty-off = Adaptive Difficulty: off
pause-phrase-section = Section: %name% — Learned: %pct%%
pause-phrase-no-sections = No phrases in this song
pause-drag-section-hint = Click a section on the progress bar above to select it
pause-notes-update-hint = Notes update live — resume to see them
pause-clear-loop = Clear Loop
pause-loop-off = Loop: off
pause-loop-range = Loop: %start%s–%end%s
pause-drag-loop-hint = Drag on the progress bar above to set a loop range

# Metronome overlay
metronome-click-off = click: off
metronome-click-on = click: on
metronome-feel-straight = feel: straight
metronome-feel-shuffle = feel: shuffle

# Bending Trainer
bending-drill-off = Drill: off
bending-drill-on = Drill: on · streak %streak%
bending-hint = Esc to go back  ·  M mutes the click  ·  feel toggles straight/shuffle
bending-no-note-for-technique = This hole has no note for that technique.
bending-key-label = Key: %key%
bending-listen-button = 🔊 Listen
bending-drill-button = 🎲 Drill
bending-play-it-target = Play it — target %note%
bending-in-tune = ✓ In tune  (%note%)
bending-cents-sharp = ↑ %cents% cents sharp  (target %note%)
bending-cents-flat = ↓ %cents% cents flat  (target %note%)
bending-detect-label = Detect

# Jam Session
jam-loop-button = ↻ Loop
jam-loop-off = Loop: off
jam-loop-on = Loop: on
jam-hole-map-hint = Your harmonica  ·  gold = chord tone right now  ·  green = blues-scale note  ·  top blow / bottom draw
jam-call-response-button = 🗣 Call & Response
jam-call-response-off = Call & Response: off
jam-call-response-on = Call & Response: on
jam-call-response-listen = Listen…
jam-call-response-your-turn = Your turn

# Results screen
results-song-complete = SONG COMPLETE
results-by-technique = By technique
results-new-best = ★ NEW BEST! ★
results-biggest-combo = Biggest combo
results-perfect-hits = Perfect hits
results-good-hits = Good hits
results-hits = Hits
results-delayed-hits = Delayed hits
results-misses = Misses
results-technique-normal = Normal notes
results-technique-bend = Bends
results-technique-vibrato = Vibrato
results-technique-wah = Wah
results-technique-overblow = Overblow
results-technique-overdraw = Overdraw
results-technique-slide = Slide
results-technique-clean-attack = Clean attack
results-avg-timing-offset = Avg timing offset
results-increase-latency = Increase Input lag to %ms%ms
results-decrease-latency = Decrease Input lag to %ms%ms
results-score = Score: %points%
results-best-score = Best score

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
tutorial-body-main = Your home base — head into Play, open Options, or find Help / About from here.
tutorial-title-play = Play
tutorial-body-play = Pick a real song, create one, start a jam, practice bends, or work through lessons — choose how you want to play.
tutorial-title-mode-select = Select Mode
tutorial-body-mode-select = Choose 2D (a scrolling note highway) or 3D (a harmonica model you play along with).
tutorial-title-gameplay = Playing a Song
tutorial-body-gameplay = Notes fall toward the hit line — play the right pitch on your harmonica at the right time to score them.
tutorial-title-jam-session-menu = Jam Session
tutorial-body-jam-session-menu = Pick a real song to jam over, or generate an instant backing track instead.
tutorial-title-jam-session = Jam Session
tutorial-body-jam-session = Free play: the 12-bar grid and a live hole map guide your improvising — nothing here is scored.
tutorial-title-bending-trainer = Bending Trainer
tutorial-body-bending-trainer = Practice bends in isolation: pick a target on the diagram, listen to it, then try to match it.
tutorial-title-options = Options
tutorial-body-options = Volume, note style, harmonica model, and microphone calibration all live here.
tutorial-title-theme = Theme
tutorial-body-theme = Pick a visual theme for the menus — swaps backgrounds and button style.
tutorial-title-lessons = Lessons
tutorial-body-lessons = A guided curriculum: single notes, chords, bends, and improvising over the blues.
tutorial-title-jam-generate = Generate Jam
tutorial-body-jam-generate = Spin up an instant backing track in any key and tempo — no song required.
tutorial-title-song-editor = Song Editor
tutorial-body-song-editor = Build or edit a chart by hand on this grid, then play it back or practice along with it live.
tutorial-title-help-about = Help / About
tutorial-body-help-about = Open the documentation, read about Harmonicon, retake this tour, or check the credits.
