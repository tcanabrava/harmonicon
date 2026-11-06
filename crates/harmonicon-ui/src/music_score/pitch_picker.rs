// SPDX-License-Identifier: MIT
//! Static pitch choices reuse the score's engraving without its song resources.
use super::*;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::ui_widgets::Button;
use harmonicon_core::midi::midi_to_note;

#[derive(Component)]
pub struct PitchChoiceInk(pub u8);

/// Draw one treble staff of individually focusable pitch buttons. The caller
/// attaches selection handlers and supplies selection/availability styling.
pub fn spawn_pitch_choices(
    parent: &mut ChildSpawnerCommands,
    font: &BravuraFont,
    pitches: &[u8],
) -> Vec<(u8, Entity)> {
    let pitches: Vec<_> = pitches.iter().map(|&pitch| (pitch, pitch)).collect();
    spawn_written_pitch_choices(parent, font, &pitches)
}

/// Written staff coordinates paired with the sounding IDs returned to callers.
pub fn spawn_written_pitch_choices(
    parent: &mut ChildSpawnerCommands,
    font: &BravuraFont,
    pitches: &[(u8, u8)],
) -> Vec<(u8, Entity)> {
    let label_top = pitches
        .iter()
        .map(|&(_, written)| y_for_step(staff_step(written, Clef::Treble)) + 20.0)
        .fold(114.0_f32, f32::max);
    let mut choices = Vec::new();
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(600.0),
            height: Val::Px(label_top + 26.0),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(42.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|clef| {
                clef.spawn(super::glyph(
                    font,
                    Clef::Treble.glyph(),
                    4.0,
                    y_for_step(2),
                    Color::WHITE,
                ));
            });
            for &(midi, written) in pitches {
                let id = row
                    .spawn((
                        Button,
                        TabIndex(0),
                        Node {
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            height: Val::Percent(100.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|cell| {
                        // Decorative children cannot intercept the note's hit target.
                        cell.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(50.0),
                                width: Val::Px(0.0),
                                ..default()
                            },
                            bevy::picking::Pickable::IGNORE,
                            PitchChoiceInk(midi),
                        ))
                        .with_children(|marks| {
                            let step = staff_step(written, Clef::Treble);
                            for ledger in ledger_line_steps(step) {
                                marks.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(-11.0),
                                        top: Val::Px(y_for_step(ledger)),
                                        width: Val::Px(22.0),
                                        height: Val::Px(1.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::WHITE),
                                    bevy::picking::Pickable::IGNORE,
                                    PitchChoiceInk(midi),
                                ));
                            }
                            marks.spawn((
                                super::glyph(
                                    font,
                                    glyph::NOTEHEAD_BLACK,
                                    -5.0,
                                    y_for_step(step),
                                    Color::WHITE,
                                ),
                                bevy::picking::Pickable::IGNORE,
                                PitchChoiceInk(midi),
                            ));
                            if needs_sharp(written) {
                                marks.spawn((
                                    super::glyph(
                                        font,
                                        glyph::ACCIDENTAL_SHARP,
                                        -18.0,
                                        y_for_step(step),
                                        Color::WHITE,
                                    ),
                                    bevy::picking::Pickable::IGNORE,
                                    PitchChoiceInk(midi),
                                ));
                            }
                            marks.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(if step < 4 { 5.0 } else { -5.0 }),
                                    top: Val::Px(
                                        y_for_step(step) - if step < 4 { 27.0 } else { 0.0 },
                                    ),
                                    width: Val::Px(1.0),
                                    height: Val::Px(27.0),
                                    ..default()
                                },
                                BackgroundColor(Color::WHITE),
                                bevy::picking::Pickable::IGNORE,
                                PitchChoiceInk(midi),
                            ));
                        });
                        cell.spawn((
                            Text::new(midi_to_note(i32::from(written))),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                            TextLayout::justify(Justify::Center),
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(label_top),
                                width: Val::Percent(100.0),
                                ..default()
                            },
                            bevy::picking::Pickable::IGNORE,
                            PitchChoiceInk(midi),
                        ));
                    })
                    .id();
                crate::dialogs::button::make_interactive(
                    &mut row.commands().entity(id),
                    Color::srgb(0.07, 0.08, 0.11),
                );
                let name = midi_to_note(i32::from(written));
                row.commands().queue(move |world: &mut World| {
                    if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(id) {
                        node.set_label(name);
                    }
                });
                choices.push((midi, id));
            }
            for line in 0..5 {
                row.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(STAFF_TOP_MARGIN + line as f32 * STAFF_LINE_SPACING),
                        width: Val::Percent(100.0),
                        height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.7, 0.72, 0.78, 0.6)),
                    bevy::picking::Pickable::IGNORE,
                ));
            }
        });
    choices
}
