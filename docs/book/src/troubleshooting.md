# Troubleshooting

## No microphone detected

If Options shows a warning banner instead of picking up sound:

1. Confirm your OS actually sees the microphone (check your system's sound
   settings outside Harmonicon first).
2. Open **Options → Microphone** and try selecting a different device from
   the dropdown — some systems expose the same physical mic under several
   names (e.g. a generic "default" entry alongside the specific hardware
   name), and only one of them may actually be live.
3. If you plug in or unplug a mic while Harmonicon is running, revisit this
   dropdown afterward — the device list doesn't always refresh itself.

## Notes aren't registering, or the wrong pitch registers

- Try a different **pitch-detect algorithm** (Options → Pitch detect).
  Different algorithms trade off differently between low-note accuracy,
  responsiveness, and noise tolerance — if one struggles with your
  particular mic/harmonica/room combination, another often won't.
- Play closer to the microphone, and closer to the harmonica than to your
  mouth — breath noise close to a laptop mic is a common source of bad
  reads.
- Use headphones instead of speakers. Backing-track or metronome audio
  leaking back into the mic can be misread as a note.
- Very low-pitched harmonicas (low-keyed diatonics, the low end of a
  chromatic) need a detector range wide enough to cover them — this is
  handled automatically per-chart, but if a specific low note never
  registers, double check the chart's declared harmonica key matches the
  harp you're actually playing.

## Timing feels consistently early or late

Run [Calibrating Input Lag](calibration.md) once — a mismatched Input Lag
offset shows up as every hit reading a bit early or a bit late, even when
your actual playing is on the beat. The results screen after any scored
song also offers a one-click "apply the measured offset" button if you
notice this without visiting the calibration screen directly.

## A chart won't load

Harmonicon checks a chart's declared `metadata.format_version` against
what the current build's loader understands, and refuses to load anything
declaring a version *newer* than that, with a clear error naming both
versions — rather than failing on a confusing, unrelated error further
into loading. If you see this, update Harmonicon, or double-check the
chart's `format_version` field wasn't set higher than it should have been.

## The game feels stuttery or slow

- Practice Speed (pause menu, 50%–100%) slows the highway/metronome down
  without pitch-shifting audio — useful for a genuinely hard passage, but
  won't fix an actual performance problem.
- Play 3D renders more than Play 2D; if performance is the issue rather
  than difficulty, try [Play 2D](play-2d.md) instead.
