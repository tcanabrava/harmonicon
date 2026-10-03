// SPDX-License-Identifier: MIT

//! [`GameplayPlugin`]: resource registration and the full system schedule —
//! setup/cleanup lifecycles, the shared [`GameplayLogic`] chain (clock tick,
//! scoring, loop handling), and the mode-specific (2D/3D/Jam Session/Bending
//! Trainer) update chains layered on top of it.

use bevy::prelude::*;

use harmonicon_app::app::tour_active;
use harmonicon_app::app::{AppState, GameplayMode};
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::PitchRange;

use super::bars::{self, AbsoluteBar, BarChanged, CurrentBar};
use super::beat_guides;
use super::clock::{self, GameplayClock};
use super::hud;
use super::judge;
use super::lifecycle;
use super::notes::SongNotes;
use super::practice_badges;
use super::state::{
    ActivePitches, ActiveTargets, HarmonicaPitchFilter, HitFeedback, LoopConfig, MusicStarted,
    NoteScored, Paused, PitchGate, PlayedHarp, PracticeRequest, Score, ScoringConfig, SongEnd,
    SongStats, ValidHarpNotes, collect_pitches,
};
use super::technique_coach;
use super::{
    adaptive_difficulty, bending_trainer, call_response, countdown_overlay, gameplay_2d,
    gameplay_3d, harmonica_overlay, metronome_overlay, modifier_legend, music_score_bridge,
    note_ribbon_2d, note_ribbon_3d, pause_menu, phrase_overlay, results, song_progress_overlay,
    twelve_bar_blues_overlay, wait_freeze_overlay, warning_banner,
};

pub struct GameplayPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
struct OverlaySet;

/// The Bending Trainer's own run of `collect_pitches`, published as a set so
/// its readers have something unambiguous to order against.
///
/// `collect_pitches` is registered **twice** in `Update` — once here for the
/// trainer and once inside [`GameplayLogic`] — and a bare
/// `.after(collect_pitches)` names a `SystemTypeSet`, which Bevy refuses to
/// order against when the system has more than one instance. That is a
/// schedule-build *panic* at the moment the screen is first entered, not a
/// compile error. Same reasoning as `MusicVolumeSet` below: publish an
/// ordering point, never a system name.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
struct TrainerPitchSet;

