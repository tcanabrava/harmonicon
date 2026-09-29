// SPDX-License-Identifier: MIT

//! The 2D technique coach: a fixed row between the highway and the hole
//! strip that coaches the bend/vibrato/wah note arriving at the hit line.
//! It never moves, and it says as little as it can in large type — the
//! player reads it at a glance mid-phrase, not as a paragraph.
//!
//! - **Bend**: a horizontal track from the unbent note (left) to the target
//!   (right) with the on-target band marked; the marker is the player's
//!   pitch, so bending slides it right toward the band.
//! - **Vibrato / wah**: a reference tick swinging at the chart's rate — the
//!   wobble to copy — and, once the note is held, the player's own pitch
//!   (vibrato) or loudness (wah) swinging with it. The dim band is the swing
//!   too small for the judge to count.
//!
//! Every reading comes from `technique_cue`, over the same samples and
//! tolerances the judge scores with.

use bevy::prelude::*;

use harmonicon_audio::AudioSettings;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::judge::judged_instant;
use super::notes::SongNotes;
use super::state::{ActivePitches, HarmonicaPitchFilter};
use super::technique_cue::{
    BendAdvice, CoachMode, RateAdvice, bend_reading, bend_target_band, bend_track_pct,
    bend_track_range, coach_mode, coach_note, format_rate, measured_rate, min_swing_band,
    pitch_class_name, rate_advice, reference_swing, swing_track_pct, vibrato_swing, wah_swing,
};

/// The coach row, holding its parts' entities so the per-frame update
/// reaches them without a marker query each.
#[derive(Component)]
pub(super) struct TechniqueCoach {
    start_label: Entity,
    band: Entity,
    reference: Entity,
    marker: Entity,
    end_label: Entity,
    advice: Entity,
}

const ROW_BG: Color = Color::srgba(0.06, 0.06, 0.09, 1.0);
const TRACK_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.14);
const TARGET_BAND: Color = Color::srgba(0.35, 0.9, 0.45, 0.35);
const DEAD_BAND: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);
const REFERENCE: Color = Color::srgba(0.8, 0.85, 1.0, 0.75);
const MARKER_ON: Color = Color::srgb(0.6, 1.0, 0.65);
const MARKER_OFF: Color = Color::srgb(1.0, 0.72, 0.2);
const LABEL: Color = Color::srgba(0.92, 0.94, 1.0, 0.95);

/// Row height: room for the large labels with the track between them.
const ROW_PX: f32 = 56.0;
/// Label and advice type size — read at a glance while playing.
const LABEL_PX: f32 = 28.0;
/// Height of the track the ticks ride on.
const TRACK_PX: f32 = 10.0;
/// Width of the reference/marker ticks; they are centred on their
/// percentage by a negative left margin of half this.
const TICK_PX: f32 = 6.0;

/// Spawns the coach row into the 2D layout's left column. Called from setup
/// only when the chart has something to coach, so a plain chart keeps its
/// full highway height.
pub(super) fn spawn_technique_coach(parent: &mut ChildSpawnerCommands) {
    /// A fixed-width cell holding one centred label; returns the label.
    fn label(row: &mut ChildSpawnerCommands, width: f32) -> Entity {
        let mut text = Entity::PLACEHOLDER;
        row.spawn(Node {
            width: Val::Px(width),
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|cell| {
            text = cell
                .spawn_empty()
                .apply_scene(bsn! {
                    Text("")
                    TextFont { font_size: {FontSize::Px(LABEL_PX)} }
                    TextColor({LABEL})
                    ~{TextLayout::no_wrap()}
                })
                .id();
        });
        text
    }
    /// A tick (or, once sized by the update, a band) on the track.
    fn tick(track: &mut ChildSpawnerCommands, color: Color, height: f32) -> Entity {
        track
            .spawn_empty()
            .apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    width: {Val::Px(TICK_PX)}, height: {Val::Px(height)},
                    top: {Val::Px((TRACK_PX - height) * 0.5)},
                    margin: {UiRect::left(Val::Px(-TICK_PX * 0.5))},
                    display: {Display::None},
                }
                BackgroundColor({color})
            })
            .id()
    }

    let mut row = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(ROW_PX),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            padding: UiRect::horizontal(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(ROW_BG),
    ));
    let mut coach = None;
    row.with_children(|row| {
        let start_label = label(row, 140.0);
        let (mut band, mut reference, mut marker) = (
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
        );
        row.spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(TRACK_PX),
                ..default()
            },
            BackgroundColor(TRACK_BG),
        ))
        .with_children(|track| {
            band = tick(track, TARGET_BAND, TRACK_PX);
            reference = tick(track, REFERENCE, 30.0);
            marker = tick(track, MARKER_OFF, 40.0);
        });
        let end_label = label(row, 140.0);
        let advice = label(row, 260.0);
        coach = Some(TechniqueCoach {
            start_label,
            band,
            reference,
            marker,
            end_label,
            advice,
        });
    });
    if let Some(coach) = coach {
        // Outlined so the marker stays distinct inside the band it aims for.
        row.commands().entity(coach.marker).insert(Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::BLACK,
        ));
        row.insert(coach);
    }
}

/// One tick's wanted state: `None` hides it; `Some((left_pct, None))` shows
/// a tick centred there, `Some((left_pct, Some(width_pct)))` a band
/// spanning right from it.
type TickState = Option<(f32, Option<f32>)>;

