// SPDX-License-Identifier: MIT
//! Wind finger-contact diagrams shared by the trainer and song view. Coordinates
//! describe a Boehm clarinet schematic, not mechanically linked pad positions.
use super::{GameplayClock, SongNotes};
use bevy::prelude::*;
use harmonicon_core::instrument::clarinet::Clarinet;
use harmonicon_core::instrument::wind::{ControlKind, FingeringAssignment, FingeringChart};
use harmonicon_platform::localization::{Localization, LocalizationExt};

#[derive(Component)]
pub(super) struct ClarinetDiagram {
    pub setup: Clarinet,
    pub horizontal: bool,
    pub next: bool,
    pub last: Option<FingeringAssignment>,
    pub initialized: bool,
}

pub(super) fn spawn_slot(parent: &mut ChildSpawnerCommands, setup: Clarinet, next: bool) {
    parent.spawn((
        ClarinetDiagram { setup, horizontal: false, next, last: None, initialized: false },
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            min_width: Val::Px(if next { 250.0 } else { 400.0 }),
            ..default()
        },
    ));
}

/// Play uses a horizontal instrument; the trainer keeps its upright contact map.
pub(super) fn spawn_play_slot(parent: &mut ChildSpawnerCommands, setup: Clarinet, next: bool) {
    parent.spawn((
        ClarinetDiagram { setup, horizontal: true, next, last: None, initialized: false },
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(if next { 200.0 } else { 330.0 }),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

#[derive(Component)]
pub(super) struct ClarinetTiming;

fn horizontal_position(id: u8) -> Option<(f32, f32)> {
    Some(match id {
        0 => (205.0, 48.0),
        1 => (285.0, 48.0),
        2..=4 => (240.0 + f32::from(id - 2) * 80.0, 120.0),
        5..=7 => (570.0 + f32::from(id - 5) * 80.0, 120.0),
        8..=11 => (380.0 + f32::from((id - 8) % 2) * 80.0, 180.0 + f32::from((id - 8) / 2) * 36.0),
        12..=15 => {
            (780.0 + f32::from((id - 12) % 2) * 80.0, 180.0 + f32::from((id - 12) / 2) * 36.0)
        }
        16 => (220.0, 180.0),
        17 => (300.0, 180.0),
        18 => (650.0, 48.0),
        _ => return None,
    })
}

fn spawn_horizontal_body(parent: &mut ChildSpawnerCommands) {
    // Original schematic: mouthpiece, barrel, jointed body and flared bell.
    for (x, y, width, height, ink) in [
        (30.0, 112.0, 65.0, 16.0, Color::srgb(0.45, 0.49, 0.55)),
        (90.0, 108.0, 90.0, 24.0, Color::srgb(0.18, 0.21, 0.26)),
        (175.0, 108.0, 660.0, 24.0, Color::srgb(0.18, 0.21, 0.26)),
        (830.0, 106.0, 95.0, 28.0, Color::srgb(0.18, 0.21, 0.26)),
        (920.0, 98.0, 55.0, 44.0, Color::srgb(0.25, 0.29, 0.34)),
    ] {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(x / 10.0),
                top: Val::Percent(y / 2.6),
                width: Val::Percent(width / 10.0),
                height: Val::Percent(height / 2.6),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(ink),
        ));
    }
    for x in [170.0, 480.0, 835.0, 920.0] {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(x / 10.0),
                top: Val::Percent(106.0 / 2.6),
                width: Val::Px(2.0),
                height: Val::Percent(28.0 / 2.6),
                ..default()
            },
            BackgroundColor(Color::srgb(0.55, 0.59, 0.65)),
        ));
    }
}

