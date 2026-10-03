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
select-mode = Play view
play-2d = Play 2D
play-3d = Play 3D

# Generate Jam (synthesized backing, no song required)
jam-generate-title = Generate a Jam Backing
jam-generate-start = Start Jam
jam-generate-preparing = Getting the band ready…
progression-standard = Standard
progression-quick-change = Quick Change
progression-minor-blues = Minor Blues
progression-jazz-blues = Jazz Blues
position-1st = 1st
position-2nd = 2nd
position-3rd = 3rd
position-4th = 4th
position-5th = 5th
position-12th = 12th
scale-1st-position = 1st Position
scale-2nd-position = 2nd Position
scale-3rd-position = 3rd Position
scale-major-scale = Major Scale
scale-minor-pentatonic = Minor Pentatonic
scale-country-scale = Country Scale
genre-blues = Blues
genre-jazz = Jazz
genre-rock = Rock
genre-reggae = Reggae
genre-country = Country
jam-generate-key = Key
jam-generate-tempo = Tempo
jam-generate-progression = Progression
jam-generate-position = Position
jam-generate-scale = Scale
jam-generate-genre = Genre
jam-generate-energy = Band energy
band-energy-low = Low
band-energy-medium = Medium
band-energy-high = High

# First-run download of lesson and song packs
sync-title = Getting lessons and songs
sync-in-progress = Harmonicon is downloading its lessons and songs. The game will start as soon as they are ready.
sync-repo-downloading = Downloading {$name}
sync-repo-failed = Could not download {$name}: {$error}
sync-retry = Retry
sync-quit = Quit

# Options → Lessons & songs (content repositories)
content-title = Lessons & songs
content-subtitle = Lessons and songs come from git repositories, which Harmonicon keeps up to date when you ask it to.
content-back-tooltip = Back to Options
content-check-updates = Check for updates
content-songs = Songs
content-lessons = Lessons
content-empty = No repositories.
content-add = Add
content-add-hint = Repository address or folder:
content-add-invalid = "{$input}" is neither a git repository address (https or ssh) nor a folder on this computer.
content-add-duplicate = That repository is already in the list.
content-trust-note = Lessons and songs are only data, never programs, but add only repositories you trust.
content-update = Update
content-download = Download
content-remove = Remove
content-confirm-update = Update {$name} to its latest version?
content-confirm-remove = Remove {$name}? Its downloaded files are deleted; your progress is kept.
content-status-downloading = Downloading
content-status-download-failed = Could not download: {$error}
content-status-not-installed = Not downloaded
content-status-unusable = Can't be used: {$reason}
content-status-local = Version {$version}, a folder on this computer
content-status-checking = Version {$version}, checking for updates
content-status-up-to-date = Version {$version}, up to date
content-status-update-available = Version {$version}, an update is available
content-status-check-failed = Version {$version}, could not check for updates: {$error}
content-status-installed = Version {$version}
sync-failed = Harmonicon could not download its lessons and songs.

# Credits
credits-back-to-menu = Back to Menu

# Song / artist selection
select-artist = Select Song
circle-of-fifths-harp-label = harp
artist-song-count-one = {$n} song
artist-song-count-many = {$n} songs
select-song = Select Song
song-search = Search
song-sort-band = Band
song-sort-difficulty = Difficulty
song-sort-name = Song name
song-sort-genre = Genre
editor-field-genre = Genre
editor-field-genre-tooltip = The song's musical genre.
no-songs-found = No songs found. Add folders under assets/songs/<artist>/<song>/

# Options
options-title = Options
options-subtitle-audio = Audio
options-theme = Theme
options-language = Language
options-adaptive-difficulty = Adaptive Difficulty
options-adaptive-difficulty-tooltip = Automatically adjusts how many of a song's charted notes you're given at once, based on how well you're doing.
options-fullscreen = Fullscreen
options-fullscreen-tooltip = Play in fullscreen instead of a window.
options-colorblind-palette = Colorblind Palette
options-colorblind-palette-tooltip = Use a fixed colorblind-safe blow/draw color pair instead of the current theme's note colors.
options-reduced-motion = Reduced Motion
options-reduced-motion-tooltip = Still the decorative motion on the highway — the pop on a hit, the flowing note tails — while notes keep scrolling as usual.
options-zoom = Zoom
options-music = Music
options-metronome = Metronome
options-zoom-tooltip = Scale the whole UI up or down.
options-zoom-label = {$percent}%
options-pitch-detect = Pitch detect
options-microphone = Microphone
options-microphone-tooltip = Which input device to capture your harmonica from.
options-mic-retry-tooltip = Try reconnecting to the microphone.
options-note-labels = Note labels
options-note-labels-tooltip = Show falling notes as hole numbers instead of blow/draw arrows.
options-music-volume-tooltip = Backing-track volume.
options-metronome-volume-tooltip = Metronome click volume.
options-theme-tooltip = Change the menu's visual theme.
options-content = Lessons & songs
options-content-tooltip = Where lessons and songs come from, and their updates.
options-calibrate-input-lag = Calibrate input lag
options-calibrate-input-lag-tooltip = Measure your setup's audio latency and apply it automatically.
options-back-tooltip = Return to the main menu.
options-button-style = Action buttons
options-button-style-tooltip = How Song Editor action buttons show icon and label.
options-button-style-icon-only = Icon only
options-button-style-text-beside-icon = Text beside icon
options-button-style-text-only = Text only
theme-back-to-options = ← Back to Options
theme-title = Theme

