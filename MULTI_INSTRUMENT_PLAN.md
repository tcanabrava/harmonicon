# Multi-instrument architecture plan

## Goal

Make new instruments inexpensive to add without losing their musical identity.
Share mechanics through composition, keep instrument-specific teaching and
curricula, and preserve existing charts and saved setups. Use typed Rust models;
a dynamic plugin system is not required.

## Architecture

1. **Identity and setup**: stable instrument IDs, names, curricula, presets and
   versioned saved configurations. Bass and cavaquinho remain distinct identities.
2. **Physical models**: fretted strings, keyed instruments, fingering-based winds,
   and bowed/fretless strings. Own pitch mappings and physical constraints.
3. **Notation**: written versus sounding pitch, spelling, key signatures, clefs,
   tablature and staff assignment. Detection and synthesis use sounding pitch.
4. **Teaching presentation**: instrument diagrams, selection tools, guidance and
   exercise-specific views. Presentation adapters consume capabilities instead
   of spreading instrument-name checks throughout editor and gameplay.
5. **Assessment**: attack/decay and sustained-note behavior, pitch and timing,
   intonation and confidence. Microphone evidence cannot prove a particular
   finger, string, posture or bow direction.

Keep tunings, verified fingering tables and diagram geometry in validated data
where appropriate. Keep algorithms and assessment behavior in code. Separate
physical possibility from beginner suitability and ergonomic guidance. Move
highway layout decisions toward presentation rather than physical identity.

## Ordered implementation

Each independently reviewable part receives a commit and relevant validation.

### 0. Song catalog instrument filter

- Add a keyboard-accessible combobox with All instruments and only instrument
  identities represented by available songs, deduplicated across tunings/parts.
- Compose with search, sorting and practice-history filters.
- Update choices after catalog changes; recover when a selected instrument
  disappears. Respect the harmonica-only Jam Session catalog.

### 1. Extract shared fretted-string mechanics

- Extract string tuning, fret/capo geometry, pitch calculation and position
  enumeration from Guitar into a reusable model.
- Retain Guitar's public behavior and chart representation through adapters.
- Keep guitar technique limits, chord dictionaries, presets and teaching policy
  outside generic geometry. Avoid assuming tuning order or six strings.
- Test alternate string counts, re-entrant tuning, capo and boundary behavior,
  plus existing guitar compatibility.

### 2. Add bass using the fretted-string model

- Distinct identity, tunings/setups, editor, trainer, gameplay and curriculum
  selection; reuse fretboard and tablature infrastructure.
- Correct notation conventions, attack/decay guidance and low-register detection
  checks. Confirm supported bass scope (initially fretted electric bass).

### 3. Add cavaquinho using the fretted-string model

- Explicit tuning traditions/presets rather than one supposedly universal tuning.
- Reuse fretboard tools; provide appropriate chord vocabulary and strumming
  exercises. Verify fingering ergonomics independently of guitar defaults.

### 4. Generalize notation and wind fingering support

- Explicit written-to-sounding pitch transformation, including spelling and key
  signatures, consistently across score, editor, auditions and detection.
- Generic fingering controls/diagram metadata with instrument-specific verified
  tables and alternate fingerings; retain flute setup and behavior.

### 5. Add clarinet

- Start with an explicitly named setup/system; support its own key diagram,
  register exercises, transposition and fingerings. Do not reuse flute key bits
  or assume flute register relationships.

### 6. Add bowed-string mechanics and violin

- Fretless positions: string, target pitch, suggested finger and hand position,
  alternate locations. Bow articulation/direction independent of left hand.
- Continuous placement and intonation feedback; no visible invented frets.
- Physical simultaneous-note constraints and sustained assessment need their
  own policies. Verify detection with recordings/real playing.

### 7. Add viola using bowed-string support

- Instrument-specific tuning/range, alto clef (and clef changes as required),
  fingering guidance and teaching content.

## Validation and compatibility

- Preserve existing serialized charts, tuning setups and instrument IDs.
- Verify pitch/position mappings and representative editor/trainer/gameplay flows.
- Check localization, keyboard operation, diagram hit testing and screen layout.
- Validate lesson packs, namespaces and instrument-first curriculum loading.
- Distinguish deterministic model tests from real-audio evaluation; do not claim
  physical technique detection based on pitch recognition alone.

## Progress

- Plan recorded.
- Step 0 complete: catalog-derived instrument combobox, localized All instruments,
  composed filtering, catalog-change recovery and dropdown keyboard isolation.
  Menu suite: 80 tests passed.
