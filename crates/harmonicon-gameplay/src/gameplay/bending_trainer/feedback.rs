// SPDX-License-Identifier: MIT

//! Selected-hole pitch classification and the optional natural-note check.

use super::*;

/// Cents to subtract from a raw reading before judging it against the
/// equal-tempered table, for one harp key and hole. Two sources, both plain
/// additive shifts in cents, which is why they can be folded into one
/// number and applied at the single point where cents is produced
/// ([`tuner_observation`]) rather than threaded through every frequency
/// lookup:
///
/// - **the A4 reference.** Retuning A4 scales every target frequency by the
///   same factor, so in cents it is the constant `1200·log2(a4/440)`.
/// - **the measured natural-reed centre** for this harp/hole, captured by
///   the readiness check. A real reed sits where it sits; the table is an
///   orientation, not ground truth.
///
/// Intervals *within* a hole (the rail's natural-to-target span, its
/// intermediate slot positions) are ratios of two table entries and so are
/// unaffected by either — do not subtract this from those.
pub(super) fn reference_shift_cents(
    settings: &BendingTrainerSettings,
    harp_key: &str,
    hole: u8,
) -> f32 {
    let a4 = 1200.0 * (settings.a4_hz / 440.0).log2();
    let center = settings
        .natural_center_cents
        .get(&BendingTrainerSettings::center_key(harp_key, hole))
        .copied()
        .unwrap_or(0.0);
    a4 + center
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum TunerObservation {
    Silent,
    WrongPitch(String),
    TargetFamily(f32),
}

pub(super) fn natural_note_for_target(harp: &Harmonica, target: TrainerTarget) -> Option<String> {
    let holes = hole_notes(harp, target.hole);
    match target.technique {
        Technique::Blow => holes.blow,
        Technique::Draw => holes.draw,
        Technique::Bend1 | Technique::Bend2 | Technique::Bend3 if target.hole <= 6 => holes.draw,
        Technique::Bend1 | Technique::Bend2 | Technique::Bend3 => holes.blow,
        Technique::Over if target.hole <= 6 => holes.blow,
        Technique::Over => holes.draw,
    }
}

/// How far off the table a measured reed centre is allowed to be before the
/// readiness check stops believing it is the same note at all.
const NATURAL_CHECK_WINDOW_CENTS: f32 = 12.0;

/// Mean deviation from the table, in cents, of the samples the readiness
/// check accepted — the reed's *observed* centre.
///
/// A mean rather than the last frame: the point of holding the note for a
/// third of a second is to average out the detector's own jitter, and one
/// frame would hand that jitter straight to every later reading as a fixed
/// bias. `None` until at least a few samples are in.
pub(super) fn observed_center_cents(samples: &[f32]) -> Option<f32> {
    if samples.len() < 5 {
        return None;
    }
    Some(samples.iter().sum::<f32>() / samples.len() as f32)
}

/// Runs the optional natural-note readiness check, and — once it confirms —
/// records the reed's observed centre into
/// [`BendingTrainerSettings::natural_center_cents`], which every later
/// reading on that hole is then measured against
/// ([`reference_shift_cents`]).
///
/// The capture is a side effect of a check the player already had a reason
/// to run, never a calibration wizard they must complete first: the trainer
/// works untouched, and a harp that was never checked just reads against
/// the table.
pub fn update_natural_check(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    active: Res<ActivePitches>,
    time: Res<Time>,
    mut settings: ResMut<BendingTrainerSettings>,
    mut check: ResMut<NaturalCheck>,
) {
    if target.is_changed() {
        *check = NaturalCheck::default();
        return;
    }
    if !check.requested || check.confirmed {
        return;
    }
    let harp = key.harp();
    let Some(note) = natural_note_for_target(harp, *target) else {
        return;
    };
    let Some(midi) = note_to_midi(&note).map(|midi| midi as u8) else {
        return;
    };
    let Some(freq) = note_freq_hz(&note) else {
        return;
    };
    // The A4 reference shifts what the table *means*, so it has to come off
    // here too — otherwise checking a harp at A=442 would measure the
    // reference back into the reed's own offset and double-count it.
    let a4_shift = 1200.0 * (settings.a4_hz / 440.0).log2();
    let deviation = active
        .0
        .iter()
        .filter(|pitch| pitch.midi == midi)
        .map(|pitch| 1200.0 * (pitch.frequency / freq).log2() - a4_shift)
        .find(|cents| cents.abs() <= NATURAL_CHECK_WINDOW_CENTS);
    if let Some(cents) = deviation {
        check.hold_secs += time.delta_secs();
        check.samples.push(cents);
    } else {
        check.hold_secs = 0.0;
        check.samples.clear();
    }
    if check.hold_secs >= 0.35 {
        check.confirmed = true;
        if let Some(center) = observed_center_cents(&check.samples) {
            settings
                .natural_center_cents
                .insert(BendingTrainerSettings::center_key(key.name(), target.hole), center);
        }
    }
}

pub fn update_natural_check_label(
    check: Res<NaturalCheck>,
    target: Res<TrainerTarget>,
    key: Res<TrainerKey>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<NaturalCheckLabel>>,
) {
    if !check.is_changed() && !target.is_changed() && !key.is_changed() && !loc.is_changed() {
        return;
    }
    let note = natural_note_for_target(key.harp(), *target).unwrap_or_else(|| "?".to_string());
    let key = if check.confirmed {
        "bending-check-natural-ready"
    } else if check.requested {
        "bending-check-natural-listening"
    } else {
        "bending-check-natural-idle"
    };
    let label = String::from(loc.msg_args(key, &[("note", note)]));
    for mut text in &mut labels {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

/// Classifies what the microphone is hearing against `target`.
/// `shift_cents` comes from [`reference_shift_cents`] and is subtracted from
/// the reported distance — **the one place the player's reference settings
/// enter the pitch maths**, so the tuner, the rail, the gesture machines and
/// the drill cannot disagree about where the target is.
pub(super) fn tuner_observation(
    harp: &Harmonica,
    target: TrainerTarget,
    active: &ActivePitches,
    shift_cents: f32,
) -> Option<TunerObservation> {
    let target_freq = note_freq_hz(&target_note(harp, target)?)?;
    if active.0.is_empty() {
        return Some(TunerObservation::Silent);
    }
    let holes = hole_notes(harp, target.hole);
    let mut family = [false; 256];
    for midi in holes
        .blow
        .iter()
        .chain(holes.draw.iter())
        .chain(holes.bends.iter())
        .chain(holes.over.iter())
        .filter_map(|note| note_to_midi(note).map(|midi| midi as u8))
    {
        family[usize::from(midi)] = true;
    }
    let target_log2 = target_freq.log2();
    let by_distance = |a: &&harmonicon_audio::pitch_detect::PitchInfo,
                       b: &&harmonicon_audio::pitch_detect::PitchInfo| {
        (a.frequency.log2() - target_log2)
            .abs()
            .total_cmp(&(b.frequency.log2() - target_log2).abs())
    };
    if let Some(heard) = active.0.iter().filter(|p| family[usize::from(p.midi)]).min_by(by_distance)
    {
        return Some(TunerObservation::TargetFamily(
            1200.0 * (heard.frequency / target_freq).log2() - shift_cents,
        ));
    }
    active
        .0
        .iter()
        .min_by(by_distance)
        .map(|heard| TunerObservation::WrongPitch(format!("{}{}", heard.note, heard.octave)))
}

fn set_tuner_readout(text: &mut Text, color: &mut TextColor, label: String, tint: Color) {
    if text.0 != label {
        text.0 = label;
    }
    if color.0 != tint {
        color.0 = tint;
    }
}

pub fn update_tuner_readout(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    active: Res<ActivePitches>,
    trace: Res<BendTrace>,
    settings: Res<BendingTrainerSettings>,
    loc: Res<Localization>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TunerReadout>>,
) {
    let Ok((mut text, mut color)) = labels.single_mut() else {
        return;
    };
    let harp = key.harp();
    let Some(target_note) = target_note(harp, *target) else {
        set_tuner_readout(
            &mut text,
            &mut color,
            String::from(loc.msg("bending-no-note-for-technique")),
            Color::srgb(0.60, 0.60, 0.65),
        );
        return;
    };
    let shift = reference_shift_cents(&settings, key.name(), target.hole);
    let Some(observation) = tuner_observation(harp, *target, &active, shift) else {
        return;
    };
    let cents = match observation {
        TunerObservation::Silent => {
            set_tuner_readout(
                &mut text,
                &mut color,
                String::from(loc.msg_args("bending-play-it-target", &[("note", target_note)])),
                Color::srgb(0.60, 0.60, 0.65),
            );
            return;
        }
        TunerObservation::WrongPitch(heard) => {
            set_tuner_readout(
                &mut text,
                &mut color,
                String::from(loc.msg_args(
                    "bending-wrong-pitch",
                    &[("note", heard), ("hole", target.hole.to_string())],
                )),
                Color::srgb(0.90, 0.60, 0.30),
            );
            return;
        }
        TunerObservation::TargetFamily(_) if trace.unstable => {
            set_tuner_readout(
                &mut text,
                &mut color,
                String::from(loc.msg("bending-signal-unstable")),
                Color::srgb(0.75, 0.65, 0.45),
            );
            return;
        }
        TunerObservation::TargetFamily(cents) => cents,
    };
    let (key, color_value) = if cents.abs() <= settings.tolerance_cents {
        ("bending-in-tune", Color::srgb(0.45, 0.85, 0.50))
    } else if cents > 0.0 {
        ("bending-cents-sharp", Color::srgb(0.90, 0.70, 0.30))
    } else {
        ("bending-cents-flat", Color::srgb(0.90, 0.70, 0.30))
    };
    let args = &[("cents", format!("{cents:+.0}")), ("note", target_note)];
    set_tuner_readout(&mut text, &mut color, String::from(loc.msg_args(key, args)), color_value);
}
