// SPDX-License-Identifier: MIT
//! A fretless fingerboard. The blue pitch marker is projected onto the requested
//! string; it is not a measurement of string choice or finger location.
use super::state::ActivePitches;
use super::{GameplayClock, SongNotes};
use bevy::prelude::*;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::instrument::bowed_strings::{
    BowArticulation, BowDirection, BowGesture, BowedStrings, FingerboardPosition,
};
use harmonicon_platform::localization::{Localization, LocalizationExt};

#[derive(Component)]
pub(crate) struct FingerboardView {
    pub setup: BowedStrings,
    pub targets: Vec<FingerboardPosition>,
    pub heard: Vec<f32>,
    pub bow: Option<BowGesture>,
}
#[derive(Component)]
pub(super) struct Marker {
    view: Entity,
    string: u8,
    heard: bool,
}
#[derive(Component)]
pub(super) struct MarkerLabel {
    view: Entity,
    string: u8,
}
#[derive(Component)]
pub(super) struct Caption(Entity);

pub(crate) fn spawn(parent: &mut ChildSpawnerCommands, setup: &BowedStrings, loc: &Localization) {
    let view = parent
        .spawn((
            FingerboardView {
                setup: setup.clone(),
                targets: Vec::new(),
                heard: Vec::new(),
                bow: None,
            },
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
        ))
        .id();
    parent.commands().entity(view).with_children(|panel| {
        panel.spawn((
            Caption(view),
            Text::new(""),
            TextFont { font_size: FontSize::Px(22.0), ..default() },
        ));
        panel
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(220.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.075, 0.065, 0.05)),
            ))
            .with_children(|board| {
                board.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(8.0),
                        width: Val::Px(3.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.8, 0.77, 0.64)),
                ));
                for (index, &pitch) in setup.strings.iter().enumerate() {
                    let string = index as u8 + 1;
                    let y = (index as f32 + 0.5) * 100.0 / setup.strings.len().max(1) as f32;
                    board.spawn((
                        Text::new(harmonicon_core::midi::midi_to_note(i32::from(pitch))),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(2.0),
                            top: Val::Percent(y),
                            ..default()
                        },
                    ));
                    board.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(8.0),
                            top: Val::Percent(y),
                            width: Val::Percent(88.0),
                            height: Val::Px(1.0 + index as f32 * 0.45),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.62, 0.64, 0.68)),
                    ));
                    for heard in [false, true] {
                        let size = if heard { 16.0 } else { 32.0 };
                        board
                            .spawn((
                                Marker { view, string, heard },
                                Visibility::Hidden,
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Percent(y),
                                    margin: UiRect::top(Val::Px(-size / 2.0)),
                                    width: Val::Px(size),
                                    height: Val::Px(size),
                                    border_radius: BorderRadius::MAX,
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                BackgroundColor(if heard {
                                    Color::srgb(0.25, 0.68, 1.0)
                                } else {
                                    Color::srgb(0.98, 0.80, 0.25)
                                }),
                            ))
                            .with_children(|dot| {
                                if !heard {
                                    dot.spawn((
                                        MarkerLabel { view, string },
                                        Text::new(""),
                                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                                        TextColor(Color::srgb(0.1, 0.09, 0.04)),
                                    ));
                                }
                            });
                    }
                }
            });
        panel.spawn((
            Text::new(loc.msg("bowed-board-help").to_string()),
            TextFont { font_size: FontSize::Px(13.0), ..default() },
            TextColor(Color::srgb(0.7, 0.75, 0.82)),
        ));
    });
}

fn nearest(heard: &[f32], target: u8) -> Option<f32> {
    heard
        .iter()
        .copied()
        .filter(|pitch| pitch.is_finite())
        .min_by(|a, b| (a - f32::from(target)).abs().total_cmp(&(b - f32::from(target)).abs()))
}

