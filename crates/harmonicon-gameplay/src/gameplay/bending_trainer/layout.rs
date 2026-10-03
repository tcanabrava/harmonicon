// SPDX-License-Identifier: MIT

//! Bending Trainer layout: a control strip, bend path, and selectable diagram.
//! The diagram moves beside or below the path as the window changes shape.
//! Drawers float above the body so opening one does not move the live trace.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::window::PrimaryWindow;
use harmonicon_ui::dialogs::drawer::Drawer;

use super::*;

/// Which way the screen is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrainerOrientation {
    Landscape,
    Portrait,
}

/// Portrait means taller than wide. A square window reads as landscape —
/// the side-by-side layout is the one designed for the wider screens this
/// game treats as its baseline.
pub(super) fn orientation_for(width: f32, height: f32) -> TrainerOrientation {
    if height > width { TrainerOrientation::Portrait } else { TrainerOrientation::Landscape }
}

/// Landscape puts the diagram beside the bend path; portrait stacks it
/// below.
pub(super) fn body_direction(orientation: TrainerOrientation) -> FlexDirection {
    match orientation {
        TrainerOrientation::Landscape => FlexDirection::Row,
        TrainerOrientation::Portrait => FlexDirection::Column,
    }
}

/// The body below the strip: bend path plus diagram.
#[derive(Component)]
pub struct TrainerBody;

/// The Setup drawer (key and detector).
#[derive(Component)]
pub struct SetupDrawer;

/// "C harp · FFT" in the strip — what the closed Setup drawer holds, so the
/// current key and detector are visible without opening it.
#[derive(Component)]
pub struct SetupSummary;

/// Wraps the Skip button: shown only while the drill runs. A [`Drawer`], so
/// the hidden button also leaves the Tab order.
#[derive(Component)]
pub struct SkipSlot;

/// Wraps the drill's explanation: shown while the drill is off, answering
/// "what does this button do" without a hover.
#[derive(Component)]
pub struct DrillIntroSlot;

/// "7 of 10 controlled" for the selected target — the text channel of the
/// progress map, which is otherwise tint and bar length.
#[derive(Component)]
pub struct TargetProgressLabel;

const CARD_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.85);
const DRAWER_BG: Color = Color::srgb(0.09, 0.09, 0.13);
const MUTED: Color = Color::srgb(0.70, 0.70, 0.80);

fn label(text: String, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color))
}

/// A horizontal cluster inside the strip or the card.
fn cluster() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        row_gap: Val::Px(6.0),
        ..default()
    }
}

/// The strip's summary of the Setup drawer's contents.
pub(super) fn setup_summary(loc: &Localization, key: &str, audio: &AudioSettings) -> String {
    String::from(loc.msg_args(
        "bending-setup-summary",
        &[("key", key.to_string()), ("algo", audio.pitch_algorithm.label().to_string())],
    ))
}

/// The selected target's record as text. `None` is "never practised", kept
/// distinct from "practised and never controlled" — the same distinction the
/// diagram's tint draws.
pub(super) fn progress_text(loc: &Localization, stat: Option<&DrillStat>) -> String {
    match stat.filter(|stat| stat.attempts > 0) {
        None => String::from(loc.msg("bending-progress-none")),
        Some(stat) => String::from(loc.msg_args(
            "bending-progress",
            &[("hits", stat.hits.to_string()), ("attempts", stat.attempts.to_string())],
        )),
    }
}

/// The top strip: Setup, Scope, Practice, tempo, Advanced. Wraps rather
/// than overflowing when a portrait tablet can't fit it on one line.
pub(super) fn spawn_strip(
    commands: &mut Commands,
    root_id: Entity,
    loc: &Localization,
    key: &str,
    audio: &AudioSettings,
    tempo: &MetronomeTempo,
) {
    let strip = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(28.0),
            row_gap: Val::Px(10.0),
            padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
            ..default()
        })
        .id();
    commands.entity(root_id).add_child(strip);
    commands.entity(strip).with_children(|strip| {
        strip.spawn(cluster()).with_children(|row| {
            row.spawn_empty()
                .apply_scene(button::small(&loc.msg("bending-setup-button"), toggle_setup_drawer));
            row.spawn((label(setup_summary(loc, key, audio), 14.0, MUTED), SetupSummary));
        });
        strip.spawn(cluster()).with_children(|row| {
            row.spawn_empty()
                .apply_scene(button::small(&loc.msg("bending-scope-button"), cycle_drill_scope));
            row.spawn((
                label(scope_status(loc, DrillScope::default(), 0), 14.0, MUTED),
                DrillScopeLabel,
            ));
        });
        strip.spawn(cluster()).with_children(|row| {
            row.spawn_empty()
                .apply_scene(button::small(&loc.msg("bending-shape-button"), cycle_practice_shape));
            row.spawn((
                label(String::from(loc.msg("bending-shape-free")), 14.0, MUTED),
                PracticeShapeLabel,
            ));
        });
        strip.spawn(cluster()).with_children(|row| {
            row.spawn_empty()
                .apply_scene(button::small(
                    "\u{2212}",
                    |_: On<Activate>, mut tempo: ResMut<MetronomeTempo>| {
                        tempo.bpm = (tempo.bpm - BPM_STEP).max(MIN_BPM);
                    },
                ))
                .insert(Tooltip(String::from(loc.msg("bending-tempo-decrease"))));
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|metro| {
                spawn_metronome(metro, loc, tempo.beats_per_bar(), tempo.bpm);
            });
            row.spawn_empty()
                .apply_scene(button::small(
                    "+",
                    |_: On<Activate>, mut tempo: ResMut<MetronomeTempo>| {
                        tempo.bpm = (tempo.bpm + BPM_STEP).min(MAX_BPM);
                    },
                ))
                .insert(Tooltip(String::from(loc.msg("bending-tempo-increase"))));
        });
        strip.spawn(cluster()).with_children(|row| {
            row.spawn_empty()
                .apply_scene(button::small(&loc.msg("bending-adv-toggle"), toggle_advanced_drawer));
        });
    });
}

