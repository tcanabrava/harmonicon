# harmonicon-jam

Jam Session: free play over a 12-bar form, with live hole-map feedback,
generated backing, improv-lesson judging and freeform call-and-response.

Builds on `harmonicon-gameplay`'s clock, bars and overlays.
`harmonicon-editor` is its sibling; neither imports the other.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **`SongManifest` doesn't have to come from the `AssetServer`.**
  `jam::backing::build_generated_manifest` synthesizes one at runtime (a
  procedurally-generated 12-bar bass line + chart, for `menu::pages::
  jam_generate`'s "Generate Jam" flow — Jam Session without picking an
  existing song) and registers it with a plain `Assets::add`. Such a
  manifest has no tracked `LoadState`, so `menu::routing::check_loading`'s
  `asset_server.is_loaded_with_dependencies` would never return true for
  it — both the initial launch and Restart route around `SongLoading`
  entirely (the `jam_generate` Start button sets `AppState::Playing`
  directly; `pause_menu::on_restart` targets `Playing` instead of
  `SongLoading` when `jam::backing::GeneratedJamSession` is present, safe
  because `NextState::set` always re-fires `OnExit`/`OnEnter` even for a
  same-state transition, per `bevy_state`). `GeneratedJamSession`'s
  presence is also what `menu::routing::route_menu_entry` checks to route "Quit
  Song" back to the jam setup page instead of `MenuPage::SongList` (a
  generated jam never went through the song list) — same end-of-life
  pattern as `lessons::LessonContext`.

