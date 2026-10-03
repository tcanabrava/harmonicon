// SPDX-License-Identifier: MIT

//! Short pitch history shared by signal-quality feedback and the bend-path UI.

use std::collections::VecDeque;

use super::*;

const STABILITY_HISTORY_SECS: f32 = 0.30;
const MIN_STABILITY_SPAN_SECS: f32 = 0.12;
const MIN_STABILITY_SAMPLES: usize = 5;
/// Line-fit residual above which the detector's estimate is called unstable
/// — also the drill's yardstick for "held unsteadily" (`DrillStat::weight`).
pub(super) const UNSTABLE_RESIDUAL_CENTS: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct TraceSample {
    pub time: f32,
    pub target_cents: f32,
}

/// Recent selected-hole pitch motion. A line-fit residual separates a smooth
/// bend from frame-to-frame detector jitter: deliberate travel may be steep,
/// but it still follows a coherent path.
#[derive(Resource, Default)]
pub struct BendTrace {
    samples: VecDeque<TraceSample>,
    elapsed: f32,
    last_hole: Option<u8>,
    last_technique: Option<Technique>,
    last_key: String,
    pub unstable: bool,
    pub target_cents: Option<f32>,
    pub stability_cents: Option<f32>,
    pub centered_hold_secs: f32,
    /// The longest unbroken centred hold since the target was last changed
    /// — the Advanced drawer's "best hold", which a live
    /// [`centered_hold_secs`](Self::centered_hold_secs) can't report because
    /// it resets the instant the player drifts out.
    pub longest_centered_hold_secs: f32,
}

impl BendTrace {
    /// The raw history, for the Advanced drawer's own analysis. Raw is the
    /// point: `trace_smoothing` is a *display* setting, and a stability or vibrato figure computed off
    /// a smoothed path would measure the knob rather than the playing.
    pub(super) fn samples(&self) -> &VecDeque<TraceSample> {
        &self.samples
    }
}

/// One attempt's pitch summary, for the Advanced drawer's stability view.
/// Three separate numbers, deliberately never collapsed into a score — a
/// player correcting a wide, wandering hold and a player correcting a tight,
/// consistently-flat one need to be told different things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AttemptStability {
    /// Mean distance from the target, in cents. Signed: a consistent lean
    /// is a different fault from scatter, and the sign says which way.
    pub mean_cents: f32,
    /// Full peak-to-peak spread of the held pitch, in cents.
    pub spread_cents: f32,
}

/// Mean and spread over `samples`, or `None` below a floor where neither
/// figure would mean anything.
pub(super) fn attempt_stability(samples: &VecDeque<TraceSample>) -> Option<AttemptStability> {
    if samples.len() < MIN_STABILITY_SAMPLES {
        return None;
    }
    let n = samples.len() as f32;
    let mean_cents = samples.iter().map(|s| s.target_cents).sum::<f32>() / n;
    let (lo, hi) = samples
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), s| (lo.min(s.target_cents), hi.max(s.target_cents)));
    Some(AttemptStability { mean_cents, spread_cents: hi - lo })
}

/// A vibrato measurement: how fast the pitch is oscillating and how wide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Vibrato {
    pub rate_hz: f32,
    /// Peak-to-peak width, in cents. Reported as width rather than as a
    /// half-amplitude because that is what a player can hear themselves
    /// widening or narrowing.
    pub depth_cents: f32,
}

/// Smallest swing that counts as vibrato rather than as the detector's own
/// noise floor wandering across the mean.
const MIN_VIBRATO_DEPTH_CENTS: f32 = 8.0;

