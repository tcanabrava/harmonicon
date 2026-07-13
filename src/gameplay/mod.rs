// SPDX-License-Identifier: MIT

mod adaptive_difficulty;
mod bending_trainer;
mod clock;
mod countdown_overlay;
mod gameplay_2d;
mod gameplay_3d;
mod harmonica_overlay;
mod jam_session;
mod metronome_overlay;
mod modifier_legend;
pub mod note_tail_2d;
mod note_tail_3d;
pub mod note_visual_2d;
mod pause_menu;
mod phrase_overlay;
mod results;
mod song_progress_overlay;
pub mod twelve_bar_blues_overlay;
mod wait_freeze_overlay;

use bevy::prelude::*;
pub use crate::scoring::{HitQuality, NoteOutcome, classify_note, compute_points, sustain_points};
use crate::scoring::{
    AttackGate, VIBRATO_MIN_SWING_CENTS, WAH_MIN_SWING_FRAC, chord_is_sounding, combo_label,
    compute_multiplier, is_clean_attack, measured_oscillation_hz,
    measured_relative_oscillation_hz, oscillation_matches_rate, should_decay_combo,
};
use std::collections::HashMap;
use std::collections::HashSet;

use bevy::audio::Volume;

use crate::{
    audio_system::midi::{midi_to_freq_hz, note_to_midi},
    audio_system::pitch_detect::{AudioFrame, PitchEvent, PitchInfo, PitchRange},
    menu::{AppState, GameplayMode, SelectedSong},
    settings::AudioSettings,
    song::{SongManifest, chart::Modifier},
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
        .add_message::<BarChanged>()
        .add_message::<NoteScored>()
        .init_resource::<bending_trainer::TrainerKey>()
        .init_resource::<bending_trainer::TrainerTarget>()
        .init_resource::<bending_trainer::DrillState>()
        .init_resource::<jam_session::JamLoop>()
        .init_resource::<pause_menu::WaitForNoteMode>()
        .init_resource::<pause_menu::PracticeSpeed>()
        .init_resource::<pause_menu::SelectedPhraseIndex>()
        // Setup: shared pause menu + mode-specific scenes
        .add_systems(
            OnEnter(AppState::Playing),
            (
                reset_score,
                setup_scoring_config,
                adaptive_difficulty::setup_adaptive_difficulty,
                pause_menu::setup_pause_menu,
                gameplay_2d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play2D),
                gameplay_3d::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
                jam_session::setup.run_if(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
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
            (cleanup_gameplay, bending_trainer::save_drill_progress),
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
                bending_trainer::handle_escape,
            )
                .run_if(in_state(AppState::BendingTrainer)),
        )
        // Cleanup: shared entity despawn + restore camera on 3D exit
        .add_systems(OnExit(AppState::Playing), cleanup_gameplay)
        .add_systems(
            OnExit(AppState::Playing),
            gameplay_3d::restore_camera.run_if(|m: Res<GameplayMode>| *m == GameplayMode::Play3D),
        )
        // Pause input always runs during Playing (even when paused). The pause
        // buttons carry their own click/hover behaviour as inline `on(...)`
        // observers (see `setup_pause_menu`), so no button systems here.
        .add_systems(
            Update,
            pause_menu::handle_pause_input.run_if(in_state(AppState::Playing)),
        )
        // Apply live volume changes to the playing song (even while paused).
        .add_systems(
            Update,
            apply_music_volume
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
                pause_menu::update_practice_speed_label,
                pause_menu::update_phrase_selector_label,
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
                tick_clock,
                handle_loop_boundary,
                track_current_bar,
                collect_pitches,
                update_active_targets,
                score_notes,
                update_score_display,
                detect_song_end,
                note_tail_2d::animate_note_tails,
            )
                .chain()
                .in_set(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        )
        // Jam Session: live harmonica hole-map feedback from the mic.
        .add_systems(
            Update,
            jam_session::update_hole_map
                .after(GameplayLogic)
                .run_if(
                    in_state(AppState::Playing)
                        .and_then(|p: Res<Paused>| !p.0)
                        .and_then(|m: Res<GameplayMode>| *m == GameplayMode::JamSession),
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

// ── Shared resources ──────────────────────────────────────────────────────────

pub use clock::GameplayClock;

#[derive(Resource, Default)]
pub struct ActivePitches(pub Vec<PitchInfo>);

/// Enforces a fresh attack per note. A sustained pitch may satisfy only **one**
/// note: once it scores, the pitch is "consumed" and cannot score again until it
/// stops sounding and is articulated anew. Without this, a single held breath on
/// (say) G4 would clear every G4 note that later scrolls into its hit window.
/// Thin `Resource` wrapper around the generic [`AttackGate`] — see
/// `crate::scoring`, which also backs the Song Editor's Practice mode.
#[derive(Resource, Default)]
pub struct PitchGate(AttackGate<u8>);

impl std::ops::Deref for PitchGate {
    type Target = AttackGate<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PitchGate {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource, Default)]
pub struct MusicStarted(pub bool);

/// The set of MIDI note numbers this harp can actually produce (across every
/// hole/action/bend), from `Harmonica::build_valid_notes`. Keying on the MIDI
/// number (rather than a formatted name like `"G4"`) means comparisons are
/// integer equality — no per-frame string allocation, and no risk of an
/// enharmonic spelling mismatch (`"A#4"` vs `"Bb4"`) silently failing to match.
#[derive(Resource, Default)]
pub struct ValidHarpNotes(pub HashSet<u8>);

#[derive(Resource, Default)]
pub struct Score {
    pub points: u32,
    pub combo: u32,
    pub max_combo: u32,
    pub last_hit_time: f64, // clock time of the last successful hit, for decay
}

/// Hit/miss tally for one technique category, so the results screen can show
/// "your bends land, your overblows don't" instead of one blended accuracy
/// number — the diagnostic a self-taught player actually needs.
#[derive(Default, Clone, Copy)]
pub struct TechniqueStats {
    pub hits: u32,
    pub misses: u32,
}

impl TechniqueStats {
    pub fn total(&self) -> u32 {
        self.hits + self.misses
    }

    /// Hit rate in `[0.0, 1.0]`, or `None` if the technique never came up in
    /// this song (nothing to report, not "0% accurate").
    pub fn accuracy(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some(self.hits as f32 / total as f32)
        }
    }
}

/// Per-song hit tally shown on the results screen. Reset at the start of each
/// song. `good` are on-time/early Good hits; `delayed` are late Good hits.
#[derive(Resource, Default)]
pub struct SongStats {
    pub perfect: u32,
    pub good: u32,
    pub delayed: u32,
    pub miss: u32,
    /// Sum of compensated timing offsets (seconds) for every hit. Divide by hit
    /// count (`perfect + good + delayed`) to get the mean offset. Positive means
    /// the player is still sounding notes after the target time even with the
    /// current `input_latency_ms` applied; increasing that setting by the mean
    /// (in ms) should centre the distribution.
    pub offset_sum: f64,
    /// Notes with no technique modifier at all — the baseline every other
    /// category is implicitly compared against.
    pub normal: TechniqueStats,
    pub bend: TechniqueStats,
    pub overblow: TechniqueStats,
    pub overdraw: TechniqueStats,
    /// Chromatic harmonica's slide button — the chromatic equivalent of a
    /// diatonic bend.
    pub slide: TechniqueStats,
    pub vibrato: TechniqueStats,
    pub wah: TechniqueStats,
    /// Onset hits where [`is_clean_attack`] confirmed no *other*
    /// harp-producible pitch sounded alongside the expected one — separate
    /// from the technique buckets above (which are keyed by chart modifier,
    /// not attack cleanliness) and tallied for every hit regardless of its
    /// modifiers, or lack of them. Never tallied for a chord/octave-split
    /// note (see `chord` below) — clean-attack and chord are mutually
    /// exclusive checks on the same onset.
    pub clean_attack: TechniqueStats,
    /// Onset hits/misses for chord/octave-split notes (non-empty
    /// `ScheduledNote::chord_pitches`) — one tally per sibling note of the
    /// chord, not per musical chord struck, same as every other technique
    /// bucket here counts per-note.
    pub chord: TechniqueStats,
}

impl SongStats {
    /// Tallies a note's hit/miss outcome against every technique modifier it
    /// carries (a note can have up to two, e.g. Bend + Vibrato — it counts
    /// toward both), or `normal` if it has none.
    fn record_technique(&mut self, modifiers: &[Modifier], hit: bool) {
        if modifiers.is_empty() {
            bump(&mut self.normal, hit);
            return;
        }
        for m in modifiers {
            let bucket = match m {
                Modifier::Bend { .. } => &mut self.bend,
                Modifier::Overblow => &mut self.overblow,
                Modifier::Overdraw => &mut self.overdraw,
                Modifier::Slide => &mut self.slide,
                Modifier::Vibrato { .. } => &mut self.vibrato,
                Modifier::WahWah { .. } => &mut self.wah,
            };
            bump(bucket, hit);
        }
    }
}

fn bump(stats: &mut TechniqueStats, hit: bool) {
    if hit {
        stats.hits += 1;
    } else {
        stats.misses += 1;
    }
}

/// Gameplay-clock time at which the song's content ends (so the results screen
/// can appear). `INFINITY` for looping songs, which never finish.
#[derive(Resource)]
pub struct SongEnd(pub f64);

impl Default for SongEnd {
    fn default() -> Self {
        Self(f64::INFINITY)
    }
}

/// Extra seconds after the last note before the results screen, so the final
/// notes ring out.
const SONG_END_TAIL: f64 = 2.5;

#[derive(Resource, Default)]
pub struct HitFeedback {
    pub quality: Option<HitQuality>,
    pub timer: f32,
}

/// Notes currently inside the good-hit window: (hole, is_blow).
/// Updated every frame so hole-display systems can show a target hint.
#[derive(Resource, Default)]
pub struct ActiveTargets(pub Vec<(u8, bool)>);

/// Emitted by [`score_notes`] whenever `Score` moves (a fresh hit, a note's
/// sustain bonus landing, a miss resetting the combo, or the combo decaying
/// from inactivity) — `update_score_display` reads this instead of
/// re-`format!`ing the score/combo `Text` every frame regardless of whether
/// either number actually changed. `quality` is only `Some` for a fresh hit,
/// which is what tells `update_score_display` to set the "PERFECT!"/"GOOD"
/// feedback label *once* rather than every frame of its fade — the alpha fade
/// itself stays a per-frame animation, driven by `HitFeedback` directly, not
/// this message.
#[derive(Message)]
pub struct NoteScored {
    pub quality: Option<HitQuality>,
}

// ── Shared components ─────────────────────────────────────────────────────────

#[derive(Component, Default, Clone)]
pub struct GameplayRoot;

#[derive(Component)]
pub struct NoteVisual {
    /// Index into [`SongNotes::notes`] — this entity is purely a rendering
    /// of that note's score state, spawned only while it's within
    /// `LOOKAHEAD` of the playhead (see `gameplay_2d::spawn_visible_notes`)
    /// and despawned once scrolled past, independent of the note's actual
    /// score state (which lives on regardless of whether anything currently
    /// renders it).
    pub note_id: usize,
}

/// One chart note's score state. Plain data, not an ECS component — lives in
/// [`SongNotes`], independent of whatever render entity (if any) currently
/// represents it on screen. That split is what lets `gameplay_2d`/
/// `gameplay_3d` spawn note *visuals* only for a `LOOKAHEAD` window around
/// the playhead instead of the whole song up front, and lets
/// `handle_loop_boundary` reset a note's state without needing it to have a
/// live entity at all.
#[derive(Clone)]
pub struct ScheduledNote {
    pub time: f64,
    /// Note length in seconds; long notes reward sustaining the pitch.
    pub duration: f64,
    pub hole: u8,
    pub is_blow: bool,
    /// The MIDI note number this note expects, pre-computed at spawn
    /// (`None` for a hole/action/bend combination the harp can't actually
    /// produce — see [`target_pitch`] — which can never be hit).
    pub expected_pitch: Option<u8>,
    pub hit: bool,
    pub missed: bool,
    /// Seconds the expected pitch has been held since the onset was hit.
    pub held: f64,
    /// Set once the sustain window has closed and its bonus was awarded.
    pub sustain_scored: bool,
    /// Technique modifiers from the chart (bend, vibrato, etc.).
    /// Used to trigger fx sounds when the note is hit.
    pub modifiers: Vec<Modifier>,
    /// `(clock time, cents-from-expected-pitch)`, sampled once per frame
    /// while held — used to verify a declared `vibrato` was actually played
    /// at roughly its declared `oscillation_hz`, not just declared. Storing
    /// the timestamp (rather than trusting sample order) keeps the measured
    /// rate frame-rate independent.
    pub pitch_samples: Vec<(f64, f32)>,
    /// `(clock time, input loudness RMS)`, sampled once per frame while
    /// held — used to verify a declared `wah-wah` was actually played at
    /// roughly its declared `oscillation_hz`, not just declared.
    pub amp_samples: Vec<(f64, f32)>,
    /// Index into `adaptive_difficulty::AdaptiveDifficulty::sections` — the
    /// musical phrase this note belongs to. A note only exists in
    /// `SongNotes` at all once adaptive difficulty has unlocked it (see
    /// `adaptive_difficulty::unlocked_flags`), so this is always a real
    /// section, not an `Option`; charts with no `phrase` tags get a single
    /// implicit section (index 0) covering the whole track.
    pub phrase_section: usize,
    /// The full set of expected MIDI pitches for this note's chart
    /// `TrackItem`, shared identically by every sibling `ScheduledNote` the
    /// item produced (one per `NoteEvent` — see `gameplay_2d::
    /// build_combined_notes`/`gameplay_3d::build_notes_3d`). Empty for an
    /// ordinary single-event item, which is the signal `score_notes` uses to
    /// skip the simultaneity check entirely — nothing about single-note
    /// charts changes. Non-empty (a `PlayMode::Chord`/`Split` item — two or
    /// more `events` at the same `time`) means this note's own onset only
    /// counts as "playing" while *every* pitch in the set sounds together,
    /// not just its own — the chord-target primitive `docs/lessons_plan.md`
    /// calls for, built on the chart format's existing multi-event
    /// `TrackItem` shape rather than a new schema field.
    pub chord_pitches: Vec<u8>,
}

/// Every note in the loaded chart, sorted by `time` ascending (matches chart
/// authoring order; nothing re-sorts `chart.track` elsewhere either). The
/// scoring systems (`score_notes`, `handle_loop_boundary`,
/// `update_active_targets`) read and mutate this directly instead of
/// querying ECS components, so a note's score state exists independent of
/// whether it currently has a render entity.
#[derive(Resource, Default)]
pub struct SongNotes {
    pub notes: Vec<ScheduledNote>,
    /// Index of the first not-fully-resolved note (not `missed`, and not
    /// both `hit` and `sustain_scored`). Advanced forward by `score_notes`
    /// as a prefix of notes finishes for good; rewound by
    /// `handle_loop_boundary` on a loop wrap, since notes before the loop's
    /// start are no longer "permanently done" once it can replay them.
    /// Purely a per-frame scan-avoidance optimization — correctness never
    /// depends on its exact value, only that it's `<=` the true first
    /// unresolved index.
    pub cursor: usize,
}

/// Indices of notes that should have a spawned visual at `elapsed` but don't
/// yet (per `already_spawned`) — the windowing logic shared by
/// `gameplay_2d::spawn_visible_notes` and `gameplay_3d::spawn_visible_notes_3d`.
/// `notes` must be sorted by `time` ascending (as `SongNotes::notes` always
/// is). A note's window is open from `LOOKAHEAD` seconds before its `time`
/// until `elapsed` passes it (recycling/despawning is each mode's own
/// concern, based on how far the note has visually scrolled — this only
/// decides when a *new* visual should appear).
pub(super) fn notes_needing_spawn(
    notes: &[ScheduledNote],
    already_spawned: &HashSet<usize>,
    elapsed: f64,
) -> Vec<usize> {
    // Sorted by time, so this is the first index whose window could
    // possibly be open — no need to consider anything before it.
    let start = notes.partition_point(|n| n.time + LOOKAHEAD < elapsed);
    let mut result = Vec::new();
    for (i, note) in notes.iter().enumerate().skip(start) {
        if note.time - LOOKAHEAD > elapsed {
            break; // sorted — nothing further out needs spawning yet either.
        }
        if !already_spawned.contains(&i) {
            result.push(i);
        }
    }
    result
}

/// Index of the first not-yet-resolved, *playable* note in `notes[cursor..]`
/// that has already reached `clock_time` — the freeze condition for
/// `pause_menu::WaitForNoteMode`. `tick_clock` uses the index both to decide
/// whether to freeze and to label the wait-freeze prompt with which note it's
/// waiting on. `notes` sorted by `time` (as `SongNotes::notes` always is)
/// lets this stop scanning as soon as it reaches a note that isn't due yet,
/// same as `score_notes`.
///
/// Notes with no `expected_pitch` (a hole/action the harp can't produce —
/// see `target_pitch`) are excluded: they can never be hit, so freezing on
/// one would wait forever.
pub(super) fn first_due_unresolved_note(
    notes: &[ScheduledNote],
    cursor: usize,
    clock_time: f64,
) -> Option<usize> {
    for (i, note) in notes.iter().enumerate().skip(cursor) {
        if note.time > clock_time {
            break;
        }
        if note.expected_pitch.is_some() && !note.hit && !note.missed {
            return Some(i);
        }
    }
    None
}

#[derive(Component)]
#[require(HoleState)]
pub struct HoleCell(pub u8);

#[derive(Component, Default)]
pub struct HoleState {
    pub brightness: f32,
    pub is_blow: bool,
}

// Score HUD marker components
#[derive(Component)]
pub struct ScoreText;
#[derive(Component)]
pub struct ComboText;
#[derive(Component)]
pub struct FeedbackText;

/// Set to true while gameplay is paused; all update chains gate on `!paused`.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// Marks the music audio entity so it can be found for pause/resume.
#[derive(Component)]
pub struct MusicPlayer;

/// Scoring parameters resolved from the song's chart at game start.
/// Falls back to sensible defaults if the chart doesn't specify them.
#[derive(Resource)]
pub struct ScoringConfig {
    pub perfect_window: f64,
    pub good_window: f64,
    pub miss_window: f64,
    pub combo_enabled: bool,
    pub base_multiplier: f32,
    pub step_multiplier: f32,
    pub max_multiplier: f32,
    /// Seconds without a hit before the combo resets. `None` = never decays.
    pub decay_secs: Option<f64>,
    /// Beats per bar resolved from `timing.time_signature_map` (or `song.time_signature`).
    pub beats_per_bar: f64,
    /// Bonus points per technique (keyed by technique name) awarded on a hit,
    /// from the chart's `scoring.style_bonus`. Empty = no style points.
    pub style_bonus: HashMap<String, f32>,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            perfect_window: 0.060,
            good_window: 0.130,
            miss_window: 0.130,
            combo_enabled: true,
            base_multiplier: 1.0,
            step_multiplier: 0.1,
            max_multiplier: 4.0,
            decay_secs: None,
            beats_per_bar: 4.0,
            style_bonus: HashMap::new(),
        }
    }
}

