// SPDX-License-Identifier: MIT

//! Pure practice-gesture state machines driven by classified pitch frames.

use super::*;

const SETTLE_SECS: f32 = 0.20;
const HOLD_SECS: f32 = 0.40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PracticeShape {
    #[default]
    Free,
    FindHold,
    BendRelease,
    Repeated,
    Ladder,
    OverbendResponse,
}

impl PracticeShape {
    const ALL: [Self; 6] = [
        Self::Free,
        Self::FindHold,
        Self::BendRelease,
        Self::Repeated,
        Self::Ladder,
        Self::OverbendResponse,
    ];
    fn next(self) -> Self {
        let index = Self::ALL.iter().position(|shape| *shape == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
    fn label_key(self) -> &'static str {
        match self {
            Self::Free => "bending-shape-free",
            Self::FindHold => "bending-shape-find-hold",
            Self::BendRelease => "bending-shape-bend-release",
            Self::Repeated => "bending-shape-repeated",
            Self::Ladder => "bending-shape-ladder",
            Self::OverbendResponse => "bending-shape-overbend-response",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GesturePhase {
    #[default]
    Waiting,
    Travel,
    Holding,
    Returning,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureFrame {
    Silence,
    WrongPitch,
    Natural,
    Moving,
    Slot(usize),
    Target,
}

#[derive(Resource, Default)]
pub struct GesturePractice {
    pub shape: PracticeShape,
    pub phase: GesturePhase,
    hold_secs: f32,
    step: usize,
    repetitions: u8,
    last_beat: Option<i64>,
}

impl GesturePractice {
    #[cfg(test)]
    pub(super) fn for_shape(shape: PracticeShape) -> Self {
        Self { shape, ..default() }
    }

    fn reset_attempt(&mut self) {
        self.phase = GesturePhase::Waiting;
        self.hold_secs = 0.0;
        self.step = 0;
        self.repetitions = 0;
    }

    pub(super) fn advance(&mut self, frame: GestureFrame, dt: f32, beat: Option<i64>) {
        match self.shape {
            PracticeShape::Free => self.reset_attempt(),
            PracticeShape::FindHold => self.advance_find_hold(frame, dt),
            PracticeShape::BendRelease => self.advance_round_trip(frame, dt),
            PracticeShape::OverbendResponse => self.advance_overbend(frame, dt),
            PracticeShape::Repeated => {
                if beat != self.last_beat {
                    self.last_beat = beat;
                    if beat.is_some() && frame == GestureFrame::Target {
                        self.repetitions += 1;
                        self.phase = if self.repetitions >= 4 {
                            GesturePhase::Complete
                        } else {
                            GesturePhase::Holding
                        };
                    }
                }
            }
            PracticeShape::Ladder => match frame {
                GestureFrame::Natural if self.step == 0 => {
                    self.step = 1;
                    self.phase = GesturePhase::Travel;
                }
                GestureFrame::Slot(index) if index + 1 == self.step => self.step += 1,
                GestureFrame::Target if self.step > 0 => self.phase = GesturePhase::Returning,
                GestureFrame::Natural if self.phase == GesturePhase::Returning => {
                    self.phase = GesturePhase::Complete
                }
                _ => {}
            },
        }
    }

    fn advance_find_hold(&mut self, frame: GestureFrame, dt: f32) {
        if frame == GestureFrame::Target {
            self.phase = GesturePhase::Holding;
            self.hold_secs += dt;
            if self.hold_secs >= HOLD_SECS {
                self.phase = GesturePhase::Complete;
            }
        } else {
            self.hold_secs = 0.0;
            self.phase = GesturePhase::Waiting;
        }
    }

    fn advance_round_trip(&mut self, frame: GestureFrame, dt: f32) {
        match self.phase {
            GesturePhase::Waiting if frame == GestureFrame::Natural => {
                self.hold_secs += dt;
                if self.hold_secs >= SETTLE_SECS {
                    self.phase = GesturePhase::Travel;
                    self.hold_secs = 0.0;
                }
            }
            GesturePhase::Travel | GesturePhase::Holding if frame == GestureFrame::Target => {
                self.phase = GesturePhase::Holding;
                self.hold_secs += dt;
                if self.hold_secs >= HOLD_SECS {
                    self.phase = GesturePhase::Returning;
                    self.hold_secs = 0.0;
                }
            }
            GesturePhase::Returning if frame == GestureFrame::Natural => {
                self.hold_secs += dt;
                if self.hold_secs >= SETTLE_SECS {
                    self.phase = GesturePhase::Complete;
                }
            }
            GesturePhase::Complete => {}
            _ => self.hold_secs = 0.0,
        }
    }

    fn advance_overbend(&mut self, frame: GestureFrame, dt: f32) {
        self.advance_round_trip(frame, dt);
        if self.phase == GesturePhase::Returning && frame == GestureFrame::Silence {
            self.phase = GesturePhase::Complete;
        }
    }
}

#[derive(Component)]
pub struct PracticeShapeLabel;

pub fn cycle_practice_shape(_: On<Activate>, mut practice: ResMut<GesturePractice>) {
    practice.shape = practice.shape.next();
    practice.reset_attempt();
}

pub fn update_gesture_practice(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    active: Res<ActivePitches>,
    trace: Res<BendTrace>,
    settings: Res<BendingTrainerSettings>,
    time: Res<Time>,
    clock: Res<GameplayClock>,
    tempo: Res<MetronomeTempo>,
    mut practice: ResMut<GesturePractice>,
) {
    if target.is_changed() || key.is_changed() {
        practice.reset_attempt();
    }
    let harp = key.harp();
    let shift = reference_shift_cents(&settings, key.name(), target.hole);
    let frame = classify_gesture_frame(harp, *target, &active, &trace, shift, &settings);
    // `subdivision` splits the beat: at 2, Repeated asks for a bend on every
    // eighth rather than every quarter. The pulse count is what changes, not
    // the tempo — `MetronomeTempo` stays the shared clock for the whole
    // screen, including the audible click.
    let pulse_secs = tempo.beat_secs() / f64::from(settings.subdivision.max(1));
    let beat = (clock.get() / pulse_secs).floor() as i64;
    let previous_phase = practice.phase;
    practice.bypass_change_detection().advance(frame, time.delta_secs(), Some(beat));
    if practice.phase != previous_phase {
        practice.set_changed();
    }
}

fn classify_gesture_frame(
    harp: &Harmonica,
    target: TrainerTarget,
    active: &ActivePitches,
    trace: &BendTrace,
    shift_cents: f32,
    settings: &BendingTrainerSettings,
) -> GestureFrame {
    let cents = match tuner_observation(harp, target, active, shift_cents) {
        Some(TunerObservation::TargetFamily(cents)) => cents,
        Some(TunerObservation::WrongPitch(_)) => return GestureFrame::WrongPitch,
        _ => return GestureFrame::Silence,
    };
    if !trace.unstable && cents.abs() <= settings.tolerance_cents {
        return GestureFrame::Target;
    }
    let Some(natural) = natural_note_for_target(harp, target).and_then(|note| note_freq_hz(&note))
    else {
        return GestureFrame::Moving;
    };
    let Some(target_freq) = target_note(harp, target).and_then(|note| note_freq_hz(&note)) else {
        return GestureFrame::Moving;
    };
    let natural_cents = 1200.0 * (natural / target_freq).log2();
    if (cents - natural_cents).abs() <= 12.0 {
        return GestureFrame::Natural;
    }
    for (index, note) in intermediate_bend_notes(harp, target).iter().enumerate() {
        if let Some(freq) = note_freq_hz(note)
            && (cents - 1200.0 * (freq / target_freq).log2()).abs() <= 12.0
        {
            return GestureFrame::Slot(index);
        }
    }
    GestureFrame::Moving
}

pub fn update_practice_shape_label(
    practice: Res<GesturePractice>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<PracticeShapeLabel>>,
) {
    if !practice.is_changed() && !loc.is_changed() {
        return;
    }
    let shape = loc.msg(practice.shape.label_key());
    let phase = loc.msg(match practice.phase {
        GesturePhase::Waiting => "bending-phase-waiting",
        GesturePhase::Travel => "bending-phase-travel",
        GesturePhase::Holding => "bending-phase-holding",
        GesturePhase::Returning => "bending-phase-returning",
        GesturePhase::Complete => "bending-phase-complete",
    });
    let label = String::from(loc.msg_args(
        "bending-shape-status",
        &[("shape", shape.to_string()), ("phase", phase.to_string())],
    ));
    for mut text in &mut labels {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}
