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
| Adaptive difficulty: phrase grouping, unlock-fraction curve, per-note unlock/section tagging, learned-fraction bump on a clean clear | `gameplay::adaptive_difficulty::tests::*` |
| Pause-menu phrase selector/adaptive-difficulty labels and stepping | `gameplay::pause_menu::tests::next_phrase_index_*`, `…::prev_phrase_index_*`, `…::phrase_selector_text_*`, `…::adaptive_difficulty_label_*`, `…::adjust_learned_*` |
| Progress-bar phrase-section rectangle geometry/color | `gameplay::song_progress_overlay::tests::phrase_rect_geometry_*`, `…::phrase_fill_color_*` |

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
- [ ] **Looping doesn't speed up the music.** With `chart.loop.repeat = true` or the pause-menu A–B loop, let the loop wrap a few times: the backing track should audibly stay in sync with the notes/metronome, not creep faster each pass. *(manual: needs a live `AudioSink`; the clock-rewind and note-reset logic is unit-tested — see the table above — but `AudioSink::try_seek` itself needs real audio output)*
- [ ] **Looped notes replay correctly on screen.** On a loop wrap, notes inside the loop range should reappear and be hittable again exactly as on the first pass (not stuck showing a stale hit/miss tint, not missing, not duplicated). This took two fixes: (1) `update_note_visuals`/`update_note_visuals_3d` only ever tinted a note gold (hit) or red (missed) and otherwise left its current color alone — so a note hit (or missed) right before the wrap, whose visual entity hadn't yet despawned off the bottom of the screen, kept showing that stale tint after `handle_loop_boundary` cleared its `hit`/`missed` flags, instead of going back to its normal blow/draw colour; fixed by always resetting to the base colour in the "neither" case (`gameplay_2d::note_tint`, `gameplay_3d::note_tint_3d`). (2) `handle_loop_boundary` only reset notes strictly inside `start_time..end_time` — but `notes_needing_spawn` previews notes up to `LOOKAHEAD` (3s) ahead of the clock, so a note just *past* `end_time` (reachable as a preview once the clock rewinds close to it, even though the loop itself never actually gets there) kept showing whatever hit/miss tint an earlier, unrelated pass left it in — most visible with a loop range shorter than `LOOKAHEAD`, or one set behind where the song had already played to. Fixed by extending the reset range to `end_time + LOOKAHEAD` (`loop_reset_range`). *(manual: rendering; both fixes are unit-tested — `gameplay_2d::tests::note_tint_restores_*`, `gameplay_3d::tests::note_tint_3d_restores_*`, `gameplay::tests::loop_reset_range_*` — but note visuals are spawned/despawned dynamically rather than kept alive across the whole song, so seeing it actually happen live still needs a real loop wrap, ideally with a short loop range)*
- [ ] **A song whose audio ends before `SongEnd` doesn't stutter-loop near the end.** With a chart whose `music.ogg` is trimmed tight to (or shorter than) the last note (no padding beyond `SONG_END_TAIL` = 2.5s), let it play to completion: once the audio actually finishes, the clock/note-highway/progress-bar playhead should keep moving forward smoothly (frame-timer driven) until the results screen appears — not repeatedly snap backward roughly every half second. (Root cause: a finished `AudioSink`'s `position()` freezes rather than continuing to advance; anchoring the clock to a frozen position drifted the clock past `SNAP_THRESHOLD_SECS` and snapped it back, over and over. Fixed by no longer anchoring once the sink is empty — `gameplay::tick_clock` / `should_anchor_to_sink`.) *(manual: needs a live `AudioSink` with a short/tightly-trimmed track; the gating logic is unit-tested — `gameplay::tests::does_not_anchor_once_the_sink_is_empty` and friends — but the audible/visual stutter itself needs a real run)*
- [ ] **Song-progress bar shows the real waveform.** On entering Playing, the top progress bar should show the song's actual amplitude contour (not a flat bar) immediately — it's pre-analyzed at asset-load time, not decoded during setup. A thin red playhead line sweeps left to right in sync with the music instead of a growing fill. Confirm it degrades gracefully (flat/empty bar, no panic) if `music.ogg` were ever undecodable. *(manual: rendering; the bucketing/downmix math is unit-tested — `audio_system::waveform::tests::*` — but the visual shape is only confirmed by looking at it)*
- [ ] **Note markers under the waveform line up with the notes themselves.** The thin strip below the waveform should show a tiny white rectangle at every chart note's onset, on the same timescale as the waveform and playhead — a dense phrase should look denser, and the markers a note highway hit lines up with visually should be the ones the playhead is crossing at that instant. *(manual: rendering; the timescale math is the same `AudioDuration`-based fraction already unit-tested for the playhead/loop marker, but marker placement itself is only confirmed by looking at it)*
- [ ] **Adaptive difficulty starts sparse and grows with clean clears (Play 2D/3D only).** On a song with no prior profile record, only a sparse subset of each phrase's notes should appear (starting at the phrase's first note(s), not scattered) — the rest of that phrase is simply absent from the highway, not shown-but-unhittable. Clear a phrase cleanly (every note that *did* appear, hit with no misses) through to the results screen, then Restart: more of that phrase's notes should now appear, starting from the beginning of the phrase. Toggle "Adaptive Difficulty" off in the pause menu, Restart, and confirm every note in every phrase now appears regardless of prior progress. *(manual: rendering + a full playthrough; the unlock-fraction curve, prefix selection, and clean-clear bump are unit-tested — see the table above — but only a live run confirms notes actually (dis)appear from the highway)*
- [ ] **Progress-bar phrase strip reflects learned progress.** The song-progress bar should show one small rectangle per musical phrase, below the note-marker strip, colored from dim gray (unlearned) toward green (fully learned). *(manual: rendering; the geometry/color math is unit-tested)*
- [ ] **Manual phrase override (pause menu, Play 2D/3D only).** Esc into the pause menu: a "◀ Section: <name> — Learned: NN% ▶" row should step through the song's phrases, and "-25%"/"+25%" should adjust the selected phrase's learned percentage. Confirm the corresponding progress-bar rectangle re-tints immediately (no need to resume), but the actual note unlock only takes effect after Restart — as the caption beneath the row says. *(manual: rendering + a restart to confirm the unlock effect)*
- [ ] **A–B loop controls (drag on the progress bar, while paused).** Esc into the pause menu: the song-progress bar (waveform + note strip) should stay visible and clickable *through* the pause dimming, not get covered by it. Click-and-drag anywhere on it: a yellow semi-transparent rectangle should sweep out live, covering the bar's full height, following the drag in either direction. Release the mouse — the readout should switch from "Loop: off" straight to "Loop: Ns–Ns" (no intermediate state), and Resume should have the song now loop that exact section (offset + duration from where you dragged, no bar-snapping). A drag with ~zero movement (a near-click) should leave the loop off rather than activating a zero-length range. Click "Clear Loop" and confirm it stops looping and the marker disappears immediately, live, even while a drag preview would otherwise be showing. *(manual: rendering + audio; the cursor→time mapping, drag-direction normalization, and request→`LoopConfig` application are unit-tested — `song_progress_overlay::tests::cursor_to_time_*`, `…::drag_marker_geometry_*`, `…::apply_requested_loop_range_*` — but the actual sink seek needs real audio output)*
- [ ] **Wait-for-Note mode freezes and resumes cleanly (Play 2D/3D only — Jam Session has no chart notes to freeze on).** Esc into the pause menu, click "⏸ Wait for Note" to turn it on, Resume: the note highway should stop scrolling and the music should stop the instant an unhit note reaches the hit line, both holding steady (not stuttering, not slowly drifting) until you actually play the correct pitch — at which point it scores and everything resumes at normal speed immediately. Toggle it off mid-song and confirm notes go back to missing normally if you don't hit them in time. *(manual: needs a live `AudioSink`; the freeze condition itself is unit-tested — `gameplay::tests::first_due_unresolved_note_*` — but only a live run shows the clock/audio actually holding steady rather than stuttering)*
- [ ] **A "Play Hole N ↑/↓" prompt appears while frozen** (`wait_freeze_overlay`), naming the actual due note, and disappears the instant it's hit or the mode is toggled off. Without it a frozen clock looks exactly like a hang. *(manual: rendering)*
- [ ] **Microphone input stays clean while frozen.** With Wait for Note on, let it hold on a note for a few seconds while talking/playing near the mic: pitch detection should behave exactly as it does unfrozen, with no dropouts, crackle, or delayed response. (`tick_clock` used to call `AudioSink::pause`/`play` every frame while frozen instead of once on the transition, which was suspected of disturbing a shared audio graph/server and affecting the — otherwise fully separate — mic capture pipeline; fixed by only touching the sink on the actual freeze/unfreeze edge.) *(manual: audio input; can't be asserted headlessly)*
- [ ] **Practice speed slows the highway and metronome without stuttering, and mutes rather than pitch-shifting.** Esc into the pause menu, click "🐢 Speed" to cycle down from 100% (90% → … → 50% → back to 100%): the note highway and metronome should visibly/audibly slow to match the label, and the music should go silent below 100% (no pitch-shifted/chipmunk audio) rather than stutter or race ahead. Cycle back to 100% mid-song and confirm the music resumes in sync with the highway (not skipped ahead or lagging) and the note highway doesn't jump or stutter across the transition. Also check the interaction with Wait for Note: turn both on, let it freeze on a note, then cycle Speed — the freeze should hold exactly as before (frozen takes priority over slow-but-playing). *(manual: needs a live `AudioSink`; the speed-cycling and label logic are unit-tested — `pause_menu::tests::next_speed_step_*`, `…::practice_speed_label_*` — but the audio pause/reseek/resume behavior needs a live run)*
- [ ] **Tab ribbon shows the current phrase's notation.** Below the phrase/groove banner in the sidebar, a line like `-4' +5 -4` should appear once the song reaches a chart phrase, listing every note in that phrase (not just ones already played) as `+N`/`-N` with bend apostrophes, an `o` for overblow/overdraw, or a `*` for a chromatic slide. It should update the instant a new phrase starts, and read empty before the first phrase. *(manual: rendering; the tab-string derivation and phrase-windowing are unit-tested — `phrase_overlay::tests::tab_label_*`, `…::phrase_tab_sequence_*` — but only a live run against a real chart confirms it reads correctly against the falling notes)*
- [ ] **"Note labels" option (Options page) swaps the arrow for the hole number, and persists.** Turn it on, start a song: every falling note should show its hole number instead of ↑/↓ — centered on the head in 2D, in a small dark pill just above-left of the note in 3D. Turn it off and confirm arrows come back. Restart the app and confirm the choice survived (`settings.json`). *(manual: rendering; the label text/toggle logic is unit-tested — `menu::options::tests::note_numbers_label_*` — but only a live run shows the actual head/overlay placement)*

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
- [ ] **Hole-number labels (when the option is on) track their note correctly.** Each label should stay glued to its note's on-screen position as it travels down the lane — no lag, no drift, no jitter — and should disappear the instant its note scrolls past behind the camera (not linger frozen on screen) or the moment the note is recycled. Check a chord/split with several simultaneous notes: each gets its own correctly-numbered label, not overlapping into one unreadable blob. Also press the UI-zoom keys (Arrow Up/Down, `dialogs::ui_scale`) mid-song and confirm labels stay glued to their notes at every zoom level — they were landing far off to the side at anything other than the default zoom until `note_label_position` accounted for `UiScale` (previously divided by the window's own scale factor, which `Camera::world_to_viewport` had already divided out; `UiScale` is a separate multiplier bevy_ui applies on top and was never divided out at all). *(manual: rendering; the label-position math is unit-tested — `gameplay_3d::tests::note_label_position_*` — but the world-to-screen projection itself has no headless test, since it needs a real camera/window)*

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
