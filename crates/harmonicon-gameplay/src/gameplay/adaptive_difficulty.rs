// SPDX-License-Identifier: MIT

//! Per-phrase note unlocking ("adaptive difficulty"). A chart is divided
//! into musical-phrase sections using the existing `TrackItem::phrase` tag
//! (same boundary rule as `phrase_overlay`) — no schema change needed.
//! Each section has its own persisted "learned" fraction
//! (`profile::SongRecord::phrase_learned`); only a prefix of its notes are
//! unlocked (spawned/scored) at a time, and clearing a section cleanly
//! bumps that fraction, unlocking more on the next attempt. A manual
//! pause-menu override takes effect immediately mid-song —
//! [`resync_notes_on_adaptive_change`] rebuilds
//! `SongNotes` the moment [`AdaptiveDifficulty`] changes, carrying over
//! already-resolved score state via [`carry_over_note_state`] so a
//! previously hit/missed note doesn't reset just because the list rebuilt.

use bevy::prelude::*;
use std::collections::HashMap;

use harmonicon_app::app::SelectedSong;
use harmonicon_app::profile::PlayerProfile;
use harmonicon_core::chart::{Timing, TrackItem};
use harmonicon_song::song::SongManifest;

use super::{ScheduledNote, last_note_end, resolve_item_time};

/// One musical-phrase section: a maximal run of chart track items sharing
/// the same "in effect" `phrase` tag — the same boundary rule
/// `phrase_overlay::active_phrase_groove` uses (a new section starts at each
/// item that declares a phrase; persists until the next one declares one). A
/// chart with no phrase tags at all is a single implicit section ("Full
/// Song") spanning the whole track.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseSection {
    pub name: String,
    pub start_time: f64,
    pub end_time: f64,
    pub note_count: usize,
}

/// Groups `(start_time, phrase, event_count)` triples — one per `TrackItem`,
/// in chart order, see [`track_items`] — into [`PhraseSection`]s. `song_end`
/// closes the final section. Empty `items` yields no sections.
pub fn group_phrase_sections(
    items: &[(f64, Option<&str>, usize)],
    song_end: f64,
) -> Vec<PhraseSection> {
    let mut sections: Vec<PhraseSection> = Vec::new();
    for &(time, phrase, count) in items {
        if phrase.is_some() || sections.is_empty() {
            if let Some(last) = sections.last_mut() {
                last.end_time = time;
            }
            sections.push(PhraseSection {
                name: phrase.unwrap_or("Full Song").to_string(),
                start_time: time,
                end_time: song_end,
                note_count: count,
            });
        } else if let Some(last) = sections.last_mut() {
            last.note_count += count;
        }
    }
    sections
}

/// Stable per-section keys for persisting `learned` progress across a chart
/// re-edit: a section's own phrase name, unless an earlier section already
/// used it (e.g. two "chorus" sections), in which case later occurrences
/// get a `" #2"`, `" #3"`, ... suffix. The read/write boundary
/// [`learned_vec_from_map`]/[`write_learned_into_map`] use to translate
/// between the session's ordinal `Vec<f32>` (indexed by section position,
/// what `unlocked_flags`/`bump_learned_sections` expect) and
/// `profile::SongRecord::phrase_learned`'s persisted, name-keyed map —
/// keying by name means reordering/inserting/removing a phrase tag
/// elsewhere in the chart can't misapply an unrelated section's progress.
pub fn section_keys(sections: &[PhraseSection]) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    sections
        .iter()
        .map(|s| {
            let n = counts.entry(s.name.as_str()).or_insert(0);
            *n += 1;
            if *n == 1 {
                s.name.clone()
            } else {
                format!("{} #{}", s.name, n)
            }
        })
        .collect()
}

/// Builds the session's ordinal `learned` vector (indexed by section
/// position, as [`unlocked_flags`]/[`bump_learned_sections`] expect) from the
/// persisted, name-keyed map — the read side of the scheme [`section_keys`]
/// documents. A section with no entry in `map` (never played, or renamed
/// since it was) reads as unlearned (0.0).
pub fn learned_vec_from_map(sections: &[PhraseSection], map: &HashMap<String, f32>) -> Vec<f32> {
    section_keys(sections)
        .iter()
        .map(|key| map.get(key).copied().unwrap_or(0.0))
        .collect()
}

