// SPDX-License-Identifier: MIT

//! What a finished run *means*, as pure functions over [`SongStats`] — the
//! results screen (`results.rs`, the only consumer) just renders what these
//! return. Every judgment here has a minimum sample size, so one attempted
//! bend never becomes "your bends need work".

use harmonicon_song::lessons::PassCriteria;

use super::state::{SongStats, TechniqueStats};

/// Notes a technique needs before its accuracy can be the run's observation
/// or count as evidence at all.
pub const MIN_TECHNIQUE_SAMPLES: u32 = 4;
/// Notes a run needs before anything about it is worth saying.
pub const MIN_NOTES: u32 = 8;
/// Hits needed before the timing distribution is trusted — for the timing
/// observation and for offering the Input-lag adjustment.
pub const MIN_TIMING_SAMPLES: u32 = 8;
/// A technique landing below this rate is a practice opportunity.
pub const WEAK_TECHNIQUE: f32 = 0.6;
/// A technique also has to trail plain notes by this much to be singled
/// out — if everything is landing at 50%, the problem isn't the bends.
pub const TECHNIQUE_GAP: f32 = 0.2;
/// A run missing this fraction of its notes is about hitting notes at all.
pub const HIGH_MISS_RATE: f32 = 0.35;
/// Share of hits that must fall on one side of the target before timing
/// reads as a consistent lean rather than ordinary scatter.
pub const LOPSIDED_SHARE: f32 = 0.6;
/// Mean offset (ms) below which no Input-lag change is worth a click.
pub const MIN_LATENCY_ADJUST_MS: f64 = 5.0;