/// Rate and depth of a periodic wobble in `samples`, or `None` when there
/// isn't one worth reporting.
///
/// Rate comes from mean crossings — each full cycle crosses the mean twice,
/// so `rate = crossings / 2 / span`. That is deliberately a *cheap*
/// estimator rather than an FFT: the window here is a fraction of a second
/// of a handful of samples per frame, far too short to resolve a 4–7 Hz
/// vibrato spectrally, and crossing-counting degrades into "no reading"
/// rather than into a confident wrong one.
///
/// This measures the pitch signal and nothing else. It says how fast and
/// how wide, not whether the vibrato is good, nor anything about breath,
/// embouchure or the reed — a microphone carries no evidence for those.
pub(super) fn vibrato(samples: &VecDeque<TraceSample>) -> Option<Vibrato> {
    let stability = attempt_stability(samples)?;
    if stability.spread_cents < MIN_VIBRATO_DEPTH_CENTS {
        return None;
    }
    let span = samples.back()?.time - samples.front()?.time;
    if span < MIN_STABILITY_SPAN_SECS {
        return None;
    }
    let crossings = samples
        .iter()
        .zip(samples.iter().skip(1))
        .filter(|(a, b)| {
            (a.target_cents - stability.mean_cents).is_sign_negative()
                != (b.target_cents - stability.mean_cents).is_sign_negative()
        })
        .count();
    if crossings < 2 {
        return None;
    }
    Some(Vibrato { rate_hz: crossings as f32 / 2.0 / span, depth_cents: stability.spread_cents })
}

/// The drawn path, low-passed by `smoothing` (0 = the raw samples).
///
/// **Display only.** A first-order filter run forward over the samples, so
/// a stronger setting also lags the live marker further behind the player —
/// which is exactly the trade-off an expert is choosing between, and why
/// this never touches [`residual_rms`] or [`vibrato`].
#[cfg(test)]
pub(super) fn smoothed_cents(samples: &VecDeque<TraceSample>, smoothing: f32) -> Vec<f32> {
    let mut out = Vec::new();
    smooth_cents_into(samples, smoothing, &mut out);
    out
}

fn smooth_cents_into(samples: &VecDeque<TraceSample>, smoothing: f32, out: &mut Vec<f32>) {
    let alpha = 1.0 - smoothing.clamp(0.0, 0.95);
    out.clear();
    out.reserve(samples.len());
    let mut state: Option<f32> = None;
    for sample in samples {
        let next = match state {
            None => sample.target_cents,
            Some(prev) => prev + alpha * (sample.target_cents - prev),
        };
        state = Some(next);
        out.push(next);
    }
}

const TRACE_DOTS: usize = 32;
const RAIL_START_PERCENT: f32 = 8.0;
const RAIL_END_PERCENT: f32 = 92.0;
/// The rail is the screen's centrepiece, so it is sized to be read from arm's
/// length on a tablet. Every vertical position inside it derives from this
/// one height, so resizing it can't leave the marker off the line.
const RAIL_HEIGHT: f32 = 96.0;
const DOT_SIZE: f32 = 8.0;
const MARKER_SIZE: f32 = 18.0;

#[derive(Component, Default, Clone)]
pub struct BendTraceDot(pub usize);
#[derive(Component, Default, Clone)]
pub struct BendLiveMarker;
#[derive(Component, Default, Clone)]
pub struct BendTargetBand;
#[derive(Component, Default, Clone)]
pub struct BendNaturalLabel;
#[derive(Component, Default, Clone)]
pub struct BendTargetLabel;
#[derive(Component, Default, Clone)]
pub struct BendSlotMarker(pub usize);
#[derive(Component, Clone, Copy)]
pub enum BendMetric {
    Distance,
    Stability,
    Hold,
}