# Shared
back = ← Back

# Song Editor 2 — transport & mod-panel buttons
editor-back-label = Back
editor-mode-edit = Edit
editor-mode-record = Record
editor-mode-play = Play
editor-mode-expected = Draw correct notes
editor-lock = Lock
editor-undo = Undo
editor-redo = Redo
editor-delete = Delete
editor-copy = Copy
editor-paste = Paste
editor-metronome = Metronome
editor-play = Play
editor-pause = Pause
editor-stop = Stop
editor-practice = Practice
editor-finish = Finish
editor-save = Save
editor-load = Load
editor-browse = 📂 Browse
editor-import-midi = ♬ Import MIDI
mod-blow = Blow
mod-draw = Draw
mod-bend = Bend
mod-overblow = Overblow
mod-overdraw = Overdraw
mod-slide = Slide
mod-wah = Wah
mod-vibrato = Vibrato
mod-transpose-up = Transpose up
mod-transpose-down = Transpose down
mod-delete = Delete
editor-tool-select = Select
editor-tool-erase = Erase
editor-tool-remove = Remove
editor-tool-tempo = Tempo

# Song Editor 2 — meta-form field labels
editor-field-tempo = Music Tempo
editor-field-pickup = Pickup (beats)
editor-field-time-signature = Time Signature
editor-field-time-signature-tooltip = How many beats are in a bar. The lower number names a note length, so it is always 1, 2, 4, 8 or 16.
editor-field-key = Harp Key
editor-field-position = Position
editor-field-harmonica = Harmonica
editor-field-music = Background Music
editor-field-name = Name
editor-field-author = Author
editor-field-difficulty = Difficulty
editor-field-difficulty-tooltip = Click to choose easy, intermediate, advanced, or expert.
editor-field-feel = Song Feel
editor-field-feel-tooltip = Choose whether the song requests straight or shuffle metronome subdivision; default leaves the player's choice unchanged.
editor-field-source = Source
editor-field-license = License
editor-field-description = Description
editor-field-perfect-window = Perfect Window (ms)
editor-field-good-window = Good Window (ms)
editor-field-miss-window = Miss Window (ms)
editor-field-combo-enabled = Combo
editor-field-combo-base = Combo Base
editor-field-combo-step = Combo Step
editor-field-combo-max = Combo Maximum
editor-field-combo-decay = Combo Decay (ms)
editor-field-loop-type = Loop Type
editor-field-loop-repeat = Repeat Loop
editor-field-loop-start = Loop Start Phrase
editor-field-loop-end = Loop End Phrase
editor-field-midi-track = MIDI Track
editor-field-midi-track-tooltip = Which track of the imported MIDI file to place onto the grid.
editor-field-scale = Scale
editor-field-scale-tooltip = Which scale the out-of-scale red tint on the grid is measured against.
editor-field-text-tooltip = Click to edit; type a value, then click away or press Enter to confirm.
editor-harmonica-diatonic = ‹ Diatonic (10 holes) ›
editor-harmonica-paddy-richter = ‹ Paddy Richter (10 holes) ›
editor-harmonica-country-tuned = ‹ Country Tuned (10 holes) ›
editor-harmonica-natural-minor = ‹ Natural Minor (10 holes) ›
editor-harmonica-chromatic = ‹ Chromatic (12 holes) ›
editor-harmonica-chromatic-16 = ‹ Chromatic (16 holes) ›
editor-field-content-kind = Recording
editor-content-kind-song = ‹ Record Song ›
editor-content-kind-lesson = ‹ Record Lesson ›
editor-field-snap-mode = Grid Snap
editor-snap-mode-sixteenth = ‹ Straight 16ths ›
editor-snap-mode-shuffle = ‹ Shuffle (swing 8ths) ›
editor-snap-mode-triplet = ‹ Triplet 8ths ›

