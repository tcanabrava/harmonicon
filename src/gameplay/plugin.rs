// SPDX-License-Identifier: MIT

//! [`GameplayPlugin`]: resource registration and the full system schedule —
//! setup/cleanup lifecycles, the shared [`GameplayLogic`] chain (clock tick,
//! scoring, loop handling), and the mode-specific (2D/3D/Jam Session/Bending
//! Trainer) update chains layered on top of it.

use bevy::prelude::*;

use crate::app::{AppState, GameplayMode};
use crate::audio_system::pitch_detect::PitchRange;
use crate::jam::{improv, session as jam_session};
use crate::menu::tutorial::tour_active;
use crate::settings::AudioSettings;

use super::{
    adaptive_difficulty, bending_trainer, call_response, countdown_overlay, gameplay_2d,
    gameplay_3d, harmonica_overlay, metronome_overlay, modifier_legend, note_tail_2d,
    note_tail_3d, pause_menu, phrase_overlay, results, song_progress_overlay,
    twelve_bar_blues_overlay, wait_freeze_overlay,
};
use super::bars::{self, AbsoluteBar, BarChanged, CurrentBar};
use super::clock::{self, GameplayClock};
use super::hud;
use super::judge;
use super::lifecycle;
use super::notes::SongNotes;
use super::state::{
    ActivePitches, ActiveTargets, HitFeedback, LoopConfig, MusicStarted, NoteScored, Paused,
    PitchGate, Score, ScoringConfig, SongEnd, SongStats, ValidHarpNotes, collect_pitches,
};

pub struct GameplayPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
struct OverlaySet;