/// The body: the bend-path card, the diagram, and the two floating drawers.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_body(
    commands: &mut Commands,
    root_id: Entity,
    loc: &Localization,
    key: &str,
    target: TrainerTarget,
    audio: &AudioSettings,
    settings: &BendingTrainerSettings,
    orientation: TrainerOrientation,
) {
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: body_direction(orientation),
                align_items: AlignItems::Center,
                column_gap: Val::Px(24.0),
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            TrainerBody,
        ))
        .id();
    commands.entity(root_id).add_child(body);
    commands.entity(body).with_children(|body| {
        body.spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            align_self: AlignSelf::Stretch,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|centre| spawn_target_card(centre, loc, key, target));

        body.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|side| {
            // The diagram is one Tab stop, navigated with the arrow keys once
            // focused — WAI-ARIA's grid pattern, the same "the group is the
            // stop" rule `dialogs::tab_bar` follows. Fifty cells as fifty Tab
            // stops would make every control after them unreachable in
            // practice.
            side.spawn((Node::default(), OverlayHost, TabIndex(0))).with_children(|host| {
                spawn_harmonica_overlay_selectable(
                    host,
                    &richter_harp(key),
                    on_diagram_cell_clicked,
                    loc,
                );
            });
            side.spawn((
                Node { width: Val::Px(320.0), padding: UiRect::all(Val::Px(8.0)), ..default() },
                BackgroundColor(CARD_BG),
            ))
            .with_children(|card| {
                card.spawn((
                    label(
                        technique_hint(loc, target.technique, target.hole),
                        15.0,
                        Color::srgb(0.75, 0.75, 0.85),
                    ),
                    HintLabel,
                ));
            });
        });

        spawn_advanced_drawer(body, loc, settings);
    });

    // The Setup drawer, top-left of the body. Built with `Commands` rather
    // than inside the closure above because `spawn_combobox` wants the
    // drawer's entity as its parent and the screen root as the backdrop's.
    let setup = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(DRAWER_BG),
            // Above the rest of the screen: the gameplay root's own background
            // is `GlobalZIndex(1)`, so `0` would paint underneath it.
            GlobalZIndex(2),
            Drawer { open: false },
            SetupDrawer,
        ))
        .id();
    commands.entity(body).add_child(setup);
    combobox::spawn_combobox(
        commands,
        setup,
        root_id,
        &loc.msg("bending-key-label"),
        &key_labels(),
        key,
        on_key_selected,
    );
    let algo_combo = combobox::spawn_combobox(
        commands,
        setup,
        root_id,
        &loc.msg("bending-detect-label"),
        &algo_labels(loc),
        audio.pitch_algorithm.label(),
        on_algo_selected,
    );
    attach_algo_tooltip(commands, algo_combo, audio.pitch_algorithm);
}