# Song Editor 2 — beat-ruler counting syllables. Printed between the beat
# numbers, on whichever ticks the active grid snap can land a note on: "&"
# at the half beat (straight 16ths) or the second triplet partial, "a" at
# the third. A shuffle uses "a", not "&" — it is the third partial of a
# triplet with the second left out.
editor-beat-count-and = &
editor-beat-count-a = a
editor-phrase-marker-tooltip = Phrase at tick {$tick}: {$details}
editor-phrase-editor-title = Phrase at {$position}
editor-phrase-editor-close = Close the phrase editor
editor-phrase-editor-section = Section
editor-phrase-editor-chord = Chord
editor-phrase-editor-groove = Groove
editor-phrase-editor-lyric = Lyric
editor-field-twelve-bar-tint = 12-Bar Blues Tint

# Song Editor 2 — color legend (third meta-form column)
editor-legend-toggle = Legend
editor-legend-toggle-tooltip = Show or hide the color-legend column.
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
editor-legend-repeat-marker = Repeat sign or ending
editor-legend-triplet-line = Triplet-subdivision gridline (ticks 4/8 of a beat)
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
editor-field-lesson-scale = Lesson scale
editor-field-lesson-path = Course path

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
editor-confirm-erase = Erase bar {$from} to bar {$to}? Every note in that range will be deleted — the rest of the song stays exactly where it is.
editor-confirm-remove = Remove bar {$from} to bar {$to}? Every note in that range will be deleted, and everything after it will shift earlier to close the gap.

# Song Editor 2 — practice mode feedback
practice-no-music = No background music set — play along with the chart!
practice-prompt = ▶ Play {$note}…
practice-wrong-note = ▶ {$got} → need {$expected}
practice-hit-perfect = ✓ PERFECT  {$note}  +{$pts} pts
practice-hit-good = ✓ GOOD  {$note}  +{$pts} pts
practice-missed = ✗ Missed {$note}
practice-done = Done — {$hits}/{$total} notes  ·  {$score} pts
editor-record-status = ⏺ Recording — {$count} notes captured
editor-count-in-status = ◔ Get ready — recording in {$seconds}s
editor-metronome-tooltip = Toggle the metronome click during Record/Play/Practice
editor-save-success = ✓ Saved: {$path}
editor-save-warning = ‼ Saved with warnings: {$detail}
editor-save-failed = ✗ Save failed: {$detail}
editor-load-success = ✓ Loaded: {$path}
editor-load-failed = ✗ Load failed: {$detail}
editor-midi-import-success = ✓ Imported {$count} MIDI notes on {$key} harp
editor-midi-import-warning = ‼ Imported {$count} MIDI notes on {$key} harp — {$approximated} approximated, {$mixed} mixed-breath chords, {$duplicate} duplicate-hole chords
editor-midi-import-failed = ✗ MIDI import failed: {$detail}

