// SPDX-License-Identifier: MIT

//! Suggested fingering for the piano visualizer's middle-C hand split.
//! Rank comfortable spans, stable hand positions and thumb crossings, following
//! Baylor Piano Basics and Parncutt et al.'s ergonomic fingering model.

use harmonicon_core::instrument::{PlayPosition, piano::is_black};
use std::collections::BTreeMap;

use super::ScheduledNote;

pub(super) const MIDDLE_C: u8 = 60;

struct Attack {
    at: f64,
    keys: Vec<u8>,
    held: Vec<u8>,
    starting: Vec<(usize, u8)>,
}

struct Fingering {
    fingers: Vec<u8>,
    cost: f32,
    previous: usize,
}

pub(super) fn suggest(notes: &mut [ScheduledNote]) {
    for left in [true, false] {
        let mut sounding = BTreeMap::<u8, f64>::new();
        let mut attacks = Vec::new();
        for (index, note) in notes.iter().enumerate() {
            let PlayPosition::Key(key) = note.position else { continue };
            if !note.playable || (key < MIDDLE_C) != left {
                continue;
            }
            if attacks.last().is_none_or(|a: &Attack| a.at != note.time) {
                sounding.retain(|_, end| *end > note.time + 1e-6);
                let held: Vec<_> = sounding.keys().copied().collect();
                attacks.push(Attack {
                    at: note.time,
                    keys: held.clone(),
                    held,
                    starting: Vec::new(),
                });
            }
            let attack = attacks.last_mut().unwrap();
            attack.starting.push((index, key));
            if !attack.keys.contains(&key) {
                attack.keys.push(key);
            }
            let end = sounding.entry(key).or_default();
            *end = end.max(note.time + note.duration);
        }
        for attack in &mut attacks {
            attack.keys.sort_unstable_by_key(|&key| if left { u8::MAX - key } else { key });
        }
        plan(notes, &attacks, left);
    }
}

fn plan(notes: &mut [ScheduledNote], attacks: &[Attack], left: bool) {
    let mut phrase: Vec<(&Attack, Vec<Fingering>)> = Vec::new();
    for attack in attacks {
        if attack.held.is_empty()
            && phrase.last().is_some_and(|(previous, _)| {
                previous.starting.iter().all(|&(index, _)| {
                    attack.at - (notes[index].time + notes[index].duration) >= 0.6
                })
            })
        {
            write_phrase(notes, &mut phrase);
        }
        let choices = candidates(attack, phrase.last(), left);
        if choices.is_empty() {
            write_phrase(notes, &mut phrase);
            continue;
        }
        // After an overfull or overstretched chord, wait for the hand to clear.
        if phrase.is_empty() && !attack.held.is_empty() {
            continue;
        }
        phrase.push((attack, choices));
    }
    write_phrase(notes, &mut phrase);
}

fn candidates(
    attack: &Attack,
    previous: Option<&(&Attack, Vec<Fingering>)>,
    left: bool,
) -> Vec<Fingering> {
    let keys = &attack.keys;
    if keys.len() > 5 || keys[0].abs_diff(*keys.last().unwrap()) > 12 {
        return Vec::new();
    }

    // Fingers always fan out from thumb to little finger in a chord. Left
    // hand keys were reversed, so the same combinations work for both hands.
    (1u8..32)
        .filter(|mask| mask.count_ones() as usize == keys.len())
        .filter_map(|mask| {
            let fingers: Vec<_> =
                (1..=5).filter(|finger| mask & (1 << (finger - 1)) != 0).collect();
            let shape = shape_cost(keys, &fingers);
            let (previous, cost) = if let Some((last, states)) = previous {
                states
                    .iter()
                    .enumerate()
                    .filter_map(|(index, state)| {
                        transition(last, &state.fingers, attack, &fingers, left)
                            .map(|movement| (index, state.cost + movement))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))?
            } else {
                (0, 0.0)
            };
            Some(Fingering { fingers, cost: cost + shape, previous })
        })
        .collect()
}

fn write_phrase(notes: &mut [ScheduledNote], phrase: &mut Vec<(&Attack, Vec<Fingering>)>) {
    let Some((_, choices)) = phrase.last() else { return };
    let (mut index, _) =
        choices.iter().enumerate().min_by(|a, b| a.1.cost.total_cmp(&b.1.cost)).unwrap();
    for (attack, choices) in phrase.drain(..).rev() {
        let choice = &choices[index];
        for &(note, key) in &attack.starting {
            notes[note].suggested_piano_finger = finger_for(attack, &choice.fingers, key);
        }
        index = choice.previous;
    }
}

fn finger_for(attack: &Attack, fingers: &[u8], key: u8) -> Option<u8> {
    attack.keys.iter().position(|&pitch| pitch == key).map(|index| fingers[index])
}