/// The centre: what is being practised, how close the player is, and every
/// action the current attempt needs.
fn spawn_target_card(
    centre: &mut ChildSpawnerCommands,
    loc: &Localization,
    key: &str,
    target: TrainerTarget,
) {
    centre
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_width: Val::Px(960.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(18.0)),
                ..default()
            },
            BackgroundColor(CARD_BG),
        ))
        .with_children(|card| {
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Baseline,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    label(
                        target_label_text(loc, target.hole, target.technique),
                        22.0,
                        Color::srgb(0.85, 0.92, 0.97),
                    ),
                    TargetLabel,
                ));
                row.spawn((label(progress_text(loc, None), 14.0, MUTED), TargetProgressLabel));
            });
            card.spawn((label(String::new(), 20.0, Color::srgb(0.55, 0.85, 0.60)), TunerReadout));
            spawn_bend_rail(card, loc);

            card.spawn(cluster()).with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg("bending-listen-natural-button"),
                    play_natural_reference,
                ));
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg("bending-listen-target-button"),
                    play_target_reference,
                ));
                row.spawn_empty()
                    .apply_scene(button::small(&loc.msg("bending-drill-button"), toggle_drill))
                    .insert(DrillToggleButton);
                row.spawn((Node::default(), Drawer { open: false }, SkipSlot)).with_children(
                    |slot| {
                        slot.spawn_empty().apply_scene(button::small(
                            &loc.msg("bending-skip-button"),
                            skip_drill_target,
                        ));
                    },
                );
                row.spawn((
                    label(String::from(loc.msg("bending-drill-off")), 15.0, MUTED),
                    DrillLabel,
                ));
            });
            card.spawn((Node::default(), Drawer { open: true }, DrillIntroSlot)).with_children(
                |slot| {
                    slot.spawn(label(
                        String::from(loc.msg("bending-drill-explanation")),
                        14.0,
                        Color::srgb(0.60, 0.60, 0.70),
                    ));
                },
            );

            card.spawn(cluster()).with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg("bending-check-natural-button"),
                    |_: On<Activate>, mut check: ResMut<NaturalCheck>| {
                        *check = NaturalCheck { requested: true, ..default() };
                    },
                ));
                row.spawn((
                    label(
                        String::from(
                            loc.msg_args(
                                "bending-check-natural-idle",
                                &[(
                                    "note",
                                    natural_note_for_target(&richter_harp(key), target)
                                        .unwrap_or_else(|| "?".to_string()),
                                )],
                            ),
                        ),
                        14.0,
                        Color::srgb(0.60, 0.60, 0.70),
                    ),
                    NaturalCheckLabel,
                ));
            });
        });
}

fn play_natural_reference(
    _: On<Activate>,
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
) {
    if let Some(note) = natural_note_for_target(key.harp(), *target) {
        play_reference_note(&note, &mut sources, &mut commands);
    }
}

fn play_target_reference(
    _: On<Activate>,
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
) {
    if let Some(note) = target_note(key.harp(), *target) {
        play_reference_note(&note, &mut sources, &mut commands);
    }
}

/// Opens or closes the Setup drawer. Not persisted, unlike Advanced: key
/// and detector are picked and the drawer shut, whereas an expert who works
/// with the precision view open wants it back next visit.
pub fn toggle_setup_drawer(_: On<Activate>, mut drawers: Query<&mut Drawer, With<SetupDrawer>>) {
    for mut drawer in &mut drawers {
        drawer.open = !drawer.open;
    }
}

/// Keeps the strip's "C harp · FFT" in step with the drawer's pickers.
pub fn update_setup_summary(
    key: Res<TrainerKey>,
    audio: Res<AudioSettings>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<SetupSummary>>,
) {
    if !key.is_changed() && !audio.is_changed() && !loc.is_changed() {
        return;
    }
    let summary = setup_summary(&loc, key.name(), &audio);
    for mut text in &mut labels {
        if text.0 != summary {
            text.0.clone_from(&summary);
        }
    }
}

/// Skip only exists while the drill runs; the explanation only while it
/// doesn't.
pub fn update_drill_slots(
    drill: Res<DrillState>,
    mut skip: Query<&mut Drawer, (With<SkipSlot>, Without<DrillIntroSlot>)>,
    mut intro: Query<&mut Drawer, (With<DrillIntroSlot>, Without<SkipSlot>)>,
) {
    if !drill.is_changed() {
        return;
    }
    for mut drawer in &mut skip {
        if drawer.open != drill.enabled {
            drawer.open = drill.enabled;
        }
    }
    for mut drawer in &mut intro {
        if drawer.open == drill.enabled {
            drawer.open = !drill.enabled;
        }
    }
}

/// Keeps the selected target's record line current.
pub fn update_target_progress(
    drill: Res<DrillState>,
    target: Res<TrainerTarget>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<TargetProgressLabel>>,
) {
    if !drill.is_changed() && !target.is_changed() && !loc.is_changed() {
        return;
    }
    let text = progress_text(&loc, drill.stats.get(&(target.hole, target.technique)));
    for mut label in &mut labels {
        if label.0 != text {
            label.0.clone_from(&text);
        }
    }
}

/// Follows the window's orientation live — see the module doc for why this
/// one, unlike `CompactLayout`, isn't read once at setup.
pub fn apply_trainer_orientation(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut bodies: Query<&mut Node, With<TrainerBody>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let direction = body_direction(orientation_for(window.width(), window.height()));
    for mut node in &mut bodies {
        if node.flex_direction != direction {
            node.flex_direction = direction;
        }
    }
}
