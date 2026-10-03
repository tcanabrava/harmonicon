// SPDX-License-Identifier: MIT

//! The score HUD: marker components and the message-driven display update.
//! `judge::score_notes` emits a [`super::state::NoteScored`] message the
//! instant `Score` moves; [`update_score_display`] is a `MessageReader`
//! consumer, not a per-frame `format!` into `Text`.

use bevy::prelude::*;

use harmonicon_core::scoring::{HitQuality, combo_label};
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::state::{
    FEEDBACK_FADE_SECS, HitFeedback, HoleTab, JudgmentFeedback, MissReason, NoteScored, Score,
    ScoringConfig,
};

// Score HUD marker components
#[derive(Component, Default, Clone)]
pub struct ScoreText;
#[derive(Component, Default, Clone)]
pub struct ComboText;
#[derive(Component, Default, Clone)]
pub struct FeedbackText;
/// The second, smaller line under [`FeedbackText`]: what a one-word verdict
/// can't say on its own, currently only the expected-vs-heard tab of a
/// wrong-pitch miss. Blank for every other judgment, so dense passages get a
/// single line rather than two.
#[derive(Component, Default, Clone)]
pub struct FeedbackDetailText;

/// Where a mode wants its score readout, expressed against whatever node it
/// passes as the parent.
///
/// The hit line lives somewhere different in each mode — 2D's is the bottom
/// of a UI highway node, 3D's is a mesh at a fixed `HIT_Z` that only the
/// camera projects to a screen position — so the shared HUD is *told* where
/// to sit rather than trying to derive it from geometry it cannot see.
pub struct ScoreReadoutAnchor {
    /// Horizontal band to span, so the readout lines up with the lanes
    /// rather than with the window.
    pub left: Val,
    pub width: Val,
    /// Gap from the bottom of the parent up to the readout — i.e. how far
    /// the hit line is off the bottom in that mode's own layout.
    pub bottom: Val,
}

/// The one score/combo/judgment readout, used by both 2D and 3D.
///
/// **Composition is shared; only the anchor differs**, so a change to the
/// readout is a change to both modes and they can't drift apart.
///
/// Laid out hugging the two edges of the band with the middle left clear, so
/// the lanes a note actually falls down stay unobstructed: score and combo on
/// the left, the transient judgment on the right, both at the height the
/// player is already looking at.
pub fn spawn_score_readout(commands: &mut Commands, parent: Entity, anchor: ScoreReadoutAnchor) {
    let root = commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                left: {anchor.left},
                bottom: {anchor.bottom},
                width: {anchor.width},
                flex_direction: {FlexDirection::Row},
                align_items: {AlignItems::FlexEnd},
                justify_content: {JustifyContent::SpaceBetween},
                padding: {UiRect::horizontal(Val::Px(10.0))},
            }
            Children [
                Node {
                    flex_direction: {FlexDirection::Column},
                    align_items: {AlignItems::FlexStart},
                    row_gap: {Val::Px(2.0)},
                } Children [
                    Text("0") TextFont { font_size: {FontSize::Px(26.0)} }
                    TextColor({Color::WHITE}) ScoreText
                    --
                    Text("") TextFont { font_size: {FontSize::Px(14.0)} }
                    TextColor({Color::srgb(0.90, 0.72, 0.20)}) ComboText
                ]
                --
                Node {
                    flex_direction: {FlexDirection::Column},
                    align_items: {AlignItems::FlexEnd},
                    row_gap: {Val::Px(1.0)},
                } Children [
                    Text("") TextFont { font_size: {FontSize::Px(22.0)} }
                    TextColor({Color::srgba(0.0, 0.0, 0.0, 0.0)}) FeedbackText
                    --
                    Text("") TextFont { font_size: {FontSize::Px(13.0)} }
                    TextColor({Color::srgba(0.0, 0.0, 0.0, 0.0)}) FeedbackDetailText
                ]
            ]
        })
        .id();
    commands.entity(parent).add_child(root);
}

/// Localization key and tint for one judgment, shared by the label-once and
/// the per-frame color-fade halves of [`update_score_display`]. Every arm is a
/// straight lookup of a decision `score_notes` already made — the timing sign
/// that splits `Early` from `Late` is the offset the scorer classified from,
/// not a threshold reapplied here.
fn feedback_style(judgment: JudgmentFeedback) -> (&'static str, f32, f32, f32) {
    match judgment {
        JudgmentFeedback::Hit { quality: HitQuality::Perfect, .. } => {
            ("gameplay-judgment-perfect", 1.00, 0.85, 0.10)
        }
        JudgmentFeedback::Hit { quality: HitQuality::Good, offset } if offset < 0.0 => {
            ("gameplay-judgment-early", 0.40, 0.82, 1.00)
        }
        JudgmentFeedback::Hit { quality: HitQuality::Good, offset } if offset > 0.0 => {
            ("gameplay-judgment-late", 1.00, 0.68, 0.28)
        }
        JudgmentFeedback::Hit { .. } => ("gameplay-judgment-good", 0.40, 1.00, 0.35),
        JudgmentFeedback::Miss(MissReason::NoAttack) => {
            ("gameplay-judgment-no-attack", 1.00, 0.35, 0.35)
        }
        JudgmentFeedback::Miss(MissReason::WrongPitch { .. }) => {
            ("gameplay-judgment-wrong-pitch", 1.00, 0.35, 0.35)
        }
        JudgmentFeedback::Miss(MissReason::IncompleteChord) => {
            ("gameplay-judgment-incomplete-chord", 1.00, 0.42, 0.30)
        }
        JudgmentFeedback::TechniqueMiss => ("gameplay-judgment-technique", 1.00, 0.55, 0.25),
    }
}

