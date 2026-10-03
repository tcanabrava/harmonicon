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
use serde::{Deserialize, Deserializer, Serialize};
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
    /// phrase section, keyed by the section's own stable key — its phrase
    /// name, disambiguated for repeats — rather than its ordinal position
    /// in the track (see `gameplay::adaptive_difficulty::section_keys`).
    /// Keying by position let a chart re-edit that reorders/inserts/removes
    /// a phrase tag silently apply old progress to the wrong section;
    /// keying by name survives that, at the cost of resetting progress if a
    /// section is later *renamed*. Empty until the song's first play or
    /// manual adjustment; a missing key reads as unlearned (0.0). Whether
    /// adaptive difficulty is on at all is a single global setting
    /// (`settings::AdaptiveDifficultyEnabled`, an Options-menu toggle), not
    /// per-song — only the learned progress itself lives here.
    #[serde(deserialize_with = "deserialize_phrase_learned")]
    pub phrase_learned: HashMap<String, f32>,
}

impl Default for SongRecord {
    fn default() -> Self {
        Self {
            best_score: 0,
            best_accuracy: 0.0,
            plays: 0,
            technique_best_accuracy: HashMap::new(),
            phrase_learned: HashMap::new(),
        }
    }
}

/// Reads `phrase_learned` as the current name-keyed map, or — from an older
/// `profile.json` — the ordinal-indexed `Vec<f32>` it used to be, which is
/// discarded rather than guessed at (no way to recover section names from
/// bare array positions). Falling back instead of erroring on a type
/// mismatch keeps loading an old profile from wiping out the rest of it
/// (every other song's best score, lesson progress, ...) over this one field.
fn deserialize_phrase_learned<'de, D>(deserializer: D) -> Result<HashMap<String, f32>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Current(HashMap<String, f32>),
        Legacy(#[allow(dead_code)] Vec<f32>),
    }
    Ok(match Repr::deserialize(deserializer)? {
        Repr::Current(map) => map,
        Repr::Legacy(_) => HashMap::new(),
    })
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
    /// Timeouts before a credible selected-hole onset. Kept separate so
    /// putting the instrument down never lowers the player's accuracy.
    pub skips: u32,
    /// Exponentially weighted recent control (0..=1); absent in old profiles.
    pub recent_control: f32,
    pub recent_samples: u32,
    /// Exponentially weighted line-fit residual (cents) of the held pitch on
    /// recent attempts. `0.0` means never measured, which reads the same as
    /// "rock steady" — deliberate, since an unmeasured target is already
    /// drawn often by [`attempts`](Self::attempts) being low.
    pub recent_stability_cents: f32,
    /// Drill ordinal of the last attempt on this target, for staleness (see
    /// the Bending Trainer's `drill::next_sequence`). **Not a wall clock**:
    /// `SystemTime::now()` panics on `wasm32-unknown-unknown`, and a
    /// monotonic count of drill attempts answers "how long since I last saw
    /// this?" in the only unit the drill actually acts on. `0` = never.
    pub practiced_at: u32,
}

/// Cross-session result for one lesson, keyed by the lesson manifest's
/// stable `id` in [`PlayerProfile::lessons`] — a plain string rather than a
/// type from `harmonicon_song::lessons`, same dependency reasoning as [`DrillRecord`].
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

/// Cross-session result for one *training* — a lesson's generated drill at
/// one tier — keyed `"<lesson id>:<tier>"` in [`PlayerProfile::trainings`]
/// by [`training_key`].
///
/// Its own map rather than sharing [`PlayerProfile::lessons`]: a training
/// must never satisfy a prerequisite. Passing tier 3 of the bend drill is
/// practice, not evidence that the *lesson* was learned, and mixing the two
/// would silently unlock everything downstream of it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default)]
pub struct TrainingRecord {
    /// Once true it stays true, like [`LessonRecord::passed`] — a worse
    /// retry must not take a tier away.
    pub passed: bool,
    pub best_accuracy: f32,
    pub attempts: u32,
    /// The day this tier is next due for review, in
    /// `harmonicon_platform::calendar` days. `None` until it is first
    /// passed — and for a tier passed before reviews existed, which
    /// [`TrainingRecord::review_due_day`] therefore treats as due now.
    pub review_due: Option<u32>,
    /// Days between reviews: see [`record_training`] for how it grows.
    pub review_interval: u32,
}

/// Longest gap between reviews, in days. Doubling without a ceiling would
/// push a well-known tier out for years; a skill left alone that long is
/// worth checking again.
pub const MAX_REVIEW_INTERVAL_DAYS: u32 = 64;