/// Active loop region. When `active`, the gameplay clock resets to `start_time`
/// each time it passes `end_time`, repeating that section indefinitely.
#[derive(Resource, Default)]
pub struct LoopConfig {
    pub active: bool,
    pub start_time: f64,
    pub end_time: f64,
}

// ── Shared constants ──────────────────────────────────────────────────────────

pub const COUNTDOWN: f64 = 3.0;
pub const HIT_H_PCT: f32 = 7.0;
pub const LOOKAHEAD: f64 = 3.0;

// ── Shared pure helpers ───────────────────────────────────────────────────────

/// Parse the beat count from an optional "N/D" time-signature string.
pub fn parse_beats(time_sig: Option<&str>) -> f64 {
    time_sig
        .and_then(|s| s.split('/').next())
        .and_then(|n| n.parse::<f64>().ok())
        .unwrap_or(4.0)
}

/// Seconds per bar given BPM and beat count.
pub fn secs_per_bar(bpm: f64, beats: f64) -> f64 {
    (60.0 / bpm) * beats
}

/// Which of the 12 bars in a twelve-bar cycle the clock is currently on.
pub fn current_bar_index(clock: f64, secs_per_bar: f64) -> usize {
    (clock.max(0.0) / secs_per_bar) as usize % 12
}

/// The bar `track_current_bar` last computed — shared so
/// `twelve_bar_blues_overlay::update_bar` and `jam_session::update_hole_map`
/// don't each recompute it (previously from two different beats-per-bar
/// sources that could disagree: `ScoringConfig::beats_per_bar`, which honors
/// a chart's `time_signature_map` override, vs `JamHoleGuide`'s own copy,
/// which didn't).
#[derive(Resource, Default)]
pub struct CurrentBar(pub usize);

/// Emitted by [`track_current_bar`] whenever the current bar changes,
/// forward or (on a loop rewind) backward — lets `update_bar` recolor the
/// 12-bar grid only on an actual bar change instead of writing
/// `BackgroundColor` on all 12 cells every frame forever. `update_hole_map`
/// doesn't need this: it repaints every frame anyway for live mic feedback,
/// so it just reads `CurrentBar` directly.
#[derive(Message)]
pub struct BarChanged(pub usize);

/// Computes the current bar once per frame (see `GameplayLogic` — must run
/// after `handle_loop_boundary` so a loop rewind is reflected the same
/// frame) and emits [`BarChanged`] on a change, detected by recomputing from
/// the clock each frame rather than advancing an incrementing counter — the
/// same trick `phrase_overlay::watch_phrase_boundaries` uses so a backward
/// jump is picked up for free instead of needing special-case handling.
fn track_current_bar(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    config: Res<ScoringConfig>,
    mut current: ResMut<CurrentBar>,
    mut last: Local<Option<usize>>,
    mut changed: MessageWriter<BarChanged>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let bpm = manifest.chart.song.tempo_bpm as f64;
    let spb = secs_per_bar(bpm, config.beats_per_bar);
    let bar = current_bar_index(clock.get(), spb);
    current.0 = bar;
    if *last != Some(bar) {
        changed.write(BarChanged(bar));
    }
    *last = Some(bar);
}

