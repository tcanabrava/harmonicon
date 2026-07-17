// SPDX-License-Identifier: MIT

//! Cross-session player progress: per-song best score/accuracy and total
//! play time, persisted to `<config>/harmonicon/profile.json` — the same
//! figment/serde pattern as `settings.rs`, but for progress data rather than
//! preferences. Unlike settings (which debounce a save on every UI change),
//! profile writes are event-driven — one write per results-screen visit —
//! so there's no debounce/dirty-flag machinery here, just a direct save
//! where the record changes, plus a flush on exit for the play-time
//! accumulator, which changes every frame while playing but is never
//! otherwise saved mid-song.

use bevy::prelude::*;
use figment::{
    Figment,
    providers::{Format, Json, Serialized},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::app::AppState;

/// Best result recorded for one song, keyed by its manifest path (stable
/// across restarts, unlike a `Handle`/`AssetId`) in [`PlayerProfile::songs`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SongRecord {
    pub best_score: u32,
    pub best_accuracy: f32,
    pub plays: u32,
    /// Best accuracy ever recorded per technique (`"bend"`, `"overblow"`,
    /// ...), each in `[0.0, 1.0]` — an improving high-water mark across
    /// sessions, not a full log of every play.
    pub technique_best_accuracy: HashMap<String, f32>,
    /// Adaptive difficulty's "learned" fraction (0.0..=1.0) per musical
    /// phrase section, indexed by the section's ordinal position in the
    /// chart track — see `gameplay::adaptive_difficulty`. Empty until the
    /// song's first play or manual adjustment; a missing/short index reads
    /// as unlearned (0.0). Whether adaptive difficulty is on at all is a
    /// single global setting (`settings::AdaptiveDifficultyEnabled`, an
    /// Options-menu toggle), not per-song — only the learned progress
    /// itself lives here.
    pub phrase_learned: Vec<f32>,
}

impl Default for SongRecord {
    fn default() -> Self {
        Self {
            best_score: 0,
            best_accuracy: 0.0,
            plays: 0,
            technique_best_accuracy: HashMap::new(),
            phrase_learned: Vec::new(),
        }
    }
}

/// Per-(hole, technique) drill hit-rate from the Bending Trainer's adaptive
/// drill, keyed by a `"{hole}:{technique}"` string (e.g. `"2:bend1"`) in
/// [`PlayerProfile::drills`] — plain strings rather than importing
/// `gameplay::bending_trainer::Technique` here, so this module stays a
/// dependency *of* gameplay features rather than *on* them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default)]
pub struct DrillRecord {
    pub attempts: u32,
    pub hits: u32,
}

/// Cross-session result for one lesson, keyed by the lesson manifest's
/// stable `id` in [`PlayerProfile::lessons`] — a plain string rather than a
/// type from `crate::lessons`, same dependency reasoning as [`DrillRecord`].
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default)]
pub struct LessonRecord {
    /// Whether the lesson's pass criteria have ever been met. Once true it
    /// stays true — a later worse run must not re-lock dependent lessons.
    pub passed: bool,
    /// High-water mark of overall accuracy across attempts (0 for
    /// instructional-only lessons marked done from the reader).
    pub best_accuracy: f32,
    pub attempts: u32,
}

/// Cross-session player progress. Loaded once at startup and updated as the
/// player finishes songs; see the module doc comment for the save policy.
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct PlayerProfile {
    pub songs: HashMap<String, SongRecord>,
    pub total_play_secs: f64,
    pub drills: HashMap<String, DrillRecord>,
    pub lessons: HashMap<String, LessonRecord>,
}

impl PlayerProfile {
    /// Ids of every lesson whose pass criteria have been met — the shape
    /// `lessons::is_unlocked` takes for prerequisite gating.
    pub fn passed_lesson_ids(&self) -> Vec<&str> {
        self.lessons
            .iter()
            .filter(|(_, r)| r.passed)
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

/// Updates `record` with a just-finished play's result, keeping whichever
/// score/accuracy is higher rather than overwriting — repeated plays should
/// only ever improve a song's recorded best, never regress it because of one
/// worse run. Returns `true` if `score` beat the previous best (so the
/// results screen can show a "New Best!" callout).
pub fn record_play(
    record: &mut SongRecord,
    score: u32,
    accuracy: f32,
    technique_accuracy: &[(&str, f32)],
) -> bool {
    record.plays += 1;
    let improved = score > record.best_score;
    record.best_score = record.best_score.max(score);
    record.best_accuracy = record.best_accuracy.max(accuracy);
    for &(name, acc) in technique_accuracy {
        let best = record.technique_best_accuracy.entry(name.into()).or_default();
        if acc > *best {
            *best = acc;
        }
    }
    improved
}

/// Updates `record` with a just-finished lesson attempt. Like
/// [`record_play`], marks only ever improve: a failed retry can't un-pass a
/// lesson or lower its best accuracy.
pub fn record_lesson(record: &mut LessonRecord, passed: bool, accuracy: f32) {
    record.attempts += 1;
    record.passed |= passed;
    record.best_accuracy = record.best_accuracy.max(accuracy);
}

fn profile_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("harmonicon").join("profile.json"))
}

fn load_profile() -> PlayerProfile {
    let mut figment = Figment::from(Serialized::defaults(PlayerProfile::default()));
    if let Some(path) = profile_path() {
        figment = figment.merge(Json::file(path));
    }
    figment.extract().unwrap_or_else(|err| {
        warn!("Could not read profile ({err}); using defaults");
        PlayerProfile::default()
    })
}