# Song Editor 2 — button tooltips
editor-back-tooltip = Leave the editor and return to the main menu
editor-mode-edit-tooltip = Switch to Edit mode — place, move, and edit notes on the grid
editor-mode-record-tooltip = Switch to Record mode — record notes from your harmonica onto the grid
editor-mode-play-tooltip = Switch to Play mode — play back or practice the chart
editor-mode-expected-tooltip = Dev builds only: mark the correct notes on top of a recorded take, for the note-detection benchmark (note_bench)
editor-lock-tooltip = Lock the grid to prevent accidental edits while reviewing
editor-undo-tooltip = Undo the last edit (note placement/move/delete, paste, Erase/Remove, a recording take, ...)
editor-redo-tooltip = Redo the last undone edit
editor-delete-tooltip = Delete the selected note(s)
editor-copy-tooltip = Copy the selected note(s)
editor-paste-tooltip = Paste the last copied note(s) at the start of the current view
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
editor-debug-recording-status = Recording — {$secs}s captured
mod-blow-tooltip = Set the selected note to a blow (exhale) note
mod-draw-tooltip = Set the selected note to a draw (inhale) note
mod-bend-tooltip = Cycle the selected note's bend depth: none → half step → whole step → step and a half
mod-overblow-tooltip = Set the selected note to an overblow (advanced blow technique, diatonic only)
mod-overdraw-tooltip = Set the selected note to an overdraw (advanced draw technique, diatonic only)
mod-slide-tooltip = Set the selected note to use the slide button (chromatic harmonicas only)
mod-wah-tooltip = Cycle the selected note's wah-wah rate
mod-vibrato-tooltip = Cycle the selected note's vibrato rate
mod-depth = Depth
mod-depth-tooltip = Vibrato/wah depth for the selected note, or for the next note you place. Click to step ¼ → ½ → ¾ → 1.
mod-call = Call
mod-call-tooltip = Mark the selected note's phrase as a call: the game plays it first and waits for you to answer.
mod-split = Split
mod-split-tooltip = Mark the selected note's phrase as a tongue-block split.
mod-phrase = Phrase
mod-phrase-tooltip = Open the section / chord / groove labels for the selected note's phrase.
mod-transpose-up-tooltip = Transpose the selection (or everything) up a semitone — Ctrl+↑, Ctrl+Shift+↑ for an octave
mod-transpose-down-tooltip = Transpose the selection (or everything) down a semitone — Ctrl+↓, Ctrl+Shift+↓ for an octave
editor-transposed = ✓ Transposed {$count} notes {$semitones} semitones
editor-transposed-warning = ‼ Transposed {$count} notes {$semitones} semitones — {$kept} left unchanged (unplayable or spot taken), {$mixed} mixed-breath chords, {$duplicate} duplicate-hole chords
editor-technique-skipped = ‼ {$count} of the selected notes can't take that technique and kept their own
mod-delete-tooltip = Delete the selected note
editor-tool-select-tooltip = Click a point on the timeline then click a side (or click-drag a range)
editor-tool-erase-tooltip = Remove all notes in the current selection.
editor-tool-remove-tooltip = Erase the selection, and shift everything after it earlier, closing the gap
editor-tool-tempo-tooltip = Click the ruler to add a tempo change there, or click an existing one to remove it
editor-tool-meter = Meter
editor-tool-meter-tooltip = Click the ruler to add a time-signature change at that beat; click a change to step it to the next signature, all the way round to remove it.
editor-tool-repeat = Repeat
editor-tool-repeat-tooltip = Repeat the bars selected on the ruler. Press again on the same bars to play them one more time, up to four times, then once more to remove the repeat.
editor-tool-ending = Ending
editor-tool-ending-tooltip = Make the selected bars an ending: inside a repeated passage they are played every time but the last; starting right after it, only the last time. Press again to remove it.
editor-repeat-needs-selection = Select bars on the ruler first (the Select tool).
editor-ending-needs-repeat = An ending goes inside a repeated passage, or starts right where it ends.
editor-harmonica-toggle-tooltip = Click to cycle through diatonic tunings and 12- or 16-hole chromatic layouts
editor-content-kind-toggle-tooltip = Click to switch between authoring a plain song and a curriculum lesson
editor-snap-mode-toggle-tooltip = Click to cycle which beat subdivisions a click on the grid snaps to — straight 16ths, shuffle (swung) 8ths, or straight triplet 8ths
editor-field-key-tooltip = Click to cycle through harp keys
editor-field-position-tooltip = Click to cycle through playing positions
editor-lesson-form-tooltip = Curriculum fields for lesson.json — only used while "Record Lesson" is active
editor-lesson-details-toggle-tooltip = Click to show or hide the lesson curriculum fields
editor-field-lesson-pass-criteria-tooltip = Click to cycle how this lesson is judged — None, Accuracy, Technique, Scale Adherence, Chord-Tone Adherence, Phrase Discipline
editor-field-lesson-technique-tooltip = Click to cycle which technique bucket is judged — only used when Pass Criteria is Technique
editor-field-lesson-progression-tooltip = Click to cycle the backing progression seeded for a jam-based lesson — None, Standard, Quick-Change, Minor
editor-field-lesson-scale-tooltip = Click to cycle the scale used to judge a jam-based lesson
editor-field-lesson-path-tooltip = Click to choose whether this lesson is required Core work or an Elective branch
editor-browse-tooltip = Choose a background-music audio file for this chart
editor-import-midi-tooltip = Load a MIDI file and pick a track to drop onto the note grid — Save then writes a backing track from its other tracks
editor-silence-track-label = Silence
editor-silence-track-tooltip = The gap, in seconds, between each pair of consecutive notes

# Lessons — menu, reader, results verdict
menu-lessons = Lessons
no-lessons-found = No lessons found. Add folders under assets/lessons/<unit>/<lesson>/
lesson-passed = Passed
lesson-start = Start Lesson
lesson-widget-metronome-toggle = Start / Stop
lesson-widget-tempo-decrease = − 5 BPM
lesson-widget-tempo-increase = + 5 BPM
lesson-widget-feel-toggle = Straight / Shuffle / Triplet
lesson-widget-sound-toggle = Sound On / Off
lesson-widget-key-previous = Previous key
lesson-widget-key-next = Next key
lesson-widget-bar-previous = Previous bar
lesson-widget-bar-next = Next bar
lesson-widget-bar-reset = Reset to bar 1
lesson-widget-section-previous = Previous section
lesson-widget-section-next = Next section
lesson-widget-section-reset = Reset to section 1
lesson-widget-step-previous = Previous step
lesson-widget-step-next = Next step
lesson-widget-step-reset = Reset to step 1
lesson-mark-done = Mark as Done
lesson-goal-accuracy = Goal: {$pct}% overall accuracy
lesson-goal-technique = Goal: {$pct}% accuracy on {$technique} notes
lesson-goal-finish = Goal: play it through to the end
lesson-goal-scale-adherence = Goal: {$pct}% of notes in-scale or better
lesson-goal-chord-tone-adherence = Goal: {$pct}% of notes as chord tones
lesson-goal-phrase-discipline = Goal: {$pct}% of notes played outside a rest — leave space
lesson-complete-banner = LESSON PASSED
lesson-failed-banner = Goal not reached — read the lesson again and retry

