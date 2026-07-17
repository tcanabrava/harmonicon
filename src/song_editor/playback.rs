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

    let bpm = state.tempo.trim().parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
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
        let wav = crate::audio_system::wav::encode_wav(
            &render_pcm(&phrase, secs_per_tick),
            SAMPLE_RATE,
        );
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
        *playhead = Playhead {
            playing: true,
            paused: false,
            elapsed: 0.0,
            total: end_tick as f32 * secs_per_tick,
            secs_per_tick,
        };
    }

    let music = state.music.trim();
    if !music.is_empty() {
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
            }
            Err(e) => warn!("Song editor: couldn't read background music {music:?}: {e}"),
        }
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
