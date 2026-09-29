// SPDX-License-Identifier: MIT

//! Precision controls and live pitch measurements for the Bending Trainer.

use harmonicon_ui::dialogs::drawer::Drawer;

use super::*;

/// Which knob a stepper row edits. One component and one label system for
/// all of them, rather than a marker type each.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedKnob {
    Tolerance,
    Hold,
    Timeout,
    A4,
    Trace,
    Smoothing,
    Subdivision,
}

/// Which measurement a readout line shows. Same one-component-many-lines
/// shape as [`AdvancedKnob`].
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedReadout {
    Mean,
    Spread,
    BestHold,
    Vibrato,
    Center,
}

/// The drawer's collapsible body.
#[derive(Component)]
pub struct AdvancedDrawer;

/// Steps for each knob, chosen so the values a player actually asks for are
/// one or two clicks apart rather than a dozen.
const TOLERANCE_STEP: f32 = 1.0;
const HOLD_STEP: f32 = 0.1;
const TIMEOUT_STEP: f32 = 2.0;
const A4_STEP: f32 = 1.0;
const TRACE_STEP: f32 = 0.5;
const SMOOTHING_STEP: f32 = 0.1;

impl AdvancedKnob {
    pub(super) fn label_key(self) -> &'static str {
        match self {
            Self::Tolerance => "bending-adv-tolerance",
            Self::Hold => "bending-adv-hold",
            Self::Timeout => "bending-adv-timeout",
            Self::A4 => "bending-adv-a4",
            Self::Trace => "bending-adv-trace",
            Self::Smoothing => "bending-adv-smoothing",
            Self::Subdivision => "bending-adv-subdivision",
        }
    }

    /// The knob's current value, formatted with its unit.
    fn value_text(self, settings: &BendingTrainerSettings) -> String {
        match self {
            Self::Tolerance => format!("±{:.0}c", settings.tolerance_cents),
            Self::Hold => format!("{:.1}s", settings.hold_secs),
            Self::Timeout => format!("{:.0}s", settings.timeout_secs),
            Self::A4 => format!("{:.0} Hz", settings.a4_hz),
            Self::Trace => format!("{:.1}s", settings.trace_secs),
            Self::Smoothing => format!("{:.0}%", settings.trace_smoothing * 100.0),
            Self::Subdivision => format!("1/{}", settings.subdivision),
        }
    }

    /// Moves this knob by `direction` (-1 or +1) of its own step, clamped to
    /// its own bounds. One place decides how each knob moves, so the two
    /// stepper buttons can't disagree about the step or the limits.
    pub(super) fn nudge(self, settings: &mut BendingTrainerSettings, direction: f32) {
        fn step(value: f32, by: f32, (lo, hi): (f32, f32)) -> f32 {
            (value + by).clamp(lo, hi)
        }
        match self {
            Self::Tolerance => {
                settings.tolerance_cents = step(
                    settings.tolerance_cents,
                    direction * TOLERANCE_STEP,
                    BendingTrainerSettings::TOLERANCE_CENTS,
                );
            }
            Self::Hold => {
                settings.hold_secs = step(
                    settings.hold_secs,
                    direction * HOLD_STEP,
                    BendingTrainerSettings::HOLD_SECS,
                );
            }
            Self::Timeout => {
                settings.timeout_secs = step(
                    settings.timeout_secs,
                    direction * TIMEOUT_STEP,
                    BendingTrainerSettings::TIMEOUT_SECS,
                );
            }
            Self::A4 => {
                settings.a4_hz = step(
                    settings.a4_hz,
                    direction * A4_STEP,
                    BendingTrainerSettings::A4_HZ,
                );
            }
            Self::Trace => {
                settings.trace_secs = step(
                    settings.trace_secs,
                    direction * TRACE_STEP,
                    BendingTrainerSettings::TRACE_SECS,
                );
            }
            Self::Smoothing => {
                settings.trace_smoothing = step(
                    settings.trace_smoothing,
                    direction * SMOOTHING_STEP,
                    BendingTrainerSettings::TRACE_SMOOTHING,
                );
            }
            Self::Subdivision => {
                let (lo, hi) = BendingTrainerSettings::SUBDIVISION;
                let next = i16::from(settings.subdivision) + direction as i16;
                settings.subdivision = next.clamp(i16::from(lo), i16::from(hi)) as u8;
            }
        }
    }
}

