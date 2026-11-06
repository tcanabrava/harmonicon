// SPDX-License-Identifier: MIT
//! Select flute targets in notation, with the instrument's actual range.
use super::*;
use bevy::ui::InteractionDisabled;
use harmonicon_ui::music_score::{BravuraFont, PitchChoiceInk, spawn_written_pitch_choices};

#[derive(Component)]
pub(crate) struct PitchChoice(u8);

impl NoteTrainer {
    fn select_flute_pitch(&mut self, midi: u8) -> bool {
        if self.instrument.fingering_chart().is_none() {
            return false;
        }
        let Some(index) = self.practice.drills.iter().position(|d| d.target.round() as u8 == midi)
        else {
            return false;
        };
        self.practice.index = index;
        self.practice.step_by(0);
        self.practice.auto_advance = false;
        self.practice.celebrate = 0.0;
        self.practice.restored = true;
        true
    }
}

/// Keep octave boundaries readable; the final C belongs with the top octave.
#[cfg(test)]
fn pitch_rows(flute: &Flute) -> Vec<Vec<u8>> {
    let Some((low, high)) = InstrumentModel::range(flute) else {
        return vec![];
    };
    let mut rows: Vec<Vec<u8>> = Vec::new();
    for pitch in low..=high {
        if rows.is_empty() || (pitch % 12 == 0 && pitch != high && pitch > 60) {
            rows.push(Vec::new());
        }
        rows.last_mut().unwrap().push(pitch);
    }
    rows
}

pub(super) fn spawn(
    parent: &mut ChildSpawnerCommands,
    font: &BravuraFont,
    flute: &Flute,
    loc: &Localization,
) {
    spawn_for_instrument(parent, font, &Instrument::Flute(*flute), loc);
}

pub(super) fn spawn_for_instrument(
    parent: &mut ChildSpawnerCommands,
    font: &BravuraFont,
    instrument: &Instrument,
    loc: &Localization,
) {
    let Some((low, high)) = instrument.range() else { return };
    let mut rows: Vec<Vec<(u8, u8)>> = Vec::new();
    for pitch in low..=high {
        let written = instrument.notation().transposition.written_semitone(pitch) as u8;
        if rows.is_empty() || (written.is_multiple_of(12) && pitch != high && pitch > low.saturating_add(1)) {
            rows.push(Vec::new());
        }
        rows.last_mut().unwrap().push((pitch, written));
    }
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|panel| {
            panel.spawn((
                Text::new(loc.msg("trainer-flute-pick-help").to_string()),
                TextFont { font_size: FontSize::Px(16.0), ..default() },
            ));
            let area = harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy(
                panel,
                Color::srgb(0.4, 0.55, 0.7),
                Color::srgb(0.1, 0.12, 0.16),
            );
            panel
                .commands()
                .entity(area)
                .insert(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_x(),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|score| {
                    for pitches in &rows {
                        for (midi, entity) in spawn_written_pitch_choices(score, font, pitches) {
                            score.commands().entity(entity).insert(PitchChoice(midi)).observe(
                                move |_: On<Activate>, mut trainer: ResMut<NoteTrainer>| {
                                    trainer.select_flute_pitch(midi);
                                },
                            );
                        }
                    }
                });
            panel
                .spawn_empty()
                .apply_scene(button::small(
                    &loc.msg("trainer-auto-off"),
                    |_: On<Activate>, mut trainer: ResMut<NoteTrainer>| {
                        trainer.practice.auto_advance = !trainer.practice.auto_advance;
                    },
                ))
                .insert(AutoLabel);
        });
}

#[derive(Component)]
pub(crate) struct AutoLabel;