- Step 1 complete: reusable borrowed `FrettedStrings` geometry and `FretPosition`;
  Guitar delegates geometry while preserving its setup fields, chart format,
  techniques and ergonomic policy. Four-/seven-string, re-entrant tuning and
  capo/boundary cases covered; core suite: 429 tests passed.
- Clippy passed for core and menu, all targets, with warnings denied.
- Step 2 complete: fretted electric bass identity, four-/five-/six-string and
  Drop D presets, custom string tuning, shared editor/trainer/gameplay, independent
  saved tuning and trainer progress, catalog and curriculum selection.
  Runtime `Instrument::Fretted(FrettedInstrument)` separates the shared physical
  family from `InstrumentKind::{Guitar,Bass}`. Legacy guitar chart JSON is retained.
- Bass uses octave-transposing bass notation, with a treble fallback for high
  targets. Scoring and audition pitches remain sounding pitches. Guitar chord
  dictionaries and finger suggestions are not applied to bass.
- Bass validation: 2,100 relevant library tests passed; final editor/menu/UI
  checks passed, workspace Clippy passed with warnings denied, dev build and
  documentation build passed. Low B0/E1 synthetic signals with harmonics and
  decay detected by all five algorithms at 44.1/48 kHz. Physical bass pickup/mic
  recordings remain a separate evaluation.
- Live UI checked: low/high trainer targets, catalog filtering, pre-song setup,
  gameplay, and the editor including changing to the five-string preset.
- Bass charts use song format 1.11.0; bass lesson manifests require lesson format
  3. Older charts/curricula remain supported; no bass curriculum is bundled yet.
- Step 3 complete: cavaquinho identity, named Brazilian traditional/natural
  and violin/mandolin tuning presets, independent saved setup, shared editor,
  trainer, catalog and curriculum selection. Starter G/C/D/Em chord shapes use
  four-string diagrams and are offered only in their declared tuning.
- Fretted tuning metadata now accepts arbitrary string counts; shared saved
  tuning accessors and preset lookup prevent instrument-specific UI duplication.
  Technique support is queried from the model instead of inheriting guitar tools.
- Cavaquinho validation: 1,753 relevant library tests passed; workspace Clippy,
  dev build and documentation build passed. Live four-string chord selection and
  editor setup checked. Sounding tuning octaves verified against cited teaching
  references. Song format 1.12.0 / lesson format 4; older content remains supported.
- Step 4 core foundation complete: explicit spelled pitches, diatonic/chromatic
  written-to-sounding transposition, key signatures and enharmonic key handling.
  Written notation can extend beyond the detector's bounded MIDI range.
- Shared wind controls, masks, stable fingering variants and register-interval
  rules added. Flute adapts its existing data without changing foot-joint range
  or fingerings. Core suite: 439 tests passed; core Clippy passed.
- Step 4 score projection is wired into gameplay, the editor and both trainers.
  Notes use written spelling and the written key signature; playback and
  assessment retain sounding pitches. Key-aware accidentals reset on the actual
  bar grid, including pickups and meter changes. Authored gameplay spellings
  survive repeat expansion and bar-line ties when they match the sounding target.
- Live score review checked low/high bass notation and a six-sharp editor key.
  The high bass fallback retains its octave-below indication. Accidental widths
  were measured from the bundled font rather than approximated.
- Step 4 validation: editor 466 tests, gameplay 460 tests and UI 193 tests
  passed, including enharmonic staff placement, key cancellation/reset,
  meter-change resets and the authored-spelling bridge. Relevant Clippy checks
  passed with warnings denied, and the dev build passed.
- Step 5 model foundation: a Boehm soprano clarinet fingering chart for B♭
  and A setups, explicitly limited to written E3–C6. Common left/right little-
  finger alternatives have stable indices; controls represent finger contacts,
  including separate thumb-hole and register-key controls.
- Clarinet basic fingerings were checked against the Woodwind Fingering Guide’s
  [chalumeau](https://www.wfg.woodwind.org/clarinet/cl_bas_1.html) and
  [clarion](https://www.wfg.woodwind.org/clarinet/cl_bas_2.html) tables. Its register
  rule is a twelfth, not the flute’s octave; throat B♭ is excluded from that drill.
- Wind practice now builds target, alternate-fingering and register drills from
  `FingeringChart` metadata. Flute uses this shared path while retaining every
  previously verified register pair and alternate. Clarinet drill tests cover
  both setups and their twelfth interval without a flute-specific branch.
- Core validation: 447 tests passed and core Clippy passed with warnings denied.
  Clarinet chart/setup selection, editor, trainer and gameplay integration are
  still pending; clarinet is not yet offered as a selectable instrument.
- Steps 6–7 remain planned.