// Distance in white-key widths, with accidentals between their neighbours.
fn key_x(key: u8) -> f32 {
    const IN_OCTAVE: [f32; 12] = [0., 0.5, 1., 1.5, 2., 3., 3.5, 4., 4.5, 5., 5.5, 6.];
    f32::from(key / 12) * 7.0 + IN_OCTAVE[usize::from(key % 12)]
}

fn shape_cost(keys: &[u8], fingers: &[u8]) -> f32 {
    let reach: f32 = keys
        .windows(2)
        .zip(fingers.windows(2))
        .map(|(key, finger)| {
            let span = (key_x(key[1]) - key_x(key[0])).abs();
            let gap = f32::from(finger[1] - finger[0]);
            // The thumb opens farther than neighbouring fingers can.
            let comfort = gap + if finger[0] == 1 { 1.0 } else { 0.25 };
            0.2 * (span - gap).abs() + (span - comfort).max(0.0).powi(2)
        })
        .sum();
    let accidentals: f32 = keys
        .iter()
        .zip(fingers)
        .map(|(&key, &finger)| {
            if is_black(key) && finger == 1 {
                1.6
            } else if is_black(key) && finger == 5 {
                0.5
            } else {
                0.0
            }
        })
        .sum();
    reach + accidentals
}

fn transition(
    last: &Attack,
    before: &[u8],
    next: &Attack,
    after: &[u8],
    left: bool,
) -> Option<f32> {
    for &key in &next.held {
        if finger_for(last, before, key) != finger_for(next, after, key) {
            return None;
        }
    }
    let travel = |old_key, old_finger, new_key, new_finger| {
        let mut step = key_x(new_key) - key_x(old_key);
        if left {
            step = -step;
        }
        let fingers = f32::from(new_finger) - f32::from(old_finger);
        if step == 0.0 {
            return if fingers == 0.0 { 0.0 } else { 1.8 };
        }
        if step * fingers < 0.0 && step.abs() <= 2.0 {
            // Passing the thumb accommodates a scale without a large hand shift.
            if (step > 0.0 && new_finger == 1) || (step < 0.0 && old_finger == 1) {
                return 1.35 + 0.25 * (fingers.abs() - 2.0).abs();
            }
        }
        let shift = (step - fingers).abs();
        0.65 * shift.min(8.0) + if fingers == 0.0 { 0.55 } else { 0.0 }
    };
    if last.keys == next.keys && before == after {
        return Some(0.0);
    }
    // A chord can share any stationary finger with the next shape. For
    // single notes this is exactly their finger-to-finger move.
    last.keys
        .iter()
        .zip(before)
        .flat_map(|(&old_key, &old_finger)| {
            next.keys.iter().zip(after).map(move |(&new_key, &new_finger)| {
                travel(old_key, old_finger, new_key, new_finger)
            })
        })
        .min_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(key: u8, time: f64, duration: f64) -> ScheduledNote {
        ScheduledNote {
            position: PlayPosition::Key(key),
            time,
            duration,
            ..ScheduledNote::default()
        }
    }

    #[test]
    fn closed_triads_use_thumb_middle_and_little_in_each_hand() {
        let mut notes: Vec<_> = [48, 52, 55, 60, 64, 67].map(|key| note(key, 0.0, 1.0)).into();
        suggest(&mut notes);
        assert_eq!(
            notes.iter().map(|note| note.suggested_piano_finger).collect::<Vec<_>>(),
            [5, 3, 1, 1, 3, 5].map(Some),
        );
    }

    #[test]
    fn sustained_keys_keep_their_fingers_free_of_new_attacks() {
        let mut notes =
            [note(60, 0.0, 2.0), note(62, 0.5, 0.5), note(64, 1.0, 0.5), note(65, 1.5, 0.5)];
        suggest(&mut notes);
        let digits = notes.map(|note| note.suggested_piano_finger.unwrap());
        assert!(digits[1..].iter().all(|&finger| finger > digits[0]));
    }

    #[test]
    fn the_right_hand_c_major_scale_passes_the_thumb_after_e() {
        let mut notes = [60, 62, 64, 65, 67, 69, 71, 72].map(|key| note(key, 0.0, 0.5));
        for (index, note) in notes.iter_mut().enumerate() {
            note.time = index as f64 * 0.5;
        }
        suggest(&mut notes);
        assert_eq!(
            notes.map(|note| note.suggested_piano_finger),
            [1, 2, 3, 1, 2, 3, 4, 5].map(Some)
        );
    }

    #[test]
    fn does_not_suggest_a_finger_for_each_key_of_an_impossible_chord() {
        let mut notes = [60, 62, 64, 65, 67, 69].map(|key| note(key, 0.0, 1.0));
        suggest(&mut notes);
        assert!(notes.iter().all(|note| note.suggested_piano_finger.is_none()));
    }
}
