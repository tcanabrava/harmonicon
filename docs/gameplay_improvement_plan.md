# Scored play: remaining experience work

The scored-play HUD, contextual panels, judgment feedback, practice controls, results coaching, and accessibility work have shipped. `docs/gameplay_validation.md` covers the manual checks; `PLAN.md` tracks open decisions.

## Visual decisions

- Decide whether technique symbols belong beside note heads. The note-head label deliberately omits bend, overblow, and slide suffixes today; the tab ribbon carries that detail.
- At the supported 1920×1080 desktop floor, check that the next notes, hit line, expected hole and direction, score, and pause control remain legible without overlap.

## Manual timing acceptance

Play live to confirm that the mid-hold drop-out and steady-versus-wobbling vibrato receive understandable feedback, and that judgment feedback appears on the same visual beat as the judged note. Headless tests cover the judgment categories, but cannot establish perceived timing with a microphone and screen.

## Conditional additions

A graded `Sustain` outcome requires an explicit scoring change; sustain currently scales points continuously. Test optional judgment sounds with microphone capture before adopting them, since speaker output can contaminate detection. Re-tune Android touch targets and `CompactLayout` on real hardware.

## Implementation boundaries

- Keep `GameplayClock` as the only time authority. Renderers may map its value
  to geometry but may not advance or reinterpret time.
- Keep scoring primitives in `harmonicon-core` and the ECS driver in
  `judge.rs`. UI consumes judgment messages and never decides hits itself.
- Derive contextual UI from authored chart data. Do not infer a blues form from
  the song key.
