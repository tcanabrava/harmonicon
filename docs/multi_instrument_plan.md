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
    fn range(&self) -> (u8, u8);                         // feeds PitchRange
    fn positions_for(&self, midi: u8) -> Vec<PlayPosition>;  // generalised pitch_map
    fn pitch_of(&self, p: &PlayPosition) -> Option<u8>;      // generalised hole_notes
    fn voices(&self) -> VoiceModel;                      // what can sound together
    fn techniques(&self) -> &[TechniqueKind];            // see Note Trainer
    fn expression_scope(&self) -> ExpressionScope;       // whole-instrument vs per-voice
    fn envelope(&self) -> Envelope;                      // Sustained | Decaying
}

enum PlayPosition {
    Hole { hole: u8, action: Action, technique: Technique },
    Fret { string: u8, fret: u8, technique: StringTechnique },
    Fingering { id: FingeringId, harmonic: u8 },          // flute: fingering + register
}
```

These belong in core, not a new crate: `chart` has to name the instrument,
and chart lives in core. The harmonica implementation is an adapter over the
existing `harmonica`/`pitch_map` code, not a rewrite.

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

- New optional top-level `instrument`. If it is absent, the chart is a
  harmonica chart and `harmonica` is the instrument, so every existing chart
  loads unchanged.
- New optional `NoteEvent.position`, e.g. `{"string": 5, "fret": 3}` or
  `{"fingering": "C5", "harmonic": 1}`.
- For non-harmonica instruments `note` (the pitch) is **required** and is
  the truth. `hole`/`action` become optional, and the schema requires them
  only when the instrument is a harmonica.
- `Modifier` gains guitar techniques: `hammer_on`, `pull_off`, `slide_to`,
  `harmonic` and `palm_mute`. The existing `bend` and `vibrato` are reused
  as they are.
- **One chart is one instrument part.** A song folder may hold several
  charts, e.g. `song/harmonica.harpchart` and `song/guitar.harpchart`. This
  keeps the schema flat and lets a pack ship a guitar part for an existing
  song without touching its harmonica chart. It changes the "first
  `*.harpchart` under `song/`" rule in both `assets_management` and
  `generate_bundled_asset_manifest`, which must stay in step. Keep the
  `.harpchart` extension: the name is now historical, but it is the play
  format (see the import-source rule) and renaming it costs every pack.

Older engines refuse a 1.8.0 chart through `format_version_supported`
rather than misreading it.

## Pitch detection per instrument

The detectors stay instrument-agnostic, as they are now. What changes is
the instrument-aware stage after them (`HarmonicaPitchFilter` /
`HarmonicaNoteTracker`), which becomes `InstrumentPitchFilter`, driven by
the voice model:

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
| Flute | **pitch lanes** limited to the pitches the chart actually uses (three chromatic octaves would make 37 lanes), labelled by note name with a register mark | a **fingering diagram** (Boehm key layout) for the upcoming note, plus the next one ghosted, so the player prepares the fingers before the note arrives |

The guitar's orientation is chosen so the screen teaches the shape players
will meet in real tablature everywhere else. It means `LaneLayout` carries
a scroll axis from the first phase, even though only the harmonica uses it
then.

Each layout is one module, e.g. `gameplay/lanes/{harmonica,strings,pitch}.rs`.
The note-spawning half of `gameplay_2d` asks the layout where a note goes
and never asks which instrument it is.

## Song Editor

- **Rows come from the instrument.** Harmonica: holes × direction, as
  today. Guitar: 6 tab rows with the fret number drawn on each note. Flute:
  a monophonic piano roll.
- **`enforce_direction` becomes `VoiceModel::reconcile(group)`**, run on the
  same `overlapping_group`:
  - harmonica: propagate the direction (today's behaviour);
  - guitar: moving a note onto an occupied string re-voices it to the next
    string that reaches the pitch, or refuses if none does; a chord beyond
    hand span is flagged;
  - flute: an overlapping note truncates the previous one.

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
  flute).

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

## Content, progress and menus

- **Packs declare their instrument** in `pack.json`, with a
  `LESSON_FORMAT_VERSION` bump. Each instrument gets its own lesson pack
  and technique DAG (`harmonicon-lessons-guitar`, …). The existing
  100-lesson pack is the harmonica's.
- **A profile is per instrument.** Song records are keyed by part,
  lesson/training records by pack, and the practice streak stays global
  (you practised today, whatever you played). The skill tree shows the
  selected instrument's DAG.
- **"My instruments"** generalises the harp-check page's
  bring-your-own-harp choice. The player lists what they own, with tunings.
  Song select shows which parts they can play, and the harp-check page
  becomes an instrument check before play.
- **Audition and call-and-response synthesis** need a timbre per
  instrument in core's `synth`:
  - Karplus–Strong for plucked strings: cheap and convincing;
  - a sine plus filtered breath noise for the flute.

  The phase-integration vibrato rule applies to both.
- **Jam Session** generalises later. `hole_map` becomes "where the scale
  lives on this instrument": pentatonic boxes on guitar, scale fingerings on
  flute.

## Phases

Each phase lands and is committed on its own. Phase 1 must not change
behaviour.

1. **The seam, harmonica only.** Add `Instrument`, `InstrumentModel`,
   `PlayPosition`, `VoiceModel` and `LaneLayout` (with its scroll axis),
   with the harmonica as the only implementation. Route `pitch_map`, the
   constraint filter, the editor's reconcile, `ScheduledNote` (position +
   lane instead of `hole`/`is_blow`) and `highway_2d` through them.
   Existing tests plus pixel-diffed captures (`capture_static_screens.py`)
   prove nothing moved.
2. **Chart 1.8.0.** Add the `instrument` field, `position`, the
   conditionally-required `note`, multi-part song folders and validator
   support. Add fixtures for each instrument.
3. **Guitar** (two of the three instruments, and the largest library via
   Guitar Pro):
   - model and tunings;
   - fingering solver;
   - Guitar Pro string/fret pass-through;
   - horizontal tab highway and fretboard strip;
   - editor tab rows;
   - Karplus–Strong synth;
   - a recorded corpus, then detection tuning (low-string resolution,
     chord onset spread, decaying sustain).
4. **Note Trainer.** Generalise the bending trainer onto `TechniqueKind`,
   with guitar bends as the second `Glide` implementation (which makes the
   abstraction honest), and add `Target` for every instrument.
5. **Flute.** Monophonic model, the Boehm fingering table as data
   (standard, alternates, harmonics), pitch lanes and fingering diagram,
   the `Register` trainer mode, and a corpus aimed at octave errors.
6. **Content and progress.** Per-instrument packs and DAGs, a
   per-instrument profile, the "My instruments" page, and per-instrument
   Jam Session.

## Decide before building

- **Timing relative to 1.0.** 1.0 is a desktop harmonica release
  (`ROADMAP.md`). Phase 1 is a refactor with no visible change and could
  happen at any point; phases 3 onwards are a new product surface.
  Recommendation: phase 1 after 1.0-rc2, because rc1–rc2 touch the same
  gameplay files.
- **The flute's foot joint** (C or B), which sets its lowest note and the
  fingering table's range.

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