fn apply_tick(node: &mut Node, state: TickState) {
    match state {
        None => {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
        Some((left_pct, width_pct)) => {
            node.display = Display::Flex;
            node.left = Val::Percent(left_pct);
            (node.width, node.margin) = match width_pct {
                Some(w) => (Val::Percent(w), UiRect::ZERO),
                None => (Val::Px(TICK_PX), UiRect::left(Val::Px(-TICK_PX * 0.5))),
            };
        }
    }
}

fn set_text(texts: &mut Query<&mut Text>, entity: Entity, wanted: &str) {
    if let Ok(mut text) = texts.get_mut(entity)
        && text.0 != wanted
    {
        text.0 = wanted.to_string();
    }
}

fn bend_advice_key(advice: BendAdvice) -> Option<&'static str> {
    match advice {
        BendAdvice::Silent => None,
        BendAdvice::BendMore => Some("coach-bend-more"),
        BendAdvice::OnTarget => Some("coach-on-target"),
        BendAdvice::TooFar => Some("coach-too-far"),
    }
}

fn rate_advice_key(advice: RateAdvice) -> &'static str {
    match advice {
        RateAdvice::FollowPulse => "coach-follow-pulse",
        RateAdvice::SwingMore => "coach-swing-more",
        RateAdvice::Faster => "coach-faster",
        RateAdvice::Slower => "coach-slower",
        RateAdvice::OnRate => "coach-on-rate",
    }
}

/// Redraws the coach for the note it should coach this frame, or empties
/// it when no technique note is near. The row itself stays put either way,
/// so the layout never jumps.
pub(super) fn update_technique_coach(
    song_notes: Res<SongNotes>,
    clock: Res<super::GameplayClock>,
    audio: Res<AudioSettings>,
    pitch_filter: Res<HarmonicaPitchFilter>,
    active: Res<ActivePitches>,
    loc: Res<Localization>,
    coaches: Query<&TechniqueCoach>,
    mut ticks: Query<(&mut Node, &mut BackgroundColor)>,
    mut texts: Query<&mut Text>,
) {
    let Ok(coach) = coaches.single() else {
        return;
    };
    let judged = judged_instant(clock.get(), &audio, Some(&pitch_filter));
    let coached = coach_note(&song_notes.notes, song_notes.cursor, judged)
        .map(|i| &song_notes.notes[i])
        .and_then(|note| coach_mode(note).map(|mode| (note, mode)));

    let (start, end, band, reference, marker, marker_on, advice): (
        String,
        String,
        TickState,
        TickState,
        TickState,
        bool,
        Option<String>,
    ) = match coached {
        None => (String::new(), String::new(), None, None, None, false, None),
        Some((_, CoachMode::Bend { natural, target })) => {
            let reading = bend_reading(active.0.iter().map(|p| p.frequency), natural, target);
            let (lo, hi) = bend_target_band(natural, target);
            let range = bend_track_range(natural, target);
            let band_left = bend_track_pct(lo, range);
            (
                pitch_class_name(natural),
                pitch_class_name(target),
                Some((band_left, Some(bend_track_pct(hi, range) - band_left))),
                None,
                reading.position.map(|p| (bend_track_pct(p, range), None)),
                reading.advice == BendAdvice::OnTarget,
                bend_advice_key(reading.advice).map(|k| String::from(loc.msg(k))),
            )
        }
        Some((note, mode @ (CoachMode::Vibrato { hz } | CoachMode::Wah { hz }))) => {
            let half = min_swing_band(mode);
            let band_left = swing_track_pct(half);
            let pulse = reference_swing(hz, judged - note.time);
            let (swing, advice) = if note.hit {
                let swing = match mode {
                    CoachMode::Wah { .. } => wah_swing(&note.amp_samples),
                    _ => vibrato_swing(&note.pitch_samples),
                };
                let (measured, samples) = measured_rate(mode, note);
                (swing, rate_advice(measured, hz, samples))
            } else {
                (None, RateAdvice::FollowPulse)
            };
            let name = match mode {
                CoachMode::Wah { .. } => "mod-wah",
                _ => "mod-vibrato",
            };
            (
                String::from(loc.msg(name)),
                String::from(loc.msg_args("coach-rate", &[("rate", format_rate(hz))])),
                Some((band_left, Some(swing_track_pct(-half) - band_left))),
                Some((swing_track_pct(pulse), None)),
                swing.map(|s| (swing_track_pct(s), None)),
                advice == RateAdvice::OnRate,
                Some(String::from(loc.msg(rate_advice_key(advice)))),
            )
        }
    };

    set_text(&mut texts, coach.start_label, &start);
    set_text(&mut texts, coach.end_label, &end);
    set_text(&mut texts, coach.advice, advice.as_deref().unwrap_or(""));

    let band_color = match coached {
        Some((_, CoachMode::Bend { .. })) => TARGET_BAND,
        _ => DEAD_BAND,
    };
    let marker_color = if marker_on { MARKER_ON } else { MARKER_OFF };
    for (entity, state, color) in [
        (coach.band, band, Some(band_color)),
        (coach.reference, reference, None),
        (coach.marker, marker, Some(marker_color)),
    ] {
        if let Ok((mut node, mut background)) = ticks.get_mut(entity) {
            apply_tick(&mut node, state);
            if let Some(color) = color
                && background.0 != color
            {
                background.0 = color;
            }
        }
    }
}
