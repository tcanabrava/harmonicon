// SPDX-License-Identifier: MIT

//! The score HUD: marker components and the message-driven display update.
//! `judge::score_notes` emits a [`super::state::NoteScored`] message the
//! instant `Score` moves; [`update_score_display`] is a `MessageReader`
//! consumer, not a per-frame `format!` into `Text`.

use bevy::prelude::*;

use crate::scoring::{HitQuality, combo_label, compute_multiplier};

use super::state::{HitFeedback, NoteScored, Score, ScoringConfig};

// Score HUD marker components
#[derive(Component)]
pub struct ScoreText;
#[derive(Component)]
pub struct ComboText;
#[derive(Component)]
pub struct FeedbackText;

/// Label and tint for a judged hit quality, shared by the label-once and the
/// per-frame color-fade halves of [`update_score_display`].
fn feedback_style(quality: HitQuality) -> (&'static str, f32, f32, f32) {
    match quality {
        HitQuality::Perfect => ("PERFECT!", 1.00, 0.85, 0.10),
        HitQuality::Good => ("GOOD", 0.40, 1.00, 0.35),
    }
}

/// The score/combo digits only get re-`format!`ed when [`NoteScored`] says
/// `Score` actually moved. The feedback label ("PERFECT!"/"GOOD") is set
/// once, on the frame a fresh hit's message carries a `quality` — not every
/// frame of its fade, which stays a per-frame animation (color/alpha only)
/// driven straight off `HitFeedback`, same as before.
pub(crate) fn update_score_display(
    mut scored: MessageReader<NoteScored>,
    score: Res<Score>,
    config: Res<ScoringConfig>,
    mut feedback: ResMut<HitFeedback>,
    time: Res<Time>,
    mut q_score: Query<&mut Text, (With<ScoreText>, Without<ComboText>, Without<FeedbackText>)>,
    mut q_combo: Query<&mut Text, (With<ComboText>, Without<ScoreText>, Without<FeedbackText>)>,
    mut q_feedback: Query<
        (&mut Text, &mut TextColor),
        (With<FeedbackText>, Without<ScoreText>, Without<ComboText>),
    >,
) {
    let mut score_moved = false;
    let mut fresh_hit = None;
    for ev in scored.read() {
        score_moved = true;
        if ev.quality.is_some() {
            fresh_hit = ev.quality;
        }
    }

    if score_moved {
        let points = format!("{}", score.points);
        for mut t in &mut q_score {
            if t.0 != points {
                t.0 = points.clone();
            }
        }

        // Same multiplier `score_notes` actually applies to points, so the HUD
        // can never show a number the score disagrees with.
        let multiplier = if config.combo_enabled {
            compute_multiplier(
                score.combo,
                config.base_multiplier,
                config.step_multiplier,
                config.max_multiplier,
            )
        } else {
            1.0
        };
        let combo = combo_label(score.combo, multiplier);
        for mut t in &mut q_combo {
            if t.0 != combo {
                t.0 = combo.clone();
            }
        }
    }

    if let Some(q) = fresh_hit {
        let (label, ..) = feedback_style(q);
        for (mut t, _) in &mut q_feedback {
            t.0 = label.to_string();
        }
    }

    feedback.timer = (feedback.timer - time.delta_secs()).max(0.0);

    for (_, mut color) in &mut q_feedback {
        match feedback.quality {
            None => {
                *color = TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
            Some(q) => {
                let alpha = (feedback.timer / 0.75).clamp(0.0, 1.0);
                // Scale up then fade: pulse from 1.4× down to 1× size isn't
                // easily done here, so we just fade alpha.
                let (_, r, g, b) = feedback_style(q);
                *color = TextColor(Color::srgba(r, g, b, alpha));
                if feedback.timer == 0.0 {
                    feedback.quality = None;
                }
            }
        }
    }
}
