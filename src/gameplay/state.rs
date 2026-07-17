// SPDX-License-Identifier: MIT

//! Shared gameplay resources and components: score/combo state, per-song
//! stats, the attack-freshness gate, scoring/loop configuration, and the
//! small marker components (`HoleCell`/`HoleState`, `GameplayRoot`,
//! `MusicPlayer`) other gameplay modules key their systems on.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::audio_system::pitch_detect::{PitchEvent, PitchInfo};
use crate::scoring::{AttackGate, HitQuality};
use crate::song::chart::Modifier;

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
    /// Onset hits where [`is_clean_attack`](crate::scoring::is_clean_attack)
    /// confirmed no *other* harp-producible pitch sounded alongside the
    /// expected one — separate from the technique buckets above (which are
    /// keyed by chart modifier, not attack cleanliness) and tallied for every
    /// hit regardless of its modifiers, or lack of them. Never tallied for a
    /// chord/octave-split note (non-empty `ScheduledNote::chord_pitches`) —
    /// "only one pitch sounding" is the wrong question for a note that's
    /// supposed to have company. Chords don't get their own bucket the way
    /// clean-attack does: `judge::score_notes` already refuses to mark a
    /// chord note `Hit` unless its siblings sound together (see
    /// `chord_is_sounding`), so an out-of-sync chord already shows up as an
    /// ordinary miss in plain accuracy — unlike a breathy leak, which plain
    /// accuracy can't see at all (that blind spot is what `clean_attack`
    /// exists to cover).
    pub clean_attack: TechniqueStats,
}

impl SongStats {
    /// Tallies a note's hit/miss outcome against every technique modifier it
    /// carries (a note can have up to two, e.g. Bend + Vibrato — it counts
    /// toward both), or `normal` if it has none.
    pub(super) fn record_technique(&mut self, modifiers: &[Modifier], hit: bool) {
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

pub(super) fn bump(stats: &mut TechniqueStats, hit: bool) {
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

#[derive(Resource, Default)]
pub struct HitFeedback {
    pub quality: Option<HitQuality>,
    pub timer: f32,
}

/// Notes currently inside the good-hit window: (hole, is_blow).
/// Updated every frame so hole-display systems can show a target hint.
#[derive(Resource, Default)]
pub struct ActiveTargets(pub Vec<(u8, bool)>);

/// Emitted by [`judge::score_notes`](super::judge::score_notes) whenever
/// `Score` moves (a fresh hit, a note's sustain bonus landing, a miss
/// resetting the combo, or the combo decaying from inactivity) —
/// `hud::update_score_display` reads this instead of re-`format!`ing the
/// score/combo `Text` every frame regardless of whether either number
/// actually changed. `quality` is only `Some` for a fresh hit, which is what
/// tells `update_score_display` to set the "PERFECT!"/"GOOD" feedback label
/// *once* rather than every frame of its fade — the alpha fade itself stays a
/// per-frame animation, driven by `HitFeedback` directly, not this message.
#[derive(Message)]
pub struct NoteScored {
    pub quality: Option<HitQuality>,
}

// ── Shared components ─────────────────────────────────────────────────────────

#[derive(Component, Default, Clone)]
pub struct GameplayRoot;

#[derive(Component)]
#[require(HoleState)]
pub struct HoleCell(pub u8);

#[derive(Component, Default)]
pub struct HoleState {
    pub brightness: f32,
    pub is_blow: bool,
}

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

/// A loop range only makes sense once `end_time` is strictly after
/// `start_time` — the single rule `LoopConfig::active` is recomputed from
/// whenever a new range is requested (see `song_progress_overlay::
/// RequestLoopRange`), so a degenerate zero-width drag on the progress bar
/// cleanly ends up inactive instead of a stale or nonsensical range.
pub fn loop_range_valid(start_time: f64, end_time: f64) -> bool {
    end_time > start_time
}

pub(super) fn collect_pitches(mut reader: MessageReader<PitchEvent>, mut active: ResMut<ActivePitches>) {
    for ev in reader.read() {
        active.0 = ev.0.clone();
    }
}