impl TrainingRecord {
    /// The day this tier is due for review, or `None` if it has never been
    /// passed and so has nothing to review. A tier passed before reviews
    /// were recorded is due from day 0 — its last pass date is unknown, so
    /// it is overdue rather than forgotten.
    pub fn review_due_day(&self) -> Option<u32> {
        if !self.passed {
            return None;
        }
        Some(self.review_due.unwrap_or(0))
    }
}

/// How many days in a row the player has practised, where a single missed
/// day doesn't break the run (`docs/training_tree_plan.md` §4).
///
/// **Never framed as loss.** A gap longer than the forgiveness quietly
/// starts a new run at one day; nothing announces what was lost, and
/// [`current`](Self::current) simply reports no streak until there is one
/// again. The anxiety streaks are rightly criticised for comes from the
/// threat of losing one, so this has no threat to make.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default)]
pub struct PracticeStreak {
    /// The last day practised, in `harmonicon_platform::calendar` days.
    pub last_day: Option<u32>,
    /// Practice days in the current run.
    pub days: u32,
}

/// Days after the last practice a new one still continues the run: the
/// next day, or the one after — a single missed day is forgiven.
pub const STREAK_FORGIVEN_GAP_DAYS: u32 = 2;

impl PracticeStreak {
    /// Counts `today` as a practice day. A second practice the same day
    /// changes nothing.
    pub fn record(&mut self, today: u32) {
        self.days = match self.last_day {
            Some(last) if today == last => return,
            Some(last) if today > last && today - last <= STREAK_FORGIVEN_GAP_DAYS => self.days + 1,
            _ => 1,
        };
        self.last_day = Some(today);
    }

    /// The run as of `today`: its length while it can still be continued,
    /// otherwise 0 — a run that has lapsed is not reported at all.
    pub fn current(&self, today: u32) -> u32 {
        match self.last_day {
            Some(last) if today >= last && today - last <= STREAK_FORGIVEN_GAP_DAYS => self.days,
            _ => 0,
        }
    }
}

/// The [`PlayerProfile::trainings`] key for one tier of one lesson.
pub fn training_key(lesson_id: &str, tier: u8) -> String {
    format!("{lesson_id}:{tier}")
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
    pub trainings: HashMap<String, TrainingRecord>,
    /// Practice days in a row; see [`PracticeStreak`].
    pub practice: PracticeStreak,
}

impl PlayerProfile {
    /// How much of a lesson's training ladder is done, 0..1 — the mastery
    /// meter. `tiers` is how many the ladder has
    /// (`harmonicon_core::training::Tier::ALL.len()`), passed in rather than
    /// imported so this module stays free of the lessons vocabulary, same
    /// reasoning as the string keys above.
    ///
    /// A lesson with no trainings reports 0, not 1: nothing has been
    /// mastered, and claiming otherwise would fill a meter the player can
    /// never legitimately fill.
    pub fn mastery(&self, lesson_id: &str, tiers: usize) -> f32 {
        if tiers == 0 {
            return 0.0;
        }
        let passed = (1..=tiers)
            .filter(|t| {
                self.trainings.get(&training_key(lesson_id, *t as u8)).is_some_and(|r| r.passed)
            })
            .count();
        passed as f32 / tiers as f32
    }

