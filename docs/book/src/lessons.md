# Lessons

**Play → Lessons** is a guided curriculum, drawn as a skill tree. Along the
top runs a row of **unit** nodes — Unit 1, Unit 2, and so on — and each
unit's lessons hang below it, connected by the order you take them in. The
tree is wider and taller than the window: scroll in both directions to see
the rest of it.

![The skill tree](images/skill-tree.png)

There are **two locks**. A lesson shows dark until you've passed whatever it
leads on from — hover it and the tooltip names what it's still waiting on. A
whole unit stays shut until you've passed most of the one before it; the
number on a unit node ("3/8") is how many of its lessons you've done and how
many open the next unit. It's *most*, not all — one lesson you're stuck on
shouldn't wall off the rest of the course.

Colour is the skill each lesson belongs to (bending, rhythm, tone…). Some
nodes also carry a **ring of dots** above them: that's a **training
ladder**, and the filled dots are how much of it you've cleared. A ladder
is five drills on the same technique, each harder than the last —
isolated and slow, then faster, then across every hole the lesson covers,
then inside a phrase, then shuffled so you can't run it from memory. The
bend lessons are the ones that have them; a lesson with no ring has no
ladder. A passed lesson stays replayable any time, and your progress is
saved (`profile.json`) and survives restarting the game.

Clicking a lesson — locked or not — opens its **reader page**: instructional
text explaining the technique, a goal line (e.g. "Goal: 70% overall accuracy"),
a **Start Lesson** button, and the ladder's tiers if it has any.

About twenty lessons show **Mark as Done** instead. Those are pure
instruction with nothing to score — tongue blocking, for instance, which
the microphone genuinely can't tell apart from puckering, or the
practice-skills lessons, which are about how you work rather than what
you play. Rather than pretend to grade them, the game asks you to say
when you've read and understood them.

## Practice tools on the page

Some reader pages carry interactive tools below the explanation, there to
play with before you start the drill. They never affect your score —
completion still comes from **Start Lesson** or **Mark as Done**.

- **Circle of fifths** — the twelve keys arranged so each neighbour is a
  fifth away, with the harp's key at the top and its playing positions
  marked. Step it up or down a semitone to see the same relationships
  from another harmonica's point of view; position lessons show only the
  position they're teaching, so the picture stays uncluttered.
- **12-bar grid** — the form laid out as twelve boxes, coloured by chord.
  Step the highlight forward or back a bar, or reset it to the top. The
  chords shown follow whichever progression the lesson is about: the
  standard blues, a quick change, a minor blues, or the jazz blues.
- **Metronome** — start and stop it, nudge the tempo by 5 BPM either way,
  switch between straight, shuffle, and equal-triplet pulses, and mute the
  click if you'd rather watch than listen. Some lessons change tempo after a
  configured number of bars to demonstrate an accelerando.
- **Form map** — a row of named sections such as A–A–B–A. Repeated sections
  share a colour, and Previous/Next lets you follow the form as you listen.

**When a page shows a metronome and a grid together, starting the
metronome walks the highlight through the form** — one bar per bar, in
the feel you picked. That pairing is the whole point of lessons like
*Counting the Bars* and *The Turnaround*: you hear where you are and see
where you are at the same time.

## Core units and electives

The curriculum runs to a hundred lessons across seventeen units. Units 1
to 6 are the numbered **core course**, taken in order; three more — Rhythm
lab, Applied positions and Ear training — continue it without numbers, and
gate the same way. Everything else is an **elective**.

An elective is exactly what it sounds like: worth your time, never in
your way. It can have prerequisites of its own, but it never counts
toward a unit's gate and never holds up the required course. You can spot
one on the tree without reading anything — the branch leading to it is
**dotted and a different colour**, and the lesson carries a small badge.
It's a hue, not a dimming, because dimming already means locked and an
elective is perfectly playable.

Eight units are electives end to end. Their unit nodes take the same
colour, and their number counts *your* progress through them ("2/6")
rather than a threshold, because there's nothing to unlock. They still
sit behind the units before them in the course, and read as locked until
you get there.

## Unit 1 — Blowing the Harmonica

Single clean notes, chords, tongue blocking, octave splits, slides, and
hand-shape/wah — the physical fundamentals of getting a controlled sound
out of the instrument, before rhythm or improvisation enter the picture.
Breathing and long tones, your first bends (working up to the deep 2- and
3-draw bends 2nd-position blues lives on), vibrato, and "ta-ka" tongued
articulation round out the fundamentals.

## Unit 2 — Counting the Blues