/// The shared per-frame gameplay logic (clock tick, scoring, loop handling).
/// Clock readers — note movement, hole/bar/metronome displays — must be ordered
/// after this set so they never sample a stale clock and stutter.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameplayLogic;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, OverlaySet);

        app.add_plugins((
            countdown_overlay::CountdownPlugin,
            twelve_bar_blues_overlay::TwelveBarBluesPlugin,
            metronome_overlay::MetronomePlugin,
            modifier_legend::ModifierLegendPlugin,
            phrase_overlay::PhrasePlugin,
            note_tail_2d::NoteTail2dPlugin,
            note_tail_3d::NoteTail3dPlugin,
            song_progress_overlay::SongProgressPlugin,
            wait_freeze_overlay::WaitFreezePlugin,
        ))
        .init_resource::<GameplayClock>()
        .init_resource::<PitchRange>()
        .init_resource::<ActivePitches>()
        .init_resource::<PitchGate>()
        .init_resource::<MusicStarted>()
        .init_resource::<ValidHarpNotes>()
        .init_resource::<SongNotes>()
        .init_resource::<adaptive_difficulty::AdaptiveDifficulty>()
        .init_resource::<gameplay_2d::NoteRenderAssets>()
        .init_resource::<gameplay_3d::NoteRenderAssets3D>()
        .init_resource::<Score>()
        .init_resource::<SongStats>()
        .init_resource::<SongEnd>()
        .init_resource::<HitFeedback>()
        .init_resource::<ScoringConfig>()
        .init_resource::<ActiveTargets>()
        .init_resource::<Paused>()
        .init_resource::<LoopConfig>()
        .init_resource::<CurrentBar>()
        .init_resource::<AbsoluteBar>()
        .add_message::<BarChanged>()
        .add_message::<NoteScored>()
        .init_resource::<bending_trainer::TrainerKey>()
        .init_resource::<bending_trainer::TrainerTarget>()
        .init_resource::<bending_trainer::DrillState>()
        .init_resource::<jam_session::JamLoop>()
        .init_resource::<improv::ImprovGate>()
        .init_resource::<improv::ImprovStats>()
        .init_resource::<call_response::CallCues>()
        .init_resource::<pause_menu::WaitForNoteMode>()
        .init_resource::<pause_menu::PracticeSpeed>()
        .init_resource::<pause_menu::SelectedPhraseIndex>()
        // Setup: shared pause menu + mode-specific scenes
        .add_systems(
            OnEnter(AppState::Playing),
            (
                lifecycle::reset_score,
                lifecycle::setup_scoring_config,
                adaptive_difficulty::setup_adaptive_difficulty,
                pause_menu::setup_pause_menu,
                gameplay_2d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                gameplay_3d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                jam_session::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                // Call-and-response cues need `SongNotes`' response notes to
                // lead into — Jam Session never builds those.
                call_response::setup_call_cues.run_if(|m: Res<GameplayMode>| {
                    matches!(*m, GameplayMode::Play2D | GameplayMode::Play3D)
                }),
            )
                // `gameplay_2d`/`gameplay_3d`'s `setup` read `AdaptiveDifficulty`
                // (`Res`) while `setup_adaptive_difficulty` writes it (`ResMut`) —
                // a real conflict, unlike the resources the earlier systems in
                // this tuple touch, so it needs an explicit order rather than
                // relying on tuple position. `.chain()` is the simplest way to
                // guarantee it (a `run_if`-skipped system still satisfies the
                // ordering edge for the next one in the chain).
                .chain(),
        )
        // Standalone Bending Trainer (its own AppState, no song).
        .add_systems(OnEnter(AppState::BendingTrainer), bending_trainer::setup)
        .add_systems(
            OnExit(AppState::BendingTrainer),
            (lifecycle::cleanup_gameplay, bending_trainer::save_drill_progress),
        )
        .add_systems(
            Update,
            harmonica_overlay::update_harmonica_overlay
                .in_set(OverlaySet)
                .run_if(in_state(AppState::BendingTrainer)),
        )
        .add_systems(
            Update,
            harmonica_overlay::update_harmonica_overlay
                .in_set(OverlaySet)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                ),
        )
        // Order against the set, not the system function.
        .add_systems(
            Update,
            bending_trainer::update_drill_progress_tint.after(OverlaySet),
        )
        .add_systems(
            Update,
            (
                bending_trainer::tick_clock,
                collect_pitches,
                bending_trainer::rebuild_overlay,
                bending_trainer::update_selected_cell_border,
                bending_trainer::update_pitch_range,
                bending_trainer::update_key_label,
                bending_trainer::update_target_label,
                bending_trainer::update_hint_label,
                bending_trainer::update_tuner_readout,
                bending_trainer::drill_update,
                bending_trainer::update_drill_label,
                // Suspended while the guided tour is showing this screen —
                // Esc shouldn't leave out from under it (see `menu::tutorial`).
                bending_trainer::handle_escape.run_if(not(tour_active)),
            )
                .run_if(in_state(AppState::BendingTrainer)),
        )
        // Cleanup: shared entity despawn + restore camera on 3D exit
        .add_systems(OnExit(AppState::Playing), lifecycle::cleanup_gameplay)
        .add_systems(
            OnExit(AppState::Playing),
            gameplay_3d::restore_camera.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
        )
        // Pause input always runs during Playing (even when paused). The pause
        // buttons carry their own click/hover behaviour as inline `on(...)`
        // observers (see `setup_pause_menu`), so no button systems here.
        // Suspended while the guided tour is showing a live-gameplay step —
        // Esc shouldn't pause out from under it (see `menu::tutorial`).
        .add_systems(
            Update,
            pause_menu::handle_pause_input
                .run_if(in_state(AppState::Playing).and_then(not(tour_active))),
        )
        // Apply live volume changes to the playing song (even while paused).
        .add_systems(
            Update,
            lifecycle::apply_music_volume
                .run_if(in_state(AppState::Playing).and_then(resource_changed::<AudioSettings>)),
        )
        // The Wait-for-Note toggle lives on the pause overlay itself, so its
        // label has to keep updating while paused (that's the only time the
        // button is visible/clickable).
        .add_systems(
            Update,
            (
                pause_menu::update_wait_mode_label,
                pause_menu::update_loop_label,
                pause_menu::update_practice_speed_slider,
                pause_menu::update_phrase_selector_label,
                pause_menu::update_phrase_learned_slider,
                pause_menu::update_adaptive_difficulty_label,
            )
                .run_if(in_state(AppState::Playing)),
        )
        // Re-unlocks/re-locks notes the instant the pause menu's phrase
        // override or on/off toggle changes `AdaptiveDifficulty` — not
        // gated on `!Paused` like the render chains below, since editing it
        // is only ever possible *while* paused (see `pause_menu`).
        .add_systems(
            Update,
            gameplay_2d::resync_notes_on_adaptive_change.run_if(
                in_state(AppState::Playing).and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
            ),
        )
        .add_systems(
            Update,
            gameplay_3d::resync_notes_on_adaptive_change.run_if(
                in_state(AppState::Playing).and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
            ),
        )
        // Gameplay-logic chains only run when not paused. This set ticks the
        // clock, so every clock reader below must run after it — otherwise the
        // executor may read a stale clock on some frames, making notes stutter.
        .add_systems(
            Update,
            (
                clock::tick_clock,
                clock::handle_loop_boundary,
                bars::track_current_bar,
                collect_pitches,
                judge::update_active_targets,
                judge::score_notes,
                hud::update_score_display,
                lifecycle::detect_song_end,
                note_tail_2d::animate_note_tails,
            )
                .chain()
                .in_set(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        )
        // Jam Session: live harmonica hole-map feedback from the mic, plus the
        // improv lesson's scale-adherence tally (always accumulating during a
        // jam, not just when a lesson is in flight — same "always-on
        // diagnostic" convention as `SongStats::clean_attack`).
        .add_systems(
            Update,
            (
                jam_session::update_hole_map,
                improv::accumulate_improv_stats,
            )
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                ),
        )
        // Call-and-response: fires each phrase's synthesized demo audio the
        // instant the clock reaches its scheduled lead time. Fire-and-forget
        // (see `call_response`'s doc comment on why it never touches the
        // clock/sink), so it only needs to run after the clock ticks, not
        // strictly ordered against scoring.
        .add_systems(
            Update,
            call_response::fire_call_cues.after(GameplayLogic).run_if(
                in_state(AppState::Playing)
                    .and_then(|p: Res<Paused>| !p.0)
                    .and_then(|m: Res<GameplayMode>| {
                        matches!(*m, GameplayMode::Play2D | GameplayMode::Play3D)
                    }),
            ),
        )
        // Jam Session: music loop toggle + its readout.
        .add_systems(
            Update,
            (
                jam_session::restart_finished_jam_music,
                jam_session::update_jam_loop_label,
            )
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
                ),
        )
        // Results screen lifecycle. The Retry/Continue buttons carry their own
        // click/hover behaviour as inline on(...) observers (see results::setup).
        .add_systems(OnEnter(AppState::Results), results::setup)
        .add_systems(OnExit(AppState::Results), results::cleanup)
        .add_systems(
            Update,
            results::handle_escape.run_if(in_state(AppState::Results)),
        )
        // 2D update chain
        .add_systems(
            Update,
            (
                gameplay_2d::spawn_visible_notes,
                gameplay_2d::update_notes,
                gameplay_2d::size_note_tails,
                gameplay_2d::update_note_visuals,
                gameplay_2d::update_holes,
            )
                .chain()
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                ),
        )
        // 3D update chain
        .add_systems(
            Update,
            (
                gameplay_3d::spawn_visible_notes_3d,
                gameplay_3d::update_notes_3d,
                gameplay_3d::update_note_hole_labels_3d,
                gameplay_3d::update_note_visuals_3d,
                gameplay_3d::animate_note_tails_3d,
                gameplay_3d::update_holes_3d,
                gameplay_3d::groove_harmonica,
            )
                .chain()
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                ),
        );
    }
}