fn draw(
    parent: &mut ChildSpawnerCommands,
    setup: &Clarinet,
    at: Option<FingeringAssignment>,
    scale: f32,
    horizontal: bool,
) {
    let pressed = at.and_then(|at| setup.pattern(at)).unwrap_or_default();
    parent
        .spawn((
            Node {
                width: if horizontal { Val::Percent(100.0) } else { Val::Px(400.0 * scale) },
                max_width: if horizontal { Val::Px(1000.0 * scale) } else { Val::Auto },
                aspect_ratio: horizontal.then_some(1000.0 / 260.0),
                height: if horizontal { Val::Auto } else { Val::Px(370.0 * scale) },
                flex_shrink: 0.0,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
        ))
        .with_children(|drawing| {
            if horizontal {
                spawn_horizontal_body(drawing);
            } else {
                drawing.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(192.0 * scale),
                        top: Val::Px(24.0 * scale),
                        width: Val::Px(16.0 * scale),
                        height: Val::Px(325.0 * scale),
                        border_radius: BorderRadius::all(Val::Px(6.0 * scale)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.21, 0.26)),
                ));
                drawing.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(190.0 * scale),
                        width: Val::Px(20.0 * scale),
                        height: Val::Px(28.0 * scale),
                        border_radius: BorderRadius::all(Val::Px(4.0 * scale)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.45, 0.49, 0.55)),
                ));
            }
            for control in setup.controls() {
                let (x, y) = if horizontal {
                    let Some(point) = horizontal_position(control.id.0) else { continue };
                    point
                } else {
                    match control.id.0 {
                        0 => (70.0, 48.0),
                        1 => (125.0, 30.0),
                        2..=4 => (200.0, 80.0 + f32::from(control.id.0 - 2) * 45.0),
                        5..=7 => (200.0, 235.0 + f32::from(control.id.0 - 5) * 45.0),
                        8..=11 => (
                            50.0 + f32::from((control.id.0 - 8) % 2) * 75.0,
                            175.0 + f32::from((control.id.0 - 8) / 2) * 34.0,
                        ),
                        12..=15 => (
                            275.0 + f32::from((control.id.0 - 12) % 2) * 75.0,
                            300.0 + f32::from((control.id.0 - 12) / 2) * 34.0,
                        ),
                        16 => (125.0, 82.0),
                        17 => (125.0, 116.0),
                        18 => (285.0, 235.0),
                        _ => continue,
                    }
                };
                let hole = control.kind == ControlKind::MainHole;
                let width = if hole { 30.0 } else { 68.0 };
                let active = at.is_some() && pressed.contains(control.id);
                let fill = if active {
                    Color::srgb(0.98, 0.80, 0.25)
                } else {
                    Color::srgb(0.07, 0.08, 0.11)
                };
                let label = if control.id.0 == 0 {
                    "T"
                } else if control.id.0 == 1 {
                    "Register"
                } else {
                    control.label
                };
                drawing
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: if horizontal {
                                Val::Percent((x - width * 0.5) / 10.0)
                            } else {
                                Val::Px((x - width * 0.5) * scale)
                            },
                            top: if horizontal {
                                Val::Percent((y - 15.0) / 2.6)
                            } else {
                                Val::Px((y - 15.0) * scale)
                            },
                            width: if horizontal {
                                Val::Percent(width / 10.0)
                            } else {
                                Val::Px(width * scale)
                            },
                            height: if horizontal {
                                Val::Percent(30.0 / 2.6)
                            } else {
                                Val::Px(30.0 * scale)
                            },
                            border: UiRect::all(Val::Px(scale)),
                            border_radius: BorderRadius::MAX,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(fill),
                        BorderColor::all(Color::srgb(0.76, 0.81, 0.88)),
                    ))
                    .with_child((
                        Text::new(label.replace('♯', "#").replace('♭', "b")),
                        TextFont { font_size: FontSize::Px(11.0 * scale), ..default() },
                        TextColor(if active {
                            Color::srgb(0.10, 0.09, 0.04)
                        } else {
                            Color::WHITE
                        }),
                    ));
            }
        });
}

pub(super) fn rebuild(
    mut commands: Commands,
    loc: Res<Localization>,
    mut slots: Query<(Entity, &mut ClarinetDiagram)>,
) {
    for (entity, mut slot) in &mut slots {
        if slot.initialized {
            continue;
        }
        slot.initialized = true;
        let at = slot.last;
        let setup = slot.setup;
        let next = slot.next;
        let horizontal = slot.horizontal;
        commands.entity(entity).insert(if at.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        commands.entity(entity).despawn_children().with_children(|parent| {
            let written = at.map(|at| {
                harmonicon_core::midi::midi_to_note(i32::from(
                    setup.notation().transposition.written_semitone(at.note),
                ))
            });
            parent.spawn((
                Text::new(
                    loc.msg_args(
                        if next { "clarinet-next" } else { "clarinet-now" },
                        &[("note", written.unwrap_or_else(|| "—".into()))],
                    )
                    .to_string(),
                ),
                TextFont { font_size: FontSize::Px(if next { 16.0 } else { 22.0 }), ..default() },
            ));
            draw(parent, &setup, at, if next { 0.6 } else { 1.0 }, horizontal);
            if horizontal && !next {
                parent.spawn((
                    ClarinetTiming,
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::srgb(0.8, 0.85, 0.92)),
                ));
            }
        });
    }
}