The 12-bar form, playing in time, call-and-response (Harmonicon plays a
short phrase, you echo it back — the game waits for you, however long you
need), and finally **improvisation**: the only lesson with no fixed
notes to hit at all. It opens an ordinary [Jam Session](jam-session.md) and
judges you on how much of what you played landed on a chord tone or in the
blues scale, via the same live hole-map coloring Jam Session itself uses.
When you feel done, open the pause menu and click **Finish Lesson**.
Counting drills (four-on-the-floor down to just beat 1, then a full 12-bar
walk of chord roots and the turnaround that leads back to the top), a
straight-vs-shuffle feel drill, and the classic train chug — including a
chorus that genuinely speeds up — round out the unit.

Most drills are scored exactly like a normal [Play 2D](play-2d.md) song
underneath — same falling notes, same HUD — with the pass/fail judgment
layered on top of the ordinary results screen instead of a song's usual
best-score tracking.

## Unit 3 — Blues Vocabulary

The bridge from drills to music. You start with the 2nd-position blues
scale itself, then learn three short original licks call-and-response
style — the game plays each one, you echo it back — followed by a set of
licks built specifically around the "crying" bent notes, and finally a full
12-bar chorus that places a different lick over each chord.

From there it's four more open-jam lessons, each judged on a different
slice of your playing: land specifically on chord tones as the chords
change (not just anywhere in the scale), improvise over a minor blues
progression instead of the usual major one, leave real space by playing
two bars and resting two, and handle a quick change — where the IV chord
arrives in bar 2 instead of bar 5. They all use the same "Finish Lesson"
pause-menu flow as Unit 2's improvisation lesson, and the two that change
the form put a 12-bar grid and a metronome on the page so you can hear it
before you play over it.

## Unit 4 — Scales and Improvisation

The major scale, the minor pentatonic and the country scale as playable
drills, then the major and minor pentatonic again as open jams where
what's scored is how much of your playing stayed inside them. The unit
closes on the **circle of fifths**: an instructional page with the
diagram on it — rotate it to any harp key and see which position lands
you in which key — followed by a jam that calls out a new position every
few bars, so you play that relationship instead of just reading it.

## Unit 5 — Jazz

Swing eighths, the ii-V-I outlined in chord tones, and the **jazz blues**
form — the 12 bars with the substitutions that turn a blues into a
standard, with a grid and metronome on the page to hear the difference.
One elective introduces the chromatic harmonica's slide button.

## Unit 6 — Finding Your Way Around

Knowing the layout without hunting for it. Landmarks on holes 1, 4, 7 and
10; the middle, low and high registers on their own; clean jumps between
holes; the two awkward crossings every diatonic player meets (3-to-4 and
6-to-7, where the breath pattern changes direction); and finally leaping
across registers in one phrase.

## Rhythm lab, Applied positions, Ear training

Three more gated units continue the course past Unit 6:

- **Rhythm lab** — eighth-note triplets, straight against shuffle with a
  metronome you can flip between the two, syncopation and rests,
  sixteenth-note articulation, and holding a tempo without drifting.
- **Applied positions** — landing notes for 1st, 2nd and 3rd position,
  each with a circle of fifths showing just that position.
- **Ear training** — four lessons that **hide the scrolling notes**. You
  hear a note, a shape (up, down or the same), or a whole phrase, and
  play it back. See the note on these below.

## The elective units

The rest of the tree is optional. Nothing here gates anything, so take
what you need:

- **Advanced electives** — high blow bends on holes 8–10, overblows and
  overdraws: the notes a diatonic harmonica isn't supposed to have.
- **Tongue-block electives** — slap, pull, side pull, rake, flutter and
  moving octaves. Mostly instructional: the microphone can't tell a
  tongue-blocked note from a puckered one, so these are explained and
  marked done rather than scored.
- **Ornaments** — the slow warble, the fast shake, trill cells and
  glissandi.
- **Expression** — hand, throat and diaphragm vibrato, controlling
  vibrato rate, crescendo and diminuendo, and long-tone control.
- **Accompaniment** — playing *behind* somebody: chord rhythm, staying
  out of a singer's way, fills between lines, and trading fours.
- **Chromatic harmonica** — button coordination, choosing between
  enharmonic slide fingerings, chromatic fragments, legato phrasing
  across the slide, and an original jazz study, *Crossing Lights*.
- **Musicianship** — intervals, building chords, guide tones, arpeggio
  into scale, transposition (with a circle of fifths to work it out on),
  and listening for form.
- **Practice skills** — how to practise rather than what: slow practice,
  isolating a loop, spaced review, recording yourself, and mixed review.

## Ear-training lessons hide the notes

The ear-training unit runs on the ordinary gameplay screen with the
falling-note prompts turned off. The call still plays, scoring still
works exactly as it does everywhere else, and the HUD still tells you how
you did — you just have to find the note by ear instead of reading it off
the highway. If a run feels broken because nothing is scrolling, that's
this, working as intended.