pub fn save_profile(profile: &PlayerProfile) {
    let Some(path) = profile_path() else {
        warn!("No config directory available; profile not saved");
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        warn!("Could not create config dir {}: {err}", parent.display());
        return;
    }
    match serde_json::to_string_pretty(profile) {
        Ok(json) => {
            if let Err(err) = std::fs::write(&path, json) {
                warn!("Could not write profile to {}: {err}", path.display());
            }
        }
        Err(err) => warn!("Could not serialize profile: {err}"),
    }
}

fn apply_loaded_profile(mut profile: ResMut<PlayerProfile>) {
    *profile = load_profile();
}

/// Accumulates wall-clock time spent actually playing — separate from
/// `GameplayClock`, which tracks position *within* a song's own timeline and
/// resets on retry/loop, not cumulative session time.
fn accumulate_play_time(time: Res<Time>, mut profile: ResMut<PlayerProfile>) {
    profile.total_play_secs += time.delta_secs_f64();
}

/// Flushes the profile on exit so `total_play_secs` (which otherwise only
/// changes in memory — see the module doc comment) isn't lost if the player
/// quits mid-song, before any results-screen save.
fn flush_profile_on_exit(mut exit: MessageReader<AppExit>, profile: Res<PlayerProfile>) {
    if exit.read().next().is_some() {
        save_profile(&profile);
    }
}

pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerProfile>()
            .add_systems(Startup, apply_loaded_profile)
            .add_systems(
                Update,
                accumulate_play_time.run_if(in_state(AppState::Playing)),
            )
            .add_systems(Last, flush_profile_on_exit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> SongRecord {
        SongRecord::default()
    }

    #[test]
    fn default_song_record_has_nothing_learned() {
        let r = record();
        assert!(r.phrase_learned.is_empty());
    }

    #[test]
    fn missing_phrase_learned_defaults_from_json_via_serde_default() {
        let r: SongRecord = serde_json::from_str("{}").unwrap();
        assert!(r.phrase_learned.is_empty());
    }

    #[test]
    fn first_play_sets_the_baseline() {
        let mut r = record();
        let improved = record_play(&mut r, 500, 0.8, &[("bend", 0.6)]);
        assert!(improved);
        assert_eq!(r.best_score, 500);
        assert_eq!(r.plays, 1);
        assert!((r.best_accuracy - 0.8).abs() < f32::EPSILON);
        assert_eq!(r.technique_best_accuracy.get("bend"), Some(&0.6));
    }

    #[test]
    fn a_worse_run_does_not_regress_the_best() {
        let mut r = record();
        record_play(&mut r, 800, 0.9, &[("bend", 0.9)]);
        let improved = record_play(&mut r, 300, 0.5, &[("bend", 0.4)]);
        assert!(!improved, "a lower score shouldn't report as a new best");
        assert_eq!(r.best_score, 800, "best score must not regress");
        assert!(
            (r.best_accuracy - 0.9).abs() < f32::EPSILON,
            "best accuracy must not regress"
        );
        assert_eq!(
            r.technique_best_accuracy.get("bend"),
            Some(&0.9),
            "per-technique best must not regress"
        );
    }

    #[test]
    fn a_better_run_raises_the_best_and_reports_improvement() {
        let mut r = record();
        record_play(&mut r, 500, 0.7, &[]);
        let improved = record_play(&mut r, 900, 0.6, &[]);
        assert!(improved, "a higher score should report as a new best");
        assert_eq!(r.best_score, 900);
        // Accuracy tracks its own high-water mark independently of score.
        assert!((r.best_accuracy - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn play_count_increments_every_call_regardless_of_improvement() {
        let mut r = record();
        record_play(&mut r, 100, 0.1, &[]);
        record_play(&mut r, 50, 0.05, &[]);
        record_play(&mut r, 900, 0.9, &[]);
        assert_eq!(r.plays, 3);
    }

    // ── record_lesson ─────────────────────────────────────────────────────────

    #[test]
    fn a_passed_lesson_stays_passed_after_a_failed_retry() {
        let mut r = LessonRecord::default();
        record_lesson(&mut r, true, 0.8);
        record_lesson(&mut r, false, 0.2);
        assert!(r.passed, "a worse retry must not un-pass a lesson");
        assert!((r.best_accuracy - 0.8).abs() < f32::EPSILON);
        assert_eq!(r.attempts, 2);
    }

    #[test]
    fn a_failed_lesson_records_the_attempt_without_passing() {
        let mut r = LessonRecord::default();
        record_lesson(&mut r, false, 0.3);
        assert!(!r.passed);
        assert_eq!(r.attempts, 1);
        assert!((r.best_accuracy - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn passed_lesson_ids_lists_only_passed_lessons() {
        let mut p = PlayerProfile::default();
        p.lessons.insert(
            "a".into(),
            LessonRecord {
                passed: true,
                ..Default::default()
            },
        );
        p.lessons.insert("b".into(), LessonRecord::default());
        let mut ids = p.passed_lesson_ids();
        ids.sort_unstable();
        assert_eq!(ids, ["a"]);
    }

    #[test]
    fn missing_lessons_field_defaults_to_empty_via_serde_default() {
        // Older profile.json files predate the lessons map.
        let p: PlayerProfile = serde_json::from_str("{}").unwrap();
        assert!(p.lessons.is_empty());
    }

    #[test]
    fn technique_bests_are_tracked_independently() {
        let mut r = record();
        record_play(&mut r, 100, 0.5, &[("bend", 0.5), ("overblow", 0.2)]);
        record_play(&mut r, 50, 0.3, &[("bend", 0.3), ("overblow", 0.9)]);
        assert_eq!(r.technique_best_accuracy.get("bend"), Some(&0.5));
        assert_eq!(r.technique_best_accuracy.get("overblow"), Some(&0.9));
    }
}
