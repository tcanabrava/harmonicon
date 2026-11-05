// SPDX-License-Identifier: MIT
//! A bend is a gesture from an established starting pitch, not a target alone.
use super::*;
use harmonicon_ui::dialogs::drawer::Drawer;

#[derive(Component)]
pub(crate) struct Panel;
#[derive(Component)]
pub(crate) struct Summary;
#[derive(Component)]
pub(crate) enum Visual {
    Sample(usize),
    Band,
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, loc: &Localization) {
    parent
        .spawn((
            Panel,
            Drawer { open: false },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(loc.msg("trainer-bend-steps").to_string()),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::WHITE),
            ));
            p.spawn((
                Summary,
                Text::new(""),
                TextFont { font_size: FontSize::Px(18.0), ..default() },
                TextColor(Color::WHITE),
            ));
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(112.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.08, 0.12)),
            ))
            .with_children(|plot| {
                plot.spawn((
                    Visual::Band,
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.3, 0.85, 0.5, 0.16)),
                ));
                for y in [15.0, 85.0] {
                    plot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Percent(y),
                            width: Val::Percent(100.0),
                            height: Val::Px(1.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.6, 0.7, 0.8, 0.6)),
                    ));
                }
                for index in 0..80 {
                    plot.spawn((
                        Visual::Sample(index),
                        Node {
                            position_type: PositionType::Absolute,
                            display: Display::None,
                            left: Val::Percent(index as f32 * 100.0 / 80.0),
                            width: Val::Percent(1.25),
                            height: Val::Px(3.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.5, 0.8, 1.0)),
                    ));
                }
            });
            p.spawn((
                Text::new(loc.msg("trainer-bend-trace-help").to_string()),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.7, 0.8, 0.9)),
            ));
        });
}

pub(crate) fn update(
    trainer: Res<NoteTrainer>,
    settings: Res<BendingTrainerSettings>,
    loc: Res<Localization>,
    mut panels: Query<&mut Drawer, With<Panel>>,
    mut labels: Query<&mut Text, With<Summary>>,
    mut visuals: Query<(&Visual, &mut Node)>,
) {
    let active = trainer.chord.is_none()
        && trainer.guitar().is_some()
        && trainer.kind == TechniqueKind::Glide;
    for mut panel in &mut panels {
        panel.open = active;
    }
    if !active {
        return;
    }
    let Some(drill) = trainer.practice.drill() else { return };
    let Some(start) = drill.start else { return };
    for mut label in &mut labels {
        label.0 = loc
            .msg_args(
                "trainer-bend-pitches",
                &[("start", note_name(start)), ("target", note_name(drill.target))],
            )
            .to_string();
    }
    let radius = settings.tolerance_cents / ((drill.target - start).abs() * 100.0) * 70.0;
    for (visual, mut node) in &mut visuals {
        match *visual {
            Visual::Band => {
                node.top = Val::Percent((15.0 - radius).max(0.0));
                node.height = Val::Percent((radius * 2.0).min(100.0));
            }
            Visual::Sample(index) => {
                let sample = trainer.practice.trace.get(index).copied().flatten();
                node.display = if sample.is_some() { Display::Flex } else { Display::None };
                if let Some(value) = sample {
                    node.top = Val::Percent((85.0 - 70.0 * value).clamp(0.0, 97.0));
                }
            }
        }
    }
}
