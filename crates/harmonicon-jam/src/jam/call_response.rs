// SPDX-License-Identifier: MIT

//! Freeform call-and-response within an open Jam Session: an off-by-default
//! toggle ([`CallResponseEnabled`], mirroring `session::JamLoop`) that has
//! the game play a short synthesized phrase over whichever bars are
//! sounding, then gives the player a couple of bars to answer it by ear.
//! Deliberately *not scored* (see `lessons::PassCriteria` for the scored
//! jam-based criteria) — the only feedback is a turn-taking banner
//! ("Listen…" / "Your turn") and a ghost highlight of the call's holes on
//! the live hole map (`hole_map::update_hole_map`), a visual memory aid
//! rather than a graded outcome. The player's answer is listened to and lit
//! like any other jam note, never compared with the call.
//!
//! The call itself comes from [`phrase::generate_call`]: rhythm cells with
//! rests and pickups, a small motif, an answering contour, and a chord-tone
//! landing that leaves air before "Your turn" — its density is the one
//! musical control ([`JamCallDensity`]: sparse, conversational, busy), never
//! a level. Paced entirely off `AbsoluteBar`, the same open-ended
//! repeating-bar-pattern building block `improv::in_rest_window` uses, so
//! the cycle always lines up with the 12-bar chart and metronome. Rendered
//! through the harmonica-timbre additive synth (`harmonicon_core::synth`)
//! `gameplay::call_response` and the Song Editor share — a different
//! instrument from the generated rhythm section, so the call reads as a
//! harmonica speaking over the band — and fired the same fire-and-forget
//! way (a plain `AudioPlayer::DESPAWN` spawn — never touches
//! `GameplayClock` or the music sink). While it speaks, the backing ducks a
//! little ([`CallDuck`]) so the phrase is easy to hear on its own.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use harmonicon_app::app::SelectedSong;
use harmonicon_audio::AudioSettings;
use harmonicon_core::chart::Feel;
use harmonicon_core::midi::midi_to_freq_hz;
use harmonicon_core::synth::{Expr, PhraseNote, SAMPLE_RATE, TICKS_PER_BEAT, render_pcm};
use harmonicon_core::wav::encode_wav;
use harmonicon_gameplay::gameplay::{
    AbsoluteBar, BarChanged, CurrentBar, GameplayClock, GameplayRoot,
};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::SongManifest;

use super::hole_map::JamHoleGuide;

pub mod phrase;

use phrase::{CALL_BARS, CallContext, CallDensity, CallNote, call_end_tick, generate_call};

/// Bars the player has to answer the call — the call itself is
/// [`CALL_BARS`] long. Together they divide evenly into the 12-bar cycle
/// (4 | 12) so the pattern always lines up with a fresh chorus, the same
/// reasoning `jam::improv`'s phrase-discipline pattern rests on.
const RESPONSE_BARS: usize = 2;

/// Vibrato rate given to a held call note, so a long tone breathes the way
/// a player's would instead of sitting dead straight.
const HELD_VIBRATO_HZ: f32 = 5.5;

/// How far the backing drops while the call speaks (a linear gain), and how
/// long the move each way takes. A dip, not a mute: the band keeps the time
/// under the phrase.
const DUCK_GAIN: f32 = 0.6;
const DUCK_SECS: f32 = 0.15;

/// Whether the freeform call-and-response cycle is turned on for this jam —
/// off by default, a player opt-in toggle next to `session::JamLoop`.
#[derive(Resource, Default)]
pub struct CallResponseEnabled(pub bool);

/// The "Call & Response: ..." readout, kept in step with
/// [`CallResponseEnabled`] the same way `session::JamLoopLabel` is.
#[derive(Component, Default, Clone)]
pub struct CallResponseLabel;

/// How dense the generated calls are — the one musical control the player
/// has over them. Persists across jams within a run, like the guides
/// toggle; never saved as a score or level.
#[derive(Resource, Default)]
pub struct JamCallDensity(pub CallDensity);

/// The "Phrasing: ..." readout beside the density button.
#[derive(Component, Default, Clone)]
pub struct CallDensityLabel;