pub(crate) fn update(
    mut commands: Commands,
    trainer: Res<NoteTrainer>,
    loc: Res<Localization>,
    mut choices: Query<(
        Entity,
        &PitchChoice,
        &mut button::BaseButtonColor,
        &mut BorderColor,
        Has<InteractionDisabled>,
    )>,
    mut inks: Query<(&PitchChoiceInk, Option<&mut TextColor>, Option<&mut BackgroundColor>)>,
    buttons: Query<&Children, With<AutoLabel>>,
    mut texts: Query<&mut Text>,
) {
    let target = trainer.practice.drill().map(|d| d.target.round() as u8);
    let start = trainer.practice.drill().and_then(|d| d.start).map(|midi| midi.round() as u8);
    for (entity, pitch, mut fill, mut border, disabled) in &mut choices {
        let available = trainer.practice.drills.iter().any(|d| d.target.round() as u8 == pitch.0);
        if available && disabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else if !available && !disabled {
            commands.entity(entity).insert(InteractionDisabled);
        }
        fill.0 = if !available {
            Color::srgba(0.04, 0.04, 0.06, 0.8)
        } else if target == Some(pitch.0) {
            Color::srgba(0.95, 0.75, 0.24, 0.22)
        } else {
            Color::NONE
        };
        *border = BorderColor::all(if available && target == Some(pitch.0) {
            Color::srgb(0.98, 0.82, 0.35)
        } else if start == Some(pitch.0) {
            Color::WHITE
        } else {
            Color::NONE
        });
    }
    for (pitch, text, background) in &mut inks {
        let available = trainer.practice.drills.iter().any(|d| d.target.round() as u8 == pitch.0);
        let color = if start == Some(pitch.0) {
            Color::WHITE
        } else if !available {
            Color::srgb(0.32, 0.34, 0.40)
        } else if target == Some(pitch.0) {
            Color::srgb(0.98, 0.82, 0.35)
        } else {
            Color::WHITE
        };
        if let Some(mut text) = text {
            text.0 = color;
        } else if let Some(mut background) = background {
            background.0 = color;
        }
    }
    for children in &buttons {
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.0 = loc
                    .msg(if trainer.practice.auto_advance {
                        "trainer-auto-on"
                    } else {
                        "trainer-auto-off"
                    })
                    .to_string();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::flute::FootJoint;
    #[test]
    fn score_contains_each_playable_pitch_once_for_both_foot_joints() {
        for foot in [FootJoint::C, FootJoint::B] {
            let flute = Flute::new(foot);
            let rows = pitch_rows(&flute);
            let (low, high) = InstrumentModel::range(&flute).unwrap();
            assert_eq!(rows.concat(), (low..=high).collect::<Vec<_>>());
            assert!(rows.iter().all(|r| r.len() <= 13));
        }
    }
    #[test]
    fn exercise_selection_rejects_unavailable_pitches_and_keeps_the_current_mode() {
        for kind in [TechniqueKind::Register, TechniqueKind::Position] {
            let mut trainer = NoteTrainer::for_instrument(Instrument::Flute(Flute::default()));
            trainer.kind = kind;
            trainer.practice = Practice::new(drills(&trainer.instrument, kind));
            let pitches: Vec<_> = trainer.practice.drills.iter().map(|d| d.target as u8).collect();
            for midi in pitches {
                assert!(trainer.select_flute_pitch(midi));
                assert_eq!(trainer.practice.drill().unwrap().kind, kind);
                assert_eq!(trainer.practice.drill().unwrap().target, f32::from(midi));
            }
            assert!(!trainer.select_flute_pitch(60));
        }
    }
    #[test]
    fn picking_every_note_resets_feedback_and_keeps_it_for_repeated_practice() {
        let mut trainer = NoteTrainer::for_instrument(Instrument::Flute(Flute::default()));
        let targets: Vec<_> = trainer.practice.drills.iter().map(|d| d.target as u8).collect();
        for midi in targets {
            trainer.practice.held = 0.4;
            assert!(trainer.select_flute_pitch(midi));
            assert_eq!(trainer.practice.drill().unwrap().target, f32::from(midi));
            assert!(!trainer.practice.auto_advance);
            assert_eq!(trainer.practice.held, 0.0);
        }
        let index = trainer.practice.index;
        assert!(!trainer.select_flute_pitch(0));
        assert_eq!(trainer.practice.index, index);
    }
}
