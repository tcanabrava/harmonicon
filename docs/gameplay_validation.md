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

## Manual checks

### All modes — entering `Playing`
- [ ] **Audio starts.** After the 3-2-1 countdown, the backing track plays. *(manual: audio output)*
- [ ] The HUD score/combo reads `0` and updates as you hit notes. *(manual)*
- [ ] No errors/panics in the console while the gameplay chain runs. *(manual)*
- [ ] **Long-song sync**: play a 3+ minute song end to end; the hit line still matches the beat at the end, with no accumulating drift. *(manual: audio + timing; correction math is unit-tested but real decoder/frame-hitch drift isn't)*
- [ ] **Low-keyed harp detection**: load (or author) a chart with a Low-F/Low-D harmonica and confirm hole-1 blow/draw register — the detector range now derives from the chart's layout instead of a fixed 200 Hz floor. *(manual: needs a real low-keyed harp and mic)*

### Play 2D
- [ ] The note **highway shows falling notes** in the ten lanes, with the comet head + animated tail. *(manual: rendering)*
- [ ] The **HUD** (song info, 12-bar grid, metronome, technique legend, score) is visible on the right. *(manual)*
- [ ] Notes recolour gold on hit / red on miss; long notes reward holding the pitch. *(manual; sustain logic is unit-tested)*

### Play 3D
- [ ] The 3D scene initializes: lane floor, hit zone, the harmonica model, and comet notes travelling down the lane. *(manual: rendering + GLB asset)*
- [ ] **Leaving 3D restores the 2D menu camera** (menu renders normally afterward — order/clear reset). *(automated; verify visually too)*
- [ ] Exiting despawns the 3D scene (no leftover meshes/cameras on the next song). *(teardown automated; verify visually)*

### Jam Session
- [ ] Starts and runs **without falling notes** — the 12-bar chart + metronome drive it; no chart-note highway is required. *(manual)*
- [ ] Does not transition to the Results screen on its own (it has no finite song end). *(manual)*

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
