// SPDX-License-Identifier: MIT

//! A live visual guide for the harmonica player's own rhythm/attack timing
//! in Jam Session — deliberately not a bass or backing-instrument display:
//! a row of pulsing pips showing *when* the picked `Genre`'s groove wants
//! a breath/attack, so a player can phrase their own playing against it.
//! Reuses `jam::backing::GrooveArrangement`'s 8-slot-per-bar bass rhythm
//! (`Some` = a hit, `None` = a rest) and swing/straight flag as the timing
//! source — the same skeleton the generated bass audio is rendered from,
//! just rendered here as a visual pulse instead of PCM. Only ever spawned
//! for a `GeneratedJamSession` (see `jam::session::setup`); a real song has
//! no genre concept attached to it.

use bevy::prelude::*;

use harmonicon_gameplay::gameplay::GameplayClock;
use harmonicon_gameplay::gameplay::metronome_overlay::MetronomeTempo;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::backing::{Genre, JamGenre, groove_arrangement, groove_arrangement_for_position};

/// Tags one of the 8 pulse-row cells with its slot index (matching
/// `jam::backing::GrooveArrangement::bass`) so [`update_rhythm_guide`]
/// can look up whether it's a hit or a rest, and pulse it accordingly.
#[derive(Component, Default, Clone)]
pub struct RhythmGuideSlot(pub usize);

// ── Pure timing ────────────────────────────────────────────────────────────

/// Which of a bar's 8 pattern slots is active right now, and how far
/// through that slot's decay window (`0.0` = just onset, approaching `1.0`
/// as it's about to hand off to the next slot) — the same swung-vs-straight
/// timing split `jam::backing::generate_bass_pcm` already uses to render
/// the bass audio, applied here to a live clock position within one bar
/// instead of an offline render, so the visual guide and the audio it's
/// modeled on can't drift apart. `bar_pos_secs` must already be the clock's
/// position wrapped into `[0, secs_per_beat * 4.0)` — one 4/4 bar, the same
/// assumption `jam::backing::generated_chart` already hardcodes.
pub(crate) fn active_slot(bar_pos_secs: f64, secs_per_beat: f64, swung: bool) -> (usize, f32) {
    let secs_per_beat = secs_per_beat.max(1e-6);
    let long_secs = if swung {
        secs_per_beat * super::backing::SWING_LONG_FRAC as f64
    } else {
        secs_per_beat * 0.5
    };
    let short_secs = secs_per_beat - long_secs;

    let beat = (bar_pos_secs / secs_per_beat).floor().max(0.0) as usize;
    let pos_in_beat = bar_pos_secs - beat as f64 * secs_per_beat;

    let (slot_in_beat, phase) = if pos_in_beat < long_secs {
        (0, pos_in_beat / long_secs)
    } else {
        (1, (pos_in_beat - long_secs) / short_secs.max(1e-6))
    };
    let slot = (beat * 2 + slot_in_beat).min(7);
    (slot, phase.clamp(0.0, 1.0) as f32)
}

// ── UI ────────────────────────────────────────────────────────────────────────

const REST_BORDER: Color = Color::srgba(0.30, 0.30, 0.35, 0.4);
const HIT_IDLE: Color = Color::srgba(0.12, 0.12, 0.16, 0.9);
const HIT_BORDER: Color = Color::srgb(0.35, 0.35, 0.50);
/// Amber, distinct from the metronome's own blue-ish beat-dot pulse — this
/// widget is a different signal (attack timing, not the beat count).
const HIT_PEAK: Color = Color::srgb(0.95, 0.80, 0.35);

/// Spawns the label + 8-cell pulse row for `genre`. Only call this for a
/// generated jam (`jam::session::setup` checks `GeneratedJamSession`
/// first) — a real song has no genre concept to show a guide for.
pub fn spawn_rhythm_guide(parent: &mut ChildSpawnerCommands, loc: &Localization, genre: Genre) {
    let arrangement = groove_arrangement(genre);
    let pattern = arrangement.bass;
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("jam-rhythm-guide"))})
                TextFont { font_size: {FontSize::Px(13.0)} }
                TextColor({Color::srgb(0.55, 0.55, 0.60)})
            });
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for (i, slot) in pattern.iter().enumerate() {
                    let is_rest = slot.is_none();
                    row.spawn_empty().apply_scene(bsn! {
                        Node {
                            width: {Val::Px(14.0)},
                            height: {Val::Px(14.0)},
                            border: {UiRect::all(Val::Px(1.0))},
                        }
                        BackgroundColor({if is_rest { Color::NONE } else { HIT_IDLE }})
                        ~{BorderColor::all(if is_rest { REST_BORDER } else { HIT_BORDER })}
                        RhythmGuideSlot(i)
                    });
                }
            });
        });
}