/// A loop range only makes sense once `end_time` is strictly after
/// `start_time` — the single rule `LoopConfig::active` is recomputed from
/// whenever a new range is requested (see `song_progress_overlay::
/// RequestLoopRange`), so a degenerate zero-width drag on the progress bar
/// cleanly ends up inactive instead of a stale or nonsensical range.
pub fn loop_range_valid(start_time: f64, end_time: f64) -> bool {
    end_time > start_time
}

/// Resolve a track item's start time in seconds, preferring an explicit `time`
/// and falling back to converting its `tick` through the tempo map.
pub fn resolve_item_time(
    item: &crate::song::chart::TrackItem,
    timing: &crate::song::chart::Timing,
) -> f64 {
    item.time.unwrap_or_else(|| {
        let tick = item.tick.unwrap_or(0);
        crate::song::chart::tick_to_seconds(tick, timing.resolution, &timing.tempo_map)
    })
}

/// The latest moment any note finishes (start + duration) across the track, in
/// seconds. Drives when the song's content ends. Zero for an empty track.
pub fn last_note_end(
    track: &[crate::song::chart::TrackItem],
    timing: &crate::song::chart::Timing,
) -> f64 {
    track
        .iter()
        .map(|item| resolve_item_time(item, timing) + item.duration)
        .fold(0.0_f64, f64::max)
}

/// The MIDI note the player must actually produce for a note. A `bend`
/// shifts the note's natural pitch by its semitones (negative = down, and
/// rounded to the nearest whole semitone — the actual bent pitch is
/// continuous, but the matched target is discrete), so the bend is
/// *validated* by scoring — playing the unbent note no longer counts.
/// `None` if `natural` isn't a parseable note name (e.g. the "—" placeholder
/// for a hole/direction the harp can't produce) or the shifted result falls
/// outside the valid MIDI range.
pub fn target_pitch(natural: &str, modifiers: &[Modifier]) -> Option<u8> {
    let bend: i32 = modifiers
        .iter()
        .find_map(|m| match m {
            Modifier::Bend { semitones, .. } => Some(semitones.round() as i32),
            _ => None,
        })
        .unwrap_or(0);
    let midi = note_to_midi(natural)? + bend;
    (0..=127).contains(&midi).then_some(midi as u8)
}

/// Style-bonus points awarded for a hit note's techniques, summed over its
/// modifiers using the chart's `style_bonus` table (keyed by technique name).
pub fn style_bonus_points(modifiers: &[Modifier], table: &HashMap<String, f32>) -> f32 {
    modifiers
        .iter()
        .map(|m| table.get(modifier_fx_key(m)).copied().unwrap_or(0.0))
        .sum()
}

/// Vibrato and wah are hand/throat articulations sustained *through* the
/// note, not a pitch shift validated by the onset alone (unlike a bend, whose
/// `expected_pitch` already encodes the bent target). Their style bonus is
/// deferred to the end of the sustain window and only paid out if
/// [`technique_confirmed`] finds the player actually wobbled the pitch/level.
fn is_sustained_technique(modifier: &Modifier) -> bool {
    matches!(modifier, Modifier::Vibrato { .. } | Modifier::WahWah { .. })
}

/// How far a measured vibrato/wah rate may drift from the chart's declared
/// `oscillation_hz` and still count — generous, since hand technique speed
/// varies naturally between players and even between notes.
const OSCILLATION_RATE_TOLERANCE_FRAC: f32 = 0.4;