/// The pass that applies the live music-volume setting to every playing
/// sink. Anything that adjusts an individual sink's volume — jam's
/// per-track mute, say — orders itself `.after` this set, so a global
/// volume change can never undo it. Published as a set rather than a
/// system so the system itself stays private.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MusicVolumeSet;

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
            note_ribbon_2d::NoteRibbon2dPlugin,
            note_ribbon_3d::NoteRibbon3dPlugin,
            song_progress_overlay::SongProgressPlugin,
            warning_banner::WarningBannerPlugin,
            wait_freeze_overlay::WaitFreezePlugin,
            music_score_bridge::MusicScoreBridgePlugin,
            super::karaoke::KaraokePlugin,
        ))
        .init_resource::<GameplayClock>()
        .init_resource::<PitchRange>()
        .init_resource::<ActivePitches>()
        .init_resource::<HarmonicaPitchFilter>()
        .init_resource::<PitchGate>()
        .init_resource::<MusicStarted>()
        .init_resource::<ValidHarpNotes>()
        .init_resource::<PlayedHarp>()
        .init_resource::<super::SongInfo>()
        .init_resource::<harmonicon_app::app::EffectiveHarmonica>()
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
        .init_resource::<PracticeRequest>()
        .init_resource::<CurrentBar>()
        .init_resource::<AbsoluteBar>()
        .add_message::<BarChanged>()
        .add_message::<NoteScored>()
        .add_message::<adaptive_difficulty::NotesRebuilt>()
        .add_message::<pause_menu::FinishLessonRequested>()
        .init_resource::<bending_trainer::TrainerKey>()
        .init_resource::<bending_trainer::TrainerTarget>()
        .init_resource::<bending_trainer::DrillState>()
        .init_resource::<bending_trainer::NaturalCheck>()
        .init_resource::<bending_trainer::BendTrace>()
        .init_resource::<bending_trainer::GesturePractice>()
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
                lifecycle::configure_pitch_filter,
                adaptive_difficulty::setup_adaptive_difficulty,
                // Everything that *shows* the song's details has to run
                // after the system that resolves them; an `add_systems`
                // tuple is otherwise unordered.
                lifecycle::setup_song_info,
                pause_menu::setup_pause_menu.after(lifecycle::setup_song_info),
                // Every mode, unlike the 2D/3D-only overlays below.
                warning_banner::setup_warning_banner,
                gameplay_2d::setup
                    .after(lifecycle::setup_song_info)
                    .run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                gameplay_3d::setup
                    .after(lifecycle::setup_song_info)
                    .run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
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
            harmonica_overlay::update_harmonica_overlay.in_set(OverlaySet).run_if(
                in_state(AppState::Playing)
                    .and_then(|p: Res<Paused>| !p.0)
                    .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
            ),
        )
        // Order against the set, not the system function.
        .add_systems(Update, bending_trainer::update_drill_progress_tint.after(OverlaySet))
        .add_systems(
            Update,
            (
                bending_trainer::tick_clock,
                collect_pitches.in_set(TrainerPitchSet),
                bending_trainer::rebuild_overlay,
                bending_trainer::update_selected_cell_border,
                bending_trainer::update_pitch_range,
                bending_trainer::update_target_label,
                bending_trainer::update_hint_label,
                bending_trainer::update_bend_trace.after(TrainerPitchSet),
                bending_trainer::update_tuner_readout.after(bending_trainer::update_bend_trace),
                bending_trainer::update_bend_rail.after(bending_trainer::update_bend_trace),
                bending_trainer::update_gesture_practice.after(bending_trainer::update_bend_trace),
                bending_trainer::update_practice_shape_label
                    .after(bending_trainer::update_gesture_practice),
                bending_trainer::update_natural_check,
                bending_trainer::update_natural_check_label
                    .after(bending_trainer::update_natural_check),
                // Reads the gesture's terminal phase to decide an attempt is
                // over, so it must see *this* frame's phase, not last one's.
                bending_trainer::drill_update
                    .after(bending_trainer::update_bend_trace)
                    .after(bending_trainer::update_gesture_practice),
                (
                    bending_trainer::update_drill_label,
                    bending_trainer::update_drill_scope_label,
                    bending_trainer::update_drill_button_visual,
                ),
                // Nested rather than flattened into the run above: Bevy's
                // `add_systems` tuple tops out at 20 elements, and a
                // 21st turns the whole tuple into a confusing
                // "cannot become an ObserverSystem" error rather than
                // anything about arity.
                (
                    bending_trainer::sync_advanced_drawer,
                    bending_trainer::update_advanced_labels,
                    // Reports the trace, so it wants this frame's samples.
                    bending_trainer::update_advanced_readouts
                        .after(bending_trainer::update_bend_trace),
                ),
                (
                    bending_trainer::update_setup_summary,
                    bending_trainer::update_drill_slots,
                    bending_trainer::update_target_progress,
                    bending_trainer::apply_trainer_orientation,
                    bending_trainer::navigate_diagram,
                    bending_trainer::update_cell_progress_bars,
                ),
                // Suspended while the guided tour is showing this screen —
                // Esc shouldn't leave out from under it (see `menu::tutorial`).
                bending_trainer::handle_escape.run_if(not(tour_active)),
            )
                .run_if(in_state(AppState::BendingTrainer)),
        )
        // `PostUpdate`, not with the rest of the trainer in `Update`:
        // `rebuild_overlay` despawns and respawns the diagram's cells through
        // deferred commands, so in `Update` this could queue an insert on a
        // cell that is despawned by the time the insert lands — which panics.
        // After `Update`'s commands have applied, every cell it sees is live.
        .add_systems(
            PostUpdate,
            bending_trainer::attach_progress_bars.run_if(in_state(AppState::BendingTrainer)),
        )
        // Cleanup: shared entity despawn + restore camera on 3D exit
        .add_systems(OnExit(AppState::Playing), lifecycle::cleanup_gameplay)
        .add_systems(OnExit(AppState::Playing), lifecycle::reset_pitch_filter)
        .add_systems(OnExit(AppState::Playing), lifecycle::clear_practice_request)
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
                .in_set(MusicVolumeSet)
                .run_if(in_state(AppState::Playing).and_then(resource_changed::<AudioSettings>)),
        )
        // The Wait-for-Note toggle lives on the pause overlay itself, so its
        // label has to keep updating while paused (that's the only time the
        // button is visible/clickable).
        .add_systems(
            Update,
            (
                pause_menu::update_wait_mode_label,
                practice_badges::update_practice_badges,
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
            (
                adaptive_difficulty::resync_notes_on_adaptive_change,
                adaptive_difficulty::invalidate_note_visuals,
            )
                .chain()
                .before(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(
                    |m: Res<GameplayMode>| {
                        matches!(*m, GameplayMode::Play2D | GameplayMode::Play3D)
                    },
                )),
        )
        // Gameplay-logic chains only run when not paused. This set ticks the
        // clock, so every clock reader below must run after it — otherwise the
        // executor may read a stale clock on some frames, making notes stutter.
        .add_systems(
            Update,
            (
                clock::tick_clock,
                lifecycle::start_at_practice_range,
                clock::handle_loop_boundary,
                bars::track_current_bar,
                collect_pitches,
                // Dev-only: overwrites what `collect_pitches` produced when
                // autoplay is on, so it has to sit exactly here.
                #[cfg(feature = "dev")]
                super::autoplay::autoplay_pitches,
                judge::update_active_targets,
                judge::score_notes,
                hud::update_score_display,
                lifecycle::detect_song_end,
                note_ribbon_2d::animate_note_ribbons,
            )
                .chain()
                .in_set(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        )
        // Call-and-response: fires each phrase's synthesized demo audio the
        // instant the clock reaches its scheduled lead time. Fire-and-forget
        // (see `call_response`'s doc comment on why it never touches the
        // clock/sink), so it only needs to run after the clock ticks, not
        // strictly ordered against scoring.
        .add_systems(
            Update,
            call_response::fire_call_cues.after(GameplayLogic).run_if(
                in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0).and_then(
                    |m: Res<GameplayMode>| {
                        matches!(*m, GameplayMode::Play2D | GameplayMode::Play3D)
                    },
                ),
            ),
        )
        // Results screen lifecycle. The Retry/Continue buttons carry their own
        // click/hover behaviour as inline on(...) observers (see results::setup).
        .add_systems(OnEnter(AppState::Results), results::setup)
        .add_systems(OnExit(AppState::Results), results::cleanup)
        .add_systems(Update, results::handle_escape.run_if(in_state(AppState::Results)))
        // 2D update chain
        .add_systems(
            Update,
            (
                gameplay_2d::spawn_visible_notes,
                gameplay_2d::update_notes,
                gameplay_2d::size_note_ribbons,
                gameplay_2d::update_note_visuals,
                gameplay_2d::animate_judged_notes,
                gameplay_2d::update_holes,
                beat_guides::update_beat_guides,
                technique_coach::update_technique_coach,
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
                gameplay_3d::animate_judged_notes_3d,
                gameplay_3d::animate_note_ribbons_3d,
                gameplay_3d::update_holes_3d,
                technique_coach::update_technique_coach,
            )
                .chain()
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                ),
        );
        // Registered for reflection by `dev_capture`, so BRP can switch it.
        #[cfg(feature = "dev")]
        app.init_resource::<super::autoplay::Autoplay>();
    }
}
