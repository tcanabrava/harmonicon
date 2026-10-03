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
  procedurally-generated 12-bar rhythm-section stems + chart, for `menu::pages::
  jam_generate`'s "Generate Jam" flow — Jam Session without picking an
  existing song) and registers it with a plain `Assets::add`. Such a
  manifest has no tracked `LoadState`, so `menu::routing::check_loading`'s
  `asset_server.is_loaded_with_dependencies` would never return true for
  it — both the initial launch and Restart route around `SongLoading`
  entirely (`jam_generate`'s Start button renders the backing on a worker
  thread — `render_generated_backing`, 100–250 ms — and
  `finish_pending_jam` assembles the manifest and sets `AppState::Playing`
  directly once it lands; `pause_menu::on_restart` targets `Playing` instead of
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
  classification `jam::hole_map::update_hole_map`'s tint uses, factored
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
  routes any of the three into `JamSession`, and `gameplay::pause_menu::
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
  phrase and gives the player a couple of bars to answer it by ear —
  deliberately not judged at all (no `PitchGate`/`ImprovStats` involved),
  since there's no authored phrase to score against. Paced by
  `AbsoluteBar` alone (`phrase::CALL_BARS`/`RESPONSE_BARS`, both dividing
  evenly into 12 so the cycle always lines up with a fresh chorus — the
  same reasoning `jam::improv`'s phrase-discipline pattern rests on)
  rather than a separate timer. **The call is a pure function**
  (`call_response::phrase::generate_call`) of the two bars' chords
  (`JamHoleGuide::chord_tones_by_bar`), the jam scale, the harp the
  player is holding (`JamHoleGuide::playable`, built by
  `phrase::playable_notes` — plain blows/draws only, so no call ever asks
  for a bend or the slide), the chart's straight/shuffle `Feel`, the
  density (`JamCallDensity`, the "Phrasing" cycle button: sparse /
  conversational / busy — the only control, never a level) and a seed
  (`CallResponseState::seed`, fresh per jam, mixed with the bar so each
  call differs but a restart replays them). It composes rather than
  rolls: a rhythm cell per bar (rests, pickups, held notes get vibrato,
  repeats), a two-to-four-note motif within a fifth of its anchor, an
  answering bar that repeats or sequences the motif up/down, and three
  invariants the tests pin down over hundreds of seeds — consecutive
  notes are one playable hole/breath move apart (`transition_ok`), a
  non-chord note is always followed by a chord tone (`usable`/`snap`),
  and the phrase ends on a chord tone by beat 3 of its last bar so there
  is air before "Your turn". Rendered through the same
  `harmonicon_core::synth` additive harmonica voice `gameplay::
  call_response` uses — a different instrument from the generated band,
  so it reads as a harmonica speaking over it — and fired the same
  fire-and-forget way. While it sounds, `CallDuck` eases the backing to
  60 % and back (`update_call_duck`, keyed off `speaking_until` against
  `GameplayClock`), applied by `midi_tracks::apply_backing_gain` in the
  same last-word pass as stem mute. Feedback is purely visual/turn-
  taking, not a score: a banner reading "Listen…"/"Your turn"
  (`CallResponseState::phase`), and the call's holes ghost-highlighted on
  the live hole map (`jam::hole_map::update_hole_map`, layered in only
  for a hole not already lit by a live pitch, so actually echoing a note
  still shows its normal chord-tone/in-scale tint) until the next call
  replaces them. The player's answer is never compared with the call.

- **The generated band listens without grading** (`jam::band`, generated
  jams only — a picked song's backing is a recording). `BandListener` is
  a pure decision-maker fed one completed beat at a time (`BeatActivity`:
  fresh-attack count, taken as the frame-to-frame delta of the always-on
  `ImprovStats::total`, plus whether anything sounded); it never sees a
  pitch. Two decisions, both at musical boundaries: at the top of each
  four-bar phrase the comping's target gain thins to 50 % if the phrase
  just finished was dense (≥ 6 attacks/bar) and returns to full otherwise
  — silence included, so a long rest keeps the pocket rather than
  filling it; and at beat 3 of a phrase's last bar, if the phrase had ≥ 4
  attacks and beats 1–2 of that bar were quiet *and not sounding*, a
  two-beat `backing::BandAnswer` (drum fill / chord push, alternating,
  rendered by `backing::render_band_answer` with the stems' own synth,
  last eighth always a rest) fires fire-and-forget — rate-capped to one
  per 8 bars. `BandTracker` accumulates the current beat (beat index from
  `GameplayClock` × bpm, `band::beat_index`) and eases the comping gain
  toward the target over one bar; `midi_tracks::apply_backing_gain`
  multiplies it into stem `backing::COMPING_STEM` only when
  `GeneratedJamSession` exists. `AdaptiveBand` (on by default; the
  "Adaptive band" toggle) turns both reactions off while the log keeps
  running. Deterministic under a recorded beat stream — the tests replay
  one. No text, counts or history ever reach the screen.

- **`jam::hole_map` is the one place a note is classified.** `JamHoleGuide`
  (per-jam: pitch → holes, the harp's playable vocabulary, scale classes,
  chord tones per bar) and `note_class` live there, not in `session`, so
  `improv`'s tally, `position_guide`'s patching, `call_response`'s phrase
  generator and the hole map's own tint all read one lookup and cannot
  disagree. `session` only builds it (`build_hole_guide`) and spawns the
  strip (`spawn_hole_map`).

- **Generated jams continue independently of `JamLoop`.** `JamLoop` remains
  the persistent opt-in behavior for a picked finite song. The presence of
  `GeneratedJamSession` makes `session::restart_finished_jam_music` respawn an
  exhausted four-chorus backing buffer without rewinding `GameplayClock`, so
  the next buffer is chorus 9 rather than a new session. `JamEnding` can queue
  a stop at the next 12-bar boundary; `finish_generated_jam_at_chorus` stops
  the backing there, plays a two-beat tonic hit as three separately tagged
  stems (so the mixer mutes still apply), and pins the free-running clock.
  `session::setup` resets the ending and call-response state on every entry/
  restart.

- **Generated-band humanization cannot move the form.** `backing::
  performance_variation` deterministically varies each role's offbeat onset
  and level from `GeneratedJamSession::seed` plus genre/chorus/bar/slot. The
  seed is created with the manifest, so Restart reproduces the performance
  and starting a new jam creates a new one. `varied_slot` always returns the
  original slot length and quarter-note downbeats have no delay, so all three
  stems stay sample-aligned.

- **Listening previews use the production renderer.** `cargo run -p
  harmonicon-jam --example listening_matrix` writes the fixed-seed
  Blues/Jazz/Reggae × 70/100/140 BPM matrix to `target/jam-listening/` through
  `backing::generate_listening_preview`. It mixes without normalization so
  the files preserve gameplay's actual balance and headroom.

- **A song can ship a raw MIDI file as its backing track**
  (`song/music.mid`, a third fallback in `song::loader` after
  `music.ogg`/`music.wav` — mutually exclusive with those; whichever is
  found first wins). Unlike an ordinary chart's music, this isn't loaded
  as one `Handle<AudioSource>` — `SongManifest::music` stays `None` and
  `SongManifest::backing_stems: Option<Vec<BackingStemAudio>>` is populated
  instead, one already-rendered `AudioSource` per non-empty track
  (`harmonicon_core::midi_file::render_track_pcm`, through the same
  additive harmonica voice in `harmonicon_core::synth` that
  `song_editor::playback` and `gameplay::call_response` use;
  `midi_file::notes_to_phrase` does the MIDI-timing-to-synth-tick
  conversion for both it and `song_editor::midi_import::
  render_backing_pcm`). Each track is rendered and registered as a labeled
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
  duplicated plumbing) and the new `BackingStemPlayer(index)` (defined
  alongside `MusicPlayer` in `gameplay::state`, not in `jam`, since
  `countdown_overlay` — which spawns it — can't depend on `jam` without
  a layering inversion; `jam::midi_tracks` reads it the other way).
  Muting a track is just zeroing that sink's volume — no live
  re-mixing, since each stem is already a complete, independent render.
  `jam::session::setup` sizes a new `JamStemMute(Vec<bool>)` resource to
  the song's own track count (empty, and thus a no-op everywhere, for
  an ordinary song) and — only for a MIDI-backed song — spawns a
  horizontal row of per-track mute-toggle buttons
  (`midi_tracks::spawn_backing_stem_row`) below the 12-bar/harmonica
  columns: the screen's root layout changed from a single Row to a
  Column wrapping those two columns in their own Row sub-container, so
  this new row can sit as a full-width sibling underneath both rather
  than a third column. Each button is tagged `TrackMuteCell(index)` and
  shares one `.observe(toggle_track_mute)` clone per button (same "one
  shared observer, N tagged cells, resolve identity via the clicked
  entity" pattern as `gameplay::harmonica_overlay::DiagramCellTarget`)
  rather than a distinct closure per track. `jam::midi_tracks::
  apply_backing_gain` is ordered `.after(gameplay::lifecycle::
  apply_music_volume)` so a mid-song global-volume change — which
  touches every `MusicPlayer` sink, per-track ones included — can never
  un-mute a muted track or un-duck a sounding call; this system always
  has the last word, and is the one writer of backing sink volume in a
  jam (mute × `CallDuck` × music volume). Looping
  (`jam::session::restart_finished_jam_music`) re-spawns every track's
  sink together the same way, and doesn't need to touch `JamStemMute`
  at all — it's a resource independent of any particular sink, so a
  track muted before the loop stays muted after it for free.
