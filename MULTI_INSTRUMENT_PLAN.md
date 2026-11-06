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
- Steps 1–7 remain planned; no new instrument identity is enabled yet.