pub(super) fn update_song(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    mut slots: Query<&mut ClarinetDiagram>,
    loc: Res<Localization>,
    mut timing: Query<&mut Text, With<ClarinetTiming>>,
) {
    let now = clock.get();
    let current = notes
        .notes
        .iter()
        .find(|note| note.playable && !note.missed && note.time + note.duration > now);
    let cue = current
        .map(|note| {
            if now < note.time {
                loc.msg_args(
                    "clarinet-starts-in",
                    &[("seconds", format!("{:.1}", note.time - now))],
                )
                .to_string()
            } else if note.hit {
                loc.msg_args(
                    "clarinet-hold",
                    &[("seconds", format!("{:.1}", (note.time + note.duration - now).max(0.0)))],
                )
                .to_string()
            } else {
                loc.msg("clarinet-play-now").to_string()
            }
        })
        .unwrap_or_default();
    for mut label in &mut timing {
        if label.0 != cue {
            label.0.clone_from(&cue);
        }
    }
    let at = current.and_then(|note| match note.position {
        harmonicon_core::instrument::PlayPosition::Fingering(at) => Some(at),
        _ => None,
    });
    let next = current.and_then(|current| notes.notes.iter().find(|note| note.playable && !note.missed && note.time > current.time && matches!(note.position, harmonicon_core::instrument::PlayPosition::Fingering(other) if Some(other) != at)))
        .and_then(|note| match note.position { harmonicon_core::instrument::PlayPosition::Fingering(at) => Some(at), _ => None });
    for mut slot in &mut slots {
        let target = if slot.next { next } else { at };
        if target != slot.last {
            slot.last = target;
            slot.initialized = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::PlayPosition;
    #[test]
    fn every_contact_fits_the_horizontal_canvas() {
        for control in Clarinet::default().controls() {
            let (x, y) = horizontal_position(control.id.0).expect("every control must be visible");
            let half_width = if control.kind == ControlKind::MainHole { 15.0 } else { 34.0 };
            assert!(x >= half_width && x + half_width <= 1000.0);
            assert!(y >= 15.0 && y + 15.0 <= 260.0);
        }
    }
    #[test]
    fn play_preview_skips_repeated_and_unplayable_notes_and_clears_at_the_end() {
        let at = |note| FingeringAssignment { note, alternate: 0 };
        let note = |time, pitch, playable| super::super::ScheduledNote {
            time,
            duration: 0.8,
            position: PlayPosition::Fingering(at(pitch)),
            playable,
            ..default()
        };
        let mut app = App::new();
        app.init_resource::<GameplayClock>()
            .init_resource::<Localization>()
            .insert_resource(SongNotes {
                notes: vec![
                    note(0.0, 82, false),
                    note(0.0, 50, true),
                    note(1.0, 50, true),
                    note(2.0, 53, true),
                ],
                cursor: 0,
            })
            .add_systems(Update, update_song);
        let now = app
            .world_mut()
            .spawn(ClarinetDiagram {
                setup: Clarinet::default(),
                horizontal: true,
                next: false,
                last: None,
                initialized: false,
            })
            .id();
        let next = app
            .world_mut()
            .spawn(ClarinetDiagram {
                setup: Clarinet::default(),
                horizontal: true,
                next: true,
                last: None,
                initialized: false,
            })
            .id();
        app.update();
        assert_eq!(app.world().get::<ClarinetDiagram>(now).unwrap().last, Some(at(50)));
        assert_eq!(app.world().get::<ClarinetDiagram>(next).unwrap().last, Some(at(53)));
        app.world_mut().resource_mut::<GameplayClock>().set_free(2.1);
        app.update();
        assert_eq!(app.world().get::<ClarinetDiagram>(now).unwrap().last, Some(at(53)));
        assert_eq!(app.world().get::<ClarinetDiagram>(next).unwrap().last, None);
        app.world_mut().resource_mut::<GameplayClock>().set_free(3.0);
        app.update();
        assert_eq!(app.world().get::<ClarinetDiagram>(now).unwrap().last, None);
    }
}
