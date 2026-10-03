// SPDX-License-Identifier: MIT

use super::*;

fn c_diatonic_chart(track_json: &str) -> HarpChart {
    serde_json::from_str(&format!(
        r#"{{
            "song": {{"title":"T","artist":"A","genre":"Test","tempo_bpm":120.0,"key":"C","difficulty":"easy"}},
            "timing": {{"resolution":480,"tempo_map":[{{"tick":0,"bpm":120.0}}]}},
            "harmonica": {{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
                "layout": {{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                           "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}}},
            "track": {track_json},
            "scoring": {{"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}}
        }}"#
    ))
    .unwrap()
}

/// Three notes that are all plain blow/draw reeds on a C harp.
fn plain_chart() -> HarpChart {
    c_diatonic_chart(
        r#"[
            {"time":0.0,"duration":0.5,"call":false,"phrase":"p1",
             "events":[{"hole":1,"action":"blow","note":"C4"}]},
            {"time":0.5,"duration":0.5,"call":false,"phrase":"p1",
             "events":[{"hole":4,"action":"blow","note":"C5"}]},
            {"time":1.0,"duration":0.5,"call":false,"phrase":"p1",
             "events":[{"hole":2,"action":"draw","note":"G4"}]}
        ]"#,
    )
}

fn choice(key: &str, mapping: HarpMapping) -> HarpChoice {
    HarpChoice { key: key.to_string(), kind: HarpKind::Diatonic, mapping, seeded: true }
}

#[test]
fn the_charts_own_harp_costs_nothing() {
    // Opening the page must report "no change", or every player is nudged
    // toward thinking something is wrong with playing it as written.
    let chart = plain_chart();
    let cost = cost_of(&chart, &choice("C", HarpMapping::SameHoles));
    assert_eq!(cost.total, 3);
    assert!(cost.is_complete());
    assert_eq!((cost.bends, cost.overblows), (0, 0));
}

#[test]
fn same_holes_on_any_harp_costs_nothing_either() {
    // Keeping the tab can't demand a new technique by construction — the
    // holes and breath are unchanged, only the pitch moves. This is the
    // property that makes "same holes" the safe default.
    let chart = plain_chart();
    for key in HARP_KEYS {
        let cost = cost_of(&chart, &choice(key, HarpMapping::SameHoles));
        assert!(
            cost.is_complete() && cost.bends == 0 && cost.overblows == 0,
            "{key} harp, same holes, reported a cost: {cost:?}"
        );
    }
}

#[test]
fn transposing_to_a_distant_harp_reports_a_real_cost() {
    // The case the readout exists for: the same three notes on a harp a
    // tritone away are no longer all plain reeds.
    let chart = plain_chart();
    let cost = cost_of(&chart, &choice("F#", HarpMapping::Transpose));
    assert_eq!(cost.total, 3);
    assert!(
        cost.bends + cost.overblows + cost.unplayable > 0,
        "expected transposing to F# to demand something, got {cost:?}"
    );
}

#[test]
fn a_choice_matching_the_chart_is_recognised_as_no_change() {
    // What keeps gameplay on its original, untouched path.
    let chart = plain_chart();
    assert!(choice("C", HarpMapping::SameHoles).matches_chart(&chart));
    assert!(!choice("G", HarpMapping::SameHoles).matches_chart(&chart));
}

#[test]
fn a_chromatic_choice_never_matches_a_diatonic_chart() {
    let chart = plain_chart();
    let chromatic = HarpChoice {
        key: "C".to_string(),
        kind: HarpKind::Chromatic,
        mapping: HarpMapping::SameHoles,
        seeded: true,
    };
    assert!(
        !chromatic.matches_chart(&chart),
        "same key but a different instrument is still a substitution"
    );
}

#[test]
fn seeding_points_the_choice_at_the_charts_own_harmonica() {
    let chart = plain_chart();
    let mut c = HarpChoice::default();
    assert!(!c.seeded);
    seed_choice(&mut c, &chart);
    assert!(c.seeded);
    assert_eq!(c.key, "C");
    assert_eq!(c.kind, HarpKind::Diatonic);
    assert!(c.matches_chart(&chart));
}

#[test]
fn an_empty_chart_reports_no_cost_at_all() {
    // Rather than "0 bends, 0 overblows", which would read as a finding.
    let chart = c_diatonic_chart("[]");
    let cost = cost_of(&chart, &choice("G", HarpMapping::Transpose));
    assert_eq!(cost.total, 0);
}

// ── The readout is relative, not absolute ───────────────────────────────────

/// A chart whose notes already need bends — a blues lick, in other words.
fn bendy_chart() -> HarpChart {
    c_diatonic_chart(
        r#"[
            {"time":0.0,"duration":0.5,"call":false,"phrase":"p1",
             "events":[{"hole":3,"action":"draw","note":"B4",
                        "modifiers":[{"type":"bend","semitones":-1.0}]}]},
            {"time":0.5,"duration":0.5,"call":false,"phrase":"p1",
             "events":[{"hole":1,"action":"blow","note":"C4"}]}
        ]"#,
    )
}

#[test]
fn the_charts_own_harp_reports_nothing_even_when_the_music_has_bends() {
    // The first version of this readout said "1 note needs a bend" for a
    // chart sitting on its own harmonica — reporting the music as if it
    // were the cost of a choice nobody had made.
    let chart = bendy_chart();
    let baseline = cost_of(&chart, &baseline_choice(&chart));
    assert_eq!(baseline.bends, 1, "the chart itself asks for one bend");

    let loc_free = cost_message_parts(&baseline, &baseline);
    assert!(
        loc_free.is_empty(),
        "playing as written must report no added technique, got {loc_free:?}"
    );
}

/// The message's *decision*, without needing a Localization: which of the
/// added-technique clauses would appear.
fn cost_message_parts(cost: &RemapCost, baseline: &RemapCost) -> Vec<&'static str> {
    let mut out = Vec::new();
    if cost.bends.saturating_sub(baseline.bends) > 0 {
        out.push("bends");
    }
    if cost.overblows.saturating_sub(baseline.overblows) > 0 {
        out.push("overblows");
    }
    if cost.unplayable > 0 {
        out.push("unreachable");
    }
    out
}

#[test]
fn only_techniques_the_swap_adds_are_reported() {
    let chart = bendy_chart();
    let baseline = cost_of(&chart, &baseline_choice(&chart));
    // Same holes on any harp keeps the chart's own bends and adds none.
    for key in HARP_KEYS {
        let cost = cost_of(&chart, &choice(key, HarpMapping::SameHoles));
        assert!(
            cost_message_parts(&cost, &baseline).is_empty(),
            "{key}, same holes, reported an added cost"
        );
    }
}