# Gameplay — countdown, legend, harmonica overlay hints
gameplay-get-ready = GET READY
gameplay-legend-blow = ■ BLOW
gameplay-legend-draw = ■ DRAW
harmonica-overlay-hint-view = Harmonica  ·  lights up as you play
harmonica-overlay-hint-select = Harmonica  ·  click a note, or focus the diagram and use the arrow keys
gameplay-chart-info = Key: {$key}  ♩ = {$bpm}  {$time_sig}
gameplay-chart-author = Chart: {$author}
gameplay-techniques-toggle = {$arrow} TECHNIQUES

# Gameplay — the judgment label at the hit line, one per scoring outcome
gameplay-judgment-perfect = PERFECT!
gameplay-judgment-good = GOOD
gameplay-judgment-early = EARLY
gameplay-judgment-late = LATE
gameplay-judgment-no-attack = MISS
gameplay-judgment-wrong-pitch = WRONG NOTE
gameplay-judgment-wrong-pitch-detail = wanted {$expected}  ·  heard {$heard}
gameplay-judgment-wrong-pitch-detail-unplaceable = wanted {$expected}
gameplay-judgment-incomplete-chord = INCOMPLETE CHORD
gameplay-judgment-technique = TECHNIQUE

# Gameplay — the wait-for-note coaching card at the hit line
gameplay-wait-play = Play {$tab}
gameplay-wait-hearing = hearing {$tab}
gameplay-wait-listening = listening…

# Gameplay — live badges for whichever practice aids are on
gameplay-badge-speed = {$pct}% speed · music off
gameplay-badge-wait = waiting for each note
gameplay-badge-loop = loop {$start}s–{$end}s

# Pause menu
pause-group-playback-aids = PLAYBACK AIDS
pause-group-phrase-practice = PHRASE PRACTICE
pause-quit-song = Quit Song
pause-paused = PAUSED
pause-resume = Resume
pause-restart = Restart
pause-learned-label = Learned:
pause-finish-lesson = Finish Lesson
pause-wait-for-note-button = ⏸ Wait for Note
pause-wait-for-note-on = Wait for Note: on
pause-wait-for-note-off = Wait for Note: off
pause-speed = Speed: {$pct}%
pause-adaptive-difficulty-button = Adaptive Difficulty
pause-adaptive-difficulty-on = Adaptive Difficulty: on
pause-adaptive-difficulty-off = Adaptive Difficulty: off
pause-phrase-section = Section: {$name} — Learned: {$pct}%
pause-phrase-no-sections = No phrases in this song
pause-drag-section-hint = Click a section on the progress bar above to select it
pause-notes-update-hint = Notes update live — resume to see them
pause-clear-loop = Clear Loop
pause-loop-off = Loop: off
pause-loop-range = Loop: {$start}s–{$end}s
pause-drag-loop-hint = Drag on the progress bar above to set a loop range

# Metronome overlay
metronome-click-off = click: off
metronome-click-on = click: on
metronome-feel-straight = feel: straight
metronome-feel-shuffle = feel: shuffle

