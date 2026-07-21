// SPDX-License-Identifier: MIT

//! Freeform call-and-response within an open Jam Session: an off-by-default
//! toggle ([`CallResponseEnabled`], mirroring `session::JamLoop`) that has
//! the game play a short synthesized lick drawn from the chord tones of
//! whichever bar is currently sounding, then gives the player a couple of
//! bars to echo it by ear. Deliberately *not scored* — this is
//! call-and-response practice, not a lesson pass/fail criterion (see
//! `lessons::PassCriteria` for the scored jam-based criteria); the only
//! feedback is a turn-taking banner ("Listen…" / "Your turn") and a ghost
//! highlight of the call's holes on the existing live hole map
//! (`session::update_hole_map`), so the player has a visual memory aid, not
//! a graded outcome.
//!
//! Paced entirely off `AbsoluteBar` (not a separate timer) — the same
//! open-ended repeating-bar-pattern building block `improv::in_rest_window`
//! uses — so the cycle always lines up with the 12-bar chart and metronome
//! the player is already watching. Reuses the harmonica-timbre additive
//! synth (`audio_system::synth`) `gameplay::call_response` and the song
//! editor already share, and fires its audio the same fire-and-forget way
//! (a plain `AudioPlayer::DESPAWN` spawn — never touches `GameplayClock` or
//! the music sink, so it can't run afoul of the sink-anchoring invariant).

use std::collections::{HashMap, HashSet};

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::app::SelectedSong;
use crate::audio_system::midi::{midi_to_freq_hz, midi_to_note};
use crate::audio_system::synth::{Expr, PhraseNote, SAMPLE_RATE, TICKS_PER_BEAT, render_pcm};
use crate::audio_system::wav::encode_wav;
use crate::gameplay::{AbsoluteBar, BarChanged, CurrentBar, GameplayRoot};
use crate::localization::LocalizationExt;
use crate::settings::AudioSettings;
use crate::song::SongManifest;

use super::session::{JamHoleGuide, note_class};

/// Bars the game's call plays for, then bars the player has to echo it —
/// repeating indefinitely. Both divide evenly into the 12-bar cycle (4 | 12)
/// so the pattern always lines up with a fresh chorus, the same reasoning
/// `jam::improv`'s phrase-discipline pattern rests on.
const CALL_BARS: usize = 2;
const RESPONSE_BARS: usize = 2;

/// How many notes make up one generated call lick.
const LICK_LEN: usize = 4;

/// Whether the freeform call-and-response cycle is turned on for this jam —
/// off by default, a player opt-in toggle next to `session::JamLoop`.
#[derive(Resource, Default)]
pub struct CallResponseEnabled(pub bool);

/// The "Call & Response: ..." readout, kept in step with
/// [`CallResponseEnabled`] the same way `session::JamLoopLabel` is.
#[derive(Component)]
pub struct CallResponseLabel;

/// Whether the cycle is currently playing its call or waiting for the
/// player's echo — see [`phase_for_bar`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CallResponsePhase {
    #[default]
    Calling,
    Responding,
}

/// Live state of the current cycle: which phase it's in, and which holes
/// the current call lick used (for the hole map's ghost highlight — see
/// `session::update_hole_map`). Empty/`Calling` before the first lick plays.
#[derive(Resource, Default)]
pub struct CallResponseState {
    pub phase: CallResponsePhase,
    pub(crate) lick_holes: Vec<u8>,
}

/// Which phase bar `absolute_bar` falls in, cycling every
/// `CALL_BARS + RESPONSE_BARS` bars forever. Pure so the pacing is directly
/// testable without a running clock.
fn phase_for_bar(absolute_bar: usize) -> CallResponsePhase {
    let cycle = CALL_BARS + RESPONSE_BARS;
    if absolute_bar % cycle < CALL_BARS {
        CallResponsePhase::Calling
    } else {
        CallResponsePhase::Responding
    }
}

/// One MIDI pitch from `pool` (assumed non-empty), chosen by `roll` —
/// wrapped so any `usize` roll is always in range. Pure so [`generate_lick`]'s
/// only non-deterministic step is producing the roll itself.
fn pick_from_pool(pool: &[u8], roll: usize) -> u8 {
    pool[roll % pool.len()]
}

/// Every harp-producible MIDI pitch that's a tone of `chord_tones`, sorted
/// low to high — the pool one generated call lick draws from.
fn chord_tone_pitches(
    note_to_holes: &HashMap<u8, Vec<u8>>,
    chord_tones: &HashSet<String>,
) -> Vec<u8> {
    let mut pitches: Vec<u8> = note_to_holes
        .keys()
        .copied()
        .filter(|&m| chord_tones.contains(note_class(&midi_to_note(m as i32))))
        .collect();
    pitches.sort_unstable();
    pitches
}