pub(super) fn spawn_bend_rail(card: &mut ChildSpawnerCommands, loc: &Localization) {
    card.spawn(Node {
        width: Val::Percent(100.0),
        justify_content: JustifyContent::SpaceBetween,
        ..default()
    })
    .with_children(|labels| {
        labels.spawn_empty().apply_scene(bsn! {
            Text({loc.msg("bending-rail-natural")})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({Color::srgb(0.70, 0.74, 0.80)})
            BendNaturalLabel
        });
        labels.spawn_empty().apply_scene(bsn! {
            Text({loc.msg("bending-rail-target")})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({Color::srgb(0.70, 0.84, 0.74)})
            BendTargetLabel
        });
    });
    card.spawn_empty()
        .apply_scene(bsn! {
            Node {
                position_type: {PositionType::Relative},
                width: {Val::Percent(100.0)},
                height: {Val::Px(RAIL_HEIGHT)},
            }
            BackgroundColor({Color::srgba(0.06, 0.07, 0.10, 0.90)})
        })
        .with_children(|rail| {
            rail.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    left: {Val::Percent(RAIL_START_PERCENT)},
                    top: {Val::Px(RAIL_HEIGHT / 2.0 - 1.0)},
                    width: {Val::Percent(RAIL_END_PERCENT - RAIL_START_PERCENT)},
                    height: {Val::Px(2.0)},
                }
                BackgroundColor({Color::srgb(0.35, 0.38, 0.44)})
            });
            rail.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    top: {Val::Px(RAIL_HEIGHT * 0.2)},
                    height: {Val::Px(RAIL_HEIGHT * 0.6)},
                }
                BackgroundColor({Color::srgba(0.25, 0.75, 0.38, 0.20)})
                BendTargetBand
            });
            for index in 0..2usize {
                rail.spawn_empty().apply_scene(bsn! {
                    Node {
                        position_type: {PositionType::Absolute},
                        top: {Val::Px(2.0)},
                    }
                    Text("")
                    TextFont { font_size: {FontSize::Px(11.0)} }
                    TextColor({Color::srgb(0.72, 0.76, 0.82)})
                    ~{Visibility::Hidden}
                    BendSlotMarker(index)
                });
            }
            for index in 0..TRACE_DOTS {
                rail.spawn_empty().apply_scene(bsn! {
                    Node {
                        position_type: {PositionType::Absolute},
                        top: {Val::Px((RAIL_HEIGHT - DOT_SIZE) / 2.0)},
                        width: {Val::Px(DOT_SIZE)},
                        height: {Val::Px(DOT_SIZE)},
                        border_radius: {BorderRadius::all(Val::Percent(50.0))},
                    }
                    BackgroundColor({Color::srgba(0.45, 0.72, 0.95, 0.0)})
                    ~{Visibility::Hidden}
                    BendTraceDot(index)
                });
            }
            rail.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    top: {Val::Px((RAIL_HEIGHT - MARKER_SIZE) / 2.0)},
                    width: {Val::Px(MARKER_SIZE)},
                    height: {Val::Px(MARKER_SIZE)},
                    border_radius: {BorderRadius::all(Val::Percent(50.0))},
                }
                BackgroundColor({Color::srgb(0.92, 0.94, 1.0)})
                ~{Visibility::Hidden}
                BendLiveMarker
            });
        });
    card.spawn(Node {
        width: Val::Percent(100.0),
        justify_content: JustifyContent::SpaceBetween,
        ..default()
    })
    .with_children(|metrics| {
        for metric in [BendMetric::Distance, BendMetric::Stability, BendMetric::Hold] {
            metrics
                .spawn_empty()
                .apply_scene(bsn! {
                    Text("")
                    TextFont { font_size: {FontSize::Px(14.0)} }
                    TextColor({Color::srgb(0.68, 0.72, 0.78)})
                })
                .insert(metric);
        }
    });
}

pub(super) fn rail_percent(target_cents: f32, natural_target_cents: f32) -> f32 {
    if natural_target_cents.abs() < 1.0 {
        return (RAIL_START_PERCENT + RAIL_END_PERCENT) * 0.5;
    }
    let progress = (natural_target_cents - target_cents) / natural_target_cents;
    (RAIL_START_PERCENT + progress * (RAIL_END_PERCENT - RAIL_START_PERCENT)).clamp(2.0, 98.0)
}

pub(super) fn intermediate_bend_notes(harp: &Harmonica, target: TrainerTarget) -> Vec<String> {
    let count = match target.technique {
        Technique::Bend2 => 1,
        Technique::Bend3 => 2,
        _ => 0,
    };
    hole_notes(harp, target.hole).bends.into_iter().take(count).collect()
}

#[cfg(test)]
pub(super) fn residual_rms(samples: &VecDeque<TraceSample>) -> Option<f32> {
    residual_rms_iter(samples.iter().copied())
}

