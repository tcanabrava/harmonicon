// SPDX-License-Identifier: MIT
//! Choosing a musical target without stepping through a long drill sequence.
use super::*;
use bevy::ui::RelativeCursorPosition;
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};

#[derive(Component)]
pub(crate) struct Controls;
#[derive(Component)]
pub(crate) struct Backdrops;

impl NoteTrainer {
    pub(super) fn select_guitar_position(&mut self, string: u8, fret: u8) -> bool {
        let current_technique = self.practice.drill().and_then(|d| match d.position {
            PlayPosition::Fret(at) => Some(at.technique),
            _ => None,
        });
        let matches = |d: &Drill| matches!(d.position, PlayPosition::Fret(at) if at.string == string && at.fret == fret);
        let preferred = self.practice.drills.iter().position(|d| matches(d)
            && matches!(d.position, PlayPosition::Fret(at) if Some(at.technique) == current_technique));
        let Some(index) = preferred.or_else(|| self.practice.drills.iter().position(matches))
        else {
            self.practice.selection_notice = Some((
                if self.kind == TechniqueKind::Glide && fret == 0 {
                    "trainer-bend-open-string"
                } else {
                    "trainer-position-unavailable"
                },
                2.5,
            ));
            return false;
        };
        self.practice.index = index;
        self.practice.step_by(0);
        self.practice.auto_advance = false;
        self.practice.celebrate = 0.0;
        self.practice.restored = true;
        self.practice.revealed = false;
        true
    }
}

pub(super) fn make_board_selectable(commands: &mut Commands, board: Entity) {
    commands
        .entity(board)
        .insert(RelativeCursorPosition::default())
        // not-a-widget-button: the fretboard is a pitch diagram; the note and
        // position comboboxes provide equivalent keyboard selection.
        .observe(
            move |_: On<bevy::picking::events::PointerClick>,
                  cursors: Query<&RelativeCursorPosition>,
                  mut trainer: ResMut<NoteTrainer>| {
                let Some(cursor) = cursors.get(board).ok().and_then(|p| p.normalized) else {
                    return;
                };
                let Some(guitar) = trainer.guitar() else { return };
                let physical = (cursor.x.clamp(0.0, 0.999) * f32::from(guitar.frets + 1)) as u8;
                let string =
                    (cursor.y.clamp(0.0, 0.999) * f32::from(guitar.string_count())) as u8 + 1;
                if let Some(fret) = physical.checked_sub(guitar.capo) {
                    if trainer.select_guitar_position(string, fret)
                        && trainer.kind == TechniqueKind::Position
                    {
                        trainer.practice.revealed = true;
                    }
                }
            },
        );
}

