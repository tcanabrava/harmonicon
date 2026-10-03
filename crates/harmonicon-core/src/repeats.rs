// SPDX-License-Identifier: MIT

//! Plays a chart's repeat signs out into one flat, linear track.
//!
//! A chart stores the score as written — `timing.repeats`, each with its
//! first/second-time endings — because that is what an author edits and what
//! the Song Editor shows. Everything that *plays* a chart wants the opposite:
//! one note after another in performance order, so the clock, judging,
//! highways and progress bar need no notion of jumping back. [`expand`] is
//! the one bridge between the two, run by the song loader.
//!
//! The expansion is a list of [`Span`]s — "play written ticks `a..b`, from
//! performed tick `t`" — and everything tick-positioned (track items, tempo
//! changes, meter changes) is copied through the same spans, so a tempo
//! change inside a repeated passage happens on every pass.

use crate::chart::{
    HarpChart, Repeat, TempoPoint, TimeSigPoint, TrackItem, seconds_to_tick, time_sig_at_tick,
};

/// The most passes a repeat may ask for; more is clamped.
pub const MAX_PASSES: u32 = 8;

/// One stretch of the written score, played from `performed` onwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub written_start: u64,
    /// Exclusive; `u64::MAX` for the final span, which runs to the end.
    pub written_end: u64,
    pub performed: u64,
}

impl Span {
    fn contains(&self, tick: u64) -> bool {
        (self.written_start..self.written_end).contains(&tick)
    }

    fn perform(&self, tick: u64) -> u64 {
        tick - self.written_start + self.performed
    }
}

/// The repeats that can actually be played, in order: a repeat must span
/// something, and one starting inside the previous is dropped — nesting
/// isn't expressible in this model, and guessing an order would be worse
/// than playing the passage once. Endings outside `start..` or ending
/// before they start are dropped the same way.
fn playable(repeats: &[Repeat]) -> Vec<Repeat> {
    let mut sorted: Vec<Repeat> =
        repeats.iter().filter(|r| r.start_tick < r.end_tick).cloned().collect();
    sorted.sort_by_key(|r| r.start_tick);
    let mut kept: Vec<Repeat> = Vec::with_capacity(sorted.len());
    for mut repeat in sorted {
        if kept.last().is_some_and(|prev| repeat.start_tick < prev.end_tick) {
            continue;
        }
        let (start, end) = (repeat.start_tick, repeat.end_tick);
        repeat.endings.retain(|e| {
            e.start_tick < e.end_tick
                && e.start_tick >= start
                // Inside the passage, or wholly after its repeat sign.
                && (e.end_tick <= end || e.start_tick >= end)
        });
        repeat.endings.sort_by_key(|e| e.start_tick);
        kept.push(repeat);
    }
    // An ending after one repeat's sign must not run into the next repeat.
    for i in 1..kept.len() {
        let next_start = kept[i].start_tick;
        kept[i - 1].endings.retain(|e| e.end_tick <= next_start);
    }
    kept
}

/// The performance order of a score with `repeats`, as spans of written
/// ticks. With no repeats this is one span, identity from tick 0.
pub fn spans(repeats: &[Repeat]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut performed = 0;
    let mut play = |from: u64, to: u64| {
        if to > from {
            spans.push(Span { written_start: from, written_end: to, performed });
            performed = performed.saturating_add(to - from);
        }
    };

    let mut cursor = 0;
    for repeat in playable(repeats) {
        play(cursor, repeat.start_tick);
        let passes = repeat.passes();
        let (inside, after): (Vec<_>, Vec<_>) =
            repeat.endings.iter().partition(|e| e.end_tick <= repeat.end_tick);
        for pass in 1..=passes {
            let mut at = repeat.start_tick;
            for ending in inside.iter().filter(|e| !e.passes.contains(&pass)) {
                play(at, ending.start_tick);
                at = at.max(ending.end_tick);
            }
            play(at, repeat.end_tick);
        }
        cursor = repeat.end_tick;
        // Only the last pass carries on past the sign, so an ending out
        // there is played or skipped on that pass's number alone.
        for ending in after.iter().filter(|e| !e.passes.contains(&passes)) {
            play(cursor, ending.start_tick);
            cursor = cursor.max(ending.end_tick);
        }
    }
    play(cursor, u64::MAX);
    spans
}

/// Copies a tick-positioned map through `spans`: each span opens with
/// whatever was in effect where it starts, followed by the changes inside
/// it. Consecutive entries that change nothing are dropped.
fn expand_map<P: Clone>(
    spans: &[Span],
    points: &[P],
    tick_of: impl Fn(&P) -> u64,
    in_effect: impl Fn(u64) -> Option<P>,
    with_tick: impl Fn(P, u64) -> P,
    same: impl Fn(&P, &P) -> bool,
) -> Vec<P> {
    let mut out: Vec<P> = Vec::new();
    let mut push = |point: P| {
        if out.last().is_some_and(|last| same(last, &point)) {
            return;
        }
        match out.last_mut() {
            Some(last) if tick_of(last) == tick_of(&point) => *last = point,
            _ => out.push(point),
        }
    };
    for span in spans {
        if let Some(opening) = in_effect(span.written_start) {
            push(with_tick(opening, span.performed));
        }
        for point in points {
            let tick = tick_of(point);
            if tick > span.written_start && span.contains(tick) {
                push(with_tick(point.clone(), span.perform(tick)));
            }
        }
    }
    out
}