# Bending Trainer
bending-drill-off = Drill: off
bending-drill-on = Drill: on · streak {$streak}
bending-hint = Esc to go back  ·  M mutes the click  ·  feel toggles straight/shuffle
bending-drill-explanation = Picks targets from the current scope, favouring ones you haven't tried, control less reliably, or haven't practised lately. Finish the practice shape to move on; Skip, or leaving the harp silent, moves on without counting against you.
bending-no-note-for-technique = This hole has no note for that technique.
bending-key-label = Key
bending-listen-button = 🔊 Listen
bending-listen-natural-button = 🔊 Natural
bending-listen-target-button = 🔊 Target
bending-drill-button = 🎲 Drill
bending-adv-toggle = Advanced
bending-adv-reset = Reset
bending-adv-tolerance = Tolerance
bending-adv-hold = Hold to pass
bending-adv-timeout = Attempt timeout
bending-adv-a4 = A4 reference
bending-adv-trace = Trace length
bending-adv-smoothing = Trace smoothing
bending-adv-subdivision = Pulse per beat
bending-adv-stability-heading = This attempt
bending-adv-mean = Mean: {$value}
bending-adv-spread = Spread: {$value}
bending-adv-best-hold = Best hold: {$value}
bending-adv-vibrato = Vibrato: {$value}
bending-adv-center = Measured reed: {$value}
bending-adv-clear-center = Forget measured reed
bending-setup-button = Setup
bending-setup-summary = {$key} harp · {$algo}
bending-skip-button = Skip
bending-progress-none = Not practised yet
bending-progress = {$hits} of {$attempts} controlled
bending-scope-button = Scope
bending-scope-status = Scope: {$scope}
bending-scope-first = First bends
bending-scope-all = All bends
bending-scope-blow = Blow bends
bending-scope-over = Overbends
bending-scope-custom-selected = Custom (selected cell)
bending-scope-custom-count = Custom ({$count} cells)
bending-shape-button = Practice
bending-shape-free = Free exploration
bending-shape-find-hold = Find and hold
bending-shape-bend-release = Bend and release
bending-shape-repeated = Repeated bends
bending-shape-ladder = Bend ladder
bending-shape-overbend-response = Overbend response
bending-shape-status = {$shape} · {$phase}
bending-phase-waiting = waiting
bending-phase-travel = move to target
bending-phase-holding = hold
bending-phase-returning = return to natural
bending-phase-complete = complete
bending-play-it-target = Play it — target {$note}
bending-wrong-pitch = Hearing {$note} — play the selected hole {$hole}
bending-signal-unstable = Signal unstable — hold the note steadily
bending-rail-natural = Natural
bending-rail-target = Target
bending-rail-natural-note = Natural {$note}
bending-rail-target-note = Target {$note}
bending-metric-distance = Distance {$value}
bending-metric-stability = Stability {$value}
bending-metric-hold = Hold {$value}
bending-check-natural-button = Check natural note
bending-check-natural-idle = Optional: check the natural {$note} before bending
bending-check-natural-listening = Hold the natural {$note} steadily…
bending-check-natural-ready = ✓ Natural {$note} is tracking clearly
bending-in-tune = ✓ In tune  ({$note})
bending-cents-sharp = ↑ {$cents} cents sharp  (target {$note})
bending-cents-flat = ↓ {$cents} cents flat  (target {$note})
bending-detect-label = Detect
bending-tempo-decrease = Decrease tempo
bending-tempo-increase = Increase tempo
bending-target-label = Target: Hole {$hole} · {$technique}
bending-technique-blow = Blow
bending-technique-draw = Draw
bending-technique-bend-half = ½-step bend
bending-technique-bend-whole = 1-step bend
bending-technique-bend-three-half = 1½-step bend
bending-technique-overblow = Overblow
bending-technique-overdraw = Overdraw
bending-technique-hint-blow = Blow steadily through the hole with relaxed, gentle pressure.
bending-technique-hint-draw = Draw steadily through the hole with relaxed, gentle pressure.
bending-technique-hint-draw-bend-half = Draw gently and move the back of your tongue to lower the pitch by a half step. Shape the air; do not pull harder.
bending-technique-hint-draw-bend-whole = Draw gently and move the back of your tongue further to lower the pitch by a whole step. Shape the air; do not pull harder.
bending-technique-hint-draw-bend-three-half = Draw gently and deepen the tongue position to lower the pitch by one and a half steps. Shape the air; do not pull harder.
bending-technique-hint-blow-bend-half = Blow gently and adjust the back of your tongue to lower the pitch by a half step. Shape the air; do not blow harder.
bending-technique-hint-blow-bend-whole = Blow gently and move the tongue position further to lower the pitch by a whole step. Shape the air; do not blow harder.
bending-technique-hint-blow-bend-three-half = Blow gently and deepen the tongue position to lower the pitch by one and a half steps. Shape the air; do not blow harder.
bending-technique-hint-overblow = Start with a soft blow and narrow the oral cavity until the blow reed closes and the draw reed sounds. Pucker and tongue-block embouchures can both work; avoid force.
bending-technique-hint-overdraw = Start with a soft draw and narrow the oral cavity until the draw reed closes and the blow reed sounds. Pucker and tongue-block embouchures can both work; avoid force.
bending-technique-hint-over-unsupported = Hole {$hole} does not support an overblow or overdraw on this layout.