fn residual_rms_iter(samples: impl Iterator<Item = TraceSample> + Clone) -> Option<f32> {
    let n = samples.clone().count();
    if n < MIN_STABILITY_SAMPLES {
        return None;
    }
    let span = samples.clone().last()?.time - samples.clone().next()?.time;
    if span < MIN_STABILITY_SPAN_SECS {
        return None;
    }

    let n = n as f32;
    let mean_t = samples.clone().map(|sample| sample.time).sum::<f32>() / n;
    let mean_c = samples.clone().map(|sample| sample.target_cents).sum::<f32>() / n;
    let variance_t = samples.clone().map(|sample| (sample.time - mean_t).powi(2)).sum::<f32>();
    if variance_t <= f32::EPSILON {
        return None;
    }
    let slope = samples
        .clone()
        .map(|sample| (sample.time - mean_t) * (sample.target_cents - mean_c))
        .sum::<f32>()
        / variance_t;
    let residual = samples
        .map(|sample| {
            let fitted = mean_c + slope * (sample.time - mean_t);
            (sample.target_cents - fitted).powi(2)
        })
        .sum::<f32>();
    Some((residual / n).sqrt())
}

pub fn update_bend_trace(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    active: Res<ActivePitches>,
    settings: Res<BendingTrainerSettings>,
    time: Res<Time>,
    mut trace: ResMut<BendTrace>,
) {
    let target_changed = trace.last_hole != Some(target.hole)
        || trace.last_technique != Some(target.technique)
        || trace.last_key != key.name();
    if target_changed {
        trace.samples.clear();
        trace.unstable = false;
        trace.target_cents = None;
        trace.stability_cents = None;
        trace.centered_hold_secs = 0.0;
        trace.longest_centered_hold_secs = 0.0;
        trace.last_hole = Some(target.hole);
        trace.last_technique = Some(target.technique);
        trace.last_key.replace_range(.., key.name());
    }

    trace.elapsed += time.delta_secs();
    let harp = key.harp();
    let shift = reference_shift_cents(&settings, key.name(), target.hole);
    match tuner_observation(harp, *target, &active, shift) {
        Some(TunerObservation::TargetFamily(target_cents)) => {
            let elapsed = trace.elapsed;
            trace.samples.push_back(TraceSample { time: elapsed, target_cents });
            while trace
                .samples
                .front()
                .is_some_and(|sample| elapsed - sample.time > settings.trace_secs)
            {
                trace.samples.pop_front();
            }
            trace.stability_cents = residual_rms_iter(
                trace
                    .samples
                    .iter()
                    .filter(|sample| elapsed - sample.time <= STABILITY_HISTORY_SECS)
                    .copied(),
            );
            trace.unstable =
                trace.stability_cents.is_some_and(|residual| residual > UNSTABLE_RESIDUAL_CENTS);
            trace.target_cents = Some(target_cents);
            trace.centered_hold_secs =
                if target_cents.abs() <= settings.tolerance_cents && !trace.unstable {
                    trace.centered_hold_secs + time.delta_secs()
                } else {
                    0.0
                };
            trace.longest_centered_hold_secs =
                trace.longest_centered_hold_secs.max(trace.centered_hold_secs);
        }
        _ => {
            trace.samples.clear();
            trace.unstable = false;
            trace.target_cents = None;
            trace.stability_cents = None;
            trace.centered_hold_secs = 0.0;
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn update_bend_rail(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    trace: Res<BendTrace>,
    settings: Res<BendingTrainerSettings>,
    loc: Res<Localization>,
    mut drawn: Local<Vec<f32>>,
    // Four of these take `&mut Node` and four take `&mut Text`, so every pair
    // sharing one needs a `Without` proving them disjoint — a marker component
    // the *other* query requires is not enough, since Bevy's check is purely
    // structural and a missing exclusion is a startup panic (B0001), not a
    // compile error. Each query excludes every earlier one it shares a
    // component with.
    mut dots: Query<(&BendTraceDot, &mut Node, &mut BackgroundColor, &mut Visibility)>,
    mut marker: Query<(&mut Node, &mut Visibility), (With<BendLiveMarker>, Without<BendTraceDot>)>,
    mut band: Query<
        &mut Node,
        (With<BendTargetBand>, Without<BendTraceDot>, Without<BendLiveMarker>),
    >,
    mut slots: Query<
        (&BendSlotMarker, &mut Node, &mut Text, &mut Visibility),
        (Without<BendTraceDot>, Without<BendLiveMarker>, Without<BendTargetBand>),
    >,
    mut natural_labels: Query<&mut Text, (With<BendNaturalLabel>, Without<BendSlotMarker>)>,
    mut target_labels: Query<
        &mut Text,
        (With<BendTargetLabel>, Without<BendSlotMarker>, Without<BendNaturalLabel>),
    >,
    mut metrics: Query<
        (&BendMetric, &mut Text),
        (Without<BendSlotMarker>, Without<BendNaturalLabel>, Without<BendTargetLabel>),
    >,
) {
    let harp = key.harp();
    let Some(target_note) = target_note(harp, *target) else {
        return;
    };
    let Some(natural_note) = natural_note_for_target(harp, *target) else {
        return;
    };
    let (Some(target_freq), Some(natural_freq)) =
        (note_freq_hz(&target_note), note_freq_hz(&natural_note))
    else {
        return;
    };
    let natural_cents = 1200.0 * (natural_freq / target_freq).log2();

    for mut text in &mut natural_labels {
        *text = Text::new(String::from(
            loc.msg_args("bending-rail-natural-note", &[("note", natural_note.clone())]),
        ));
    }
    for mut text in &mut target_labels {
        *text = Text::new(String::from(
            loc.msg_args("bending-rail-target-note", &[("note", target_note.clone())]),
        ));
    }
    for (metric, mut text) in &mut metrics {
        let (key, value) = match metric {
            BendMetric::Distance => (
                "bending-metric-distance",
                trace.target_cents.map_or_else(|| "—".to_string(), |cents| format!("{cents:+.0}c")),
            ),
            BendMetric::Stability => (
                "bending-metric-stability",
                trace
                    .stability_cents
                    .map_or_else(|| "—".to_string(), |cents| format!("{cents:.0}c")),
            ),
            BendMetric::Hold => {
                ("bending-metric-hold", format!("{:.1}s", trace.centered_hold_secs))
            }
        };
        *text = Text::new(String::from(loc.msg_args(key, &[("value", value)])));
    }
    let intermediate = intermediate_bend_notes(harp, *target);
    for (slot, mut node, mut text, mut visibility) in &mut slots {
        let Some(note) = intermediate.get(slot.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(freq) = note_freq_hz(note) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let cents = 1200.0 * (freq / target_freq).log2();
        node.left = Val::Percent(rail_percent(cents, natural_cents));
        *text = Text::new(format!("│ {note}"));
        *visibility = Visibility::Visible;
    }
    let band_width = if natural_cents.abs() < 1.0 {
        10.0
    } else {
        (2.0 * settings.tolerance_cents / natural_cents.abs()
            * (RAIL_END_PERCENT - RAIL_START_PERCENT))
            .clamp(3.0, 20.0)
    };
    for mut node in &mut band {
        node.left = Val::Percent(rail_percent(0.0, natural_cents) - band_width * 0.5);
        node.width = Val::Percent(band_width);
    }

    // The drawn path is smoothed (a no-op at the default 0.0); everything
    // measured off the trace stays on the raw samples.
    smooth_cents_into(&trace.samples, settings.trace_smoothing, &mut drawn);
    let sample_step = trace.samples.len().div_ceil(TRACE_DOTS).max(1);
    let mut visible = [None; TRACE_DOTS];
    for (index, (sample, cents)) in trace
        .samples
        .iter()
        .zip(drawn.iter())
        .rev()
        .step_by(sample_step)
        .take(TRACE_DOTS)
        .enumerate()
    {
        visible[index] = Some((*sample, *cents));
    }
    for (dot, mut node, mut color, mut visibility) in &mut dots {
        let Some(Some((sample, cents))) = visible.get(dot.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        node.left = Val::Percent(rail_percent(*cents, natural_cents));
        let age = trace.elapsed - sample.time;
        let alpha = (1.0 - age / settings.trace_secs).clamp(0.08, 0.72);
        color.0 = Color::srgba(0.45, 0.72, 0.95, alpha);
        *visibility = Visibility::Visible;
    }
    let Ok((mut node, mut visibility)) = marker.single_mut() else {
        return;
    };
    if let Some(cents) = drawn.last() {
        node.left = Val::Percent(rail_percent(*cents, natural_cents));
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}
