// SPDX-License-Identifier: MIT

//! Choosing where a whole part is played on the neck.
//!
//! Every pitch on a guitar lives in several places, and picking each note's
//! position on its own (the lowest fret, say) makes a melody played around
//! the fifth fret jump between open strings and high frets note by note. A
//! player keeps their hand in one place and shifts as rarely and as little
//! as the music allows; [`fingering`] picks positions the same way.
//!
//! It is a shortest path over the part's onsets. At each onset the pitches
//! sounding together are given every legal voicing (one string each, all
//! holdable at once); moving from one voicing to the next costs how far the
//! hand shifts, plus a small preference for the lower frets beginners know.
//! A note the source already placed (a tab) keeps its place.

use super::{FretAssignment, Guitar, StringTechnique};

/// The pitches sounding at one onset, each optionally already placed.
#[derive(Clone, Debug, PartialEq)]
pub struct Onset {
    pub notes: Vec<(u8, Option<FretAssignment>)>,
}

/// Voicings kept per onset. A six-note chord has at most a few hundred legal
/// voicings; keeping the cheapest few bounds the search on dense parts
/// without losing any a player would choose.
const MAX_VOICINGS: usize = 48;

/// What one fret of hand shift costs, against [`HEIGHT_COST`] per fret of
/// height: shifting is the thing to avoid, height only breaks ties.
const SHIFT_COST: f32 = 1.0;
const HEIGHT_COST: f32 = 0.05;

/// Where every onset's notes are played, in the same order, or `None` for
/// an onset with a pitch the neck can't reach or a chord no hand can hold.
/// An unplayable onset doesn't break the path: the hand simply keeps its
/// place across it.
pub fn fingering(guitar: &Guitar, onsets: &[Onset]) -> Vec<Option<Vec<FretAssignment>>> {
    let voicings: Vec<Vec<Vec<FretAssignment>>> =
        onsets.iter().map(|onset| voicings(guitar, onset)).collect();

    // Viterbi: per onset, per voicing, the cheapest cost to reach it and
    // where it came from. `hand` carries the hand's place through onsets
    // whose voicing doesn't fix one (all open strings), or that are
    // unplayable.
    #[derive(Clone, Copy)]
    struct Step {
        cost: f32,
        hand: Option<f32>,
        from: Option<usize>,
    }
    let mut steps: Vec<Vec<Step>> = Vec::with_capacity(onsets.len());
    let mut previous: Vec<Step> = vec![Step { cost: 0.0, hand: None, from: None }];
    for options in &voicings {
        if options.is_empty() {
            // Carry every path through unchanged.
            let carried: Vec<Step> = previous
                .iter()
                .enumerate()
                .map(|(i, p)| Step { cost: p.cost, hand: p.hand, from: Some(i) })
                .collect();
            previous.clone_from(&carried);
            steps.push(carried);
            continue;
        }
        let current: Vec<Step> = options
            .iter()
            .map(|voicing| {
                let here = hand_position(voicing);
                let height = voicing.iter().map(|a| f32::from(a.fret)).sum::<f32>();
                let (from, cost, hand) = previous
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let shift = match (p.hand, here) {
                            (Some(a), Some(b)) => (a - b).abs(),
                            _ => 0.0,
                        };
                        (i, p.cost + SHIFT_COST * shift + HEIGHT_COST * height, here.or(p.hand))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .expect("there is always a previous step");
                Step { cost, hand, from: Some(from) }
            })
            .collect();
        previous.clone_from(&current);
        steps.push(current);
    }

    // Walk back from the cheapest end.
    let mut chosen = vec![None; onsets.len()];
    let Some(mut at) = steps
        .last()
        .and_then(|last| (0..last.len()).min_by(|&a, &b| last[a].cost.total_cmp(&last[b].cost)))
    else {
        return chosen;
    };
    for index in (0..onsets.len()).rev() {
        let step = &steps[index][at];
        if !voicings[index].is_empty() {
            chosen[index] = Some(voicings[index][at].clone());
        }
        at = step.from.unwrap_or(0);
    }
    chosen
}

/// Where the fretting hand sits for `voicing`: the lowest fret it holds
/// down. `None` when it holds none (open strings, harmonics), which leaves
/// the hand wherever it was.
fn hand_position(voicing: &[FretAssignment]) -> Option<f32> {
    voicing
        .iter()
        .filter(|a| a.fret > 0 && a.technique == StringTechnique::Fretted)
        .map(|a| f32::from(a.fret))
        .min_by(f32::total_cmp)
}

/// Every legal voicing of `onset`, cheapest-looking (lowest) first, at most
/// [`MAX_VOICINGS`]. A note already placed is fixed there if its place
/// sounds its pitch; otherwise it is placed like any other.
fn voicings(guitar: &Guitar, onset: &Onset) -> Vec<Vec<FretAssignment>> {
    let choices: Vec<Vec<FretAssignment>> = onset
        .notes
        .iter()
        .map(|&(pitch, placed)| match placed.filter(|a| guitar.pitch_of(a) == Some(pitch)) {
            Some(fixed) => vec![fixed],
            None => guitar.fretted_positions(pitch),
        })
        .collect();
    if choices.iter().any(Vec::is_empty) {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut partial = Vec::with_capacity(choices.len());
    extend(guitar, &choices, &mut partial, &mut found);
    found.sort_by_key(|v: &Vec<FretAssignment>| v.iter().map(|a| u32::from(a.fret)).sum::<u32>());
    found.truncate(MAX_VOICINGS);
    found
}

/// Depth-first: give the next note each position that can be held with
/// every note already placed.
fn extend(
    guitar: &Guitar,
    choices: &[Vec<FretAssignment>],
    partial: &mut Vec<FretAssignment>,
    found: &mut Vec<Vec<FretAssignment>>,
) {
    let Some(options) = choices.get(partial.len()) else {
        found.push(partial.clone());
        return;
    };
    for option in options {
        if partial.iter().all(|held| guitar.can_hold_together(held, option)) {
            partial.push(*option);
            extend(guitar, choices, partial, found);
            partial.pop();
        }
    }
}

#[cfg(test)]
mod tests;
