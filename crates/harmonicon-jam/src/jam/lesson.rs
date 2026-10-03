// SPDX-License-Identifier: MIT

//! On-demand judging for open-ended Jam lessons.

use bevy::prelude::*;

use harmonicon_app::app::{AppState, ReturnToSongList};
use harmonicon_app::profile::{PlayerProfile, record_lesson, save_profile};
use harmonicon_gameplay::gameplay::Paused;
use harmonicon_gameplay::gameplay::pause_menu::{FinishLessonRequested, apply_quit};
use harmonicon_song::lessons::{LessonContext, PassCriteria, lesson_passed};

use super::improv::ImprovStats;

/// Picks the one `ImprovStats` fraction relevant to `criteria` — the three
/// jam-based `PassCriteria` variants (`ScaleAdherence`/`ChordToneAdherence`/
/// `PhraseDiscipline`) each read a different tally off the same running
/// stats; `None` for a chart-backed criterion (or no criterion), which never
/// reads `ImprovStats` at all. Pure so it's directly unit-testable.
pub(crate) fn jam_fraction_for(
    criteria: Option<&PassCriteria>,
    stats: &ImprovStats,
) -> Option<f32> {
    match criteria {
        Some(PassCriteria::ScaleAdherence { .. }) => stats.adherence(),
        Some(PassCriteria::ChordToneAdherence { .. }) => stats.chord_tone_adherence(),
        Some(PassCriteria::PhraseDiscipline { .. }) => stats.phrase_discipline(),
        _ => None,
    }
}

/// Judges a jam-based lesson on demand — the only lesson types with no
/// natural end to judge them at (see `PassCriteria::ScaleAdherence`).
/// Records the result and returns to the menu the same way "Quit Song"
/// does; `route_menu_entry` sees the still-present `LessonContext` and
/// routes to the lesson list from there, same as any other lesson.
pub fn finish_jam_lesson(
    mut requested: MessageReader<FinishLessonRequested>,
    lesson: Option<Res<LessonContext>>,
    improv_stats: Res<ImprovStats>,
    mut profile: ResMut<PlayerProfile>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
) {
    if requested.read().next().is_none() {
        return;
    }
    // Only meaningful with a lesson in flight; the button is only ever shown
    // then, but the message carries no proof of that.
    let Some(lesson) = lesson else {
        return;
    };
    let fraction = jam_fraction_for(lesson.pass_criteria.as_ref(), &improv_stats);
    let passed = lesson_passed(lesson.pass_criteria.as_ref(), 0.0, &[], fraction);
    let record = profile.lessons.entry(lesson.lesson_id.clone()).or_default();
    record_lesson(record, passed, fraction.unwrap_or(0.0));
    save_profile(&profile);
    apply_quit(&mut paused, &mut next_state, &mut return_to_song_list);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── jam_fraction_for ─────────────────────────────────────────────────────

    fn stats(
        chord_tone: u32,
        in_scale: u32,
        out_of_scale: u32,
        rest_violations: u32,
    ) -> ImprovStats {
        ImprovStats { chord_tone, in_scale, out_of_scale, rest_violations }
    }

    #[test]
    fn jam_fraction_for_reads_the_matching_stat() {
        let s = stats(3, 5, 2, 1);
        assert_eq!(
            jam_fraction_for(Some(&PassCriteria::ScaleAdherence { threshold: 0.1 }), &s),
            s.adherence()
        );
        assert_eq!(
            jam_fraction_for(Some(&PassCriteria::ChordToneAdherence { threshold: 0.1 }), &s),
            s.chord_tone_adherence()
        );
        assert_eq!(
            jam_fraction_for(Some(&PassCriteria::PhraseDiscipline { threshold: 0.1 }), &s),
            s.phrase_discipline()
        );
    }

    #[test]
    fn jam_fraction_for_is_none_for_a_non_jam_criterion() {
        let s = stats(3, 5, 2, 1);
        assert_eq!(jam_fraction_for(None, &s), None);
        assert_eq!(jam_fraction_for(Some(&PassCriteria::Accuracy { threshold: 0.5 }), &s), None);
    }
}