/// Writes the session's ordinal `learned` vector back into the persisted,
/// name-keyed map — the write side of the scheme [`section_keys`]
/// documents. Extra entries in `map` for keys no longer present among
/// `sections` (e.g. a renamed/removed phrase) are left alone rather than
/// pruned — harmless stale data, not worth the risk of deleting a section
/// that's only temporarily absent from a partial rebuild.
pub fn write_learned_into_map(
    sections: &[PhraseSection],
    learned: &[f32],
    map: &mut HashMap<String, f32>,
) {
    for (key, &value) in section_keys(sections).iter().zip(learned) {
        map.insert(key.clone(), value);
    }
}

/// Fraction of a phrase's notes unlocked at a given `learned` level (clamped
/// to 0..=1): 10% at `learned = 0`, scaling linearly up to 100% at
/// `learned = 1` — the "start with a skeleton, fill in as you learn it" curve.
pub const fn visible_fraction(learned: f32) -> f32 {
    (0.1 + 0.9 * learned.clamp(0.0, 1.0)).min(1.0)
}

/// How many of a section's `note_count` notes are unlocked at `learned` —
/// always at least 1 (never a fully silent phrase) and never more than
/// `note_count`.
const fn active_note_count(note_count: usize, learned: f32) -> usize {
    if note_count == 0 {
        return 0;
    }
    let visible = visible_fraction(learned);
    let count = (visible * note_count as f32).ceil() as usize;
    if count < 1 {
        1
    } else if count > note_count {
        note_count
    } else {
        count
    }
}

/// Per-event `(unlocked, section_index)`, in the same flattened
/// `for item in items { for _ in 0..event_count }` order `gameplay_2d`/
/// `gameplay_3d` build their `ScheduledNote`s in, so each can filter while
/// building without duplicating phrase grouping. Within a section, the
/// first `active_note_count` notes (in time order) are unlocked — a prefix
/// reveal, not scattered sampling, so a single percentage fully describes
/// how far into the phrase play has been unlocked. `learned` is indexed by
/// section ordinal; a missing/short entry defaults to unlearned (0.0).
/// `enabled: false` unlocks every note regardless of `learned`.
pub fn unlocked_flags(
    items: &[(f64, Option<&str>, usize)],
    sections: &[PhraseSection],
    learned: &[f32],
    enabled: bool,
) -> Vec<(bool, usize)> {
    let mut flags = Vec::new();
    let mut section_idx = 0usize;
    let mut ordinal = 0usize;
    let mut first = true;
    for &(_, phrase, count) in items {
        if phrase.is_some() && !first {
            section_idx += 1;
            ordinal = 0;
        }
        first = false;
        let note_count = sections
            .get(section_idx)
            .map(|s| s.note_count)
            .unwrap_or(count);
        let learned_frac = learned.get(section_idx).copied().unwrap_or(0.0);
        let active_count = if enabled {
            active_note_count(note_count, learned_frac)
        } else {
            note_count
        };
        for _ in 0..count {
            flags.push((ordinal < active_count, section_idx));
            ordinal += 1;
        }
    }
    flags
}

/// How much a phrase section's `learned` fraction advances per clean clear
/// (every unlocked note hit, none missed) — reaches 100% after four clean
/// clears from scratch.
pub const PHRASE_LEARN_STEP: f32 = 0.25;

/// After a full playthrough, bumps each phrase section's learned fraction by
/// [`PHRASE_LEARN_STEP`] (capped at 1.0) if every one of its notes present
/// in `notes` was hit cleanly (no misses) — locked notes never made it into
/// `notes` in the first place (see [`unlocked_flags`]), so this only judges
/// what was actually playable. `learned` grows to at least `section_count`
/// entries (new sections start at 0.0, same as a song's first-ever play).
/// Sections with no notes seen this run (e.g. a partial/looped play that
/// never reached them) are left untouched — nothing to judge a clear from.
pub fn bump_learned_sections(
    notes: &[ScheduledNote],
    section_count: usize,
    learned: &mut Vec<f32>,
) {
    if learned.len() < section_count {
        learned.resize(section_count, 0.0);
    }
    let mut seen = vec![false; section_count];
    let mut all_hit = vec![true; section_count];
    for note in notes {
        let idx = note.phrase_section;
        if idx >= section_count {
            continue;
        }
        seen[idx] = true;
        if !note.hit || note.missed {
            all_hit[idx] = false;
        }
    }
    for i in 0..section_count {
        if seen[i] && all_hit[i] {
            learned[i] = (learned[i] + PHRASE_LEARN_STEP).min(1.0);
        }
    }
}

