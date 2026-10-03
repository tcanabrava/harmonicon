// SPDX-License-Identifier: MIT

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};

use super::state::{Dir, EditorState, GridNote, HarmonicaKind, Pitch};
use super::{TICK_W, TICKS_PER_BEAT};
use harmonicon_audio::AudioSettings;
use harmonicon_core::harmonica::{
    Harmonica, chromatic_16_harp, chromatic_harp, country_tuned_harp, hole_notes,
    natural_minor_harp, paddy_richter_harp, richter_harp,
};
use harmonicon_core::midi::{midi_to_freq_hz, note_to_midi};
use harmonicon_core::synth::{PhraseNote, SAMPLE_RATE, render_pcm};
use harmonicon_core::wav::encode_wav;

// ── Components / Resources ───────────────────────────────────────────────────

/// Marks an audio player spawned by the editor's Play button.
#[derive(Component)]
pub(super) struct EditorAudio;

/// The moving playback cursor (a vertical line) drawn over the grid.
#[derive(Component)]
pub(super) struct PlayheadLine;

/// The growing fill of the top progress bar.
#[derive(Component, Default, Clone)]
pub(super) struct EditorProgressFill;

/// A one-shot seek to apply to the *next* editor audio sink that appears.
/// `spawn_background_music` can only spawn an `AudioPlayer`; the
/// `AudioSink` it needs to seek is inserted later by Bevy's audio systems,
/// so a mid-song start (recording from a clicked/paused position) parks
/// the offset here and [`apply_pending_music_seek`] delivers it once the
/// sink exists.
#[derive(Resource, Default)]
pub(super) struct PendingMusicSeek(pub(super) Option<f32>);

/// Seeks a freshly created editor music sink to the parked
/// [`PendingMusicSeek`] offset, then clears it — see that type's docs.
pub(super) fn apply_pending_music_seek(
    mut pending: ResMut<PendingMusicSeek>,
    sinks: Query<&AudioSink, (With<EditorAudio>, Added<AudioSink>)>,
) {
    let Some(offset) = pending.0 else {
        return;
    };
    let Some(sink) = sinks.iter().next() else {
        return;
    };
    if let Err(e) = sink.try_seek(std::time::Duration::from_secs_f32(offset.max(0.0))) {
        warn!("Song editor: couldn't seek background music to {offset:.2}s: {e:?}");
    }
    pending.0 = None;
}

#[derive(Resource, Default)]
pub(super) struct Playhead {
    pub(super) playing: bool,
    /// True while playback/practice is frozen mid-song by the Pause button.
    /// Left orthogonal to `playing` (which stays `true` throughout a pause) so
    /// the playhead line's existing `!playing` visibility check keeps showing
    /// it, just not advancing — see `update_playhead_view`.
    pub(super) paused: bool,
    pub(super) elapsed: f32,
    pub(super) total: f32,
    pub(super) secs_per_tick: f32,
}

// ── Pure functions ────────────────────────────────────────────────────────────

/// Builds the named preset for the selected key and harmonica kind.
/// Loaded chart layouts are retained by `EditorState::effective_harp`.
pub(super) fn build_harp(key: &str, kind: HarmonicaKind) -> Harmonica {
    match kind {
        HarmonicaKind::Diatonic => richter_harp(key),
        HarmonicaKind::PaddyRichter => paddy_richter_harp(key),
        HarmonicaKind::CountryTuned => country_tuned_harp(key),
        HarmonicaKind::NaturalMinor => natural_minor_harp(key),
        HarmonicaKind::Chromatic => chromatic_harp(key),
        HarmonicaKind::Chromatic16 => chromatic_16_harp(key),
    }
}

/// `note`'s resolved frequency (Hz) on `harp`, or `None` for a hole/technique
/// combination the harp can't produce (e.g. Overblow outside holes 1–6).
/// Bend depth applies as a fractional semitone offset on the natural
/// blow/draw pitch; overblow/overdraw resolve via [`hole_notes`], which
/// knows Overblow sits above the *draw* reed on holes 1/4/5/6 and Overdraw
/// above the *blow* reed on holes 7–10.
pub(super) fn note_freq(note: &GridNote, harp: &Harmonica) -> Option<f32> {
    let (midi, bend) = resolve_pitch(note, harp)?;
    Some(midi_to_freq_hz(midi as f32 - bend))
}