/// Every knob, in drawer order: the constraints that change what counts as a
/// success first, then the reference, then what the trace *looks* like.
pub(super) const KNOBS: [AdvancedKnob; 7] = [
    AdvancedKnob::Tolerance,
    AdvancedKnob::Hold,
    AdvancedKnob::Timeout,
    AdvancedKnob::A4,
    AdvancedKnob::Trace,
    AdvancedKnob::Smoothing,
    AdvancedKnob::Subdivision,
];

const READOUTS: [AdvancedReadout; 5] = [
    AdvancedReadout::Mean,
    AdvancedReadout::Spread,
    AdvancedReadout::BestHold,
    AdvancedReadout::Vibrato,
    AdvancedReadout::Center,
];

/// Toggles the drawer, persisting the new state — reopening the trainer
/// lands an expert back in the view they were using.
pub fn toggle_advanced_drawer(_: On<Activate>, mut settings: ResMut<BendingTrainerSettings>) {
    settings.advanced_open = !settings.advanced_open;
}

/// Restores every knob to its shipped default. The drawer is the only way
/// to reach a bad combination of settings (a 1-cent tolerance with a 3-second
/// hold is not practice, it's a wall), so it owns the way back out.
pub fn reset_advanced_settings(_: On<Activate>, mut settings: ResMut<BendingTrainerSettings>) {
    let keep = std::mem::take(&mut settings.natural_center_cents);
    let advanced_open = settings.advanced_open;
    *settings = BendingTrainerSettings {
        // A measured reed centre is not a preference and survives a reset of
        // the preferences; `clear_natural_center` is how it's discarded.
        natural_center_cents: keep,
        advanced_open,
        ..default()
    };
}

/// Forgets the measured centre for the selected harp/hole, so readings there
/// go back to the equal-tempered table. The readiness check captures a new
/// one on demand; nothing here is permanent.
pub fn clear_natural_center(
    _: On<Activate>,
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    mut settings: ResMut<BendingTrainerSettings>,
) {
    settings
        .natural_center_cents
        .remove(&BendingTrainerSettings::center_key(key.name(), target.hole));
}

/// Builds the Advanced drawer: a floating panel anchored top-right of the
/// body, above the diagram. Its toggle lives in the strip
/// (`layout::spawn_strip`); its open state is the persisted
/// `BendingTrainerSettings::advanced_open`, mirrored onto the [`Drawer`] by
/// [`sync_advanced_drawer`].
pub(super) fn spawn_advanced_drawer(
    body: &mut ChildSpawnerCommands,
    loc: &Localization,
    settings: &BendingTrainerSettings,
) {
    body.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            width: Val::Px(340.0),
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(12.0)),
            display: Display::None,
            ..default()
        },
        // Opaque, and above the rest of the screen: an absolutely positioned
        // node still paints in tree order, and a floating panel has to win
        // outright wherever it overlaps. `GlobalZIndex(2)`, not `0`: the
        // gameplay root's own background carries `GlobalZIndex(1)` and would
        // otherwise paint straight over this (the trap
        // `spawn_gameplay_music_score` documents).
        BackgroundColor(Color::srgb(0.09, 0.09, 0.13)),
        GlobalZIndex(2),
        Drawer {
            open: settings.advanced_open,
        },
        AdvancedDrawer,
    ))
    .with_children(|drawer| {
        for knob in KNOBS {
            spawn_knob_row(drawer, loc, knob, settings);
        }
        drawer.spawn_empty().apply_scene(bsn! {
            Node {
                margin: {UiRect::top(Val::Px(6.0))},
            }
            Text({String::from(loc.msg("bending-adv-stability-heading"))})
            TextFont { font_size: {FontSize::Px(13.0)} }
            TextColor({Color::srgb(0.62, 0.66, 0.74)})
        });
        for readout in READOUTS {
            drawer.spawn((
                Text::new(String::new()),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.74, 0.78, 0.86)),
                readout,
            ));
        }
        drawer
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg("bending-adv-clear-center"),
                    clear_natural_center,
                ));
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg("bending-adv-reset"),
                    reset_advanced_settings,
                ));
            });
    });
}