/// Technique buckets in display order, keyed by the same names
/// `PlayerProfile` stores per-technique bests under (`"normal"` for the
/// no-modifier baseline; see `judge::modifier_fx_key`).
pub fn technique_buckets(stats: &SongStats) -> [(&'static str, TechniqueStats); 8] {
    [
        ("normal", stats.normal),
        ("bend", stats.bend),
        ("vibrato", stats.vibrato),
        ("wah-wah", stats.wah),
        ("overblow", stats.overblow),
        ("overdraw", stats.overdraw),
        ("slide", stats.slide),
        ("clean-attack", stats.clean_attack),
    ]
}

/// Technique rows the song actually exercised, most practice-worthy first:
/// by misses (notes to be gained), then by accuracy, then display order.
/// Sample counts stay attached so a 0/1 row can't masquerade as a trend.
pub fn ranked_techniques(stats: &SongStats) -> Vec<(&'static str, TechniqueStats)> {
    let mut rows: Vec<_> =
        technique_buckets(stats).into_iter().filter(|(_, s)| s.total() > 0).collect();
    // `sort_by` is stable, so equal rows keep display order.
    rows.sort_by(|(_, a), (_, b)| {
        b.misses.cmp(&a.misses).then_with(|| {
            a.accuracy().partial_cmp(&b.accuracy()).unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    rows
}

/// The one thing a run most wants the player to hear. Picked in a fixed
/// order — most specific advice first — from evidence that clears the
/// minimum sample sizes above; `Solid` when enough was played and nothing
/// stands out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Observation {
    /// One technique lags well behind plain notes.
    Technique {
        technique: &'static str,
        hits: u32,
        total: u32,
    },
    /// Too many notes never scored at all.
    MissedNotes {
        misses: u32,
        total: u32,
    },
    /// Hits lean late (`late == true`) or early: `share` of them fell on that
    /// side, well outside the on-time band.
    Timing {
        late: bool,
        share: f32,
    },
    /// The right note landed, but with another hole leaking alongside it.
    LeakyAttacks {
        clean: u32,
        total: u32,
    },
    Solid,
}

/// Which side of the target most hits fell on, when that share clears
/// [`LOPSIDED_SHARE`] over at least [`MIN_TIMING_SAMPLES`] hits. `None` for
/// too few hits, or scatter with no lean either way.
pub fn timing_lean(stats: &SongStats) -> Option<(bool, f32)> {
    let total = stats.timing.total();
    if total < MIN_TIMING_SAMPLES {
        return None;
    }
    let late = stats.timing.late() as f32 / total as f32;
    let early = stats.timing.early() as f32 / total as f32;
    if late >= LOPSIDED_SHARE {
        Some((true, late))
    } else if early >= LOPSIDED_SHARE {
        Some((false, early))
    } else {
        None
    }
}

/// Mean timing offset in milliseconds over all hits — positive when the
/// player sounds notes after the target even with the current Input-lag
/// compensation. `None` with nothing to average.
pub fn mean_offset_ms(stats: &SongStats) -> Option<f64> {
    let hits = stats.perfect + stats.good + stats.delayed;
    if hits == 0 {
        return None;
    }
    Some(stats.offset_sum / hits as f64 * 1000.0)
}

/// The Input-lag change (ms, signed) this run's timing justifies, or `None`
/// when the evidence is too thin: fewer than [`MIN_TIMING_SAMPLES`] hits, a
/// mean under [`MIN_LATENCY_ADJUST_MS`], or a distribution that isn't
/// actually lopsided — a wide symmetric scatter can carry a nonzero mean
/// without any offset being the cause.
pub fn latency_suggestion(stats: &SongStats) -> Option<i32> {
    let (late, _) = timing_lean(stats)?;
    let mean = mean_offset_ms(stats)?;
    if mean.abs() < MIN_LATENCY_ADJUST_MS || (mean > 0.0) != late {
        return None;
    }
    Some(mean.round() as i32)
}

pub fn observation(stats: &SongStats) -> Option<Observation> {
    let total = stats.perfect + stats.good + stats.delayed + stats.miss;
    if total < MIN_NOTES {
        return None;
    }

    // A technique that trails plain notes is the most specific thing there
    // is to say. Without enough plain notes to compare against, the
    // absolute rate has to stand on its own.
    let baseline =
        (stats.normal.total() >= MIN_TECHNIQUE_SAMPLES).then(|| stats.normal.accuracy()).flatten();
    let weakest = technique_buckets(stats)
        .into_iter()
        .filter(|(name, s)| {
            *name != "normal" && *name != "clean-attack" && s.total() >= MIN_TECHNIQUE_SAMPLES
        })
        .filter_map(|(name, s)| s.accuracy().map(|a| (name, s, a)))
        .filter(|(_, _, a)| *a < WEAK_TECHNIQUE && baseline.is_none_or(|b| b - *a >= TECHNIQUE_GAP))
        .min_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((technique, s, _)) = weakest {
        return Some(Observation::Technique { technique, hits: s.hits, total: s.total() });
    }

    if stats.miss as f32 / total as f32 >= HIGH_MISS_RATE {
        return Some(Observation::MissedNotes { misses: stats.miss, total });
    }

    if let Some((late, share)) = timing_lean(stats) {
        return Some(Observation::Timing { late, share });
    }

    let clean = stats.clean_attack;
    if clean.total() >= MIN_TECHNIQUE_SAMPLES
        && clean.accuracy().is_some_and(|a| a < WEAK_TECHNIQUE)
    {
        return Some(Observation::LeakyAttacks { clean: clean.hits, total: clean.total() });
    }

    Some(Observation::Solid)
}

/// How far a lesson run got toward its pass threshold: `(reached, goal)`
/// as fractions, for the criteria a chart run can measure. Jam-judged
/// criteria (scale adherence and friends) never come through the results
/// screen, so they report nothing here rather than a made-up zero.
pub fn lesson_progress(
    criteria: Option<&PassCriteria>,
    accuracy: f32,
    technique_accuracy: &[(&str, f32)],
) -> Option<(f32, f32)> {
    match criteria? {
        PassCriteria::Accuracy { threshold } => Some((accuracy, *threshold)),
        PassCriteria::Technique { technique, threshold } => {
            let reached = technique_accuracy
                .iter()
                .find(|(name, _)| name == technique)
                .map_or(0.0, |(_, a)| *a);
            Some((reached, *threshold))
        }
        PassCriteria::ScaleAdherence { .. }
        | PassCriteria::ChordToneAdherence { .. }
        | PassCriteria::PhraseDiscipline { .. } => None,
    }
}

/// The range to loop for "Practice missed section": the `window_secs`-long
/// stretch holding the most missed notes (earliest on a tie), padded by
/// `lead_in_secs` on both sides and floored at zero. `misses` is each missed
/// note's `(start, end)`, sorted by start. `None` with nothing missed.
pub fn missed_range(
    misses: &[(f64, f64)],
    window_secs: f64,
    lead_in_secs: f64,
) -> Option<(f64, f64)> {
    let mut best: Option<(usize, usize)> = None;
    for (i, &(start, _)) in misses.iter().enumerate() {
        let end_idx = misses[i..].partition_point(|&(t, _)| t < start + window_secs) + i;
        let count = end_idx - i;
        if best.is_none_or(|(bi, be)| count > be - bi) {
            best = Some((i, end_idx));
        }
    }
    let (first, end_idx) = best?;
    let last_end =
        misses[first..end_idx].iter().map(|&(_, end)| end).fold(f64::NEG_INFINITY, f64::max);
    Some(((misses[first].0 - lead_in_secs).max(0.0), last_end + lead_in_secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::state::{TIMING_BUCKETS, TimingHistogram};

    fn stats(perfect: u32, good: u32, delayed: u32, miss: u32) -> SongStats {
        let mut s = SongStats { perfect, good, delayed, miss, ..Default::default() };
        // Plain notes by default, so a technique test has to opt in.
        s.normal = TechniqueStats { hits: perfect + good + delayed, misses: miss };
        s
    }

    fn with_hits(stats: &mut SongStats, offsets_ms: &[f64]) {
        for &ms in offsets_ms {
            stats.offset_sum += ms / 1000.0;
            stats.timing.record(ms / 1000.0);
        }
    }

    // ── TimingHistogram ────────────────────────────────────────────────────

    #[test]
    fn histogram_centre_bucket_straddles_zero() {
        let centre = TIMING_BUCKETS / 2;
        assert_eq!(TimingHistogram::bucket(0.0), centre);
        assert_eq!(TimingHistogram::bucket(-0.009), centre);
        assert_eq!(TimingHistogram::bucket(0.009), centre);
        assert_eq!(TimingHistogram::bucket(0.011), centre + 1);
        assert_eq!(TimingHistogram::bucket(-0.011), centre - 1);
    }

    #[test]
    fn histogram_clamps_outliers_into_the_end_buckets() {
        assert_eq!(TimingHistogram::bucket(-5.0), 0);
        assert_eq!(TimingHistogram::bucket(5.0), TIMING_BUCKETS - 1);
    }

    #[test]
    fn histogram_three_way_split_covers_every_hit() {
        let mut h = TimingHistogram::default();
        for ms in [-80.0, -35.0, -29.0, 0.0, 29.0, 31.0, 90.0] {
            h.record(ms / 1000.0);
        }
        assert_eq!(h.early(), 2);
        assert_eq!(h.on_time(), 3);
        assert_eq!(h.late(), 2);
        assert_eq!(h.early() + h.on_time() + h.late(), h.total());
    }

    // ── timing ─────────────────────────────────────────────────────────────

    #[test]
    fn no_hits_yields_no_mean() {
        assert_eq!(mean_offset_ms(&stats(0, 0, 0, 5)), None);
    }

    #[test]
    fn misses_do_not_dilute_the_mean() {
        let mut s = stats(4, 0, 0, 6);
        with_hits(&mut s, &[40.0; 4]);
        assert!((mean_offset_ms(&s).unwrap() - 40.0).abs() < 1e-4);
    }

    #[test]
    fn too_few_hits_is_no_timing_evidence() {
        let mut s = stats(7, 0, 0, 0);
        with_hits(&mut s, &[60.0; 7]);
        assert_eq!(timing_lean(&s), None);
        assert_eq!(latency_suggestion(&s), None);
    }

    #[test]
    fn consistently_late_hits_suggest_the_mean_as_the_adjustment() {
        let mut s = stats(10, 0, 0, 0);
        with_hits(&mut s, &[50.0; 10]);
        assert_eq!(timing_lean(&s), Some((true, 1.0)));
        assert_eq!(latency_suggestion(&s), Some(50));
    }

    #[test]
    fn consistently_early_hits_suggest_a_negative_adjustment() {
        let mut s = stats(10, 0, 0, 0);
        with_hits(&mut s, &[-40.0; 10]);
        assert_eq!(timing_lean(&s), Some((false, 1.0)));
        assert_eq!(latency_suggestion(&s), Some(-40));
    }

    #[test]
    fn a_wide_symmetric_scatter_earns_no_adjustment() {
        // Mean is +5 ms, but the hits are all over the place: nothing an
        // Input-lag change would fix.
        let mut s = stats(10, 0, 0, 0);
        with_hits(&mut s, &[-80.0, -60.0, -40.0, -40.0, 0.0, 10.0, 50.0, 60.0, 70.0, 80.0]);
        assert_eq!(timing_lean(&s), None);
        assert_eq!(latency_suggestion(&s), None);
    }

    #[test]
    fn a_tiny_mean_is_not_worth_a_click() {
        let mut s = stats(10, 0, 0, 0);
        // Lopsided late, but only just.
        with_hits(&mut s, &[31.0, 31.0, 31.0, 31.0, 31.0, 31.0, -60.0, -60.0, -60.0, 4.0]);
        assert_eq!(timing_lean(&s), Some((true, 0.6)));
        assert_eq!(latency_suggestion(&s), None);
    }

    // ── observation ────────────────────────────────────────────────────────

    #[test]
    fn too_few_notes_is_no_observation() {
        assert_eq!(observation(&stats(3, 0, 0, 4)), None);
    }

    #[test]
    fn a_clean_run_reads_as_solid() {
        let mut s = stats(20, 0, 0, 0);
        with_hits(&mut s, &[0.0; 20]);
        assert_eq!(observation(&s), Some(Observation::Solid));
    }

    #[test]
    fn one_attempted_bend_never_becomes_advice() {
        let mut s = stats(20, 0, 0, 1);
        with_hits(&mut s, &[0.0; 20]);
        s.bend = TechniqueStats { hits: 0, misses: 1 };
        assert_eq!(observation(&s), Some(Observation::Solid));
    }

    #[test]
    fn a_weak_technique_beats_every_other_observation() {
        let mut s = stats(20, 0, 0, 4);
        with_hits(&mut s, &[60.0; 20]); // also consistently late
        s.bend = TechniqueStats { hits: 1, misses: 4 };
        assert_eq!(
            observation(&s),
            Some(Observation::Technique { technique: "bend", hits: 1, total: 5 })
        );
    }

    #[test]
    fn the_weakest_qualifying_technique_is_the_one_named() {
        let mut s = stats(20, 0, 0, 6);
        s.bend = TechniqueStats { hits: 2, misses: 3 };
        s.overblow = TechniqueStats { hits: 1, misses: 4 };
        assert!(matches!(
            observation(&s),
            Some(Observation::Technique { technique: "overblow", .. })
        ));
    }

    #[test]
    fn a_technique_no_worse_than_plain_notes_is_not_singled_out() {
        // Everything lands at ~50%: the problem is hitting notes, not bends.
        let mut s = stats(10, 0, 0, 10);
        s.bend = TechniqueStats { hits: 2, misses: 3 };
        assert_eq!(observation(&s), Some(Observation::MissedNotes { misses: 10, total: 20 }));
    }

    #[test]
    fn without_a_plain_baseline_a_weak_technique_stands_on_its_own() {
        let mut s = stats(3, 0, 0, 6);
        s.normal = TechniqueStats::default();
        s.bend = TechniqueStats { hits: 3, misses: 6 };
        assert!(matches!(observation(&s), Some(Observation::Technique { technique: "bend", .. })));
    }

    #[test]
    fn late_timing_is_reported_when_notes_otherwise_land() {
        let mut s = stats(10, 0, 0, 1);
        with_hits(&mut s, &[50.0; 10]);
        assert_eq!(observation(&s), Some(Observation::Timing { late: true, share: 1.0 }));
    }

    #[test]
    fn leaky_attacks_are_the_last_resort_observation() {
        let mut s = stats(10, 0, 0, 0);
        with_hits(&mut s, &[0.0; 10]);
        s.clean_attack = TechniqueStats { hits: 3, misses: 7 };
        assert_eq!(observation(&s), Some(Observation::LeakyAttacks { clean: 3, total: 10 }));
    }

    // ── ranked_techniques ──────────────────────────────────────────────────

    #[test]
    fn techniques_rank_by_misses_then_accuracy_then_display_order() {
        let s = SongStats {
            normal: TechniqueStats { hits: 18, misses: 2 },
            bend: TechniqueStats { hits: 2, misses: 2 },
            vibrato: TechniqueStats { hits: 6, misses: 2 },
            wah: TechniqueStats { hits: 1, misses: 4 },
            ..Default::default()
        };
        let names: Vec<_> = ranked_techniques(&s).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["wah-wah", "bend", "vibrato", "normal"]);
    }

    #[test]
    fn unused_techniques_are_not_listed() {
        let s = SongStats { normal: TechniqueStats { hits: 5, misses: 0 }, ..Default::default() };
        assert_eq!(ranked_techniques(&s).len(), 1);
    }

    // ── lesson_progress ────────────────────────────────────────────────────

    #[test]
    fn lesson_progress_reports_the_measured_criterion() {
        let acc = Some(PassCriteria::Accuracy { threshold: 0.7 });
        assert_eq!(lesson_progress(acc.as_ref(), 0.55, &[]), Some((0.55, 0.7)));

        let tech = Some(PassCriteria::Technique { technique: "bend".into(), threshold: 0.8 });
        assert_eq!(lesson_progress(tech.as_ref(), 0.9, &[("bend", 0.5)]), Some((0.5, 0.8)));
        // A technique the run never exercised counts as zero progress.
        assert_eq!(lesson_progress(tech.as_ref(), 0.9, &[]), Some((0.0, 0.8)));
    }

    #[test]
    fn jam_judged_criteria_have_no_chart_progress() {
        let jam = Some(PassCriteria::ScaleAdherence { threshold: 0.7 });
        assert_eq!(lesson_progress(jam.as_ref(), 0.9, &[]), None);
        assert_eq!(lesson_progress(None, 0.9, &[]), None);
    }

    // ── missed_range ───────────────────────────────────────────────────────

    #[test]
    fn no_misses_means_no_range() {
        assert_eq!(missed_range(&[], 4.0, 0.5), None);
    }

    #[test]
    fn the_densest_window_wins() {
        // One miss at 2 s, three clustered at 20–22 s, one at 40 s.
        let misses = [(2.0, 2.5), (20.0, 20.5), (21.0, 21.5), (22.0, 22.5), (40.0, 40.5)];
        assert_eq!(missed_range(&misses, 4.0, 1.0), Some((19.0, 23.5)));
    }

    #[test]
    fn a_tie_goes_to_the_earliest_window() {
        let misses = [(2.0, 2.5), (10.0, 10.5)];
        assert_eq!(missed_range(&misses, 4.0, 0.5), Some((1.5, 3.0)));
    }

    #[test]
    fn lead_in_is_floored_at_the_song_start() {
        assert_eq!(missed_range(&[(0.2, 0.7)], 4.0, 1.0), Some((0.0, 1.7)));
    }

    #[test]
    fn the_window_is_measured_from_note_start() {
        // 5.9 s apart: inside a 6 s window, outside a 4 s one.
        let misses = [(0.0, 0.5), (5.9, 6.4)];
        assert_eq!(missed_range(&misses, 6.0, 0.0), Some((0.0, 6.4)));
        assert_eq!(missed_range(&misses, 4.0, 0.0), Some((0.0, 0.5)));
    }
}