/// Did the player actually perform this sustained technique, judged from the
/// pitch/loudness samples collected while the note was held — both that it
/// swung enough to be a real wobble, and that it swung at roughly the
/// chart's declared `oscillation_hz` rather than some unrelated rate.
/// Non-sustained modifiers (bend, overblow, overdraw) are validated at onset
/// instead — this always returns `true` for them since it shouldn't be asked.
fn technique_confirmed(
    modifier: &Modifier,
    pitch_samples: &[(f64, f32)],
    amp_samples: &[(f64, f32)],
) -> bool {
    match modifier {
        Modifier::Vibrato { oscillation_hz, .. } => {
            measured_oscillation_hz(pitch_samples, VIBRATO_MIN_SWING_CENTS).is_some_and(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        Modifier::WahWah { oscillation_hz, .. } => {
            measured_relative_oscillation_hz(amp_samples, WAH_MIN_SWING_FRAC).is_some_and(|hz| {
                oscillation_matches_rate(hz, *oscillation_hz, OSCILLATION_RATE_TOLERANCE_FRAC)
            })
        }
        _ => true,
    }
}

/// The currently-detected frequency (Hz) matching `midi` (a MIDI note
/// number), or `None` if that exact pitch isn't among the detected pitches
/// this frame.
fn active_frequency_for(active: &[PitchInfo], midi: u8) -> Option<f32> {
    active.iter().find(|p| p.midi == midi).map(|p| p.frequency)
}

/// RMS loudness of a block of audio samples.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

// ── Shared systems ────────────────────────────────────────────────────────────

fn reset_score(
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut paused: ResMut<Paused>,
    mut gate: ResMut<PitchGate>,
) {
    *score = Score::default();
    *stats = SongStats::default();
    *feedback = HitFeedback::default();
    paused.0 = false;
    *gate = PitchGate::default();
}

/// Semitone margin added on each side of the harmonica's natural range when
/// sizing the pitch detector — covers bends/overblows landing just past a
/// charted note plus a little slop before a clean attack. Also used by the
/// bend trainer, which derives its own range from the current key.
pub(crate) const PITCH_RANGE_MARGIN_SEMITONES: f32 = 1.0;

fn setup_scoring_config(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut config: ResMut<ScoringConfig>,
    mut loop_cfg: ResMut<LoopConfig>,
    mut song_end: ResMut<SongEnd>,
    mut pitch_range: ResMut<PitchRange>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let s = &chart.scoring;

    // Size the detector to this harmonica instead of a fixed constant, so a
    // Low-F/Low-D harp's low notes aren't cut off by a floor tuned for
    // standard keys (see TODO.md).
    *pitch_range = chart
        .harmonica
        .frequency_range()
        .map(|(lo, hi)| PitchRange::from_freqs([lo, hi], PITCH_RANGE_MARGIN_SEMITONES))
        .unwrap_or_default();

    config.perfect_window = s.perfect_window_ms as f64 / 1000.0;
    config.good_window = s.good_window_ms as f64 / 1000.0;
    config.miss_window = s.miss_window_ms as f64 / 1000.0;

    // Resolve beats per bar: time_signature_map at tick=0 takes precedence over song field.
    let beats_str = chart
        .timing
        .time_signature_map
        .as_deref()
        .and_then(|m| crate::song::chart::time_sig_at_tick(0, m))
        .or(chart.song.time_signature.as_deref());
    config.beats_per_bar = parse_beats(beats_str);

    if let Some(combo) = &s.combo {
        config.combo_enabled = combo.enabled;
        config.base_multiplier = combo.base_multiplier;
        config.step_multiplier = combo.step_multiplier;
        config.max_multiplier = combo.max_multiplier;
        config.decay_secs = combo.decay_ms.map(|ms| ms as f64 / 1000.0);
    }

    // Per-technique style points awarded when a technique note is hit.
    config.style_bonus = s.style_bonus.clone().unwrap_or_default();

    // Set up loop section if the chart requests repeat playback.
    *loop_cfg = LoopConfig::default();
    if let Some(ls) = &chart.loop_section
        && ls.repeat == Some(true)
    {
        let track = &chart.track;
        let si = ls.start_index;
        let ei = ls.end_index;
        if si < track.len() && ei < track.len() && si <= ei {
            loop_cfg.active = true;
            loop_cfg.start_time = resolve_item_time(&track[si], &chart.timing);
            loop_cfg.end_time = resolve_item_time(&track[ei], &chart.timing) + track[ei].duration;
            info!(
                "Loop section ({:?}): {:.2}s – {:.2}s",
                ls.section_type, loop_cfg.start_time, loop_cfg.end_time,
            );
        }
    }

    // Song end = last note's end + a tail, so the results screen appears once the
    // content finishes. Looping songs never end.
    song_end.0 = if loop_cfg.active {
        f64::INFINITY
    } else {
        last_note_end(&chart.track, &chart.timing) + SONG_END_TAIL
    };

    info!(
        "Scoring config: perfect={:.0}ms good={:.0}ms miss={:.0}ms combo={} beats/bar={}",
        config.perfect_window * 1000.0,
        config.good_window * 1000.0,
        config.miss_window * 1000.0,
        config.combo_enabled,
        config.beats_per_bar,
    );
}

/// Ticks the single [`GameplayClock`] all gameplay systems read. Once the
/// countdown finishes and the song's music starts, the clock is kept
/// anchored to the `AudioSink` playback position instead of free-running on
/// `Time::delta` — otherwise decoder start-up delay and frame hitches drift
/// the notes out of sync with the audio over a long song. Jam Session has no
/// long track to drift against and stays frame-timer driven (metronome-led).
///
/// Two things can take the clock off that path, and both work the same way:
/// the music sink should or shouldn't be audible right now
/// (`should_play`), computed once and compared against the sink's own
/// `is_paused()` so `AudioSink::pause`/`play` only ever fires on the actual
/// edge — calling it ~60 times a second turned out to visibly upset the
/// audio backend (observed as odd behaviour in the *microphone* input, a
/// fully separate pipeline, which only makes sense if repeatedly toggling
/// the output stream was disturbing a shared audio graph/server).
///
/// - `WaitForNoteMode` on and a playable note due and still unhit
///   (`first_due_unresolved_note`): the clock simply isn't advanced this
///   frame, holding it exactly at the hit line. `score_notes` keeps
///   re-judging the same held instant every frame, so the moment the player
///   plays the note it scores (typically a Perfect, since the offset never
///   moved) and the very next frame the condition is false again. Jam
///   Session never populates `SongNotes`, so this is a no-op there.
/// - `PracticeSpeed` below 100%: real time-stretched audio isn't
///   implemented, so the sink just pauses instead of playing pitch-shifted,
///   and the clock free-runs on `Time::delta` scaled by the speed instead of
///   anchoring (the sink's position wouldn't mean anything at the wrong
///   speed anyway). Coming back to 100% re-seeks the sink to the clock's
///   current position (`GameplayClock::rewind_to`) before resuming it, since
///   it sat still the whole time the clock kept moving.
fn tick_clock(
    mut clock: ResMut<GameplayClock>,
    time: Res<Time>,
    mode: Res<GameplayMode>,
    music_started: Res<MusicStarted>,
    wait_mode: Res<pause_menu::WaitForNoteMode>,
    mut wait_freeze: ResMut<wait_freeze_overlay::WaitFreezeState>,
    practice_speed: Res<pause_menu::PracticeSpeed>,
    song_notes: Res<SongNotes>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    let due = wait_mode
        .0
        .then(|| first_due_unresolved_note(&song_notes.notes, song_notes.cursor, clock.get()))
        .flatten();
    // Gated so `ResMut`'s change detection (which `wait_freeze_overlay`'s
    // prompt reacts to) only fires on an actual transition, not every frame.
    if due != wait_freeze.0 {
        wait_freeze.0 = due;
    }

    let full_speed = practice_speed.0 == 1.0;
    let should_play = due.is_none() && full_speed;
    if let Ok(sink) = sinks.single() {
        if should_play && sink.is_paused() {
            let t = clock.get();
            clock.rewind_to(t, Some(sink));
            sink.play();
        } else if !should_play && !sink.is_paused() {
            sink.pause();
        }
    }

    if due.is_some() {
        return;
    }
    if !full_speed {
        clock.advance(time.delta_secs_f64() * practice_speed.0 as f64, None);
        return;
    }

    let dt = time.delta_secs_f64();
    let audio_pos = sinks
        .single()
        .ok()
        .filter(|sink| should_anchor_to_sink(clock.get(), music_started.0, &mode, sink.empty()))
        .map(|sink| sink.position().as_secs_f64());
    clock.advance(dt, audio_pos);
}

/// Whether `tick_clock` should anchor the clock to the music sink's reported
/// position this frame, rather than free-running on frame delta: past the
/// countdown, once music has actually started, and never in Jam Session (no
/// long track to drift against there — see `tick_clock`'s doc comment).
///
/// Also `false` once the sink's queue is empty. A finished sink's
/// `position()` freezes at its last value instead of continuing to advance,
/// so anchoring to it would make `advance_clock` repeatedly snap the clock
/// back to that frozen point once real time drifts past
/// `SNAP_THRESHOLD_SECS` — better to free-run past that point instead.
fn should_anchor_to_sink(
    clock: f64,
    music_started: bool,
    mode: &GameplayMode,
    sink_empty: bool,
) -> bool {
    clock >= 0.0 && music_started && *mode != GameplayMode::JamSession && !sink_empty
}

/// Index range (into `notes`, sorted by `time`) that a loop wrap must reset
/// `hit`/`missed`/`held`/`sustain_scored` for: `start_time..end_time`,
/// extended by `LOOKAHEAD` past `end_time` since `notes_needing_spawn` can
/// preview a note that far ahead of the clock before the loop ever actually
/// reaches it.
pub(super) fn loop_reset_range(
    notes: &[ScheduledNote],
    start_time: f64,
    end_time: f64,
) -> (usize, usize) {
    let start_idx = notes.partition_point(|n| n.time < start_time);
    let end_idx = notes.partition_point(|n| n.time <= end_time + LOOKAHEAD);
    (start_idx, end_idx)
}

fn handle_loop_boundary(
    loop_cfg: Res<LoopConfig>,
    mut clock: ResMut<GameplayClock>,
    mut song_notes: ResMut<SongNotes>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    if !loop_cfg.active || clock.get() < loop_cfg.end_time {
        return;
    }
    // `rewind_to` also seeks the sink, so `tick_clock`'s anchoring doesn't
    // see it far ahead of the just-rewound clock next frame and drag the
    // clock forward again — see the doc comment on `GameplayClock`.
    clock.rewind_to(loop_cfg.start_time, sinks.single().ok());

    // `notes` is sorted by `time`, so the reset range is one contiguous
    // slice — binary search it instead of scanning the whole song.
    let (start_idx, end_idx) =
        loop_reset_range(&song_notes.notes, loop_cfg.start_time, loop_cfg.end_time);
    for note in &mut song_notes.notes[start_idx..end_idx] {
        note.hit = false;
        note.missed = false;
        note.held = 0.0;
        note.sustain_scored = false;
    }
    // These notes are playable again, so `score_notes`'s cursor (which only
    // ever advances past *permanently* resolved notes) can't stay ahead of
    // them — `min` in case the loop wraps before ever reaching this section.
    song_notes.cursor = song_notes.cursor.min(start_idx);
}

fn collect_pitches(mut reader: MessageReader<PitchEvent>, mut active: ResMut<ActivePitches>) {
    for ev in reader.read() {
        active.0 = ev.0.clone();
    }
}

fn update_active_targets(
    clock: Res<GameplayClock>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    song_notes: Res<SongNotes>,
    mut targets: ResMut<ActiveTargets>,
) {
    targets.0.clear();
    if clock.get() < 0.0 {
        return;
    }
    // Shift the judgment point back by the microphone pipeline latency so the
    // highlighted hole tracks what the player is *actually* hearing, not what
    // the raw clock says.
    let judged = clock.get() - audio.input_latency_ms as f64 / 1000.0;
    // Starting from `score_notes`'s cursor (possibly a frame stale — that's
    // fine, it only ever lags a monotonically-advancing lower bound) means
    // this never re-scans notes long done. `notes` is sorted by `time`, so
    // once a not-yet-due note is too far out, everything after it is too.
    for note in &song_notes.notes[song_notes.cursor..] {
        if note.time > judged + config.good_window {
            break;
        }
        if note.hit || note.missed {
            continue;
        }
        if (judged - note.time).abs() <= config.good_window {
            targets.0.push((note.hole, note.is_blow));
        }
    }
}

fn score_notes(
    clock: Res<GameplayClock>,
    time: Res<Time>,
    active: Res<ActivePitches>,
    frame: Res<AudioFrame>,
    valid_notes: Res<ValidHarpNotes>,
    config: Res<ScoringConfig>,
    audio: Res<AudioSettings>,
    mut song_notes: ResMut<SongNotes>,
    mut score: ResMut<Score>,
    mut stats: ResMut<SongStats>,
    mut feedback: ResMut<HitFeedback>,
    mut gate: ResMut<PitchGate>,
    mut scored: MessageWriter<NoteScored>,
) {
    if clock.get() < 0.0 {
        return;
    }
    let dt = time.delta_secs_f64();
    // Compensate for microphone pipeline latency: a pitch detected at clock T
    // was actually played at T - latency. Shift the judgment window accordingly.
    let judged = clock.get() - audio.input_latency_ms as f64 / 1000.0;

    if config.combo_enabled
        && should_decay_combo(
            score.combo,
            clock.get(),
            score.last_hit_time,
            config.decay_secs,
        )
    {
        score.combo = 0;
        scored.write(NoteScored { quality: None });
    }

    let harp_pitches: HashSet<u8> = active
        .0
        .iter()
        .map(|p| p.midi)
        .filter(|m| valid_notes.0.contains(m))
        .collect();

    // Re-arm any pitch the player has stopped sounding, so its next attack is
    // fresh. Pitches still held remain consumed and can't score again.
    gate.release_absent(|p| harp_pitches.contains(&p));

    // A prefix of `notes` (sorted by `time`) that's permanently resolved
    // (missed, or hit and fully sustained) never needs visiting again —
    // advance past it so a long chart's already-finished notes don't cost a
    // scan every frame. A later note occasionally resolving before an
    // earlier still-pending one (e.g. a chord) is fine: the cursor just
    // stays put until that earlier one finishes too.
    while song_notes.cursor < song_notes.notes.len() {
        let n = &song_notes.notes[song_notes.cursor];
        if n.missed || (n.hit && n.sustain_scored) {
            song_notes.cursor += 1;
        } else {
            break;
        }
    }

    // Not-yet-hit-or-missed notes are classified in a second pass below,
    // ordered by |offset| (closest to the judged instant first) rather than
    // array order, so when two same-pitch notes overlap the hit window,
    // whichever is actually due consumes the attack — not just whichever
    // happened to be classified first.
    let mut pending: Vec<usize> = Vec::new();
    let len = song_notes.notes.len();

    for i in song_notes.cursor..len {
        let note = &mut song_notes.notes[i];
        if note.missed {
            continue;
        }

        // Already-hit notes are in their sustain phase: reward holding the pitch
        // through the note's length, then award the bonus once when it ends.
        if note.hit {
            if note.sustain_scored {
                continue;
            }
            if clock.get() < note.time + note.duration {
                // The held pitch stays "consumed" by the gate, so checking the
                // raw detected set keeps crediting this same note's sustain.
                if note
                    .expected_pitch
                    .is_some_and(|m| harp_pitches.contains(&m))
                {
                    note.held += dt;
                }
                // Track pitch/loudness through the hold so a declared vibrato
                // or wah can be verified (rather than trusted) once it ends.
                if note.modifiers.iter().any(is_sustained_technique) {
                    if let Some(midi) = note.expected_pitch
                        && let Some(hz) = active_frequency_for(&active.0, midi)
                    {
                        let expected_hz = midi_to_freq_hz(midi as f32);
                        note.pitch_samples
                            .push((clock.get(), 1200.0 * (hz / expected_hz).log2()));
                    }
                    note.amp_samples.push((clock.get(), rms(&frame.samples)));
                }
            } else {
                score.points += sustain_points(note.held, note.duration);

                let sustained: Vec<Modifier> = note
                    .modifiers
                    .iter()
                    .filter(|&x| is_sustained_technique(x))
                    .cloned()
                    .collect();
                if !sustained.is_empty() {
                    let (verified, unverified): (Vec<Modifier>, Vec<Modifier>) =
                        sustained.into_iter().partition(|m| {
                            technique_confirmed(m, &note.pitch_samples, &note.amp_samples)
                        });
                    if !verified.is_empty() {
                        score.points +=
                            style_bonus_points(&verified, &config.style_bonus).round() as u32;
                        stats.record_technique(&verified, true);
                    }
                    if !unverified.is_empty() {
                        stats.record_technique(&unverified, false);
                    }
                }
                note.sustain_scored = true;
                scored.write(NoteScored { quality: None });
            }
            continue;
        }

        // Anything further out than `good_window` classifies as `TooEarly`
        // regardless of `playing` (see `classify_note`) — a guaranteed no-op
        // match arm below. `notes` is sorted by `time`, so once one note is
        // this far out, every note after it is too — stop scanning outright
        // instead of just skipping the push, so a long chart's untouched
        // future notes cost nothing per frame, not even a visit.
        let offset = judged - note.time;
        if offset < -config.good_window {
            break;
        }
        pending.push(i);
    }

    pending.sort_by(|&a, &b| {
        let offset_a = (judged - song_notes.notes[a].time).abs();
        let offset_b = (judged - song_notes.notes[b].time).abs();
        offset_a
            .partial_cmp(&offset_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for i in pending {
        let note = &mut song_notes.notes[i];
        let offset = judged - note.time;
        // A note counts as "playing" only on a fresh attack: the pitch must be
        // sounding and not already consumed by an earlier note in this sustain.
        // A note with no valid `expected_pitch` (the harp can't produce it) can
        // never be "playing". A chord/octave-split note (non-empty
        // `chord_pitches`) additionally requires every sibling pitch of its
        // `TrackItem` to be sounding at the same instant — its own freshness
        // alone isn't enough, or a chord could be "hit" one note at a time.
        let playing = note.expected_pitch.is_some_and(|m| {
            gate.is_fresh(m, harp_pitches.contains(&m))
                && (note.chord_pitches.is_empty()
                    || chord_is_sounding(&note.chord_pitches, &harp_pitches))
        });

        match classify_note(
            offset,
            playing,
            config.perfect_window,
            config.good_window,
            config.miss_window,
        ) {
            NoteOutcome::Missed => {
                note.missed = true;
                stats.miss += 1;
                stats.record_technique(&note.modifiers, false);
                if !note.chord_pitches.is_empty() {
                    bump(&mut stats.chord, false);
                }
                if config.combo_enabled {
                    score.combo = 0;
                }
                scored.write(NoteScored { quality: None });
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap | NoteOutcome::Waiting => {}
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                // Vibrato/wah are judged from the sustain, not the onset — see
                // the sustain branch above. A note with only those modifiers
                // has nothing to credit yet, so it's left out of `stats` here
                // rather than falling through to the "normal" bucket.
                let immediate: Vec<Modifier> = note
                    .modifiers
                    .iter()
                    .filter(|&m| !is_sustained_technique(m))
                    .cloned()
                    .collect();
                if note.modifiers.is_empty() || !immediate.is_empty() {
                    stats.record_technique(&immediate, true);
                }
                // Claim the attack so a held breath can't also clear the next
                // same-pitch note; the player must re-articulate for that one.
                // `playing` was only true above if `expected_pitch` is `Some`.
                if let Some(m) = note.expected_pitch {
                    gate.consume(m);
                    if note.chord_pitches.is_empty() {
                        // `is_clean_attack` means "nothing else sounded" —
                        // meaningless for a chord note, where other pitches
                        // sounding is the whole point; tracked separately below.
                        bump(&mut stats.clean_attack, is_clean_attack(&harp_pitches, m));
                    } else {
                        bump(&mut stats.chord, true);
                    }
                }
                match quality {
                    HitQuality::Perfect => stats.perfect += 1,
                    // A late Good hit counts as "delayed"; early/on-time as "good".
                    HitQuality::Good if offset > 0.0 => stats.delayed += 1,
                    HitQuality::Good => stats.good += 1,
                }
                stats.offset_sum += offset;
                score.last_hit_time = clock.get();
                score.combo += 1;
                score.max_combo = score.max_combo.max(score.combo);
                let multiplier = if config.combo_enabled {
                    compute_multiplier(
                        score.combo,
                        config.base_multiplier,
                        config.step_multiplier,
                        config.max_multiplier,
                    )
                } else {
                    1.0
                };
                score.points += compute_points(quality, multiplier);
                // Reward executing the note's onset techniques. Bends are
                // genuinely validated (the note's expected pitch is the bent
                // one); the bonus is the payoff for nailing them. Vibrato/wah
                // bonuses are awarded later, once the sustain confirms them.
                score.points += style_bonus_points(&immediate, &config.style_bonus).round() as u32;
                feedback.quality = Some(quality);
                feedback.timer = 0.75;
                scored.write(NoteScored {
                    quality: Some(quality),
                });
            }
        }
    }
}

fn modifier_fx_key(modifier: &Modifier) -> &'static str {
    match modifier {
        Modifier::Bend { .. } => "bend",
        Modifier::Vibrato { .. } => "vibrato",
        Modifier::WahWah { .. } => "wah-wah",
        Modifier::Overblow => "overblow",
        Modifier::Overdraw => "overdraw",
        Modifier::Slide => "slide",
    }
}

/// Label and tint for a judged hit quality, shared by the label-once and the
/// per-frame color-fade halves of `update_score_display`.
fn feedback_style(quality: HitQuality) -> (&'static str, f32, f32, f32) {
    match quality {
        HitQuality::Perfect => ("PERFECT!", 1.00, 0.85, 0.10),
        HitQuality::Good => ("GOOD", 0.40, 1.00, 0.35),
    }
}

/// The score/combo digits only get re-`format!`ed when [`NoteScored`] says
/// `Score` actually moved. The feedback label ("PERFECT!"/"GOOD") is set
/// once, on the frame a fresh hit's message carries a `quality` — not every
/// frame of its fade, which stays a per-frame animation (color/alpha only)
/// driven straight off `HitFeedback`, same as before.
fn update_score_display(
    mut scored: MessageReader<NoteScored>,
    score: Res<Score>,
    config: Res<ScoringConfig>,
    mut feedback: ResMut<HitFeedback>,
    time: Res<Time>,
    mut q_score: Query<&mut Text, (With<ScoreText>, Without<ComboText>, Without<FeedbackText>)>,
    mut q_combo: Query<&mut Text, (With<ComboText>, Without<ScoreText>, Without<FeedbackText>)>,
    mut q_feedback: Query<
        (&mut Text, &mut TextColor),
        (With<FeedbackText>, Without<ScoreText>, Without<ComboText>),
    >,
) {
    let mut score_moved = false;
    let mut fresh_hit = None;
    for ev in scored.read() {
        score_moved = true;
        if ev.quality.is_some() {
            fresh_hit = ev.quality;
        }
    }

    if score_moved {
        let points = format!("{}", score.points);
        for mut t in &mut q_score {
            if t.0 != points {
                t.0 = points.clone();
            }
        }

        // Same multiplier `score_notes` actually applies to points, so the HUD
        // can never show a number the score disagrees with.
        let multiplier = if config.combo_enabled {
            compute_multiplier(
                score.combo,
                config.base_multiplier,
                config.step_multiplier,
                config.max_multiplier,
            )
        } else {
            1.0
        };
        let combo = combo_label(score.combo, multiplier);
        for mut t in &mut q_combo {
            if t.0 != combo {
                t.0 = combo.clone();
            }
        }
    }

    if let Some(q) = fresh_hit {
        let (label, ..) = feedback_style(q);
        for (mut t, _) in &mut q_feedback {
            t.0 = label.to_string();
        }
    }

    feedback.timer = (feedback.timer - time.delta_secs()).max(0.0);

    for (_, mut color) in &mut q_feedback {
        match feedback.quality {
            None => {
                *color = TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
            Some(q) => {
                let alpha = (feedback.timer / 0.75).clamp(0.0, 1.0);
                // Scale up then fade: pulse from 1.4× down to 1× size isn't
                // easily done here, so we just fade alpha.
                let (_, r, g, b) = feedback_style(q);
                *color = TextColor(Color::srgba(r, g, b, alpha));
                if feedback.timer == 0.0 {
                    feedback.quality = None;
                }
            }
        }
    }
}

/// Once the song's content has finished (and we're not looping or jamming),
/// transition to the results screen. Gated on `music_started` so it never fires
/// during the countdown.
fn detect_song_end(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    music_started: Res<MusicStarted>,
    mode: Res<GameplayMode>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if *mode == GameplayMode::JamSession || !music_started.0 {
        return;
    }
    if clock.get() >= song_end.0 {
        next_state.set(AppState::Results);
    }
}

/// Push the current music level onto the playing song's sink whenever the
/// `AudioSettings` resource changes, so dragging the Options slider is heard
/// immediately. (Metronome clicks pick up their level when each click spawns.)
fn apply_music_volume(
    audio: Res<AudioSettings>,
    mut sinks: Query<&mut AudioSink, With<MusicPlayer>>,
) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(audio.music_volume));
    }
}

fn cleanup_gameplay(
    mut commands: Commands,
    roots: Query<Entity, With<GameplayRoot>>,
    mut pitch_range: ResMut<PitchRange>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    // Leaving Playing/BendingTrainer drops the chart- or key-derived range;
    // menus and the spectrogram fall back to the default until another chart
    // (or the trainer) sets it again.
    *pitch_range = PitchRange::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TechniqueStats / SongStats::record_technique ───────────────────────

    #[test]
    fn technique_stats_accuracy_is_none_when_never_exercised() {
        assert_eq!(TechniqueStats::default().accuracy(), None);
    }

    #[test]
    fn technique_stats_accuracy_divides_hits_by_total() {
        let s = TechniqueStats { hits: 3, misses: 1 };
        assert_eq!(s.total(), 4);
        assert!((s.accuracy().unwrap() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn record_technique_with_no_modifiers_goes_to_normal() {
        let mut stats = SongStats::default();
        stats.record_technique(&[], true);
        stats.record_technique(&[], false);
        assert_eq!(stats.normal.hits, 1);
        assert_eq!(stats.normal.misses, 1);
        assert_eq!(stats.bend.total(), 0);
    }

    #[test]
    fn record_technique_routes_each_modifier_to_its_own_bucket() {
        let mut stats = SongStats::default();
        stats.record_technique(
            &[Modifier::Bend {
                semitones: -1.0,
                intensity: None,
            }],
            true,
        );
        stats.record_technique(&[Modifier::Overblow], false);
        stats.record_technique(
            &[Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: None,
            }],
            true,
        );
        stats.record_technique(
            &[Modifier::WahWah {
                oscillation_hz: 3.0,
                intensity: None,
            }],
            true,
        );
        stats.record_technique(&[Modifier::Overdraw], true);
        stats.record_technique(&[Modifier::Slide], true);

        assert_eq!(stats.bend.hits, 1);
        assert_eq!(stats.overblow.misses, 1);
        assert_eq!(stats.vibrato.hits, 1);
        assert_eq!(stats.wah.hits, 1);
        assert_eq!(stats.overdraw.hits, 1);
        assert_eq!(stats.slide.hits, 1);
        assert_eq!(stats.normal.total(), 0, "no plain notes were recorded");
    }

    #[test]
    fn record_technique_with_two_modifiers_credits_both() {
        // A note that's both bent and vibrato'd counts as a data point for
        // both techniques' accuracy — hitting/missing it is informative for both.
        let mut stats = SongStats::default();
        stats.record_technique(
            &[
                Modifier::Bend {
                    semitones: -1.0,
                    intensity: None,
                },
                Modifier::Vibrato {
                    oscillation_hz: 5.0,
                    intensity: None,
                },
            ],
            true,
        );
        assert_eq!(stats.bend.hits, 1);
        assert_eq!(stats.vibrato.hits, 1);
    }

    #[test]
    fn parse_beats_4_4() {
        assert_eq!(parse_beats(Some("4/4")), 4.0);
    }

    #[test]
    fn parse_beats_3_4() {
        assert_eq!(parse_beats(Some("3/4")), 3.0);
    }

    #[test]
    fn parse_beats_none_defaults_to_4() {
        assert_eq!(parse_beats(None), 4.0);
    }

    #[test]
    fn parse_beats_malformed_defaults_to_4() {
        assert_eq!(parse_beats(Some("invalid")), 4.0);
    }

    #[test]
    fn secs_per_bar_120bpm_4beats() {
        assert!((secs_per_bar(120.0, 4.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn secs_per_bar_60bpm_4beats() {
        assert!((secs_per_bar(60.0, 4.0) - 4.0).abs() < 1e-9);
    }

    // `advance_clock`'s own tests live in `clock.rs` alongside the type.

    // ── handle_loop_boundary ─────────────────────────────────────────────────

    /// No `AudioSink`/`MusicPlayer` entity is spawned in these tests — the
    /// seek-on-wrap fix (see the doc comment on `handle_loop_boundary`)
    /// degrades gracefully to a no-op when no sink exists (as it does for a
    /// real chart before the music sink spawns, or if audio init failed), so
    /// the clock/note-reset behaviour is testable headlessly without a real
    /// audio backend; the actual seek call needs a live sink and is a manual
    /// check (see `docs/gameplay_validation.md`).
    fn loop_test_note(time: f64) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.5,
            hole: 1,
            is_blow: true,
            expected_pitch: Some(60), // C4
            hit: true,
            missed: true,
            held: 1.0,
            sustain_scored: true,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: 0,
            chord_pitches: Vec::new(),
        }
    }

    #[test]
    fn loop_boundary_rewinds_the_clock_and_resets_notes_in_range() {
        let mut world = World::new();
        world.insert_resource(LoopConfig {
            active: true,
            start_time: 2.0,
            end_time: 10.0,
        });
        world.insert_resource(GameplayClock::new(10.0));
        // Sorted by time, as `SongNotes` requires: before-range, in-range,
        // just-past-range-but-within-LOOKAHEAD (still gets a reset — see
        // `loop_reset_range`), and genuinely far beyond it.
        world.insert_resource(SongNotes {
            notes: vec![
                loop_test_note(1.0),
                loop_test_note(5.0),
                loop_test_note(11.0),
                loop_test_note(20.0),
            ],
            cursor: 4, // as if all four had already resolved and rolled off.
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(handle_loop_boundary);
        schedule.run(&mut world);

        assert_eq!(world.resource::<GameplayClock>().get(), 2.0);

        let song_notes = world.resource::<SongNotes>();
        for i in [1, 2] {
            let reset = &song_notes.notes[i];
            assert!(
                !reset.hit && !reset.missed && !reset.sustain_scored && reset.held == 0.0,
                "note {i} (in range or a LOOKAHEAD preview past it) should be reset"
            );
        }
        assert_eq!(song_notes.cursor, 1, "cursor rewinds to the in-range note");

        let untouched = &song_notes.notes[0];
        assert!(
            untouched.hit && untouched.missed && untouched.sustain_scored,
            "a note before start_time must not be reset"
        );
        let untouched = &song_notes.notes[3];
        assert!(
            untouched.hit && untouched.missed && untouched.sustain_scored,
            "a note well beyond end_time + LOOKAHEAD must not be reset"
        );
    }

    #[test]
    fn loop_boundary_is_a_no_op_before_end_time_or_when_inactive() {
        let mut world = World::new();
        world.insert_resource(LoopConfig {
            active: true,
            start_time: 2.0,
            end_time: 10.0,
        });
        world.insert_resource(GameplayClock::new(9.999));
        world.insert_resource(SongNotes::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(handle_loop_boundary);
        schedule.run(&mut world);
        assert_eq!(world.resource::<GameplayClock>().get(), 9.999);

        let mut world = World::new();
        world.insert_resource(LoopConfig {
            active: false,
            start_time: 2.0,
            end_time: 10.0,
        });
        world.insert_resource(SongNotes::default());
        world.insert_resource(GameplayClock::new(10.0));
        let mut schedule = Schedule::default();
        schedule.add_systems(handle_loop_boundary);
        schedule.run(&mut world);
        assert_eq!(world.resource::<GameplayClock>().get(), 10.0);
    }

    #[test]
    fn current_bar_index_at_zero() {
        assert_eq!(current_bar_index(0.0, 2.0), 0);
    }

    #[test]
    fn current_bar_index_advances() {
        assert_eq!(current_bar_index(2.0, 2.0), 1);
        assert_eq!(current_bar_index(4.0, 2.0), 2);
    }

    #[test]
    fn current_bar_index_wraps_at_12() {
        // 12 bars × 2 s/bar = 24 s → wraps back to bar 0
        assert_eq!(current_bar_index(24.0, 2.0), 0);
    }

    #[test]
    fn current_bar_index_clamps_negative_clock() {
        // During countdown the clock is negative — should give bar 0
        assert_eq!(current_bar_index(-1.5, 2.0), 0);
    }

    // ── should_anchor_to_sink (tick_clock's audio-anchoring gate) ────────────

    #[test]
    fn anchors_once_playing_with_a_nonempty_sink() {
        assert!(should_anchor_to_sink(
            1.0,
            true,
            &GameplayMode::Play2D,
            false
        ));
    }

    #[test]
    fn does_not_anchor_during_the_countdown() {
        assert!(!should_anchor_to_sink(
            -1.0,
            true,
            &GameplayMode::Play2D,
            false
        ));
    }

    #[test]
    fn does_not_anchor_before_music_started() {
        assert!(!should_anchor_to_sink(
            1.0,
            false,
            &GameplayMode::Play2D,
            false
        ));
    }

    #[test]
    fn does_not_anchor_in_jam_session() {
        assert!(!should_anchor_to_sink(
            1.0,
            true,
            &GameplayMode::JamSession,
            false
        ));
    }

    #[test]
    fn does_not_anchor_once_the_sink_is_empty() {
        // A finished sink's reported position freezes rather than continuing
        // to advance — anchoring to it would repeatedly snap the clock back
        // once real time drifts past it.
        assert!(!should_anchor_to_sink(
            1.0,
            true,
            &GameplayMode::Play2D,
            true
        ));
    }

    // ── loop_range_valid (progress-bar drag loop range) ──────────────────────

    #[test]
    fn loop_range_valid_requires_end_strictly_after_start() {
        assert!(loop_range_valid(4.0, 8.0));
        assert!(!loop_range_valid(8.0, 8.0));
        assert!(!loop_range_valid(8.0, 4.0));
    }

    // ── resolve_item_time ───────────────────────────────────────────────────────

    use crate::song::chart::{TempoPoint, Timing, TrackItem};

    fn track_item(time: Option<f64>, tick: Option<u64>) -> TrackItem {
        TrackItem {
            id: None,
            time,
            tick,
            duration: 0.5,
            phrase: None,
            groove: None,
            play_mode: None,
            events: vec![],
        }
    }

    fn timing_120bpm() -> Timing {
        Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint {
                tick: 0,
                bpm: 120.0,
            }],
            time_signature_map: None,
        }
    }

    #[test]
    fn resolve_item_time_prefers_explicit_time() {
        let item = track_item(Some(2.5), Some(9999));
        assert!((resolve_item_time(&item, &timing_120bpm()) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn resolve_item_time_falls_back_to_tick() {
        // One quarter note (480 ticks) at 120 BPM = 0.5 s
        let item = track_item(None, Some(480));
        assert!((resolve_item_time(&item, &timing_120bpm()) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn resolve_item_time_defaults_missing_tick_to_zero() {
        let item = track_item(None, None);
        assert_eq!(resolve_item_time(&item, &timing_120bpm()), 0.0);
    }

    // ── last_note_end ─────────────────────────────────────────────────────────────

    #[test]
    fn last_note_end_is_latest_finish() {
        // Items at 0.0 and 2.0, each 0.5 s long → latest finish is 2.5 s.
        let track = vec![track_item(Some(0.0), None), track_item(Some(2.0), None)];
        assert!((last_note_end(&track, &timing_120bpm()) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn last_note_end_ignores_order() {
        // The latest end wins even when the longest note isn't last in the track.
        let track = vec![track_item(Some(5.0), None), track_item(Some(1.0), None)];
        assert!((last_note_end(&track, &timing_120bpm()) - 5.5).abs() < 1e-9);
    }

    #[test]
    fn last_note_end_empty_track_is_zero() {
        assert_eq!(last_note_end(&[], &timing_120bpm()), 0.0);
    }

    // ── modifier_fx_key ───────────────────────────────────────────────────────────

    #[test]
    fn modifier_fx_keys_match_technique_names() {
        use crate::song::chart::Modifier::*;
        assert_eq!(
            modifier_fx_key(&Bend {
                semitones: -1.0,
                intensity: None
            }),
            "bend"
        );
        assert_eq!(
            modifier_fx_key(&Vibrato {
                oscillation_hz: 5.0,
                intensity: None
            }),
            "vibrato"
        );
        assert_eq!(
            modifier_fx_key(&WahWah {
                oscillation_hz: 3.0,
                intensity: None
            }),
            "wah-wah"
        );
        assert_eq!(modifier_fx_key(&Overblow), "overblow");
        assert_eq!(modifier_fx_key(&Overdraw), "overdraw");
        assert_eq!(modifier_fx_key(&Slide), "slide");
    }

    // `PitchGate` is now a thin `Resource` wrapper around the shared
    // `AttackGate` (see `crate::scoring`) — its re-attack-detection behaviour
    // is covered by `AttackGate`'s own tests there, not duplicated here.

    // ── target_pitch (bend validation) ───────────────────────────────────────────

    #[test]
    fn bend_targets_the_bent_pitch() {
        let bend = vec![Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        }];
        // A 1-semitone draw bend on B4 (71) must be played as A#4 (70), not
        // the natural B4.
        assert_eq!(target_pitch("B4", &bend), Some(70));
    }

    #[test]
    fn deeper_bend_targets_lower_pitch() {
        let bend = vec![Modifier::Bend {
            semitones: -2.0,
            intensity: None,
        }];
        assert_eq!(target_pitch("B4", &bend), Some(69)); // A4
    }

    #[test]
    fn non_bend_techniques_keep_the_natural_pitch() {
        let vib = vec![Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        }];
        assert_eq!(target_pitch("D5", &vib), Some(74));
        assert_eq!(target_pitch("D5", &[]), Some(74));
    }

    #[test]
    fn unknown_pitch_name_has_no_target() {
        // The "—" placeholder for a hole/direction the harp can't produce
        // isn't a parseable note name, so there's no valid target at all —
        // this note can never be hit.
        let bend = vec![Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        }];
        assert_eq!(target_pitch("\u{2014}", &bend), None);
    }

    // ── style_bonus_points ───────────────────────────────────────────────────────

    fn bonus_table() -> HashMap<String, f32> {
        [("bend".to_string(), 50.0), ("vibrato".to_string(), 25.0)]
            .into_iter()
            .collect()
    }

    #[test]
    fn style_bonus_sums_matched_techniques() {
        let mods = vec![
            Modifier::Bend {
                semitones: -1.0,
                intensity: None,
            },
            Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: None,
            },
        ];
        assert_eq!(style_bonus_points(&mods, &bonus_table()), 75.0);
    }

    #[test]
    fn style_bonus_ignores_techniques_absent_from_the_table() {
        let mods = vec![Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        }];
        assert_eq!(style_bonus_points(&mods, &bonus_table()), 0.0);
    }

    #[test]
    fn style_bonus_is_zero_without_modifiers() {
        assert_eq!(style_bonus_points(&[], &bonus_table()), 0.0);
    }

    // ── sustained-technique validation (vibrato / wah) ──────────────────────────

    #[test]
    fn vibrato_and_wah_are_sustained_bend_and_overblow_are_not() {
        let vibrato = Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        };
        let wah = Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        };
        let bend = Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        };
        assert!(is_sustained_technique(&vibrato));
        assert!(is_sustained_technique(&wah));
        assert!(!is_sustained_technique(&bend));
        assert!(!is_sustained_technique(&Modifier::Slide));
        assert!(!is_sustained_technique(&Modifier::Overblow));
        assert!(!is_sustained_technique(&Modifier::Overdraw));
    }

    // Timestamped sine samples around `offset`, `n` samples spaced `dt` seconds apart.
    fn timestamped_sine(
        freq_hz: f32,
        offset: f32,
        amplitude: f32,
        n: usize,
        dt: f64,
    ) -> Vec<(f64, f32)> {
        (0..n)
            .map(|i| {
                let t = i as f64 * dt;
                let v =
                    offset + amplitude * (2.0 * std::f32::consts::PI * freq_hz * t as f32).sin();
                (t, v)
            })
            .collect()
    }

    #[test]
    fn technique_confirmed_requires_real_wobble_for_vibrato() {
        let vibrato = Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        };
        let steady: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.0)).collect();
        let wobbling = timestamped_sine(5.0, 0.0, 25.0, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&vibrato, &steady, &[]));
        assert!(technique_confirmed(&vibrato, &wobbling, &[]));
    }

    #[test]
    fn technique_confirmed_requires_real_wobble_for_wah() {
        let wah = Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        };
        let steady_volume: Vec<(f64, f32)> = (0..20).map(|i| (i as f64 / 60.0, 0.2)).collect();
        let pumping_volume = timestamped_sine(3.0, 0.2, 0.06, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&wah, &[], &steady_volume));
        assert!(technique_confirmed(&wah, &[], &pumping_volume));
    }

    #[test]
    fn technique_confirmed_rejects_vibrato_at_the_wrong_rate() {
        // The chart declares a 5 Hz vibrato, but the player wobbled at ~1.5 Hz
        // — real oscillation, just not the declared rate. A flip-count-only
        // check couldn't tell these apart.
        let vibrato = Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        };
        let slow_wobble = timestamped_sine(1.5, 0.0, 25.0, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&vibrato, &slow_wobble, &[]));
    }

    #[test]
    fn technique_confirmed_rejects_wah_at_the_wrong_rate() {
        let wah = Modifier::WahWah {
            oscillation_hz: 3.0,
            intensity: None,
        };
        let fast_pumping = timestamped_sine(9.0, 0.2, 0.06, 40, 1.0 / 60.0);
        assert!(!technique_confirmed(&wah, &[], &fast_pumping));
    }

    #[test]
    fn technique_confirmed_is_always_true_for_onset_validated_modifiers() {
        // Bend/overblow/overdraw/slide are judged at onset, not from the
        // sustain buffers — this should never gate them on empty/steady samples.
        assert!(technique_confirmed(
            &Modifier::Bend {
                semitones: -1.0,
                intensity: None
            },
            &[],
            &[]
        ));
        assert!(technique_confirmed(&Modifier::Overblow, &[], &[]));
        assert!(technique_confirmed(&Modifier::Slide, &[], &[]));
    }

    fn pitch_info(midi: u8, note: &str, octave: i32, frequency: f32) -> PitchInfo {
        PitchInfo {
            midi,
            note: note.into(),
            octave,
            frequency,
        }
    }

    #[test]
    fn active_frequency_for_matches_by_midi_number() {
        let active = vec![
            pitch_info(62, "D", 4, 293.66),
            pitch_info(67, "G", 4, 392.00),
        ];
        assert_eq!(active_frequency_for(&active, 62), Some(293.66));
        assert_eq!(active_frequency_for(&active, 69), None);
    }

    // ── cleanup_gameplay ──────────────────────────────────────────────────────────

    #[test]
    fn cleanup_despawns_only_gameplay_entities() {
        // Leaving Playing must tear down the scene (every `GameplayRoot`) while
        // leaving unrelated entities (e.g. the persistent camera) untouched.
        let mut world = World::new();
        world.init_resource::<PitchRange>();
        let scene_a = world.spawn(GameplayRoot).id();
        let scene_b = world.spawn((GameplayRoot, Transform::default())).id();
        let keep = world.spawn_empty().id();

        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_gameplay);
        schedule.run(&mut world);

        assert!(
            !world.entities().contains(scene_a),
            "GameplayRoot should be despawned"
        );
        assert!(
            !world.entities().contains(scene_b),
            "GameplayRoot should be despawned"
        );
        assert!(
            world.entities().contains(keep),
            "unrelated entities must survive"
        );
    }

    // ── score_notes (same-pitch overlap ordering) ───────────────────────────

    fn overlap_test_note(time: f64) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 1.0,
            hole: 1,
            is_blow: true,
            expected_pitch: Some(60), // C4
            hit: false,
            missed: false,
            held: 0.0,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: 0,
            chord_pitches: Vec::new(),
        }
    }

    #[test]
    fn score_notes_credits_the_closest_offset_when_two_same_pitch_notes_overlap() {
        // Two C4 notes both sit inside the hit window at clock=0.5 while C4 is
        // sounding: one 0.01s away (should score), one 0.10s away (should
        // stay `Waiting` — the pitch is fresh only once). Array order alone
        // would coincidentally put the closer note second too, so this
        // checks that classification actually goes by |offset|, not array
        // position.
        let mut world = World::new();
        world.insert_resource(GameplayClock::new(0.5));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ActivePitches(vec![PitchInfo {
            midi: 60,
            note: "C".to_string(),
            octave: 4,
            frequency: midi_to_freq_hz(60.0),
        }]));
        world.insert_resource(AudioFrame::default());
        world.insert_resource(ValidHarpNotes(HashSet::from([60u8])));
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(AudioSettings::default());
        world.insert_resource(Score::default());
        world.insert_resource(SongStats::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(PitchGate::default());
        world.init_resource::<Messages<NoteScored>>();
        world.insert_resource(SongNotes {
            // Sorted by time: index 0 is farther from `judged` (offset
            // -0.10), index 1 is closer (offset -0.01).
            notes: vec![overlap_test_note(0.40), overlap_test_note(0.49)],
            cursor: 0,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        let song_notes = world.resource::<SongNotes>();
        assert!(
            song_notes.notes[1].hit,
            "the note actually due should be credited"
        );
        assert!(
            !song_notes.notes[0].hit,
            "the farther note must not steal the attack meant for the closer one"
        );
    }

    #[test]
    fn score_notes_leaves_a_far_future_note_untouched() {
        // A note well beyond `good_window` classifies as `TooEarly` — a no-op
        // — so it's skipped before the sort/classify pass entirely (the
        // optimization for long charts). Confirm that skip doesn't change its
        // observable state: still neither hit nor missed.
        let mut world = World::new();
        world.insert_resource(GameplayClock::new(0.0));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ActivePitches(vec![]));
        world.insert_resource(AudioFrame::default());
        world.insert_resource(ValidHarpNotes(HashSet::from([60u8])));
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(AudioSettings::default());
        world.insert_resource(Score::default());
        world.insert_resource(SongStats::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(PitchGate::default());
        world.init_resource::<Messages<NoteScored>>();
        world.insert_resource(SongNotes {
            notes: vec![overlap_test_note(120.0)],
            cursor: 0,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        let song_notes = world.resource::<SongNotes>();
        assert!(!song_notes.notes[0].hit);
        assert!(!song_notes.notes[0].missed);
    }

    // ── score_notes (clean-attack tallying) ──────────────────────────────────

    /// Builds a world set up to score one `overlap_test_note` at clock=0.5
    /// against whatever `ActivePitches` the caller supplies, for the
    /// clean-attack tests below.
    fn clean_attack_test_world(active: Vec<PitchInfo>) -> World {
        let mut world = World::new();
        world.insert_resource(GameplayClock::new(0.5));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ActivePitches(active));
        world.insert_resource(AudioFrame::default());
        world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 64u8])));
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(AudioSettings::default());
        world.insert_resource(Score::default());
        world.insert_resource(SongStats::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(PitchGate::default());
        world.init_resource::<Messages<NoteScored>>();
        world.insert_resource(SongNotes {
            notes: vec![overlap_test_note(0.49)],
            cursor: 0,
        });
        world
    }

    #[test]
    fn score_notes_counts_a_solo_pitch_as_a_clean_attack() {
        let mut world =
            clean_attack_test_world(vec![pitch_info(60, "C", 4, midi_to_freq_hz(60.0))]);
        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        assert!(world.resource::<SongNotes>().notes[0].hit, "should still hit");
        let stats = world.resource::<SongStats>();
        assert_eq!(stats.clean_attack.hits, 1);
        assert_eq!(stats.clean_attack.misses, 0);
    }

    #[test]
    fn score_notes_counts_a_breathy_leak_as_a_hit_but_not_a_clean_attack() {
        // A second, unintended harp-producible pitch (64 = E4) sounds
        // alongside the expected one (60 = C4): the note still scores — the
        // expected pitch is present and on time — but it must not count
        // toward `clean_attack`.
        let mut world = clean_attack_test_world(vec![
            pitch_info(60, "C", 4, midi_to_freq_hz(60.0)),
            pitch_info(64, "E", 4, midi_to_freq_hz(64.0)),
        ]);
        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        assert!(
            world.resource::<SongNotes>().notes[0].hit,
            "the expected pitch was present and on time, so it should still hit"
        );
        let stats = world.resource::<SongStats>();
        assert_eq!(stats.clean_attack.hits, 0);
        assert_eq!(stats.clean_attack.misses, 1);
    }

    // ── score_notes (chord-target simultaneity) ──────────────────────────────

    /// Two `ScheduledNote`s from one chord `TrackItem` (same `time`, sharing
    /// `chord_pitches: [60, 64]`), one per sibling pitch — the shape
    /// `gameplay_2d::build_combined_notes`/`gameplay_3d::build_notes_3d`
    /// actually produce for a multi-event item.
    fn chord_test_notes() -> Vec<ScheduledNote> {
        let base = overlap_test_note(0.49);
        vec![
            ScheduledNote {
                hole: 1,
                expected_pitch: Some(60),
                chord_pitches: vec![60, 64],
                ..base.clone()
            },
            ScheduledNote {
                hole: 2,
                expected_pitch: Some(64),
                chord_pitches: vec![60, 64],
                ..base
            },
        ]
    }

    fn chord_test_world(active: Vec<PitchInfo>) -> World {
        let mut world = World::new();
        world.insert_resource(GameplayClock::new(0.5));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ActivePitches(active));
        world.insert_resource(AudioFrame::default());
        world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 64u8])));
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(AudioSettings::default());
        world.insert_resource(Score::default());
        world.insert_resource(SongStats::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(PitchGate::default());
        world.init_resource::<Messages<NoteScored>>();
        world.insert_resource(SongNotes {
            notes: chord_test_notes(),
            cursor: 0,
        });
        world
    }

    #[test]
    fn score_notes_hits_both_chord_notes_when_both_pitches_sound_together() {
        let mut world = chord_test_world(vec![
            pitch_info(60, "C", 4, midi_to_freq_hz(60.0)),
            pitch_info(64, "E", 4, midi_to_freq_hz(64.0)),
        ]);
        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        let notes = &world.resource::<SongNotes>().notes;
        assert!(notes[0].hit, "60 should hit — both pitches sounded together");
        assert!(notes[1].hit, "64 should hit — both pitches sounded together");
        let stats = world.resource::<SongStats>();
        assert_eq!(stats.chord.hits, 2);
        assert_eq!(stats.clean_attack.total(), 0, "chord notes aren't clean-attack notes");
    }

    #[test]
    fn score_notes_does_not_hit_a_chord_note_from_only_one_of_its_pitches() {
        // Only 60 sounds — 64 never joins it. Neither half of the chord
        // should score just because its own pitch happens to be present.
        let mut world = chord_test_world(vec![pitch_info(60, "C", 4, midi_to_freq_hz(60.0))]);
        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        let notes = &world.resource::<SongNotes>().notes;
        assert!(!notes[0].hit, "60 alone must not satisfy the chord");
        assert!(!notes[1].hit);
    }

    #[test]
    fn score_notes_misses_a_chord_note_that_never_sounded_together_with_its_partner() {
        let mut world = chord_test_world(vec![]);
        world.resource_mut::<GameplayClock>().set_free(10.0); // well past miss_window
        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);
        schedule.run(&mut world);

        let stats = world.resource::<SongStats>();
        assert_eq!(stats.chord.misses, 2);
    }

    // ── update_score_display (message-gated HUD writes) ─────────────────────

    #[test]
    fn update_score_display_only_writes_text_when_score_moved() {
        let mut world = World::new();
        world.insert_resource(Score {
            points: 100,
            combo: 3,
            max_combo: 3,
            last_hit_time: 0.0,
        });
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(Time::<()>::default());
        world.init_resource::<Messages<NoteScored>>();

        let score_entity = world.spawn((Text::new(""), ScoreText)).id();
        let combo_entity = world.spawn((Text::new(""), ComboText)).id();
        let feedback_entity = world
            .spawn((
                Text::new(""),
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                FeedbackText,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_score_display);

        // No `NoteScored` this frame: the digits (still their spawn-time
        // default) must stay untouched.
        schedule.run(&mut world);
        assert_eq!(world.get::<Text>(score_entity).unwrap().0, "");
        assert_eq!(world.get::<Text>(combo_entity).unwrap().0, "");

        // `score_notes` would have set `HitFeedback` itself before emitting a
        // message with a quality — mirror that here rather than depending on
        // `update_score_display` to do it.
        world.insert_resource(HitFeedback {
            quality: Some(HitQuality::Perfect),
            timer: 0.75,
        });
        world.write_message(NoteScored {
            quality: Some(HitQuality::Perfect),
        });
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(score_entity).unwrap().0, "100");
        let expected_multiplier = compute_multiplier(3, 1.0, 0.1, 4.0);
        assert_eq!(
            world.get::<Text>(combo_entity).unwrap().0,
            combo_label(3, expected_multiplier)
        );
        assert_eq!(world.get::<Text>(feedback_entity).unwrap().0, "PERFECT!");
        let color = world.get::<TextColor>(feedback_entity).unwrap();
        assert!(color.0.alpha() > 0.0, "the feedback flash should be visible right after a fresh hit");
    }

    // ── notes_needing_spawn (windowed rendering) ─────────────────────────────
    //
    // `gameplay_2d`/`gameplay_3d` can't be smoke-tested headlessly (they need
    // a real render/asset harness), so this pure windowing logic — the part
    // that actually decides which notes get a visual and when — is the one
    // piece of the windowed-spawn refactor that's directly testable. LOOKAHEAD
    // is 3.0s throughout.

    #[test]
    fn notes_needing_spawn_is_empty_well_before_or_after_a_note() {
        let notes = [overlap_test_note(10.0)];
        let none = HashSet::new();
        assert_eq!(notes_needing_spawn(&notes, &none, 0.0), Vec::<usize>::new());
        assert_eq!(
            notes_needing_spawn(&notes, &none, 20.0),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn notes_needing_spawn_includes_a_note_right_at_the_lookahead_edge() {
        let notes = [overlap_test_note(10.0)];
        let none = HashSet::new();
        // Window opens at note.time - LOOKAHEAD = 7.0.
        assert_eq!(notes_needing_spawn(&notes, &none, 7.0), vec![0]);
        assert_eq!(
            notes_needing_spawn(&notes, &none, 6.999),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn notes_needing_spawn_skips_indices_already_spawned() {
        let notes = [overlap_test_note(10.0), overlap_test_note(10.5)];
        let one_spawned = HashSet::from([0]);
        assert_eq!(notes_needing_spawn(&notes, &one_spawned, 8.0), vec![1]);
    }

    #[test]
    fn notes_needing_spawn_returns_every_note_whose_window_is_open() {
        let notes = [
            overlap_test_note(10.0),
            overlap_test_note(10.2),
            overlap_test_note(20.0), // window not open yet at elapsed=9.0
        ];
        let none = HashSet::new();
        assert_eq!(notes_needing_spawn(&notes, &none, 9.0), vec![0, 1]);
    }

    #[test]
    fn notes_needing_spawn_stops_scanning_once_a_note_is_too_far_out() {
        // A note far beyond the window sits after several already-open ones —
        // confirms the scan doesn't spuriously include (or choke on) it.
        let notes = [
            overlap_test_note(10.0),
            overlap_test_note(10.1),
            overlap_test_note(1000.0),
        ];
        let none = HashSet::new();
        assert_eq!(notes_needing_spawn(&notes, &none, 9.0), vec![0, 1]);
    }

    // ── loop_reset_range (A/B loop wrap note reset) ───────────────────────────

    #[test]
    fn loop_reset_range_covers_notes_from_start_through_end_time() {
        let notes = [
            overlap_test_note(4.0),  // before start_time — excluded
            overlap_test_note(5.0),  // == start_time — included
            overlap_test_note(8.0),  // inside the range — included
            overlap_test_note(10.0), // == end_time — included
        ];
        assert_eq!(loop_reset_range(&notes, 5.0, 10.0), (1, 4));
    }

    #[test]
    fn loop_reset_range_extends_past_end_time_by_lookahead() {
        let notes = [
            overlap_test_note(10.0),                    // == end_time
            overlap_test_note(10.0 + LOOKAHEAD),        // exactly at the reach
            overlap_test_note(10.0 + LOOKAHEAD + 0.01), // just past — excluded
        ];
        assert_eq!(loop_reset_range(&notes, 5.0, 10.0), (0, 2));
    }

    // ── first_due_unresolved_note (wait-for-note freeze condition) ──────────

    #[test]
    fn first_due_unresolved_note_is_none_until_the_clock_reaches_it() {
        let notes = [overlap_test_note(10.0)];
        assert_eq!(first_due_unresolved_note(&notes, 0, 9.999), None);
        assert_eq!(first_due_unresolved_note(&notes, 0, 10.0), Some(0));
    }

    #[test]
    fn first_due_unresolved_note_ignores_already_hit_or_missed_notes() {
        let mut hit = overlap_test_note(10.0);
        hit.hit = true;
        let mut missed = overlap_test_note(10.0);
        missed.missed = true;
        assert_eq!(first_due_unresolved_note(&[hit], 0, 10.0), None);
        assert_eq!(first_due_unresolved_note(&[missed], 0, 10.0), None);
    }

    #[test]
    fn first_due_unresolved_note_ignores_unplayable_notes() {
        // A note the harp can't produce (`expected_pitch: None`) can never be
        // hit — freezing on one would wait forever.
        let mut unplayable = overlap_test_note(10.0);
        unplayable.expected_pitch = None;
        assert_eq!(first_due_unresolved_note(&[unplayable], 0, 10.0), None);
    }

    #[test]
    fn first_due_unresolved_note_stops_scanning_once_a_note_is_not_due_yet() {
        // Sorted by time: an unresolved note far in the future shouldn't
        // match, and the earlier resolved note shouldn't either.
        let mut resolved = overlap_test_note(1.0);
        resolved.hit = true;
        let notes = [resolved, overlap_test_note(1000.0)];
        assert_eq!(first_due_unresolved_note(&notes, 0, 5.0), None);
    }

    #[test]
    fn first_due_unresolved_note_returns_the_matching_index_after_the_cursor() {
        let mut resolved = overlap_test_note(1.0);
        resolved.hit = true;
        let notes = [resolved, overlap_test_note(2.0)];
        assert_eq!(first_due_unresolved_note(&notes, 0, 5.0), Some(1));
    }

    /// A tiny synthetic 3-note "song" driven frame by frame through
    /// `score_notes`, exercising the full detected-pitch → classify →
    /// score/combo/stats path together rather than each piece in isolation.
    /// This is the headless stand-in for
    /// `docs/gameplay_validation.md`'s "HUD score/combo updates as you hit
    /// notes" manual check.
    #[test]
    fn end_to_end_synthetic_song_drives_score_combo_and_stats() {
        let mut world = World::new();
        world.insert_resource(GameplayClock::new(0.0));
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ActivePitches(vec![]));
        world.insert_resource(AudioFrame::default());
        world.insert_resource(ValidHarpNotes(HashSet::from([60u8, 62, 64]))); // C4, D4, E4
        world.insert_resource(ScoringConfig::default());
        world.insert_resource(AudioSettings::default());
        world.insert_resource(Score::default());
        world.insert_resource(SongStats::default());
        world.insert_resource(HitFeedback::default());
        world.insert_resource(PitchGate::default());
        world.init_resource::<Messages<NoteScored>>();

        fn note(time: f64, pitch: u8) -> ScheduledNote {
            ScheduledNote {
                time,
                duration: 0.2,
                hole: 1,
                is_blow: true,
                expected_pitch: Some(pitch),
                hit: false,
                missed: false,
                held: 0.0,
                sustain_scored: false,
                modifiers: Vec::new(),
                pitch_samples: Vec::new(),
                amp_samples: Vec::new(),
                phrase_section: 0,
                chord_pitches: Vec::new(),
            }
        }
        fn pitch(note: &str, octave: i32) -> PitchInfo {
            let midi = note_to_midi(&format!("{note}{octave}")).unwrap() as u8;
            PitchInfo {
                midi,
                note: note.to_string(),
                octave,
                frequency: midi_to_freq_hz(midi as f32),
            }
        }

        // C4 at t=0.0 is played right on time (Perfect); D4 at t=0.5 is played
        // 90ms late (inside `good_window` 130ms but past `perfect_window`
        // 60ms, so "Good"/delayed); E4 at t=1.0 is never played (Missed).
        // Already sorted by time, as `SongNotes` requires.
        world.insert_resource(SongNotes {
            notes: vec![note(0.0, 60), note(0.5, 62), note(1.0, 64)],
            cursor: 0,
        });
        let (perfect_idx, good_idx, missed_idx) = (0, 1, 2);

        let mut schedule = Schedule::default();
        schedule.add_systems(score_notes);

        // (clock time, active pitches this frame) — irregular steps are fine
        // since `score_notes` classifies purely from clock time, not frame
        // count; only the sustain-hold measurement cares about elapsed `dt`,
        // which `Time::advance_by` sets exactly per step below.
        let steps: &[(f64, &[(&str, i32)])] = &[
            (0.0, &[("C", 4)]),
            (0.05, &[("C", 4)]),
            (0.1, &[("C", 4)]),
            (0.15, &[("C", 4)]),
            (0.2, &[("C", 4)]),
            (0.21, &[]),
            (0.5, &[]),
            (0.59, &[("D", 4)]),
            (0.6, &[]),
            (1.0, &[]),
            (1.14, &[]),
            (1.3, &[]),
        ];
        let mut prev_t = 0.0f64;
        for &(t, pitches) in steps {
            world.resource_mut::<GameplayClock>().set_free(t);
            world.resource_mut::<ActivePitches>().0 =
                pitches.iter().map(|&(n, o)| pitch(n, o)).collect();
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f64(t - prev_t));
            schedule.run(&mut world);
            prev_t = t;
        }

        let song_notes = world.resource::<SongNotes>();
        assert!(
            song_notes.notes[perfect_idx].hit,
            "on-time note should be hit"
        );
        assert!(
            song_notes.notes[good_idx].hit,
            "late-but-in-window note should still be hit"
        );
        assert!(
            song_notes.notes[missed_idx].missed,
            "never-played note should be missed"
        );

        let stats = world.resource::<SongStats>();
        assert_eq!(stats.perfect, 1);
        assert_eq!(
            stats.delayed, 1,
            "the D4 hit landed after its onset, inside the good window"
        );
        assert_eq!(stats.miss, 1);

        let score = world.resource::<Score>();
        assert!(score.points > 0, "hits and sustain should award points");
        assert_eq!(
            score.max_combo, 2,
            "combo should have peaked at 2 (both hits) before the miss reset it"
        );
        assert_eq!(score.combo, 0, "the miss should have reset the live combo");
    }
}