- **One lesson type breaks the "ordinary pipeline" rule above:**
  `PassCriteria::ScaleAdherence` (the improvisation lesson) has no chart
  notes to score and no natural end — it's an open `GameplayMode::
  JamSession`. `menu::pages::lesson_reader::setup_lesson_reader`'s Start button
  routes it into `JamSession` instead of `Play2D`; `jam::improv::
  ImprovStats` (fresh-attack-gated, like `PitchGate`) accumulates
  scale/chord-tone adherence live via `classify_note_fit` — the same
  classification `jam::session::update_hole_map`'s tint uses, factored
  out so the two can't disagree — and a dedicated "Finish Lesson"
  pause-menu button (visible only for a jam session with a
  `LessonContext` in flight) judges it and returns to the menu on
  demand, via the same `apply_quit` path the ordinary Quit button uses;
  `route_menu_entry` sees the still-present `LessonContext` and routes
  to the skill tree, same as any other lesson. It never touches the
  results screen at all.
- **Two more jam-based criteria join `ScaleAdherence`:**
  `PassCriteria::ChordToneAdherence` (stricter — only counts chord tones,
  not "merely in-scale") and `PassCriteria::PhraseDiscipline` ("did you
  leave space" — `jam::improv::in_rest_window` classifies each fresh
  attack against a fixed repeating play/rest bar pattern,
  `PHRASE_PLAY_BARS`/`PHRASE_REST_BARS`, against `gameplay::AbsoluteBar` —
  an absolute, non-wrapped bar count kept alongside `CurrentBar` since the
  pattern must repeat consistently across an open-ended jam rather than
  resetting every 12 bars). All three read different fields off the same
  always-accumulating `ImprovStats` (`chord_tone`/`in_scale`/
  `out_of_scale`/`rest_violations`); `menu::pages::lesson_reader::is_jam_criteria`
  routes any of the three into `JamSession` the same way
  `ScaleAdherence` alone used to, and `gameplay::pause_menu::
  jam_fraction_for` picks the one relevant fraction for whichever
  criterion a given lesson declares before calling `lesson_passed`.
  Separately, `LessonManifest::progression` (an optional
  `"standard"`/`"quick-change"`/`"minor"` string, `menu::pages::lesson_reader::
  parse_progression`) seeds `harmonicon_app::app::JamProgression` on Start for any
  jam-based lesson, defaulting to `Standard` — same "don't let a stale
  pick linger" reasoning the real-song Jam Session button already
  applies.

- **Freeform call-and-response** (`jam::call_response`) is `gameplay::
  call_response`'s unscored, chart-free sibling: an opt-in toggle next to
  `jam::session::JamLoop` (`CallResponseEnabled`, off by default) that,
  while an open Jam Session runs, has the game play a short generated
  lick and gives the player a couple of bars to echo it by ear —
  deliberately not judged at all (no `PitchGate`/`ImprovStats` involved),
  since there's no authored phrase to score against. Paced by
  `AbsoluteBar` alone (`CALL_BARS`/`RESPONSE_BARS`, both dividing evenly
  into 12 so the cycle always lines up with a fresh chorus — the same
  reasoning `jam::improv`'s phrase-discipline pattern rests on) rather
  than a separate timer; a lick is a handful of MIDI pitches rolled from
  the pool of harp-producible notes that are tones of the bar's current
  chord (`JamHoleGuide::chord_tones_by_bar`/`note_to_holes`), rendered
  through the same `harmonicon_core::synth` additive harmonica voice and
  fired the same fire-and-forget way `gameplay::call_response` does.
  Feedback is purely visual/turn-taking, not a score: a banner reading
  "Listen…"/"Your turn" (`CallResponseState::phase`), and the lick's
  holes ghost-highlighted on the live hole map
  (`jam::session::update_hole_map`, layered in only for a hole not
  already lit by a live pitch, so actually echoing a note still shows its
  normal chord-tone/in-scale tint) until the next call replaces them.

- **A song can ship a raw MIDI file as its backing track**
  (`song/music.mid`, a third fallback in `song::loader` after
  `music.ogg`/`music.wav` — mutually exclusive with those; whichever is
  found first wins). Unlike an ordinary chart's music, this isn't loaded
  as one `Handle<AudioSource>` — `SongManifest::music` stays `None` and
  `SongManifest::midi_tracks: Option<Vec<MidiTrackAudio>>` is populated
  instead, one already-rendered `AudioSource` per non-empty track
  (`song::midi::render_track_pcm`, the same additive harmonica-voice
  synth `song_editor::playback`/`gameplay::call_response` share; a
  `notes_to_phrase` helper factors the MIDI-timing-to-synth-tick
  conversion out of `song_editor::midi_import::render_backing_pcm` so
  both share it). Each track is rendered and registered as a labeled
  sub-asset at song-load time (`song::loader::load_midi_tracks`), off
  the main thread like the rest of `SongChartLoader` — nothing about
  gameplay ever touches MIDI parsing itself. `waveform`/
  `music_duration_secs` still populate (every track's stems summed
  together, purely for the progress bar's display — actual playback
  sums them for real, as separate sinks) so a MIDI-backed song's
  progress bar behaves like an ordinary one.
  **Playback and per-track muting** (`jam::midi_tracks`, Jam Session
  only — scored Play2D/3D have nothing meaningful to do with a chart's
  *backing* track regardless of stem count): `gameplay::
  countdown_overlay::update_countdown` spawns one `AudioPlayer` per
  track, all in the same frame so they start in sync, each tagged both
  `MusicPlayer` (the ordinary single-track tag — pause and the global
  music-volume slider apply to every track's sink for free, no
  duplicated plumbing) and the new `MidiTrackPlayer(index)` (defined
  alongside `MusicPlayer` in `gameplay::state`, not in `jam`, since
  `countdown_overlay` — which spawns it — can't depend on `jam` without
  a layering inversion; `jam::midi_tracks` reads it the other way).
  Muting a track is just zeroing that sink's volume — no live
  re-mixing, since each stem is already a complete, independent render.
  `jam::session::setup` sizes a new `JamMidiMute(Vec<bool>)` resource to
  the song's own track count (empty, and thus a no-op everywhere, for
  an ordinary song) and — only for a MIDI-backed song — spawns a
  horizontal row of per-track mute-toggle buttons
  (`midi_tracks::spawn_midi_track_row`) below the 12-bar/harmonica
  columns: the screen's root layout changed from a single Row to a
  Column wrapping those two columns in their own Row sub-container, so
  this new row can sit as a full-width sibling underneath both rather
  than a third column. Each button is tagged `TrackMuteCell(index)` and
  shares one `.observe(toggle_track_mute)` clone per button (same "one
  shared observer, N tagged cells, resolve identity via the clicked
  entity" pattern as `gameplay::harmonica_overlay::DiagramCellTarget`)
  rather than a distinct closure per track. `jam::midi_tracks::
  apply_midi_track_mute` is ordered `.after(gameplay::lifecycle::
  apply_music_volume)` so a mid-song global-volume change — which
  touches every `MusicPlayer` sink, per-track ones included — can never
  un-mute a muted track; this system always has the last word. Looping
  (`jam::session::restart_finished_jam_music`) re-spawns every track's
  sink together the same way, and doesn't need to touch `JamMidiMute`
  at all — it's a resource independent of any particular sink, so a
  track muted before the loop stays muted after it for free.