pub(crate) fn sync_controls(
    mut commands: Commands,
    trainer: Res<NoteTrainer>,
    loc: Res<Localization>,
    controls: Query<(Entity, Option<&Children>), With<Controls>>,
    overlays: Query<(Entity, Option<&Children>), With<Backdrops>>,
    mut last: Local<Option<(Entity, TechniqueKind, usize, bool)>>,
) {
    let Ok((host, children)) = controls.single() else { return };
    let Ok((overlay, overlay_children)) = overlays.single() else { return };
    let key = (host, trainer.kind, trainer.practice.index, trainer.practice.auto_advance);
    if *last == Some(key) && !loc.is_changed() {
        return;
    }
    *last = Some(key);
    for children in [children, overlay_children].into_iter().flatten() {
        for &child in children {
            commands.entity(child).despawn();
        }
    }
    let Some(drill) = trainer.practice.drill() else { return };
    let display_pitch = |d: &Drill| {
        if d.kind == TechniqueKind::Glide {
            d.start.unwrap_or(d.target).round() as u8
        } else {
            d.target.round() as u8
        }
    };
    let mut pitches: Vec<u8> = trainer.practice.drills.iter().map(display_pitch).collect();
    pitches.sort_unstable();
    pitches.dedup();
    let options: Vec<String> = pitches.iter().map(|&m| midi_to_note(i32::from(m))).collect();
    spawn_combobox(
        &mut commands,
        host,
        overlay,
        &loc.msg(if trainer.kind == TechniqueKind::Glide {
            "trainer-bend-start-pitch"
        } else {
            "trainer-choose-note"
        }),
        &options,
        &midi_to_note(i32::from(display_pitch(drill))),
        move |ev: On<ComboboxSelect>, mut trainer: ResMut<NoteTrainer>| {
            let Some(&pitch) = pitches.get(ev.index) else { return };
            let string = trainer.practice.drill().and_then(|d| match d.position {
                PlayPosition::Fret(at) => Some(at.string),
                _ => None,
            });
            let candidate = trainer
                .practice
                .drills
                .iter()
                .filter(|d| display_pitch(d) == pitch)
                .min_by_key(|d| match d.position {
                    PlayPosition::Fret(at) => (Some(at.string) != string, at.fret),
                    _ => (true, 0),
                })
                .copied();
            if let Some(Drill { position: PlayPosition::Fret(at), .. }) = candidate {
                trainer.select_guitar_position(at.string, at.fret);
            }
        },
    );
    let positions: Vec<FretAssignment> = trainer
        .practice
        .drills
        .iter()
        .filter(|d| display_pitch(d) == display_pitch(drill))
        .filter(|d| {
            trainer.kind != TechniqueKind::Glide
                || matches!((d.position, drill.position),
            (PlayPosition::Fret(a), PlayPosition::Fret(b)) if a.technique == b.technique)
        })
        .filter_map(|d| match d.position {
            PlayPosition::Fret(at) => Some(at),
            _ => None,
        })
        .collect();
    let label = |at: &FretAssignment| {
        if trainer.kind == TechniqueKind::Position {
            return loc
                .msg_args("trainer-string-only", &[("string", at.string.to_string())])
                .to_string();
        }
        loc.msg_args(
            "trainer-position",
            &[("string", at.string.to_string()), ("fret", at.fret.to_string())],
        )
        .to_string()
    };
    let options: Vec<String> = positions.iter().map(label).collect();
    let selected = match drill.position {
        PlayPosition::Fret(at) => label(&at),
        _ => String::new(),
    };
    spawn_combobox(
        &mut commands,
        host,
        overlay,
        &loc.msg("trainer-choose-position"),
        &options,
        &selected,
        move |ev: On<ComboboxSelect>, mut trainer: ResMut<NoteTrainer>| {
            if let Some(at) = positions.get(ev.index) {
                trainer.select_guitar_position(at.string, at.fret);
            }
        },
    );
    if trainer.kind == TechniqueKind::Glide {
        let options =
            [loc.msg("trainer-bend-half").to_string(), loc.msg("trainer-bend-whole").to_string()];
        let selected =
            match drill.position {
                PlayPosition::Fret(FretAssignment {
                    technique: StringTechnique::Bend(d), ..
                }) if d > 1.5 => 1,
                _ => 0,
            };
        spawn_combobox(
            &mut commands,
            host,
            overlay,
            &loc.msg("trainer-bend-interval"),
            &options,
            &options[selected],
            |ev: On<ComboboxSelect>, mut trainer: ResMut<NoteTrainer>| {
                let Some(Drill { position: PlayPosition::Fret(at), .. }) =
                    trainer.practice.drill().copied()
                else {
                    return;
                };
                let depth = if ev.index == 0 { 1.0 } else { 2.0 };
                let index = trainer.practice.drills.iter().position(|d| matches!(d.position,
                    PlayPosition::Fret(p) if p.string == at.string && p.fret == at.fret && p.technique == StringTechnique::Bend(depth)));
                if let Some(index) = index {
                    trainer.practice.index = index;
                    trainer.select_guitar_position(at.string, at.fret);
                }
            },
        );
    }
    commands.entity(host).with_children(|p| {
        p.spawn_empty().apply_scene(button::small(
            &loc.msg(if trainer.practice.auto_advance {
                "trainer-auto-on"
            } else {
                "trainer-auto-off"
            }),
            |_: On<Activate>, mut trainer: ResMut<NoteTrainer>| {
                trainer.practice.auto_advance = !trainer.practice.auto_advance;
                trainer.practice.awaiting_release = false;
                trainer.practice.step_by(0);
            },
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_a_position_pins_the_exercise_until_release_and_replay() {
        let mut trainer = NoteTrainer::default();
        assert!(trainer.select_guitar_position(3, 2));
        assert_eq!(trainer.practice.drill().unwrap().target, 57.0);
        assert!(!trainer.practice.auto_advance);
        let index = trainer.practice.index;
        assert!(trainer.practice.listen(Some(57.0), 1.0, 20.0, 0.6));
        assert_eq!(trainer.practice.index, index);
        assert!(!trainer.practice.listen(Some(57.0), 1.0, 20.0, 0.6));
        assert_eq!(
            trainer.practice.landed, 1,
            "holding one note must not count as repeated attempts"
        );
        trainer.practice.listen(None, 0.1, 20.0, 0.6);
        assert!(trainer.practice.listen(Some(57.0), 1.0, 20.0, 0.6));
        assert_eq!(trainer.practice.landed, 2);
    }
}