/// One `− label value +` row. The value text carries the [`AdvancedKnob`]
/// so [`update_advanced_labels`] can rewrite it without a marker per knob.
fn spawn_knob_row(
    drawer: &mut ChildSpawnerCommands,
    loc: &Localization,
    knob: AdvancedKnob,
    settings: &BendingTrainerSettings,
) {
    drawer
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn_empty().apply_scene(button::small(
                "\u{2212}",
                move |_: On<Activate>, mut settings: ResMut<BendingTrainerSettings>| {
                    knob.nudge(&mut settings, -1.0);
                },
            ));
            row.spawn_empty().apply_scene(button::small(
                "+",
                move |_: On<Activate>, mut settings: ResMut<BendingTrainerSettings>| {
                    knob.nudge(&mut settings, 1.0);
                },
            ));
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                Text::new(String::from(loc.msg(knob.label_key()))),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.74, 0.82)),
            ));
            row.spawn((
                Text::new(knob.value_text(settings)),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.90, 0.96)),
                knob,
            ));
        });
}

/// Mirrors the persisted `advanced_open` onto the drawer; the [`Drawer`]
/// widget handles display and Tab order from there.
pub fn sync_advanced_drawer(
    settings: Res<BendingTrainerSettings>,
    mut drawers: Query<&mut Drawer, With<AdvancedDrawer>>,
) {
    if !settings.is_changed() {
        return;
    }
    for mut drawer in &mut drawers {
        if drawer.open != settings.advanced_open {
            drawer.open = settings.advanced_open;
        }
    }
}

/// Rewrites every knob's value text. Change-gated: these only move when the
/// player clicks a stepper.
pub fn update_advanced_labels(
    settings: Res<BendingTrainerSettings>,
    mut labels: Query<(&AdvancedKnob, &mut Text)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (knob, mut text) in &mut labels {
        let want = knob.value_text(&settings);
        if text.0 != want {
            text.0.clone_from(&want);
        }
    }
}

/// One readout line's text, or the em-dash placeholder when there isn't
/// enough signal to say anything. Pure, so the wording and the "when is
/// there nothing to report" rule are both testable without a `World`.
pub(super) fn readout_text(
    loc: &Localization,
    readout: AdvancedReadout,
    trace: &BendTrace,
    center_cents: Option<f32>,
) -> String {
    const NONE: &str = "—";
    let stability = attempt_stability(trace.samples());
    let value = match readout {
        AdvancedReadout::Mean => stability.map(|s| format!("{:+.1}c", s.mean_cents)),
        AdvancedReadout::Spread => stability.map(|s| format!("{:.1}c", s.spread_cents)),
        AdvancedReadout::BestHold => Some(format!("{:.2}s", trace.longest_centered_hold_secs)),
        AdvancedReadout::Vibrato => {
            vibrato(trace.samples()).map(|v| format!("{:.1} Hz · {:.0}c", v.rate_hz, v.depth_cents))
        }
        AdvancedReadout::Center => center_cents.map(|cents| format!("{cents:+.1}c")),
    };
    let key = match readout {
        AdvancedReadout::Mean => "bending-adv-mean",
        AdvancedReadout::Spread => "bending-adv-spread",
        AdvancedReadout::BestHold => "bending-adv-best-hold",
        AdvancedReadout::Vibrato => "bending-adv-vibrato",
        AdvancedReadout::Center => "bending-adv-center",
    };
    String::from(loc.msg_args(key, &[("value", value.unwrap_or_else(|| NONE.to_string()))]))
}

/// Keeps the measurement view live. Not change-gated — it reports the pitch
/// trace, which moves every frame the player is sounding a note.
pub fn update_advanced_readouts(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    trace: Res<BendTrace>,
    settings: Res<BendingTrainerSettings>,
    loc: Res<Localization>,
    added: Query<(), Added<AdvancedReadout>>,
    mut labels: Query<(&AdvancedReadout, &mut Text)>,
) {
    if !settings.advanced_open
        || (!key.is_changed()
            && !target.is_changed()
            && !trace.is_changed()
            && !settings.is_changed()
            && !loc.is_changed()
            && added.is_empty())
    {
        return;
    }
    let center = settings
        .natural_center_cents
        .get(&BendingTrainerSettings::center_key(key.name(), target.hole))
        .copied();
    for (readout, mut text) in &mut labels {
        let want = readout_text(&loc, *readout, &trace, center);
        if text.0 != want {
            text.0.clone_from(&want);
        }
    }
}