/// `(time, phrase, event_count)` triples for every track item, in chart
/// order — the shared shape `group_phrase_sections`/`unlocked_flags` and the
/// note-building loops in `gameplay_2d`/`gameplay_3d` all key off.
pub fn track_items<'a>(
    track: &'a [TrackItem],
    timing: &Timing,
) -> Vec<(f64, Option<&'a str>, usize)> {
    track
        .iter()
        .map(|item| {
            (
                resolve_item_time(item, timing),
                item.phrase.as_deref(),
                item.events.len(),
            )
        })
        .collect()
}

/// Copies resolved score state (`hit`, `missed`, `held`, `sustain_scored`,
/// `pitch_samples`, `amp_samples`) from `old` into matching notes in `new`,
/// matched by `(time, hole, is_blow)` — stable across a rebuild since both
/// lists derive from the same chart, regardless of which other notes got
/// unlocked/relocked around a given note. Used when the pause menu changes
/// `learned`/`enabled` mid-song so already-hit/missed notes don't reset
/// just because the list was rebuilt. A note with no match in `old`
/// (freshly unlocked) keeps its default unresolved state; `used` guards
/// against double-matching two notes sharing an identical key (e.g. a
/// chord/split voicing the same hole/direction twice — rare, cheap to guard).
pub fn carry_over_note_state(old: &[ScheduledNote], new: &mut [ScheduledNote]) {
    let mut used = vec![false; old.len()];
    for note in new.iter_mut() {
        let Some(idx) = old.iter().enumerate().position(|(i, o)| {
            !used[i]
                && o.hole == note.hole
                && o.is_blow == note.is_blow
                && (o.time - note.time).abs() < 1e-6
        }) else {
            continue;
        };
        used[idx] = true;
        let src = &old[idx];
        note.hit = src.hit;
        note.missed = src.missed;
        note.held = src.held;
        note.sustain_scored = src.sustain_scored;
        note.pitch_samples = src.pitch_samples.clone();
        note.amp_samples = src.amp_samples.clone();
    }
}

/// Index of the first not-fully-resolved note in a freshly rebuilt list —
/// the same definition as `SongNotes::cursor`'s doc comment (not `missed`,
/// and not both `hit` and `sustain_scored`). Used to reset the cursor after
/// [`carry_over_note_state`], since a rebuild can reorder/insert/remove
/// notes such that whatever the old cursor pointed at is no longer
/// meaningful.
pub fn first_unresolved_index(notes: &[ScheduledNote]) -> usize {
    notes
        .iter()
        .position(|n| !(n.missed || (n.hit && n.sustain_scored)))
        .unwrap_or(notes.len())
}

/// The shared core of `gameplay_2d`/`gameplay_3d`'s own
/// `resync_notes_on_adaptive_change`: rebuilds `song_notes` for `chart`'s
/// current unlock state, carries over already-resolved score state
/// ([`carry_over_note_state`]), and resets the cursor
/// ([`first_unresolved_index`]). Each mode keeps only its own
/// visual-despawn (and, for 2D, `NoteRenderAssets::play_mode_tags`) lines —
/// this is what both call sites had in common before extraction. Returns
/// the chord/split-badge tags parallel to the rebuilt notes; 3D has no such
/// badge and discards them.
pub fn rebuild_song_notes(
    effective: &harmonicon_app::app::EffectiveHarmonica,
    chart: &harmonicon_core::chart::HarpChart,
    adaptive: &AdaptiveDifficulty,
    song_notes: &mut super::SongNotes,
) -> Vec<Option<&'static str>> {
    let (mut new_notes, new_tags) = super::build_scheduled_notes(effective, chart, adaptive);
    carry_over_note_state(&song_notes.notes, &mut new_notes);
    song_notes.cursor = first_unresolved_index(&new_notes);
    song_notes.notes = new_notes;
    new_tags
}

/// Emitted only after the shared note array and its parallel tags are rebuilt.
#[derive(Message)]
pub(super) struct NotesRebuilt;

