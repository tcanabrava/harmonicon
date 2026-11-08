# Multi-instrument plan

Extending Harmonicon from harmonica-only to any instrument whose notes a
microphone can hear. The first targets are the instruments on hand to test
with: electric guitar, acoustic guitar and transverse (Boehm) flute.

The hurdy gurdy is out of scope for now. Its drones sound for the whole
song, its trompette is judged by rhythm rather than pitch, and its keybox
needs its own model, so it costs more than any of the other three.
Nothing below rules it out; see "Later instruments".

## The principle: pitch is universal, position is per-instrument

The game already scores **pitches**, not holes. `ScheduledNote::expected_pitch`
is a MIDI number, the judge compares it against `ActivePitches`, and
`hole`/`is_blow` only pick a lane and a colour. Harmonica knowledge comes in
at three edges:

1. **Where a pitch is played**: `pitch_map` (pitch → hole/action/technique)
   and `hole_notes` (hole → pitches).
2. **What can sound together**: `harmonica_constraints` (one breath
   direction per frame) and the editor's `enforce_direction`/`enforce_expr`.
3. **How it is drawn**: the hole lanes in `highway_2d`, the hole strip and
   `harmonica_overlay`, the editor's hole × direction rows, and the bend
   trainer's diagram.

So the plan is to put a single instrument model behind those three edges
and leave everything between them (clock, judging, combo, adaptive
difficulty, repeats, lyrics, call-and-response, lessons, training records)
alone. About 60 files name `Harmonica` today, but most of the actual
`hole`/`is_blow` use sits in roughly a dozen of them (`gameplay_2d`,
`notes`, `call_response`, `phrase_overlay`, `training`, `pitch_map`,
`hole_map`, the bend trainer and the editor's state/grid).

**Position is guidance, not something we score.** A microphone hears the
pitch. It cannot tell an A on the open fifth string from the same A on the
fifth fret of the sixth string, or a flute's alternate fingering from the
standard one. The harmonica already works this way: a 2-draw G and a 3-blow
G look the same to the judge. The chart's position is a fingering
suggestion, used for display and in the editor, and scoring stays
pitch-only. Identifying the string from its timbre is a research topic for
later, not part of this plan.

## The instrument model (`harmonicon-core`, Bevy-free)

There is one closed, serde-tagged `Instrument` enum, shaped like
`Harmonica` today, plus one `InstrumentModel` trait that each variant
implements. It should be a closed enum rather than trait objects for two
reasons: a chart has to name its instrument in JSON, and every UI `match`
should break at compile time when a new instrument is added.

```rust
enum Instrument {
    Harmonica(Harmonica),          // existing type, unchanged
    Guitar(StringedConfig),        // tuning, strings, frets, capo, acoustic/electric
    Flute(FluteConfig),            // foot joint (C or B), range
}

trait InstrumentModel {
    fn range(&self) -> Option<(u8, u8)>;                 // feeds PitchRange
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition>;  // generalised pitch_map
    fn pitch_of(&self, p: &PlayPosition) -> Option<u8>;      // generalised hole_notes
    fn can_sound_together(&self, a: &PlayPosition, b: &PlayPosition) -> bool;
    fn reconcile(&self, anchor: &PlayPosition, others: &mut [PlayPosition]);
    fn techniques(&self) -> &'static [TechniqueKind];    // see Note Trainer
    fn expression_scope(&self) -> ExpressionScope;       // whole-instrument vs per-voice
    fn envelope(&self) -> Envelope;                      // Sustained | Decaying
    fn lane_layout(&self) -> LaneLayout;                 // the play highway's lanes
}

enum PlayPosition {
    Hole(HoleAssignment),                                 // pitch_map's hole, action, technique
    Fret { string: u8, fret: u8, technique: StringTechnique },
    Fingering(FingeringAssignment),                       // flute: note + which fingering
}
```

These belong in core, not a new crate: `chart` has to name the instrument,
and chart lives in core. The harmonica implementation is an adapter over the
existing `harmonica`/`pitch_map` code, not a rewrite.

**Phase 1 has landed** (`harmonicon_core::instrument`, harmonica only). What
the voice model below describes is split across two places in the code:
`can_sound_together`/`reconcile` on the trait, for the Song Editor, and
`note_tracker::VoiceFilter`, the stateful per-frame filter live detection
runs before onset counting. `ScrollAxis` supports `Vertical` for harmonica
and `Horizontal` for guitar.

### Voice model: what can sound together

This answers "the Song Editor needs to know the instrument's behaviour". The
editor, the post-detection filter and the import converter all ask the same
model:

| Instrument | Voices | Rule for simultaneous notes |
|---|---|---|
| Harmonica | holes | all sounding holes share one breath direction (today's `enforce_direction` and `BreathDirectionTracker`) |
| Guitar | 6 strings (or the tuning's count) | one note per string; fretted notes span at most ~4–5 frets (open strings are free); at most 6 notes |
| Flute | 1 | monophonic: a new note ends the previous one |

`ExpressionScope` replaces `enforce_expr`'s hard-coded "vibrato and wah
apply to every hole" rule. That is right for the harmonica and the flute,
where breath shapes the whole sound. It is wrong for the guitar: vibrato on
one string leaves the others alone.

`Envelope::Decaying` matters for scoring. `sustain_points` rewards holding
a pitch, but a plucked string decays naturally, so a guitar's sustain
window has to accept a decaying signal (or score onset only) rather than
punish the instrument for doing what it does.

## Chart format (format 1.8.0, additive)

Landed, including multi-part song discovery. Guitar charts load for Play 2D's
horizontal tab highway. The Song Editor edits them on its shared grid, one row per string. Score imports offer guitar arrangements alongside harmonica
arrangements, preserving tab tuning and authored string/fret positions.

- New optional top-level `instrument`. If it is absent, the chart is a
  harmonica chart and `harmonica` is the instrument, so every existing chart
  loads unchanged.
- New optional `NoteEvent.position`, e.g. `{"string": 5, "fret": 3}`.
- **Flute (format 1.9.0):** `"instrument": {"type": "flute", "foot":
  "c"|"b"}`, and an event is its `note` plus an optional `fingering`, the
  index of an alternate (absent or 0 the standard). It is not
  `{fingering, harmonic}` as first sketched: the source chart gives the
  third octave as key sets with no partial number, and two notes on the
  same keys already *are* a register pair (`Flute::overblown_pairs`),
  so nothing needs a harmonic number written down.
- For non-harmonica instruments `note` (the pitch) is **required** and is
  the truth. `hole`/`action` become optional, and the schema requires them
  only when the instrument is a harmonica.
- `Modifier` gains guitar techniques: `hammer_on`, `pull_off`, `slide_to`,
  `harmonic` and `palm_mute`. The existing `bend` and `vibrato` are reused
  as they are.
- **One chart is one instrument part.** A song folder may hold several
  charts, e.g. `song/harmonica.harpchart` and `song/guitar.harpchart`. This
  keeps the schema flat and lets a pack ship a guitar part for an existing
  song without touching its harmonica chart. Desktop discovery and the
  bundled manifest share `assets_management/song_parts.rs`: every native
  chart is offered, sorted by filename, with the filename stem appended to
  the song name when it has multiple parts. A sibling MIDI stays backing
  audio; imports are a fallback only when no native chart exists. Keep the
  `.harpchart` extension: the name is now historical, but it is the play
  format (see the import-source rule) and renaming it costs every pack.

Older engines refuse a 1.8.0 chart (or a flute's 1.9.0) through
`format_version_supported` rather than misreading it. The loader runs
`chart_check::instrument_problems` after the schema, since what an event
must name depends on the root's instrument.

## Pitch detection per instrument

The detectors stay instrument-agnostic, as they are now. What changes per
instrument is the stage after them: core's `note_tracker::NoteTracker`
(behind gameplay's `InstrumentPitchFilter`) applies the instrument's
`VoiceFilter` before onset counting. Only the harmonica's `Breath` filter
exists so far; each instrument below adds its own:

- **Guitar:** the hard part. Chords need a polyphonic detector (FFT or
  NMF).
  - With `CHUNK_SIZE = 4096`, an FFT bin is about 11 Hz wide, but low E
    (82 Hz) and F (87 Hz) are 5 Hz apart. Bin resolution can't separate
    the lowest notes, so expect to need time-domain refinement or a longer
    window below roughly A2. Measure this; don't assume it.
  - Strings are slightly inharmonic, so the NMF templates (already planned
    per harmonica technique, item 3 in `pitch_detection_plan.md`) need a
    per-instrument dictionary.
  - A strum spreads a chord's onsets over 20–50 ms, so
    `chord_is_sounding` needs an onset tolerance for arpeggiated attacks.
  - An unplugged electric guitar is nearly silent. Either it goes through an
    amp near the mic or through an audio interface, which is just another
    cpal capture device.
- **Flute:** close to a sine wave, which suits YIN, pYIN and MPM. The known
  failure is **octave errors**: an overblown note and its fundamental share
  most of their partials. Breath noise is broadband and needs a noise
  floor.

Every instrument needs its own recorded corpus through `note_bench` before
any constant is tuned. This is the same rule `pitch_detection_plan.md`
already applies to the harmonica.

## Play Song: lanes per instrument

A `LaneLayout` built from the instrument replaces `highway_2d`'s
`100.0 / hole_count` assumption. It holds the **scroll axis** (vertical for
the harmonica and flute, horizontal for the guitar), the lane specs, the
position → lane mapping, and the "instrument strip" drawn beside the hit
zone (today's hole strip).

| Instrument | Highway | Instrument strip |
|---|---|---|
| Harmonica | unchanged: vertical hole lanes, blow/draw colour | hole strip and overlay |
| Guitar | **horizontal strings scrolling right to left, read like tablature** (Rocksmith's 2D views, Yousician): each string a colour-coded row, note gems carry the **fret number**, a chord is one gem spanning its strings with the chord name on it | a **fretboard strip** at the hit line lighting the target frets, with a highlighted **anchor window** (the hand position) that slides ahead of position shifts |
| Flute | **one row per pitch** the song uses, horizontal like tab, each row named at the hit line and each note carrying its name (landed: `flute_visuals`, `LaneLayout::pitch_rows`). A highway of key rows, with each fingering as blocks on its keys, was built and dropped: a fingering is a whole-hand shape and read poorly streaming past | a **fingering strip under the highway**: a chart lying along the screen as a horizontal printed chart draws it (head joint left, six holes on the line, the thumb key below, G♯/trill/E♭/foot keys small above) for the note to play now, its name, and a bar filling to its start then emptying as it is held, plus a smaller, dimmer chart for the next note. The staff marks the same note gold. The pause menu's **Fingering chart** hides it (landed; `FluteView`) |

The guitar's orientation is chosen so the screen teaches the shape players
will meet in real tablature everywhere else. It means `LaneLayout` carries
a scroll axis from the first phase, even though only the harmonica uses it
then.

Each layout is one module, e.g. `gameplay/lanes/{harmonica,strings,pitch}.rs`.
The note-spawning half of `gameplay_2d` asks the layout where a note goes
and never asks which instrument it is.

## Song Editor

The document seam is implemented: `GridNote` stores identity, timing,
expression and core's `NoteTarget`, with no hole/string/fret fields.
`NoteTarget::Sounding` preserves a sounding pitch and optionally pins an
authored placement; `Positioned` preserves a physical instruction across
instrument tuning changes. Existing harmonica editing uses `Positioned` to
keep its current same-hole key-change behaviour. This makes the distinction
explicit without duplicating a pitch that can go stale after retuning.

The current harmonica view uses `harmonica_note.rs`; geometry receives a
visual row, and audio resolves notes through `InstrumentModel`. Core tests
exercise alternate guitar placements without changing sounding pitch and
reject invalid authored assignments. Automatic placement supplies candidates
to a sequence solver and never rewrites the document during rendering.

The editor state now holds one `InstrumentSelection`, tracked by undo and
grid invalidation. It resolves a preset or a configured core `Instrument`;
configured instruments retain their tuning when the song key changes.

`harmonicon_core::note_event_codec` now supplies the chart-event boundary:
it decodes instrument positions into note targets and encodes resolved targets
back into events while retaining expression modifiers and their intensity.
Guitar round-trip fixtures cover sounding pitches, fingering, bends and
harmonics. This boundary rejects incompatible authored placements rather
than guessing a replacement. Guitar charts now load and save through this
boundary in the editor, retaining configured tuning and capo.

A guitar chart is edited on the same grid as a harmonica one: one row per
string, notes in their string's colour labelled as tab, and the note column
offering fret and string moves (the latter pitch-preserving), bends,
harmonics and per-string vibrato. Edits reject occupied strings and
excessive hand span across overlapping sustains, including while a note
is resized or pasted.

A flute chart is a piano roll of pitch rows, C7 at the top, rows narrower
above sixteen so the range fits. A note plays its standard fingering unless
the note column's Fingering button picks an alternate; one voice, so a new
note ends the one sounding and two onsets together are refused. A
fingering view draws the flute's keys as the rows and each note as the
fingering that plays it; a key is not a pitch, so there notes edit only
in time.

An empty harmonica document can select a standard guitar or a flute in
Details. Loaded parts keep their own tuning, capo and foot joint; picking
them for a new document belongs with "My instruments" (phase 5). A
clarinet adapter would also have to separate written notation from
sounding pitch; hurdy-gurdy support needs voice roles and rhythmic events.

- **Rows come from the instrument.** Harmonica: holes × direction, as
  today. Guitar: 6 tab rows with the fret number drawn on each note. Flute:
  a monophonic piano roll, or its keys in fingering view (all landed).
- **`enforce_direction` becomes `VoiceModel::reconcile(group)`**, run on the
  same `overlapping_group`:
  - harmonica: propagate the direction (today's behaviour);
  - guitar: moving a note onto an occupied string re-voices it to the next
    string that reaches the pitch, or refuses if none does; a chord beyond
    hand span is flagged;
  - flute: a new note ends the previous one (landed: `flute_note::place`,
    and `guitar_note::fits` refusing any overlap on drag, resize and
    paste).

  `pitch_compatible`/`pitch_deny_key` already give the refusal path a
  localized reason.
- **A fingering solver** for guitar. MIDI import, recording and transposing
  only know the pitch, so a dynamic-programming pass picks strings and frets
  that minimise hand movement, under the span rule. This is the guitar's
  version of `pitch_map::suggest_key`.
- **Guitar Pro import keeps its strings and frets.** `harmonicon-score`'s
  `guitar_pro` reads string and fret today and throws them away after
  converting to pitch. For a guitar part those are the author's own
  fingering and beat anything the solver guesses. `convert` should take a
  target `Instrument` rather than always a `Harmonica`.
- The `HarmonicaKind` picker becomes an instrument picker that includes the
  tuning (standard, drop D, DADGAD or capo for guitar; C or B foot for
  flute). **Open**, with "My instruments": today Details has "Use guitar"
  and "Use flute" for a standard guitar and a C-foot flute.

## Note Trainer (replaces the Bending Trainer)

**Is the flute's "many notes on one fingering" a bend? No.** The two
techniques are physically different, and the trainer needs to teach them
differently:

- A **bend** is a *continuous* glide from one position to a target. The
  skill is landing on the right pitch, holding it, and releasing back. The
  trainer shows a live pitch trace against the target.
- **Register selection** jumps between *discrete* harmonics of one
  fingering, chosen by air speed and embouchure. The skill is choosing the
  harmonic cleanly without cracking to its neighbour. The trainer shows a
  ladder of targets and scores a clean landing, not a glide.

The categories also cross instruments. A guitar's natural harmonics are
register selection on a string. A harmonica overblow sits close to
register selection, though the physics differ. A flute does have a small
continuous lip bend (about ±50 cents), which is an intonation skill, not a
note-changing one.

So `TechniqueKind` becomes:

| Kind | Trainer mode | Harmonica | Guitar | Flute |
|---|---|---|---|---|
| `Glide` | pitch trace to target | bends | string bends, slides | lip bend (fine intonation) |
| `Register` | discrete harmonic ladder | overblow/overdraw | natural harmonics | **overblown octaves/twelfths on one fingering** |
| `Position` | same pitch, different place | (rare) | the same note on different strings | alternate fingerings |
| `Target` | tuner: hit and hold a note | every hole | every fret | every fingering |

Every instrument gets `Target`, the basic tuner drill, so an instrument
with no bends at all still has a trainer. The trainer asks `techniques()`
which modes to offer and shows only those, with the instrument strip from
Play Song as its diagram. The training generator's `DrillTechnique::Bend`
generalises the same way. Its "alternate the plain note with the altered
one" rule carries over directly to `Register` (play the fundamental, then
the harmonic).

**Landed**: `harmonicon_core::note_trainer` lists each instrument's drills
per kind and reads a heard pitch against one (`hear`), and the guitar has
its own trainer screen (`AppState::NoteTrainer`, the highway's fretboard
strip as its diagram), and so does the flute (its upright drawing). The Play menu's Note Trainer entry asks for the
instrument; the harmonica still opens the Bending Trainer, and moves onto
the shared model only after that trainer's live validation, so the move
can't be confused with a detection problem.

## Content, progress and menus

- **One lessons pack, a curriculum per instrument.** The pack's top-level
  folders are instruments (`harmonica/`, `guitar/`, `flute/`, `piano/`),
  each lesson names its `instrument`, and the catalog loads only the
  selected instrument's, so each has its own technique DAG. This replaced
  the earlier idea of a pack per instrument: one pack keeps one release,
  one CI and one set of translations. `validate-pack` keeps lesson ids
  unique across the whole pack and refuses a prerequisite that crosses
  instruments.
- **Progress is per instrument through its keys.** Song records are keyed
  by the part's chart, lesson and training records by the lesson's
  pack-unique id, Note Trainer records by `instrument:kind`; the practice
  streak stays global (you practised today, whatever you played). Ids are
  unique within a pack, not across packs, so a second pack reusing an id
  for another instrument would share its progress.
- **"My instruments"** generalises the harp-check page's
  bring-your-own-harp choice. It describes how each instrument the player
  holds is set up rather than listing which they own. **Landed already:** the
  song list names each part's instrument, and the harp-check page is an
  instrument check: a guitar part asks for the tuning, a flute part for
  the foot joint, per song, and play places the chart onto the answer
  (`instrument::substitute`, carried in `EffectiveHarmonica::instrument`).
  **Landed too:** those answers stick. `settings::MyInstruments` holds
  the guitar's tuning and capo, the flute's foot joint (each `None` =
  follow each chart) and the flute view, persisted in `settings.json`; the pre-song
  page opens on them (`harp_check::played_for`) and saves each answer,
  and Options has a "My instruments" section for them. Play 2D draws the
  fretboard strip, flute drawing and hint line from the played instrument
  (`EffectiveHarmonica::instrument_for`), so a capo's frets count from it
  and the hint names it. Song select's Instrument column shows each
  guitar or flute part's reach on the player's setup ("Guitar · 83%"):
  the scan keeps each part's instrument and sounding pitches
  (`song_parts::catalog_fields` → `SongEntry::part`, also through the
  bundled manifest), and `substitute::reach` counts the pitches with
  any position on the played instrument. Per note, so it can't see a
  chord asking two notes of one string; the pre-song page's count is
  exact.
- **Audition and call-and-response synthesis** need a timbre per
  instrument in core's `synth`:
  - Karplus–Strong for plucked strings: cheap and convincing;
  - a sine plus filtered breath noise for the flute.

  The phase-integration vibrato rule applies to both.
- **Jam Session**: see the next section.

## Jam Session per instrument (landed)

A jam is a tune playing while the player improvises over it. For every
instrument the screen shows the backing's form and chords (instrument-free
already) and **the instrument the player chose, with the notes that fit the
scale marked** — what the harmonica's hole map does today.

- **Counting is instrument-free** (landed). `improv` tallies every pitch
  the detector hears by its note name — chord tone of the current bar, in
  scale, outside — so a bent note counts like any other, and a guitar or
  flute counts without any instrument model. The detector's range is
  already sized to the part's instrument (`setup_scoring_config`). The
  "detected note" readout names a hole and breath when the harp has one,
  and the note otherwise.
- **The instrument is the part's.** The Jam song picker offers every part,
  and the pre-song page that runs before a jam asks a guitar for its tuning
  and a flute for its foot joint.
- **Each instrument is drawn with its in-scale notes marked**
  (`jam::instrument_map`), and the note being played lit by its fit
  (`classify_note_fit`, the hole map's rule):
  - harmonica: the hole strip, as before;
  - fretted (guitar, bass, cavaquinho): its strings, open to the 12th
    fret, which draws the familiar scale boxes without authoring any;
  - piano: the keyboard (`dialogs::piano_keys`), C3 to C6;
  - anything else (flute, clarinet, bowed strings): its range as note
    cells, an octave to a row.

  The guitar's was checked on screen; the piano's and flute's are covered
  by `instrument_map`'s tests only.
- **Harmonica-only, and staying so:** the "grab a C harp" hint, the bend
  overlay, call-and-response and the position compass.

## Phases

Each phase lands and is committed on its own. Phase 1 must not change
behaviour.

1. **The seam, harmonica only** (landed). Add `Instrument`, `InstrumentModel`,
   `PlayPosition`, `VoiceModel` and `LaneLayout` (with its scroll axis),
   with the harmonica as the only implementation. Route `pitch_map`, the
   constraint filter, the editor's reconcile, `ScheduledNote` (position +
   lane instead of `hole`/`is_blow`) and `highway_2d` through them.
   Existing tests plus pixel-diffed captures (`capture_static_screens.py`)
   prove nothing moved.
2. **Guitar** (two of the three instruments, and the largest library via
   Guitar Pro):
   - the core model (landed);
   - chart 1.8.0 naming it: the `instrument` field, `position`, the
     conditionally-required `note` and validator support have landed;
     multi-part folders are discovered and imports offer both instruments. A chart field is
     only designed alongside the model it names, so the flute's fields
     arrive with the flute;
   - fretboard strip, string colours and grouped chord gems (the basic
     horizontal tab highway, fingering solver and Guitar Pro string/fret
     pass-through are implemented);
   - editor rows, on the shared grid, one per string (landed);
   - Karplus–Strong synth (landed);
   - **open:** a recorded corpus, then detection tuning (low-string
     resolution, chord onset spread, decaying sustain).
3. **Note Trainer** (landed for the guitar and the flute). The drill
   model in core is generic over `TechniqueKind`; the guitar and the flute
   have the trainer screen. **Open:** the harmonica moves onto it from the
   Bending Trainer after that trainer's live validation, keeping its drill
   stats and profile keys.
4. **Flute** (landed but for its corpus). Chart format 1.9.0, a
   monophonic model with a C or B foot, the Boehm fingering table as data
   (standard fingerings and third-octave alternates, from the Woodwind
   Fingering Guide; register pairs are read off matching key sets, not
   written as harmonic numbers), Play 2D (pitch rows with a horizontal
   fingering strip under them, which can be hidden), the Song
   Editor in the same two views, and the trainer's `Register` and
   `Position` tabs. **Open:** a corpus aimed at octave errors.
5. **Content and progress** (landed but for Jam Session). "My
   instruments" (tuning, capo, foot joint, flute view). Per-instrument
   curricula: one lessons pack with a top-level folder per instrument
   (harmonica, guitar, flute, piano), loaded one instrument at a time, so
   each has its own lesson graph; `validate-pack` keeps lesson ids unique
   across the pack and refuses a prerequisite crossing instruments.
   Progress is per instrument through its keys: a song record per part's
   chart, a lesson's pack-unique id, the Note Trainer's `instrument:kind`.
   Guitar and flute parts ship in the songs pack (`port-part`).
   **Open:** per-instrument Jam Session, and with it `hole_map`, the one
   seam still reading holes. Lesson ids are unique within a pack, not
   across packs: a second pack's lesson for another instrument with an id
   already in use would share that lesson's progress.

## Scheduling

Every phase above is required for 1.0 (`ROADMAP.md`). Phases 1–4 have
landed apart from the two recorded corpora (guitar, flute) and the
harmonica's move onto the shared trainer; phase 5 has landed but for
per-instrument Jam Session. rc1–rc2 touch
the same gameplay files, so land each phase in small commits rather than
one long-lived branch.

**The flute's foot joint is configurable**: `Flute` carries a
`FootJoint::{C, B}` (default C), written in the chart's `instrument` and
chosen in "My instruments" alongside the guitar's tuning. It sets the lowest note (C4 or B3), so the fingering table
holds the B-foot-only fingerings (B3, and the low-C♯/B keys used in some
high-register alternates) and the model filters them out on a C foot.

## Later instruments

**Hurdy gurdy** is deferred, not rejected. When it returns, it needs three
things none of the instruments above do:

- a keybox model: lower diatonic and upper chromatic rows of keys, and a
  melody that is monophonic because one key press sets every chanterelle;
- drone subtraction in the pitch filter, since the configured drone pitches
  sound on every frame of the song;
- an onset-judged note kind for the trompette, whose buzz is broadband and
  scored by rhythm rather than pitch.

It also needs the instrument's own facts first: its key (G/C or D/G), how
many keys, which drones, and whether it has a trompette with a chien.