# Jam Session
jam-loop-button = ↻ Loop
jam-loop-off = Loop: off
jam-loop-on = Loop: on
jam-end-after-chorus-button = End after this chorus
jam-keep-playing = Keep playing
jam-ending-after-chorus = Ending after this chorus…
jam-ended = Finished — Restart or Quit
jam-form-position = Chorus {$chorus} · Bar {$bar}
jam-hole-map-hint = Your harmonica  ·  gold = chord tone right now  ·  green = blues-scale note  ·  top blow / bottom draw
jam-call-response-button = ⇄ Call & Response
jam-call-response-off = Call & Response: off
jam-call-response-on = Call & Response: on
jam-call-response-listen = Listen…
jam-call-response-your-turn = Your turn
jam-call-density-button = Phrasing
jam-call-density-sparse = Phrasing: sparse
jam-call-density-conversational = Phrasing: conversational
jam-call-density-busy = Phrasing: busy
jam-adaptive-band-button = Adaptive band
jam-adaptive-band-on = Adaptive band: on
jam-adaptive-band-off = Adaptive band: off
jam-detected-blow = Hole {$hole} blow
jam-detected-draw = Hole {$hole} draw
jam-detected-none = —
jam-midi-track-mute-tooltip = Click to mute/unmute this track
jam-rhythm-guide = Rhythm Guide
jam-guides-button = Guides
jam-guides-off = Guides: hidden
jam-guides-on = Guides: shown
jam-position-label = Position: {$position}
jam-spectrogram-style-button = ↻ View
jam-spectrogram-style-bars = Bars
jam-spectrogram-style-oscilloscope = Oscilloscope

# Results screen
results-song-complete = SONG COMPLETE
results-by-technique = By technique
results-new-best = ◆ NEW BEST! ◆
results-biggest-combo = Biggest combo
results-perfect-hits = Perfect hits
results-good-hits = Good hits
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
results-increase-latency = Increase Input lag to {$ms}ms
results-decrease-latency = Decrease Input lag to {$ms}ms
results-score = Score: {$points}
results-best-score = Best score
results-accuracy-caption = accuracy
results-observation-technique = {$technique}: {$hits} of {$total} landed — that's where the next points are
results-observation-missed = {$misses} of {$total} notes never sounded — slow it down with Practice speed or Wait for Note
results-observation-late = {$pct}% of your hits came in late — anticipate the hit line, or apply the timing fix below
results-observation-early = {$pct}% of your hits came in early — let the note reach the line, or apply the timing fix below
results-observation-leaky = Only {$clean} of {$total} attacks were clean — a neighbouring hole is leaking; tighten your embouchure
results-observation-solid = Nothing stands out — this one is ready for a harder song
results-timing = Timing (mean {$ms}ms)
results-timing-early = early {$n}
results-timing-on-time = on time {$n}
results-timing-late = late {$n}
results-lesson-reached = This run: {$pct}%
results-retry = Retry
results-practice-missed = Practice missed section
results-continue = Continue

# Latency calibration
calibration-title = Latency Calibration
calibration-mic-label = Mic
calibration-instructions = Play any note on each beat — the game measures how late your mic detects sound.
calibration-mean-offset-placeholder = Mean offset: —
calibration-mean-offset = Mean offset: {$sign}{$ms}ms
calibration-suggested-placeholder = Current: —   →   Suggested: —
calibration-suggested = Current: {$current}ms   →   Suggested: {$suggested}ms
calibration-get-ready = Get ready…
calibration-hits-recorded = {$hits} / {$total} hits recorded
calibration-complete = Calibration complete!
calibration-start = Start
calibration-apply = Apply
calibration-try-again = Try Again
calibration-cancel = ← Cancel

# Options
options-input-lag = Input lag
options-input-lag-tooltip = Shift detected notes earlier/later to match your setup's audio delay.

# Guided tutorial tour (menu::tutorial)
tutorial-step = Step {$n} of {$total}
tutorial-skip = Skip Tutorial
tutorial-title-main = Main Menu
tutorial-body-main = Your home base — head into Play, open Options, or find Help / About from here.
tutorial-title-play = Play
tutorial-body-play = Pick a real song, create one, start a jam, practice bends, or work through lessons — choose how you want to play.
tutorial-title-mode-select = Song picker
tutorial-body-mode-select = Browse every song, sort by band, difficulty, name, or genre, search the catalog, then choose 2D or 3D on this screen.
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

