# Glossary

Short definitions for terms this book uses repeatedly, each pointing at
the chapter that covers it in depth. Not exhaustive — see each chapter's
own prose for anything not listed here.

**`AppState`** — the top-level screen state machine (`Startup`, `Menu`,
`SongLoading`, `Playing`, `Results`, `Calibration`, `Credits`,
`SongEditor2`, `BendingTrainer`). See
[Application States and Modes](app-states.md).

**`AssetLoader`** — Bevy's mechanism for turning raw file bytes into a
typed `Asset`, run asynchronously off the main thread. Harmonicon has
two custom ones: `SongChartLoader` (a chart folder → `SongManifest`)
and `ThemeJsonLoader` (`theme.json` → `ThemeJson`). See
[Chart Format and Asset Loading](chart-and-assets.md) and
[Localization and Theming](localization-and-theming.md).

**`EditorState`** — the Song Editor's single resource holding its
entire in-memory document: every placed note, the tempo map, meta-form
fields, selection, and mode. See [The Song Editor](
song-editor-architecture.md).

**`GameplayClock`** — the single authoritative "now" every scored-
gameplay system reads, anchored to the music sink's own playback
position once music starts rather than tracking wall-clock time
directly. See [The Gameplay Clock](gameplay-clock.md).

**`GameplayMode`** — which of two experiences `AppState::Playing`
currently means: `Play2D` or `JamSession`. See
[Application States and Modes](app-states.md).

**`GameplayLogic`** — the Bevy `SystemSet` grouping the clock tick,
scoring, and loop-boundary systems; every clock-reading system must be
ordered `.after` it. See [The Gameplay Clock](gameplay-clock.md) and
[The Plugin Architecture](plugin-architecture.md).

**`GridNote`** — the Song Editor's own per-note type (distinct from
gameplay's `ScheduledNote`): `{ id, hole, tick, len, dir, pitch, expr }`.
See [The Song Editor](song-editor-architecture.md).

**`HarpChart`** — the parsed, typed representation of a `.harpchart`
JSON file. See [Chart Format and Asset Loading](chart-and-assets.md).

**`LessonManifest`** — a lesson's authored content (`lesson.json`):
identity, prerequisites, pass criteria, and Fluent key references (never
raw display text). See [The Lessons Engine](lessons-engine.md).

**`LessonContext`** — the resource kept in flight for the duration of a
lesson run, read by the results screen (or, for a jam-based lesson, the
pause menu's "Finish Lesson" button) to judge the run's `pass_criteria`.
See [The Lessons Engine](lessons-engine.md).

**`LoadedTheme`** — the resource holding the currently active UI
theme's resolved colors and asset handles, populated asynchronously once
its `ThemeJson` asset load resolves. See
[Localization and Theming](localization-and-theming.md).

**`MenuPage`** — the Bevy `SubStates` enum scoped to `AppState::Menu`,
one variant per menu screen. See
[Application States and Modes](app-states.md).

**`BackingStemAudio`** / **`BackingStemPlayer`** — `BackingStemAudio` (on
`SongManifest`) is one independently rendered `AudioSource` stem, whether it
came from MIDI or the generated rhythm section; `BackingStemPlayer(usize)`
(a component, in `gameplay::state`) tags the live `AudioSink` entity playing
that stem so per-stem mute can find it.
See [Jam Session](jam-session-architecture.md).

**`MusicPlayer`** — the component tagging whichever entity is currently
playing a song's background music, used by pause/resume and global-
volume-slider systems to find it (or, for MIDI multi-track backing, all
of them at once). See [Jam Session](jam-session-architecture.md).

**`PitchEvent`** — the `Message` published once per analyzed audio
chunk, carrying every pitch detected in it. See
[The Audio Input Pipeline](audio-pipeline.md).

**`PitchGate`** — the fresh-attack gate (wrapping `scoring::
AttackGate<u8>`) that keeps a single sustained note from being
re-credited to multiple chart notes at the same pitch. See
[The Scoring System](scoring-system.md).

**`PitchRange`** — the current min/max frequency pitch detection
searches, narrowed to the active harmonica's real playable range at
song start (rather than a fixed global range) to reduce false positives
and, for the NMF algorithm, to know which dictionary to build. See
[The Audio Input Pipeline](audio-pipeline.md).

**`Plugin` (composition root)** — a plugin, like `gameplay::plugin`,
whose entire job is assembling other features' systems into one shared
schedule — legitimately depends on everything it wires together, unlike
an ordinary feature module. See
[Module Boundaries and Dependency Rules](module-dependency-rules.md).

**`ScheduledNote`** — one chart note's live score state during real
gameplay, held in `SongNotes` as plain data, independent of whatever
(if any) render entity currently represents it on screen. See
[The Scoring System](scoring-system.md).

**`SnapMode`** — the Song Editor's Straight/Shuffle/Triplet grid
subdivision setting, constraining where a note placement or drag lands.
See [The Song Editor](song-editor-architecture.md).

**`SongManifest`** — the fully-loaded representation of a song: its
`HarpChart` plus every resolved (or gracefully defaulted) sibling
asset — background art, backing music or MIDI track stems, note-theme
configs. See [Chart Format and Asset Loading](chart-and-assets.md).

**`SongNotes`** — the resource holding the entire loaded chart's worth
of `ScheduledNote`s plus a scan-avoidance cursor; scoring's actual data
model. See [The Scoring System](scoring-system.md).

**`AttackGate<K>`** — the pure, generic fresh-attack state machine
`PitchGate` and Jam Session's `ImprovStats` both wrap, keyed by whatever
identity type `K` the caller needs (a MIDI note number for scoring). See
[The Scoring System](scoring-system.md).