/// Pulses whichever slot is active right now — rest slots (per the current
/// `JamGenre`'s pattern) are left at their static spawn-time color, since
/// there's nothing to attack there; that's itself the point for e.g.
/// Reggae's off-beat-only pattern. Only meaningful while a generated jam is
/// in flight (see this module's own doc comment); registered gated on
/// `GeneratedJamSession`'s presence, same as [`spawn_rhythm_guide`]'s own
/// caller-side check.
pub(crate) fn update_rhythm_guide(
    clock: Res<GameplayClock>,
    tempo: Res<MetronomeTempo>,
    genre: Res<JamGenre>,
    mut slots: Query<(&RhythmGuideSlot, &mut BackgroundColor, &mut BorderColor)>,
) {
    if clock.get() < 0.0 {
        return;
    }

    // Off `MetronomeTempo` rather than `60 / bpm` and a hard-coded four:
    // a generated jam is always 4/4 today, so those agreed by luck, but
    // this is the same beat the click is ticking and must stay so.
    let secs_per_beat = tempo.beat_secs();
    let bar_secs = tempo.bar_secs();
    let bar_pos = clock.get().rem_euclid(bar_secs);
    let absolute_bar = (clock.get() / bar_secs).floor().max(0.0) as usize;
    let arrangement =
        groove_arrangement_for_position(genre.0, absolute_bar / 12, absolute_bar % 12);
    let (current, phase) = active_slot(bar_pos, secs_per_beat, arrangement.swung);

    for (slot, mut bg, mut border) in &mut slots {
        if arrangement.bass[slot.0].is_none() {
            let color = BackgroundColor(Color::NONE);
            let outline = BorderColor::all(REST_BORDER);
            if *bg != color {
                *bg = color;
            }
            if *border != outline {
                *border = outline;
            }
            continue;
        }
        let outline = BorderColor::all(HIT_BORDER);
        if *border != outline {
            *border = outline;
        }
        let brightness = if slot.0 == current { (1.0 - phase).powf(1.5) } else { 0.0 };
        let color = BackgroundColor(HIT_IDLE.mix(&HIT_PEAK, brightness));
        if *bg != color {
            *bg = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_slot_zero_starts_a_bar_at_full_onset() {
        let (slot, phase) = active_slot(0.0, 1.0, false);
        assert_eq!(slot, 0);
        assert_eq!(phase, 0.0);
    }

    #[test]
    fn straight_splits_each_beat_evenly() {
        // secs_per_beat = 1.0, straight => 0.5s per slot.
        let (slot, phase) = active_slot(0.4, 1.0, false);
        assert_eq!(slot, 0);
        assert!((phase - 0.8).abs() < 1e-6, "{phase}");

        let (slot, phase) = active_slot(0.6, 1.0, false);
        assert_eq!(slot, 1);
        assert!((phase - 0.2).abs() < 1e-6, "{phase}");
    }

    #[test]
    fn straight_advances_to_the_next_beats_slots() {
        let (slot, phase) = active_slot(1.0, 1.0, false);
        assert_eq!(slot, 2);
        assert_eq!(phase, 0.0);
    }

    #[test]
    fn swung_splits_a_beat_two_to_one() {
        // secs_per_beat = 1.0, swung => long = 2/3s, short = 1/3s.
        let (slot, phase) = active_slot(0.5, 1.0, true);
        assert_eq!(slot, 0);
        assert!((phase - 0.75).abs() < 1e-6, "{phase}");

        let (slot, phase) = active_slot(0.7, 1.0, true);
        assert_eq!(slot, 1);
        assert!(phase > 0.0 && phase < 1.0, "{phase}");
    }

    #[test]
    fn never_reports_a_slot_past_the_last_one_in_a_bar() {
        // Right at the very end of the bar (just before it would wrap).
        let (slot, _) = active_slot(3.999_999, 1.0, false);
        assert!(slot <= 7);
    }

    #[test]
    fn phase_always_stays_within_unit_range() {
        for i in 0..400 {
            let t = i as f64 * 0.01;
            for swung in [false, true] {
                let (_, phase) = active_slot(t, 1.0, swung);
                assert!((0.0..=1.0).contains(&phase), "t={t} swung={swung} phase={phase}");
            }
        }
    }
}