pub(super) fn resync_notes_on_adaptive_change(
    effective: Res<harmonicon_app::app::EffectiveHarmonica>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    adaptive: Res<AdaptiveDifficulty>,
    mut notes: ResMut<super::SongNotes>,
    mut assets: ResMut<super::gameplay_2d::NoteRenderAssets>,
    mut rebuilt: MessageWriter<NotesRebuilt>,
) {
    if !adaptive.is_changed() {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    assets.play_mode_tags = rebuild_song_notes(&effective, &manifest.chart, &adaptive, &mut notes);
    rebuilt.write(NotesRebuilt);
}

pub(super) fn invalidate_note_visuals(
    mut rebuilt: MessageReader<NotesRebuilt>,
    mut commands: Commands,
    visuals: Query<
        Entity,
        Or<(
            With<super::NoteVisual>,
            With<super::gameplay_3d::NoteVisual3D>,
        )>,
    >,
) {
    if rebuilt.read().count() == 0 {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
}

/// Live per-session cache of a song's phrase sections + adaptive-difficulty
/// state — built once at song start (`setup_adaptive_difficulty`) from the
/// chart, [`PlayerProfile`] (learned progress), and the global
/// `settings::AdaptiveDifficultyEnabled` setting, read by the note-unlock
/// filter (`gameplay_2d`/`gameplay_3d` setup), the progress-bar phrase strip
/// (`song_progress_overlay`), and the pause menu's phrase selector. The
/// pause menu's on/off toggle writes through to both
/// `settings::AdaptiveDifficultyEnabled` (persisted, global) and this
/// resource's `enabled` directly, driving both the immediate mid-song
/// note re-unlock and the progress-bar overlay's live re-tint.
#[derive(Resource, Default)]
pub struct AdaptiveDifficulty {
    pub enabled: bool,
    pub sections: Vec<PhraseSection>,
    pub learned: Vec<f32>,
}

/// Populates [`AdaptiveDifficulty`] from the selected song's chart, its
/// `PlayerProfile` record (learned progress only), and the global
/// `AdaptiveDifficultyEnabled` Options-menu setting (whether it's on at
/// all — not per-song). Must run before `gameplay_2d::setup`/
/// `gameplay_3d::setup` so they see this run's unlock state — ordered by
/// tuple position in `GameplayPlugin::build`, same as `setup_scoring_config`
/// precedes them today.
pub(super) fn setup_adaptive_difficulty(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    profile: Res<PlayerProfile>,
    setting: Res<harmonicon_platform::settings::AdaptiveDifficultyEnabled>,
    lesson: Option<Res<harmonicon_song::lessons::LessonContext>>,
    mut adaptive: ResMut<AdaptiveDifficulty>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        *adaptive = AdaptiveDifficulty::default();
        return;
    };
    let chart = &manifest.chart;
    let items = track_items(&chart.track, &chart.timing);
    let song_end = last_note_end(&chart.track, &chart.timing);
    let sections = group_phrase_sections(&items, song_end);

    // A lesson chart is a curriculum unit, not a song being learned
    // phrase-by-phrase: its pass criteria assume every note is present, so
    // adaptive gating stays off for lesson runs regardless of the global
    // setting.
    let enabled_for_lesson = lesson.is_none();

    let key = manifest.path.display().to_string();
    let record = profile.songs.get(&key);
    let learned = record
        .map(|r| learned_vec_from_map(&sections, &r.phrase_learned))
        .unwrap_or_default();
    *adaptive = AdaptiveDifficulty {
        enabled: enabled_for_lesson && setting.0,
        learned,
        sections,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(hit: bool, missed: bool, section: usize) -> ScheduledNote {
        ScheduledNote {
            time: 0.0,
            duration: 0.1,
            hole: 4,
            is_blow: false,
            expected_pitch: Some(62),
            hit,
            missed,
            held: 0.0,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: section,
            chord_pitches: Vec::new(),
            playable: true,
            force_wait: false,
        }
    }

    // ── group_phrase_sections ──────────────────────────────────────────────

    #[test]
    fn empty_items_yields_no_sections() {
        assert!(group_phrase_sections(&[], 10.0).is_empty());
    }

    #[test]
    fn no_phrase_tags_is_one_implicit_section() {
        let items = [(0.0, None, 2usize), (1.0, None, 3)];
        let sections = group_phrase_sections(&items, 5.0);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Full Song");
        assert_eq!(sections[0].start_time, 0.0);
        assert_eq!(sections[0].end_time, 5.0);
        assert_eq!(sections[0].note_count, 5);
    }

    #[test]
    fn phrase_tags_split_into_sections() {
        let items = [
            (0.0, Some("intro"), 2usize),
            (1.0, None, 1),
            (2.0, Some("turnaround"), 3),
        ];
        let sections = group_phrase_sections(&items, 10.0);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "intro");
        assert_eq!(sections[0].start_time, 0.0);
        assert_eq!(sections[0].end_time, 2.0);
        assert_eq!(sections[0].note_count, 3);
        assert_eq!(sections[1].name, "turnaround");
        assert_eq!(sections[1].start_time, 2.0);
        assert_eq!(sections[1].end_time, 10.0);
        assert_eq!(sections[1].note_count, 3);
    }

    // ── visible_fraction / active_note_count ───────────────────────────────

    #[test]
    fn visible_fraction_is_ten_percent_unlearned() {
        assert!((visible_fraction(0.0) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn visible_fraction_is_full_when_learned() {
        assert!((visible_fraction(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn visible_fraction_clamps_out_of_range_input() {
        assert!((visible_fraction(-1.0) - 0.1).abs() < 1e-6);
        assert!((visible_fraction(2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn active_note_count_is_never_zero_for_a_nonempty_section() {
        assert_eq!(active_note_count(20, 0.0), 2); // ceil(0.1 * 20)
        assert_eq!(active_note_count(3, 0.0), 1); // ceil(0.1 * 3) = 1
    }

    #[test]
    fn active_note_count_reaches_the_full_section_when_learned() {
        assert_eq!(active_note_count(20, 1.0), 20);
    }

    #[test]
    fn active_note_count_is_zero_for_an_empty_section() {
        assert_eq!(active_note_count(0, 0.5), 0);
    }

    // ── unlocked_flags ──────────────────────────────────────────────────────

    #[test]
    fn disabled_unlocks_everything_regardless_of_learned() {
        let items = [(0.0, Some("intro"), 4usize)];
        let sections = group_phrase_sections(&items, 4.0);
        let flags = unlocked_flags(&items, &sections, &[0.0], false);
        assert!(flags.iter().all(|&(unlocked, _)| unlocked));
    }

    #[test]
    fn unlearned_section_unlocks_only_a_prefix() {
        let items = [(0.0, Some("intro"), 10usize)];
        let sections = group_phrase_sections(&items, 10.0);
        let flags = unlocked_flags(&items, &sections, &[0.0], true);
        let unlocked_count = flags.iter().filter(|&&(u, _)| u).count();
        assert_eq!(unlocked_count, 1); // ceil(0.1 * 10) = 1
        // Unlocked notes are the earliest ones — a prefix, not scattered.
        assert!(flags[0].0);
        assert!(!flags[9].0);
    }

    #[test]
    fn missing_learned_entry_defaults_to_unlearned() {
        let items = [(0.0, Some("intro"), 10usize)];
        let sections = group_phrase_sections(&items, 10.0);
        let flags = unlocked_flags(&items, &sections, &[], true);
        assert_eq!(flags.iter().filter(|&&(u, _)| u).count(), 1);
    }

    #[test]
    fn fully_learned_section_unlocks_everything() {
        let items = [(0.0, Some("intro"), 10usize)];
        let sections = group_phrase_sections(&items, 10.0);
        let flags = unlocked_flags(&items, &sections, &[1.0], true);
        assert!(flags.iter().all(|&(u, _)| u));
    }

    #[test]
    fn each_note_is_tagged_with_its_section_index() {
        let items = [(0.0, Some("intro"), 2usize), (1.0, Some("turnaround"), 2)];
        let sections = group_phrase_sections(&items, 5.0);
        let flags = unlocked_flags(&items, &sections, &[1.0, 1.0], true);
        assert_eq!(
            flags.iter().map(|&(_, s)| s).collect::<Vec<_>>(),
            vec![0, 0, 1, 1]
        );
    }

    // ── bump_learned_sections ────────────────────────────────────────────────

    #[test]
    fn clean_clear_bumps_learned_by_one_step() {
        let notes = vec![note(true, false, 0), note(true, false, 0)];
        let mut learned = vec![0.0];
        bump_learned_sections(&notes, 1, &mut learned);
        assert!((learned[0] - PHRASE_LEARN_STEP).abs() < 1e-6);
    }

    #[test]
    fn a_miss_leaves_learned_unchanged() {
        let notes = vec![note(true, false, 0), note(false, true, 0)];
        let mut learned = vec![0.25];
        bump_learned_sections(&notes, 1, &mut learned);
        assert_eq!(learned[0], 0.25);
    }

    #[test]
    fn repeated_clean_clears_cap_at_one() {
        let notes = vec![note(true, false, 0)];
        let mut learned = vec![0.9];
        bump_learned_sections(&notes, 1, &mut learned);
        assert_eq!(learned[0], 1.0);
    }

    #[test]
    fn a_section_with_no_notes_seen_is_left_untouched() {
        let notes = vec![note(true, false, 0)]; // section 1 never appears
        let mut learned = vec![0.2, 0.2];
        bump_learned_sections(&notes, 2, &mut learned);
        assert!((learned[0] - (0.2 + PHRASE_LEARN_STEP)).abs() < 1e-6);
        assert_eq!(learned[1], 0.2);
    }

    #[test]
    fn learned_vec_grows_to_fit_new_sections() {
        let notes = vec![note(true, false, 2)];
        let mut learned = vec![0.0]; // only one section previously known
        bump_learned_sections(&notes, 3, &mut learned);
        assert_eq!(learned.len(), 3);
        assert!((learned[2] - PHRASE_LEARN_STEP).abs() < 1e-6);
    }

    // ── section_keys / learned_vec_from_map / write_learned_into_map ────────

    fn section(name: &str) -> PhraseSection {
        PhraseSection {
            name: name.to_string(),
            start_time: 0.0,
            end_time: 1.0,
            note_count: 1,
        }
    }

    #[test]
    fn section_keys_uses_the_phrase_name_when_unique() {
        let sections = [section("intro"), section("turnaround")];
        assert_eq!(section_keys(&sections), vec!["intro", "turnaround"]);
    }

    #[test]
    fn section_keys_disambiguates_repeated_names() {
        let sections = [section("chorus"), section("verse"), section("chorus")];
        assert_eq!(
            section_keys(&sections),
            vec!["chorus", "verse", "chorus #2"]
        );
    }

    #[test]
    fn learned_vec_from_map_reads_by_name_not_position() {
        let sections = [section("intro"), section("turnaround")];
        let mut map = HashMap::new();
        map.insert("turnaround".to_string(), 0.75);
        assert_eq!(learned_vec_from_map(&sections, &map), vec![0.0, 0.75]);
    }

    #[test]
    fn reordering_sections_keeps_progress_with_the_right_name() {
        // The bug this whole scheme fixes: re-editing a chart can reorder
        // phrase tags. Progress recorded against "turnaround" must stay
        // with "turnaround" even though its ordinal position changed.
        let old_sections = [section("intro"), section("turnaround")];
        let mut map = HashMap::new();
        map.insert("turnaround".to_string(), 0.75);
        let reordered = [section("turnaround"), section("intro")];
        assert_eq!(learned_vec_from_map(&old_sections, &map), vec![0.0, 0.75]);
        assert_eq!(learned_vec_from_map(&reordered, &map), vec![0.75, 0.0]);
    }

    #[test]
    fn write_learned_into_map_round_trips_through_learned_vec_from_map() {
        let sections = [section("intro"), section("turnaround")];
        let mut map = HashMap::new();
        write_learned_into_map(&sections, &[0.25, 1.0], &mut map);
        assert_eq!(map.get("intro"), Some(&0.25));
        assert_eq!(map.get("turnaround"), Some(&1.0));
        assert_eq!(learned_vec_from_map(&sections, &map), vec![0.25, 1.0]);
    }

    // ── carry_over_note_state / first_unresolved_index ──────────────────────

    fn note_at(time: f64, hole: u8, is_blow: bool) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.5,
            hole,
            is_blow,
            expected_pitch: Some(60),
            hit: false,
            missed: false,
            held: 0.0,
            sustain_scored: false,
            modifiers: Vec::new(),
            pitch_samples: Vec::new(),
            amp_samples: Vec::new(),
            phrase_section: 0,
            chord_pitches: Vec::new(),
            playable: true,
            force_wait: false,
        }
    }

    #[test]
    fn carry_over_preserves_hit_state_for_a_matching_note() {
        let mut old = note_at(1.0, 2, false);
        old.hit = true;
        old.sustain_scored = true;
        old.held = 0.4;
        let old_notes = vec![old];
        let mut new_notes = vec![note_at(1.0, 2, false)];
        carry_over_note_state(&old_notes, &mut new_notes);
        assert!(new_notes[0].hit);
        assert!(new_notes[0].sustain_scored);
        assert_eq!(new_notes[0].held, 0.4);
    }

    #[test]
    fn carry_over_leaves_a_newly_unlocked_note_unresolved() {
        let old_notes = vec![note_at(1.0, 2, false)]; // no note at t=2.0 yet
        let mut new_notes = vec![note_at(1.0, 2, false), note_at(2.0, 3, false)];
        carry_over_note_state(&old_notes, &mut new_notes);
        assert!(!new_notes[1].hit);
        assert!(!new_notes[1].missed);
    }

    #[test]
    fn carry_over_matches_by_time_hole_and_direction_not_position() {
        // Inserting a new note earlier shifts every later note's index —
        // matching must go by identity, not array position.
        let mut old = note_at(5.0, 3, true);
        old.missed = true;
        let old_notes = vec![old];
        let mut new_notes = vec![note_at(1.0, 1, false), note_at(5.0, 3, true)];
        carry_over_note_state(&old_notes, &mut new_notes);
        assert!(
            !new_notes[0].missed,
            "the newly-inserted note must not match"
        );
        assert!(new_notes[1].missed, "the pre-existing note keeps its state");
    }

    #[test]
    fn carry_over_does_not_double_match_two_old_notes_to_one_new_note() {
        let mut hit_note = note_at(1.0, 2, false);
        hit_note.hit = true;
        let mut missed_note = note_at(1.0, 2, false); // identical key
        missed_note.missed = true;
        let old_notes = vec![hit_note, missed_note];
        let mut new_notes = vec![note_at(1.0, 2, false)];
        carry_over_note_state(&old_notes, &mut new_notes);
        // Whichever it matched first, it must reflect exactly one source,
        // not a mix (e.g. both hit and missed at once).
        assert_ne!(new_notes[0].hit, new_notes[0].missed);
    }

    #[test]
    fn first_unresolved_index_finds_the_first_note_not_fully_resolved() {
        let mut n0 = note_at(0.0, 1, false);
        n0.hit = true;
        n0.sustain_scored = true;
        let mut n1 = note_at(1.0, 1, false);
        n1.missed = true;
        let n2 = note_at(2.0, 1, false); // untouched
        assert_eq!(first_unresolved_index(&[n0, n1, n2]), 2);
    }

    #[test]
    fn first_unresolved_index_is_the_length_when_everything_is_resolved() {
        let mut n0 = note_at(0.0, 1, false);
        n0.missed = true;
        assert_eq!(first_unresolved_index(&[n0]), 1);
    }

    #[test]
    fn first_unresolved_index_is_zero_for_an_empty_list() {
        assert_eq!(first_unresolved_index(&[]), 0);
    }
}

#[cfg(test)]
mod invalidation_tests {
    use super::*;

    #[test]
    fn both_presentations_invalidate_only_after_a_successful_rebuild() {
        let mut app = App::new();
        app.add_message::<NotesRebuilt>()
            .add_systems(Update, invalidate_note_visuals);
        let two = app
            .world_mut()
            .spawn(super::super::NoteVisual { note_id: 0 })
            .id();
        let three = app
            .world_mut()
            .spawn(super::super::gameplay_3d::NoteVisual3D { note_id: 0 })
            .id();
        let other = app.world_mut().spawn_empty().id();
        app.update();
        assert!(app.world().get_entity(two).is_ok());
        assert!(app.world().get_entity(three).is_ok());
        app.world_mut().write_message(NotesRebuilt);
        app.update();
        assert!(app.world().get_entity(two).is_err());
        assert!(app.world().get_entity(three).is_err());
        assert!(app.world().get_entity(other).is_ok());
    }
}
