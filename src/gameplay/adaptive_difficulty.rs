// SPDX-License-Identifier: MIT

//! Per-phrase note unlocking ("adaptive difficulty"). A chart is divided
//! into musical-phrase sections using the existing `TrackItem::phrase` tag
//! (see `phrase_overlay`'s identical boundary rule) — no chart schema change
//! needed, since every bundled/external song already tags phrases densely.
//! Each section has its own persisted "learned" fraction
//! (`profile::SongRecord::phrase_learned`). Only a prefix of a section's
//! notes are unlocked (spawned/scored) at a time; clearing a section
//! cleanly bumps its learned fraction, unlocking more of it on the next
//! attempt — chart notes are fixed once `SongNotes` is built at song start,
//! so newly-unlocked notes can only take effect on the next `OnEnter
//! (Playing)` (a Restart), not mid-song.

use bevy::prelude::*;

use crate::menu::SelectedSong;
use crate::profile::PlayerProfile;
use crate::song::SongManifest;
use crate::song::chart::{Timing, TrackItem};

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

/// Fraction of a phrase's notes unlocked at a given `learned` level (clamped
/// to 0..=1): 10% at `learned = 0`, scaling linearly up to 100% at
/// `learned = 1` — the "start with a skeleton, fill in as you learn it" curve.
pub fn visible_fraction(learned: f32) -> f32 {
    (0.1 + 0.9 * learned.clamp(0.0, 1.0)).min(1.0)
}

/// How many of a section's `note_count` notes are unlocked at `learned` —
/// always at least 1 (never a fully silent phrase) and never more than
/// `note_count`.
fn active_note_count(note_count: usize, learned: f32) -> usize {
    if note_count == 0 {
        return 0;
    }
    let visible = visible_fraction(learned);
    ((visible * note_count as f32).ceil() as usize).clamp(1, note_count)
}

/// Per-event `(unlocked, section_index)`, in the same flattened
/// `for item in items { for _ in 0..event_count }` order `gameplay_2d`/
/// `gameplay_3d` build their `ScheduledNote`s in, so each can filter while
/// building without duplicating phrase grouping. Within a section, the
/// first `active_note_count` notes (in time order) are unlocked — a prefix
/// reveal, not an evenly-spaced sampling, so a single percentage is enough
/// to describe (and manually set) how far into the phrase play has been
/// unlocked. `learned` is indexed by section ordinal; a missing/short entry
/// defaults to unlearned (0.0). When `enabled` is false every note is
/// unlocked regardless of `learned`.
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
pub fn bump_learned_sections(notes: &[ScheduledNote], section_count: usize, learned: &mut Vec<f32>) {
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
pub fn track_items<'a>(track: &'a [TrackItem], timing: &Timing) -> Vec<(f64, Option<&'a str>, usize)> {
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

/// Live per-session cache of a song's phrase sections + adaptive-difficulty
/// state — built once at song start (`setup_adaptive_difficulty`) from the
/// chart + [`PlayerProfile`], read by the note-unlock filter (`gameplay_2d`/
/// `gameplay_3d` setup), the progress-bar phrase strip
/// (`song_progress_overlay`), and the pause menu's manual phrase selector.
/// Pause-menu edits write through to both `PlayerProfile` (persisted) and
/// this resource (so the overlay updates instantly) — the note-unlock
/// effect itself only takes hold on the next `OnEnter(Playing)` (a
/// Restart), since `SongNotes` is only ever built once per song entry.
#[derive(Resource, Default)]
pub struct AdaptiveDifficulty {
    pub enabled: bool,
    pub sections: Vec<PhraseSection>,
    pub learned: Vec<f32>,
}

/// Populates [`AdaptiveDifficulty`] from the selected song's chart and its
/// `PlayerProfile` record (defaulting to enabled + nothing learned yet for a
/// song with no record). Must run before `gameplay_2d::setup`/
/// `gameplay_3d::setup` so they see this run's unlock state — ordered by
/// tuple position in `GameplayPlugin::build`, same as `setup_scoring_config`
/// precedes them today.
pub(super) fn setup_adaptive_difficulty(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    profile: Res<PlayerProfile>,
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

    let key = manifest.path.display().to_string();
    let record = profile.songs.get(&key);
    *adaptive = AdaptiveDifficulty {
        enabled: record.map(|r| r.adaptive_difficulty_enabled).unwrap_or(true),
        learned: record.map(|r| r.phrase_learned.clone()).unwrap_or_default(),
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
}