    /// Ids of every lesson whose pass criteria have been met — the shape
    /// `lessons::is_unlocked` takes for prerequisite gating.
    pub fn passed_lesson_ids(&self) -> Vec<&str> {
        self.lessons.iter().filter(|(_, r)| r.passed).map(|(id, _)| id.as_str()).collect()
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

/// Updates `record` with a just-finished training attempt on day `today`.
/// Same once-passed-always-passed rule as [`record_lesson`], for the same
/// reason.
///
/// Also moves the tier's review schedule, a doubling interval:
/// - **First pass**: due again tomorrow.
/// - **Passing a review on or after its due day**: the gap doubles, up to
///   [`MAX_REVIEW_INTERVAL_DAYS`] — the skill held, so check it less often.
/// - **Passing before it was due**: the gap stays, counted from today.
///   Early practice is welcome but isn't evidence the skill lasts.
/// - **Failing a tier already passed**: back to tomorrow. The tier stays
///   passed; only how soon it comes round again changes.
pub fn record_training(record: &mut TrainingRecord, passed: bool, accuracy: f32, today: u32) {
    record.attempts += 1;
    record.best_accuracy = record.best_accuracy.max(accuracy);
    let was_passed = record.passed;
    record.passed |= passed;

    let interval = if !was_passed {
        if !passed {
            return;
        }
        1
    } else if !passed {
        1
    } else if record.review_due_day().is_some_and(|due| today >= due) {
        (record.review_interval.max(1) * 2).min(MAX_REVIEW_INTERVAL_DAYS)
    } else {
        record.review_interval.max(1)
    };
    record.review_interval = interval;
    record.review_due = Some(today + interval);
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
    harmonicon_platform::paths::config_file("profile.json")
}

/// Whether this looks like the player's very first launch — no
/// `profile.json` on disk yet. Consumed once by `menu::routing::
/// route_menu_entry`, which lands on the welcome page instead of the main
/// menu when it's set.
///
/// A resource rather than a flag each screen re-checks, because the answer
/// must be sampled *before* anything writes a profile: the welcome flow
/// itself ends by saving one, so a later `Path::exists` would report `false`
/// and the flow would appear never to have run.
#[derive(Resource, Default, Debug)]
pub struct FirstRun(pub bool);

/// True only when a profile *could* be written and none is there yet.
///
/// The "could be written" half matters: with no writable config directory
/// (`config_dir()` is `None`), finishing the welcome flow could never be
/// recorded, so it would greet the player again on every single launch.
/// Skipping it entirely is the better failure — it's a nicety, and an
/// unskippable one that repeats forever is worse than none.
///
/// Deliberately a file-existence check rather than "is `PlayerProfile`
/// empty": `load_profile` merges through figment, which silently treats a
/// missing file the same as an empty one, and a player who has genuinely
/// played nothing yet shouldn't be re-onboarded on their second launch.
fn is_first_run() -> bool {
    profile_path().is_some_and(|path| !path.exists())
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
        warn!("No writable config directory; profile not saved");
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
            if let Err(err) = harmonicon_core::config_file::write_atomic(&path, &json) {
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
/// resets on retry/loop, not cumulative session time. Ticks without marking
/// the profile changed: nothing shows play time live, and a per-frame change
/// would defeat any `PlayerProfile::is_changed()` gate for the whole song.
fn accumulate_play_time(time: Res<Time>, mut profile: ResMut<PlayerProfile>) {
    profile.bypass_change_detection().total_play_secs += time.delta_secs_f64();
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
            // Sampled at plugin build, i.e. before `apply_loaded_profile` and
            // long before anything can save — see `is_first_run`.
            .insert_resource(FirstRun(is_first_run()))
            .add_systems(Startup, apply_loaded_profile)
            .add_systems(Update, accumulate_play_time.run_if(in_state(AppState::Playing)))
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
    fn name_keyed_phrase_learned_round_trips() {
        let r: SongRecord =
            serde_json::from_str(r#"{"phrase_learned":{"intro":0.5,"turnaround":1.0}}"#).unwrap();
        assert_eq!(r.phrase_learned.get("intro"), Some(&0.5));
        assert_eq!(r.phrase_learned.get("turnaround"), Some(&1.0));
    }

    #[test]
    fn legacy_ordinal_phrase_learned_is_discarded_not_fatal() {
        // A profile.json written before name-keying existed stored
        // phrase_learned as a plain array; there's no way to recover
        // section names from bare positions, so it resets rather than
        // taking down the rest of the record.
        let r: SongRecord =
            serde_json::from_str(r#"{"best_score":42,"phrase_learned":[0.25,0.5]}"#).unwrap();
        assert_eq!(r.best_score, 42);
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
        assert!((r.best_accuracy - 0.9).abs() < f32::EPSILON, "best accuracy must not regress");
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
        p.lessons.insert("a".into(), LessonRecord { passed: true, ..Default::default() });
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
    fn legacy_drill_records_gain_recent_evidence_defaults() {
        let record: DrillRecord = serde_json::from_str(r#"{"attempts":7,"hits":3}"#).unwrap();
        assert_eq!(record.attempts, 7);
        assert_eq!(record.hits, 3);
        assert_eq!(record.skips, 0);
        assert_eq!(record.recent_samples, 0);
        assert_eq!(record.recent_control, 0.0);
        assert_eq!(record.recent_stability_cents, 0.0);
        assert_eq!(record.practiced_at, 0);
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

#[cfg(test)]
mod training_tests {
    use super::*;

    #[test]
    fn a_passed_training_stays_passed_after_a_failed_retry() {
        // Same rule as a lesson: a worse retry must not take a tier away.
        let mut r = TrainingRecord::default();
        record_training(&mut r, true, 0.9, 100);
        record_training(&mut r, false, 0.1, 100);
        assert!(r.passed);
        assert_eq!(r.attempts, 2);
        assert_eq!(r.best_accuracy, 0.9);
    }

    #[test]
    fn mastery_counts_only_the_tiers_actually_passed() {
        let mut p = PlayerProfile::default();
        assert_eq!(p.mastery("first-bend", 5), 0.0);
        for tier in [1, 2] {
            let r = p.trainings.entry(training_key("first-bend", tier)).or_default();
            record_training(r, true, 0.8, 100);
        }
        assert_eq!(p.mastery("first-bend", 5), 0.4);
    }

    #[test]
    fn an_attempted_but_failed_tier_does_not_count_toward_mastery() {
        let mut p = PlayerProfile::default();
        let r = p.trainings.entry(training_key("x", 1)).or_default();
        record_training(r, false, 0.5, 100);
        assert_eq!(p.mastery("x", 5), 0.0);
    }

    #[test]
    fn a_lesson_with_no_ladder_reports_no_mastery_rather_than_full() {
        // Claiming 1.0 would fill a meter the player can never legitimately
        // fill, and would read as "mastered" for a lesson with no drills.
        assert_eq!(PlayerProfile::default().mastery("anything", 0), 0.0);
    }

    #[test]
    fn a_training_never_lands_in_the_lessons_map() {
        // The load-bearing separation: a passed tier must not satisfy a
        // prerequisite, or finishing practice would unlock the curriculum.
        let mut p = PlayerProfile::default();
        let r = p.trainings.entry(training_key("first-bend", 5)).or_default();
        record_training(r, true, 1.0, 100);
        assert!(p.passed_lesson_ids().is_empty());
    }

    #[test]
    fn a_first_pass_is_due_again_tomorrow_and_a_failure_schedules_nothing() {
        let mut r = TrainingRecord::default();
        record_training(&mut r, false, 0.3, 100);
        assert_eq!(r.review_due_day(), None, "nothing passed, nothing to review");
        record_training(&mut r, true, 0.8, 100);
        assert_eq!(r.review_due_day(), Some(101));
    }

    #[test]
    fn on_time_reviews_double_the_gap_up_to_the_ceiling() {
        let mut r = TrainingRecord::default();
        record_training(&mut r, true, 0.8, 100);
        let mut expected_gap = 1;
        for _ in 0..10 {
            let due = r.review_due_day().unwrap();
            record_training(&mut r, true, 0.8, due);
            expected_gap = (expected_gap * 2).min(MAX_REVIEW_INTERVAL_DAYS);
            assert_eq!(r.review_due_day(), Some(due + expected_gap));
        }
        assert_eq!(r.review_interval, MAX_REVIEW_INTERVAL_DAYS);
    }

    #[test]
    fn an_early_pass_keeps_the_gap_and_a_failed_review_resets_it() {
        let mut r = TrainingRecord::default();
        record_training(&mut r, true, 0.8, 100);
        record_training(&mut r, true, 0.8, 101); // on time: gap 2, due 103
        record_training(&mut r, true, 0.8, 102); // early: gap stays 2
        assert_eq!((r.review_interval, r.review_due_day()), (2, Some(104)));

        record_training(&mut r, false, 0.2, 104);
        assert!(r.passed, "a failed review never takes the tier away");
        assert_eq!((r.review_interval, r.review_due_day()), (1, Some(105)));
    }

    #[test]
    fn a_tier_passed_before_reviews_existed_is_due_now() {
        let old: TrainingRecord =
            serde_json::from_str(r#"{"passed": true, "best_accuracy": 0.9, "attempts": 3}"#)
                .unwrap();
        assert_eq!(old.review_due_day(), Some(0));
    }

    #[test]
    fn a_streak_grows_daily_and_forgives_one_missed_day() {
        let mut streak = PracticeStreak::default();
        assert_eq!(streak.current(100), 0);
        streak.record(100);
        streak.record(100); // twice in a day is still one day
        streak.record(101);
        assert_eq!(streak.current(101), 2);
        streak.record(103); // 102 missed: forgiven
        assert_eq!(streak.current(103), 3);
        assert_eq!(streak.current(105), 3, "still continuable after one missed day");
    }

    #[test]
    fn a_lapsed_streak_is_not_reported_and_quietly_restarts() {
        let mut streak = PracticeStreak::default();
        for day in 100..105 {
            streak.record(day);
        }
        assert_eq!(streak.current(104), 5);
        assert_eq!(streak.current(107), 0, "two missed days lapse it");
        streak.record(107);
        assert_eq!(streak.current(107), 1);
    }

    #[test]
    fn a_clock_set_back_neither_extends_nor_reports_a_streak() {
        let mut streak = PracticeStreak::default();
        streak.record(100);
        streak.record(101);
        assert_eq!(streak.current(99), 0);
        streak.record(99);
        assert_eq!(streak.current(99), 1);
    }

    #[test]
    fn training_keys_do_not_collide_between_lessons_or_tiers() {
        assert_ne!(training_key("a", 1), training_key("a", 2));
        assert_ne!(training_key("a", 1), training_key("b", 1));
    }
}
