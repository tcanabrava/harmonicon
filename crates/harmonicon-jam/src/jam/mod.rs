// SPDX-License-Identifier: MIT

//! Jam Session systems for free play, generated backing, and practice guides.

use bevy::prelude::*;

use harmonicon_app::app::{AppState, GameplayMode, GeneratedJamSession};
use harmonicon_gameplay::gameplay::Paused;
use harmonicon_gameplay::gameplay::plugin::GameplayLogic;

pub mod backing;
pub mod band;
pub mod call_response;
pub mod hole_map;
pub mod improv;
pub mod lesson;
pub mod midi_tracks;
pub mod position_guide;
pub mod rhythm_guide;
pub mod session;
pub mod session_ui;

use call_response as jam_call_response;
use midi_tracks as jam_stems;
use position_guide as jam_position_guide;
use rhythm_guide as jam_rhythm_guide;
use session as jam_session;

pub struct JamPlugin;

impl Plugin for JamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<jam_session::JamLoop>()
            .init_resource::<jam_session::JamEnding>()
            .init_resource::<session_ui::JamGuidesVisible>()
            .init_resource::<session_ui::JamChordSequence>()
            .init_resource::<jam_stems::JamStemMute>()
            .init_resource::<improv::ImprovGate>()
            .init_resource::<improv::ImprovStats>()
            .add_message::<jam_position_guide::PositionCalled>()
            .init_resource::<jam_call_response::CallResponseEnabled>()
            .init_resource::<jam_call_response::CallResponseState>()
            .init_resource::<jam_call_response::JamCallDensity>()
            .init_resource::<jam_call_response::CallDuck>()
            .init_resource::<band::AdaptiveBand>()
            .init_resource::<band::BandListener>()
            .init_resource::<band::BandTracker>()
            // Jam's own per-session reset, alongside gameplay's `reset_score`.
            .add_systems(
                OnEnter(AppState::Playing),
                (
                    improv::reset_improv_stats,
                    jam_session::setup
                        .run_if(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                ),
            )
            // Judges a jam-based lesson when the pause menu asks.
            .add_systems(Update, lesson::finish_jam_lesson)
            // Background-music looping + the loop toggle's label.
            .add_systems(
                Update,
                (
                    jam_session::finish_generated_jam_at_chorus
                        .run_if(resource_exists::<GeneratedJamSession>),
                    jam_session::restart_finished_jam_music,
                    jam_session::update_jam_loop_label,
                    jam_session::update_jam_status_labels,
                    session_ui::update_jam_guides,
                    session_ui::update_jam_chord_position,
                )
                    .chain()
                    .after(GameplayLogic)
                    .run_if(
                        in_state(AppState::Playing)
                            .and_then(|p: Res<Paused>| !p.0)
                            .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                    ),
            )
            // Jam Session, position-cycling lesson mechanic: calls a new position
            // (cycling `JamScale`) every few bars and patches `JamHoleGuide` to
            // match — ordered before `improv::accumulate_improv_stats` so a
            // bar-boundary frame is never scored against the stale scale. A
            // no-op for an ordinary jam (`JamPositionCycle` off).
            .add_systems(
                Update,
                (jam_position_guide::cycle_position, jam_position_guide::on_position_called)
                    .chain()
                    .after(GameplayLogic)
                    .before(improv::accumulate_improv_stats)
                    .run_if(
                        in_state(AppState::Playing)
                            .and_then(|p: Res<Paused>| !p.0)
                            .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                    ),
            )
            // Jam Session: live harmonica hole-map feedback from the mic, plus the
            // improv lesson's scale-adherence tally (always accumulating during a
            // jam, not just when a lesson is in flight — same "always-on
            // diagnostic" convention as `SongStats::clean_attack`).
            .add_systems(
                Update,
                (
                    hole_map::update_hole_map,
                    hole_map::update_detected_note,
                    improv::accumulate_improv_stats,
                    jam_call_response::drive_call_response,
                    jam_call_response::update_call_response_banner,
                    jam_call_response::update_call_response_label,
                    jam_call_response::update_call_density_label,
                    jam_call_response::update_call_duck,
                    jam_stems::update_stem_mute_buttons,
                )
                    .after(GameplayLogic)
                    .run_if(
                        in_state(AppState::Playing)
                            .and_then(|p: Res<Paused>| !p.0)
                            .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                    ),
            )
            // Backing sink gain (stem mute + call-and-response duck) — after
            // `apply_music_volume` (a mid-song global-volume change touches
            // every `MusicPlayer` sink, per-track ones included) so a muted
            // track always ends up silent and a ducked band stays ducked
            // regardless of which order the two would otherwise run in.
            .add_systems(
                Update,
                jam_stems::apply_backing_gain
                    .after(harmonicon_gameplay::gameplay::plugin::MusicVolumeSet)
                    .run_if(
                        in_state(AppState::Playing)
                            .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                    ),
            )
            // Jam Session: the harmonica rhythm-guide pulse row — only ever
            // spawned for a generated jam (see `jam::rhythm_guide`'s own doc
            // comment), so gated on `GeneratedJamSession`'s presence too, not
            // just the mode. The adaptive band is generated-only for the same
            // reason (a picked song's backing is a recording); it reads the
            // improv tally's fresh-attack count, so it runs after it.
            .add_systems(
                Update,
                (
                    jam_rhythm_guide::update_rhythm_guide,
                    band::listen_and_react.after(improv::accumulate_improv_stats),
                    band::update_adaptive_band_label,
                )
                    .after(GameplayLogic)
                    .run_if(
                        in_state(AppState::Playing)
                            .and_then(|p: Res<Paused>| !p.0)
                            .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession)
                            .and_then(resource_exists::<GeneratedJamSession>),
                    ),
            );
    }
}