/// `chart` in performance order: every repeated passage written out once
/// per pass, with endings taken on the passes they name. A chart with no
/// repeats comes back untouched.
///
/// Track items keep their written order within a pass. One positioned by
/// `time` rather than `tick` comes out positioned by `tick`, since a moved
/// note's seconds depend on the tempo map it lands in. The practice loop's
/// item indices point at the first performance of those items.
pub fn expand(mut chart: HarpChart) -> HarpChart {
    if chart.timing.repeats.is_empty() {
        return chart;
    }
    let timing = &chart.timing;
    let spans = spans(&timing.repeats);
    let written_tick = |item: &TrackItem| {
        item.tick.unwrap_or_else(|| {
            seconds_to_tick(item.time.unwrap_or(0.0), timing.resolution, &timing.tempo_map)
        })
    };

    let mut track = Vec::with_capacity(chart.track.len());
    let mut first_performance = vec![None; chart.track.len()];
    for span in &spans {
        for (index, item) in chart.track.iter().enumerate() {
            let tick = written_tick(item);
            if span.contains(tick) {
                first_performance[index].get_or_insert(track.len());
                let mut copy = item.clone();
                copy.tick = Some(span.perform(tick));
                copy.time = None;
                track.push(copy);
            }
        }
    }

    let tempo_map = expand_map(
        &spans,
        &timing.tempo_map,
        |p| p.tick,
        |tick| timing.tempo_map.iter().rev().find(|p| p.tick <= tick).cloned(),
        |p, tick| TempoPoint { tick, ..p },
        |a, b| a.bpm == b.bpm,
    );
    let time_signature_map = timing.time_signature_map.as_ref().map(|map| {
        expand_map(
            &spans,
            map,
            |p| p.tick,
            |tick| {
                time_sig_at_tick(tick, map)
                    .map(|sig| TimeSigPoint { tick, time_signature: sig.to_string() })
            },
            |p, tick| TimeSigPoint { tick, ..p },
            |a, b| a.time_signature == b.time_signature,
        )
    });

    if let Some(section) = chart.loop_section.as_mut() {
        let at = |index: usize| first_performance.get(index).copied().flatten();
        match (at(section.start_index), at(section.end_index)) {
            (Some(start), Some(end)) if start <= end => {
                section.start_index = start;
                section.end_index = end;
            }
            _ => chart.loop_section = None,
        }
    }
    chart.track = track;
    chart.timing.tempo_map = tempo_map;
    chart.timing.time_signature_map = time_signature_map;
    chart.timing.repeats.clear();
    chart
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::Ending;

    fn repeat(start: u64, end: u64) -> Repeat {
        Repeat { start_tick: start, end_tick: end, times: None, endings: Vec::new() }
    }

    fn ending(start: u64, end: u64, passes: &[u32]) -> Ending {
        Ending { start_tick: start, end_tick: end, passes: passes.to_vec() }
    }

    /// `(written_start, written_end)` of every span but the open final one.
    fn played(repeats: &[Repeat]) -> Vec<(u64, u64)> {
        let spans = spans(repeats);
        let (last, rest) = spans.split_last().unwrap();
        assert_eq!(last.written_end, u64::MAX, "the song runs on after");
        rest.iter()
            .map(|s| (s.written_start, s.written_end))
            .chain(std::iter::once((last.written_start, u64::MAX)))
            .collect()
    }

    #[test]
    fn no_repeats_is_the_score_as_written() {
        assert_eq!(played(&[]), vec![(0, u64::MAX)]);
    }

    #[test]
    fn a_plain_repeat_plays_twice_then_carries_on() {
        assert_eq!(
            played(&[repeat(100, 200)]),
            vec![(0, 100), (100, 200), (100, 200), (200, u64::MAX)]
        );
        let spans = spans(&[repeat(100, 200)]);
        assert_eq!(spans[2].performed, 200, "the second pass follows the first");
        assert_eq!(spans[3].performed, 300);
    }

    #[test]
    fn times_sets_the_pass_count_and_is_bounded() {
        let mut thrice = repeat(0, 10);
        thrice.times = Some(3);
        assert_eq!(played(&[thrice.clone()]).len(), 4);
        thrice.times = Some(1000);
        assert_eq!(played(&[thrice.clone()]).len(), MAX_PASSES as usize + 1);
        thrice.times = Some(0);
        assert_eq!(played(&[thrice]), vec![(0, 10), (10, u64::MAX)]);
    }

    #[test]
    fn first_and_second_endings() {
        // Bars 1-4 repeated; bar 4 is the first ending, bar 5 the second.
        let mut r = repeat(0, 400);
        r.endings = vec![ending(300, 400, &[1]), ending(400, 500, &[2])];
        assert_eq!(
            played(&[r]),
            vec![(0, 400), (0, 300), (400, u64::MAX)],
            "pass 1 takes bar 4, pass 2 skips from bar 3 into bar 5"
        );
    }

    #[test]
    fn an_ending_after_the_sign_not_for_the_last_pass_is_skipped() {
        let mut r = repeat(0, 100);
        r.endings = vec![ending(100, 150, &[1])];
        assert_eq!(played(&[r]), vec![(0, 100), (0, 100), (150, u64::MAX)]);
    }

    #[test]
    fn an_ending_can_serve_several_passes() {
        let mut r = repeat(0, 100);
        r.times = Some(3);
        r.endings = vec![ending(80, 100, &[1, 2]), ending(100, 120, &[3])];
        assert_eq!(played(&[r]), vec![(0, 100), (0, 100), (0, 80), (100, u64::MAX)]);
    }

    #[test]
    fn overlapping_and_empty_repeats_are_played_once() {
        assert_eq!(
            played(&[repeat(0, 100), repeat(50, 150), repeat(300, 300)]),
            vec![(0, 100), (0, 100), (100, u64::MAX)]
        );
    }

    #[test]
    fn two_repeats_in_a_row() {
        assert_eq!(
            played(&[repeat(100, 200), repeat(0, 100)]),
            vec![(0, 100), (0, 100), (100, 200), (100, 200), (200, u64::MAX)]
        );
    }

    #[test]
    fn a_malformed_ending_is_ignored() {
        let mut r = repeat(100, 200);
        r.endings = vec![
            ending(150, 250, &[1]), // straddles the sign
            ending(50, 120, &[1]),  // starts before the passage
            ending(180, 180, &[1]), // empty
        ];
        assert_eq!(played(&[r]), vec![(0, 100), (100, 200), (100, 200), (200, u64::MAX)]);
    }

    fn chart(track: &[u64], repeats: Vec<Repeat>) -> HarpChart {
        let mut chart: HarpChart = serde_json::from_str(
            r#"{
                "song": { "title": "T", "artist": "A", "genre": "Test", "tempo_bpm": 120.0,
                          "key": "C", "difficulty": "easy" },
                "timing": { "resolution": 100,
                            "tempo_map": [{"tick": 0, "bpm": 120.0}, {"tick": 150, "bpm": 60.0}],
                            "time_signature_map": [{"tick": 0, "time_signature": "4/4"}] },
                "harmonica": {
                    "type": "diatonic", "holes": 10,
                    "bending_profile": "richter_standard",
                    "layout": {
                        "blow": ["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                        "draw": ["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]
                    }
                },
                "track": [],
                "scoring": { "perfect_window_ms": 50, "good_window_ms": 100,
                             "miss_window_ms": 130 }
            }"#,
        )
        .unwrap();
        chart.track = track
            .iter()
            .map(|&tick| {
                serde_json::from_value(serde_json::json!({
                    "tick": tick, "duration": 0.25,
                    "events": [{ "hole": 4, "action": "blow" }]
                }))
                .unwrap()
            })
            .collect();
        chart.timing.repeats = repeats;
        chart
    }

    fn ticks(chart: &HarpChart) -> Vec<u64> {
        chart.track.iter().map(|i| i.tick.unwrap()).collect()
    }

    #[test]
    fn expanding_writes_every_pass_out() {
        let expanded = expand(chart(&[0, 100, 200, 300], vec![repeat(100, 300)]));
        assert_eq!(ticks(&expanded), vec![0, 100, 200, 300, 400, 500]);
        assert!(expanded.timing.repeats.is_empty(), "nothing left to repeat");
    }

    #[test]
    fn a_tempo_change_inside_the_passage_happens_on_every_pass() {
        let expanded = expand(chart(&[], vec![repeat(100, 300)]));
        let tempo: Vec<(u64, f32)> =
            expanded.timing.tempo_map.iter().map(|p| (p.tick, p.bpm)).collect();
        // Pass 1 starts at 120 and slows at 150; pass 2 restarts at 120.
        assert_eq!(tempo, vec![(0, 120.0), (150, 60.0), (300, 120.0), (350, 60.0)]);
        assert_eq!(
            expanded.timing.time_signature_map.unwrap().len(),
            1,
            "an unchanged meter adds no points"
        );
    }

    #[test]
    fn a_chart_without_repeats_is_untouched() {
        let mut written = chart(&[0, 100], Vec::new());
        written.track[1].tick = None;
        written.track[1].time = Some(0.5);
        let expanded = expand(written);
        assert_eq!(expanded.track[1].time, Some(0.5));
        assert_eq!(expanded.track[1].tick, None);
    }

    #[test]
    fn the_practice_loop_follows_its_first_performance() {
        let mut written = chart(&[0, 100, 200, 300], vec![repeat(0, 200)]);
        written.loop_section = Some(crate::chart::LoopSection {
            start_index: 2,
            end_index: 3,
            section_type: None,
            repeat: None,
        });
        let expanded = expand(written);
        let section = expanded.loop_section.unwrap();
        // 0 100 0 100 200 300: item 2 is played fifth.
        assert_eq!((section.start_index, section.end_index), (4, 5));
    }
}