/// A tab as the player reads it off the highway: hole number, then ↑ for blow
/// and ↓ for draw — the same arrows the hole map and the wait-for-note prompt
/// use, so one glyph means one thing everywhere on screen.
pub fn tab_label(tab: HoleTab) -> String {
    format!("{}{}", tab.hole, if tab.is_blow { "\u{2191}" } else { "\u{2193}" })
}

/// The detail line's text for one judgment — empty for everything a single
/// word already explains.
///
/// A wrong-pitch miss is the exception: "WRONG NOTE" alone tells a player
/// something they already suspected, while the tab they hit next to the tab
/// they wanted is the thing they can act on. When the heard pitch can't be
/// placed on the played harp, this names the target alone rather than
/// inventing a hole for it.
fn feedback_detail(judgment: JudgmentFeedback, loc: &Localization) -> String {
    let MissReason::WrongPitch { expected, heard } = (match judgment {
        JudgmentFeedback::Miss(reason) => reason,
        _ => return String::new(),
    }) else {
        return String::new();
    };
    match heard {
        Some(heard) => String::from(loc.msg_args(
            "gameplay-judgment-wrong-pitch-detail",
            &[("expected", tab_label(expected)), ("heard", tab_label(heard))],
        )),
        None => String::from(loc.msg_args(
            "gameplay-judgment-wrong-pitch-detail-unplaceable",
            &[("expected", tab_label(expected))],
        )),
    }
}

/// The score/combo digits only get re-`format!`ed when [`NoteScored`] says
/// `Score` actually moved. The feedback label is set once, on the frame a
/// message carries a `judgment` — not every frame of its fade, which stays a
/// per-frame color/alpha animation driven off `HitFeedback`.
pub(crate) fn update_score_display(
    mut scored: MessageReader<NoteScored>,
    score: Res<Score>,
    config: Res<ScoringConfig>,
    loc: Res<Localization>,
    mut feedback: ResMut<HitFeedback>,
    time: Res<Time>,
    mut q_score: Query<
        &mut Text,
        (With<ScoreText>, Without<ComboText>, Without<FeedbackText>, Without<FeedbackDetailText>),
    >,
    mut q_combo: Query<
        &mut Text,
        (With<ComboText>, Without<ScoreText>, Without<FeedbackText>, Without<FeedbackDetailText>),
    >,
    mut q_feedback: Query<
        (&mut Text, &mut TextColor),
        (With<FeedbackText>, Without<ScoreText>, Without<ComboText>, Without<FeedbackDetailText>),
    >,
    mut q_detail: Query<
        (&mut Text, &mut TextColor),
        (With<FeedbackDetailText>, Without<ScoreText>, Without<ComboText>, Without<FeedbackText>),
    >,
) {
    let mut score_moved = false;
    let mut fresh_judgment = None;
    for ev in scored.read() {
        score_moved = true;
        if ev.judgment.is_some() {
            fresh_judgment = ev.judgment;
        }
    }

    if score_moved {
        let points = format!("{}", score.points);
        for mut t in &mut q_score {
            if t.0 != points {
                t.0 = points.clone();
            }
        }

        let multiplier = config.multiplier(score.combo);
        let combo = combo_label(score.combo, multiplier);
        for mut t in &mut q_combo {
            if t.0 != combo {
                t.0 = combo.clone();
            }
        }
    }

    if let Some(judgment) = fresh_judgment {
        let (label_key, ..) = feedback_style(judgment);
        let label = String::from(loc.msg(label_key));
        for (mut t, _) in &mut q_feedback {
            t.0 = label.clone();
        }
        // Written on the same frame as the headline, including the empty
        // string: otherwise a wrong-pitch detail would outlive its own miss
        // and sit under the *next* note's verdict.
        let detail = feedback_detail(judgment, &loc);
        for (mut t, _) in &mut q_detail {
            t.0 = detail.clone();
        }
    }

    if feedback.timer > 0.0 {
        feedback.timer = (feedback.timer - time.delta_secs()).max(0.0);
    }

    // Colours are only written when they differ, so an idle HUD (nothing
    // judged, or a verdict already faded out) stops touching its text.
    let headline = match feedback.judgment {
        None => Color::srgba(0.0, 0.0, 0.0, 0.0),
        Some(judgment) => {
            let alpha = (feedback.timer / FEEDBACK_FADE_SECS).clamp(0.0, 1.0);
            // Scale up then fade: pulse from 1.4× down to 1× size isn't
            // easily done here, so we just fade alpha.
            let (_, r, g, b) = feedback_style(judgment);
            Color::srgba(r, g, b, alpha)
        }
    };
    for (_, mut color) in &mut q_feedback {
        if color.0 != headline {
            color.0 = headline;
        }
    }

    // The detail line fades on the same timer but in a neutral tint: the
    // headline already carries the colour coding, and repeating it twice at
    // two sizes reads as an alarm rather than an explanation.
    let detail_alpha = match feedback.judgment {
        None => 0.0,
        Some(_) => (feedback.timer / FEEDBACK_FADE_SECS).clamp(0.0, 1.0) * 0.85,
    };
    let detail = Color::srgba(0.92, 0.92, 0.92, detail_alpha);
    for (_, mut color) in &mut q_detail {
        if color.0 != detail {
            color.0 = detail;
        }
    }

    // Cleared here rather than inside the colour loop above, so a screen with
    // no feedback text spawned at all still lets a judgment expire.
    if feedback.timer == 0.0 && feedback.judgment.is_some() {
        feedback.judgment = None;
    }
}