/// `note`'s resolved MIDI pitch on `harp` — same resolution as [`note_freq`],
/// but the identity `u8` the shared `music_score` overlay keys notation on
/// (bends rounded to the nearest semitone, like `gameplay::notes::
/// target_pitch`). `None` under the same conditions as `note_freq`.
pub(super) fn note_midi(note: &GridNote, harp: &Harmonica) -> Option<u8> {
    let (midi, bend) = resolve_pitch(note, harp)?;
    u8::try_from(midi - bend.round() as i32).ok()
}

/// The MIDI note `note`'s reed sounds on `harp`, and how many (fractional)
/// semitones a bend takes it below that. `None` for a combination the harp
/// can't produce.
fn resolve_pitch(note: &GridNote, harp: &Harmonica) -> Option<(i32, f32)> {
    let action = match note.dir {
        Dir::Blow => harmonicon_core::chart::Action::Blow,
        Dir::Draw => harmonicon_core::chart::Action::Draw,
    };
    let (label, bend) = match note.pitch {
        Pitch::Normal => (harp.wind_direction_label(note.hole, &action), 0.0),
        Pitch::Slide => (harp.slide_label(note.hole, &action), 0.0),
        Pitch::Overblow | Pitch::Overdraw => (hole_notes(harp, note.hole).over?, 0.0),
        Pitch::Bend(depth) => (harp.wind_direction_label(note.hole, &action), depth),
    };
    Some((note_to_midi(&label)?, bend))
}

/// Ticks-to-seconds for `state.tempo` — the flat nominal-BPM conversion
/// Play/Practice/Record all need before turning tick positions into real
/// time. Deliberately not the real multi-point tempo map (`state::
/// EditorState::tempo_map`/`song::chart::tick_to_seconds`) — audio synthesis
/// stays on one constant tempo (see `CLAUDE.md`).
pub(super) fn secs_per_tick(state: &EditorState) -> f32 {
    let bpm = state.tempo.trim().parse::<f32>().unwrap_or(120.0).max(1.0);
    60.0 / bpm / TICKS_PER_BEAT as f32
}

/// A fresh, playing [`Playhead`] running for `total_ticks` at `secs_per_tick`
/// — the shape Play/Practice both construct once they know how long the
/// take should run. Record needs an effectively unbounded `total` instead
/// (a take has no natural end — see `record::start_record`'s own doc
/// comment) and builds its `Playhead` directly rather than through this.
pub(super) fn playhead_for(total_ticks: usize, secs_per_tick: f32) -> Playhead {
    Playhead {
        playing: true,
        paused: false,
        elapsed: 0.0,
        total: total_ticks as f32 * secs_per_tick,
        secs_per_tick,
    }
}

/// Spawns `state.music` (if set) as a fire-and-forget background-music
/// player at the configured volume — the shared "play the chart's backing
/// track" step Play/Practice/Record each need. Reads straight from disk
/// rather than the asset server, since the chart being edited may not be
/// registered as an asset. Returns whether a player was actually spawned —
/// `false` for both an empty path and a read failure (`warn!`-logged) so a
/// caller can show a "no background music" fallback either way.
pub(super) fn spawn_background_music(
    state: &EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    commands: &mut Commands,
) -> bool {
    let music = state.music.trim();
    if music.is_empty() {
        return false;
    }
    match std::fs::read(music) {
        Ok(bytes) => {
            let handle = sources.add(AudioSource { bytes: bytes.into() });
            commands.spawn((
                EditorAudio,
                AudioPlayer::<AudioSource>(handle),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.music_volume)),
            ));
            true
        }
        Err(e) => {
            warn!("Song editor: couldn't read background music {music:?}: {e}");
            false
        }
    }
}

