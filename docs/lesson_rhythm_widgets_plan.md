# Lesson rhythm widgets

Add two reusable teaching aids to lesson reader pages. Rendering and pure
timing/layout models live in `harmonicon-ui`; manifests live in
`harmonicon-song`; lesson-specific state, controls, and lifecycle live in
`harmonicon-lessons`.

## Rhythm pattern visualizer

- [x] Add a reusable subdivision-strip widget with authored beat labels,
  rests, accents, and active-step highlighting.
- [x] Add a schema-validated `rhythm-pattern` lesson widget and reader
  controls for previous, next, and reset.
- [ ] Add it to rhythm lessons where seeing the subdivision or silence is
  part of the instruction.

## Phrase looper

- [ ] Add a reusable phrase-strip widget and a clock that wraps its active
  step over an authored A/B range.
- [ ] Add a schema-validated `phrase-looper` lesson widget and reader
  controls for play/pause, previous, next, reset, and tempo.
- [ ] Add it to lick and loop-isolation lessons with concise authored phrase
  cells rather than coupling the reader to song playback.

## Completion

- [ ] Cover schema rejection, timing/wrap behavior, and reader lifecycle;
  update authoring and gameplay-validation documentation.
- [ ] Run formatting, focused tests, Clippy, and the bundled-manifest checks.

Each checked point is committed separately.
