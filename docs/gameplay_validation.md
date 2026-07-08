# Gameplay Loop Validation

A structured, repeatable check that the main gameplay loop still works across all
supported modes — **Play 2D**, **Play 3D**, and **Jam Session** — through startup,
pause, resume, and exit. Run this when changing anything in `src/gameplay/`,
`src/menu/`, the audio pipeline, or song/asset formats.

Each item lists its **expected outcome** and whether it's **automated** (a unit
test guards it) or **manual** (needs a run, because it depends on rendering,
audio output, or loaded assets that can't be asserted headlessly).

## How to run

```bash
cargo test          # automated coverage below
cargo run           # manual checks below (needs a mic, audio out, a display)
```

Navigation to gameplay: **Play → Play Song → (2D | 3D) → artist → song**, or
**Play → Jam Session → artist → song**. In-game keys: **Esc** pause/resume,
**M** metronome mute, **V** cycle spectrogram.

## Automated coverage (`cargo test`)

| Behavior | Test |
|---|---|
| Menu pages open/close in the correct order | `menu::tests::navigation_graph_opens_and_closes_to_the_right_pages`, `…::changing_page_exits_the_old_before_entering_the_new` |
| Esc pauses then resumes (flag + overlay) | `gameplay::pause_menu::tests::escape_pauses_then_resumes` |
| Resume button stays in gameplay | `…::resume_button_unpauses_without_changing_state` |
| Restart reloads the song | `…::restart_button_reloads_the_song` |
| **Quit Song returns to the song list** | `…::quit_song_returns_to_the_song_list` |
| **Play 3D restores the 2D camera on exit** | `gameplay::gameplay_3d::tests::leaving_3d_restores_the_2d_camera` |
| Scene teardown despawns only `GameplayRoot` | `gameplay::tests::cleanup_despawns_only_gameplay_entities` |
| Scoring / sustain / techniques / pitch logic | `gameplay::scoring::tests`, `gameplay::tests::*`, `audio_system::*` |
| Clock re-anchors to the audio sink, clamped so it can't jump | `gameplay::tests::advance_clock_*` |
| Detector range derives from the harmonica layout (low-keyed harps not cut off) | `song::harmonica::tests::frequency_range_*`, `audio_system::pitch_detect::tests::pitch_range_from_freqs_*` |
| Vibrato/wah bonus requires the measured rate to match the chart's `oscillation_hz` | `gameplay::scoring::tests::measured_oscillation_hz_*`, `…::oscillation_matches_rate_*`, `gameplay::tests::technique_confirmed_rejects_*_at_the_wrong_rate` |
| Overlapping same-pitch notes credit the one actually due, not query order | `gameplay::tests::score_notes_credits_the_closest_offset_when_two_same_pitch_notes_overlap` |
| End-to-end: a synthetic pitch stream drives a mini chart through hit/good/miss and the score/combo/stats update accordingly | `gameplay::tests::end_to_end_synthetic_song_drives_score_combo_and_stats` |
| Loop boundary rewinds the clock and resets only the notes inside the loop range | `gameplay::tests::loop_boundary_rewinds_the_clock_and_resets_notes_in_range`, `…::loop_boundary_is_a_no_op_before_end_time_or_when_inactive` |
| Windowed note-visual spawning: a note's window opens/closes at the right time, already-spawned notes aren't respawned, far-out notes are excluded | `gameplay::tests::notes_needing_spawn_*` |

## Manual checks

### All modes — entering `Playing`
- [ ] **Audio starts.** After the 3-2-1 countdown, the backing track plays. *(manual: audio output)*
- [ ] The HUD score/combo reads `0` and updates as you hit notes. *(the
  underlying score/combo/stats math is now covered end to end by
  `gameplay::tests::end_to_end_synthetic_song_drives_score_combo_and_stats`;
  this check is now just about the HUD actually *displaying* those numbers —
  manual: rendering)*
- [ ] No errors/panics in the console while the gameplay chain runs. *(manual)*
- [ ] **Long-song sync**: play a 3+ minute song end to end; the hit line still matches the beat at the end, with no accumulating drift. *(manual: audio + timing; correction math is unit-tested but real decoder/frame-hitch drift isn't)*
- [ ] **Low-keyed harp detection**: load (or author) a chart with a Low-F/Low-D harmonica and confirm hole-1 blow/draw register — the detector range now derives from the chart's layout instead of a fixed 200 Hz floor. *(manual: needs a real low-keyed harp and mic)*
- [ ] **Looping doesn't speed up the music.** With `chart.loop.repeat = true` (or any future A–B loop UI), let the loop wrap a few times: the backing track should audibly stay in sync with the notes/metronome, not creep faster each pass. *(manual: needs a live `AudioSink`; the clock-rewind and note-reset logic is unit-tested — see the table above — but `AudioSink::try_seek` itself needs real audio output)*
- [ ] **Looped notes replay correctly on screen.** On a loop wrap, notes inside the loop range should reappear and be hittable again exactly as on the first pass (not stuck showing a stale hit/miss tint, not missing, not duplicated). *(manual: rendering; the note-reset data is unit-tested, but note visuals are now spawned/despawned dynamically rather than kept alive across the whole song, so this is the one loop behavior that can only be confirmed by watching it happen)*
- [ ] **Wait-for-Note mode freezes and resumes cleanly (Play 2D/3D only — Jam Session has no chart notes to freeze on).** Esc into the pause menu, click "⏸ Wait for Note" to turn it on, Resume: the note highway should stop scrolling and the music should stop the instant an unhit note reaches the hit line, both holding steady (not stuttering, not slowly drifting) until you actually play the correct pitch — at which point it scores and everything resumes at normal speed immediately. Toggle it off mid-song and confirm notes go back to missing normally if you don't hit them in time. *(manual: needs a live `AudioSink`; the freeze condition itself is unit-tested — `gameplay::tests::note_due_and_unresolved_*` — but only a live run shows the clock/audio actually holding steady rather than stuttering)*

### Play 2D
- [ ] The note **highway shows falling notes** in the ten lanes, with the comet head + animated tail. *(manual: rendering)*
- [ ] The **HUD** (song info, 12-bar grid, metronome, technique legend, score) is visible on the right. *(manual)*
- [ ] Notes recolour gold on hit / red on miss; long notes reward holding the pitch. *(manual; sustain logic is unit-tested)*
- [ ] **Notes appear and disappear cleanly** — no pop-in right at the top of the highway, no note lingering after it's scrolled off, no duplicate/ghost note. *(manual: rendering; note visuals are now spawned in a `LOOKAHEAD` window rather than all up front — the windowing math is unit-tested (`notes_needing_spawn_*`), but only a live run shows whether it's visually seamless)*

### Play 3D
- [ ] The 3D scene initializes: lane floor, hit zone, the harmonica model, and comet notes travelling down the lane. *(manual: rendering + GLB asset)*
- [ ] **Leaving 3D restores the 2D menu camera** (menu renders normally afterward — order/clear reset). *(automated; verify visually too)*
- [ ] Exiting despawns the 3D scene (no leftover meshes/cameras on the next song). *(teardown automated; verify visually)*
- [ ] **Notes appear and disappear cleanly**, same windowed-spawning check as 2D above. *(manual: rendering)*

### Jam Session
- [ ] Starts and runs **without falling notes** — the 12-bar chart + metronome drive it; no chart-note highway is required. *(manual)*
- [ ] Does not transition to the Results screen on its own (it has no finite song end). *(manual)*
- [ ] **Toggling Loop keeps the music playing.** Click "Loop" on and off a few times, including after the jam has run past one full play-through of the backing track: the music should keep playing (looping when on, stopping only naturally when off), never go silent. *(manual: needs a live `AudioSink`. Four earlier attempts all despawned the live `MusicPlayer` entity and respawned it mid-track in reaction to the toggle, and all four went silent for various reasons (see TODO.md) — resuming from the gameplay clock, resuming from the sink's own `position()`, an explicit `.with_start_position(Duration::ZERO)` restart, and even dropping `.with_start_position()` entirely. The actual fix is `restart_finished_jam_music`: Jam Session's music always spawns as `PlaybackSettings::DESPAWN` (self-despawns on completion), the toggle itself does nothing, and a new entity is only ever spawned after the previous one is already gone and Loop is on at that moment — never touching a live sink at all. Treat any future logic that reacts to the toggle by touching the *current* sink as regression-prone until proven otherwise)*

### Pause / resume / exit (all modes)
- [ ] **Esc pauses**: gameplay freezes, the music pauses, the PAUSED overlay appears. *(automated for flag/overlay; audio manual)*
- [ ] **Esc again resumes**: overlay hides, music resumes, gameplay continues. *(automated for flag/overlay)*
- [ ] **Resume** button behaves like Esc-resume. *(automated)*
- [ ] **Restart** reloads the same song from the countdown. *(automated for the state change)*
- [ ] **Quit Song** returns to the **song list** (not the main menu). *(automated)*

## Acceptance criteria → coverage

- Main gameplay chain runs without errors in the observed modes — *manual run + automated state/teardown tests.*
- Play 2D shows HUD and active notes — *manual (rendering).*
- Play 3D initializes the scene and restores state when leaving — *camera restore + teardown automated; scene init manual.*
- Jam Session functional without a traditional chart — *manual.*
- Esc pauses and resumes correctly — *automated (flag + overlay); audio manual.*