/// The backing's current duck gain (1.0 = untouched), eased toward
/// [`DUCK_GAIN`] while a call is speaking and back afterwards. Read by
/// `midi_tracks::apply_backing_gain` alongside the mute state.
#[derive(Resource)]
pub struct CallDuck(pub f32);

impl Default for CallDuck {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Whether the cycle is currently playing its call or waiting for the
/// player's echo — see [`phase_for_bar`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CallResponsePhase {
    #[default]
    Calling,
    Responding,
}

/// Live state of the current cycle: which phase it's in, which holes the
/// current call used (for the hole map's ghost highlight — see
/// `hole_map::update_hole_map`), when the call stops sounding (clock
/// seconds, for the duck), and the seed every call of this jam derives
/// from. Empty/`Calling` before the first call plays.
#[derive(Resource, Default)]
pub struct CallResponseState {
    pub phase: CallResponsePhase,
    pub(crate) lick_holes: Vec<u8>,
    pub(crate) speaking_until: Option<f64>,
    pub(crate) seed: u64,
}

impl CallResponseState {
    /// A reset state with a fresh seed, for `session::setup` — every jam
    /// gets its own phrases, while within one jam each bar's call is a pure
    /// function of that seed.
    pub fn fresh() -> Self {
        Self { seed: rand::random(), ..Self::default() }
    }
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

/// The seed for the call starting at `absolute_bar`: the jam's seed mixed
/// with the bar, so the same jam never repeats a call bar-for-bar yet
/// restarting it replays the same ones.
fn call_seed(session_seed: u64, absolute_bar: usize) -> u64 {
    session_seed ^ (absolute_bar as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Builds the [`PhraseNote`]s for `call`, on the same tick grid
/// `gameplay::call_response::build_phrase_notes` uses; a held note gets
/// vibrato.
fn call_phrase_notes(call: &[CallNote]) -> Vec<PhraseNote> {
    call.iter()
        .map(|n| PhraseNote {
            tick: n.tick,
            len: n.len,
            freq: Some(midi_to_freq_hz(f32::from(n.note.midi))),
            expr: if n.held { Expr::Vibrato(HELD_VIBRATO_HZ) } else { Expr::None },
        })
        .collect()
}

/// Moves `current` toward `target` by at most one frame's share of the
/// duck ramp, so the backing dips and recovers over [`DUCK_SECS`] rather
/// than stepping.
fn approach(current: f32, target: f32, dt_secs: f32) -> f32 {
    let step = (1.0 - DUCK_GAIN) * dt_secs / DUCK_SECS;
    if current < target { (current + step).min(target) } else { (current - step).max(target) }
}

/// The turn-taking banner's text node.
#[derive(Component, Default, Clone)]
pub struct CallResponseBanner;

/// Spawns the turn-taking banner as a row of the stage, between the chord
/// readout and the live indicator, so it sits with the form rather than
/// floating over whatever the layout centres there. The row keeps its
/// height while hidden so the stage doesn't shift when a call begins.
/// Text and visibility follow [`CallResponseEnabled`]/
/// [`CallResponseState`] from the first frame (see
/// [`update_call_response_banner`]).
pub fn spawn_call_response_banner(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            height: Val::Px(36.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn_empty().apply_scene(bsn! {
                Text("")
                TextFont { font_size: {FontSize::Px(28.0)} }
                TextColor({Color::srgb(1.0, 0.85, 0.35)})
                ~{Visibility::Hidden}
                CallResponseBanner
            });
        });
}

/// Keeps the banner's text/visibility in step with [`CallResponseEnabled`]/
/// [`CallResponseState`] — hidden entirely when the feature is off, else
/// "Listen…" during the call and "Your turn" during the response window.
pub fn update_call_response_banner(
    enabled: Res<CallResponseEnabled>,
    state: Res<CallResponseState>,
    loc: Res<Localization>,
    added: Query<(), Added<CallResponseBanner>>,
    mut banners: Query<(&mut Text, &mut Visibility), With<CallResponseBanner>>,
) {
    if !enabled.is_changed() && !state.is_changed() && added.is_empty() {
        return;
    }
    for (mut text, mut vis) in &mut banners {
        if !enabled.0 {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        if *vis != Visibility::Visible {
            *vis = Visibility::Visible;
        }
        let want = match state.phase {
            CallResponsePhase::Calling => loc.msg("jam-call-response-listen"),
            CallResponsePhase::Responding => loc.msg("jam-call-response-your-turn"),
        };
        if text.0 != *want {
            text.0.clear();
            text.0.push_str(&want);
        }
    }
}

/// Keeps the "Call & Response: ..." readout in step with the toggle.
pub fn update_call_response_label(
    enabled: Res<CallResponseEnabled>,
    loc: Res<Localization>,
    added: Query<(), Added<CallResponseLabel>>,
    mut labels: Query<&mut Text, With<CallResponseLabel>>,
) {
    // The toggle persists across jams (like `JamLoop`), so a freshly spawned
    // label has to read the current state, not a default.
    if !enabled.is_changed() && added.is_empty() {
        return;
    }
    let want = loc.msg(if enabled.0 { "jam-call-response-on" } else { "jam-call-response-off" });
    for mut text in &mut labels {
        if text.0 != *want {
            text.0.clear();
            text.0.push_str(&want);
        }
    }
}

/// The localization key of the "Phrasing: ..." readout for `density`.
pub fn density_key(density: CallDensity) -> &'static str {
    match density {
        CallDensity::Sparse => "jam-call-density-sparse",
        CallDensity::Conversational => "jam-call-density-conversational",
        CallDensity::Busy => "jam-call-density-busy",
    }
}

/// Keeps the "Phrasing: ..." readout in step with [`JamCallDensity`].
pub fn update_call_density_label(
    density: Res<JamCallDensity>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<CallDensityLabel>>,
) {
    if !density.is_changed() {
        return;
    }
    let want = loc.msg(density_key(density.0));
    for mut text in &mut labels {
        if text.0 != *want {
            text.0.clear();
            text.0.push_str(&want);
        }
    }
}

/// Eases [`CallDuck`] toward the dip while a call is sounding and back to
/// unity once it has finished (or the feature is off).
pub fn update_call_duck(
    enabled: Res<CallResponseEnabled>,
    state: Res<CallResponseState>,
    clock: Res<GameplayClock>,
    time: Res<Time>,
    mut duck: ResMut<CallDuck>,
) {
    let speaking = enabled.0 && state.speaking_until.is_some_and(|until| clock.get() < until);
    let target = if speaking { DUCK_GAIN } else { 1.0 };
    let next = approach(duck.0, target, time.delta_secs());
    if next != duck.0 {
        duck.0 = next;
    }
}

/// Drives the whole cycle: on every bar change (while enabled), updates
/// [`CallResponseState::phase`] and, exactly at the top of a new call
/// (`absolute_bar % cycle == 0`), generates a phrase over the chords of the
/// call's bars and fires its synthesized audio — fire-and-forget, like a
/// hit-feedback sound (see this module's doc comment on why that's safe).
pub fn drive_call_response(
    enabled: Res<CallResponseEnabled>,
    density: Res<JamCallDensity>,
    mut bar_changed: MessageReader<BarChanged>,
    absolute: Res<AbsoluteBar>,
    current: Res<CurrentBar>,
    clock: Res<GameplayClock>,
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

    let bars = guide.chord_tones_by_bar.len();
    let call = generate_call(&CallContext {
        playable: &guide.playable,
        opening_chord: &guide.chord_tones_by_bar[current.0],
        ending_chord: &guide.chord_tones_by_bar[(current.0 + 1) % bars],
        scale: &guide.scale_classes,
        swung: manifest.chart.song.feel == Some(Feel::Shuffle),
        density: density.0,
        seed: call_seed(state.seed, absolute.0),
    });
    state.lick_holes.clear();
    state.lick_holes.extend(call.iter().map(|n| n.note.hole));
    state.lick_holes.sort_unstable();
    state.lick_holes.dedup();
    if call.is_empty() {
        return;
    }

    let bpm = manifest.chart.song.tempo_bpm;
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let pcm = render_pcm(&call_phrase_notes(&call), secs_per_tick);
    if pcm.is_empty() {
        return;
    }
    state.speaking_until =
        Some(clock.get() + f64::from(call_end_tick(&call) as f32 * secs_per_tick));
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