pub(super) fn paint(
    views: Query<&FingerboardView>,
    loc: Res<Localization>,
    mut markers: Query<(&Marker, &mut Node, &mut Visibility)>,
    mut labels: Query<
        (&mut Text, Option<&MarkerLabel>, Option<&Caption>),
        Or<(With<MarkerLabel>, With<Caption>)>,
    >,
) {
    for (marker, mut node, mut visible) in &mut markers {
        let Ok(view) = views.get(marker.view) else { continue };
        let target = view.targets.iter().find(|at| at.string == marker.string);
        let pitch = target.and_then(|at| {
            if marker.heard {
                nearest(&view.heard, at.note)
                    .filter(|heard| (heard - f32::from(at.note)).abs() < 1.0)
            } else {
                Some(f32::from(at.note))
            }
        });
        let position = pitch.and_then(|pitch| view.setup.stopping_fraction(marker.string, pitch));
        *visible = if position.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if let Some(position) = position {
            let end = 1.0 - 2.0_f32.powf(-f32::from(view.setup.stopped_semitones) / 12.0);
            node.left = Val::Percent(8.0 + 88.0 * position / end.max(0.001));
            node.margin.left = Val::Px(if marker.heard { -8.0 } else { -16.0 });
        }
    }
    for (mut text, marker, caption) in &mut labels {
        let next = if let Some(marker) = marker {
            let Ok(view) = views.get(marker.view) else { continue };
            view.targets
                .iter()
                .find(|at| at.string == marker.string)
                .and_then(|at| at.finger)
                .map_or_else(|| "·".into(), |finger| finger.to_string())
        } else if let Some(caption) = caption {
            let Ok(view) = views.get(caption.0) else { continue };
            let mut lines: Vec<String> = view
                .targets
                .iter()
                .map(|at| {
                    loc.msg_args(
                        "bowed-target",
                        &[
                            ("note", harmonicon_core::midi::midi_to_note(i32::from(at.note))),
                            ("string", at.string.to_string()),
                            (
                                "finger",
                                at.finger.map_or_else(|| "—".into(), |finger| finger.to_string()),
                            ),
                        ],
                    )
                    .to_string()
                })
                .collect();
            if let Some(bow) = view.bow {
                let direction = loc.msg(match bow.direction {
                    None => "bowed-unspecified",
                    Some(BowDirection::Down) => "bowed-down",
                    Some(BowDirection::Up) => "bowed-up",
                });
                let articulation = loc.msg(match bow.articulation {
                    BowArticulation::Detache => "bowed-detache",
                    BowArticulation::Legato => "bowed-legato",
                    BowArticulation::Staccato => "bowed-staccato",
                });
                lines.push(format!("{direction} · {articulation}"));
            }
            if let Some(target) = view.targets.first()
                && let Some(heard) = nearest(&view.heard, target.note)
            {
                if (heard - f32::from(target.note)).abs() < 1.0 {
                    lines.push(
                        loc.msg_args(
                            "bowed-cents",
                            &[(
                                "cents",
                                format!("{:+.0}", (heard - f32::from(target.note)) * 100.0),
                            )],
                        )
                        .to_string(),
                    );
                } else {
                    lines.push(
                        loc.msg_args(
                            "bowed-heard",
                            &[("note", harmonicon_core::midi::midi_to_note(heard.round() as i32))],
                        )
                        .to_string(),
                    );
                }
            }
            lines.join("\n")
        } else {
            continue;
        };
        if text.0 != next {
            text.0 = next;
        }
    }
}

pub(super) fn update_song(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    active: Res<ActivePitches>,
    mut views: Query<&mut FingerboardView>,
) {
    let now = clock.get();
    let current = notes.notes.iter().find(|note| {
        note.playable
            && !note.missed
            && note.time + note.duration > now
            && matches!(note.position, PlayPosition::Fingerboard(_))
    });
    for mut view in &mut views {
        view.targets = current
            .map(|current| {
                notes
                    .notes
                    .iter()
                    .filter(|note| note.playable && !note.missed && note.time == current.time)
                    .filter_map(|note| match note.position {
                        PlayPosition::Fingerboard(at) => Some(at),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        view.bow = current.and_then(|note| note.performance.bowed());
        view.heard = active
            .0
            .iter()
            .filter(|pitch| pitch.frequency > 0.0)
            .map(|pitch| 69.0 + 12.0 * (pitch.frequency / 440.0).log2())
            .collect();
    }
}
