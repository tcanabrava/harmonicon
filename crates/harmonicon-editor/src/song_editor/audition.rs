// SPDX-License-Identifier: MIT

//! Plays a short reference tone for a note the instant it becomes the
//! primary selection — confirms a bend/overblow/overdraw sounds like what
//! was meant, without running Play/Practice or reaching for a real harp.
//! Uses `playback::note_freq` and the shared `harmonicon_core::synth` voice.
//!
//! Scoped to *selection changing to a different note*: a fresh placement or
//! clicking an existing note both go through `EditorState::selected_note`,
//! so neither call site needs touching. Re-clicking an already-selected
//! note doesn't replay it.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::synth::{PhraseNote, SAMPLE_RATE, render_pcm};
use harmonicon_core::wav::encode_wav;

use super::playback::note_freq;
use super::state::{EditorState, GridNote};

/// Fixed audition length — long enough to judge pitch/tone, short enough
/// not to feel like an interruption. Independent of the note's own on-grid
/// duration (that's "how long it plays in the song", not "how long you
/// need to hear it to judge it").
const AUDITION_SECS: f32 = 0.6;

/// The id of whichever note was last auditioned, so [`audition_on_select`]
/// only plays a fresh blip when the primary selection changes to a
/// different note — not every frame it stays selected.
#[derive(Resource, Default)]
pub(super) struct LastAuditioned(Option<u32>);

/// Renders a short blip of `note`'s resolved pitch on `harp` as WAV bytes —
/// `None` for a hole/technique combination that can't sound at all (mirrors
/// `playback::note_freq`'s own `None` case).
fn audition_wav(note: &GridNote, harp: &Harmonica) -> Option<Vec<u8>> {
    let freq = note_freq(note, harp)?;
    let phrase = [PhraseNote { tick: 0, len: 1, freq: Some(freq), expr: note.expr }];
    Some(encode_wav(&render_pcm(&phrase, AUDITION_SECS), SAMPLE_RATE))
}

/// Plays [`audition_wav`] the instant the primary selection changes to a
/// different note.
pub(super) fn audition_on_select(
    state: Res<EditorState>,
    mut last: ResMut<LastAuditioned>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
) {
    let Some(note) = state.selected_note() else {
        if last.0.is_some() {
            last.0 = None;
        }
        return;
    };
    if last.0 == Some(note.id) {
        return;
    }
    last.0 = Some(note.id);
    let harp = state.effective_harp();
    let Some(wav) = audition_wav(note, &harp) else {
        return;
    };
    let handle = sources.add(AudioSource { bytes: wav.into() });
    commands.spawn((
        AudioPlayer::<AudioSource>(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.5)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song_editor::state::{Dir, Expr, Pitch};
    use harmonicon_core::harmonica::richter_harp;

    fn note(hole: u8, dir: Dir, pitch: Pitch) -> GridNote {
        GridNote { id: 1, hole, tick: 0, len: 4, dir, pitch, expr: Expr::None }
    }

    #[test]
    fn audition_wav_renders_something_audible_for_a_playable_note() {
        let harp = richter_harp("C");
        let wav = audition_wav(&note(4, Dir::Blow, Pitch::Normal), &harp).unwrap();
        // A real WAV: header plus a nonzero amount of sample data.
        assert!(wav.len() > 44);
    }

    #[test]
    fn audition_wav_is_none_for_an_unproducible_note() {
        let harp = richter_harp("C");
        // Neither overblow nor overdraw exists on hole 2 — the blow/draw
        // gap there is too narrow (`song::harmonica::hole_notes`'s `over`
        // is `None` for any hole outside 1/4/5/6/7–10) — so this checks
        // `audition_wav` defends itself against an unproducible note
        // rather than assuming a caller only ever passes it a valid one.
        let unplayable = note(2, Dir::Blow, Pitch::Overblow);
        assert_eq!(audition_wav(&unplayable, &harp), None);
    }
}
