// SPDX-License-Identifier: MIT

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use super::state::{Dir, GridNote, HarmonicaKind, Pitch};
use super::{TICK_W, TICKS_PER_BEAT};
use crate::audio_system::midi::{midi_to_freq_hz, note_to_midi};
use crate::audio_system::synth::{PhraseNote, SAMPLE_RATE, render_pcm};
use crate::settings::AudioSettings;
use crate::song::harmonica::{Harmonica, chromatic_harp, hole_notes, richter_harp};

// ── Components / Resources ───────────────────────────────────────────────────

/// Marks an audio player spawned by the editor's Play button.
#[derive(Component)]
pub(super) struct EditorAudio;

/// The moving playback cursor (a vertical line) drawn over the grid.
#[derive(Component)]
pub(super) struct PlayheadLine;

/// The growing fill of the top progress bar.
#[derive(Component)]
pub(super) struct EditorProgressFill;

/// A one-shot seek to apply to the *next* editor audio sink that appears.
/// `spawn_background_music` can only spawn an `AudioPlayer`; the `AudioSink`
/// it needs to seek is inserted later by Bevy's audio systems, so a
/// mid-song start (recording from a clicked/paused position) parks the
/// offset here and [`apply_pending_music_seek`] delivers it once the sink
/// exists.
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

/// Builds the synthetic [`Harmonica`] the editor's own `GridNote`s (not a
/// loaded chart's authored layout) are resolved against — a Richter diatonic
/// or 12-hole chromatic, transposed to `key`. Shared with the Bending
/// Trainer via `crate::song::harmonica::{richter_harp, chromatic_harp}`, so
/// both agree on note names, key transposition, and (via [`hole_notes`])
/// which reed an overblow/overdraw actually sounds above.
pub(super) fn build_harp(key: &str, kind: HarmonicaKind) -> Harmonica {
    match kind {
        HarmonicaKind::Diatonic => richter_harp(key),
        HarmonicaKind::Chromatic => chromatic_harp(key),
    }
}

/// `note`'s resolved frequency (Hz) on `harp`, or `None` for a hole/technique
/// combination the harp can't produce (e.g. Overblow requested on a hole
/// outside 1–6). Bend depth is applied as a fractional semitone offset on the
/// natural blow/draw pitch; overblow/overdraw are resolved via
/// [`hole_notes`], which — unlike a flat "+1 semitone from whichever
/// direction the note is tagged with" — knows Overblow sits above the *draw*
/// reed on holes 1/4/5/6 and Overdraw above the *blow* reed on holes 7–10.
pub(super) fn note_freq(note: &GridNote, harp: &Harmonica) -> Option<f32> {
    let action = match note.dir {
        Dir::Blow => crate::song::chart::Action::Blow,
        Dir::Draw => crate::song::chart::Action::Draw,
    };
    let label = match note.pitch {
        Pitch::Normal => harp.wind_direction_label(note.hole, &action),
        Pitch::Slide => harp.slide_label(note.hole, &action),
        Pitch::Overblow | Pitch::Overdraw => hole_notes(harp, note.hole).over?,
        Pitch::Bend(a) => {
            let base = harp.wind_direction_label(note.hole, &action);
            let midi = note_to_midi(&base)?;
            return Some(midi_to_freq_hz(midi as f32 - a));
        }
    };
    Some(midi_to_freq_hz(note_to_midi(&label)? as f32))
}

/// Ticks-to-seconds for `state.tempo` — the flat nominal-BPM conversion
/// every Play/Practice/Record start function needs before it can turn tick
/// positions into real time. Deliberately not the real, possibly
/// multi-point tempo map (`state::EditorState::tempo_map`/`song::chart::
/// tick_to_seconds`) — audio synthesis stays on one constant tempo, the
/// documented scope boundary of the tempo-map feature (see `CLAUDE.md`).
pub(super) fn secs_per_tick(state: &super::state::EditorState) -> f32 {
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
/// player at the configured music volume — the shared "play the chart's
/// backing track" step Play/Practice/Record each need. Reads straight from
/// disk rather than through the asset server, since the chart being edited
/// may not be registered as an asset at all. Returns whether a player was
/// actually spawned, so a caller needing an "nothing is playing" fallback
/// (Practice's "no background music" hint) knows when to show one — true
/// for an empty path *and* a read failure (already `warn!`-logged either
/// way), since both leave nothing audible playing.
pub(super) fn spawn_background_music(
    state: &super::state::EditorState,
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
            let handle = sources.add(AudioSource {
                bytes: bytes.into(),
            });
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

pub(super) fn start_playback(
    state: &super::state::EditorState,
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

    let spt = secs_per_tick(state);
    if !state.notes.is_empty() {
        let harp = build_harp(&state.key, state.harmonica_kind);
        let phrase: Vec<PhraseNote> = state
            .notes
            .iter()
            .map(|n| PhraseNote {
                tick: n.tick,
                len: n.len,
                freq: note_freq(n, &harp),
                expr: n.expr,
            })
            .collect();
        let wav = crate::audio_system::wav::encode_wav(&render_pcm(&phrase, spt), SAMPLE_RATE);
        let handle = sources.add(AudioSource { bytes: wav.into() });
        commands.spawn((
            EditorAudio,
            AudioPlayer::<AudioSource>(handle),
            PlaybackSettings::DESPAWN,
        ));
        let end_tick = state
            .notes
            .iter()
            .map(|n| n.tick + n.len)
            .max()
            .unwrap_or(0);
        *playhead = playhead_for(end_tick, spt);
    }

    spawn_background_music(state, sources, settings, commands);
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
        *vis = Visibility::Hidden;
        return;
    }
    let cur_tick = playhead.elapsed / playhead.secs_per_tick;
    node.left = Val::Px(cur_tick * TICK_W);
    *vis = Visibility::Inherited;
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
        node.width = Val::Percent(p * 100.0);
    }
}