/// A Play preview still being synthesized on a worker thread. Rendering a
/// whole song takes tens of milliseconds, too long for the main thread.
/// The entity also carries [`EditorAudio`], so every path that stops
/// playback by despawning editor audio drops the task too, which cancels
/// the render.
#[derive(Component)]
pub(super) struct PendingPlayback {
    task: Task<Vec<u8>>,
    end_tick: usize,
    secs_per_tick: f32,
}

pub(super) fn start_playback(
    state: &EditorState,
    sources: &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing: &Query<Entity, With<EditorAudio>>,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    *playhead = Playhead::default();

    if state.notes.is_empty() {
        spawn_background_music(state, sources, settings, commands);
        return;
    }
    let spt = secs_per_tick(state);
    let harp = state.effective_harp();
    let phrase: Vec<PhraseNote> = state
        .notes
        .iter()
        .map(|n| PhraseNote { tick: n.tick, len: n.len, freq: note_freq(n, &harp), expr: n.expr })
        .collect();
    let end_tick = state.notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);
    let task = AsyncComputeTaskPool::get()
        .spawn(async move { encode_wav(&render_pcm(&phrase, spt), SAMPLE_RATE) });
    commands.spawn((EditorAudio, PendingPlayback { task, end_tick, secs_per_tick: spt }));
}

/// Starts a Play preview once its render lands. The synth track, the
/// playhead and the background music start on the same frame, so the music
/// stays aligned with the notes.
pub(super) fn finish_pending_playback(
    mut pending: Query<(Entity, &mut PendingPlayback)>,
    state: Res<EditorState>,
    mut sources: ResMut<Assets<AudioSource>>,
    settings: Res<AudioSettings>,
    mut playhead: ResMut<Playhead>,
    mut commands: Commands,
) {
    for (entity, mut render) in &mut pending {
        let Some(wav) = future::block_on(future::poll_once(&mut render.task)) else {
            continue;
        };
        let handle = sources.add(AudioSource { bytes: wav.into() });
        commands
            .entity(entity)
            .remove::<PendingPlayback>()
            .insert((AudioPlayer::<AudioSource>(handle), PlaybackSettings::DESPAWN));
        *playhead = playhead_for(render.end_tick, render.secs_per_tick);
        spawn_background_music(&state, &mut sources, &settings, &mut commands);
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

pub(super) fn advance_playhead(time: Res<Time>, mut playhead: ResMut<Playhead>) {
    if playhead.playing && !playhead.paused {
        playhead.elapsed += time.delta_secs();
        if playhead.elapsed >= playhead.total {
            playhead.playing = false;
        }
    }
}

/// Toggles pause on the currently running playback/practice: pauses/resumes
/// every editor audio sink and freezes/unfreezes the playhead timer. The
/// playhead line stays visible while paused — only `paused` changes, not
/// `playing` (see the doc comment on [`Playhead::paused`]). A no-op if
/// nothing is currently playing.
pub(super) fn toggle_pause(playhead: &mut Playhead, sinks: &Query<&AudioSink, With<EditorAudio>>) {
    if !playhead.playing {
        return;
    }
    playhead.paused = !playhead.paused;
    for sink in sinks {
        if playhead.paused {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

pub(super) fn update_playhead_view(
    playhead: Res<Playhead>,
    mut line: Query<(&mut Node, &mut Visibility), With<PlayheadLine>>,
) {
    let Ok((mut node, mut vis)) = line.single_mut() else {
        return;
    };
    if !playhead.playing || playhead.secs_per_tick <= 0.0 {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let cur_tick = playhead.elapsed / playhead.secs_per_tick;
    let left = Val::Px(cur_tick * TICK_W);
    if node.left != left {
        node.left = left;
    }
    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }
}

pub(super) fn update_progress_bar(
    playhead: Res<Playhead>,
    mut fills: Query<&mut Node, With<EditorProgressFill>>,
) {
    let p = if playhead.total > 0.0 {
        (playhead.elapsed / playhead.total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for mut node in &mut fills {
        let width = Val::Percent(p * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
}