editor-tab-chart = Chart
editor-tab-details = Details
# First-launch greeting (menu::pages::welcome) — shown once, when no
# profile.json exists yet.
welcome-title = Welcome to Harmonicon
welcome-body = You play a real harmonica into your microphone, and Harmonicon listens and scores you as the notes scroll by. Nothing works until it can hear you, so start there. A C diatonic harmonica suits almost everything here.
welcome-setup-mic = Set up your microphone
welcome-tour = Take the guided tour
welcome-lessons = Start with a lesson
help-first-run = First-time setup
welcome-skip = Skip for now
# Microphone trouble. `mic-warning-*` is the in-play overlay
# (gameplay::mic_warning_overlay), deliberately terse and free of the raw
# device error; `options-mic-*` is the Options banner, where the specifics
# are what the player came for.
mic-warning-failed = No microphone — nothing you play will be scored. Check Options.
mic-warning-permission = Waiting for microphone permission.
options-mic-failed = No microphone: {$reason}
options-mic-awaiting-permission = Waiting for microphone permission — grant it, then retry
# Shown in the picker beside a detector that can only resolve one note at a
# time, because choosing one makes every chord in a chart unhittable.
algo-single-notes-only = single notes only
# In-play banner for that combination (gameplay::warning_banner).
chord-warning-monophonic = This song has chords, and the chosen pitch detector hears one note at a time. Pick FFT or NMF in Options to score them.
# "Which harmonica are you holding?" (menu::pages::harp_check) — between
# picking a song and loading it.
harp-check-title = Your harmonica
harp-check-intro = If you have a different one, say so here and pick what the swap should keep.
harp-check-key = Key
harp-check-type = Type
harp-check-mapping = When the harp differs
harp-check-same-holes = Same holes (the tune changes key)
harp-check-transpose = Same tune (the holes change)
harp-check-play = Play
harp-check-cost-clean = Plays as written on this harmonica.
harp-check-cost-bends = {$count} note(s) need a bend
harp-check-cost-overblows = {$count} note(s) need an overblow
harp-check-cost-unreachable = {$count} note(s) can't be played on this harmonica
# Names the harmonica a chart was written for, on the harp-check page.
harp-check-chart-harp = Written for a {$key} {$kind} harmonica.
harp-kind-diatonic = diatonic
harp-kind-chromatic = chromatic
harp-summary-diatonic = Diatonic
harp-summary-chromatic = Chromatic
harp-summary-holes = {$n} holes
harp-summary-position = {$pos} position
harp-banner-use = Use a {$key} harmonica
harp-banner-key = key of {$key}
harp-banner-fallback = Playing in {$key}
harp-row-blow = blow
harp-row-draw = draw
harp-row-overblow = overblow
harp-row-overdraw = overdraw
harp-row-slide = slide
# The track picker on the harp-check page, shown only for an imported score
# file with more than one playable part.
harp-check-track = Part to play
harp-check-track-option = {$name} — {$notes} notes, {$percent}% playable
harp-check-track-unnamed = Track {$index}
# Heading above a lesson's five training tiers, with how much of the ladder
# is done (the mastery meter).
lesson-training-heading = Training — {$percent}% mastered
# The five training tiers: each one's name, and what it asks of the player.
lesson-training-tier-isolate = Isolate
lesson-training-tier-isolate-about = The technique on its own, slowly, on one hole.
lesson-training-tier-consolidate = Consolidate
lesson-training-tier-consolidate-about = The same exercise, a little faster.
lesson-training-tier-vary = Vary
lesson-training-tier-vary-about = Every hole the lesson covers, at every depth they reach.
lesson-training-tier-in-context = In Context
lesson-training-tier-in-context-about = The technique inside a musical phrase, in eighth notes.
lesson-training-tier-interleave = Interleave
lesson-training-tier-interleave-about = An unpredictable order, so it can't be played from memory.
# A training tier's goal: the lesson goal line, then the tier's tempo and length.
lesson-training-goal = {$goal}, at {$bpm} BPM for {$bars} bars
lesson-training-start = Start Training

# The skill-tree view of the curriculum: one row per track.
lesson-tree-title = Skill Tree
lesson-tree-find-next = Find next lesson
lesson-tree-broken = This curriculum cannot be drawn: {$error}
lesson-tree-unit-progress = {$done}/{$needed}
# One track's mastery meter above the skill tree: how much of its training ladders is cleared.
lesson-tree-track-mastery = {$track} — {$percent}%
# The warm-up review queue above the skill tree: its label, and one due training as "lesson · tier".
lesson-tree-warmup = Warm-up:
lesson-tree-review-due = ↻ Review due: play its highest tier again to keep the skill
lesson-tree-warmup-item = {$lesson} · {$tier}
# The practice streak above the skill tree, shown from two days up.
lesson-tree-streak = {$days}-day practice streak
lesson-tree-needs = Needs: {$lessons}
lesson-tree-optional = Elective

# Scored play — technique cue beside a note head, and the technique coach row
cue-target = → {$note}
cue-vibrato = vib {$rate}/s
cue-wah = wah {$rate}/s
coach-follow-pulse = Follow
coach-bend-more = Bend more
coach-on-target = Hold it
coach-too-far = Too far
coach-swing-more = Wider
coach-faster = Faster
coach-slower = Slower
coach-on-rate = Good
coach-rate = {$rate}/s

song-clear-search = Clear
song-results = {$count} songs
song-none-selected = Select a song
song-no-results = No matching songs. Try fewer words or clear your search.
song-picker-keys = Up/Down: Select   Enter: Play   /: Search   Esc: Back
song-result-one = 1 song

song-update = Update songs: {$name}

song-delete = Delete song
song-confirm-delete = Hide {$name} from this song source? It will stay hidden after updates. Its files are kept.
song-retained = Kept locally — no longer in {$repository}
song-delete-failed = Could not hide song: {$error}