/// Rolls [`LICK_LEN`] random pitches from `pool` — empty if `pool` is empty
/// (a chord with no representable tone on this harp; vanishingly unlikely,
/// but not assumed away).
fn generate_lick(pool: &[u8]) -> Vec<u8> {
    if pool.is_empty() {
        return Vec::new();
    }
    (0..LICK_LEN)
        .map(|_| pick_from_pool(pool, rand::random_range(0..pool.len())))
        .collect()
}

/// Builds the [`PhraseNote`]s for `lick`: one note per beat, in order,
/// starting at tick 0 — the same tick-grid vocabulary
/// `gameplay::call_response::build_phrase_notes` uses.
fn lick_phrase_notes(lick: &[u8]) -> Vec<PhraseNote> {
    lick.iter()
        .enumerate()
        .map(|(i, &midi)| PhraseNote {
            tick: i * TICKS_PER_BEAT,
            len: TICKS_PER_BEAT,
            freq: Some(midi_to_freq_hz(midi as f32)),
            expr: Expr::None,
        })
        .collect()
}

/// The turn-taking banner's text node.
#[derive(Component, Default, Clone)]
pub struct CallResponseBanner;

/// Spawns the (initially hidden) turn-taking banner. Tagged `GameplayRoot`
/// so it's torn down with the rest of Jam Session; only shown while
/// [`CallResponseEnabled`] is on (see [`update_call_response_banner`]).
pub fn spawn_call_response_banner(commands: &mut Commands) {
    commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                top: {Val::Percent(40.0)},
                width: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
            }
            GlobalZIndex(90)
            GameplayRoot
            Children [
                (
                    Text({""})
                    TextFont { font_size: {FontSize::Px(28.0)} }
                    TextColor({Color::srgb(1.0, 0.85, 0.35)})
                    CallResponseBanner
                )
            ]
        })
        .insert(Visibility::Hidden);
}

/// Keeps the banner's text/visibility in step with [`CallResponseEnabled`]/
/// [`CallResponseState`] — hidden entirely when the feature is off, else
/// "Listen…" during the call and "Your turn" during the response window.
pub fn update_call_response_banner(
    enabled: Res<CallResponseEnabled>,
    state: Res<CallResponseState>,
    loc: Res<Localization>,
    mut banners: Query<(&mut Text, &mut Visibility), With<CallResponseBanner>>,
) {
    if !enabled.is_changed() && !state.is_changed() {
        return;
    }
    for (mut text, mut vis) in &mut banners {
        if !enabled.0 {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        *text = Text::new(String::from(match state.phase {
            CallResponsePhase::Calling => loc.msg("jam-call-response-listen"),
            CallResponsePhase::Responding => loc.msg("jam-call-response-your-turn"),
        }));
    }
}

/// Keeps the "Call & Response: ..." readout in step with the toggle.
pub fn update_call_response_label(
    enabled: Res<CallResponseEnabled>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<CallResponseLabel>>,
) {
    if !enabled.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(String::from(if enabled.0 {
            loc.msg("jam-call-response-on")
        } else {
            loc.msg("jam-call-response-off")
        }));
    }
}

/// Drives the whole cycle: on every bar change (while enabled), updates
/// [`CallResponseState::phase`] and, exactly at the top of a new call
/// (`absolute_bar % cycle == 0`), rolls a fresh lick from the bar's chord
/// tones and fires its synthesized audio — fire-and-forget, like a
/// hit-feedback sound (see this module's doc comment on why that's safe).
pub fn drive_call_response(
    enabled: Res<CallResponseEnabled>,
    mut bar_changed: MessageReader<BarChanged>,
    absolute: Res<AbsoluteBar>,
    current: Res<CurrentBar>,
    guide: Option<Res<JamHoleGuide>>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut state: ResMut<CallResponseState>,
    mut commands: Commands,
) {
    if bar_changed.read().count() == 0 || !enabled.0 {
        return;
    }
    let (Some(guide), Some(manifest)) = (guide, manifests.get(&selected.0)) else {
        return;
    };

    state.phase = phase_for_bar(absolute.0);

    let cycle = CALL_BARS + RESPONSE_BARS;
    if !absolute.0.is_multiple_of(cycle) {
        return;
    }

    let chord_tones = &guide.chord_tones_by_bar[current.0];
    let pool = chord_tone_pitches(&guide.note_to_holes, chord_tones);
    let lick = generate_lick(&pool);
    state.lick_holes = lick
        .iter()
        .filter_map(|m| guide.note_to_holes.get(m))
        .flatten()
        .copied()
        .collect();
    if lick.is_empty() {
        return;
    }

    let bpm = manifest.chart.song.tempo_bpm;
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let pcm = render_pcm(&lick_phrase_notes(&lick), secs_per_tick);
    if pcm.is_empty() {
        return;
    }
    let wav = encode_wav(&pcm, SAMPLE_RATE);
    let source = sources.add(AudioSource { bytes: wav.into() });
    commands.spawn((
        AudioPlayer::<AudioSource>(source),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
        GameplayRoot,
    ));
}

#[cfg(test)]
mod tests;
